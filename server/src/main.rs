//! ribbit-server — Ribbit's speech pipeline behind one HTTP endpoint.
//!
//! A device that can record but can't run Ribbit (the Steam Deck, for WoW
//! chat) POSTs a WAV and gets the finished text back: the same provider
//! stacks, failover, hallucination cleanup, LLM editor and vocabulary the
//! desktop app uses, via `ribbit_core::pipeline::run`.
//!
//! Configuration is Ribbit's own layout under `$XDG_CONFIG_HOME/ribbit/`
//! (`config.json`, `.env`, `vocab.json`), so the server is set up by copying
//! the app's files. Security model: listen only on a private-network address
//! (`RIBBIT_LISTEN`), plus a bearer token checked against its SHA-256 hash
//! (`RIBBIT_TOKEN_SHA256_FILE`). Audio and text are never stored or logged —
//! log lines carry sizes and timings only, as the app's session log does.

use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ribbit_core::pipeline;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// One dictation is a chat line, not a lecture: a minute of 16 kHz mono
/// 16-bit WAV is ~1.9 MB. Anything longer is refused before it's decoded.
const MAX_AUDIO_SECS: f32 = 60.0;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// Same floor the app uses: shorter than this is a key bounce, not speech.
const MIN_AUDIO_SECS: f32 = 0.3;
/// Same silence floor the app uses (RMS of the normalised samples).
const SILENCE_RMS: f32 = 0.001;
/// Requests per minute per server — one player talking, with room to spare.
/// It exists so a leaked token can't turn into a bill.
const RATE_PER_MIN: usize = 30;
/// Whole-request ceiling. The pipeline has per-provider timeouts; this stops
/// a stack walk from holding the client past the point the text is useful.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
/// Ceiling on each of `context` and `rules`. Whisper reads at most 224 prompt
/// tokens; a glossary plus a party's names fits in a fraction of this.
const MAX_EXTRA_CHARS: usize = 1000;

struct AppState {
    token_sha256: [u8; 32],
    cfg: serde_json::Value,
    languages: Vec<String>,
    recent: Mutex<VecDeque<Instant>>,
}

#[tokio::main]
async fn main() {
    let listen: SocketAddr = std::env::var("RIBBIT_LISTEN")
        .expect("RIBBIT_LISTEN (private-network ip:port) is required")
        .parse()
        .expect("RIBBIT_LISTEN must be ip:port");
    let token_file = std::env::var("RIBBIT_TOKEN_SHA256_FILE")
        .expect("RIBBIT_TOKEN_SHA256_FILE is required");
    let token_sha256 = parse_hash(&std::fs::read_to_string(&token_file).expect("token hash file unreadable"));

    let dir = std::env::var("XDG_CONFIG_HOME").expect("XDG_CONFIG_HOME is required");
    let dir = std::path::Path::new(&dir).join("ribbit");
    pipeline::load_env_file(&dir.join(".env"), true);
    let cfg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("config.json")).expect("ribbit/config.json unreadable"),
    )
    .expect("ribbit/config.json is not JSON");
    let languages = cfg["languages"]
        .as_array()
        .expect("config.json: languages must be a list")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let state = Arc::new(AppState { token_sha256, cfg, languages, recent: Mutex::new(VecDeque::new()) });
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/transcribe", post(transcribe))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen).await.expect("bind failed");
    eprintln!("ribbit-server listening on {}", listen);
    axum::serve(listener, app)
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await
        .expect("server failed");
}

fn parse_hash(s: &str) -> [u8; 32] {
    hex::decode(s.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .expect("token hash must be 64 hex chars (sha256)")
}

fn authorized(headers: &HeaderMap, expected: &[u8; 32]) -> bool {
    let Some(token) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return false;
    };
    let got: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    got.ct_eq(expected).into()
}

/// Sliding one-minute window; `true` admits the request and records it.
fn admit(recent: &Mutex<VecDeque<Instant>>, now: Instant) -> bool {
    let mut q = recent.lock().unwrap();
    while q.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60)) {
        q.pop_front();
    }
    if q.len() >= RATE_PER_MIN {
        return false;
    }
    q.push_back(now);
    true
}

/// WAV bytes → mono f32 samples + rate. Channels are averaged; integer and
/// float PCM both accepted, so any recorder's default output works.
fn decode_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32), String> {
    let reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|e| format!("not a WAV: {}", e))?;
    let spec = reader.spec();
    let frames = reader.duration() as f32 / spec.sample_rate as f32;
    if frames > MAX_AUDIO_SECS {
        return Err(format!("audio longer than {}s", MAX_AUDIO_SECS));
    }
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.into_samples::<f32>().collect::<Result<_, _>>(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect()
        }
    }
    .map_err(|e| format!("bad WAV data: {}", e))?;
    let ch = spec.channels as usize;
    let mono = interleaved.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
    Ok((mono, spec.sample_rate))
}

fn fail(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

/// `?context=` (words the speech model should expect) and `?rules=` (extra
/// instructions for the editor) let the client say where the text is going —
/// the WoW chat sends its glossary and "one language per line". Both optional.
async fn transcribe(
    State(st): State<Arc<AppState>>,
    Query(extra): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !authorized(&headers, &st.token_sha256) {
        return fail(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if extra.values().any(|v| v.chars().count() > MAX_EXTRA_CHARS) {
        return fail(StatusCode::UNPROCESSABLE_ENTITY, "context or rules too long");
    }
    let context = extra.get("context").cloned();
    let rules = extra.get("rules").cloned();
    if !admit(&st.recent, Instant::now()) {
        return fail(StatusCode::TOO_MANY_REQUESTS, "rate limit");
    }
    let (samples, rate) = match decode_wav(&body) {
        Ok(v) => v,
        Err(e) => return fail(StatusCode::UNPROCESSABLE_ENTITY, &e),
    };
    let audio_secs = samples.len() as f32 / rate as f32;
    let rms = if samples.is_empty() {
        0.0
    } else {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    };
    if audio_secs < MIN_AUDIO_SECS || rms < SILENCE_RMS {
        // Not an error for the caller: nothing was said, nothing to paste.
        return Json(serde_json::json!({ "text": "", "audio_secs": audio_secs })).into_response();
    }

    let t0 = Instant::now();
    let st2 = st.clone();
    let job = tokio::task::spawn_blocking(move || {
        pipeline::run(&samples, rate, &st2.languages, context.as_deref(), rules.as_deref(), &st2.cfg)
    });
    match tokio::time::timeout(REQUEST_TIMEOUT, job).await {
        Ok(Ok(Ok(t))) => {
            eprintln!(
                "ok audio={:.1}s stt={:.1}s llm={} total={:.1}s chars={} edited={}",
                audio_secs,
                t.stt_secs,
                t.llm_secs.map(|s| format!("{:.1}s", s)).unwrap_or_else(|| "off".into()),
                t0.elapsed().as_secs_f32(),
                t.text.chars().count(),
                t.edited,
            );
            Json(serde_json::json!({ "text": t.text, "edited": t.edited, "audio_secs": audio_secs }))
                .into_response()
        }
        Ok(Ok(Err(e))) => {
            eprintln!("stt failed after {:.1}s: {}", t0.elapsed().as_secs_f32(), e);
            fail(StatusCode::BAD_GATEWAY, "speech-to-text failed")
        }
        Ok(Err(e)) => {
            eprintln!("pipeline panicked: {}", e);
            fail(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
        Err(_) => {
            eprintln!("timeout after {:?}", REQUEST_TIMEOUT);
            fail(StatusCode::GATEWAY_TIMEOUT, "timeout")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
        let spec = hound::WavSpec { channels, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut buf = Cursor::new(Vec::new());
        let mut w = hound::WavWriter::new(&mut buf, spec).unwrap();
        for s in samples {
            w.write_sample(*s).unwrap();
        }
        w.finalize().unwrap();
        buf.into_inner()
    }

    #[test]
    fn stereo_int_wav_decodes_to_mono_unit_range() {
        let (mono, rate) = decode_wav(&wav(&[16384, 0, -32768, -32768], 16000, 2)).unwrap();
        assert_eq!(rate, 16000);
        assert_eq!(mono, vec![0.25, -1.0]);
    }

    #[test]
    fn audio_over_the_cap_is_refused_before_decoding() {
        let long = vec![0i16; 16000 * 61];
        assert!(decode_wav(&wav(&long, 16000, 1)).unwrap_err().contains("longer"));
    }

    #[test]
    fn garbage_is_not_a_wav() {
        assert!(decode_wav(b"hello").is_err());
    }

    #[test]
    fn token_is_checked_by_hash_and_scheme() {
        let expected: [u8; 32] = Sha256::digest(b"s3cret").into();
        let mut h = HeaderMap::new();
        assert!(!authorized(&h, &expected));
        h.insert(header::AUTHORIZATION, "Bearer wrong".parse().unwrap());
        assert!(!authorized(&h, &expected));
        h.insert(header::AUTHORIZATION, "s3cret".parse().unwrap());
        assert!(!authorized(&h, &expected));
        h.insert(header::AUTHORIZATION, "Bearer s3cret".parse().unwrap());
        assert!(authorized(&h, &expected));
    }

    #[test]
    fn rate_window_admits_limit_then_refuses_then_recovers() {
        let q = Mutex::new(VecDeque::new());
        let t = Instant::now();
        for _ in 0..RATE_PER_MIN {
            assert!(admit(&q, t));
        }
        assert!(!admit(&q, t + Duration::from_secs(30)));
        assert!(admit(&q, t + Duration::from_secs(61)));
    }
}

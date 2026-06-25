use std::io::Cursor;
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            // Generous: Groq's free STT tier sometimes queues a request for tens
            // of seconds. A tight cap here just *loses the dictation* (a timed-out
            // STT call has no text to fall back to), which is worse than waiting —
            // the user has to repeat themselves. So we wait it out; the real fix
            // for the slowness is upstream (16kHz downsample below + faster tier),
            // not a short deadline that drops audio on the floor.
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Pre-initialize the HTTP client so first request doesn't pay TLS cost
pub fn warm_up_client() {
    let _ = client();
    crate::debug_log::log("HTTP client warmed up");
}

/// Connection defaults for one OpenAI-compatible `/audio/transcriptions`
/// endpoint. The catalog the "+ add provider" UI prefills audio entries from;
/// url/model stay editable per entry afterward.
pub struct AudioProvider {
    pub name: &'static str,
    pub label: &'static str,
    pub url: &'static str,
    pub default_model: &'static str,
    pub key_env: &'static str,
}

/// Audio providers Ribbit knows about. Both speak the OpenAI multipart audio
/// API, so a stack of them is uniform — that uniformity is what makes fallback
/// across them work in the first place.
pub const AUDIO_PROVIDERS: &[AudioProvider] = &[
    AudioProvider {
        name: "groq",
        label: "groq",
        url: "https://api.groq.com/openai/v1/audio/transcriptions",
        default_model: "whisper-large-v3-turbo",
        key_env: "GROQ_API_KEY",
    },
    AudioProvider {
        name: "openai",
        label: "openai",
        url: "https://api.openai.com/v1/audio/transcriptions",
        default_model: "whisper-1",
        key_env: "OPENAI_API_KEY",
    },
];

pub fn find_audio_provider(name: &str) -> Option<&'static AudioProvider> {
    AUDIO_PROVIDERS.iter().find(|p| p.name == name)
}

/// Encode f32 PCM audio data as WAV bytes
fn encode_wav(audio_data: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    let bits_per_sample = 16u16;
    let channels = 1u16;
    let byte_rate = sample_rate * (bits_per_sample as u32 / 8) * channels as u32;
    let block_align = channels * (bits_per_sample / 8);
    let data_size = (audio_data.len() * 2) as u32;

    // WAV header
    use std::io::Write;
    cursor.write_all(b"RIFF").unwrap();
    cursor.write_all(&(36 + data_size).to_le_bytes()).unwrap();
    cursor.write_all(b"WAVE").unwrap();

    // fmt chunk
    cursor.write_all(b"fmt ").unwrap();
    cursor.write_all(&16u32.to_le_bytes()).unwrap();
    cursor.write_all(&1u16.to_le_bytes()).unwrap();
    cursor.write_all(&channels.to_le_bytes()).unwrap();
    cursor.write_all(&sample_rate.to_le_bytes()).unwrap();
    cursor.write_all(&byte_rate.to_le_bytes()).unwrap();
    cursor.write_all(&block_align.to_le_bytes()).unwrap();
    cursor.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    cursor.write_all(b"data").unwrap();
    cursor.write_all(&data_size.to_le_bytes()).unwrap();

    for &sample in audio_data {
        let clamped = sample.clamp(-1.0, 1.0);
        let int_sample = (clamped * 32767.0) as i16;
        cursor.write_all(&int_sample.to_le_bytes()).unwrap();
    }

    buf
}

/// Language names for prompt hints (used when multiple languages selected)
fn lang_name(code: &str) -> &'static str {
    match code {
        "en" => "English", "ru" => "Russian", "zh" => "Chinese",
        "de" => "German", "es" => "Spanish", "fr" => "French",
        "it" => "Italian", "ja" => "Japanese", "ko" => "Korean",
        "nl" => "Dutch", "pl" => "Polish", "pt" => "Portuguese",
        "tr" => "Turkish", "uk" => "Ukrainian",
        _ => "Unknown",
    }
}

/// Downsample mono PCM to 16 kHz — Whisper's native rate. The MacBook mic can't
/// capture 16 kHz directly and falls back to 48 kHz, which triples the upload for
/// no quality gain (the model resamples to 16 kHz server-side anyway). Averaging
/// each output window is a cheap anti-aliased decimation — adequate for speech —
/// and handles any input rate above 16 kHz, not just the exact 3× case.
fn resample_to_16k(input: &[f32], from_rate: u32) -> Vec<f32> {
    const TARGET: u32 = 16000;
    if from_rate <= TARGET || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / TARGET as f64;
    let out_len = (input.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let start = (i as f64 * ratio) as usize;
        let end = (((i + 1) as f64 * ratio) as usize).clamp(start + 1, input.len());
        let sum: f32 = input[start..end].iter().sum();
        out.push(sum / (end - start) as f32);
    }
    out
}

/// Blocking transcription against one resolved provider endpoint. The caller
/// (lib.rs) picks the active audio-stack entry and drives fallback on the
/// returned `CallError`; this fn is provider-agnostic — any OpenAI-compatible
/// `/audio/transcriptions` URL works.
pub fn transcribe_audio_blocking(
    audio_data: &[f32],
    sample_rate: u32,
    languages: &[String],
    url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, crate::fallback::CallError> {
    use crate::fallback::CallError;
    crate::debug_log::log(&format!("STT: {} ({}), langs={:?}", model, url.split('/').nth(2).unwrap_or("?"), languages));

    // Send at 16 kHz: smaller upload = faster round-trip, no accuracy loss.
    let (samples, sample_rate) = if sample_rate > 16000 {
        let r = resample_to_16k(audio_data, sample_rate);
        crate::debug_log::log(&format!("resample {}Hz->16000Hz: {} -> {} samples", sample_rate, audio_data.len(), r.len()));
        (r, 16000u32)
    } else {
        (audio_data.to_vec(), sample_rate)
    };

    let wav_bytes = encode_wav(&samples, sample_rate);
    crate::debug_log::log(&format!("WAV: {} bytes", wav_bytes.len()));

    let file_part = reqwest::blocking::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| CallError::rejected(format!("multipart build: {}", e)))?;

    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string());

    // Pass first language as `language` param (strongest signal for Whisper)
    // For multiple languages, also add a prompt hint
    if !languages.is_empty() {
        form = form.text("language", languages[0].clone());
        if languages.len() > 1 {
            let names: Vec<&str> = languages.iter().map(|c| lang_name(c)).collect();
            form = form.text("prompt", format!("Dictation in {}.", names.join(" and ")));
        }
    }

    let t0 = std::time::Instant::now();
    let response = client()
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .map_err(|e| CallError::transport(e.is_timeout(), format!("Network error: {}", e)))?;

    let elapsed = t0.elapsed();
    crate::debug_log::log(&format!("API response in {:.1}s, status={}", elapsed.as_secs_f32(), response.status()));

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(CallError::http(
            status.as_u16(),
            format!("API error {}: {}", status, body.chars().take(200).collect::<String>()),
        ));
    }

    let result: serde_json::Value = response
        .json()
        .map_err(|e| CallError::rejected(format!("Failed to parse response: {}", e)))?;

    Ok(result["text"].as_str().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_to_16k_thirds_length() {
        let input = vec![0.5f32; 4800]; // 0.1s @ 48 kHz
        let out = resample_to_16k(&input, 48000);
        assert_eq!(out.len(), 1600); // 0.1s @ 16 kHz
        // A constant signal must survive window-averaging unchanged.
        assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn resample_noop_at_or_below_16k() {
        let input = vec![0.1f32, -0.2, 0.3];
        assert_eq!(resample_to_16k(&input, 16000), input);
        assert_eq!(resample_to_16k(&input, 8000), input);
    }

    #[test]
    fn resample_empty_is_empty() {
        assert!(resample_to_16k(&[], 48000).is_empty());
    }
}

use std::io::Cursor;
use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| reqwest::blocking::Client::new())
}

/// Pre-initialize the HTTP client so first request doesn't pay TLS cost
pub fn warm_up_client() {
    let _ = client();
    crate::debug_log::log("HTTP client warmed up");
}

/// Detect which STT provider to use based on available API keys
/// Priority: GROQ_API_KEY > OPENAI_API_KEY
fn get_provider() -> Result<(&'static str, String, &'static str), String> {
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        Ok((
            "https://api.groq.com/openai/v1/audio/transcriptions",
            key,
            "whisper-large-v3-turbo",
        ))
    } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        Ok((
            "https://api.openai.com/v1/audio/transcriptions",
            key,
            "whisper-1",
        ))
    } else {
        Err("No API key set. Add GROQ_API_KEY or OPENAI_API_KEY.".into())
    }
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

/// Blocking transcription — auto-selects Groq or OpenAI based on available keys
pub fn transcribe_audio_blocking(audio_data: &[f32], sample_rate: u32) -> Result<String, String> {
    let (url, api_key, model) = get_provider()?;
    crate::debug_log::log(&format!("STT: {} ({})", model, url.split('/').nth(2).unwrap_or("?")));

    let wav_bytes = encode_wav(audio_data, sample_rate);
    crate::debug_log::log(&format!("WAV: {} bytes", wav_bytes.len()));

    let file_part = reqwest::blocking::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::blocking::multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("language", "ru");

    let t0 = std::time::Instant::now();
    let response = client()
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    let elapsed = t0.elapsed();
    crate::debug_log::log(&format!("API response in {:.1}s, status={}", elapsed.as_secs_f32(), response.status()));

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("API error {}: {}", status, body));
    }

    let result: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result["text"].as_str().unwrap_or("").to_string())
}

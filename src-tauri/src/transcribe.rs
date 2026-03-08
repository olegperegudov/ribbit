use std::io::Cursor;

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
    cursor.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    cursor.write_all(&1u16.to_le_bytes()).unwrap(); // PCM format
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

/// Blocking version — runs on a std::thread, no tokio needed.
pub fn transcribe_audio_blocking(audio_data: &[f32], sample_rate: u32) -> Result<String, String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not set. Please configure your API key.".to_string())?;

    let wav_bytes = encode_wav(audio_data, sample_rate);

    let file_part = reqwest::blocking::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::blocking::multipart::Form::new()
        .part("file", file_part)
        .text("model", "whisper-1")
        .text("prompt", "Transcribe accurately, preserving both Russian and English words as spoken.");

    let client = reqwest::blocking::Client::new();
    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, body));
    }

    let result: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result["text"].as_str().unwrap_or("").to_string())
}

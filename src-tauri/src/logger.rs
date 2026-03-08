use chrono::Local;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;

pub fn log_transcription(text: &str) {
    let log_dir = match dirs::config_dir() {
        Some(d) => d.join("ribbit").join("logs"),
        None => return,
    };

    if create_dir_all(&log_dir).is_err() {
        return;
    }

    let now = Local::now();
    let log_file = log_dir.join(format!("{}.jsonl", now.format("%Y-%m-%d")));

    let entry = serde_json::json!({
        "ts": now.to_rfc3339(),
        "text": text,
    });

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = writeln!(file, "{}", entry);
    }
}

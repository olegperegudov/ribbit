use chrono::Local;
use std::fs::{self, OpenOptions, create_dir_all};
use std::io::Write;

fn log_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ribbit").join("logs"))
}

pub fn log_transcription(text: &str, duration_secs: f32) {
    let log_dir = match log_dir() {
        Some(d) => d,
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
        "duration": duration_secs,
    });

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = writeln!(file, "{}", entry);
    }
}

/// Delete log files older than today
pub fn cleanup_old_logs() {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return,
    };

    let today = Local::now().format("%Y-%m-%d").to_string();

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".jsonl") && !name.starts_with(&today) {
                let _ = fs::remove_file(entry.path());
                crate::debug_log::log(&format!("cleaned up old log: {}", name));
            }
        }
    }
}

pub fn read_recent_entries(limit: usize) -> Vec<serde_json::Value> {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return vec![],
    };

    let today_file = log_dir.join(format!("{}.jsonl", Local::now().format("%Y-%m-%d")));
    let contents = match fs::read_to_string(&today_file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut entries: Vec<serde_json::Value> = contents
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    entries.reverse(); // newest first
    entries.truncate(limit);
    entries
}

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

/// Delete log files older than yesterday
pub fn cleanup_old_logs() {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return,
    };

    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let yesterday = (now - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".jsonl") && !name.starts_with(&today) && !name.starts_with(&yesterday) {
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

    let now = Local::now();
    let cutoff = now - chrono::Duration::days(1);

    // Read today + yesterday files
    let mut all_entries = Vec::new();
    for offset in 0..=1 {
        let date = (now - chrono::Duration::days(offset)).format("%Y-%m-%d").to_string();
        let file = log_dir.join(format!("{}.jsonl", date));
        if let Ok(contents) = fs::read_to_string(&file) {
            for line in contents.lines() {
                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                    // Filter: only entries within last 24h
                    if let Some(ts) = entry["ts"].as_str() {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                            if dt >= cutoff {
                                all_entries.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    all_entries.reverse(); // newest first
    all_entries.truncate(limit);
    all_entries
}

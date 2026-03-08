use chrono::Local;
use std::fs::{self, OpenOptions, create_dir_all};
use std::io::Write;

fn log_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ribbit").join("logs"))
}

pub fn log_transcription(text: &str) {
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
    });

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = writeln!(file, "{}", entry);
    }
}

pub fn read_recent_entries(limit: usize) -> Vec<serde_json::Value> {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return vec![],
    };

    if !log_dir.exists() {
        return vec![];
    }

    // Collect log files, sorted by name descending (newest first)
    let mut files: Vec<_> = fs::read_dir(&log_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    let mut entries = Vec::new();
    for file in files {
        if entries.len() >= limit {
            break;
        }
        if let Ok(contents) = fs::read_to_string(file.path()) {
            let mut file_entries: Vec<serde_json::Value> = contents
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            file_entries.reverse(); // newest first
            entries.extend(file_entries);
        }
    }

    entries.truncate(limit);
    entries
}

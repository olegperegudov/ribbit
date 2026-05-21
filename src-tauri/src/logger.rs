use chrono::Local;
use std::fs::{self, OpenOptions, create_dir_all};
use std::io::Write;

fn log_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ribbit").join("logs"))
}

pub fn log_transcription(text: &str, duration_secs: f32, edited: bool) {
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
        "edited": edited,
    });

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = writeln!(file, "{}", entry);
    }
}

/// Day-file names to keep: today and the (history_days - 1) days before it.
fn kept_dates(history_days: i64) -> Vec<String> {
    let now = Local::now();
    (0..history_days.max(1))
        .map(|o| (now - chrono::Duration::days(o)).format("%Y-%m-%d").to_string())
        .collect()
}

/// Delete log files that fall outside the rolling retention window.
pub fn cleanup_old_logs(history_days: i64) {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return,
    };

    let keep = kept_dates(history_days);

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".jsonl") && !keep.iter().any(|k| name.starts_with(k.as_str())) {
                let _ = fs::remove_file(entry.path());
                crate::debug_log::log(&format!("cleaned up old log: {}", name));
            }
        }
    }
}

pub fn read_recent_entries(limit: usize, history_days: i64) -> Vec<serde_json::Value> {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return vec![],
    };

    // Each day-file already partitions entries by calendar day, so reading the
    // files inside the retention window needs no extra per-entry time filter.
    let mut all_entries = Vec::new();
    for date in kept_dates(history_days) {
        let file = log_dir.join(format!("{}.jsonl", date));
        if let Ok(contents) = fs::read_to_string(&file) {
            for line in contents.lines() {
                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                    all_entries.push(entry);
                }
            }
        }
    }

    // Sort by ts descending (newest first). Naive reverse() ломался когда
    // вчерашних записей > limit — сегодняшние оказывались в хвосте и резались
    // truncate'ом, хотя фактически они новее.
    all_entries.sort_by(|a, b| {
        let ta = a["ts"].as_str().unwrap_or("");
        let tb = b["ts"].as_str().unwrap_or("");
        tb.cmp(ta)
    });
    all_entries.truncate(limit);
    all_entries
}

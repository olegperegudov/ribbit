use chrono::Local;
use std::fs;
use std::io::Write;

fn log_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ribbit").join("logs"))
}

/// One dictation's persistent record.
///
/// The timing fields exist to answer a single question: when a dictation feels
/// slow, which stage actually ate the time — speech-to-text, the LLM editor,
/// or text insertion. They are written to the daily jsonl next to the
/// transcript so slowness can be analysed days later (the in-memory debug log
/// is gone on restart).
pub struct TranscriptionLog<'a> {
    pub text: &'a str,
    /// Transcript before the LLM editor. `Some` only when the editor actually
    /// ran — lets us compare its input ("question") against `text` ("answer").
    pub raw_text: Option<&'a str>,
    pub edited: bool,
    /// Length of the recorded audio in seconds.
    pub audio_secs: f32,
    /// Wall time of the STT HTTP call (encode + upload + transcribe + download).
    pub stt_secs: f32,
    pub stt_model: &'a str,
    /// `None` when LLM post-processing is disabled. Set even when the call
    /// failed and we fell back to strict vocab — a timed-out LLM still burns
    /// its full timeout, and that time must show up somewhere.
    pub llm_secs: Option<f32>,
    pub llm_model: Option<&'a str>,
    /// Endpoint host of the text LLM that produced this entry (e.g.
    /// "routerai.ru"). Shown next to the green/yellow indicator in the history
    /// so a glance tells which provider — and which fallback rung — was live.
    pub llm_host: Option<&'a str>,
    /// Why the editor didn't produce this entry ("timed out", "rate limit /
    /// free tier"), `None` when it ran or was never asked to.
    /// The yellow dot in the history is drawn from the daily log after a
    /// restart, so the reason has to outlive the session that saw it.
    pub llm_error: Option<&'a str>,
    /// Wall time of typing the text into the focused app.
    pub insert_secs: f32,
    /// Bundle id of the app that received the keystrokes. Answers "the text is
    /// missing — where did it actually go?" days later, when the session log
    /// that would have shown it is long gone.
    pub insert_target: Option<&'a str>,
    /// Why insertion failed, `None` when it went through. The daily log is the
    /// only record that survives a restart, so a swallowed dictation has to
    /// leave its reason here rather than only in the session log.
    pub insert_error: Option<&'a str>,
    /// Whole post-release pipeline. `total - stt - llm - insert` is the fixed
    /// overhead (the 200ms focus-regain sleep + bookkeeping).
    pub total_secs: f32,
    /// Seconds the app sat idle since the previous dictation, `None` for the
    /// first of the session. Separates "slow because the TLS connection went
    /// cold" from "slow regardless".
    pub idle_secs: Option<f32>,
}

pub fn log_transcription(rec: &TranscriptionLog) {
    let log_dir = match log_dir() {
        Some(d) => d,
        None => return,
    };

    // The transcript log is a record of everything the user dictated.
    if crate::private::create_dir(&log_dir).is_err() {
        return;
    }

    let now = Local::now();
    let log_file = log_dir.join(format!("{}.jsonl", now.format("%Y-%m-%d")));
    let entry = build_entry(rec, &now.to_rfc3339());

    if let Ok(mut file) = crate::private::append(&log_file) {
        let _ = writeln!(file, "{}", entry);
    }
}

/// Serialize one record to its jsonl shape. Split out from the file IO so the
/// field set can be pinned by unit tests. `duration` keeps its old name — the
/// history UI reads that key — and equals `audio_secs`.
fn build_entry(rec: &TranscriptionLog, ts: &str) -> serde_json::Value {
    serde_json::json!({
        "ts": ts,
        "text": rec.text,
        "raw_text": rec.raw_text,
        "edited": rec.edited,
        "duration": rec.audio_secs,
        "text_chars": rec.text.chars().count(),
        "stt_secs": rec.stt_secs,
        "stt_model": rec.stt_model,
        "llm_secs": rec.llm_secs,
        "llm_model": rec.llm_model,
        "llm_host": rec.llm_host,
        "llm_error": rec.llm_error,
        "insert_secs": rec.insert_secs,
        "insert_target": rec.insert_target,
        "insert_error": rec.insert_error,
        "total_secs": rec.total_secs,
        "idle_secs": rec.idle_secs,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_entry_carries_all_timing_fields() {
        // Values are exactly representable in both f32 and f64 so the JSON
        // numbers compare cleanly (an f32 like 1.2 widens to 1.2000000476...).
        let rec = TranscriptionLog {
            text: "привет",
            raw_text: Some("привед"),
            edited: true,
            audio_secs: 3.5,
            stt_secs: 2.0,
            stt_model: "groq/whisper-large-v3-turbo",
            llm_secs: Some(1.5),
            llm_model: Some("google/gemma-4-26b-a4b-it"),
            llm_host: Some("routerai.ru"),
            llm_error: None,
            insert_secs: 0.5,
            insert_target: Some("com.apple.Safari"),
            insert_error: None,
            total_secs: 3.75,
            idle_secs: Some(42.0),
        };
        let e = build_entry(&rec, "2026-05-21T12:00:00+03:00");
        assert_eq!(e["ts"], "2026-05-21T12:00:00+03:00");
        assert_eq!(e["text"], "привет");
        assert_eq!(e["raw_text"], "привед");
        assert_eq!(e["duration"], 3.5);
        assert_eq!(e["text_chars"], 6);
        assert_eq!(e["stt_secs"], 2.0);
        assert_eq!(e["stt_model"], "groq/whisper-large-v3-turbo");
        assert_eq!(e["llm_secs"], 1.5);
        assert_eq!(e["llm_model"], "google/gemma-4-26b-a4b-it");
        assert_eq!(e["llm_host"], "routerai.ru");
        assert_eq!(e["insert_secs"], 0.5);
        assert_eq!(e["insert_target"], "com.apple.Safari");
        assert!(e["insert_error"].is_null());
        assert_eq!(e["total_secs"], 3.75);
        assert_eq!(e["idle_secs"], 42.0);
    }

    /// The whole point of the field: a dictation nobody saw must say so in the
    /// only log that outlives the session.
    #[test]
    fn build_entry_records_why_insertion_failed() {
        let rec = TranscriptionLog {
            text: "привет",
            raw_text: None,
            edited: false,
            audio_secs: 1.0,
            stt_secs: 0.5,
            stt_model: "groq/whisper-large-v3-turbo",
            llm_secs: None,
            llm_model: None,
            llm_host: None,
            llm_error: None,
            insert_secs: 0.0,
            insert_target: Some("com.apple.Terminal"),
            insert_error: Some("macOS secure input is on"),
            total_secs: 0.6,
            idle_secs: None,
        };
        let e = build_entry(&rec, "ts");
        assert_eq!(e["insert_error"], "macOS secure input is on");
        assert_eq!(e["insert_target"], "com.apple.Terminal");
    }

    /// A yellow dot with no reason is the bug this field exists to fix: the
    /// history is rebuilt from these lines, so "why not edited" has to be in
    /// them and not only in the session that watched it happen.
    #[test]
    fn build_entry_records_why_the_edit_did_not_happen() {
        let rec = TranscriptionLog {
            text: "привет",
            raw_text: Some("привет"),
            edited: false,
            audio_secs: 1.0,
            stt_secs: 0.5,
            stt_model: "groq/whisper-large-v3-turbo",
            llm_secs: Some(5.0),
            llm_model: Some("llama-3.3-70b-versatile"),
            llm_host: Some("api.groq.com"),
            llm_error: Some("rate limit / free tier"),
            insert_secs: 0.1,
            insert_target: None,
            insert_error: None,
            total_secs: 5.7,
            idle_secs: None,
        };
        let e = build_entry(&rec, "ts");
        assert_eq!(e["edited"], false);
        assert_eq!(e["llm_error"], "rate limit / free tier");
        assert_eq!(e["llm_host"], "api.groq.com");
    }

    #[test]
    fn build_entry_nulls_llm_fields_when_disabled() {
        let rec = TranscriptionLog {
            text: "hi",
            raw_text: None,
            edited: false,
            audio_secs: 1.0,
            stt_secs: 0.9,
            stt_model: "groq/whisper-large-v3-turbo",
            llm_secs: None,
            llm_model: None,
            llm_host: None,
            llm_error: None,
            insert_secs: 0.1,
            insert_target: None,
            insert_error: None,
            total_secs: 1.1,
            idle_secs: None,
        };
        let e = build_entry(&rec, "ts");
        assert!(e["llm_secs"].is_null());
        assert!(e["llm_model"].is_null());
        assert!(e["llm_host"].is_null());
        assert!(e["llm_error"].is_null());
        assert!(e["raw_text"].is_null());
        assert!(e["idle_secs"].is_null());
    }
}

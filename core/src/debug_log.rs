//! The session log: what the app did, never what was said.
//!
//! Ribbit lives in the menu bar, so when a dictation lands in the wrong window or
//! a provider quietly fails there is no console to look at — this file is the
//! only witness. Two rules keep it from turning into a transcript:
//!
//! * **Events, not content.** Lines record *that* a transcription of N characters
//!   came back from a model, never the words. Speech is the most personal thing
//!   the app touches, and the transcript already has a home with an expiry
//!   (`logger.rs`, `history_days`). A second, permanent copy in here would make
//!   that expiry a lie.
//! * **Fresh file per launch.** An append-only log on an app that runs for weeks
//!   is a slow disk leak, and only the current session is ever useful.
//!
//! Owner-only (0600) through `private.rs` either way — Quill and CopyPaster keep
//! their logs the same way, deliberately.

use chrono::Local;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

fn log_file() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("ribbit").join("logs");
    let _ = crate::private::create_dir(&dir);
    Some(dir.join("debug.log"))
}

pub fn init() {
    let Some(path) = log_file() else { return };
    let _ = crate::private::write(&path, b"");
    if let Ok(mut g) = LOG_PATH.lock() {
        *g = Some(path);
    }
}

pub fn log(msg: &str) {
    let cached = LOG_PATH.lock().ok().and_then(|g| g.clone());
    // Before init() the path is not cached yet — resolve it rather than drop the
    // line; a log that starts late is still a log.
    let Some(path) = cached.or_else(log_file) else { return };
    if let Ok(mut file) = crate::private::append(&path) {
        let ts = Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", ts, msg);
    }
}

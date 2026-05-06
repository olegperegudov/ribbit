//! Authoritatively re-arms macOS TCC permissions when the binary's cdhash
//! changes. Each ad-hoc-signed Tauri build has a fresh cdhash, and TCC's
//! csreq blob anchors permission entries to the cdhash that was present when
//! the prompt was first granted. Result: every release silently breaks
//! `kTCCServicePostEvent` (synthetic keystrokes, i.e. Cmd+V) — System
//! Settings still shows Ribbit as "allowed" but events get filtered at
//! kCGHIDEventTap. We persist the last-seen cdhash in config.json; on
//! mismatch we shell out to `tccutil reset All <bundle-id>` so macOS will
//! re-prompt on the next mic/paste action.
//!
//! On non-macOS this module is a no-op.

#[cfg(target_os = "macos")]
pub fn ensure_permissions(bundle_id: &str) {
    use std::process::Command;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            crate::debug_log::log(&format!("tcc: current_exe failed: {}", e));
            return;
        }
    };

    // codesign prints metadata to stderr, not stdout
    let cdhash = Command::new("codesign")
        .args(["-dvvv", &exe.to_string_lossy()])
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stderr).to_string();
            s.lines()
                .find_map(|l| l.strip_prefix("CDHash="))
                .map(|h| h.trim().to_string())
        })
        .unwrap_or_default();

    if cdhash.is_empty() {
        crate::debug_log::log("tcc: could not read cdhash, skipping");
        return;
    }

    let cfg_path = match dirs::config_dir() {
        Some(d) => d.join("ribbit").join("config.json"),
        None => return,
    };

    let mut cfg: serde_json::Value = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let saved = cfg["cdhash"].as_str().unwrap_or("").to_string();

    if saved == cdhash {
        return;
    }

    crate::debug_log::log(&format!(
        "tcc: cdhash {} -> {}, resetting permissions",
        if saved.is_empty() { "<none>" } else { saved.as_str() },
        &cdhash
    ));

    // We'd prefer to reset only PostEvent — that's the one that actually breaks
    // — but `tccutil` doesn't expose a stable CLI name for it. `All` also drops
    // the Microphone grant, so the user gets one extra prompt per release. Beats
    // Cmd+V silently failing.
    match Command::new("tccutil")
        .args(["reset", "All", bundle_id])
        .output()
    {
        Ok(out) if out.status.success() => {
            crate::debug_log::log("tcc: reset OK");
        }
        Ok(out) => {
            crate::debug_log::log(&format!(
                "tcc: reset exited {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Err(e) => {
            crate::debug_log::log(&format!("tcc: tccutil failed: {}", e));
        }
    }

    // Save new cdhash even if reset failed — better than retrying every launch.
    cfg["cdhash"] = serde_json::Value::String(cdhash);
    if let Some(parent) = cfg_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(&cfg) {
        let _ = std::fs::write(&cfg_path, s);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_permissions(_bundle_id: &str) {}

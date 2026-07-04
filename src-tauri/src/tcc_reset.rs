//! Clear stale macOS TCC permissions when the app's *code-signing identity*
//! changes — which now happens exactly once, on the ad-hoc → stable-cert
//! switch.
//!
//! Background: a TCC permission entry (e.g. Accessibility / kTCCServicePostEvent
//! for the synthetic keystrokes that insert dictated text) is anchored to the
//! binary's designated requirement. With ad-hoc signing the requirement pinned
//! the *cdhash*, which changes on every release, so each update silently broke
//! the grant (System Settings still showed "allowed" while events were filtered
//! at kCGHIDEventTap). We now ship a stable self-signed certificate, so the
//! requirement anchors to the certificate and the grant survives updates.
//!
//! This module used to reset on every cdhash change (i.e. every release). That is
//! wrong under stable signing — it would wipe a perfectly good grant on each
//! build. Instead we key off the signing identity: it stays "Ribbit Code Signing"
//! across cert-signed builds (no reset), and differs only when migrating from the
//! old ad-hoc binaries (one reset, so the stale ad-hoc grant is cleared and the
//! user gets a clean Accessibility prompt for the cert-signed app).
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

    // codesign prints metadata to stderr. Cert-signed → an `Authority=<name>`
    // line; ad-hoc → `Signature=adhoc` and no Authority.
    let meta = Command::new("codesign")
        .args(["-dvvv", &exe.to_string_lossy()])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stderr).to_string())
        .unwrap_or_default();

    let signing_id = meta
        .lines()
        .find_map(|l| l.strip_prefix("Authority="))
        .map(|s| s.trim().to_string())
        .or_else(|| meta.contains("Signature=adhoc").then(|| "adhoc".to_string()))
        .unwrap_or_default();

    // Couldn't tell what we are — never reset on a guess (that would nuke a good
    // grant). Skip until we can read it.
    if signing_id.is_empty() {
        crate::debug_log::log("tcc: could not read signing identity, skipping");
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

    let saved = cfg["signing_id"].as_str().unwrap_or("").to_string();

    if saved == signing_id {
        return;
    }

    crate::debug_log::log(&format!(
        "tcc: signing identity {} -> {}, resetting permissions once",
        if saved.is_empty() { "<none>" } else { saved.as_str() },
        &signing_id
    ));

    match Command::new("tccutil")
        .args(["reset", "All", bundle_id])
        .output()
    {
        Ok(out) if out.status.success() => crate::debug_log::log("tcc: reset OK"),
        Ok(out) => crate::debug_log::log(&format!(
            "tcc: reset exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => crate::debug_log::log(&format!("tcc: tccutil failed: {}", e)),
    }

    // Record the identity even if the reset failed — better than retrying every
    // launch.
    cfg["signing_id"] = serde_json::Value::String(signing_id);
    if let Some(parent) = cfg_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(&cfg) {
        let _ = std::fs::write(&cfg_path, s);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_permissions(_bundle_id: &str) {}

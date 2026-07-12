mod audio;
mod transcribe;
mod inserter;
mod logger;
mod debug_log;
mod sound;
mod vocab;
mod postprocess;
mod fallback;
mod mac_window;
mod tcc_reset;
mod mic_permission;

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, Emitter,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_updater::UpdaterExt;

struct RecordingState {
    is_recording: bool,
    audio_data: Vec<f32>,
    sample_rate: u32,
    current_shortcut: String,
}

/// When the previous dictation finished. Read at the start of each new one to
/// record the idle gap — a long gap usually means the pooled TLS connection to
/// the STT/LLM endpoint went cold and the next request pays a reconnect.
static LAST_DICTATION: std::sync::OnceLock<Mutex<Option<std::time::Instant>>> =
    std::sync::OnceLock::new();

fn last_dictation() -> &'static Mutex<Option<std::time::Instant>> {
    LAST_DICTATION.get_or_init(|| Mutex::new(None))
}

/// Last LLM post-process failure, surfaced in Settings. Without this the
/// feature rots silently: a provider retires a model id, every call 404s, the
/// code falls back to plain vocab, and the user just sees "the LLM does
/// nothing" with no clue why. Cleared on the next successful edit.
static LAST_LLM_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_last_llm_error(e: Option<String>) {
    if let Ok(mut g) = LAST_LLM_ERROR.lock() {
        *g = e;
    }
}

/// Endpoint host of a stack entry, e.g. "routerai.ru".
fn entry_host(e: &fallback::ProviderEntry) -> &str {
    e.url.split('/').nth(2).unwrap_or("?")
}

/// "provider/model" label for the transcription log; label falls back to the
/// endpoint host for custom entries.
fn entry_label(e: &fallback::ProviderEntry) -> String {
    let prov = if e.label.is_empty() { entry_host(e) } else { e.label.as_str() };
    format!("{}/{}", prov, e.model)
}

fn parse_stack(kind: &str) -> Result<fallback::Stack, String> {
    match kind {
        "audio" => Ok(fallback::Stack::Audio),
        "text" => Ok(fallback::Stack::Text),
        _ => Err(format!("unknown stack: {}", kind)),
    }
}

/// Catalog defaults for a known provider name in the given stack:
/// `(label, url, default_model, key_env)`. `None` for an unknown name.
fn catalog_lookup(stack: fallback::Stack, name: &str) -> Option<(String, String, String, String)> {
    match stack {
        fallback::Stack::Text => postprocess::find_provider(name).map(|p| {
            (p.label.to_string(), p.base_url.to_string(), p.default_model.to_string(), p.env_var.to_string())
        }),
        fallback::Stack::Audio => transcribe::find_audio_provider(name).map(|p| {
            (p.label.to_string(), p.url.to_string(), p.default_model.to_string(), p.key_env.to_string())
        }),
    }
}

/// Stack entries as JSON for the UI, each tagged with whether its key is set —
/// so the panel can show a "saved" chip vs an input without exposing the key.
fn stack_json(cfg: &serde_json::Value, stack: fallback::Stack) -> serde_json::Value {
    let entries = fallback::read_stack(cfg, stack);
    let arr: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let has_key = std::env::var(&e.key_env).map(|k| !k.is_empty()).unwrap_or(false);
            serde_json::json!({
                "id": e.id, "label": e.label, "url": e.url,
                "model": e.model, "key_env": e.key_env, "has_key": has_key,
            })
        })
        .collect();
    serde_json::Value::Array(arr)
}

/// Live status for the Settings status line: which fallback we sit on and how
/// long until the cooldown returns us to the primary. `null` while on primary.
fn stack_state_json(cfg: &serde_json::Value, stack: fallback::Stack) -> serde_json::Value {
    let total = fallback::read_stack(cfg, stack).len();
    match fallback::snapshot(stack) {
        Some((active, ago)) => {
            let cooldown = fallback::cooldown(cfg).as_secs();
            let remaining = cooldown.saturating_sub(ago.as_secs());
            serde_json::json!({ "active": active, "total": total, "remaining_secs": remaining })
        }
        None => serde_json::Value::Null,
    }
}

/// Smallest unused `p<N>` id across both stacks — stable, no clock needed.
fn next_provider_id(cfg: &serde_json::Value) -> String {
    let mut max = 0u64;
    for s in [fallback::Stack::Audio, fallback::Stack::Text] {
        for e in fallback::read_stack(cfg, s) {
            if let Some(n) = e.id.strip_prefix('p').and_then(|d| d.parse::<u64>().ok()) {
                max = max.max(n);
            }
        }
    }
    format!("p{}", max + 1)
}

#[tauri::command]
fn get_config() -> Result<serde_json::Value, String> {
    let cfg = read_config();

    // First usable audio key gates the first-run setup screen.
    let has_api_key = fallback::read_stack(&cfg, fallback::Stack::Audio)
        .iter()
        .any(|e| std::env::var(&e.key_env).map(|k| !k.is_empty()).unwrap_or(false));

    Ok(serde_json::json!({
        "has_api_key": has_api_key,
        "has_groq_key": std::env::var("GROQ_API_KEY").map(|k| !k.is_empty()).unwrap_or(false),
        "postprocess_enabled": cfg["postprocess_enabled"].as_bool().unwrap_or(false),
        "audio_providers": stack_json(&cfg, fallback::Stack::Audio),
        "text_providers": stack_json(&cfg, fallback::Stack::Text),
        "fallback_threshold": fallback::threshold(&cfg),
        "fallback_cooldown_mins": fallback::cooldown(&cfg).as_secs() / 60,
        "fallback_state": serde_json::json!({
            "audio": stack_state_json(&cfg, fallback::Stack::Audio),
            "text": stack_state_json(&cfg, fallback::Stack::Text),
        }),
        "history_days": history_days(),
        "always_on_top": cfg["always_on_top"].as_bool().unwrap_or(false),
    }))
}

/// Known providers for the "+ add provider" picker in the given stack.
#[tauri::command]
fn list_provider_catalog(kind: String) -> Result<Vec<serde_json::Value>, String> {
    let stack = parse_stack(&kind)?;
    let v = match stack {
        fallback::Stack::Text => postprocess::PROVIDERS
            .iter()
            .map(|p| serde_json::json!({ "name": p.name, "label": p.label, "default_model": p.default_model }))
            .collect(),
        fallback::Stack::Audio => transcribe::AUDIO_PROVIDERS
            .iter()
            .map(|p| serde_json::json!({ "name": p.name, "label": p.label, "default_model": p.default_model }))
            .collect(),
    };
    Ok(v)
}

/// Append a provider to a stack. `provider` is a catalog name (prefills
/// url/model/key) or "custom" (blank, user fills the url). Returns the updated
/// stack so the UI re-renders.
#[tauri::command]
fn add_provider(kind: String, provider: String) -> Result<serde_json::Value, String> {
    let stack = parse_stack(&kind)?;
    let mut config = read_config();
    let id = next_provider_id(&config);
    let entry = if provider == "custom" {
        serde_json::json!({
            "id": id, "label": "custom", "url": "", "model": "",
            "key_env": format!("RIBBIT_KEY_{}", id),
        })
    } else {
        let (label, url, model, key_env) = catalog_lookup(stack, &provider)
            .ok_or_else(|| format!("unknown provider: {}", provider))?;
        serde_json::json!({ "id": id, "label": label, "url": url, "model": model, "key_env": key_env })
    };
    if !config[stack.config_key()].is_array() {
        config[stack.config_key()] = serde_json::json!([]);
    }
    config[stack.config_key()].as_array_mut().unwrap().push(entry);
    save_config(&config)?;
    debug_log::log(&format!("add_provider {}/{} -> {}", kind, provider, id));
    Ok(stack_json(&config, stack))
}

#[tauri::command]
fn remove_provider(kind: String, id: String) -> Result<serde_json::Value, String> {
    let stack = parse_stack(&kind)?;
    let mut config = read_config();
    if let Some(arr) = config[stack.config_key()].as_array_mut() {
        arr.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));
    }
    save_config(&config)?;
    debug_log::log(&format!("remove_provider {}/{}", kind, id));
    Ok(stack_json(&config, stack))
}

/// Edit one editable field (url / model / label) of a stack entry.
#[tauri::command]
fn set_provider_field(kind: String, id: String, field: String, value: String) -> Result<(), String> {
    if !matches!(field.as_str(), "url" | "model" | "label") {
        return Err(format!("field not editable: {}", field));
    }
    let stack = parse_stack(&kind)?;
    let mut config = read_config();
    let arr = config[stack.config_key()].as_array_mut().ok_or("stack missing")?;
    let entry = arr
        .iter_mut()
        .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
        .ok_or("unknown provider entry")?;
    entry[field.as_str()] = serde_json::Value::String(value.trim().to_string());
    save_config(&config)?;
    Ok(())
}

/// Move an entry up or down — the order IS the fallback priority.
#[tauri::command]
fn move_provider(kind: String, id: String, up: bool) -> Result<serde_json::Value, String> {
    let stack = parse_stack(&kind)?;
    let mut config = read_config();
    let arr = config[stack.config_key()].as_array_mut().ok_or("stack missing")?;
    let pos = arr
        .iter()
        .position(|e| e.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
        .ok_or("unknown provider entry")?;
    let target = if up { pos.checked_sub(1) } else if pos + 1 < arr.len() { Some(pos + 1) } else { None };
    if let Some(t) = target {
        arr.swap(pos, t);
        save_config(&config)?;
    }
    Ok(stack_json(&config, stack))
}

/// Store a stack entry's API key into the entry's `key_env` (in `.env` + live).
#[tauri::command]
async fn set_provider_key(kind: String, id: String, key: String) -> Result<(), String> {
    let stack = parse_stack(&kind)?;
    let config = read_config();
    let entry = fallback::read_stack(&config, stack)
        .into_iter()
        .find(|e| e.id == id)
        .ok_or("unknown provider entry")?;
    write_env_var(&entry.key_env, key.trim())?;
    debug_log::log(&format!("provider key saved: {}/{} -> {}", kind, id, entry.key_env));
    Ok(())
}

#[tauri::command]
fn set_fallback_threshold(value: u64) -> Result<(), String> {
    let mut config = read_config();
    config["fallback_threshold"] = serde_json::json!(value.clamp(1, 100));
    save_config(&config)?;
    Ok(())
}

#[tauri::command]
fn set_fallback_cooldown(minutes: u64) -> Result<(), String> {
    let mut config = read_config();
    config["fallback_cooldown_mins"] = serde_json::json!(minutes.clamp(1, 1440));
    save_config(&config)?;
    Ok(())
}

/// Last LLM post-process failure (None once a later call succeeds). The
/// Settings panel shows it so a silently-rotted provider is visible.
#[tauri::command]
fn get_llm_last_error() -> Option<String> {
    LAST_LLM_ERROR.lock().ok().and_then(|g| g.clone())
}

#[tauri::command]
fn set_postprocess_enabled(enabled: bool) -> Result<(), String> {
    let mut config = read_config();
    config["postprocess_enabled"] = serde_json::Value::Bool(enabled);
    save_config(&config)?;
    debug_log::log(&format!("postprocess_enabled set to: {}", enabled));
    Ok(())
}

#[tauri::command]
fn get_log_history(limit: usize) -> Vec<serde_json::Value> {
    // limit 0 = no cap (load the whole retention window so search covers it).
    let cap = if limit == 0 { usize::MAX } else { limit };
    logger::read_recent_entries(cap, history_days())
}

#[tauri::command]
fn set_history_days(days: i64) -> Result<(), String> {
    let d = days.clamp(1, 365);
    let mut config = read_config();
    config["history_days"] = serde_json::json!(d);
    save_config(&config)?;
    // Apply the new (possibly shorter) window right away.
    logger::cleanup_old_logs(d);
    debug_log::log(&format!("history_days set to: {}", d));
    Ok(())
}

#[tauri::command]
fn js_debug_log(msg: String) {
    debug_log::log(&format!("[js] {}", msg));
}

#[tauri::command]
fn get_debug_log() -> String {
    let log_path = match dirs::config_dir() {
        Some(d) => d.join("ribbit").join("logs").join("debug.log"),
        None => return "Cannot find config directory".to_string(),
    };
    match std::fs::read_to_string(&log_path) {
        Ok(contents) => {
            // Return last 200 lines max
            let lines: Vec<&str> = contents.lines().collect();
            let start = if lines.len() > 200 { lines.len() - 200 } else { 0 };
            lines[start..].join("\n")
        }
        Err(_) => "No debug log found.".to_string(),
    }
}

#[tauri::command]
fn test_sound(app: AppHandle) {
    app.state::<sound::SoundPlayer>().play(sound::SoundKind::Start);
}

#[tauri::command]
fn get_sound_pack() -> String {
    sound::get_pack().to_string()
}

#[tauri::command]
fn get_languages() -> Vec<String> {
    let config = read_config();
    config["languages"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

#[tauri::command]
fn set_languages(languages: Vec<String>) -> Result<(), String> {
    let mut config = read_config();
    config["languages"] = serde_json::json!(languages);
    save_config(&config)?;
    debug_log::log(&format!("Languages set to: {:?}", languages));
    Ok(())
}

#[tauri::command]
fn set_sound_pack(pack: String) -> Result<(), String> {
    sound::set_pack(&pack);
    let mut config = read_config();
    config["sound_pack"] = serde_json::Value::String(pack.clone());
    save_config(&config)?;
    debug_log::log(&format!("Sound pack changed to: {}", pack));
    Ok(())
}

/// Hide the main window to the tray and keep the tray menu label in step.
/// Uses hide() (orderOut on macOS): no Dock entry, no Space-flip. Native
/// minimize is never used on macOS — it sends the window to the Dock and the
/// later restore snaps back to the window's home Space (see `minimize_window`).
fn hide_main(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    mac_window::hide_panel(app);
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    if let Some(item) = app.try_state::<tauri::menu::MenuItem<tauri::Wry>>() {
        let _ = item.set_text("Show Ribbit");
    }
}

/// The X button: hide to tray.
#[tauri::command]
fn hide_to_tray(app: AppHandle) {
    hide_main(&app);
}

/// The "_" (minimize) button. On Windows/Linux it genuinely minimizes to the
/// taskbar. On macOS native minimize is broken for a tray app: it sends the
/// window to the Dock and unminimize forces a Space switch back to the window's
/// home Space (the "flashes then vanishes / teleports me to another desktop"
/// bug), so there we hide to the tray instead — same as the close button.
#[tauri::command]
fn minimize_window(app: AppHandle) {
    #[cfg(target_os = "macos")]
    hide_main(&app);
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<serde_json::Value, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let body = update.body.clone().unwrap_or_default();
            debug_log::log(&format!("Update available: v{}", version));
            let _ = app.emit("update-available", &version);
            Ok(serde_json::json!({
                "available": true,
                "version": version,
                "body": body,
            }))
        }
        Ok(None) => {
            debug_log::log("No update available");
            Ok(serde_json::json!({ "available": false }))
        }
        Err(e) => {
            debug_log::log(&format!("Update check failed: {}", e));
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            debug_log::log(&format!("Downloading update v{}...", update.version));

            let mut downloaded: u64 = 0;
            let app_for_event = app.clone();
            update.download_and_install(
                move |chunk, total| {
                    downloaded += chunk as u64;
                    let progress = total.map(|t| (downloaded as f64 / t as f64 * 100.0) as u32);
                    let _ = app_for_event.emit("update-progress", progress.unwrap_or(0));
                },
                || {
                    debug_log::log("Update downloaded, restarting...");
                },
            ).await.map_err(|e| {
                debug_log::log(&format!("Update install failed: {}", e));
                e.to_string()
            })?;

            app.restart();
        }
        Ok(None) => Err("No update available".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn get_current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn set_always_on_top(app: AppHandle, value: bool) -> Result<(), String> {
    apply_always_on_top(&app, value)?;
    let mut config = read_config();
    config["always_on_top"] = serde_json::json!(value);
    save_config(&config)
}

/// Raise/lower the window level and tell the macOS panel whether it may keep
/// floating when it loses focus. Used by the command and at startup, so the
/// setting survives a restart instead of silently resetting to off.
fn apply_always_on_top(app: &AppHandle, value: bool) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.set_always_on_top(value).map_err(|e| e.to_string())?;
    }
    mac_window::set_always_on_top(value);
    Ok(())
}

/// Write (or replace) one `KEY=value` line in the app's `.env` and update the
/// live process env. Single source of truth for key persistence — used by the
/// setup screen and by per-entry provider keys.
fn write_env_var(name: &str, value: &str) -> Result<(), String> {
    let env_path = dirs::config_dir()
        .ok_or("Cannot find config directory")?
        .join("ribbit")
        .join(".env");
    std::fs::create_dir_all(env_path.parent().unwrap()).map_err(|e| e.to_string())?;
    let prefix = format!("{}=", name);
    let existing = std::fs::read_to_string(&env_path).unwrap_or_default();
    let mut lines: Vec<String> = existing
        .lines()
        .filter(|l| !l.starts_with(&prefix))
        .map(|l| l.to_string())
        .collect();
    lines.push(format!("{}={}", name, value));
    std::fs::write(&env_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    unsafe { std::env::set_var(name, value); }
    Ok(())
}

#[tauri::command]
async fn set_api_key(key: String, provider: Option<String>) -> Result<(), String> {
    // Detect provider from key prefix or explicit parameter
    let prov = provider.unwrap_or_else(|| {
        if key.starts_with("gsk_") { "groq".into() } else { "openai".into() }
    });

    let var_name = match prov.as_str() {
        "groq" => "GROQ_API_KEY",
        "router_ai" | "routerai" => "ROUTERAI_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        _ => "OPENAI_API_KEY",
    };

    write_env_var(var_name, &key)?;
    debug_log::log(&format!("API key saved: {} ({})", var_name, prov));
    Ok(())
}

#[tauri::command]
fn get_shortcut(state: tauri::State<'_, Arc<Mutex<RecordingState>>>) -> String {
    state.lock().unwrap().current_shortcut.clone()
}

#[tauri::command]
fn set_shortcut(app: AppHandle, shortcut: String, state: tauri::State<'_, Arc<Mutex<RecordingState>>>) -> Result<(), String> {
    let new_shortcut: Shortcut = shortcut.parse()
        .map_err(|e| format!("Invalid shortcut: {}", e))?;

    let old_str = state.lock().unwrap().current_shortcut.clone();
    if let Ok(old) = old_str.parse::<Shortcut>() {
        let _ = app.global_shortcut().unregister(old);
    }

    if let Err(e) = register_shortcut(&app, new_shortcut) {
        // Restore old shortcut on failure
        if let Ok(old) = old_str.parse::<Shortcut>() {
            let _ = register_shortcut(&app, old);
        }
        return Err(e);
    }

    state.lock().unwrap().current_shortcut = shortcut.clone();

    let mut config = read_config();
    config["shortcut"] = serde_json::Value::String(shortcut.clone());
    save_config(&config)?;

    debug_log::log(&format!("Shortcut changed to: {}", shortcut));
    Ok(())
}

#[tauri::command]
fn get_vocab() -> std::collections::HashMap<String, Vec<String>> {
    vocab::read_vocab()
}

#[tauri::command]
fn set_vocab(vocab_data: std::collections::HashMap<String, Vec<String>>) -> Result<(), String> {
    vocab::save_vocab(&vocab_data)
}

#[tauri::command]
fn add_vocab_entry(target: String, alias: String) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let mut v = vocab::read_vocab();
    let aliases = v.entry(target).or_default();
    let alias_lower = alias.to_lowercase();
    if !aliases.iter().any(|a| a.to_lowercase() == alias_lower) {
        aliases.push(alias);
    }
    vocab::save_vocab(&v)?;
    Ok(v)
}

#[tauri::command]
fn remove_vocab_alias(target: String, alias: String) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let mut v = vocab::read_vocab();
    if let Some(aliases) = v.get_mut(&target) {
        aliases.retain(|a| a.to_lowercase() != alias.to_lowercase());
        if aliases.is_empty() {
            v.remove(&target);
        }
    }
    vocab::save_vocab(&v)?;
    Ok(v)
}

#[tauri::command]
fn remove_vocab_entry(target: String) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let mut v = vocab::read_vocab();
    v.remove(&target);
    vocab::save_vocab(&v)?;
    Ok(v)
}

fn start_recording(state: &Arc<Mutex<RecordingState>>, app: &AppHandle) {
    debug_log::log("start_recording called");
    let mut s = state.lock().unwrap();
    if s.is_recording {
        debug_log::log("already recording, ignoring");
        return;
    }
    s.is_recording = true;
    s.audio_data.clear();
    drop(s);

    // Show "warming up" immediately — the actual "Listening" + start sound
    // fires from audio::record_audio after the mic stream starts
    let _ = app.emit("status-detail", "Starting mic...");

    let state_clone = Arc::clone(state);
    let app_clone = app.clone();
    std::thread::spawn(move || {
        debug_log::log("audio thread started");
        audio::record_audio(state_clone, app_clone);
        debug_log::log("audio thread finished");
    });
}

fn stop_recording_and_transcribe(state: &Arc<Mutex<RecordingState>>, app: &AppHandle) {
    debug_log::log("stop_recording called");
    let (audio_data, sample_rate) = {
        let mut s = state.lock().unwrap();
        if !s.is_recording {
            debug_log::log("not recording, ignoring");
            return;
        }
        s.is_recording = false;
        (s.audio_data.clone(), s.sample_rate)
    };

    let _ = app.emit("recording-status", false);
    // No stop sound — the Done sound after transcription is enough

    let duration_secs = audio_data.len() as f32 / sample_rate as f32;
    let rms = if audio_data.is_empty() {
        0.0
    } else {
        (audio_data.iter().map(|s| s * s).sum::<f32>() / audio_data.len() as f32).sqrt()
    };
    debug_log::log(&format!("audio: {} samples, {:.1}s, RMS={:.6}", audio_data.len(), duration_secs, rms));

    if audio_data.is_empty() || duration_secs < 0.3 {
        let _ = app.emit("status-detail", "Too short, try again.");
        return;
    }

    if rms < 0.001 {
        debug_log::log("WARNING: audio is silence (RMS < 0.001), check mic permissions");
        let _ = app.emit("status-detail", "Mic silent — check mic permissions in system settings");
        return;
    }

    let _ = app.emit("status-detail",
        format!("Ribbiting... ({:.1}s of audio)", duration_secs));

    // Use a dedicated thread for the transcribe flow.
    // Avoids tokio runtime issues.
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let _ = app_handle.emit("transcribing", true);

        // Time the whole post-release pipeline. Each stage is timed separately
        // so a slow dictation can be attributed to STT, the LLM editor, or
        // text insertion after the fact — see logger::TranscriptionLog.
        let t_pipeline = std::time::Instant::now();
        let idle_secs = (*last_dictation().lock().unwrap())
            .map(|t| t.elapsed().as_secs_f32());

        // Read configured languages for Whisper hint
        let languages = get_languages();

        // Config read once for the whole pipeline (audio stack + text stack).
        let cfg = read_config();

        // STT with in-request failover: walk the audio stack from the sticky
        // active entry, so a transient failure (429/5xx/timeout) tries the
        // next provider for THIS dictation — speech is never lost just because
        // the primary blinked. Hard errors (bad key/url/model) still surface.
        let t_stt = std::time::Instant::now();
        let (result, stt_model): (Result<String, String>, String) = {
            let entries = fallback::read_stack(&cfg, fallback::Stack::Audio);
            if entries.is_empty() {
                (Err("No audio provider configured. Add one in Settings.".into()), "none".to_string())
            } else {
                let start = fallback::active_index(fallback::Stack::Audio, fallback::cooldown(&cfg))
                    .min(entries.len() - 1);
                match fallback::run_with_failover(
                    fallback::Stack::Audio, &entries, start, fallback::threshold(&cfg),
                    |e, key| transcribe::transcribe_audio_blocking(
                        &audio_data, sample_rate, &languages, &e.url, key, &e.model,
                    ),
                ) {
                    Ok((text, used)) => (Ok(text), entry_label(&entries[used])),
                    Err(msg) => (Err(msg), entry_label(&entries[start])),
                }
            }
        };
        let stt_secs = t_stt.elapsed().as_secs_f32();

        match result {
            Ok(raw_text) => {
                // Pipeline: if LLM post-processing is enabled we send raw text +
                // vocab to the model (it handles both punctuation and vocab
                // mapping with context). Otherwise — strict vocab::apply.
                // On LLM error we fall back to strict vocab::apply so the user
                // is never blocked beyond the 5s timeout (+retry). Like STT,
                // the edit walks the text stack within this request; entries
                // without a key are skipped.
                let postprocess_enabled = cfg["postprocess_enabled"].as_bool().unwrap_or(false);

                let mut llm_secs: Option<f32> = None;
                let mut llm_model: Option<String> = None;
                let mut llm_host: Option<String> = None;
                let mut llm_attempted = false;

                let text_entries = fallback::read_stack(&cfg, fallback::Stack::Text);
                let any_text_key = text_entries
                    .iter()
                    .any(|e| std::env::var(&e.key_env).map(|k| !k.is_empty()).unwrap_or(false));
                let (text, edited): (String, bool) = if postprocess_enabled && !text_entries.is_empty() {
                    if !any_text_key {
                        let msg = "no key set for any text provider".to_string();
                        debug_log::log(&format!("postprocess: {} — falling back to strict vocab", msg));
                        set_last_llm_error(Some(msg));
                        (vocab::apply(&raw_text), false)
                    } else {
                        let vocab_data = vocab::read_vocab();
                        llm_attempted = true;
                        let start = fallback::active_index(fallback::Stack::Text, fallback::cooldown(&cfg))
                            .min(text_entries.len() - 1);
                        // Timed even on failure: a timed-out LLM burns its full
                        // 5s timeout (+retry) before we fall back to vocab, and
                        // that lost time must show up in the log.
                        let t_llm = std::time::Instant::now();
                        let outcome = fallback::run_with_failover(
                            fallback::Stack::Text, &text_entries, start, fallback::threshold(&cfg),
                            |e, key| postprocess::edit_text(&raw_text, &vocab_data, &e.url, key, &e.model),
                        );
                        llm_secs = Some(t_llm.elapsed().as_secs_f32());
                        match outcome {
                            // Clearing on success means the Settings note only ever
                            // reflects the *current* state, not a stale failure.
                            Ok((edited_text, used)) => {
                                // Host (not the label) so the history shows the
                                // real endpoint that ran the edit — including
                                // which fallback rung it was.
                                let e = &text_entries[used];
                                llm_host = Some(entry_host(e).to_string());
                                llm_model = Some(e.model.clone());
                                set_last_llm_error(None);
                                (edited_text, true)
                            }
                            Err(msg) => {
                                debug_log::log(&format!("postprocess failed ({}) — falling back to strict vocab", msg));
                                set_last_llm_error(Some(msg));
                                // The failed attempt still identifies itself in
                                // the transcription log (edited=false).
                                let e = &text_entries[start];
                                llm_host = Some(entry_host(e).to_string());
                                llm_model = Some(e.model.clone());
                                (vocab::apply(&raw_text), false)
                            }
                        }
                    }
                } else {
                    (vocab::apply(&raw_text), false)
                };

                let preview: String = text.chars().take(80).collect();
                debug_log::log(&format!("transcription OK (edited={}): {:?}", edited, preview));
                if text.is_empty() {
                    let _ = app_handle.emit("status-detail", "No speech detected.");
                } else {
                    let _ = app_handle.emit("transcription", serde_json::json!({
                        "text": &text,
                        "duration": duration_secs,
                        "edited": edited,
                        "llm_host": llm_host,
                        "llm_model": llm_model,
                    }));

                    let _ = app_handle.emit("status-detail", "Inserting text...");

                    // Small delay so the UI updates and the previous app regains focus.
                    std::thread::sleep(std::time::Duration::from_millis(200));

                    // Direct keyboard input — does NOT touch the clipboard.
                    // If insert fails, the transcript is still in the log;
                    // the user can click it to copy manually.
                    let t_insert = std::time::Instant::now();
                    if let Err(e) = inserter::insert_text(&text) {
                        debug_log::log(&format!("insert error: {}", e));
                        let _ = app_handle.emit("error",
                            format!("Insert failed — text saved to log. {}", e));
                    }
                    let insert_secs = t_insert.elapsed().as_secs_f32();

                    let _ = app_handle.emit("status-detail", "Done!");

                    // Logged after insert so insert_secs/total_secs are real.
                    // The UI history already received the transcript via the
                    // `transcription` event above, so a crash here can't lose
                    // it from the user's view.
                    let total_secs = t_pipeline.elapsed().as_secs_f32();
                    logger::log_transcription(&logger::TranscriptionLog {
                        text: &text,
                        raw_text: if llm_attempted { Some(raw_text.as_str()) } else { None },
                        edited,
                        audio_secs: duration_secs,
                        stt_secs,
                        stt_model: &stt_model,
                        llm_secs,
                        llm_model: llm_model.as_deref(),
                        llm_host: llm_host.as_deref(),
                        insert_secs,
                        total_secs,
                        idle_secs,
                    });
                    debug_log::log(&format!(
                        "timing: stt={:.1}s llm={} insert={:.1}s total={:.1}s ({} chars)",
                        stt_secs,
                        llm_secs.map(|s| format!("{:.1}s", s)).unwrap_or_else(|| "off".into()),
                        insert_secs,
                        total_secs,
                        text.chars().count(),
                    ));
                }
            }
            Err(e) => {
                debug_log::log(&format!("transcription error: {}", e));
                let _ = app_handle.emit("error", e.clone());
            }
        }

        *last_dictation().lock().unwrap() = Some(std::time::Instant::now());
        let _ = app_handle.emit("transcribing", false);
    });
}

fn load_env_file(path: &std::path::Path, overwrite: bool) {
    if let Ok(contents) = std::fs::read_to_string(path) {
        for line in contents.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !key.starts_with('#') {
                    if overwrite || std::env::var(key).is_err() {
                        unsafe { std::env::set_var(key, value); }
                    }
                }
            }
        }
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ribbit").join("config.json"))
}

/// Days of transcript history to keep on disk. Default 7; the rolling window
/// is today plus the previous (N-1) days.
fn history_days() -> i64 {
    read_config()["history_days"].as_i64().unwrap_or(7).clamp(1, 365)
}

fn read_config() -> serde_json::Value {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}))
}

fn save_config(config: &serde_json::Value) -> Result<(), String> {
    let path = config_path().ok_or("Cannot find config directory")?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(config).unwrap())
        .map_err(|e| e.to_string())
}

/// Single source of truth for showing/hiding the main window from the tray.
///
/// macOS: the window is a non-activating NSPanel (see `mac_window::setup_panel`).
/// `show_panel` surfaces it on the user's CURRENT Space — over a full-screen app
/// included — and gives it keyboard, all without activating Ribbit, so there is
/// no Space teleport. A plain NSWindow could not do this at all; five earlier
/// attempts to coax one (MoveToActiveSpace, CanJoinAllSpaces|FullScreenAuxiliary,
/// accessory policy, dropping set_focus, orderFrontRegardless) each failed
/// because macOS simply won't show a normal window over a foreign full-screen
/// Space. The panel is the mechanism that can.
///
/// Other platforms keep the plain window show/hide (no Spaces to worry about).
fn toggle_main_window(app: &AppHandle, label: &tauri::menu::MenuItem<tauri::Wry>) {
    #[cfg(target_os = "macos")]
    {
        // The panel hides itself the moment focus leaves it (mac_window), and
        // clicking this very icon is what took the focus away — so by the time we
        // run, an open panel already reads as hidden. Showing it again here would
        // make the icon unable to close the window (open → auto-hide → shown
        // again, a flicker). `just_auto_hid` is that "the click you are handling
        // is the one that closed it" signal.
        let visible = mac_window::panel_visible(app);
        let dismissed_by_this_click = !visible && mac_window::just_auto_hid();
        crate::debug_log::log(&format!(
            "tray toggle: visible={} dismissed_by_this_click={}",
            visible, dismissed_by_this_click
        ));
        if visible || dismissed_by_this_click {
            mac_window::hide_panel(app);
            let _ = label.set_text("Show Ribbit");
        } else {
            mac_window::show_panel(app);
            let _ = label.set_text("Hide Ribbit");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let Some(w) = app.get_webview_window("main") else { return };
        let visible = w.is_visible().unwrap_or(false);
        if visible && w.is_focused().unwrap_or(false) {
            let _ = w.hide();
            let _ = label.set_text("Show Ribbit");
        } else {
            // unminimize is harmless if not minimized — safety net for the "_"
            // button's real minimize; show()+set_focus raise and focus it.
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
            let _ = label.set_text("Hide Ribbit");
        }
    }
}

fn register_shortcut(app: &AppHandle, shortcut: Shortcut) -> Result<(), String> {
    use tauri_plugin_global_shortcut::ShortcutState;
    app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
        let state = app.state::<Arc<Mutex<RecordingState>>>();
        match event.state() {
            ShortcutState::Pressed => {
                if !state.lock().unwrap().is_recording {
                    start_recording(state.inner(), app);
                }
                // Ignore repeated Pressed events (key held down)
            }
            ShortcutState::Released => {
                if state.lock().unwrap().is_recording {
                    stop_recording_and_transcribe(state.inner(), app);
                }
            }
        }
    }).map_err(|e| {
        debug_log::log(&format!("shortcut registration failed: {}", e));
        e.to_string()
    })
}

/// Build an audio-stack entry from a catalog provider name.
fn audio_entry_json(id: &str, name: &str) -> serde_json::Value {
    let p = transcribe::find_audio_provider(name).expect("known audio provider");
    serde_json::json!({
        "id": id, "label": p.label, "url": p.url,
        "model": p.default_model, "key_env": p.key_env,
    })
}

/// Fold legacy single-provider settings into the audio/text stacks. Idempotent:
/// once a stack array exists it's left untouched, so this is safe on every
/// launch. Reads key presence from the env (already loaded) to seed sensibly.
fn migrate_stacks(config: &mut serde_json::Value) -> bool {
    let groq = std::env::var("GROQ_API_KEY").map(|k| !k.is_empty()).unwrap_or(false);
    let openai = std::env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false);
    migrate_stacks_inner(config, groq, openai)
}

/// Pure core of the migration — `groq`/`openai` are "is that key present".
/// Split out so it can be unit-tested without touching process env.
fn migrate_stacks_inner(config: &mut serde_json::Value, groq: bool, openai: bool) -> bool {
    let mut changed = false;

    if !config.get("audio_providers").map(|v| v.is_array()).unwrap_or(false) {
        // Primary = groq unless only an OpenAI key exists. A fresh install with
        // no key still seeds groq so the setup screen has an entry to fill.
        let mut arr = Vec::new();
        if groq || !openai {
            arr.push(audio_entry_json("a_groq", "groq"));
            if openai {
                arr.push(audio_entry_json("a_openai", "openai"));
            }
        } else {
            arr.push(audio_entry_json("a_openai", "openai"));
        }
        config["audio_providers"] = serde_json::Value::Array(arr);
        changed = true;
    }

    if !config.get("text_providers").map(|v| v.is_array()).unwrap_or(false) {
        // Seed from the old llm_provider + its model override (or default).
        let prov_name = config["llm_provider"]
            .as_str()
            .unwrap_or(postprocess::DEFAULT_PROVIDER)
            .to_string();
        let p = postprocess::find_provider(&prov_name)
            .unwrap_or_else(|| postprocess::find_provider(postprocess::DEFAULT_PROVIDER).unwrap());
        let model = config["llm_provider_models"]
            .get(p.name)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(p.default_model)
            .to_string();
        config["text_providers"] = serde_json::json!([{
            "id": "t_primary", "label": p.label, "url": p.base_url,
            "model": model, "key_env": p.env_var,
        }]);
        changed = true;
    }

    if !config.get("fallback_threshold").map(|v| v.is_u64()).unwrap_or(false) {
        config["fallback_threshold"] = serde_json::json!(2);
        changed = true;
    }
    if !config.get("fallback_cooldown_mins").map(|v| v.is_u64()).unwrap_or(false) {
        config["fallback_cooldown_mins"] = serde_json::json!(60);
        changed = true;
    }

    changed
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn migrate_seeds_groq_primary_with_openai_fallback() {
        let mut cfg = serde_json::json!({});
        assert!(migrate_stacks_inner(&mut cfg, true, true));
        let audio = fallback::read_stack(&cfg, fallback::Stack::Audio);
        assert_eq!(audio.len(), 2);
        assert_eq!(audio[0].label, "groq");
        assert_eq!(audio[1].label, "openai");
        assert_eq!(audio[0].key_env, "GROQ_API_KEY");
    }

    #[test]
    fn migrate_seeds_openai_primary_when_only_openai_key() {
        let mut cfg = serde_json::json!({});
        migrate_stacks_inner(&mut cfg, false, true);
        let audio = fallback::read_stack(&cfg, fallback::Stack::Audio);
        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].label, "openai");
    }

    #[test]
    fn migrate_seeds_groq_on_fresh_install() {
        let mut cfg = serde_json::json!({});
        migrate_stacks_inner(&mut cfg, false, false);
        let audio = fallback::read_stack(&cfg, fallback::Stack::Audio);
        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].label, "groq");
    }

    #[test]
    fn migrate_seeds_text_from_legacy_provider_and_model() {
        let mut cfg = serde_json::json!({
            "llm_provider": "routerai",
            "llm_provider_models": { "routerai": "custom/model-x" }
        });
        migrate_stacks_inner(&mut cfg, true, false);
        let text = fallback::read_stack(&cfg, fallback::Stack::Text);
        assert_eq!(text.len(), 1);
        assert_eq!(text[0].label, "routerai");
        assert_eq!(text[0].model, "custom/model-x");
        assert_eq!(text[0].key_env, "ROUTERAI_API_KEY");
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut cfg = serde_json::json!({});
        assert!(migrate_stacks_inner(&mut cfg, true, true));
        // Second pass over an already-migrated config changes nothing.
        assert!(!migrate_stacks_inner(&mut cfg, true, true));
    }

    #[test]
    fn migrate_sets_default_knobs() {
        let mut cfg = serde_json::json!({});
        migrate_stacks_inner(&mut cfg, true, false);
        assert_eq!(cfg["fallback_threshold"], 2);
        assert_eq!(cfg["fallback_cooldown_mins"], 60);
    }

    #[test]
    fn next_id_avoids_collisions() {
        let cfg = serde_json::json!({
            "audio_providers": [{"id": "p1", "url": "u", "key_env": "K"}],
            "text_providers": [{"id": "p3", "url": "u", "key_env": "K"}]
        });
        assert_eq!(next_provider_id(&cfg), "p4");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug_log::log("=== Ribbit starting ===");
    logger::cleanup_old_logs(history_days());
    tcc_reset::ensure_permissions("com.ribbit.app");

    // Load .env from config dir (primary)
    if let Some(config_dir) = dirs::config_dir() {
        let env_path = config_dir.join("ribbit").join(".env");
        debug_log::log(&format!("loading env from {:?}", env_path));
        load_env_file(&env_path, true);
    }

    // Also try .env in current directory (development fallback)
    load_env_file(std::path::Path::new(".env"), false);

    // One-time migration: fold the old single-provider settings into the new
    // audio/text provider stacks. No-op once the stacks exist, so it's safe to
    // run on every launch. Runs after env load so key presence can seed it.
    {
        let mut cfg = read_config();
        if migrate_stacks(&mut cfg) {
            match save_config(&cfg) {
                Ok(()) => debug_log::log("migrated config to provider stacks"),
                Err(e) => debug_log::log(&format!("stack migration save failed: {}", e)),
            }
        }
    }

    debug_log::log(&format!(
        "audio keys present: groq={} openai={}",
        std::env::var("GROQ_API_KEY").map(|k| !k.is_empty()).unwrap_or(false),
        std::env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false),
    ));

    // Warm up HTTP client (TLS handshake) in background so first transcription is fast
    std::thread::spawn(|| {
        transcribe::warm_up_client();
    });

    let config = read_config();

    let saved_shortcut = config["shortcut"]
        .as_str()
        .unwrap_or("ctrl+alt+space")
        .to_string();

    // Restore saved sound pack
    if let Some(pack) = config["sound_pack"].as_str() {
        sound::set_pack(pack);
    }

    let state = Arc::new(Mutex::new(RecordingState {
        is_recording: false,
        audio_data: Vec::new(),
        sample_rate: 16000,
        current_shortcut: saved_shortcut,
    }));

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build());
    // macOS-only: converts the main window into an NSPanel (see mac_window).
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }
    builder
        .invoke_handler(tauri::generate_handler![get_config, set_api_key, get_log_history, set_history_days, get_debug_log, js_debug_log, set_always_on_top, get_shortcut, set_shortcut, test_sound, hide_to_tray, minimize_window, check_for_update, install_update, get_current_version, get_sound_pack, set_sound_pack, get_languages, set_languages, get_vocab, set_vocab, add_vocab_entry, remove_vocab_alias, remove_vocab_entry, set_postprocess_enabled, get_llm_last_error, list_provider_catalog, add_provider, remove_provider, set_provider_field, move_provider, set_provider_key, set_fallback_threshold, set_fallback_cooldown])
        .setup(move |app| {
            let handle = app.handle().clone();

            // macOS: run as a menu-bar accessory (no Dock icon, no Cmd-Tab).
            // Correct type for a tray utility, and it keeps app activation out
            // of the picture — the panel (below) does the actual Space work.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Ask macOS for the mic on launch so the app registers in Privacy →
            // Microphone and can be granted. Without an explicit request, cpal
            // just opens a silent stream when the grant is missing (e.g. right
            // after the one-time reset on the ad-hoc → cert switch) and never
            // prompts — and that pane has no manual "+". Self-heals future resets.
            #[cfg(target_os = "macos")]
            mic_permission::request_mic_access();

            // System tray
            let show = MenuItemBuilder::with_id("show", "Show Ribbit").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit Ribbit").build(app)?;
            let menu = MenuBuilder::new(app).item(&show).item(&quit).build()?;

            let show_for_menu = show.clone();
            let show_for_tray = show.clone();
            // hide_to_tray (the X button) flips this label too.
            app.manage(show.clone());

            let mut tray_builder = TrayIconBuilder::new()
                .tooltip("Ribbit - Voice to Text")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    if event.id() == "show" {
                        toggle_main_window(app, &show_for_menu);
                    } else if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up, ..
                    } = event {
                        toggle_main_window(tray.app_handle(), &show_for_tray);
                    }
                });

            // Use app icon for tray
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            // macOS-only window tweaks. Convert to the NSPanel first, then round
            // its corners. No-op on other platforms.
            if let Some(main_window) = app.get_webview_window("main") {
                if let Err(e) = mac_window::setup_panel(&main_window) {
                    debug_log::log(&format!("panel setup: {}", e));
                }
                if let Err(e) = mac_window::apply_rounded_corners(&main_window, 10.0) {
                    debug_log::log(&format!("rounded corners: {}", e));
                }
            }

            // Restore the saved Always-on-Top choice; the panel's yield-on-blur
            // behaviour reads the same flag.
            let on_top = read_config()["always_on_top"].as_bool().unwrap_or(false);
            if let Err(e) = apply_always_on_top(&handle, on_top) {
                debug_log::log(&format!("always-on-top restore: {}", e));
            }

            // Manage state for commands and shortcut handler
            app.manage(Arc::clone(&state));
            app.manage(sound::SoundPlayer::new());

            // Register saved shortcut
            let shortcut_str = state.lock().unwrap().current_shortcut.clone();
            let shortcut: Shortcut = shortcut_str.parse()
                .map_err(|e| format!("Failed to parse shortcut: {}", e))?;

            debug_log::log(&format!("registering hotkey: {}", shortcut_str));
            register_shortcut(&handle, shortcut)?;

            // Auto-check for updates: shortly after launch, then on an
            // interval. Ribbit lives in the tray all day, so a release that
            // ships while it's running must light the gear on its own —
            // polling every 30 min spares the user the manual "check update".
            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                // Small delay so the app finishes loading first.
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                loop {
                    debug_log::log("update: running auto-check...");
                    match update_handle.updater() {
                        Ok(updater) => match updater.check().await {
                            Ok(Some(update)) => {
                                debug_log::log(&format!("update: v{} available", update.version));
                                let _ = update_handle.emit("update-available", &update.version);
                                // Gear is lit — nothing more to poll for.
                                break;
                            }
                            Ok(None) => debug_log::log("update: up to date"),
                            Err(e) => debug_log::log(&format!("update: auto-check failed: {}", e)),
                        },
                        Err(e) => debug_log::log(&format!("update: auto-check error: {}", e)),
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                }
            });

            debug_log::log("setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ribbit");
}

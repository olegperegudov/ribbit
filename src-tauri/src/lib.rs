mod audio;
mod transcribe;
mod inserter;
mod logger;
mod debug_log;
mod usage;
mod sound;
mod vocab;

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

#[tauri::command]
fn get_config() -> Result<serde_json::Value, String> {
    let (provider, key) = if let Ok(k) = std::env::var("GROQ_API_KEY") {
        ("groq", k)
    } else if let Ok(k) = std::env::var("OPENAI_API_KEY") {
        ("openai", k)
    } else {
        ("none", String::new())
    };

    let preview = if key.len() > 8 {
        format!("{}...{}", &key[..4], &key[key.len()-4..])
    } else if !key.is_empty() {
        "****".to_string()
    } else {
        "".to_string()
    };

    Ok(serde_json::json!({
        "has_api_key": !key.is_empty(),
        "api_key_preview": preview,
        "provider": provider,
    }))
}

#[tauri::command]
fn get_log_history(limit: usize) -> Vec<serde_json::Value> {
    logger::read_recent_entries(if limit == 0 { 50 } else { limit })
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
fn get_usage_stats() -> Vec<serde_json::Value> {
    usage::get_monthly()
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

#[tauri::command]
fn hide_to_tray(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
}

#[tauri::command]
fn show_from_tray(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
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
    if let Some(w) = app.get_webview_window("main") {
        w.set_always_on_top(value).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_api_key(key: String, provider: Option<String>) -> Result<(), String> {
    let env_path = dirs::config_dir()
        .ok_or("Cannot find config directory")?
        .join("ribbit")
        .join(".env");

    std::fs::create_dir_all(env_path.parent().unwrap())
        .map_err(|e| e.to_string())?;

    // Detect provider from key prefix or explicit parameter
    let prov = provider.unwrap_or_else(|| {
        if key.starts_with("gsk_") { "groq".into() } else { "openai".into() }
    });

    let var_name = if prov == "groq" { "GROQ_API_KEY" } else { "OPENAI_API_KEY" };

    // Read existing env, replace/add the key
    let existing = std::fs::read_to_string(&env_path).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines()
        .filter(|l| !l.starts_with("GROQ_API_KEY=") && !l.starts_with("OPENAI_API_KEY="))
        .map(|l| l.to_string())
        .collect();
    lines.push(format!("{}={}", var_name, key));

    std::fs::write(&env_path, lines.join("\n") + "\n")
        .map_err(|e| e.to_string())?;

    unsafe { std::env::set_var(var_name, &key); }
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

    // Use a dedicated thread for the whole transcribe+insert flow.
    // Avoids tokio runtime issues and ensures enigo runs on a proper thread.
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let _ = app_handle.emit("transcribing", true);

        // Read configured languages for Whisper hint
        let languages = get_languages();

        // Create a blocking reqwest client instead of async
        let result = transcribe::transcribe_audio_blocking(&audio_data, sample_rate, &languages);

        match result {
            Ok(raw_text) => {
                let text = vocab::apply(&raw_text);
                let preview: String = text.chars().take(80).collect();
                debug_log::log(&format!("transcription OK: {:?}", preview));
                if text.is_empty() {
                    let _ = app_handle.emit("status-detail", "No speech detected.");
                } else {
                    // Log transcription first — ensures text is saved even if paste fails
                    logger::log_transcription(&text, duration_secs);
                    usage::record(duration_secs);
                    let _ = app_handle.emit("transcription", serde_json::json!({
                        "text": &text,
                        "duration": duration_secs,
                    }));

                    let _ = app_handle.emit("status-detail", "Inserting text...");

                    // Small delay to let the UI update and the previous app regain focus
                    std::thread::sleep(std::time::Duration::from_millis(200));

                    if let Err(e) = inserter::insert_text(&text) {
                        debug_log::log(&format!("insert error: {}", e));
                        let _ = app_handle.emit("error",
                            format!("Paste failed — text saved to log. {}", e));
                    } else {
                        debug_log::log("text inserted OK");
                    }

                    let _ = app_handle.emit("status-detail", "Done!");
                }
            }
            Err(e) => {
                debug_log::log(&format!("transcription error: {}", e));
                let _ = app_handle.emit("error", e.clone());
            }
        }

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug_log::log("=== Ribbit starting ===");
    logger::cleanup_old_logs();

    // Load .env from config dir (primary)
    if let Some(config_dir) = dirs::config_dir() {
        let env_path = config_dir.join("ribbit").join(".env");
        debug_log::log(&format!("loading env from {:?}", env_path));
        load_env_file(&env_path, true);
    }

    // Also try .env in current directory (development fallback)
    load_env_file(std::path::Path::new(".env"), false);

    let has_key = std::env::var("OPENAI_API_KEY").is_ok();
    debug_log::log(&format!("API key present: {}", has_key));

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

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![get_config, set_api_key, get_log_history, get_debug_log, set_always_on_top, get_usage_stats, get_shortcut, set_shortcut, test_sound, hide_to_tray, show_from_tray, check_for_update, install_update, get_current_version, get_sound_pack, set_sound_pack, get_languages, set_languages, get_vocab, set_vocab, add_vocab_entry, remove_vocab_alias, remove_vocab_entry])
        .setup(move |app| {
            let handle = app.handle().clone();

            // System tray
            let show = MenuItemBuilder::with_id("show", "Show Ribbit").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit Ribbit").build(app)?;
            let menu = MenuBuilder::new(app).item(&show).item(&quit).build()?;

            let show_for_menu = show.clone();
            let show_for_tray = show.clone();

            let mut tray_builder = TrayIconBuilder::new()
                .tooltip("Ribbit - Voice to Text")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    if event.id() == "show" {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_minimized().unwrap_or(false) || !w.is_visible().unwrap_or(true) {
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                                let _ = show_for_menu.set_text("Hide Ribbit");
                            } else {
                                let _ = w.minimize();
                                let _ = show_for_menu.set_text("Show Ribbit");
                            }
                        }
                    } else if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up, ..
                    } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_minimized().unwrap_or(false) || !w.is_visible().unwrap_or(true) {
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                                let _ = show_for_tray.set_text("Hide Ribbit");
                            } else {
                                let _ = w.minimize();
                                let _ = show_for_tray.set_text("Show Ribbit");
                            }
                        }
                    }
                });

            // Use app icon for tray
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            // Manage state for commands and shortcut handler
            app.manage(Arc::clone(&state));
            app.manage(sound::SoundPlayer::new());

            // Register saved shortcut
            let shortcut_str = state.lock().unwrap().current_shortcut.clone();
            let shortcut: Shortcut = shortcut_str.parse()
                .map_err(|e| format!("Failed to parse shortcut: {}", e))?;

            debug_log::log(&format!("registering hotkey: {}", shortcut_str));
            register_shortcut(&handle, shortcut)?;

            // Auto-check for updates on every launch
            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                // Small delay so the app finishes loading first
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                debug_log::log("update: running auto-check...");
                let now = chrono::Utc::now().timestamp();
                let updater = match update_handle.updater() {
                    Ok(u) => u,
                    Err(e) => {
                        debug_log::log(&format!("update: auto-check error: {}", e));
                        return;
                    }
                };
                match updater.check().await {
                    Ok(Some(update)) => {
                        debug_log::log(&format!("update: v{} available", update.version));
                        let _ = update_handle.emit("update-available", &update.version);
                    }
                    Ok(None) => {
                        debug_log::log("update: up to date");
                    }
                    Err(e) => {
                        debug_log::log(&format!("update: auto-check failed: {}", e));
                    }
                }
                // Save check timestamp regardless of result
                let mut cfg = read_config();
                cfg["last_update_check"] = serde_json::json!(now);
                let _ = save_config(&cfg);
            });

            debug_log::log("setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ribbit");
}

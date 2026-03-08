mod audio;
mod transcribe;
mod inserter;
mod logger;

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, Emitter,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

struct RecordingState {
    is_recording: bool,
    audio_data: Vec<f32>,
    sample_rate: u32,
}

#[tauri::command]
fn get_config() -> Result<serde_json::Value, String> {
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    Ok(serde_json::json!({
        "has_api_key": !api_key.is_empty(),
        "api_key_preview": if api_key.len() > 8 {
            format!("{}...{}", &api_key[..4], &api_key[api_key.len()-4..])
        } else if !api_key.is_empty() {
            "****".to_string()
        } else {
            "".to_string()
        },
    }))
}

#[tauri::command]
fn get_log_history(limit: usize) -> Vec<serde_json::Value> {
    logger::read_recent_entries(if limit == 0 { 50 } else { limit })
}

#[tauri::command]
async fn set_api_key(key: String) -> Result<(), String> {
    let env_path = dirs::config_dir()
        .ok_or("Cannot find config directory")?
        .join("ribbit")
        .join(".env");

    std::fs::create_dir_all(env_path.parent().unwrap())
        .map_err(|e| e.to_string())?;

    std::fs::write(&env_path, format!("OPENAI_API_KEY={}\n", key))
        .map_err(|e| e.to_string())?;

    unsafe { std::env::set_var("OPENAI_API_KEY", &key); }
    Ok(())
}

fn start_recording(state: &Arc<Mutex<RecordingState>>, app: &AppHandle) {
    let mut s = state.lock().unwrap();
    if s.is_recording {
        return;
    }
    s.is_recording = true;
    s.audio_data.clear();
    drop(s);

    let _ = app.emit("recording-status", true);
    let _ = app.emit("status-detail", "Listening...");

    let state_clone = Arc::clone(state);
    std::thread::spawn(move || {
        audio::record_audio(state_clone);
    });
}

fn stop_recording_and_transcribe(state: &Arc<Mutex<RecordingState>>, app: &AppHandle) {
    let (audio_data, sample_rate) = {
        let mut s = state.lock().unwrap();
        if !s.is_recording {
            return;
        }
        s.is_recording = false;
        (s.audio_data.clone(), s.sample_rate)
    };

    let _ = app.emit("recording-status", false);

    let duration_secs = audio_data.len() as f32 / sample_rate as f32;

    if audio_data.is_empty() || duration_secs < 0.3 {
        let _ = app.emit("status-detail", "Too short, try again.");
        return;
    }

    let _ = app.emit("status-detail",
        format!("Ribbiting... ({:.1}s of audio)", duration_secs));

    let app_handle = app.clone();
    tokio::spawn(async move {
        let _ = app_handle.emit("transcribing", true);

        match transcribe::transcribe_audio(&audio_data, sample_rate).await {
            Ok(text) => {
                if text.is_empty() {
                    let _ = app_handle.emit("status-detail", "No speech detected.");
                } else {
                    let _ = app_handle.emit("status-detail", "Inserting text...");

                    if let Err(e) = inserter::insert_text(&text) {
                        let _ = app_handle.emit("error",
                            format!("Paste failed ({}). Text copied to clipboard.", e));
                    }

                    logger::log_transcription(&text);
                    let _ = app_handle.emit("transcription", &text);
                    let _ = app_handle.emit("status-detail", "Done!");
                }
            }
            Err(e) => {
                eprintln!("Transcription error: {}", e);
                let _ = app_handle.emit("error", e.clone());
                let _ = app_handle.emit("status-detail", format!("Error: {}", e));
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env from config dir (primary)
    if let Some(config_dir) = dirs::config_dir() {
        load_env_file(&config_dir.join("ribbit").join(".env"), true);
    }

    // Also try .env in current directory (development fallback)
    load_env_file(std::path::Path::new(".env"), false);

    let state = Arc::new(Mutex::new(RecordingState {
        is_recording: false,
        audio_data: Vec::new(),
        sample_rate: 16000,
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![get_config, set_api_key, get_log_history])
        .setup(move |app| {
            let handle = app.handle().clone();

            // System tray
            let quit = MenuItemBuilder::with_id("quit", "Quit Ribbit").build(app)?;
            let menu = MenuBuilder::new(app).item(&quit).build()?;

            let _tray = TrayIconBuilder::new()
                .tooltip("Ribbit - Voice to Text")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            // Global shortcut: Ctrl+Alt+Space (toggle)
            let shortcut: Shortcut = "ctrl+alt+space".parse().unwrap();
            let state_for_shortcut = Arc::clone(&state);

            handle.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, _event| {
                let is_recording = {
                    let s = state_for_shortcut.lock().unwrap();
                    s.is_recording
                };

                if is_recording {
                    stop_recording_and_transcribe(&state_for_shortcut, app);
                } else {
                    start_recording(&state_for_shortcut, app);
                }
            })?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ribbit");
}

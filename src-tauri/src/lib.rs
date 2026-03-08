mod audio;
mod transcribe;
mod inserter;
mod logger;

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, Emitter,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    image::Image,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

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
    }))
}

#[tauri::command]
async fn set_api_key(key: String) -> Result<(), String> {
    // For MVP: save to .env file next to the executable
    let env_path = dirs::config_dir()
        .ok_or("Cannot find config directory")?
        .join("ribbit")
        .join(".env");

    std::fs::create_dir_all(env_path.parent().unwrap())
        .map_err(|e| e.to_string())?;

    std::fs::write(&env_path, format!("OPENAI_API_KEY={}\n", key))
        .map_err(|e| e.to_string())?;

    std::env::set_var("OPENAI_API_KEY", &key);
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

    if audio_data.is_empty() {
        return;
    }

    let app_handle = app.clone();
    tokio::spawn(async move {
        let _ = app_handle.emit("transcribing", true);

        match transcribe::transcribe_audio(&audio_data, sample_rate).await {
            Ok(text) => {
                if !text.is_empty() {
                    // Insert text at cursor position
                    if let Err(e) = inserter::insert_text(&text) {
                        eprintln!("Failed to insert text: {}", e);
                        let _ = app_handle.emit("error", format!("Insert failed: {}", e));
                    }

                    // Log transcription
                    logger::log_transcription(&text);

                    let _ = app_handle.emit("transcription", &text);
                }
            }
            Err(e) => {
                eprintln!("Transcription error: {}", e);
                let _ = app_handle.emit("error", format!("Transcription failed: {}", e));
            }
        }

        let _ = app_handle.emit("transcribing", false);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env from config dir
    if let Some(config_dir) = dirs::config_dir() {
        let env_path = config_dir.join("ribbit").join(".env");
        if env_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&env_path) {
                for line in contents.lines() {
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim();
                        if !key.is_empty() && !key.starts_with('#') {
                            std::env::set_var(key, value);
                        }
                    }
                }
            }
        }
    }

    // Also try .env in current directory (for development)
    if let Ok(contents) = std::fs::read_to_string(".env") {
        for line in contents.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !key.starts_with('#') {
                    // Don't override if already set from config dir
                    if std::env::var(key).is_err() {
                        std::env::set_var(key, value);
                    }
                }
            }
        }
    }

    let state = Arc::new(Mutex::new(RecordingState {
        is_recording: false,
        audio_data: Vec::new(),
        sample_rate: 16000,
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![get_config, set_api_key])
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

            // Global shortcut: Ctrl+Alt+Space
            let shortcut: Shortcut = "ctrl+alt+space".parse().unwrap();
            let state_for_shortcut = Arc::clone(&state);

            handle.plugin_global_shortcut().on_shortcut(
                shortcut,
                move |app, _shortcut, event| {
                    match event.state() {
                        ShortcutState::Pressed => {
                            start_recording(&state_for_shortcut, app);
                        }
                        ShortcutState::Released => {
                            stop_recording_and_transcribe(&state_for_shortcut, app);
                        }
                    }
                },
            )?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ribbit");
}

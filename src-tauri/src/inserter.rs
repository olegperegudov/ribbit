use arboard::Clipboard;
use enigo::{Enigo, Key, Keyboard, Settings, Direction};
use std::thread;
use std::time::Duration;

use crate::debug_log;

pub fn insert_text(text: &str) -> Result<(), String> {
    // Save current clipboard content
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let old_clipboard = clipboard.get_text().ok();

    // Set our text to clipboard
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // Simulate paste: Ctrl+V on Windows, Cmd+V on macOS
    let paste_result = (|| -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let modifier = Key::Meta;
        #[cfg(not(target_os = "macos"))]
        let modifier = Key::Control;

        enigo.key(modifier, Direction::Press).map_err(|e| format!("mod press: {}", e))?;
        thread::sleep(Duration::from_millis(20));
        enigo.key(Key::Unicode('v'), Direction::Press).map_err(|e| format!("v press: {}", e))?;
        thread::sleep(Duration::from_millis(20));
        enigo.key(Key::Unicode('v'), Direction::Release).map_err(|e| format!("v release: {}", e))?;
        thread::sleep(Duration::from_millis(20));
        enigo.key(modifier, Direction::Release).map_err(|e| format!("mod release: {}", e))?;
        debug_log::log(if cfg!(target_os = "macos") { "Cmd+V simulated" } else { "Ctrl+V simulated" });
        Ok(())
    })();

    if let Err(e) = &paste_result {
        debug_log::log(&format!("Ctrl+V failed ({}), falling back to direct typing", e));
        // Fallback: type text directly using Unicode input events
        enigo.text(text).map_err(|e| format!("text fallback: {}", e))?;
        debug_log::log("text typed via enigo.text() fallback");
    }

    // Wait for paste to complete, then restore clipboard
    thread::sleep(Duration::from_millis(100));
    if let Some(old) = old_clipboard {
        let _ = clipboard.set_text(old);
    }

    paste_result
}

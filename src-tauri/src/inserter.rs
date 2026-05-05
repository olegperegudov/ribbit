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

    // Simulate paste: Cmd+V on macOS, Ctrl+V on Windows.
    // Both branches use raw scan codes (Key::Other) instead of Key::Unicode.
    // On macOS, Key::Unicode triggers HIToolbox layout lookup
    // (TSMGetInputSourceProperty), which asserts main-thread and SIGTRAPs
    // when called from the background thread that runs post-transcription.
    // 0x09 = kVK_ANSI_V; Cmd+physical-V pastes regardless of layout.
    let paste_result = (|| -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            enigo.key(Key::Meta, Direction::Press).map_err(|e| format!("cmd press: {}", e))?;
            thread::sleep(Duration::from_millis(20));
            enigo.key(Key::Other(0x09), Direction::Press).map_err(|e| format!("v press: {}", e))?;
            thread::sleep(Duration::from_millis(20));
            enigo.key(Key::Other(0x09), Direction::Release).map_err(|e| format!("v release: {}", e))?;
            thread::sleep(Duration::from_millis(20));
            enigo.key(Key::Meta, Direction::Release).map_err(|e| format!("cmd release: {}", e))?;
            debug_log::log("Cmd+V simulated");
        }
        #[cfg(not(target_os = "macos"))]
        {
            enigo.key(Key::Control, Direction::Press).map_err(|e| format!("ctrl press: {}", e))?;
            thread::sleep(Duration::from_millis(20));
            enigo.key(Key::Other(0x56), Direction::Press).map_err(|e| format!("v press: {}", e))?;
            thread::sleep(Duration::from_millis(20));
            enigo.key(Key::Other(0x56), Direction::Release).map_err(|e| format!("v release: {}", e))?;
            thread::sleep(Duration::from_millis(20));
            enigo.key(Key::Control, Direction::Release).map_err(|e| format!("ctrl release: {}", e))?;
            debug_log::log("Ctrl+V simulated via VK_V");
        }
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

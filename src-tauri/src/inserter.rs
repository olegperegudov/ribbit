use arboard::Clipboard;
use enigo::{Enigo, Keyboard, Settings, Direction};
use std::thread;
use std::time::Duration;

use crate::debug_log;

pub fn insert_text(text: &str) -> Result<(), String> {
    // Save current clipboard content
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let old_clipboard = clipboard.get_text().ok();

    // Set our text to clipboard
    clipboard.set_text(text).map_err(|e| e.to_string())?;

    // Small delay to ensure clipboard is ready
    thread::sleep(Duration::from_millis(50));

    // Simulate Ctrl+V using raw virtual key codes (layout-independent)
    // Key::Unicode('v') fails on non-Latin layouts (e.g. Russian)
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    // VK_CONTROL = 0x11, VK_V = 0x56
    enigo.raw(0x11, Direction::Press).map_err(|e| format!("ctrl press: {}", e))?;
    thread::sleep(Duration::from_millis(10));
    enigo.raw(0x56, Direction::Press).map_err(|e| format!("v press: {}", e))?;
    thread::sleep(Duration::from_millis(10));
    enigo.raw(0x56, Direction::Release).map_err(|e| format!("v release: {}", e))?;
    thread::sleep(Duration::from_millis(10));
    enigo.raw(0x11, Direction::Release).map_err(|e| format!("ctrl release: {}", e))?;

    debug_log::log("Ctrl+V simulated via raw keycodes");

    // Wait for paste to complete, then restore clipboard
    thread::sleep(Duration::from_millis(100));
    if let Some(old) = old_clipboard {
        let _ = clipboard.set_text(old);
    }

    Ok(())
}

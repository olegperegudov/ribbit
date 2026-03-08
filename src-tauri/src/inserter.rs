use arboard::Clipboard;
use enigo::{Enigo, Key, Keyboard, Settings, Direction};
use std::thread;
use std::time::Duration;

pub fn insert_text(text: &str) -> Result<(), String> {
    // Save current clipboard content
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    let old_clipboard = clipboard.get_text().ok();

    // Set our text to clipboard
    clipboard.set_text(text).map_err(|e| e.to_string())?;

    // Small delay to ensure clipboard is ready
    thread::sleep(Duration::from_millis(50));

    // Simulate Ctrl+V to paste
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;

    // Wait for paste to complete, then restore clipboard
    thread::sleep(Duration::from_millis(100));
    if let Some(old) = old_clipboard {
        let _ = clipboard.set_text(old);
    }

    Ok(())
}

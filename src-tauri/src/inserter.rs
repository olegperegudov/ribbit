//! Insert transcribed text at the user's current cursor position.
//!
//! Uses direct Unicode keyboard events (enigo.text) on every platform.
//! We deliberately do NOT use the system clipboard: clipboard-based paste
//! (set text → Cmd+V/Ctrl+V) overwrites whatever the user had saved there,
//! and every set_text() creates a new entry in clipboard managers
//! (Maccy/Paste/Alfred). Direct typing leaves the clipboard untouched.

use enigo::{Enigo, Keyboard, Settings};

use crate::debug_log;

pub fn insert_text(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| format!("text input failed: {}", e))?;
    debug_log::log(&format!("inserted {} chars at cursor", text.chars().count()));
    Ok(())
}

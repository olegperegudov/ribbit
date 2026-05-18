//! macOS-specific window tweaks.
//!
//! Tauri 2 with `decorations: false, transparent: true` produces a square
//! NSWindow; CSS `border-radius` clips only the DOM contents, so the corners
//! of the window itself show the desktop wallpaper through the transparent
//! gaps. The fix is to round the NSWindow's content layer directly via
//! AppKit — same effect macOS gives every standard titled window.
//!
//! No-op on every other platform (Windows 11 DWM already rounds borderless
//! windows itself; Linux is out of scope for now).

#[cfg(target_os = "macos")]
pub fn apply_rounded_corners(window: &tauri::WebviewWindow, radius: f64) -> Result<(), String> {
    use cocoa::base::{id, YES};
    use objc::{msg_send, sel, sel_impl};

    let ns_window = window.ns_window().map_err(|e| e.to_string())? as id;
    unsafe {
        let content_view: id = msg_send![ns_window, contentView];
        let _: () = msg_send![content_view, setWantsLayer: YES];
        let layer: id = msg_send![content_view, layer];
        if layer.is_null() {
            return Err("contentView.layer is null".into());
        }
        let _: () = msg_send![layer, setCornerRadius: radius];
        let _: () = msg_send![layer, setMasksToBounds: YES];
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn apply_rounded_corners(_window: &tauri::WebviewWindow, _radius: f64) -> Result<(), String> {
    Ok(())
}

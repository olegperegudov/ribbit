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
    use cocoa::base::{id, nil, YES};
    use objc::{msg_send, sel, sel_impl};

    let ns_window = window.ns_window().map_err(|e| e.to_string())? as id;
    unsafe {
        // contentView is the WKWebView. Its CALayer won't clip the window
        // outline by itself, so we apply cornerRadius/masksToBounds to its
        // *superview* — the private _NSThemeFrame that actually paints the
        // window edge. This is the same trick used by most macOS Electron/
        // Tauri apps that need round corners on a borderless window.
        let content_view: id = msg_send![ns_window, contentView];
        if content_view == nil {
            return Err("contentView is nil".into());
        }
        let frame_view: id = msg_send![content_view, superview];
        if frame_view == nil {
            return Err("frame view is nil".into());
        }
        let _: () = msg_send![frame_view, setWantsLayer: YES];
        let layer: id = msg_send![frame_view, layer];
        if layer == nil {
            return Err("frame view layer is nil".into());
        }
        let r: f64 = radius;
        let _: () = msg_send![layer, setCornerRadius: r];
        let _: () = msg_send![layer, setMasksToBounds: YES];
        crate::debug_log::log(&format!("rounded corners applied: radius={}", r));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn apply_rounded_corners(_window: &tauri::WebviewWindow, _radius: f64) -> Result<(), String> {
    Ok(())
}

/// Make the window appear on whatever Space the user is on when it's shown —
/// including full-screen Spaces. Two flags:
///
/// - NSWindowCollectionBehaviorCanJoinAllSpaces (1 << 0): the window has no
///   home Space; it simply shows on the active one.
/// - NSWindowCollectionBehaviorFullScreenAuxiliary (1 << 8): the window may
///   appear *over* a full-screen app. Without it macOS refuses to place a
///   normal window on a full-screen Space and switches to the first free
///   desktop instead — "clicking the tray icon while in a full-screen
///   terminal throws me to an empty desktop".
///
/// The previous MoveToActiveSpace (1 << 1) handled regular desktops only: a
/// normal window cannot *join* a full-screen Space, it can only be auxiliary
/// to it (and MoveToActiveSpace is mutually exclusive with CanJoinAllSpaces).
#[cfg(target_os = "macos")]
pub fn apply_spaces_behavior(window: &tauri::WebviewWindow) -> Result<(), String> {
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};

    let ns_window = window.ns_window().map_err(|e| e.to_string())? as id;
    unsafe {
        let behavior: u64 = (1 << 0) | (1 << 8);
        let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
        crate::debug_log::log("collectionBehavior=CanJoinAllSpaces|FullScreenAuxiliary applied");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn apply_spaces_behavior(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

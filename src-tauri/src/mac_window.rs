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

// The "main" window is turned into a non-activating NSPanel. A plain NSWindow
// cannot be shown over another app's full-screen Space — macOS silently refuses
// (confirmed from the app's own debug log: the show call ran, the window never
// appeared). Five earlier attempts poked window/app flags on a plain NSWindow
// and all failed for this reason. An NSPanel with the NonactivatingPanel style
// mask is the mechanism Spotlight/Raycast use: it surfaces on the current Space
// over full-screen apps AND can take keyboard, all WITHOUT activating the app
// (which is what teleported the user to desktop 1).
// The tauri_panel! macro expansion calls `.app_handle()`, which needs Manager.
#[cfg(target_os = "macos")]
use tauri::Manager as _;

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(RibbitPanel {
        config: {
            can_become_key_window: true,   // non-activating but still keyable → text fields work
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

/// Convert the borderless "main" window into the panel and configure it. Called
/// once at setup, after the accessory activation policy is set.
#[cfg(target_os = "macos")]
pub fn setup_panel(window: &tauri::WebviewWindow) -> Result<(), String> {
    use tauri_nspanel::{CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt};

    let panel = window.to_panel::<RibbitPanel>().map_err(|e| e.to_string())?;
    // Float above ordinary windows so the dictation HUD is never buried.
    panel.set_level(PanelLevel::Floating.value());
    // NonactivatingPanel: showing/keying the panel never activates the app, so
    // no Space switch. empty() keeps it borderless (decorations:false).
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    // Appear on the active Space and over a full-screen app.
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .into(),
    );
    // A utility panel hides itself when the app deactivates by default; we want
    // it to stay put until the user dismisses it.
    panel.set_hides_on_deactivate(false);
    crate::debug_log::log("panel: main window converted to non-activating NSPanel");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn setup_panel(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

/// Whether the main panel is currently on screen.
#[cfg(target_os = "macos")]
pub fn panel_visible(app: &tauri::AppHandle) -> bool {
    use tauri_nspanel::ManagerExt;
    app.get_webview_panel("main").map(|p| p.is_visible()).unwrap_or(false)
}

/// Show the panel on the user's CURRENT Space (over full-screen apps included)
/// and give it keyboard focus, without activating Ribbit.
#[cfg(target_os = "macos")]
pub fn show_panel(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    match app.get_webview_panel("main") {
        Ok(p) => p.show_and_make_key(),
        Err(e) => crate::debug_log::log(&format!("show_panel: panel not found ({:?})", e)),
    }
}

/// Hide the panel to the tray.
#[cfg(target_os = "macos")]
pub fn hide_panel(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(p) = app.get_webview_panel("main") {
        p.hide();
    }
}

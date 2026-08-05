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
            is_floating_panel: false       // window level is owned by the Always-on-Top toggle
        }
    })

    panel_event!(RibbitPanelEvents {
        window_did_resign_key(notification: &NSNotification) -> ()
    })
}

/// Mirrors the Always-on-Top setting for the resign-key handler, which runs on
/// the AppKit thread and has no access to the Tauri config.
#[cfg(target_os = "macos")]
static ALWAYS_ON_TOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "macos")]
pub fn set_always_on_top(value: bool) {
    ALWAYS_ON_TOP.store(value, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(target_os = "macos"))]
pub fn set_always_on_top(_value: bool) {}


/// Convert the borderless "main" window into the panel and configure it. Called
/// once at setup, after the accessory activation policy is set.
#[cfg(target_os = "macos")]
pub fn setup_panel(window: &tauri::WebviewWindow) -> Result<(), String> {
    use tauri_nspanel::{CollectionBehavior, StyleMask, WebviewWindowExt};

    let panel = window.to_panel::<RibbitPanel>().map_err(|e| e.to_string())?;
    // Window level is deliberately left at normal: show_and_make_key() brings
    // the panel to front when summoned from the tray, after which it behaves
    // like an ordinary window (other windows can cover it). Forcing the
    // floating level here made the panel always-on-top and overrode the
    // user's Always-on-Top toggle (set_always_on_top command).
    // NonactivatingPanel: showing/keying the panel never activates the app, so
    // no Space switch. empty() keeps it borderless (decorations:false).
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    // MoveToActiveSpace: the panel follows the user to whatever Space it is
    // summoned on. CanJoinAllSpaces (used until 0.7.72) instead pinned a copy
    // of it to *every* Space like the menu bar, so it resurfaced on top of each
    // desktop the user switched to and no click could push it back.
    // FullScreenAuxiliary keeps it able to appear over a full-screen app.
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .move_to_active_space()
            .into(),
    );
    // A utility panel hides itself when the app deactivates by default; we want
    // it to stay put until the user dismisses it.
    panel.set_hides_on_deactivate(false);

    // Get out of the way as soon as focus goes elsewhere: back to the tray, one
    // click away from returning. A non-activating panel is never reordered by
    // AppKit (its app never activates), so without this it keeps floating over
    // whatever the user clicked and only the X button removes it.
    //
    // Hiding — not orderBack. Two reasons, both learned the hard way in 0.7.73/74:
    // over a full-screen app there is no "behind" (the full-screen window IS the
    // Space and the panel is drawn over it as a FullScreenAuxiliary companion),
    // and `orderBack:` *orders a window in*, so calling it on the panel the user
    // had just dismissed with X resurrected it — it vanished and came straight
    // back, un-closable.
    //
    // Always-on-Top is what suspends this.
    let app = window.app_handle().clone();
    let handler = RibbitPanelEvents::new();
    handler.window_did_resign_key(move |_notification| {
        if ALWAYS_ON_TOP.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        hide_panel(&app);
        crate::debug_log::log("panel: focus left → hidden to tray");
    });
    panel.set_event_handler(Some(handler.as_ref()));
    // The delegate is weakly referenced by AppKit; this one lives for the whole
    // process, so hand its ownership to the panel by leaking it deliberately.
    std::mem::forget(handler);

    crate::debug_log::log("panel: main window converted to non-activating NSPanel");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn setup_panel(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
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

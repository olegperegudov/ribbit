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

/// Send the panel behind every other window on its level — what AppKit does for
/// an ordinary window when you click a different one.
#[cfg(target_os = "macos")]
fn order_back(app: &tauri::AppHandle) {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    let Some(window) = app.get_webview_window("main") else { return };
    let Ok(ns_window) = window.ns_window() else { return };
    unsafe {
        let _: () = msg_send![ns_window as id, orderBack: nil];
    }
}

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

    // Yield like an ordinary window: the moment focus goes to another window,
    // drop behind it. A non-activating panel never activates its app, so AppKit
    // does not reorder it for us — without this the panel keeps floating over
    // whatever the user clicked, and only the X button gets rid of it.
    // Always-on-Top, when the user asks for it, is what suspends this.
    let app = window.app_handle().clone();
    let handler = RibbitPanelEvents::new();
    handler.window_did_resign_key(move |_notification| {
        if ALWAYS_ON_TOP.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        order_back(&app);
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

/// Whether the main panel is currently on screen.
#[cfg(target_os = "macos")]
pub fn panel_visible(app: &tauri::AppHandle) -> bool {
    use tauri_nspanel::ManagerExt;
    app.get_webview_panel("main").map(|p| p.is_visible()).unwrap_or(false)
}

/// Whether the main panel is the key window — i.e. actually frontmost and
/// taking input, not merely ordered-in. `panel_visible` (NSWindow `isVisible`)
/// stays true even when the panel is fully covered by another app's window, so
/// a tray toggle keyed on visibility alone would `orderOut` an already-buried
/// panel and read as "the tray does nothing". Since 0.7.70 dropped the
/// always-on-top floating level, the panel sits at the normal level and can be
/// covered — so "raise unless frontmost" needs this second signal.
#[cfg(target_os = "macos")]
pub fn panel_is_key(app: &tauri::AppHandle) -> bool {
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};

    let Some(window) = app.get_webview_window("main") else { return false };
    let Ok(ns_window) = window.ns_window() else { return false };
    unsafe { msg_send![ns_window as id, isKeyWindow] }
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

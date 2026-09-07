//! Insert transcribed text at the user's current cursor position.
//!
//! Uses direct Unicode keyboard events (enigo.text) on every platform.
//! We deliberately do NOT use the system clipboard: clipboard-based paste
//! (set text → Cmd+V/Ctrl+V) overwrites whatever the user had saved there,
//! and every set_text() creates a new entry in clipboard managers
//! (Maccy/Paste/Alfred). Direct typing leaves the clipboard untouched.
//!
//! macOS drops these keystrokes silently, and that is what the checks below are
//! for. Two independent gates can swallow them: the TCC grant for synthetic
//! events (Accessibility / kTCCServicePostEvent), and secure event input, which
//! any password field switches on for the whole system. In both cases the events
//! die at the event tap while CGEventPost — and therefore enigo — still reports
//! success, so a dictation that went nowhere reads exactly like one that landed:
//! same "inserted N chars" line, same insert_secs in the daily log. Asking the
//! system about both gates before typing is the only way the logs can tell the
//! two apart afterwards.

use enigo::{Enigo, Keyboard, Settings};

use crate::debug_log;

/// Whether macOS currently trusts this process to post synthetic keyboard
/// events. `true` everywhere else — no other platform gates typing.
pub fn accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Boolean is a byte in C; taking it as `bool` would be UB for any other
        // value the framework might return.
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> u8;
        }
        unsafe { AXIsProcessTrusted() != 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Whether some app holds secure event input — a system-wide mode any password
/// field turns on. While it is up, no app can type for the user.
pub fn secure_input_active() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "Carbon", kind = "framework")]
        extern "C" {
            fn IsSecureEventInputEnabled() -> u8;
        }
        unsafe { IsSecureEventInputEnabled() != 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// PID holding secure input, parsed out of `ioreg`. macOS publishes it as
/// `"kCGSSessionSecureInputPID" = <pid>` on the console session; a zero means
/// nobody holds it. Split from the process call so the parsing is testable —
/// the whole point of this code is to name the culprit, and a silent parse miss
/// would put us back to "a password field somewhere".
fn parse_secure_input_pid(ioreg: &str) -> Option<i32> {
    for line in ioreg.lines() {
        let Some(rest) = line.split("kCGSSessionSecureInputPID").nth(1) else {
            continue;
        };
        let digits: String = rest
            .trim_start_matches(|c: char| !c.is_ascii_digit() && c != '-')
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(pid) = digits.parse::<i32>() {
            if pid > 0 {
                return Some(pid);
            }
        }
    }
    None
}

/// Human-readable name of the app currently holding secure event input, e.g.
/// "Google Chrome". `None` when nothing holds it or the owner can't be named.
pub fn secure_input_holder() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/sbin/ioreg")
            .args(["-l", "-w", "0"])
            .output()
            .ok()?;
        let pid = parse_secure_input_pid(&String::from_utf8_lossy(&out.stdout))?;
        Some(app_name_for_pid(pid).unwrap_or_else(|| format!("pid {}", pid)))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn app_name_for_pid(pid: i32) -> Option<String> {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let app: id = msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        if app == nil {
            return None;
        }
        let name: id = msg_send![app, localizedName];
        if name == nil {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![name, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Bundle id of the app about to receive the keystrokes ("com.apple.Safari"),
/// `None` when it can't be read. Recorded with every dictation because "typed
/// into the wrong window" and "typed nowhere" leave identical traces otherwise.
pub fn target_app() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::{id, nil};
        use objc::{class, msg_send, sel, sel_impl};

        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            if workspace == nil {
                return None;
            }
            let app: id = msg_send![workspace, frontmostApplication];
            if app == nil {
                return None;
            }
            let bundle_id: id = msg_send![app, bundleIdentifier];
            if bundle_id == nil {
                return None;
            }
            let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
            if utf8.is_null() {
                return None;
            }
            Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Message for the gate that is up, if any. Split from the system calls so the
/// wording — which the user reads in the error toast — is pinned by a test.
fn gate_error(trusted: bool, secure_input: bool, holder: Option<&str>) -> Option<String> {
    if !trusted {
        return Some(
            "macOS is blocking keystrokes: Ribbit is not allowed in System Settings → \
             Privacy & Security → Accessibility."
                .into(),
        );
    }
    if secure_input {
        return Some(match holder {
            Some(app) => format!(
                "macOS secure input is on — {} is holding it and blocking keystrokes \
                 system-wide. Close its password field (or quit it), then dictate again.",
                app
            ),
            None => "macOS secure input is on — a password field somewhere is blocking \
                     keystrokes system-wide. Close or leave that field, then dictate again."
                .into(),
        });
    }
    None
}

pub fn insert_text(text: &str) -> Result<(), String> {
    let secure_input = secure_input_active();
    // The holder lookup shells out to ioreg, so it only runs on the blocked path.
    let holder = if secure_input { secure_input_holder() } else { None };
    if let Some(reason) = gate_error(accessibility_trusted(), secure_input, holder.as_deref()) {
        if let Some(app) = &holder {
            debug_log::log(&format!("secure input held by {}", app));
        }
        return Err(reason);
    }
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| format!("text input failed: {}", e))?;
    debug_log::log(&format!("inserted {} chars at cursor", text.chars().count()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_gates_let_the_text_through() {
        assert!(gate_error(true, false, None).is_none());
    }

    #[test]
    fn missing_accessibility_grant_is_named() {
        let msg = gate_error(false, false, None).expect("blocked");
        assert!(msg.contains("Accessibility"), "{}", msg);
    }

    #[test]
    fn secure_input_is_named_even_when_the_grant_is_fine() {
        let msg = gate_error(true, true, None).expect("blocked");
        assert!(msg.contains("secure input"), "{}", msg);
    }

    #[test]
    fn the_app_holding_secure_input_is_named_in_the_error() {
        let msg = gate_error(true, true, Some("Google Chrome")).expect("blocked");
        assert!(msg.contains("Google Chrome"), "{}", msg);
    }

    #[test]
    fn secure_input_pid_is_read_from_ioreg() {
        let sample = r#"
    | |   "CGSSessionScreenIsLocked" = No
    | |   "kCGSSessionSecureInputPID" = 4821
    | |   "kCGSSessionOnConsoleKey" = Yes
"#;
        assert_eq!(parse_secure_input_pid(sample), Some(4821));
    }

    #[test]
    fn a_zero_pid_means_nobody_holds_secure_input() {
        assert_eq!(
            parse_secure_input_pid("    \"kCGSSessionSecureInputPID\" = 0"),
            None
        );
        assert_eq!(parse_secure_input_pid("nothing here"), None);
    }
}

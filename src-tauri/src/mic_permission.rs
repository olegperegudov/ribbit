//! Explicitly request macOS microphone authorization on launch.
//!
//! cpal opens the input stream directly through CoreAudio. When the mic grant is
//! missing but the app never *asked* for it (status "not determined"), macOS
//! hands back a running stream of silence instead of prompting — and the app
//! never appears in System Settings → Privacy → Microphone, which has no manual
//! "+" to add it. That is exactly the dead end after the one-time TCC reset on
//! the ad-hoc → stable-cert switch: recording captures pure silence with no way
//! to re-enable it.
//!
//! `AVCaptureDevice requestAccessForMediaType:` forces the system prompt (and
//! registers the app in the Microphone pane) when the status is undetermined,
//! and is a no-op once granted or denied. So calling it every launch is safe and
//! self-heals any future reset. No-op off macOS.

#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
pub fn request_mic_access() {
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::runtime::Class;
    use objc::{msg_send, sel, sel_impl};

    // Resolve at runtime rather than class!() so a missing framework no-ops
    // instead of panicking on launch.
    let cls = match Class::get("AVCaptureDevice") {
        Some(c) => c,
        None => {
            crate::debug_log::log("mic: AVCaptureDevice unavailable, skipping request");
            return;
        }
    };

    unsafe {
        // AVMediaTypeAudio is the four-char constant string "soun".
        let media_type: id = NSString::alloc(nil).init_str("soun");
        let handler = ConcreteBlock::new(move |granted: bool| {
            crate::debug_log::log(&format!("mic: authorization granted={}", granted));
        })
        .copy();
        let _: () = msg_send![cls, requestAccessForMediaType: media_type completionHandler: handler];
    }
    crate::debug_log::log("mic: requested microphone authorization");
}

#[cfg(not(target_os = "macos"))]
pub fn request_mic_access() {}

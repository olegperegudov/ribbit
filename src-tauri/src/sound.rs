use std::io::Cursor;
use std::sync::{mpsc, atomic::{AtomicU8, Ordering}};

const QUACK_OGG: &[u8] = include_bytes!("../../src/quack.ogg");
const KNOCK_START_OGG: &[u8] = include_bytes!("../../src/knock_start.ogg");
const KNOCK_DONE_OGG: &[u8] = include_bytes!("../../src/knock_done.ogg");

pub enum SoundKind {
    Start, // mic ready
    Stop,  // recording stopped
    Done,  // transcription complete
}

// 0 = frog, 1 = knock
static SOUND_PACK: AtomicU8 = AtomicU8::new(0);

pub fn set_pack(pack: &str) {
    SOUND_PACK.store(if pack == "ping" { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn get_pack() -> &'static str {
    if SOUND_PACK.load(Ordering::Relaxed) == 1 { "ping" } else { "frog" }
}

pub struct SoundPlayer {
    tx: std::sync::Mutex<mpsc::Sender<SoundKind>>,
}

/// Play sound on the given output handle.
/// Returns true on success, false on failure.
fn play_on(handle: &rodio::OutputStreamHandle, ogg_data: &'static [u8], pack_name: &str, label: &str) -> bool {
    let cursor = Cursor::new(ogg_data);
    match rodio::Decoder::new(cursor) {
        Ok(source) => {
            use rodio::Source;
            match handle.play_raw(source.convert_samples()) {
                Ok(()) => {
                    crate::debug_log::log(&format!("sound: played {}-{}", pack_name, label));
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    true
                }
                Err(e) => {
                    crate::debug_log::log(&format!("sound: play failed: {}", e));
                    false
                }
            }
        }
        Err(e) => {
            crate::debug_log::log(&format!("sound: decode error: {}", e));
            false
        }
    }
}

impl SoundPlayer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            crate::debug_log::log("sound: thread started");

            // Keep a cached stream for when we can't open a fresh one (e.g. unfocused).
            // Try fresh stream first each time — this handles device changes and stale streams.
            // Fall back to cached stream if fresh open fails (Windows blocks new audio sessions
            // for unfocused apps).
            let mut cached: Option<(rodio::OutputStream, rodio::OutputStreamHandle)> = None;

            // Initialize cached stream
            match rodio::OutputStream::try_default() {
                Ok((stream, handle)) => {
                    crate::debug_log::log("sound: initial stream opened");
                    cached = Some((stream, handle));
                }
                Err(e) => {
                    crate::debug_log::log(&format!("sound: initial open failed: {}", e));
                }
            };

            for kind in rx {
                let label = match &kind {
                    SoundKind::Start => "start",
                    SoundKind::Stop => "stop",
                    SoundKind::Done => "done",
                };

                let is_knock = SOUND_PACK.load(Ordering::Relaxed) == 1;

                let (ogg_data, pack_name) = if is_knock {
                    let data = match kind {
                        SoundKind::Start => KNOCK_START_OGG,
                        SoundKind::Stop  => KNOCK_START_OGG,
                        SoundKind::Done  => KNOCK_DONE_OGG,
                    };
                    (data, "knock")
                } else {
                    (QUACK_OGG, "frog")
                };

                // Strategy: try fresh stream first, fall back to cached.
                // This ensures we always use the current default audio device,
                // but still work when unfocused (Windows blocks new stream creation).
                let mut played = false;

                // Try opening a fresh stream (picks up device changes)
                if let Ok((new_stream, new_handle)) = rodio::OutputStream::try_default() {
                    if play_on(&new_handle, ogg_data, pack_name, label) {
                        // Fresh stream worked — update cache
                        cached = Some((new_stream, new_handle));
                        played = true;
                    }
                }

                // Fall back to cached stream
                if !played {
                    if let Some((_, ref handle)) = cached {
                        crate::debug_log::log("sound: fresh stream failed, using cached");
                        played = play_on(handle, ogg_data, pack_name, label);
                    }
                }

                // Both failed — try to re-init cache for next time
                if !played {
                    crate::debug_log::log("sound: all playback failed, will retry next time");
                    cached = None;
                }
            }
            crate::debug_log::log("sound: channel closed, thread exiting");
        });

        Self {
            tx: std::sync::Mutex::new(tx),
        }
    }

    pub fn play(&self, kind: SoundKind) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(kind);
        }
    }
}

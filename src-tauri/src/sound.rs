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

impl SoundPlayer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            crate::debug_log::log("sound: thread started");

            // Open output stream once and keep it alive for the thread's lifetime.
            // This avoids Windows audio session issues when the window is not focused.
            let (_stream, handle) = match rodio::OutputStream::try_default() {
                Ok(s) => s,
                Err(e) => {
                    crate::debug_log::log(&format!("sound: output open failed: {}", e));
                    return;
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

                let cursor = Cursor::new(ogg_data);
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        use rodio::Source;
                        match handle.play_raw(source.convert_samples()) {
                            Ok(()) => {
                                crate::debug_log::log(&format!("sound: played {}-{}", pack_name, label));
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            Err(e) => crate::debug_log::log(&format!("sound: play failed: {}", e)),
                        }
                    }
                    Err(e) => crate::debug_log::log(&format!("sound: decode error: {}", e)),
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

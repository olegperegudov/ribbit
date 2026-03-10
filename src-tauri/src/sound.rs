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

            for kind in rx {
                let label = match &kind {
                    SoundKind::Start => "start",
                    SoundKind::Stop => "stop",
                    SoundKind::Done => "done",
                };

                // Re-open output stream each time to follow the current default device
                let (_stream, handle) = match rodio::OutputStream::try_default() {
                    Ok(s) => s,
                    Err(e) => {
                        crate::debug_log::log(&format!("sound: output open failed: {}", e));
                        continue;
                    }
                };

                let is_knock = SOUND_PACK.load(Ordering::Relaxed) == 1;

                // Pick the right ogg bytes and volume
                let (ogg_data, volume, pack_name) = if is_knock {
                    let data = match kind {
                        SoundKind::Start => KNOCK_START_OGG,
                        SoundKind::Stop  => KNOCK_START_OGG, // reuse start for stop
                        SoundKind::Done  => KNOCK_DONE_OGG,
                    };
                    let vol = match kind {
                        SoundKind::Start => 1.0_f32,
                        SoundKind::Stop  => 0.7,
                        SoundKind::Done  => 1.0,
                    };
                    (data, vol, "knock")
                } else {
                    let vol = match kind {
                        SoundKind::Start => 0.8_f32,
                        SoundKind::Stop  => 0.6,
                        SoundKind::Done  => 0.4,
                    };
                    (QUACK_OGG, vol, "frog")
                };

                let speed = if !is_knock {
                    match kind {
                        SoundKind::Start => 1.15_f32,
                        SoundKind::Stop  => 0.85,
                        SoundKind::Done  => 1.3,
                    }
                } else {
                    1.0
                };

                let cursor = Cursor::new(ogg_data);
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        use rodio::Source;
                        match handle.play_raw(
                            source.speed(speed).amplify(volume).convert_samples(),
                        ) {
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

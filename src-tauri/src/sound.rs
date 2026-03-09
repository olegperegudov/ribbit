use std::io::Cursor;
use std::sync::mpsc;

const QUACK_OGG: &[u8] = include_bytes!("../../src/quack.ogg");

pub enum SoundKind {
    Start, // mic ready — higher pitch
    Stop,  // recording stopped — lower pitch
    Done,  // transcription complete — highest pitch, quiet
}

pub struct SoundPlayer {
    tx: std::sync::Mutex<mpsc::Sender<SoundKind>>,
}

impl SoundPlayer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            crate::debug_log::log("sound: initializing output stream...");
            let (_stream, handle) = match rodio::OutputStream::try_default() {
                Ok(s) => {
                    crate::debug_log::log("sound: output stream OK");
                    s
                }
                Err(e) => {
                    crate::debug_log::log(&format!("sound: FAILED to init output: {}", e));
                    return;
                }
            };

            // Verify we can decode the embedded ogg
            match rodio::Decoder::new(Cursor::new(QUACK_OGG)) {
                Ok(_) => crate::debug_log::log(&format!(
                    "sound: quack.ogg decoded OK ({} bytes)",
                    QUACK_OGG.len()
                )),
                Err(e) => {
                    crate::debug_log::log(&format!("sound: quack.ogg decode FAILED: {}", e));
                    return;
                }
            }

            for kind in rx {
                let label = match &kind {
                    SoundKind::Start => "start",
                    SoundKind::Stop => "stop",
                    SoundKind::Done => "done",
                };
                let (speed, volume) = match kind {
                    SoundKind::Start => (1.15_f32, 0.8_f32),
                    SoundKind::Stop => (0.85, 0.6),
                    SoundKind::Done => (1.3, 0.4),
                };

                let cursor = Cursor::new(QUACK_OGG);
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        use rodio::Source;
                        match handle.play_raw(
                            source
                                .speed(speed)
                                .amplify(volume)
                                .convert_samples(),
                        ) {
                            Ok(()) => crate::debug_log::log(&format!("sound: played {}", label)),
                            Err(e) => crate::debug_log::log(&format!(
                                "sound: play_raw failed for {}: {}",
                                label, e
                            )),
                        }
                    }
                    Err(e) => {
                        crate::debug_log::log(&format!("sound: decode error for {}: {}", label, e));
                    }
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

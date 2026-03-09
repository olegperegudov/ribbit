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
            let (_stream, handle) = match rodio::OutputStream::try_default() {
                Ok(s) => s,
                Err(e) => {
                    crate::debug_log::log(&format!("Sound init failed: {}", e));
                    return;
                }
            };

            for kind in rx {
                let (speed, volume) = match kind {
                    SoundKind::Start => (1.15_f32, 0.8_f32),
                    SoundKind::Stop => (0.85, 0.6),
                    SoundKind::Done => (1.3, 0.4),
                };

                let cursor = Cursor::new(QUACK_OGG);
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        use rodio::Source;
                        let _ = handle.play_raw(
                            source
                                .speed(speed)
                                .amplify(volume)
                                .convert_samples(),
                        );
                    }
                    Err(e) => {
                        crate::debug_log::log(&format!("Sound decode error: {}", e));
                    }
                }
            }
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

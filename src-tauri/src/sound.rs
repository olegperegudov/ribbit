use std::io::Cursor;
use std::sync::{mpsc, atomic::{AtomicU8, Ordering}};

const QUACK_OGG: &[u8] = include_bytes!("../../src/quack.ogg");

pub enum SoundKind {
    Start, // mic ready
    Stop,  // recording stopped
    Done,  // transcription complete
}

// 0 = frog, 1 = ping
static SOUND_PACK: AtomicU8 = AtomicU8::new(0);

pub fn set_pack(pack: &str) {
    SOUND_PACK.store(if pack == "ping" { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn get_pack() -> &'static str {
    if SOUND_PACK.load(Ordering::Relaxed) == 1 { "ping" } else { "frog" }
}

fn generate_ping(freq: f32, duration_ms: u32, volume: f32) -> Vec<f32> {
    let sample_rate = 48000u32;
    let num_samples = sample_rate * duration_ms / 1000;
    let mut samples = Vec::with_capacity(num_samples as usize);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 6.0).exp();
        let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * envelope * volume;
        samples.push(sample);
    }
    samples
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

                let is_ping = SOUND_PACK.load(Ordering::Relaxed) == 1;

                if is_ping {
                    use rodio::Source;
                    let (freq, dur, vol) = match kind {
                        SoundKind::Start => (280.0, 200, 0.6),
                        SoundKind::Stop  => (180.0, 250, 0.5),
                        SoundKind::Done  => (350.0, 180, 0.4),
                    };
                    let samples = generate_ping(freq, dur, vol);
                    let source = rodio::buffer::SamplesBuffer::new(1, 48000, samples);
                    match handle.play_raw(source.convert_samples()) {
                        Ok(()) => {
                            crate::debug_log::log(&format!("sound: played ping-{}", label));
                            std::thread::sleep(std::time::Duration::from_millis(dur as u64 + 50));
                        }
                        Err(e) => crate::debug_log::log(&format!("sound: ping play failed: {}", e)),
                    }
                } else {
                    let cursor = Cursor::new(QUACK_OGG);
                    match rodio::Decoder::new(cursor) {
                        Ok(source) => {
                            use rodio::Source;
                            let (speed, volume) = match kind {
                                SoundKind::Start => (1.15_f32, 0.8_f32),
                                SoundKind::Stop  => (0.85, 0.6),
                                SoundKind::Done  => (1.3, 0.4),
                            };
                            match handle.play_raw(
                                source.speed(speed).amplify(volume).convert_samples(),
                            ) {
                                Ok(()) => {
                                    crate::debug_log::log(&format!("sound: played frog-{}", label));
                                    // Keep stream alive while sound plays
                                    std::thread::sleep(std::time::Duration::from_millis(500));
                                }
                                Err(e) => crate::debug_log::log(&format!("sound: frog play failed: {}", e)),
                            }
                        }
                        Err(e) => crate::debug_log::log(&format!("sound: decode error: {}", e)),
                    }
                }
                // _stream drops here, closing the device — ready for next sound on fresh device
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

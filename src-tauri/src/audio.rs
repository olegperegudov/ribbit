use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

use crate::RecordingState;
use crate::debug_log;

pub fn record_audio(state: Arc<Mutex<RecordingState>>) {
    let host = cpal::default_host();

    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            debug_log::log("ERROR: no input device available");
            return;
        }
    };

    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    debug_log::log(&format!("audio device: {}", device_name));

    // Request 16kHz mono — ideal for Whisper
    let desired_config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(16000),
        buffer_size: cpal::BufferSize::Default,
    };

    let state_clone = Arc::clone(&state);

    let stream = match device.build_input_stream(
        &desired_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut s = state_clone.lock().unwrap();
            if s.is_recording {
                s.audio_data.extend_from_slice(data);
            }
        },
        |err| debug_log::log(&format!("audio stream error: {}", err)),
        None,
    ) {
        Ok(s) => {
            let mut st = state.lock().unwrap();
            st.sample_rate = 16000;
            debug_log::log("audio config: 16kHz mono");
            drop(st);
            s
        }
        Err(e) => {
            debug_log::log(&format!("16kHz not supported ({}), using device default", e));
            let default_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    debug_log::log(&format!("ERROR: no input config: {}", e));
                    return;
                }
            };

            let sample_rate = default_config.sample_rate().0;
            let channels = default_config.channels();
            debug_log::log(&format!("audio config: {}Hz {}ch", sample_rate, channels));

            {
                let mut s = state.lock().unwrap();
                s.sample_rate = sample_rate;
            }

            let state_clone2 = Arc::clone(&state);
            match device.build_input_stream(
                &default_config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut s = state_clone2.lock().unwrap();
                    if s.is_recording {
                        if channels > 1 {
                            // Downmix to mono by averaging channels
                            for chunk in data.chunks(channels as usize) {
                                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                                s.audio_data.push(mono);
                            }
                        } else {
                            s.audio_data.extend_from_slice(data);
                        }
                    }
                },
                |err| debug_log::log(&format!("audio stream error: {}", err)),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    debug_log::log(&format!("ERROR: failed to create stream: {}", e));
                    return;
                }
            }
        }
    };

    if let Err(e) = stream.play() {
        debug_log::log(&format!("ERROR: failed to start stream: {}", e));
        return;
    }

    // Keep recording until is_recording becomes false
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let s = state.lock().unwrap();
        if !s.is_recording {
            break;
        }
    }
}

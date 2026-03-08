use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

use crate::RecordingState;

pub fn record_audio(state: Arc<Mutex<RecordingState>>) {
    let host = cpal::default_host();

    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            eprintln!("No input device available");
            return;
        }
    };

    // Request 16kHz mono — ideal for Whisper
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(16000),
        buffer_size: cpal::BufferSize::Default,
    };

    // Update the sample rate in state
    {
        let mut s = state.lock().unwrap();
        s.sample_rate = 16000;
    }

    let state_clone = Arc::clone(&state);
    let err_fn = |err| eprintln!("Audio stream error: {}", err);

    let stream = match device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut s = state_clone.lock().unwrap();
            if s.is_recording {
                s.audio_data.extend_from_slice(data);
            }
        },
        err_fn,
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            // Fallback: try default config if 16kHz not supported
            eprintln!("16kHz not supported ({}), trying default config", e);
            let default_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("No input config available: {}", e);
                    return;
                }
            };

            let sample_rate = default_config.sample_rate().0;
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
                        s.audio_data.extend_from_slice(data);
                    }
                },
                err_fn,
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to create audio stream: {}", e);
                    return;
                }
            }
        }
    };

    if let Err(e) = stream.play() {
        eprintln!("Failed to start audio stream: {}", e);
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

    // Stream is dropped here, stopping the recording
}

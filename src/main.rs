use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

fn main() {
    let host = cpal::default_host();
    println!("Audio host: {:?}", host.id());

    // Get default input device
    let device = host
        .default_input_device()
        .expect("No default input device found");
    println!("Using input device: {:?}", device.description());

    // Get the first supported config
    let config = device
        .supported_input_configs()
        .expect("Failed to get supported configs")
        .next()
        .expect("No supported config found")
        .with_max_sample_rate();

    let sample_format = config.sample_format();
    let config: cpal::StreamConfig = config.into();

    println!(
        "Recording with: {} channels, {} Hz, {:?}",
        config.channels, config.sample_rate, sample_format
    );

    // Set up WAV writer - always use 16-bit signed int for compatibility
    // We'll convert U8 samples to I16 during recording
    let spec = WavSpec {
        channels: config.channels as u16,
        sample_rate: config.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let writer = WavWriter::create("output.wav", spec).expect("Failed to create WAV file");
    let writer = Arc::new(Mutex::new(Some(writer)));
    let writer_clone = Arc::clone(&writer);

    // Build input stream
    let err_fn = |err| eprintln!("Stream error: {}", err);

    let stream = match sample_format {
        SampleFormat::U8 => device.build_input_stream(
            &config,
            move |data: &[u8], _: &cpal::InputCallbackInfo| {
                if let Ok(mut guard) = writer_clone.lock() {
                    if let Some(ref mut w) = *guard {
                        for &sample in data {
                            // Convert U8 (0-255, center 128) to I16 (-32768 to 32767)
                            let sample_i16 = ((sample as i16) - 128) * 256;
                            let _ = w.write_sample(sample_i16);
                        }
                    }
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => {
            let writer_clone = Arc::clone(&writer);
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut guard) = writer_clone.lock() {
                        if let Some(ref mut w) = *guard {
                            for &sample in data {
                                let _ = w.write_sample(sample);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::F32 => {
            let writer_clone = Arc::clone(&writer);
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut guard) = writer_clone.lock() {
                        if let Some(ref mut w) = *guard {
                            for &sample in data {
                                // Convert F32 (-1.0 to 1.0) to I16
                                let sample_i16 = (sample * 32767.0) as i16;
                                let _ = w.write_sample(sample_i16);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I32 => {
            let writer_clone = Arc::clone(&writer);
            device.build_input_stream(
                &config,
                move |data: &[i32], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut guard) = writer_clone.lock() {
                        if let Some(ref mut w) = *guard {
                            for &sample in data {
                                // Convert I32 to I16 by shifting
                                let sample_i16 = (sample >> 16) as i16;
                                let _ = w.write_sample(sample_i16);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        _ => panic!("Unsupported sample format: {:?}", sample_format),
    }
    .expect("Failed to build input stream");

    // Start recording
    stream.play().expect("Failed to start stream");
    println!("\nRecording... Press Enter to stop.");

    // Wait for user input
    let _ = io::stdout().flush();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    // Stop and finalize
    drop(stream);

    // Finalize the WAV file
    if let Ok(mut guard) = writer.lock() {
        if let Some(w) = guard.take() {
            w.finalize().expect("Failed to finalize WAV file");
        }
    }

    println!("Recording saved to output.wav");
}

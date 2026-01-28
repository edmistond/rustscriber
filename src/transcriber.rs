use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use parakeet_rs::Nemotron;
use rubato::{FftFixedIn, Resampler};
use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

const ASR_SAMPLE_RATE: usize = 16000;
/// 560ms at 16kHz — required chunk size for Nemotron
const NEMOTRON_CHUNK_SIZE: usize = 8960;

pub struct Transcriber {
    stream: Option<Stream>,
    processing_thread: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl Transcriber {
    pub fn new(
        model_path: &Path,
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let input_rate = config.sample_rate as usize;
        let channels = config.channels as usize;
        let needs_resample = input_rate != ASR_SAMPLE_RATE;

        println!(
            "ASR pipeline: {}Hz {}ch → 16kHz mono (resample: {})",
            input_rate, channels, needs_resample
        );

        println!("Loading Nemotron model from {}...", model_path.display());
        let model = Nemotron::from_pretrained(model_path, None)?;
        println!("Model loaded.");

        let buffer: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let buffer_for_callback = Arc::clone(&buffer);

        let stream = Self::build_stream(
            device,
            config,
            sample_format,
            channels,
            buffer_for_callback,
        )?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_thread = Arc::clone(&stop_flag);

        let processing_thread = thread::spawn(move || {
            Self::processing_loop(model, buffer, stop_flag_thread, input_rate, needs_resample);
        });

        Ok(Self {
            stream: Some(stream),
            processing_thread: Some(processing_thread),
            stop_flag,
        })
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref stream) = self.stream {
            stream.play()?;
        }
        Ok(())
    }

    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // Drop the stream to stop audio capture
        self.stream.take();
        if let Some(handle) = self.processing_thread.take() {
            let _ = handle.join();
        }
    }

    fn build_stream(
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
        channels: usize,
        buffer: Arc<Mutex<VecDeque<f32>>>,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
        let err_fn = |err| eprintln!("Stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => {
                let buf = Arc::clone(&buffer);
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        push_mono(data, channels, &buf);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let buf = Arc::clone(&buffer);
                device.build_input_stream(
                    config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let floats: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 32768.0).collect();
                        push_mono(&floats, channels, &buf);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U8 => {
                let buf = Arc::clone(&buffer);
                device.build_input_stream(
                    config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        let floats: Vec<f32> =
                            data.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect();
                        push_mono(&floats, channels, &buf);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I32 => {
                let buf = Arc::clone(&buffer);
                device.build_input_stream(
                    config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        let floats: Vec<f32> =
                            data.iter().map(|&s| s as f32 / 2147483648.0).collect();
                        push_mono(&floats, channels, &buf);
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(format!("Unsupported sample format: {:?}", sample_format).into()),
        };

        Ok(stream)
    }

    fn processing_loop(
        mut model: Nemotron,
        buffer: Arc<Mutex<VecDeque<f32>>>,
        stop_flag: Arc<AtomicBool>,
        input_rate: usize,
        needs_resample: bool,
    ) {
        // Set up resampler if needed
        let mut resampler: Option<FftFixedIn<f32>> = if needs_resample {
            // Use a chunk size that divides nicely into our workflow.
            // 1024 input frames is a reasonable FFT size.
            let r = FftFixedIn::<f32>::new(input_rate, ASR_SAMPLE_RATE, 1024, 1, 1);
            match r {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("Failed to create resampler: {}", e);
                    return;
                }
            }
        } else {
            None
        };

        // Buffer to accumulate 16kHz samples until we have a full Nemotron chunk
        let mut asr_buffer: Vec<f32> = Vec::with_capacity(NEMOTRON_CHUNK_SIZE * 2);

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Drain available samples from the shared buffer
            let drained: Vec<f32> = {
                let mut guard = buffer.lock().unwrap();
                guard.drain(..).collect()
            };

            if drained.is_empty() {
                thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }

            // Resample or pass through
            let samples_16k = if let Some(ref mut resampler) = resampler {
                // rubato expects non-interleaved channel data: &[&[f32]]
                // We already have mono, so it's one channel.
                let chunk_size = resampler.input_frames_next();
                let mut resampled = Vec::new();

                // Feed drained samples through the resampler in chunk_size batches
                let mut offset = 0;
                while offset + chunk_size <= drained.len() {
                    let input_chunk = &drained[offset..offset + chunk_size];
                    match resampler.process(&[input_chunk], None) {
                        Ok(output) => {
                            if !output.is_empty() {
                                resampled.extend_from_slice(&output[0]);
                            }
                        }
                        Err(e) => {
                            eprintln!("Resampler error: {}", e);
                        }
                    }
                    offset += chunk_size;
                }

                // Put leftover samples back into the shared buffer so they're
                // picked up next iteration
                if offset < drained.len() {
                    let mut guard = buffer.lock().unwrap();
                    for &s in &drained[offset..] {
                        guard.push_front(s);
                    }
                }

                resampled
            } else {
                drained
            };

            asr_buffer.extend_from_slice(&samples_16k);

            // Feed full chunks to Nemotron
            while asr_buffer.len() >= NEMOTRON_CHUNK_SIZE {
                let chunk: Vec<f32> = asr_buffer.drain(..NEMOTRON_CHUNK_SIZE).collect();
                match model.transcribe_chunk(&chunk) {
                    Ok(text) => {
                        if !text.is_empty() {
                            print!("{}", text);
                            let _ = std::io::stdout().flush();
                        }
                    }
                    Err(e) => {
                        eprintln!("\nASR error: {}", e);
                    }
                }
            }
        }
    }
}

/// Downmix interleaved multi-channel audio to mono and push into the shared buffer.
fn push_mono(data: &[f32], channels: usize, buffer: &Arc<Mutex<VecDeque<f32>>>) {
    let mono: Vec<f32> = if channels == 1 {
        data.to_vec()
    } else {
        data.chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    if let Ok(mut guard) = buffer.lock() {
        guard.extend(mono.iter());
    }
}

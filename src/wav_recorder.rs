use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

type WavWriterHandle = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

pub struct WavRecorder {
    writer: WavWriterHandle,
    stream: Option<Stream>,
}

impl WavRecorder {
    pub fn new(
        filename: &str,
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let spec = WavSpec {
            channels: config.channels as u16,
            sample_rate: config.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = WavWriter::create(filename, spec)?;
        let writer: WavWriterHandle = Arc::new(Mutex::new(Some(writer)));

        let stream = Self::build_stream(device, config, sample_format, Arc::clone(&writer))?;

        Ok(Self {
            writer,
            stream: Some(stream),
        })
    }

    fn build_stream(
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
        writer: WavWriterHandle,
    ) -> Result<Stream, Box<dyn std::error::Error>> {
        let err_fn = |err| eprintln!("Stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::U8 => {
                let writer_clone = Arc::clone(&writer);
                device.build_input_stream(
                    config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut guard) = writer_clone.lock() {
                            if let Some(ref mut w) = *guard {
                                for &sample in data {
                                    let sample_i16 = ((sample as i16) - 128) * 256;
                                    let _ = w.write_sample(sample_i16);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let writer_clone = Arc::clone(&writer);
                device.build_input_stream(
                    config,
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
                )?
            }
            SampleFormat::F32 => {
                let writer_clone = Arc::clone(&writer);
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut guard) = writer_clone.lock() {
                            if let Some(ref mut w) = *guard {
                                for &sample in data {
                                    let sample_i16 = (sample * 32767.0) as i16;
                                    let _ = w.write_sample(sample_i16);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I32 => {
                let writer_clone = Arc::clone(&writer);
                device.build_input_stream(
                    config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut guard) = writer_clone.lock() {
                            if let Some(ref mut w) = *guard {
                                for &sample in data {
                                    let sample_i16 = (sample >> 16) as i16;
                                    let _ = w.write_sample(sample_i16);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(format!("Unsupported sample format: {:?}", sample_format).into()),
        };

        Ok(stream)
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref stream) = self.stream {
            stream.play()?;
        }
        Ok(())
    }

    pub fn stop_and_finalize(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Drop the stream first to stop recording
        self.stream.take();

        // Finalize the WAV file
        if let Ok(mut guard) = self.writer.lock() {
            if let Some(w) = guard.take() {
                w.finalize()?;
            }
        }

        Ok(())
    }
}

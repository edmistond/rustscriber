mod audio_config;
mod device_enumerator;
mod transcriber;
mod wav_recorder;

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait};
use std::io::{self, Write};
use std::path::Path;
use transcriber::Transcriber;
use wav_recorder::WavRecorder;

const MODEL_PATH: &str = "/Users/edmistond/Downloads/prs-nemotron";

#[derive(Parser)]
#[command(name = "rustscriber")]
#[command(about = "Audio transcription tool", long_about = None)]
struct Args {
    /// List all available audio input and output devices
    #[arg(long)]
    enumerate: bool,

    /// Record audio to a WAV file
    #[arg(long, value_name = "FILE")]
    record: Option<String>,
}

fn main() {
    let args = Args::parse();

    if args.enumerate {
        device_enumerator::enumerate_devices();
        return;
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("No default input device found");
    println!("Using input device: {:?}", device.description());

    let (supported_config, sample_format) = audio_config::select_input_config(&device)
        .expect("Failed to select input config");
    let config: cpal::StreamConfig = supported_config.into();

    println!(
        "Audio config: {} channels, {} Hz, {:?}",
        config.channels, config.sample_rate, sample_format
    );

    if let Some(filename) = args.record {
        let recorder = WavRecorder::new(&filename, &device, &config, sample_format)
            .expect("Failed to create WAV recorder");

        recorder.start().expect("Failed to start recording");
        println!("\nRecording to {}... Press Enter to stop.", filename);

        let _ = io::stdout().flush();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        recorder
            .stop_and_finalize()
            .expect("Failed to finalize recording");
        println!("Recording saved to {}", filename);
    } else {
        // Default: live ASR
        let t = Transcriber::new(Path::new(MODEL_PATH), &device, &config, sample_format)
            .expect("Failed to create transcriber");

        t.start().expect("Failed to start transcription");
        println!("\nListening... Press Enter to stop.\n");

        let _ = io::stdout().flush();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        t.stop();
        println!("\nTranscription stopped.");
    }
}

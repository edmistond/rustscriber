mod device_enumerator;
mod wav_recorder;

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait};
use std::io::{self, Write};
use wav_recorder::WavRecorder;

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

    if let Some(filename) = args.record {
        let host = cpal::default_host();
        println!("Audio host: {:?}", host.id());

        let device = host
            .default_input_device()
            .expect("No default input device found");
        println!("Using input device: {:?}", device.description());

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
        println!(
            "No mode specified. Use --enumerate to list devices or --record <FILE> to record."
        );
        println!("ASR support coming soon.");
    }
}

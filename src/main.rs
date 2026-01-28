use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let host = cpal::default_host();
    println!("Audio host: {:?}\n", host.id());

    if let Some(device) = host.default_input_device() {
        println!("Default input device: {:?}", device.description());
    } else {
        println!("No default input device found");
    }

    if let Some(device) = host.default_output_device() {
        println!("Default output device: {:?}", device.description());
    }

    println!("\n--- Input Devices ---");
    match host.input_devices() {
        Ok(devices) => {
            for device in devices {
                println!("  {:?}", device.description());
                if let Ok(configs) = device.supported_input_configs() {
                    for cfg in configs {
                        println!(
                            "    channels={}, sample_rate={}..{}, format={:?}",
                            cfg.channels(),
                            cfg.min_sample_rate(),
                            cfg.max_sample_rate(),
                            cfg.sample_format(),
                        );
                    }
                }
            }
        }
        Err(e) => eprintln!("  Error listing input devices: {e}"),
    }

    println!("\n--- Output Devices ---");
    match host.output_devices() {
        Ok(devices) => {
            for device in devices {
                println!("  {:?}", device.description());
                if let Ok(configs) = device.supported_output_configs() {
                    for cfg in configs {
                        println!(
                            "    channels={}, sample_rate={}..{}, format={:?}",
                            cfg.channels(),
                            cfg.min_sample_rate(),
                            cfg.max_sample_rate(),
                            cfg.sample_format(),
                        );
                    }
                }
            }
        }
        Err(e) => eprintln!("  Error listing output devices: {e}"),
    }
}

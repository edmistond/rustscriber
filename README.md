# rustscriber

A real-time audio transcription tool built in Rust using NVIDIA's Parakeet speech recognition models via ONNX.

> **Note** parakeet-rs supports speaker diarization. However, that is not currently implemented in rustscriber. I plan to iterate extensively on this application in the near future (including adding a GUI) so it is extremely in flux.

## Dependencies

- [parakeet-rs](https://github.com/altunenes/parakeet-rs) - NVIDIA Parakeet ASR & speaker diarization via ONNX
- [cpal](https://crates.io/crates/cpal) - Cross-platform audio capture
- [rubato](https://crates.io/crates/rubato) - Sample rate conversion (resampling captured audio to 16kHz for the ASR model)
- [hound](https://crates.io/crates/hound) - WAV file recording
- [clap](https://crates.io/crates/clap) - Command-line argument parsing

## Getting Models

rustscriber currently uses the **Nemotron** streaming model. Download the model files from HuggingFace:

https://huggingface.co/altunenes/parakeet-rs/tree/main/nemotron-speech-streaming-en-0.6b

Required files (place all in the same directory):
- `encoder.onnx`
- `encoder.onnx.data`
- `decoder_joint.onnx`
- `tokenizer.model`

Other models supported by parakeet-rs (CTC, TDT, EOU, Sortformer) can be found on the [parakeet-rs HuggingFace page](https://huggingface.co/altunenes/parakeet-rs).

> **Note:** The model directory path is currently hardcoded in `main.rs`. This will be addressed soon.

## Building

```sh
cargo build --release
```

### Hardware acceleration features

Optional features can be enabled for GPU-accelerated inference:

```sh
# macOS - CoreML
cargo build --release --features coreml

# Windows - DirectML
cargo build --release --features directml

# AMD GPUs - MIGraphX
cargo build --release --features migraphx
```

## Usage

```sh
# List available audio devices
rustscriber --enumerate

# Transcribe from default input device
rustscriber

# Transcribe from a specific input device (use the ID from --enumerate)
rustscriber --input <DEVICE_ID>

# Record audio to a WAV file instead of transcribing
rustscriber --record output.wav

# Record from a specific device
rustscriber --input <DEVICE_ID> --record output.wav
```

# audio-waveform

[![Crates.io](https://img.shields.io/crates/v/audio-waveform.svg)](https://crates.io/crates/audio-waveform)
[![Documentation](https://docs.rs/audio-waveform/badge.svg)](https://docs.rs/audio-waveform)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

A utility library to generate visual waveform data from audio files.

It decodes audio files using `symphonia` and produces a downsampled amplitude vector for rendering audio waveforms, along with the track's duration. Downsampling is configurable via `WaveformOptions`: choose the number of points, a peak or RMS summary per point, how channels are combined, and whether to normalize.

Supports decoding from a file path, any in-memory reader, or raw pre-decoded samples. Multi-channel tracks (stereo, 5.1 surround) can be decoded into one waveform per channel in a single pass. You may also exclude `symphonia` and use your own decoder.

> **Note**: To render the amplitude vectors this library produces into customized SVGs or raster images (PNG, JPEG, WebP, AVIF, BMP), see the crate [`audio-waveform-render`](https://crates.io/crates/audio-waveform-render).

## Usage

Add `audio-waveform` to your `Cargo.toml`:

```toml
[dependencies]
audio-waveform = "1.1.0"
```

Or from git:

```toml
[dependencies]
audio-waveform = { git = "https://github.com/s1nn3rv2/audio-waveform.git" }
```

Or if you have your own decoder/samples and want to exclude `symphonia` and all its dependencies, disable the default features:

```toml
[dependencies]
audio-waveform = { version = "1.1.0", default-features = false }
```

### Examples

#### Basic File Decoding with Custom Target Resolution

```rust
use audio_waveform::WaveformOptions;
use std::path::Path;

fn main() {
    let audio_path = Path::new("path/to/song.mp3");
    let options = WaveformOptions::new(250);

    match audio_waveform::generate(&audio_path, &options) {
        Ok((waveform, duration)) => {
            println!("Decoded song with duration: {:.2}s", duration);
            println!("First 5 waveform points: {:?}", &waveform[..5]);
        }
        Err(e) => {
            eprintln!("Error generating waveform: {}", e);
        }
    }
}
```

#### Multi-Channel Decoding (Stereo & Surround)

`generate_channels` decodes every audio channel into its own waveform vector in a single pass, returning them in channel order (for example `[Left, Right]` for stereo or `[FL, FR, FC, LFE, RL, RR]` for 5.1 surround).

```rust,no_run
use audio_waveform::{generate_channels, WaveformOptions};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (channels, duration) =
        generate_channels(Path::new("surround.flac"), &WaveformOptions::new(200))?;

    println!("Decoded {} channels, duration {:.2}s", channels.len(), duration);
    for (i, waveform) in channels.iter().enumerate() {
        println!("channel {i}: {} points", waveform.len());
    }

    Ok(())
}
```

#### Decoding from Memory

```rust,no_run
use std::io::Cursor;

fn main() {
    // read audio bytes (for example from a network request or resource file)
    let audio_bytes: Vec<u8> = std::fs::read("song.mp3").unwrap();
    let source = Box::new(Cursor::new(audio_bytes));

    // provide a format extension hint so symphonia knows which decoder to probe
    let extension_hint = Some("mp3");
    let options = audio_waveform::WaveformOptions::new(100);

    match audio_waveform::generate_from_source(source, extension_hint, &options) {
        Ok((waveform, duration)) => {
            println!("Successfully decoded from memory! Duration: {:.2}s", duration);
        }
        Err(e) => {
            eprintln!("Failed to decode from memory: {}", e);
        }
    }
}
```

#### Downsampling Raw Samples

```rust
use audio_waveform::{Measure, WaveformOptions};

fn main() {
    // already decoded mono floating point samples
    let my_samples = vec![0.1, 0.2, 0.4, 0.8, 0.4, 0.2, 0.1];
    let options = WaveformOptions::new(3).measure(Measure::Peak);

    let waveform = audio_waveform::generate_from_samples(&my_samples, &options);
    println!("Downsampled/normalized waveform: {:?}", waveform);
}
```

### Options

`WaveformOptions` controls downsampling:

- `WaveformOptions::new(target_len)`: sets the number of output points. Defaults to an RMS summary with normalization.
- `.measure(Measure::Peak)` / `.measure(Measure::Rms)`: choose a peak envelope or an RMS (loudness) summary per point.
- `.channels(ChannelMode::Mix)` / `.channels(ChannelMode::Single(0))`: average all channels together, or use a single channel by index (out-of-range indices fall back to the last channel). Only affects the single-waveform decoding functions; `generate_channels` always keeps channels separate, and `generate_from_samples` already takes mono input.
- `.normalize(false)`: keep raw amplitudes instead of scaling the largest point to `1.0` (useful for comparing loudness across files).

## How Fast is It?

Fast: downsampling pre-decoded samples runs at billions+ samples per second and decoding of tracks stays well over 900x realtime.

### Downsampling Benchmark (`generate_from_samples`, 500 points)

Time to reduce pre-decoded mono samples down to a 500-point waveform.

| Sample Count | Approx. Audio | Peak | RMS |
| :--- | :--- | :--- | :--- |
| **100,000** | ~2.2 s | **31.6 µs** | **35.2 µs** |
| **1,000,000** | ~22 s | **353.9 µs** | **474.3 µs** |
| **10,000,000** | ~3.7 min | **4.03 ms** | **5.27 ms** |

Sustained throughput sits around **2-3 billion samples/sec**.

### Full Decode + Downsample Benchmark (`generate_from_source`, 500 points)

End-to-end decode of a real, full-length track (read into memory once, so disk IO is excluded) plus downsampling to 500 points. Both tracks are stereo at 44.1 kHz; "samples" counts decoded sample frames per channel.

| Format | Track Duration | Samples | File Size | Decode Time | Speed |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **FLAC** (stereo) | 3:08 (188 s) | ~8.3 M | ~28 MB | **142 ms** | **~1,320x realtime** |
| **MP3** (stereo) | 4:27 (267 s) | ~11.8 M | ~11 MB | **271 ms** | **~990x realtime** |

The decode benchmark runs against real audio files rather than synthetic data. Drop your own files into `benches/fixtures/` then run `cargo bench` - it benchmarks every file found. Throughput varies by codec (FLAC and MP3 do more decoding than uncompressed WAV).

*All benchmarks done on CachyOS on Intel i5-12600k*

## Feature Gates

By default, the crate enables the `symphonia` decoder dependency with the `all` feature.

To customize exactly which audio formats/codecs are supported, disable default features, enable the `symphonia` feature in this crate, and define your own dependency on `symphonia` in your `Cargo.toml` with the desired features:

```toml
[dependencies]
audio-waveform = { version = "1.1.0", default-features = false, features = ["symphonia"] }

# configure Symphonia features directly in your project
symphonia = { version = "0.6", features = ["mp3", "wav"] }
```

Available features:
- `symphonia`: Enables the `symphonia` dependency and decoding APIs (`generate`, `generate_from_source`, `generate_channels`, and `generate_channels_from_source`).
- `all`: Enables the `all` feature on Symphonia.

## License

Licensed under the MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>).
</content>
</invoke>

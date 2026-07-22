# audio-waveform

A utility library to generate visual waveform data from audio files.

It decodes audio files using `symphonia` and produces a downsampled amplitude vector for rendering audio waveforms, along with the track's duration. Downsampling is configurable via `WaveformOptions`: choose the number of points, a peak or RMS summary per point, how channels are combined, and whether to normalize. You may also exclude symphonia and use your own decoder.

## Usage

Add `audio-waveform` to your `Cargo.toml`:

```toml
[dependencies]
audio-waveform = "1.0.0"
```

Or from git:

```toml
[dependencies]
audio-waveform = { git = "https://github.com/s1nn3rv2/audio-waveform.git" }
```

Or if you have your own decoder/samples and want to exclude `symphonia` and all its dependencies, disable the default features:

```toml
[dependencies]
audio-waveform = { version = "1.0.0", default-features = false }
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
- `.channels(ChannelMode::Mix)` / `.channels(ChannelMode::Single(0))`: average all channels together, or use a single channel by index (out-of-range indices fall back to the last channel). Only affects the decoding functions; `generate_from_samples` already takes mono input.
- `.normalize(false)`: keep raw amplitudes instead of scaling the largest point to `1.0` (useful for comparing loudness across files).

## Feature Gates

By default, the crate enables the `symphonia` decoder dependency with the `all` feature. 

To customize exactly which audio formats/codecs are supported, disable default features, enable the `symphonia` feature in this crate, and define your own dependency on `symphonia` in your `Cargo.toml` with the desired features:

```toml
[dependencies]
audio-waveform = { version = "1.0.0", default-features = false, features = ["symphonia"] }

# configure Symphonia features directly in your project
symphonia = { version = "0.6", features = ["mp3", "wav"] }
```

Available features:
- `symphonia`: Enables the `symphonia` dependency and decoding APIs (`generate` and `generate_from_source`).
- `all`: Enables the `all` feature on Symphonia.

## License

Licensed under the MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>).

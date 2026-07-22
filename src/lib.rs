//! Decodes audio files or streams and downsamples them into a normalized amplitude
//! vector suitable for rendering waveforms.
//!
//! Supports file paths, generic readers (via Symphonia), or processing raw,
//! pre-decoded mono samples directly via [`generate_from_samples`].
//!
//! Downsampling is controlled by [`WaveformOptions`], which selects the number of
//! output points, how each bucket is summarized ([`Measure::Peak`] or
//! [`Measure::Rms`]), and whether the result is normalized.

#[cfg(feature = "symphonia")]
use std::fs::File;
#[cfg(feature = "symphonia")]
use std::io::ErrorKind;
#[cfg(feature = "symphonia")]
use std::path::Path;
#[cfg(feature = "symphonia")]
use symphonia::core::audio::GenericAudioBufferRef;
#[cfg(feature = "symphonia")]
use symphonia::core::codecs::CodecParameters;
#[cfg(feature = "symphonia")]
use symphonia::core::codecs::audio::{AudioDecoderOptions, CODEC_ID_NULL_AUDIO};
#[cfg(feature = "symphonia")]
use symphonia::core::errors::Error;
#[cfg(feature = "symphonia")]
use symphonia::core::formats::FormatOptions;
#[cfg(feature = "symphonia")]
use symphonia::core::formats::probe::Hint;
#[cfg(feature = "symphonia")]
use symphonia::core::meta::MetadataOptions;
#[cfg(feature = "symphonia")]
use symphonia::core::units::Timestamp;
#[cfg(feature = "symphonia")]
pub use symphonia::core::io::{MediaSource, ReadOnlySource};
#[cfg(feature = "symphonia")]
use symphonia::core::io::MediaSourceStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    Peak,
    Rms,
}

/// Controls how samples are downsampled into a waveform.
///
/// Construct with [`WaveformOptions::new`] and adjust with the builder methods:
///
/// ```
/// use audio_waveform::{Measure, WaveformOptions};
///
/// let options = WaveformOptions::new(250)
///     .measure(Measure::Peak)
///     .normalize(false);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveformOptions {
    pub target_len: usize, //num of points
    pub measure: Measure,
    pub normalize: bool, //when true, largest point is 1.0
}

impl WaveformOptions {
    pub fn new(target_len: usize) -> Self {
        Self {
            target_len,
            measure: Measure::Rms,
            normalize: true,
        }
    }

    pub fn measure(mut self, measure: Measure) -> Self {
        self.measure = measure;
        self
    }

    pub fn normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }
}

impl Default for WaveformOptions {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Decodes an audio file at the given path and generates a waveform.
///
/// Returns both the waveform points and the calculated duration in seconds.
#[cfg(feature = "symphonia")]
pub fn generate(path: &Path, options: &WaveformOptions) -> Result<(Vec<f32>, f64), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let extension_hint = path.extension().map(|e| e.to_string_lossy());
    generate_from_source(Box::new(file), extension_hint.as_deref(), options)
}

/// Decodes audio directly from any source implementing `MediaSource`.
///
/// Returns the waveform vector and the duration in seconds.
#[cfg(feature = "symphonia")]
pub fn generate_from_source(
    source: Box<dyn MediaSource>,
    extension_hint: Option<&str>,
    options: &WaveformOptions,
) -> Result<(Vec<f32>, f64), String> {
    let mss = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = extension_hint {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| e.to_string())?;

    let (track_id, codec_params, num_frames, time_base, duration) = format
        .tracks()
        .iter()
        .find_map(|t| match t.codec_params {
            Some(CodecParameters::Audio(ref params)) if params.codec != CODEC_ID_NULL_AUDIO => {
                Some((t.id, params.clone(), t.num_frames, t.time_base, t.duration))
            }
            _ => None,
        })
        .ok_or("No audio track found")?;

    let sample_rate = codec_params.sample_rate.unwrap_or(44100) as f64;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
        .map_err(|e| e.to_string())?;

    // Full-resolution mono samples. Buffered so the waveform resolution is bound by the
    // audio content rather than by the codec's packet size.
    let mut mono = Vec::new();
    let mut interleaved: Vec<f32> = Vec::new();
    let mut total_frames = 0usize;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break, // clean end of stream
            Err(Error::IoError(ref e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(_) => break,
        };

        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                total_frames += decoded.frames();
                append_mono(&decoded, &mut interleaved, &mut mono);
            }
            // if malformed -> skip this one and keep deforming
            Err(Error::DecodeError(_) | Error::ResetRequired) => continue,
            // stop if IO error
            Err(_) => break,
        }
    }

    let calculated_duration = if let (Some(tb), Some(dur)) = (time_base, duration) {
        tb.calc_time(Timestamp::new(dur.get() as i64))
            .map(|t| t.as_secs_f64())
            .unwrap_or(0.0)
    } else if let Some(frames) = num_frames {
        frames as f64 / sample_rate
    } else {
        total_frames as f64 / sample_rate
    };

    let waveform = generate_from_samples(&mono, options);
    Ok((waveform, calculated_duration))
}

/// Appends the mono downmix of a decoded buffer to `mono`.
///
/// Any sample format is converted to `f32` in `[-1.0, 1.0]`, then channels are
/// averaged per frame. `interleaved` is a reusable scratch buffer.
#[cfg(feature = "symphonia")]
fn append_mono(buf: &GenericAudioBufferRef<'_>, interleaved: &mut Vec<f32>, mono: &mut Vec<f32>) {
    let channels = buf.num_planes();
    if channels == 0 {
        return;
    }

    buf.copy_to_vec_interleaved(interleaved);
    for frame in interleaved.chunks_exact(channels) {
        let sum: f32 = frame.iter().sum();
        mono.push(sum / channels as f32);
    }
}

/// Downsamples and (optionally) normalizes pre-decoded mono samples into a waveform.
///
/// Produces exactly `options.target_len` points. The input is split into `target_len`
/// contiguous buckets that together cover every sample; each bucket is summarized
/// according to [`WaveformOptions::measure`]. When there are fewer samples than points,
/// buckets hold the nearest sample rather than collapsing.
pub fn generate_from_samples(samples: &[f32], options: &WaveformOptions) -> Vec<f32> {
    let target_len = options.target_len;
    if target_len == 0 || samples.is_empty() {
        return Vec::new();
    }

    let len = samples.len();
    let mut waveform = Vec::with_capacity(target_len);
    for i in 0..target_len {
        let start = i * len / target_len;
        // clamp so every bucket has at least one sample
        let end = ((i + 1) * len / target_len).max(start + 1).min(len);
        let bucket = &samples[start..end];

        let value = match options.measure {
            Measure::Peak => bucket.iter().fold(0.0f32, |max, &s| max.max(s.abs())),
            Measure::Rms => {
                let sum_sq: f32 = bucket.iter().map(|&s| s * s).sum();
                (sum_sq / bucket.len() as f32).sqrt()
            }
        };
        waveform.push(value);
    }

    if options.normalize {
        let max = waveform.iter().copied().fold(0.0f32, f32::max);
        if max > 0.0 {
            for point in &mut waveform {
                *point /= max;
            }
        }
    }

    waveform
}

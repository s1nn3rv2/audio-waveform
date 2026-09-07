use audio_waveform::{Measure, WaveformOptions, generate_from_samples};

// Bug 1: target_len == 0 must not panic.
#[test]
fn target_zero_no_panic() {
    let w = generate_from_samples(&[0.1, 0.2, 0.3], &WaveformOptions::new(0));
    assert!(w.is_empty());
}

// Bug 2: fewer samples than target must still yield target_len points, no empty result.
#[test]
fn upsample_yields_target_len() {
    let w = generate_from_samples(
        &[0.2, -0.8, 0.4],
        &WaveformOptions::new(10).measure(Measure::Peak),
    );
    assert_eq!(w.len(), 10);
    assert!(w.iter().all(|&p| p >= 0.0));
}

// Bug 3: no trailing samples dropped; last bucket reaches the end.
#[test]
fn covers_all_samples() {
    // Peak, un-normalized: the loudest sample (1.0) is the last one and must appear.
    let mut s = vec![0.1; 7];
    s[6] = 1.0;
    let w = generate_from_samples(
        &s,
        &WaveformOptions::new(3)
            .measure(Measure::Peak)
            .normalize(false),
    );
    assert_eq!(w.len(), 3);
    assert_eq!(
        w[2], 1.0,
        "last sample must be represented in the final bucket"
    );
}

// Peak uses absolute value (bipolar audio).
#[test]
fn peak_is_absolute() {
    let w = generate_from_samples(
        &[-1.0, -0.5],
        &WaveformOptions::new(1)
            .measure(Measure::Peak)
            .normalize(false),
    );
    assert_eq!(w, vec![1.0]);
}

// RMS of a constant-magnitude signal equals that magnitude.
#[test]
fn rms_of_constant() {
    let w = generate_from_samples(
        &[0.5, -0.5, 0.5, -0.5],
        &WaveformOptions::new(1)
            .measure(Measure::Rms)
            .normalize(false),
    );
    assert!((w[0] - 0.5).abs() < 1e-6, "got {}", w[0]);
}

// Normalization scales the peak point to 1.0.
#[test]
fn normalize_scales_peak_to_one() {
    let w = generate_from_samples(
        &[0.1, 0.2, 0.4],
        &WaveformOptions::new(3).measure(Measure::Peak),
    );
    let max = w
        .iter()
        .cloned()
        .fold(0.0f32, f32::max);
    assert!((max - 1.0).abs() < 1e-6, "peak should be 1.0, got {}", max);
}

// empty input -> empty output.
#[test]
fn empty_input() {
    assert!(generate_from_samples(&[], &WaveformOptions::new(5)).is_empty());
}

// u8 waveforms: 0 is silence, 255 is full amplitude.
use audio_waveform::generate_from_u8_samples;

#[test]
fn u8_target_zero_no_panic() {
    assert!(generate_from_u8_samples(&[10, 20, 30], &WaveformOptions::new(0)).is_empty());
}

#[test]
fn u8_empty_input() {
    assert!(generate_from_u8_samples(&[], &WaveformOptions::new(5)).is_empty());
}

#[test]
fn u8_upsample_yields_target_len() {
    let w = generate_from_u8_samples(&[10, 200, 40], &WaveformOptions::new(10));
    assert_eq!(w.len(), 10);
}

#[test]
fn u8_covers_all_samples() {
    let mut s = vec![10u8; 7];
    s[6] = 255;
    let options = WaveformOptions::new(3)
        .measure(Measure::Peak)
        .normalize(false);
    let w = generate_from_u8_samples(&s, &options);
    assert_eq!(w.len(), 3);
    assert_eq!(
        w[2], 255,
        "last sample must be represented in the final bucket"
    );
}

#[test]
fn u8_peak_takes_bucket_max() {
    let options = WaveformOptions::new(2)
        .measure(Measure::Peak)
        .normalize(false);
    let w = generate_from_u8_samples(&[10, 200, 30, 40], &options);
    assert_eq!(w, vec![200, 40]);
}

#[test]
fn u8_rms_of_constant() {
    let options = WaveformOptions::new(1)
        .measure(Measure::Rms)
        .normalize(false);
    let w = generate_from_u8_samples(&[100; 8], &options);
    assert_eq!(w, vec![100]);
}

// Peak would return 200 here.
#[test]
fn u8_rms_is_not_peak() {
    let options = WaveformOptions::new(1)
        .measure(Measure::Rms)
        .normalize(false);
    let w = generate_from_u8_samples(&[0, 200], &options);
    assert_eq!(w, vec![141]); // sqrt(200^2 / 2) = 141.42
}

#[test]
fn u8_normalize_scales_peak_to_full() {
    let w = generate_from_u8_samples(
        &[10, 20, 40],
        &WaveformOptions::new(3).measure(Measure::Peak),
    );
    assert_eq!(w, vec![64, 128, 255], "ratios must survive normalization");
}

// Silence must stay silent, not scale up into a solid block.
#[test]
fn u8_normalize_all_zero_stays_zero() {
    let w = generate_from_u8_samples(&[0; 16], &WaveformOptions::new(4));
    assert_eq!(w, vec![0; 4]);
}

// Both paths must bucket identically, or the two APIs disagree about where a
// given point in the track lives.
#[test]
fn u8_buckets_match_f32_path() {
    let f32_samples: Vec<f32> = (0..1000)
        .map(|i| i as f32 / 999.0)
        .collect();
    let u8_samples: Vec<u8> = f32_samples
        .iter()
        .map(|&v| (v * 255.0).round() as u8)
        .collect();

    let options = WaveformOptions::new(50)
        .measure(Measure::Peak)
        .normalize(false);
    let from_f32 = generate_from_samples(&f32_samples, &options);
    let from_u8 = generate_from_u8_samples(&u8_samples, &options);

    for (i, (&f, &u)) in from_f32
        .iter()
        .zip(from_u8.iter())
        .enumerate()
    {
        let expected = (f * 255.0).round() as u8;
        assert!(
            u.abs_diff(expected) <= 1,
            "point {i}: f32 path gave {expected}, u8 path gave {u}"
        );
    }
}

// full-scale track must reach 255, not truncate to 254.
#[cfg(feature = "symphonia")]
#[test]
fn generate_u8_reaches_full_scale() {
    use std::io::Write;

    let path = std::env::temp_dir().join("audio_waveform_u8_test.wav");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&ramp_wav(4410))
        .unwrap();
    drop(file);

    let (waveform, duration) = audio_waveform::generate_u8(&path, &WaveformOptions::new(20))
        .expect("test wav should decode");
    std::fs::remove_file(&path).ok();

    assert_eq!(waveform.len(), 20);
    assert_eq!(waveform.iter().copied().max(), Some(255));
    assert!(
        (duration - 0.1).abs() < 0.01,
        "expected ~0.1s, got {duration}"
    );
}

/// Builds a mono 16-bit 44.1 kHz WAV rising from silence to full scale.
#[cfg(feature = "symphonia")]
fn ramp_wav(frames: u32) -> Vec<u8> {
    let data_len = frames * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&44100u32.to_le_bytes()); // sample rate
    wav.extend_from_slice(&88200u32.to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let amplitude = (i as f32 / (frames - 1) as f32 * i16::MAX as f32) as i16;
        wav.extend_from_slice(&amplitude.to_le_bytes());
    }
    wav
}

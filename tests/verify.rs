use audio_waveform::{generate_from_samples, Measure, WaveformOptions};

// Bug 1: target_len == 0 must not panic.
#[test]
fn target_zero_no_panic() {
    let w = generate_from_samples(&[0.1, 0.2, 0.3], &WaveformOptions::new(0));
    assert!(w.is_empty());
}

// Bug 2: fewer samples than target must still yield target_len points, no empty result.
#[test]
fn upsample_yields_target_len() {
    let w = generate_from_samples(&[0.2, -0.8, 0.4], &WaveformOptions::new(10).measure(Measure::Peak));
    assert_eq!(w.len(), 10);
    assert!(w.iter().all(|&p| p >= 0.0));
}

// Bug 3: no trailing samples dropped; last bucket reaches the end.
#[test]
fn covers_all_samples() {
    // Peak, un-normalized: the loudest sample (1.0) is the last one and must appear.
    let mut s = vec![0.1; 7];
    s[6] = 1.0;
    let w = generate_from_samples(&s, &WaveformOptions::new(3).measure(Measure::Peak).normalize(false));
    assert_eq!(w.len(), 3);
    assert_eq!(w[2], 1.0, "last sample must be represented in the final bucket");
}

// Peak uses absolute value (bipolar audio).
#[test]
fn peak_is_absolute() {
    let w = generate_from_samples(&[-1.0, -0.5], &WaveformOptions::new(1).measure(Measure::Peak).normalize(false));
    assert_eq!(w, vec![1.0]);
}

// RMS of a constant-magnitude signal equals that magnitude.
#[test]
fn rms_of_constant() {
    let w = generate_from_samples(&[0.5, -0.5, 0.5, -0.5], &WaveformOptions::new(1).measure(Measure::Rms).normalize(false));
    assert!((w[0] - 0.5).abs() < 1e-6, "got {}", w[0]);
}

// Normalization scales the peak point to 1.0.
#[test]
fn normalize_scales_peak_to_one() {
    let w = generate_from_samples(&[0.1, 0.2, 0.4], &WaveformOptions::new(3).measure(Measure::Peak));
    let max = w.iter().cloned().fold(0.0f32, f32::max);
    assert!((max - 1.0).abs() < 1e-6, "peak should be 1.0, got {}", max);
}

// empty input -> empty output.
#[test]
fn empty_input() {
    assert!(generate_from_samples(&[], &WaveformOptions::new(5)).is_empty());
}

use audio_waveform::{generate_from_samples, generate_from_source, Measure, WaveformOptions};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

fn generate_synthetic_samples(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (i as f32 * 0.01).sin() * 0.8)
        .collect()
}

/// Collects real audio files to benchmark full decoding against.
///
/// Looks at the `AUDIO_WAVEFORM_BENCH` env var (a single file or a directory) first,
/// then falls back to the gitignored `benches/fixtures/` directory. Returns an empty
/// vector when nothing is available so the decode benchmark can skip gracefully.
fn collect_fixtures() -> Vec<PathBuf> {
    const AUDIO_EXT: [&str; 6] = ["mp3", "flac", "wav", "ogg", "m4a", "aac"];

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("AUDIO_WAVEFORM_BENCH") {
        roots.push(PathBuf::from(p));
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/fixtures"));

    let is_audio = |p: &PathBuf| {
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| AUDIO_EXT.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    };

    let mut files = Vec::new();
    for root in roots {
        if root.is_file() {
            files.push(root);
        } else if let Ok(entries) = fs::read_dir(&root) {
            for path in entries.flatten().map(|e| e.path()) {
                if path.is_file() && is_audio(&path) {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

fn bench_generate_from_samples(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate_from_samples");

    // Sample sizes: 100k (2.2s audio), 1M (22s audio), 10M (3.7m audio)
    for &sample_count in &[100_000, 1_000_000, 10_000_000] {
        let samples = generate_synthetic_samples(sample_count);
        group.throughput(Throughput::Elements(sample_count as u64));

        // Benchmark Peak measure
        let options_peak = WaveformOptions::new(500).measure(Measure::Peak);
        group.bench_with_input(
            BenchmarkId::new("peak_500pts", sample_count),
            &sample_count,
            |b, _| {
                b.iter(|| {
                    generate_from_samples(black_box(&samples), black_box(&options_peak))
                })
            },
        );

        // Benchmark RMS measure
        let options_rms = WaveformOptions::new(500).measure(Measure::Rms);
        group.bench_with_input(
            BenchmarkId::new("rms_500pts", sample_count),
            &sample_count,
            |b, _| {
                b.iter(|| {
                    generate_from_samples(black_box(&samples), black_box(&options_rms))
                })
            },
        );
    }
    group.finish();
}

fn bench_resolution_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolution_scaling");
    let samples = generate_synthetic_samples(1_000_000); // 1M samples

    for &target_len in &[100, 500, 2000, 10000] {
        let options = WaveformOptions::new(target_len);
        group.bench_with_input(
            BenchmarkId::new("target_len", target_len),
            &target_len,
            |b, _| {
                b.iter(|| {
                    generate_from_samples(black_box(&samples), black_box(&options))
                })
            },
        );
    }
    group.finish();
}

fn bench_full_decoding_pipeline(c: &mut Criterion) {
    let fixtures = collect_fixtures();
    if fixtures.is_empty() {
        eprintln!(
            "skipping `full_decoding_pipeline`: no audio fixtures found. \
             Set AUDIO_WAVEFORM_BENCH=<file|dir> or drop audio files into benches/fixtures/."
        );
        return;
    }

    let mut group = c.benchmark_group("full_decoding_pipeline");
    let options = WaveformOptions::new(500);

    for path in &fixtures {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_owned);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio")
            .to_owned();

        // decode from an in-memory copy so we measure decode + downsample, not disk IO
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_function(BenchmarkId::new("decode", &name), |b| {
            b.iter(|| {
                let source = Box::new(Cursor::new(bytes.clone()));
                generate_from_source(black_box(source), ext.as_deref(), black_box(&options))
                    .unwrap()
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_generate_from_samples,
    bench_resolution_scaling,
    bench_full_decoding_pipeline
);
criterion_main!(benches);

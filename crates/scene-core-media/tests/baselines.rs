//! Measurement harness. Ignored by default; refresh with:
//!   SCENE_CORE_FFMPEG_DIR=<bin> cargo test --release -p scene-core-media --test baselines -- --ignored --nocapture
//!
//! `performance_smoke_gate` is the CI regression guard. It runs only when
//! `SCENE_CORE_PERF_GATE` and `SCENE_CORE_FFMPEG_DIR` are both set, and its
//! thresholds are deliberately loose smoke guards for obvious latency or
//! throughput regressions on shared runners. They are **not** a release SLA;
//! release numbers come from `media_baselines` on a dedicated machine.

use scene_core_media::hash::hash_file;
use scene_core_media::preview::generate;
use scene_core_media::probe::probe;
use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn toolchain() -> Option<Toolchain> {
    match Toolchain::from_env() {
        Ok(toolchain) => Some(toolchain),
        Err(_) => {
            eprintln!("skipping: SCENE_CORE_FFMPEG_DIR is not set");
            None
        }
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scene-core-bench-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn ffmpeg(toolchain: &Toolchain, args: &[&str]) {
    let args: Vec<String> = args.iter().map(|flag| (*flag).to_owned()).collect();
    let result =
        run(&ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(Duration::from_secs(300)))
            .expect("spawn");
    assert!(result.success, "ffmpeg failed: {}", result.stdout_text());
}

fn percentile(values: &mut [u128], percent: f64) -> u128 {
    values.sort_unstable();
    let index = ((values.len() as f64 - 1.0) * percent).round() as usize;
    values[index]
}

#[test]
#[ignore = "measurement harness; run with --release --ignored --nocapture"]
fn media_baselines() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("baselines");
    let media = dir.join("bench.mkv");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=640x480:rate=30",
            "-c:v",
            "mjpeg",
            "-f",
            "matroska",
            "-y",
            &media.to_string_lossy(),
        ],
    );

    let mut probe_times = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        probe(&toolchain, &media).expect("probe");
        probe_times.push(started.elapsed().as_micros());
    }

    let output = dir.join("opening.jpg");
    let mut preview_times = Vec::new();
    for _ in 0..10 {
        let _ = std::fs::remove_file(&output);
        let started = Instant::now();
        generate(&toolchain, &media, 0, 0, &output, None).expect("preview");
        preview_times.push(started.elapsed().as_micros());
    }
    let preview_bytes = std::fs::metadata(&output).expect("metadata").len();

    let large_mib: u64 = std::env::var("SCENE_CORE_BENCH_LARGE_MIB")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(256);
    let large = write_large(&dir, large_mib);
    let started = Instant::now();
    let _ = hash_file(&large).expect("hash");
    let elapsed = started.elapsed();
    let throughput = large_mib as f64 / elapsed.as_secs_f64();

    println!(
        "probe us: p50={} p95={} | preview us: p50={} p95={} | preview bytes: {} | hash {large_mib} MiB/s: {:.0}",
        percentile(&mut probe_times, 0.5),
        percentile(&mut probe_times, 0.95),
        percentile(&mut preview_times, 0.5),
        percentile(&mut preview_times, 0.95),
        preview_bytes,
        throughput
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn limit(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn write_large(dir: &std::path::Path, mib: u64) -> PathBuf {
    use std::io::Write as _;

    let path = dir.join("large.bin");
    let mut file = std::fs::File::create(&path).expect("create");
    let chunk = vec![0x5A_u8; 1024 * 1024];
    for _ in 0..mib {
        file.write_all(&chunk).expect("write chunk");
    }
    file.sync_all().expect("sync");
    path
}

fn millis(micros: u128) -> f64 {
    micros as f64 / 1000.0
}

#[test]
fn performance_smoke_gate() {
    if std::env::var("SCENE_CORE_PERF_GATE").is_err() {
        eprintln!("skipping: SCENE_CORE_PERF_GATE is not set");
        return;
    }
    let toolchain =
        toolchain().expect("SCENE_CORE_PERF_GATE requires SCENE_CORE_FFMPEG_DIR to be set");
    let min_hash_mibps = limit("SCENE_CORE_PERF_MIN_HASH_MIBPS", 50.0);
    let max_probe_p95_ms = limit("SCENE_CORE_PERF_MAX_PROBE_P95_MS", 1500.0);
    let max_preview_p95_ms = limit("SCENE_CORE_PERF_MAX_PREVIEW_P95_MS", 3000.0);
    let max_jitter_ratio = limit("SCENE_CORE_PERF_MAX_JITTER_RATIO", 5.0);
    let hash_mib = limit("SCENE_CORE_PERF_HASH_MIB", 64.0) as u64;

    let dir = temp_dir("perf-gate");
    let media = dir.join("gate.mkv");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=640x480:rate=30",
            "-c:v",
            "mjpeg",
            "-f",
            "matroska",
            "-y",
            &media.to_string_lossy(),
        ],
    );

    let mut probe_times = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        probe(&toolchain, &media).expect("probe");
        probe_times.push(started.elapsed().as_micros());
    }

    let output = dir.join("opening.jpg");
    let mut preview_times = Vec::new();
    for _ in 0..10 {
        let _ = std::fs::remove_file(&output);
        let started = Instant::now();
        generate(&toolchain, &media, 0, 0, &output, None).expect("preview");
        preview_times.push(started.elapsed().as_micros());
    }

    let large = write_large(&dir, hash_mib);
    let started = Instant::now();
    let _ = hash_file(&large).expect("hash");
    let throughput = hash_mib as f64 / started.elapsed().as_secs_f64();

    let probe_p50_ms = millis(percentile(&mut probe_times, 0.5));
    let probe_p95_ms = millis(percentile(&mut probe_times, 0.95));
    let preview_p50_ms = millis(percentile(&mut preview_times, 0.5));
    let preview_p95_ms = millis(percentile(&mut preview_times, 0.95));

    println!(
        "perf smoke: probe p50={probe_p50_ms:.1}ms p95={probe_p95_ms:.1}ms (limit {max_probe_p95_ms}ms) | \
         preview p50={preview_p50_ms:.1}ms p95={preview_p95_ms:.1}ms (limit {max_preview_p95_ms}ms) | \
         hash {hash_mib} MiB {throughput:.0} MiB/s (floor {min_hash_mibps} MiB/s) | \
         jitter ratio limit {max_jitter_ratio}"
    );

    assert!(
        throughput >= min_hash_mibps,
        "hash throughput {throughput:.0} MiB/s is below the smoke floor {min_hash_mibps} MiB/s"
    );
    assert!(
        probe_p95_ms <= max_probe_p95_ms,
        "probe p95 {probe_p95_ms:.1}ms exceeds the smoke ceiling {max_probe_p95_ms}ms"
    );
    assert!(
        preview_p95_ms <= max_preview_p95_ms,
        "preview p95 {preview_p95_ms:.1}ms exceeds the smoke ceiling {max_preview_p95_ms}ms"
    );
    assert!(
        probe_p95_ms <= probe_p50_ms * max_jitter_ratio + 50.0,
        "probe p95/p50 jitter {probe_p95_ms:.1}/{probe_p50_ms:.1}ms exceeds ratio {max_jitter_ratio}"
    );
    assert!(
        preview_p95_ms <= preview_p50_ms * max_jitter_ratio + 50.0,
        "preview p95/p50 jitter {preview_p95_ms:.1}/{preview_p50_ms:.1}ms exceeds ratio {max_jitter_ratio}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

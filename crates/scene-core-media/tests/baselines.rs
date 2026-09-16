//! Measurement harness. Ignored by default; refresh with:
//!   SCENE_CORE_FFMPEG_DIR=<bin> cargo test --release -p scene-core-media --test baselines -- --ignored --nocapture

use scene_core_media::hash::hash_file;
use scene_core_media::preview::generate;
use scene_core_media::probe::probe;
use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
use std::path::{Path, PathBuf};
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
        generate(&toolchain, &media, Some(0), 0, &output, None).expect("preview");
        preview_times.push(started.elapsed().as_micros());
    }
    let preview_bytes = std::fs::metadata(&output).expect("metadata").len();

    let large = dir.join("large.bin");
    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&large).expect("create");
        let chunk = vec![0x5A_u8; 1024 * 1024];
        for _ in 0..256 {
            file.write_all(&chunk).expect("write chunk");
        }
    }
    let started = Instant::now();
    let _ = hash_file(&large).expect("hash");
    let elapsed = started.elapsed();
    let throughput = 256.0 / elapsed.as_secs_f64();

    println!(
        "probe us: p50={} p95={} | preview us: p50={} p95={} | preview bytes: {} | hash MiB/s: {:.0}",
        percentile(&mut probe_times, 0.5),
        percentile(&mut probe_times, 0.95),
        percentile(&mut preview_times, 0.5),
        percentile(&mut preview_times, 0.95),
        preview_bytes,
        throughput
    );
    let _ = std::fs::remove_dir_all(&dir);
}

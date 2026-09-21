//! Audio extraction contract against the locked toolchain.

use scene_core_media::audio::{AudioError, extract, wav_info};
use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

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
    let dir = std::env::temp_dir().join(format!("scene-core-audio-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn generate(toolchain: &Toolchain, output: &std::path::Path, args: &[&str]) {
    let args: Vec<String> = args
        .iter()
        .map(|flag| (*flag).to_owned())
        .chain(["-y".to_owned(), output.to_string_lossy().into_owned()])
        .collect();
    let result =
        run(&ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(Duration::from_secs(120)))
            .expect("spawn");
    assert!(result.success, "media generation failed");
}

#[test]
fn extracts_pcm_at_the_source_rate() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("single");
    let media = dir.join("single.mkv");
    generate(
        &toolchain,
        &media,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1:sample_rate=48000",
            "-ac",
            "2",
            "-c:a",
            "pcm_s16le",
            "-f",
            "matroska",
        ],
    );
    let output = dir.join("track.wav");
    let info = extract(&toolchain, &media, 0, &output, None).expect("extract");
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.channels, 2);
    assert_eq!(info.sample_count, 48_000);
    assert_eq!(wav_info(&output).expect("wav info"), info);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selects_the_requested_track() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("multi");
    let media = dir.join("multi.mkv");
    generate(
        &toolchain,
        &media,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1:sample_rate=48000",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:duration=1:sample_rate=8000",
            "-map",
            "0:a",
            "-map",
            "1:a",
            "-c:a",
            "pcm_s16le",
            "-f",
            "matroska",
        ],
    );
    let first = dir.join("first.wav");
    let info = extract(&toolchain, &media, 0, &first, None).expect("first track");
    assert_eq!((info.sample_rate, info.channels), (48_000, 1));

    let second = dir.join("second.wav");
    let info = extract(&toolchain, &media, 1, &second, None).expect("second track");
    assert_eq!((info.sample_rate, info.channels), (8_000, 1));

    let first_bytes = std::fs::read(&first).expect("read first");
    let second_bytes = std::fs::read(&second).expect("read second");
    assert_ne!(first_bytes, second_bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancellation_stops_the_extraction() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("cancel");
    let media = dir.join("cancel.mkv");
    generate(
        &toolchain,
        &media,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1:sample_rate=48000",
            "-c:a",
            "pcm_s16le",
            "-f",
            "matroska",
        ],
    );
    let flag = Arc::new(AtomicBool::new(true));
    let output = dir.join("track.wav");
    let error = extract(&toolchain, &media, 0, &output, Some(flag)).expect_err("cancelled");
    assert_eq!(error, AudioError::Cancelled);
    let _ = std::fs::remove_dir_all(&dir);
}

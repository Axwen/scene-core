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

fn first_loud_frame(path: &std::path::Path, channels: u16) -> u64 {
    let bytes = std::fs::read(path).expect("read wav");
    let data_start = bytes
        .windows(4)
        .position(|window| window == b"data")
        .expect("data chunk")
        + 8;
    let frame_bytes = usize::from(channels) * 2;
    let mut frame = 0;
    while data_start + (frame + 1) * frame_bytes <= bytes.len() {
        for channel in 0..usize::from(channels) {
            let offset = data_start + frame * frame_bytes + channel * 2;
            let value = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            if value.abs() > 2_000 {
                return frame as u64;
            }
        }
        frame += 1;
    }
    panic!("no loud frame found");
}

#[test]
fn tone_onset_maps_within_one_millisecond() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("onset");
    let media = dir.join("onset.mkv");
    // A hard step at t=0.5s makes the onset sample exact, so the material
    // time computed from the WAV frame index is directly comparable.
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
            "aevalsrc=0.5*gte(t\\,0.5):s=48000:d=1.5",
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
    assert_eq!(info.sample_count, 72_000);

    let onset_frame = first_loud_frame(&output, info.channels);
    let onset_ms = onset_frame * 1000 / u64::from(info.sample_rate);
    assert!(
        onset_ms.abs_diff(500) <= 1,
        "onset mapped to {onset_ms}ms instead of 500ms"
    );
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

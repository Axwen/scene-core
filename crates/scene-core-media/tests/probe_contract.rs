//! Probe contract tests against the locked toolchain. They skip unless
//! `SCENE_CORE_FFMPEG_DIR` points at the extracted bin directory.

use scene_core_media::probe::{ProbeError, probe};
use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
use scene_core_protocol::StreamKind;
use std::path::{Path, PathBuf};
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
    let dir = std::env::temp_dir().join(format!("scene-core-probe-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn generate(toolchain: &Toolchain, output: &Path, arguments: &[&str]) {
    let mut args: Vec<String> = arguments.iter().map(|flag| (*flag).to_owned()).collect();
    args.push(output.to_string_lossy().into_owned());
    let result =
        run(&ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(Duration::from_secs(120)))
            .expect("spawn ffmpeg");
    assert!(result.success, "ffmpeg failed: {}", result.stdout_text());
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/media/golden/synthetic-av.json")
}

#[test]
fn probes_generated_audio_video_media() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("av");
    let media_path = dir.join("synthetic-av.mkv");
    generate(
        &toolchain,
        &media_path,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=64x48:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:v",
            "mjpeg",
            "-c:a",
            "pcm_s16le",
            "-y",
        ],
    );

    let media = probe(&toolchain, &media_path).expect("probe succeeds");
    assert_eq!(media.container.start_time_ms, Some(0));
    assert_eq!(media.streams.len(), 2);
    assert_eq!(media.primary_video_stream_index, Some(0));
    let video = &media.streams[0];
    assert_eq!(video.kind, StreamKind::Video);
    assert_eq!(video.display_width, Some(64));
    assert_eq!(video.display_height, Some(48));
    let duration = media.container.duration_ms.expect("duration is known");
    assert!((900..=1100).contains(&duration), "duration was {duration}");

    let value = serde_json::to_value(&media).expect("media serializes");
    if std::env::var_os("UPDATE_MEDIA_GOLDEN").is_some() {
        let path = golden_path();
        std::fs::create_dir_all(path.parent().expect("golden parent")).expect("golden dir");
        let mut text = serde_json::to_string_pretty(&value).expect("golden JSON");
        text.push('\n');
        std::fs::write(&path, text).expect("write golden");
    } else {
        let text = std::fs::read_to_string(golden_path()).expect("golden exists");
        let golden: serde_json::Value = serde_json::from_str(&text).expect("golden JSON");
        assert_eq!(
            value, golden,
            "probe output drifted from fixtures/media/golden/synthetic-av.json; regenerate with UPDATE_MEDIA_GOLDEN=1 if the toolchain changed"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn probes_audio_only_media() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("audio");
    let media_path = dir.join("synthetic-audio.m4a");
    generate(
        &toolchain,
        &media_path,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:a",
            "aac",
            "-y",
        ],
    );
    let media = probe(&toolchain, &media_path).expect("probe succeeds");
    assert!(
        media
            .streams
            .iter()
            .all(|stream| stream.kind == StreamKind::Audio)
    );
    assert_eq!(media.primary_video_stream_index, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_and_missing_inputs_fail_controlled() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("corrupt");
    let corrupt_path = dir.join("corrupt.bin");
    std::fs::write(&corrupt_path, vec![0x5A; 4096]).expect("write corrupt file");
    let error = probe(&toolchain, &corrupt_path).expect_err("corrupt input fails");
    assert!(matches!(
        error,
        ProbeError::CorruptMedia | ProbeError::UnsupportedInput
    ));
    assert!(matches!(
        error.code(),
        scene_core_protocol::ErrorCode::CorruptMedia
            | scene_core_protocol::ErrorCode::UnsupportedInput
    ));

    let missing = probe(&toolchain, &dir.join("missing.mkv")).expect_err("missing input fails");
    assert!(matches!(missing, ProbeError::UnsupportedInput));
    let _ = std::fs::remove_dir_all(&dir);
}

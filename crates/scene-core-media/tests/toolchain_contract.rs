//! Fixed-toolchain contract tests. They skip unless `SCENE_CORE_FFMPEG_DIR`
//! points at the bin directory extracted by `scripts/fetch-toolchain.sh`.

use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
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
    let dir = std::env::temp_dir().join(format!("scene-core-media-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn tool_args(flags: &[&str], path: &Path) -> Vec<String> {
    let mut args: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
    args.push(path.to_string_lossy().into_owned());
    args
}

#[test]
fn ffprobe_reports_its_version() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let output =
        run(&ProcessSpec::new(toolchain.ffprobe(), ["-version"])
            .with_timeout(Duration::from_secs(30)))
        .expect("spawn");
    assert!(output.success, "ffprobe -version failed");
    assert!(output.stdout_text().contains("ffprobe version"));
}

#[test]
fn ffmpeg_generates_media_and_ffprobe_reads_it_back() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("probe");
    let media = dir.join("sample.avi");

    let generate = run(&ProcessSpec::new(
        toolchain.ffmpeg(),
        tool_args(
            &[
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=0.5:size=64x48:rate=10",
                "-c:v",
                "mjpeg",
                "-y",
            ],
            &media,
        ),
    )
    .with_timeout(Duration::from_secs(120)))
    .expect("spawn");
    assert!(
        generate.success,
        "media generation failed: {}",
        generate.stdout_text()
    );

    let probe = run(&ProcessSpec::new(
        toolchain.ffprobe(),
        tool_args(
            &["-v", "error", "-show_streams", "-print_format", "json"],
            &media,
        ),
    )
    .with_timeout(Duration::from_secs(30)))
    .expect("spawn");
    assert!(probe.success, "ffprobe failed: {}", probe.stdout_text());
    assert!(probe.stdout_text().contains("codec_type"));

    let missing = run(&ProcessSpec::new(
        toolchain.ffprobe(),
        tool_args(
            &["-v", "error", "-show_streams", "-print_format", "json"],
            &dir.join("missing.avi"),
        ),
    )
    .with_timeout(Duration::from_secs(30)))
    .expect("spawn");
    assert!(!missing.success);
    assert!(missing.stderr.len() < 64 * 1024);

    let _ = std::fs::remove_dir_all(&dir);
}

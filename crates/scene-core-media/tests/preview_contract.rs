//! Preview profile contract against the locked toolchain.

use scene_core_media::preview::{MAX_EDGE, generate, jpeg_dimensions};
use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
use std::path::PathBuf;
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
    let dir =
        std::env::temp_dir().join(format!("scene-core-preview-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn generate_media(toolchain: &Toolchain, output: &std::path::Path, size: &str) {
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"]
        .iter()
        .map(|flag| (*flag).to_owned())
        .collect();
    args.push(format!("testsrc=duration=1:size={size}:rate=10"));
    args.extend(
        ["-c:v", "mjpeg", "-f", "matroska", "-y"]
            .iter()
            .map(|flag| (*flag).to_owned()),
    );
    args.push(output.to_string_lossy().into_owned());
    let result =
        run(&ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(Duration::from_secs(120)))
            .expect("spawn");
    assert!(result.success, "media generation failed");
}

#[test]
fn large_frames_are_scaled_down_without_cropping() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("large");
    let media = dir.join("large.mkv");
    generate_media(&toolchain, &media, "640x480");
    let output = dir.join("opening.jpg");
    generate(&toolchain, &media, Some(0), 0, &output, None).expect("preview");
    let bytes = std::fs::read(&output).expect("read preview");
    let (width, height) = jpeg_dimensions(&bytes).expect("jpeg dimensions");
    assert_eq!(width, MAX_EDGE);
    assert_eq!(height, MAX_EDGE * 480 / 640);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn small_frames_are_not_upscaled() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("small");
    let media = dir.join("small.mkv");
    generate_media(&toolchain, &media, "64x48");
    let output = dir.join("opening.jpg");
    generate(&toolchain, &media, Some(0), 0, &output, None).expect("preview");
    let bytes = std::fs::read(&output).expect("read preview");
    let (width, height) = jpeg_dimensions(&bytes).expect("jpeg dimensions");
    assert_eq!((width, height), (64, 48));
    let _ = std::fs::remove_dir_all(&dir);
}

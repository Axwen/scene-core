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

fn generate_material_media(
    toolchain: &Toolchain,
    output: &std::path::Path,
    time_offset: Option<&str>,
) {
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"]
        .iter()
        .map(|flag| (*flag).to_owned())
        .collect();
    args.push("testsrc2=duration=1:size=64x48:rate=10".to_owned());
    args.extend(["-c:v", "mpeg4"].iter().map(|flag| (*flag).to_owned()));
    if let Some(offset) = time_offset {
        args.extend(
            ["-output_ts_offset", offset]
                .iter()
                .map(|flag| (*flag).to_owned()),
        );
    }
    args.extend(["-f", "mp4", "-y"].iter().map(|flag| (*flag).to_owned()));
    args.push(output.to_string_lossy().into_owned());
    let result =
        run(&ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(Duration::from_secs(120)))
            .expect("spawn");
    assert!(result.success, "media generation failed");
}

#[test]
fn nonzero_origin_previews_seek_by_material_time() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("origin");
    let shifted = dir.join("shifted.mp4");
    let flat = dir.join("flat.mp4");
    // The shifted file keeps the same frames at the same material times, but
    // its container origin is 10s, so file time is material time + 10s.
    generate_material_media(&toolchain, &shifted, Some("10"));
    generate_material_media(&toolchain, &flat, None);

    let frame = |name: &str, media: &std::path::Path, requested_ms: u64| {
        let output = dir.join(name);
        generate(&toolchain, media, requested_ms, &output, None)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        std::fs::read(&output).expect("read preview")
    };

    let shifted_opening = frame("shifted-opening.jpg", &shifted, 0);
    let shifted_midpoint = frame("shifted-midpoint.jpg", &shifted, 500);
    let flat_opening = frame("flat-opening.jpg", &flat, 0);
    let flat_midpoint = frame("flat-midpoint.jpg", &flat, 500);

    assert_eq!(
        shifted_opening, flat_opening,
        "material 0 is the first frame"
    );
    assert_eq!(
        shifted_midpoint, flat_midpoint,
        "material 500ms is the same frame in both files"
    );
    assert_ne!(
        shifted_opening, shifted_midpoint,
        "the midpoint preview must not repeat the opening frame"
    );
    let _ = std::fs::remove_dir_all(&dir);
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
    generate(&toolchain, &media, 0, &output, None).expect("preview");
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
    generate(&toolchain, &media, 0, &output, None).expect("preview");
    let bytes = std::fs::read(&output).expect("read preview");
    let (width, height) = jpeg_dimensions(&bytes).expect("jpeg dimensions");
    assert_eq!((width, height), (64, 48));
    let _ = std::fs::remove_dir_all(&dir);
}

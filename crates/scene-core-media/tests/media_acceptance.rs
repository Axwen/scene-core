//! Media acceptance samples: rotation, attached pictures, B-frames, VFR and
//! quick performance smoke checks against the locked toolchain.

use scene_core_media::hash::hash_file;
use scene_core_media::preview::{MAX_EDGE, generate, jpeg_dimensions};
use scene_core_media::probe::probe;
use scene_core_media::process::{ProcessSpec, run};
use scene_core_media::toolchain::Toolchain;
use scene_core_protocol::StreamKind;
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
    let dir = std::env::temp_dir().join(format!("scene-core-accept-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn ffmpeg(toolchain: &Toolchain, args: &[&str]) {
    let args: Vec<String> = args.iter().map(|flag| (*flag).to_owned()).collect();
    let result =
        run(&ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(Duration::from_secs(120)))
            .expect("spawn");
    assert!(
        result.success,
        "ffmpeg failed: {} {}",
        result.stdout_text(),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn rotation_metadata_is_reported() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("rotation");
    let base = dir.join("base.mkv");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=64x48:rate=10",
            "-c:v",
            "mjpeg",
            "-f",
            "matroska",
            "-y",
            &path(&base),
        ],
    );
    let rotated = dir.join("rotated.mkv");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-display_rotation",
            "90",
            "-i",
            &path(&base),
            "-c",
            "copy",
            "-f",
            "matroska",
            "-y",
            &path(&rotated),
        ],
    );
    let media = probe(&toolchain, &rotated).expect("probe");
    assert_eq!(media.streams[0].rotation_degrees, Some(90));
    assert_eq!(media.streams[0].display_width, Some(64));
    assert_eq!(media.streams[0].display_height, Some(48));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn attached_cover_art_is_not_a_primary_video_stream() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("cover");
    let cover = dir.join("cover.flac");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=32x32:rate=1",
            "-map",
            "0:a",
            "-map",
            "1:v",
            "-c:a",
            "flac",
            "-c:v",
            "mjpeg",
            "-disposition:v",
            "attached_pic",
            "-frames:v",
            "1",
            "-f",
            "flac",
            "-y",
            &path(&cover),
        ],
    );
    let media = probe(&toolchain, &cover).expect("probe");
    assert_eq!(media.primary_video_stream_index, None);
    assert!(
        media
            .streams
            .iter()
            .any(|stream| stream.kind == StreamKind::Video && stream.attached_picture)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn b_frames_and_vfr_probe_cleanly() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("bframes");
    let bframes = dir.join("bframes.mkv");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=64x48:rate=10",
            "-c:v",
            "mpeg4",
            "-bf",
            "2",
            "-f",
            "matroska",
            "-y",
            &path(&bframes),
        ],
    );
    let media = probe(&toolchain, &bframes).expect("probe");
    assert_eq!(media.streams[0].start_time_ms, Some(0));
    assert!(media.container.duration_ms.is_some());

    let vfr = dir.join("vfr.mkv");
    ffmpeg(
        &toolchain,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=64x48:rate=10",
            "-vf",
            "select='not(mod(n,3))'",
            "-fps_mode",
            "vfr",
            "-c:v",
            "mpeg4",
            "-f",
            "matroska",
            "-y",
            &path(&vfr),
        ],
    );
    let vfr_media = probe(&toolchain, &vfr).expect("vfr probe");
    assert_eq!(vfr_media.streams[0].start_time_ms, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn preview_and_hash_performance_smoke() {
    let Some(toolchain) = toolchain() else {
        return;
    };
    let dir = temp_dir("perf");
    let media = dir.join("perf.mkv");
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
            &path(&media),
        ],
    );

    let started = Instant::now();
    let output = dir.join("opening.jpg");
    generate(&toolchain, &media, Some(0), 0, &output, None).expect("preview");
    let preview_ms = started.elapsed().as_millis();
    let bytes = std::fs::read(&output).expect("read");
    let (width, height) = jpeg_dimensions(&bytes).expect("dims");
    assert_eq!(width, MAX_EDGE);
    assert_eq!(height, MAX_EDGE * 480 / 640);
    assert!(bytes.len() < 512 * 1024, "preview bytes: {}", bytes.len());
    println!(
        "preview: {preview_ms} ms, {} bytes, {width}x{height}",
        bytes.len()
    );

    let large = dir.join("large.bin");
    std::fs::write(&large, vec![0xAB_u8; 64 * 1024 * 1024]).expect("write large file");
    let started = Instant::now();
    let digest = hash_file(&large).expect("hash");
    let elapsed = started.elapsed();
    let megabytes = 64.0 / elapsed.as_secs_f64();
    println!("hash: 64 MiB in {elapsed:?} ({megabytes:.0} MiB/s) {digest}");
    assert!(
        megabytes > 2.0,
        "hash throughput collapsed: {megabytes:.0} MiB/s"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

//! Fixed diagnostic preview: JPEG, max edge 512, no upscale, display rotation
//! applied by the decoder, metadata stripped, software decode.

use crate::process::{ProcessError, ProcessSpec, run};
use crate::toolchain::Toolchain;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const MAX_EDGE: u32 = 512;
const PREVIEW_TIMEOUT: Duration = Duration::from_secs(120);
const SCALE_FILTER: &str =
    "scale='min(512,iw)':'min(512,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    ToolUnavailable,
    ToolFailed,
    Timeout,
    Cancelled,
    NoVideoStream,
    ResourceLimit,
    EngineInternal(&'static str),
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            PreviewError::ToolUnavailable => "ffmpeg is unavailable",
            PreviewError::ToolFailed => "preview generation failed",
            PreviewError::Timeout => "preview generation reached its deadline",
            PreviewError::Cancelled => "preview generation was cancelled",
            PreviewError::NoVideoStream => "the media has no decodable video stream",
            PreviewError::ResourceLimit => "a resource limit was reached",
            PreviewError::EngineInternal(reason) => reason,
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PreviewError {}

/// Writes one JPEG frame for `requested_time_ms` material time, that is time
/// relative to the container presentation origin. Input-side `-ss` is the
/// right coordinate system for that: without `-seek_timestamp` FFmpeg offsets
/// the argument by the input start time itself, so adding the origin here
/// would seek twice as far.
pub fn generate(
    toolchain: &Toolchain,
    input: &Path,
    requested_time_ms: u64,
    output: &Path,
    cancellation: Option<Arc<AtomicBool>>,
) -> Result<(), PreviewError> {
    let mut args: Vec<String> = vec![
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-nostdin".to_owned(),
        "-ss".to_owned(),
        format_seconds(i64::try_from(requested_time_ms).unwrap_or(i64::MAX)),
    ];
    args.extend([
        "-i".to_owned(),
        input.to_string_lossy().into_owned(),
        "-frames:v".to_owned(),
        "1".to_owned(),
        "-vf".to_owned(),
        SCALE_FILTER.to_owned(),
        "-q:v".to_owned(),
        "3".to_owned(),
        "-f".to_owned(),
        "image2".to_owned(),
        "-y".to_owned(),
        output.to_string_lossy().into_owned(),
    ]);
    let mut spec = ProcessSpec::new(toolchain.ffmpeg(), args).with_timeout(PREVIEW_TIMEOUT);
    if let Some(flag) = cancellation {
        spec = spec.with_cancellation(flag);
    }
    let result = run(&spec).map_err(|error| match error {
        ProcessError::Spawn(_) => PreviewError::ToolUnavailable,
        ProcessError::Wait(_) => PreviewError::EngineInternal("ffmpeg wait failed"),
        ProcessError::Read(_) => PreviewError::EngineInternal("ffmpeg output could not be read"),
    })?;
    if result.cancelled {
        return Err(PreviewError::Cancelled);
    }
    if result.timed_out {
        return Err(PreviewError::Timeout);
    }
    if !result.success {
        let stderr = String::from_utf8_lossy(&result.stderr).to_lowercase();
        if stderr.contains("no space left") || stderr.contains("permission denied") {
            return Err(PreviewError::ResourceLimit);
        }
        return Err(PreviewError::ToolFailed);
    }
    if !output.is_file() {
        return Err(PreviewError::ToolFailed);
    }
    Ok(())
}

fn format_seconds(milliseconds: i64) -> String {
    let sign = if milliseconds < 0 { "-" } else { "" };
    let magnitude = milliseconds.unsigned_abs();
    format!("{sign}{}.{:03}", magnitude / 1000, magnitude % 1000)
}

/// Reads width/height from the first JPEG SOF marker.
pub fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut index = 2;
    while index + 3 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        index += 2;
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if index + 1 >= bytes.len() {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([bytes[index], bytes[index + 1]]));
        if matches!(
            marker,
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
        ) {
            if index + 7 >= bytes.len() {
                return None;
            }
            let height = u32::from(u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]));
            let width = u32::from(u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]));
            return Some((width, height));
        }
        index += length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_milliseconds_as_seconds() {
        assert_eq!(format_seconds(0), "0.000");
        assert_eq!(format_seconds(1500), "1.500");
        assert_eq!(format_seconds(-80), "-0.080");
    }

    #[test]
    fn rejects_non_jpeg_input() {
        assert_eq!(jpeg_dimensions(b"not a jpeg"), None);
    }
}

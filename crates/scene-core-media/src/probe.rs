//! FFprobe normalization into the Protocol 0.1 `NormalizedMedia` DTO.
//!
//! Times follow the confirmed material time policy: the container
//! presentation origin is the single origin, stream starts are signed
//! milliseconds relative to it, unknown values stay `null`, and an
//! unexplainable timeline is a controlled failure rather than a guess.

use crate::process::{ProcessError, ProcessSpec, run};
use crate::toolchain::Toolchain;
use scene_core_protocol::{
    ContainerInfo, ErrorCode, ExactTime, MediaStream, NormalizedMedia, Rational, StreamKind,
};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    ToolUnavailable,
    ToolFailed,
    Timeout,
    Cancelled,
    CorruptMedia,
    UnsupportedInput,
    EngineInternal(&'static str),
}

impl ProbeError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            ProbeError::ToolUnavailable => ErrorCode::ToolUnavailable,
            ProbeError::ToolFailed => ErrorCode::ToolFailed,
            ProbeError::Timeout => ErrorCode::Timeout,
            ProbeError::Cancelled => ErrorCode::Cancelled,
            ProbeError::CorruptMedia => ErrorCode::CorruptMedia,
            ProbeError::UnsupportedInput => ErrorCode::UnsupportedInput,
            ProbeError::EngineInternal(_) => ErrorCode::EngineInternal,
        }
    }
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            ProbeError::ToolUnavailable => "ffprobe is unavailable",
            ProbeError::ToolFailed => "ffprobe failed",
            ProbeError::Timeout => "ffprobe reached its deadline",
            ProbeError::Cancelled => "ffprobe was cancelled",
            ProbeError::CorruptMedia => "the media is corrupt or cannot be parsed",
            ProbeError::UnsupportedInput => "the media format is unsupported",
            ProbeError::EngineInternal(reason) => reason,
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProbeError {}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    format: Option<FormatInfo>,
    #[serde(default)]
    streams: Vec<StreamInfo>,
}

#[derive(Debug, Deserialize)]
struct FormatInfo {
    #[serde(default)]
    format_name: Option<String>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    bit_rate: Option<String>,
    #[serde(default)]
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamInfo {
    index: u32,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    time_base: Option<String>,
    #[serde(default)]
    disposition: Option<Disposition>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    coded_width: Option<u32>,
    #[serde(default)]
    coded_height: Option<u32>,
    #[serde(default)]
    avg_frame_rate: Option<String>,
    #[serde(default)]
    sample_rate: Option<String>,
    #[serde(default)]
    channels: Option<u32>,
    #[serde(default)]
    channel_layout: Option<String>,
    #[serde(default)]
    side_data_list: Vec<SideData>,
}

#[derive(Debug, Deserialize)]
struct Disposition {
    #[serde(default)]
    default: i64,
    #[serde(default)]
    attached_pic: i64,
}

#[derive(Debug, Deserialize)]
struct SideData {
    #[serde(default)]
    rotation: Option<f64>,
}

/// Runs the locked ffprobe and normalizes its output.
pub fn probe(toolchain: &Toolchain, input: &Path) -> Result<NormalizedMedia, ProbeError> {
    probe_with_cancel(toolchain, input, None)
}

/// Same as [`probe`], with an optional cancellation flag for deadline and
/// host cancellation.
pub fn probe_with_cancel(
    toolchain: &Toolchain,
    input: &Path,
    cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<NormalizedMedia, ProbeError> {
    if !input.is_file() {
        return Err(ProbeError::UnsupportedInput);
    }
    let mut spec = ProcessSpec::new(
        toolchain.ffprobe(),
        [
            "-v".to_owned(),
            "error".to_owned(),
            "-print_format".to_owned(),
            "json".to_owned(),
            "-show_format".to_owned(),
            "-show_streams".to_owned(),
            input.to_string_lossy().into_owned(),
        ],
    )
    .with_timeout(PROBE_TIMEOUT);
    if let Some(flag) = cancellation {
        spec = spec.with_cancellation(flag);
    }
    let output = match run(&spec) {
        Ok(output) => output,
        Err(ProcessError::Spawn(_)) => return Err(ProbeError::ToolUnavailable),
        Err(ProcessError::Wait(_)) => {
            return Err(ProbeError::EngineInternal("ffprobe wait failed"));
        }
    };
    if output.cancelled {
        return Err(ProbeError::Cancelled);
    }
    if output.timed_out {
        return Err(ProbeError::Timeout);
    }
    if !output.success {
        return Err(classify_failure(
            &output.stdout_text(),
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
        .map_err(|_| ProbeError::EngineInternal("ffprobe output is not valid JSON"))?;
    normalize(&parsed, input)
}

fn classify_failure(stdout: &str, stderr: &str) -> ProbeError {
    let haystack = format!("{stdout}\n{stderr}").to_lowercase();
    if haystack.contains("invalid data")
        || haystack.contains("moov atom")
        || haystack.contains("malformed")
        || haystack.contains("corrupt")
    {
        ProbeError::CorruptMedia
    } else {
        ProbeError::UnsupportedInput
    }
}

fn normalize(parsed: &FfprobeOutput, input: &Path) -> Result<NormalizedMedia, ProbeError> {
    let format = parsed.format.as_ref();
    let format_name = format
        .and_then(|format| format.format_name.clone())
        .filter(|name| !name.is_empty())
        .ok_or(ProbeError::EngineInternal(
            "ffprobe returned no format name",
        ))?;

    let origin = match format.and_then(|format| format.start_time.as_deref()) {
        Some(value) => optional_seconds(value)?,
        None => None,
    };

    let mut streams = Vec::new();
    for stream in &parsed.streams {
        let Some(kind) = stream_kind(stream) else {
            continue;
        };
        let time_base = stream.time_base.as_deref().and_then(parse_rational);
        let start_time_ms = match origin {
            Some(origin) => match stream.start_time.as_deref() {
                Some(value) => match optional_seconds(value)? {
                    Some(start) => Some(
                        start
                            .checked_sub(&origin)
                            .map_err(|_| ProbeError::UnsupportedInput)?
                            .to_ms_round_nearest()
                            .map_err(|_| ProbeError::UnsupportedInput)?,
                    ),
                    None => None,
                },
                None => None,
            },
            None => None,
        };
        let duration_ms = match stream.duration.as_deref() {
            Some(value) => duration_ms(value)?,
            None => None,
        };
        let disposition = stream.disposition.as_ref();
        let attached_picture = disposition.is_some_and(|value| value.attached_pic != 0);
        let default_disposition = disposition.is_some_and(|value| value.default != 0);
        let is_video = kind == StreamKind::Video;
        let is_audio = kind == StreamKind::Audio;
        let rotation_degrees = if is_video {
            stream
                .side_data_list
                .iter()
                .find_map(|side_data| side_data.rotation)
                .map(|rotation| rotation.round() as i32)
        } else {
            None
        };
        streams.push(MediaStream {
            index: stream.index,
            kind,
            codec_name: stream
                .codec_name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "unknown".to_owned()),
            duration_ms,
            start_time_ms,
            time_base,
            default_disposition,
            attached_picture,
            coded_width: if is_video {
                stream.coded_width.or(stream.width)
            } else {
                None
            },
            coded_height: if is_video {
                stream.coded_height.or(stream.height)
            } else {
                None
            },
            display_width: if is_video { stream.width } else { None },
            display_height: if is_video { stream.height } else { None },
            rotation_degrees,
            average_frame_rate: if is_video {
                stream
                    .avg_frame_rate
                    .as_deref()
                    .and_then(parse_rational)
                    .filter(|rate| rate.num > 0)
            } else {
                None
            },
            sample_rate: if is_audio {
                parse_u32(stream.sample_rate.as_deref())
            } else {
                None
            },
            channels: if is_audio { stream.channels } else { None },
            channel_layout: if is_audio {
                stream
                    .channel_layout
                    .clone()
                    .filter(|layout| !layout.is_empty())
            } else {
                None
            },
        });
    }

    let primary_video_stream_index = streams
        .iter()
        .find(|stream| stream.kind == StreamKind::Video && !stream.attached_picture)
        .or_else(|| {
            streams
                .iter()
                .find(|stream| stream.kind == StreamKind::Video)
        })
        .map(|stream| stream.index);

    let container = ContainerInfo {
        format_name,
        duration_ms: match format.and_then(|format| format.duration.as_deref()) {
            Some(value) => duration_ms(value)?,
            None => None,
        },
        start_time_ms: scene_core_protocol::container_start_ms(origin),
        bit_rate_bps: parse_u64(format.and_then(|format| format.bit_rate.as_deref())),
        file_size_bytes: parse_u64(format.and_then(|format| format.size.as_deref()))
            .or_else(|| std::fs::metadata(input).ok().map(|metadata| metadata.len()))
            .unwrap_or(0),
    };

    let media = NormalizedMedia {
        container,
        streams,
        primary_video_stream_index,
    };
    media
        .validate()
        .map_err(|_| ProbeError::EngineInternal("normalized media failed validation"))?;
    Ok(media)
}

fn stream_kind(stream: &StreamInfo) -> Option<StreamKind> {
    match stream.codec_type.as_deref() {
        Some("video") => Some(StreamKind::Video),
        Some("audio") => Some(StreamKind::Audio),
        Some("subtitle") => Some(StreamKind::Subtitle),
        Some("data") => Some(StreamKind::Data),
        Some("attachment") => Some(StreamKind::Attachment),
        _ => None,
    }
}

fn optional_seconds(value: &str) -> Result<Option<ExactTime>, ProbeError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") {
        return Ok(None);
    }
    parse_seconds(trimmed)
        .map(Some)
        .ok_or(ProbeError::UnsupportedInput)
}

fn parse_seconds(value: &str) -> Option<ExactTime> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("n/a") {
        return None;
    }
    let (negative, digits) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value.strip_prefix('+').unwrap_or(value)),
    };
    let (integer, fraction) = match digits.split_once('.') {
        Some((integer, fraction)) => (integer, fraction),
        None => (digits, ""),
    };
    if integer.is_empty() && fraction.is_empty() {
        return None;
    }
    let integer: i128 = if integer.is_empty() {
        0
    } else {
        integer.parse().ok()?
    };
    if fraction.len() > 9 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let scale = 10_i128.checked_pow(fraction.len() as u32)?;
    let fraction: i128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().ok()?
    };
    let numerator = integer.checked_mul(scale)?.checked_add(fraction)?;
    let numerator = if negative {
        numerator.checked_neg()?
    } else {
        numerator
    };
    ExactTime::from_parts(numerator, scale).ok()
}

fn parse_rational(value: &str) -> Option<Rational> {
    let (num, den) = value.trim().split_once('/')?;
    let num: i64 = num.trim().parse().ok()?;
    let den: i64 = den.trim().parse().ok()?;
    if den <= 0 {
        return None;
    }
    Some(Rational { num, den })
}

fn parse_u64(value: Option<&str>) -> Option<u64> {
    value.and_then(|value| value.trim().parse().ok())
}

fn parse_u32(value: Option<&str>) -> Option<u32> {
    value.and_then(|value| value.trim().parse().ok())
}

fn duration_ms(value: &str) -> Result<Option<u64>, ProbeError> {
    let Some(seconds) = optional_seconds(value)? else {
        return Ok(None);
    };
    if seconds.is_negative() {
        return Ok(None);
    }
    u64::try_from(
        seconds
            .to_ms_round_nearest()
            .map_err(|_| ProbeError::UnsupportedInput)?,
    )
    .map(Some)
    .map_err(|_| ProbeError::UnsupportedInput)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_json(value: &str) -> FfprobeOutput {
        serde_json::from_str(value).expect("fixture JSON")
    }

    fn normalize_json(value: &str) -> Result<NormalizedMedia, ProbeError> {
        normalize(&probe_json(value), Path::new("/nonexistent/fixture.media"))
    }

    const HEADER: &str = r#""format_name": "matroska,webm", "duration": "60.000000", "start_time": "10.000000", "bit_rate": "1000000", "size": "1048576""#;

    #[test]
    fn keeps_audio_offset_relative_to_the_shared_origin() {
        let value = format!(
            r#"{{ "format": {{ {HEADER} }},
                 "streams": [
                   {{ "index": 0, "codec_name": "h264", "codec_type": "video", "start_time": "10.000000", "duration": "60.000000", "time_base": "1/1000", "width": 1920, "height": 1080, "coded_width": 1920, "coded_height": 1088, "avg_frame_rate": "30/1", "disposition": {{ "default": 1 }} }},
                   {{ "index": 1, "codec_name": "aac", "codec_type": "audio", "start_time": "10.080000", "duration": "60.000000", "time_base": "1/48000", "sample_rate": "48000", "channels": 2, "channel_layout": "stereo", "disposition": {{ "default": 1 }} }}
                 ] }}"#
        );
        let media = normalize_json(&value).expect("normalizes");
        assert_eq!(media.container.start_time_ms, Some(0));
        assert_eq!(media.streams[0].start_time_ms, Some(0));
        assert_eq!(media.streams[1].start_time_ms, Some(80));
        assert_eq!(media.primary_video_stream_index, Some(0));
        assert_eq!(media.streams[0].coded_height, Some(1088));
        assert_eq!(media.streams[0].display_height, Some(1080));
        assert_eq!(
            media.streams[1].average_frame_rate, None,
            "audio streams have no frame rate"
        );
    }

    #[test]
    fn unknown_origin_keeps_every_start_null() {
        let value = r#"{ "format": { "format_name": "matroska,webm" },
             "streams": [ { "index": 0, "codec_name": "h264", "codec_type": "video", "start_time": "1.000000" } ] }"#;
        let media = normalize_json(value).expect("normalizes");
        assert_eq!(media.container.start_time_ms, None);
        assert_eq!(media.streams[0].start_time_ms, None);
        assert_eq!(media.container.duration_ms, None);
    }

    #[test]
    fn negative_stream_start_is_signed() {
        let value = r#"{ "format": { "format_name": "matroska,webm", "start_time": "0.000000" },
             "streams": [ { "index": 0, "codec_name": "aac", "codec_type": "audio", "start_time": "-0.080000", "sample_rate": "48000", "channels": 2 } ] }"#;
        let media = normalize_json(value).expect("normalizes");
        assert_eq!(media.streams[0].start_time_ms, Some(-80));
    }

    #[test]
    fn half_millisecond_offsets_round_away_from_zero() {
        let value = r#"{ "format": { "format_name": "matroska,webm", "start_time": "10.000000" },
             "streams": [
               { "index": 0, "codec_name": "h264", "codec_type": "video", "start_time": "10.000500" },
               { "index": 1, "codec_name": "aac", "codec_type": "audio", "start_time": "9.999500" }
             ] }"#;
        let media = normalize_json(value).expect("normalizes");
        assert_eq!(media.streams[0].start_time_ms, Some(1));
        assert_eq!(media.streams[1].start_time_ms, Some(-1));
    }

    #[test]
    fn attached_pictures_are_not_the_primary_video_stream() {
        let value = r#"{ "format": { "format_name": "mp3" },
             "streams": [
               { "index": 0, "codec_name": "mjpeg", "codec_type": "video", "disposition": { "attached_pic": 1 } },
               { "index": 1, "codec_name": "mp3", "codec_type": "audio", "sample_rate": "44100", "channels": 2 }
             ] }"#;
        let media = normalize_json(value).expect("normalizes");
        assert_eq!(media.primary_video_stream_index, Some(0));
        assert!(media.streams[0].attached_picture);
        assert_eq!(media.streams[0].kind, StreamKind::Video);
    }

    #[test]
    fn undecodable_values_stay_null() {
        let value = r#"{ "format": { "format_name": "mpegts", "duration": "N/A" },
             "streams": [ { "index": 0, "codec_name": "h264", "codec_type": "video", "avg_frame_rate": "0/0", "time_base": "0/0" } ] }"#;
        let media = normalize_json(value).expect("N/A duration is unknown, not an error");
        assert_eq!(media.container.duration_ms, None);
        assert_eq!(media.streams[0].average_frame_rate, None);
        assert_eq!(media.streams[0].time_base, None);
    }

    #[test]
    fn negative_duration_is_unknown() {
        let value = r#"{ "format": { "format_name": "matroska,webm", "duration": "-1.000000" },
             "streams": [] }"#;
        let media = normalize_json(value).expect("normalizes");
        assert_eq!(media.container.duration_ms, None);
    }

    #[test]
    fn subtitle_streams_carry_no_av_fields() {
        let value = r#"{ "format": { "format_name": "matroska,webm" },
             "streams": [ { "index": 3, "codec_name": "subrip", "codec_type": "subtitle" } ] }"#;
        let media = normalize_json(value).expect("normalizes");
        assert_eq!(media.streams[0].kind, StreamKind::Subtitle);
        assert_eq!(media.streams[0].coded_width, None);
        assert_eq!(media.streams[0].sample_rate, None);
    }

    #[test]
    fn corrupt_and_unsupported_failures_are_classified() {
        assert_eq!(
            classify_failure("", "Invalid data found when processing input"),
            ProbeError::CorruptMedia
        );
        assert_eq!(
            classify_failure("", "moov atom not found"),
            ProbeError::CorruptMedia
        );
        assert_eq!(
            classify_failure("", "Unknown input format: 'xyz'"),
            ProbeError::UnsupportedInput
        );
    }

    #[test]
    fn parsed_times_keep_exact_rationals() {
        assert_eq!(
            parse_seconds("1.5").expect("parses"),
            ExactTime::from_milliseconds(1500)
        );
        assert!(parse_seconds("n/a").is_none());
        assert!(parse_seconds("1.2.3").is_none());
    }
}

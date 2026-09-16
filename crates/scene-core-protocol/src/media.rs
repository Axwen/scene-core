//! Normalized media metadata returned by `probe` and `extract_preview`.
//!
//! Optional values are explicit fields and serialize as `null` when unknown.
//! Container presentation start is `0` when the display origin is known,
//! otherwise `null`; stream starts are signed material milliseconds relative
//! to that single origin and are never zeroed per track.

use crate::error::{ValidationError, validate_safe_text};
use crate::time::Rational;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NormalizedMedia {
    pub container: ContainerInfo,
    pub streams: Vec<MediaStream>,
    pub primary_video_stream_index: Option<u32>,
}

impl NormalizedMedia {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.container.validate()?;
        for (position, stream) in self.streams.iter().enumerate() {
            stream.validate(&format!("streams[{position}]"))?;
        }
        for (position, stream) in self.streams.iter().enumerate() {
            for other in self.streams.iter().skip(position + 1) {
                if stream.index == other.index {
                    return Err(ValidationError::new(
                        "streams[].index",
                        "must be unique within one media",
                    ));
                }
            }
        }
        if let Some(primary) = self.primary_video_stream_index {
            let is_video = self
                .streams
                .iter()
                .any(|stream| stream.index == primary && stream.kind == StreamKind::Video);
            if !is_video {
                return Err(ValidationError::new(
                    "primaryVideoStreamIndex",
                    "must reference an existing video stream",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContainerInfo {
    pub format_name: String,
    pub duration_ms: Option<u64>,
    #[schemars(extend("maximum" = 0))]
    pub start_time_ms: Option<u64>,
    pub bit_rate_bps: Option<u64>,
    pub file_size_bytes: u64,
}

impl ContainerInfo {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text("container.formatName", &self.format_name, 128)?;
        if let Some(start_ms) = self.start_time_ms {
            if start_ms != 0 {
                return Err(ValidationError::new(
                    "container.startTimeMs",
                    "must be 0 when the presentation origin is known, or null",
                ));
            }
        }
        Ok(())
    }
}

/// Stream kinds recognized by the normalized media DTO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Data,
    Attachment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MediaStream {
    pub index: u32,
    pub kind: StreamKind,
    pub codec_name: String,
    pub duration_ms: Option<u64>,
    pub start_time_ms: Option<i64>,
    pub time_base: Option<Rational>,
    pub default_disposition: bool,
    pub attached_picture: bool,
    pub coded_width: Option<u32>,
    pub coded_height: Option<u32>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub rotation_degrees: Option<i32>,
    pub average_frame_rate: Option<Rational>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
}

impl MediaStream {
    pub fn validate(&self, path: &str) -> Result<(), ValidationError> {
        validate_safe_text(&format!("{path}.codecName"), &self.codec_name, 128)?;
        if let Some(time_base) = &self.time_base {
            time_base.validate().map_err(|error| {
                ValidationError::new(format!("{path}.timeBase.{}", error.path()), error.message())
            })?;
        }
        if let Some(frame_rate) = &self.average_frame_rate {
            frame_rate.validate().map_err(|error| {
                ValidationError::new(
                    format!("{path}.averageFrameRate.{}", error.path()),
                    error.message(),
                )
            })?;
        }
        if self.attached_picture && self.kind != StreamKind::Video {
            return Err(ValidationError::new(
                format!("{path}.attachedPicture"),
                "requires a video stream",
            ));
        }
        for (field, value) in [
            ("codedWidth", self.coded_width),
            ("codedHeight", self.coded_height),
            ("displayWidth", self.display_width),
            ("displayHeight", self.display_height),
            ("sampleRate", self.sample_rate),
            ("channels", self.channels),
        ] {
            if value == Some(0) {
                return Err(ValidationError::new(
                    format!("{path}.{field}"),
                    "must be positive when known, or null",
                ));
            }
        }
        if let Some(layout) = &self.channel_layout {
            validate_safe_text(&format!("{path}.channelLayout"), layout, 128)?;
        }
        let video_fields_present = self.coded_width.is_some()
            || self.coded_height.is_some()
            || self.display_width.is_some()
            || self.display_height.is_some()
            || self.rotation_degrees.is_some()
            || self.average_frame_rate.is_some();
        let audio_fields_present =
            self.sample_rate.is_some() || self.channels.is_some() || self.channel_layout.is_some();
        match self.kind {
            StreamKind::Video => {
                if audio_fields_present {
                    return Err(ValidationError::new(
                        format!("{path}.kind"),
                        "video streams must not carry audio fields",
                    ));
                }
            }
            StreamKind::Audio => {
                if video_fields_present {
                    return Err(ValidationError::new(
                        format!("{path}.kind"),
                        "audio streams must not carry video fields",
                    ));
                }
            }
            StreamKind::Subtitle | StreamKind::Data | StreamKind::Attachment => {
                if video_fields_present || audio_fields_present {
                    return Err(ValidationError::new(
                        format!("{path}.kind"),
                        "non-av streams must not carry video or audio fields",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn unknown_stream() -> MediaStream {
        MediaStream {
            index: 0,
            kind: StreamKind::Video,
            codec_name: "h264".to_owned(),
            duration_ms: None,
            start_time_ms: None,
            time_base: None,
            default_disposition: false,
            attached_picture: false,
            coded_width: None,
            coded_height: None,
            display_width: None,
            display_height: None,
            rotation_degrees: None,
            average_frame_rate: None,
            sample_rate: None,
            channels: None,
            channel_layout: None,
        }
    }

    #[test]
    fn unknown_optional_values_serialize_as_explicit_null() {
        let value = serde_json::to_value(unknown_stream()).expect("serializes");
        for field in [
            "durationMs",
            "startTimeMs",
            "timeBase",
            "codedWidth",
            "displayHeight",
            "rotationDegrees",
            "averageFrameRate",
            "channelLayout",
        ] {
            assert_eq!(value[field], Value::Null, "{field}");
        }

        let media = NormalizedMedia {
            container: ContainerInfo {
                format_name: "matroska,webm".to_owned(),
                duration_ms: None,
                start_time_ms: None,
                bit_rate_bps: None,
                file_size_bytes: 0,
            },
            streams: vec![unknown_stream()],
            primary_video_stream_index: None,
        };
        assert!(media.validate().is_ok());
        let value = serde_json::to_value(&media).expect("serializes");
        assert_eq!(value["container"]["startTimeMs"], Value::Null);
        assert_eq!(value["container"]["durationMs"], Value::Null);
        assert_eq!(value["primaryVideoStreamIndex"], Value::Null);
    }

    #[test]
    fn kind_specific_fields_must_stay_empty() {
        let mut audio_with_video_fields = unknown_stream();
        audio_with_video_fields.kind = StreamKind::Audio;
        audio_with_video_fields.coded_width = Some(1920);
        assert!(audio_with_video_fields.validate("stream").is_err());

        audio_with_video_fields.coded_width = None;
        audio_with_video_fields.coded_height = None;
        audio_with_video_fields.display_width = None;
        audio_with_video_fields.display_height = None;
        audio_with_video_fields.rotation_degrees = None;
        audio_with_video_fields.average_frame_rate = None;
        audio_with_video_fields.sample_rate = Some(48_000);
        audio_with_video_fields.channels = Some(2);
        audio_with_video_fields.channel_layout = Some("stereo".to_owned());
        assert!(audio_with_video_fields.validate("stream").is_ok());
    }

    #[test]
    fn container_start_and_primary_index_are_validated() {
        let mut media = NormalizedMedia {
            container: ContainerInfo {
                format_name: "matroska,webm".to_owned(),
                duration_ms: Some(1),
                start_time_ms: Some(0),
                bit_rate_bps: None,
                file_size_bytes: 1,
            },
            streams: vec![unknown_stream()],
            primary_video_stream_index: Some(9),
        };
        assert!(media.validate().is_err());

        media.primary_video_stream_index = None;
        media.container.start_time_ms = Some(5);
        assert!(media.validate().is_err());

        media.container.start_time_ms = None;
        assert!(media.validate().is_ok());
    }
}

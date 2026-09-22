//! Media adapters behind [`MediaBackend`]: the real FFmpeg toolchain, the
//! unavailable placeholder, and the mapping from media errors to failures.

use super::{control::RunControl, remove_file_if_exists};
use scene_core_media::audio::{self, AudioError};
use scene_core_media::hash::hash_file;
use scene_core_media::preview::{self, PreviewError};
use scene_core_media::probe::{ProbeError, probe_with_cancel, probe_with_origin};
use scene_core_media::staging::StagingRoot;
use scene_core_media::toolchain::Toolchain;
use scene_core_protocol::{
    Artifact, ArtifactKind, ArtifactRole, AudioPcmInfo, AudioPcmOptions, ErrorCode, Identifier,
    MAX_AUDIO_CHANNELS, MAX_AUDIO_SAMPLE_RATE, MediaStream, MediaType, NormalizedMedia,
    RelativeRef, StreamKind,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaFailure {
    ToolUnavailable,
    ToolFailed,
    Timeout,
    Cancelled,
    UnsupportedInput,
    CorruptMedia,
    MissingVideoStream,
    MissingAudioStream,
    ResourceLimit,
    StagingViolation,
    Internal,
}

impl MediaFailure {
    pub const fn code(&self) -> ErrorCode {
        match self {
            MediaFailure::ToolUnavailable => ErrorCode::ToolUnavailable,
            MediaFailure::ToolFailed => ErrorCode::ToolFailed,
            MediaFailure::Timeout => ErrorCode::Timeout,
            MediaFailure::Cancelled => ErrorCode::Cancelled,
            MediaFailure::UnsupportedInput => ErrorCode::UnsupportedInput,
            MediaFailure::CorruptMedia => ErrorCode::CorruptMedia,
            MediaFailure::MissingVideoStream => ErrorCode::MissingVideoStream,
            MediaFailure::MissingAudioStream => ErrorCode::MissingAudioStream,
            MediaFailure::ResourceLimit => ErrorCode::ResourceLimit,
            MediaFailure::StagingViolation => ErrorCode::InvalidRequest,
            MediaFailure::Internal => ErrorCode::EngineInternal,
        }
    }
}

/// Result of one preview extraction.
pub struct PreviewOutcome {
    pub media: NormalizedMedia,
    pub artifacts: Vec<Artifact>,
}

/// Result of one PCM audio extraction.
pub struct AudioPcmOutcome {
    pub media: NormalizedMedia,
    pub artifacts: Vec<Artifact>,
}

/// Media operations used by the state machine. Tests inject fake backends.
pub trait MediaBackend {
    fn probe(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
    ) -> Result<NormalizedMedia, MediaFailure>;

    fn extract_preview(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
    ) -> Result<PreviewOutcome, MediaFailure>;

    fn extract_audio_pcm(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
        options: Option<&AudioPcmOptions>,
    ) -> Result<AudioPcmOutcome, MediaFailure>;
}

/// Backend used when the fixed toolchain is unavailable; the session still
/// emits `accepted` and then a controlled `TOOL_UNAVAILABLE` failure.
pub struct UnavailableBackend;

impl MediaBackend for UnavailableBackend {
    fn probe(
        &self,
        _staging: &StagingRoot,
        _control: &RunControl,
    ) -> Result<NormalizedMedia, MediaFailure> {
        Err(MediaFailure::ToolUnavailable)
    }

    fn extract_preview(
        &self,
        _staging: &StagingRoot,
        _control: &RunControl,
    ) -> Result<PreviewOutcome, MediaFailure> {
        Err(MediaFailure::ToolUnavailable)
    }

    fn extract_audio_pcm(
        &self,
        _staging: &StagingRoot,
        _control: &RunControl,
        _options: Option<&AudioPcmOptions>,
    ) -> Result<AudioPcmOutcome, MediaFailure> {
        Err(MediaFailure::ToolUnavailable)
    }
}

/// Real backend over the locked toolchain.
pub struct FfmpegBackend {
    pub toolchain: Toolchain,
}

impl MediaBackend for FfmpegBackend {
    fn probe(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
    ) -> Result<NormalizedMedia, MediaFailure> {
        probe_with_cancel(
            &self.toolchain,
            &staging.input(),
            Some(control.process_flag()),
        )
        .map_err(|error| classify_probe_error(error, control))
    }

    fn extract_preview(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
    ) -> Result<PreviewOutcome, MediaFailure> {
        let input = staging.input();
        let probed = probe_with_cancel(&self.toolchain, &input, Some(control.process_flag()))
            .map_err(|error| classify_probe_error(error, control))?;
        let Some(primary_video_index) = probed.primary_video_stream_index else {
            return Err(MediaFailure::MissingVideoStream);
        };
        let mut requests: Vec<(u64, ArtifactRole, &str, &str)> = vec![(
            0,
            ArtifactRole::Opening,
            "preview-opening",
            "output/preview/opening.jpg",
        )];
        // The midpoint must stay inside the primary video track: a seek past
        // its last frame produces no file (or an encoder error) even though
        // the container keeps running for audio.
        let video_duration_ms = probed
            .streams
            .iter()
            .find(|stream| stream.index == primary_video_index)
            .and_then(|stream| stream.duration_ms);
        let midpoint_bound = match (probed.container.duration_ms, video_duration_ms) {
            (Some(container), Some(video)) => Some(container.min(video)),
            (Some(container), None) => Some(container),
            (None, Some(video)) => Some(video),
            (None, None) => None,
        };
        if let Some(duration) = midpoint_bound {
            let midpoint = duration / 2;
            if midpoint > 0 {
                requests.push((
                    midpoint,
                    ArtifactRole::Midpoint,
                    "preview-midpoint",
                    "output/preview/midpoint.jpg",
                ));
            }
        }
        let mut pending = Vec::new();
        for request in requests {
            match self.prepare_preview(staging, control, &input, primary_video_index, request) {
                Ok(Some(artifact)) => pending.push(artifact),
                Ok(None) => {}
                Err(error) => {
                    discard_pending(&pending)?;
                    return Err(error);
                }
            }
        }
        let artifacts = finalize_artifacts(pending)?;
        Ok(PreviewOutcome {
            media: probed,
            artifacts,
        })
    }

    fn extract_audio_pcm(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
        options: Option<&AudioPcmOptions>,
    ) -> Result<AudioPcmOutcome, MediaFailure> {
        let input = staging.input();
        let probed = probe_with_origin(&self.toolchain, &input, Some(control.process_flag()))
            .map_err(|error| classify_probe_error(error, control))?;
        // A material-time mapping needs the raw container origin and the
        // stream start; unknown values fail the operation instead of guessing.
        let Some(origin) = probed.origin else {
            return Err(MediaFailure::UnsupportedInput);
        };
        let origin_ms = origin
            .to_ms_round_nearest()
            .map_err(|_| MediaFailure::UnsupportedInput)?;
        let media = probed.media;
        let index = select_audio_stream(
            &media.streams,
            options.and_then(|options| options.audio_stream_index),
        )?;
        let stream = media
            .streams
            .iter()
            .find(|stream| stream.index == index)
            .expect("selected stream exists");
        let Some(stream_start_ms) = stream.start_time_ms else {
            return Err(MediaFailure::UnsupportedInput);
        };
        let trim = audio::AudioTrim {
            start_ms: options.and_then(|options| options.start_ms),
            end_ms: options.and_then(|options| options.end_ms),
            sample_rate: options.and_then(|options| options.sample_rate),
            channels: options.and_then(|options| options.channels),
        };
        let requested_start_ms = trim.start_ms.unwrap_or(0);
        let stream_end_ms = stream
            .duration_ms
            .map(|duration| {
                material_start_ms(stream_start_ms)
                    .unwrap_or(0)
                    .saturating_add(duration)
            })
            .or(media.container.duration_ms);
        // The output starts at the later of the requested and the stream start,
        // so the budget must cover that window, not the requested one.
        let actual_start_ms = requested_start_ms.max(material_start_ms(stream_start_ms)?);
        let window_end_ms = bounded_end_ms(requested_start_ms, trim.end_ms, stream_end_ms);
        let (budget_rate, budget_channels) = budget_parameters(
            trim.sample_rate,
            stream.sample_rate,
            trim.channels,
            stream.channels,
        );
        check_output_budget(window_end_ms, actual_start_ms, budget_rate, budget_channels)?;

        let reference =
            RelativeRef::new("output/audio/track.wav").map_err(|_| MediaFailure::Internal)?;
        let path = staging
            .output(&reference)
            .map_err(|_| MediaFailure::StagingViolation)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| MediaFailure::ResourceLimit)?;
        }
        let temporary = TempPath::new(path.with_extension("tmp"));
        staging
            .contain(&temporary.0)
            .map_err(|_| MediaFailure::StagingViolation)?;
        let info = audio::extract(
            &self.toolchain,
            &input,
            index,
            origin_ms,
            &trim,
            &temporary.0,
            Some(control.process_flag()),
        )
        .map_err(|error| classify_audio_error(error, control))?;

        if info.sample_count == 0 {
            return Err(MediaFailure::UnsupportedInput);
        }

        // Gross continuity guard: a window that decodes to a very different
        // length than the probed stream cannot carry a single mapping. It only
        // runs when the track end is known; a shorter track would otherwise
        // look like a mismatch against a requested end beyond it.
        if stream_end_ms.is_some() {
            // The decoded window must match the requested window (clamped to
            // the track end), not the whole remaining track.
            let window_end_ms = window_end_ms.expect("a known track end bounds the window");
            let expected = window_end_ms
                .max(actual_start_ms)
                .saturating_sub(actual_start_ms);
            let actual = info.duration_ms();
            let tolerance = (expected / 100).max(1_000);
            if actual.abs_diff(expected) > tolerance {
                return Err(MediaFailure::UnsupportedInput);
            }
        }

        let byte_size = std::fs::metadata(&temporary.0)
            .map_err(|_| MediaFailure::Internal)?
            .len();
        let content_hash = hash_file(&temporary.0).map_err(|_| MediaFailure::Internal)?;
        let artifact = Artifact {
            artifact_id: Identifier::new("audio-pcm").map_err(|_| MediaFailure::Internal)?,
            kind: ArtifactKind::AudioPcm,
            role: ArtifactRole::Audio,
            media_type: MediaType::new("audio/wav").map_err(|_| MediaFailure::Internal)?,
            relative_ref: reference,
            byte_size,
            content_hash,
            requested_time_ms: requested_start_ms,
            presentation_time_ms: Some(actual_start_ms),
            audio_pcm: Some(AudioPcmInfo {
                sample_rate: info.sample_rate,
                channels: u32::from(info.channels),
                sample_count: info.sample_count,
            }),
            pixel_width: None,
            pixel_height: None,
        };
        // The final rename is the publish step: nothing can fail after it, and
        // every failure before it removes the temporary through `TempPath`.
        std::fs::rename(&temporary.0, &path).map_err(|_| MediaFailure::Internal)?;
        Ok(AudioPcmOutcome {
            media,
            artifacts: vec![artifact],
        })
    }
}

/// Material time of the first output sample: pre-roll before the container
/// origin is not part of the artifact, so negative stream starts clamp to 0.
fn material_start_ms(stream_start_ms: i64) -> Result<u64, MediaFailure> {
    u64::try_from(stream_start_ms.max(0)).map_err(|_| MediaFailure::Internal)
}

impl FfmpegBackend {
    /// Generates one preview frame into its temporary path and derives the
    /// artifact fields. `Ok(None)` skips a midpoint frame that valid media may
    /// not have; a temporary that does not become part of the result is always
    /// removed.
    fn prepare_preview(
        &self,
        staging: &StagingRoot,
        control: &RunControl,
        input: &Path,
        primary_video_index: u32,
        request: (u64, ArtifactRole, &str, &str),
    ) -> Result<Option<PendingArtifact>, MediaFailure> {
        let (requested_time_ms, role, artifact_id, relative) = request;
        let reference = RelativeRef::new(relative).map_err(|_| MediaFailure::Internal)?;
        let path = staging
            .output(&reference)
            .map_err(|_| MediaFailure::StagingViolation)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| MediaFailure::ResourceLimit)?;
        }
        let temporary = TempPath::new(path.with_extension("tmp"));
        staging
            .contain(&temporary.0)
            .map_err(|_| MediaFailure::StagingViolation)?;
        match preview::generate(
            &self.toolchain,
            input,
            requested_time_ms,
            primary_video_index,
            &temporary.0,
            Some(control.process_flag()),
        ) {
            Ok(()) => {}
            // A midpoint frame may be missing even for valid media; the
            // manifest allows an opening-only artifact set.
            Err(PreviewError::NoFrame | PreviewError::ToolFailed)
                if role == ArtifactRole::Midpoint =>
            {
                return Ok(None);
            }
            Err(error) => return Err(classify_preview_error(error, control)),
        }
        let bytes = std::fs::read(&temporary.0).map_err(|_| MediaFailure::Internal)?;
        let (width, height) = preview::jpeg_dimensions(&bytes).ok_or(MediaFailure::Internal)?;
        let byte_size = u64::try_from(bytes.len()).map_err(|_| MediaFailure::Internal)?;
        let content_hash = hash_file(&temporary.0).map_err(|_| MediaFailure::Internal)?;
        let artifact = Artifact {
            artifact_id: Identifier::new(artifact_id).map_err(|_| MediaFailure::Internal)?,
            kind: ArtifactKind::PreviewFrame,
            role,
            media_type: MediaType::new("image/jpeg").map_err(|_| MediaFailure::Internal)?,
            relative_ref: reference,
            byte_size,
            content_hash,
            requested_time_ms,
            presentation_time_ms: None,
            audio_pcm: None,
            pixel_width: Some(width),
            pixel_height: Some(height),
        };
        Ok(Some(PendingArtifact {
            temporary: temporary.keep(),
            final_path: path,
            artifact,
        }))
    }
}

/// One artifact prepared in its temporary path, waiting for the batch rename
/// that publishes the whole set at once.
struct PendingArtifact {
    temporary: PathBuf,
    final_path: PathBuf,
    artifact: Artifact,
}

/// Removes the temporary files of prepared artifacts when the set is
/// abandoned before finalization.
fn discard_pending(pending: &[PendingArtifact]) -> Result<(), MediaFailure> {
    let mut cleanup_failed = false;
    for item in pending {
        cleanup_failed |= remove_file_if_exists(&item.temporary).is_err();
    }
    if cleanup_failed {
        return Err(MediaFailure::Internal);
    }
    Ok(())
}

/// Publishes every prepared artifact by renaming it to its final path. On the
/// first failure the already-published finals are rolled back and the
/// remaining temporaries are removed, so a failed operation leaves nothing.
fn finalize_artifacts(pending: Vec<PendingArtifact>) -> Result<Vec<Artifact>, MediaFailure> {
    for (index, item) in pending.iter().enumerate() {
        if std::fs::rename(&item.temporary, &item.final_path).is_err() {
            let mut cleanup_failed = false;
            for published in &pending[..index] {
                cleanup_failed |= remove_file_if_exists(&published.final_path).is_err();
            }
            for remaining in &pending[index..] {
                cleanup_failed |= remove_file_if_exists(&remaining.temporary).is_err();
            }
            if cleanup_failed {
                eprintln!("scene-core: failed to roll back preview artifacts");
            }
            return Err(MediaFailure::Internal);
        }
    }
    Ok(pending.into_iter().map(|item| item.artifact).collect())
}

/// Temporary output path whose file is removed on drop, unless
/// [`TempPath::keep`] transfers ownership to the caller.
struct TempPath(PathBuf);

impl TempPath {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn keep(mut self) -> PathBuf {
        std::mem::take(&mut self.0)
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() && remove_file_if_exists(&self.0).is_err() {
            eprintln!("scene-core: failed to remove a temporary artifact");
        }
    }
}

/// Lowest non-attached audio stream, or the explicitly requested index.
fn select_audio_stream(
    streams: &[MediaStream],
    requested: Option<u32>,
) -> Result<u32, MediaFailure> {
    let is_audio =
        |stream: &MediaStream| stream.kind == StreamKind::Audio && !stream.attached_picture;
    match requested {
        Some(index) => streams
            .iter()
            .find(|stream| stream.index == index && is_audio(stream))
            .map(|stream| stream.index)
            .ok_or(MediaFailure::MissingAudioStream),
        None => streams
            .iter()
            .filter(|stream| is_audio(stream))
            .map(|stream| stream.index)
            .min()
            .ok_or(MediaFailure::MissingAudioStream),
    }
}

/// End of the requested window clamped to the probed track end, when either
/// bound is known. `None` means neither the request nor the probe bounds the
/// window, so the extraction relies on the tool timeout and the Host disk
/// quota (a full disk fails the tool and maps to `RESOURCE_LIMIT`).
fn bounded_end_ms(
    requested_start_ms: u64,
    requested_end_ms: Option<u64>,
    stream_end_ms: Option<u64>,
) -> Option<u64> {
    let stream_bound = stream_end_ms.map(|end| end.max(requested_start_ms));
    match (requested_end_ms, stream_bound) {
        (Some(requested), Some(stream)) => Some(requested.min(stream)),
        (Some(requested), None) => Some(requested),
        (None, Some(stream)) => Some(stream),
        (None, None) => None,
    }
}

/// Rejects a window whose decoded PCM would exceed the output budget before
/// any tool starts. One known bound (request end or track end) is enough; when
/// neither bound is known the tool timeout and the Host disk quota are the
/// backstop.
fn check_output_budget(
    window_end_ms: Option<u64>,
    output_start_ms: u64,
    sample_rate: u32,
    channels: u16,
) -> Result<(), MediaFailure> {
    let Some(end) = window_end_ms else {
        return Ok(());
    };
    let window_ms = end.saturating_sub(output_start_ms);
    if audio::estimate_output_bytes(window_ms, sample_rate, channels) > audio::MAX_OUTPUT_BYTES {
        return Err(MediaFailure::ResourceLimit);
    }
    Ok(())
}

/// Output parameters used for the budget estimate: requested values win, then
/// probed values, then the protocol maxima. The maxima are a deliberate
/// fail-closed default: when the probe cannot report an output shape (or
/// reports zeros), a bounded window must not estimate as zero bytes.
fn budget_parameters(
    requested_rate: Option<u32>,
    source_rate: Option<u32>,
    requested_channels: Option<u16>,
    source_channels: Option<u32>,
) -> (u32, u16) {
    let rate = requested_rate
        .or(source_rate)
        .filter(|rate| *rate > 0)
        .unwrap_or(MAX_AUDIO_SAMPLE_RATE);
    let channels = requested_channels
        .or_else(|| source_channels.and_then(|value| u16::try_from(value).ok()))
        .filter(|channels| *channels > 0)
        .unwrap_or(MAX_AUDIO_CHANNELS);
    (rate, channels)
}

fn classify_audio_error(error: AudioError, control: &RunControl) -> MediaFailure {
    match error {
        AudioError::ToolUnavailable => MediaFailure::ToolUnavailable,
        AudioError::ToolFailed => MediaFailure::ToolFailed,
        AudioError::Timeout => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::ResourceLimit
            }
        }
        AudioError::Cancelled => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::Cancelled
            }
        }
        AudioError::ResourceLimit => MediaFailure::ResourceLimit,
        AudioError::EngineInternal(_) => MediaFailure::Internal,
    }
}

fn classify_preview_error(error: PreviewError, control: &RunControl) -> MediaFailure {
    match error {
        PreviewError::ToolUnavailable => MediaFailure::ToolUnavailable,
        PreviewError::ToolFailed | PreviewError::NoFrame => MediaFailure::ToolFailed,
        // The tool's own 120 s cap is a resource guard, not the request
        // deadline; only report TIMEOUT when the deadline actually fired.
        PreviewError::Timeout => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::ResourceLimit
            }
        }
        PreviewError::Cancelled => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::Cancelled
            }
        }
        PreviewError::NoVideoStream => MediaFailure::MissingVideoStream,
        PreviewError::ResourceLimit => MediaFailure::ResourceLimit,
        PreviewError::EngineInternal(_) => MediaFailure::Internal,
    }
}

fn classify_probe_error(error: ProbeError, control: &RunControl) -> MediaFailure {
    match error {
        ProbeError::ToolUnavailable => MediaFailure::ToolUnavailable,
        ProbeError::ToolFailed => MediaFailure::ToolFailed,
        ProbeError::Timeout => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::ResourceLimit
            }
        }
        ProbeError::Cancelled => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::Cancelled
            }
        }
        ProbeError::CorruptMedia => MediaFailure::CorruptMedia,
        ProbeError::UnsupportedInput => MediaFailure::UnsupportedInput,
        ProbeError::ResourceLimit => MediaFailure::ResourceLimit,
        ProbeError::EngineInternal(_) => MediaFailure::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_material_start_clamps_pre_roll_to_zero() {
        assert_eq!(material_start_ms(-80).expect("negative start"), 0);
        assert_eq!(material_start_ms(0).expect("zero start"), 0);
        assert_eq!(material_start_ms(80).expect("positive start"), 80);
        assert_eq!(
            material_start_ms(i64::MAX).expect("large start"),
            i64::MAX as u64
        );
    }

    #[test]
    fn bounded_window_end_prefers_the_known_bound() {
        assert_eq!(bounded_end_ms(0, Some(3_600_000), None), Some(3_600_000));
        assert_eq!(bounded_end_ms(400, Some(700), Some(60_000)), Some(700));
        assert_eq!(bounded_end_ms(400, None, Some(60_000)), Some(60_000));
        assert_eq!(
            bounded_end_ms(400, Some(700), Some(300)),
            Some(400),
            "the requested start clamps a track end that precedes it"
        );
        assert_eq!(bounded_end_ms(400, None, None), None);
        assert_eq!(check_output_budget(None, 0, 192_000, 2), Ok(()));
    }

    #[test]
    fn the_budget_uses_the_output_start_and_fails_closed_on_unknown_parameters() {
        // A track that starts at 10 min and runs 40 min: the output is 40 min
        // (~1.84 GB at 192 kHz stereo), under the 2 GiB budget. The old
        // requested-start basis estimated 50 min (~2.30 GB) and rejected it.
        assert_eq!(
            check_output_budget(Some(3_000_000), 600_000, 192_000, 2),
            Ok(()),
            "a delayed track must be budgeted from its actual output start"
        );
        assert_eq!(
            check_output_budget(Some(3_000_000), 0, 192_000, 2),
            Err(MediaFailure::ResourceLimit),
            "the same window from the requested start is over budget"
        );

        // 5 h at the protocol maxima is ~13.8 GB: unknown output parameters
        // must not estimate as zero bytes.
        assert_eq!(
            budget_parameters(None, None, None, None),
            (MAX_AUDIO_SAMPLE_RATE, MAX_AUDIO_CHANNELS)
        );
        assert_eq!(
            budget_parameters(None, Some(0), None, Some(0)),
            (MAX_AUDIO_SAMPLE_RATE, MAX_AUDIO_CHANNELS),
            "zero probe metadata is unknown, not a free pass"
        );
        assert_eq!(
            budget_parameters(None, Some(48_000), None, Some(6)),
            (48_000, 6),
            "probed values are used when the request omits them"
        );
        let (rate, channels) = budget_parameters(None, None, None, None);
        assert_eq!(
            check_output_budget(Some(5 * 3_600_000), 0, rate, channels),
            Err(MediaFailure::ResourceLimit)
        );
    }

    fn pending_artifact(temporary: PathBuf, final_path: PathBuf) -> PendingArtifact {
        PendingArtifact {
            temporary,
            final_path,
            artifact: Artifact {
                artifact_id: Identifier::new("audio-pcm").expect("id"),
                kind: ArtifactKind::AudioPcm,
                role: ArtifactRole::Audio,
                media_type: MediaType::new("audio/wav").expect("media type"),
                relative_ref: RelativeRef::new("output/audio/track.wav").expect("ref"),
                byte_size: 3,
                content_hash: scene_core_protocol::Sha256Digest::from_bytes(b"x"),
                requested_time_ms: 0,
                presentation_time_ms: Some(0),
                audio_pcm: None,
                pixel_width: None,
                pixel_height: None,
            },
        }
    }

    #[test]
    fn finalize_publishes_the_whole_set_or_rolls_back() {
        let dir = std::env::temp_dir().join(format!("scene-core-finalize-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");

        let first_temporary = dir.join("first.tmp");
        let first_final = dir.join("first.wav");
        let second_temporary = dir.join("second.tmp");
        let second_final = dir.join("second.wav");
        std::fs::write(&first_temporary, b"one").expect("temp");
        std::fs::write(&second_temporary, b"two").expect("temp");
        let artifacts = finalize_artifacts(vec![
            pending_artifact(first_temporary.clone(), first_final.clone()),
            pending_artifact(second_temporary.clone(), second_final.clone()),
        ])
        .expect("finalize");
        assert_eq!(artifacts.len(), 2);
        assert!(first_final.is_file() && second_final.is_file());
        assert!(!first_temporary.exists() && !second_temporary.exists());

        // A blocked final path must roll the whole set back.
        let blocked_temporary = dir.join("blocked.tmp");
        let blocked_final = dir.join("blocked.wav");
        let blocked_sibling_temporary = dir.join("sibling.tmp");
        let blocked_sibling_final = dir.join("sibling.wav");
        std::fs::write(&blocked_temporary, b"one").expect("temp");
        std::fs::write(&blocked_sibling_temporary, b"two").expect("temp");
        std::fs::create_dir(&blocked_sibling_final).expect("blocked final path");
        let result = finalize_artifacts(vec![
            pending_artifact(blocked_temporary.clone(), blocked_final.clone()),
            pending_artifact(
                blocked_sibling_temporary.clone(),
                blocked_sibling_final.clone(),
            ),
        ]);
        assert_eq!(result, Err(MediaFailure::Internal));
        assert!(
            !blocked_final.exists(),
            "a published artifact must be rolled back"
        );
        assert!(!blocked_temporary.exists() && !blocked_sibling_temporary.exists());
        assert!(blocked_sibling_final.is_dir());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_temporary_is_removed_unless_kept() {
        let dir = std::env::temp_dir().join(format!("scene-core-temp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");

        let dropped = dir.join("dropped.tmp");
        std::fs::write(&dropped, b"x").expect("temp");
        drop(TempPath::new(dropped.clone()));
        assert!(!dropped.exists(), "a dropped temporary must be removed");

        let kept = dir.join("kept.tmp");
        std::fs::write(&kept, b"x").expect("temp");
        let path = TempPath::new(kept.clone()).keep();
        assert_eq!(path, kept);
        assert!(kept.exists(), "a kept temporary survives until rename");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tool_timeouts_only_report_deadline_when_the_deadline_fired() {
        let control = RunControl::new();
        assert_eq!(
            classify_probe_error(ProbeError::Timeout, &control),
            MediaFailure::ResourceLimit
        );
        assert_eq!(
            classify_preview_error(PreviewError::Timeout, &control),
            MediaFailure::ResourceLimit
        );

        control.request_timeout();
        assert_eq!(
            classify_probe_error(ProbeError::Timeout, &control),
            MediaFailure::Timeout
        );
        assert_eq!(
            classify_preview_error(PreviewError::Timeout, &control),
            MediaFailure::Timeout
        );
    }
}

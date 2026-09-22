//! Request validation, the event state machine and timestamp formatting.

use super::backend::{MediaBackend, MediaFailure};
use super::control::RunControl;
use super::remove_file_if_exists;
use scene_core_media::hash::hash_file_cancellable;
use scene_core_media::staging::StagingRoot;
use scene_core_protocol::{
    Artifact, ArtifactManifest, DerivationIdentity, EngineIdentity, ErrorCode, EventEnvelope,
    EventMessageType, EventType, ExecutionContext, ExtractAudioPcmResult, ExtractPreviewResult,
    Identifier, Operation, OperationResult, ProbeResult, ProtocolError, ProtocolVersion,
    ResourceUsage, Sha256Digest, StageName, StagedSourceFacts, StartRequest, UtcTimestamp,
    verify_staged_source,
};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Identity fields echoed unchanged by every event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Echo {
    pub request_id: Identifier,
    pub source_version_id: Identifier,
    pub input_fingerprint: Sha256Digest,
    pub derivation_key: Sha256Digest,
    pub operation: Operation,
    pub execution_context: ExecutionContext,
}

impl Echo {
    pub fn from_request(request: &StartRequest) -> Self {
        Self {
            request_id: request.request_id.clone(),
            source_version_id: request.source_version_id.clone(),
            input_fingerprint: request.input_fingerprint.clone(),
            derivation_key: request.derivation_key.clone(),
            operation: request.operation,
            execution_context: request.execution_context.clone(),
        }
    }
}

#[derive(Debug)]
pub struct VerifiedInput {
    pub staging: StagingRoot,
}

/// Validates the request, its derivation key, the staging layout and the
/// staged snapshot. Returns the verified input path.
pub fn validate_request(
    request: &StartRequest,
    engine_identity: &EngineIdentity,
    toolchain_fingerprint: &Sha256Digest,
    staging_root: &Path,
    control: &RunControl,
) -> Result<VerifiedInput, Box<ProtocolError>> {
    request.validate()?;
    request.validate_derivation(&DerivationIdentity {
        engine_cache_compatibility_id: engine_identity.engine_cache_compatibility_id.clone(),
        toolchain_fingerprint: toolchain_fingerprint.clone(),
    })?;
    if !crate::identity::implemented_operations().contains(&request.operation) {
        return Err(Box::new(
            ProtocolError::new(
                ErrorCode::OperationUnavailable,
                "the requested operation is not implemented by this build",
            )
            .with_stage("validation")
            .with_next_step("use a supported operation or a newer engine build."),
        ));
    }
    let staging = StagingRoot::new(staging_root).map_err(|_| {
        ProtocolError::new(ErrorCode::InputNotFound, "the staging root is unavailable")
            .with_stage("staging")
    })?;
    let input = staging.require_input().map_err(|_| {
        Box::new(
            ProtocolError::new(
                ErrorCode::InputNotFound,
                "the staged input/source.media is missing",
            )
            .with_stage("staging")
            .with_next_step("stage the input snapshot again and retry."),
        )
    })?;
    let declared = request.inputs.first().ok_or_else(|| {
        Box::new(ProtocolError::invalid_request(
            "inputs must contain source_media",
        ))
    })?;
    let byte_size = std::fs::metadata(&input)
        .map_err(|_| {
            Box::new(
                ProtocolError::new(ErrorCode::InputNotFound, "the staged input is unreadable")
                    .with_stage("staging"),
            )
        })?
        .len();
    let content_hash = match hash_file_cancellable(&input, Some(control.cancelled_flag())) {
        Ok(Some(digest)) => digest,
        Ok(None) => return Err(Box::new(cancelled_error())),
        Err(_) => {
            return Err(Box::new(
                ProtocolError::new(ErrorCode::InputNotFound, "the staged input is unreadable")
                    .with_stage("staging"),
            ));
        }
    };
    let facts = StagedSourceFacts {
        byte_size,
        content_hash,
    };
    verify_staged_source(declared, Some(&facts))?;
    Ok(VerifiedInput { staging })
}

/// Runs the state machine and emits events in order. Returns the process exit
/// code. `accepted` is sequence 1, `progress` 2 (for `probe`) and the terminal
/// event is the last sequence.
pub fn run_session(
    request: &StartRequest,
    engine: &EngineIdentity,
    toolchain_fingerprint: &Sha256Digest,
    backend: &dyn MediaBackend,
    staging: &StagingRoot,
    control: &RunControl,
    emit: &mut dyn FnMut(EventEnvelope),
) -> u8 {
    let echo = Echo::from_request(request);
    emit(event(
        &echo,
        engine,
        1,
        EventType::Accepted,
        None,
        None,
        None,
        None,
    ));

    if let Some(violation) = control.violation() {
        return terminal_failure(&echo, engine, 2, EventType::Failed, violation, emit);
    }
    if control.is_timed_out() {
        return terminal_failure(&echo, engine, 2, EventType::TimedOut, timeout_error(), emit);
    }
    if control.is_cancelled() {
        return terminal_failure(
            &echo,
            engine,
            2,
            EventType::Cancelled,
            cancelled_error(),
            emit,
        );
    }

    let started = Instant::now();
    let (stage, total) = match request.operation {
        Operation::Probe => ("probe", 1),
        Operation::ExtractPreview => ("preview", 2),
        Operation::ExtractAudioPcm => ("audio", 1),
    };
    emit(event(
        &echo,
        engine,
        2,
        EventType::Progress,
        Some(StageName::new(stage).expect("stage")),
        Some(0),
        Some(total),
        None,
    ));

    let outcome = match request.operation {
        Operation::Probe => backend.probe(staging, control).map(|media| {
            OperationResult::Probe(ProbeResult {
                media,
                resource_usage: ResourceUsage {
                    wall_time_ms: 0,
                    cpu_time_ms: None,
                    peak_memory_bytes: None,
                },
            })
        }),
        Operation::ExtractPreview => {
            backend
                .extract_preview(staging, control)
                .and_then(|outcome| {
                    let manifest = ArtifactManifest {
                        manifest_version: ProtocolVersion::current(),
                        request_id: echo.request_id.clone(),
                        source_version_id: echo.source_version_id.clone(),
                        input_fingerprint: echo.input_fingerprint.clone(),
                        derivation_key: echo.derivation_key.clone(),
                        operation: Operation::ExtractPreview,
                        operation_config_hash: request.operation_config_hash.clone(),
                        output_contract_version: request.output_contract_version.clone(),
                        engine: engine.clone(),
                        toolchain_fingerprint: toolchain_fingerprint.clone(),
                        artifacts: outcome.artifacts,
                    };
                    if manifest.validate().is_err() {
                        discard_artifacts(staging, &manifest.artifacts)?;
                        return Err(MediaFailure::Internal);
                    }
                    Ok(OperationResult::ExtractPreview(Box::new(
                        ExtractPreviewResult {
                            media: outcome.media,
                            artifact_manifest: manifest,
                            resource_usage: ResourceUsage {
                                wall_time_ms: 0,
                                cpu_time_ms: None,
                                peak_memory_bytes: None,
                            },
                        },
                    )))
                })
        }
        Operation::ExtractAudioPcm => backend
            .extract_audio_pcm(staging, control, request.options.audio_pcm())
            .and_then(|outcome| {
                let manifest = ArtifactManifest {
                    manifest_version: ProtocolVersion::current(),
                    request_id: echo.request_id.clone(),
                    source_version_id: echo.source_version_id.clone(),
                    input_fingerprint: echo.input_fingerprint.clone(),
                    derivation_key: echo.derivation_key.clone(),
                    operation: Operation::ExtractAudioPcm,
                    operation_config_hash: request.operation_config_hash.clone(),
                    output_contract_version: request.output_contract_version.clone(),
                    engine: engine.clone(),
                    toolchain_fingerprint: toolchain_fingerprint.clone(),
                    artifacts: outcome.artifacts,
                };
                if manifest.validate().is_err() {
                    discard_artifacts(staging, &manifest.artifacts)?;
                    return Err(MediaFailure::Internal);
                }
                Ok(OperationResult::ExtractAudioPcm(Box::new(
                    ExtractAudioPcmResult {
                        media: outcome.media,
                        artifact_manifest: manifest,
                        resource_usage: ResourceUsage {
                            wall_time_ms: 0,
                            cpu_time_ms: None,
                            peak_memory_bytes: None,
                        },
                    },
                )))
            }),
    };
    let wall_time_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let late_terminal = control
        .violation()
        .map(|violation| (EventType::Failed, violation))
        .or_else(|| {
            control
                .is_timed_out()
                .then(|| (EventType::TimedOut, timeout_error()))
        });
    if let Some((event_type, error)) = late_terminal {
        if let Ok(result) = &outcome {
            if discard_result_artifacts(staging, result).is_err() {
                let error = failure_error(request.operation, &MediaFailure::Internal);
                return terminal_failure(&echo, engine, 3, EventType::Failed, error, emit);
            }
        }
        return terminal_failure(&echo, engine, 3, event_type, error, emit);
    }

    match outcome {
        Ok(mut result) => {
            if control.is_cancelled() {
                if discard_result_artifacts(staging, &result).is_err() {
                    let error = failure_error(request.operation, &MediaFailure::Internal);
                    return terminal_failure(&echo, engine, 3, EventType::Failed, error, emit);
                }
                return terminal_failure(
                    &echo,
                    engine,
                    3,
                    EventType::Cancelled,
                    cancelled_error(),
                    emit,
                );
            }
            match &mut result {
                OperationResult::Probe(probe) => probe.resource_usage.wall_time_ms = wall_time_ms,
                OperationResult::ExtractPreview(preview) => {
                    preview.resource_usage.wall_time_ms = wall_time_ms
                }
                OperationResult::ExtractAudioPcm(audio) => {
                    audio.resource_usage.wall_time_ms = wall_time_ms
                }
            }
            emit(event(
                &echo,
                engine,
                3,
                EventType::Completed,
                None,
                None,
                None,
                Some(result),
            ));
            0
        }
        Err(MediaFailure::Timeout) => {
            terminal_failure(&echo, engine, 3, EventType::TimedOut, timeout_error(), emit)
        }
        Err(MediaFailure::Cancelled) => terminal_failure(
            &echo,
            engine,
            3,
            EventType::Cancelled,
            cancelled_error(),
            emit,
        ),
        Err(failure) => {
            let error = failure_error(request.operation, &failure);
            terminal_failure(&echo, engine, 3, EventType::Failed, error, emit)
        }
    }
}

/// Artifacts carried by a completed operation result.
fn result_artifacts(result: &OperationResult) -> &[Artifact] {
    match result {
        OperationResult::Probe(_) => &[],
        OperationResult::ExtractPreview(preview) => &preview.artifact_manifest.artifacts,
        OperationResult::ExtractAudioPcm(audio) => &audio.artifact_manifest.artifacts,
    }
}

/// Removes artifacts a completed operation had already finalized when the
/// terminal state turns out non-success (late cancel, timeout or protocol
/// violation): a failed run must not leave a published artifact set behind.
fn discard_result_artifacts(
    staging: &StagingRoot,
    result: &OperationResult,
) -> Result<(), MediaFailure> {
    discard_artifacts(staging, result_artifacts(result))
}

/// Removes a finalized artifact set. Cleanup must complete before a late
/// non-success terminal is emitted; otherwise the run reports an internal
/// failure instead of falsely claiming a clean cancellation or timeout.
fn discard_artifacts(staging: &StagingRoot, artifacts: &[Artifact]) -> Result<(), MediaFailure> {
    let mut cleanup_failed = false;
    for artifact in artifacts {
        if let Ok(path) = staging.output(&artifact.relative_ref) {
            cleanup_failed |= remove_file_if_exists(&path).is_err();
        } else {
            cleanup_failed = true;
        }
    }
    if cleanup_failed {
        return Err(MediaFailure::Internal);
    }
    Ok(())
}

fn failure_error(operation: Operation, failure: &MediaFailure) -> ProtocolError {
    let code = failure.code();
    let message = match failure {
        MediaFailure::ToolUnavailable => "the fixed media tool is unavailable",
        MediaFailure::ToolFailed => "the fixed media tool failed",
        MediaFailure::UnsupportedInput => "the media format is unsupported",
        MediaFailure::CorruptMedia => "the media is corrupt or cannot be parsed",
        MediaFailure::MissingVideoStream => "the media has no usable video stream",
        MediaFailure::MissingAudioStream => "the media has no usable audio stream",
        MediaFailure::ResourceLimit => "a resource limit was reached",
        MediaFailure::StagingViolation => "the staging root violates containment",
        MediaFailure::Timeout => "the request reached its deadline",
        MediaFailure::Cancelled => "the request was cancelled",
        MediaFailure::Internal => "an internal error occurred",
    };
    let stage = match operation {
        Operation::Probe => "probe",
        Operation::ExtractPreview => "preview",
        Operation::ExtractAudioPcm => "audio",
    };
    ProtocolError::new(code, message).with_stage(stage)
}

pub fn timeout_error() -> ProtocolError {
    ProtocolError::new(ErrorCode::Timeout, "the request reached its deadline")
        .with_stage("run")
        .with_next_step("retry with a larger deadlineMs if the input is large.")
}

pub fn cancelled_error() -> ProtocolError {
    ProtocolError::new(ErrorCode::Cancelled, "the host cancelled the request").with_stage("run")
}

pub fn invalid_request_error() -> ProtocolError {
    ProtocolError::invalid_request("the request is not valid for this engine")
        .with_stage("validation")
}

pub fn unsupported_protocol_error() -> ProtocolError {
    ProtocolError::unsupported_protocol("engineProtocolVersion is not supported by this engine")
        .with_stage("validation")
}

fn terminal_failure(
    echo: &Echo,
    engine: &EngineIdentity,
    sequence: u64,
    event_type: EventType,
    error: ProtocolError,
    emit: &mut dyn FnMut(EventEnvelope),
) -> u8 {
    let exit_code = error.code.exit_code();
    let mut envelope = event(echo, engine, sequence, event_type, None, None, None, None);
    envelope.error = Some(error);
    emit(envelope);
    exit_code
}

pub fn event_for(
    echo: &Echo,
    engine: &EngineIdentity,
    sequence: u64,
    event_type: EventType,
    stage: Option<StageName>,
    completed: Option<u64>,
    result: Option<OperationResult>,
) -> EventEnvelope {
    event(
        echo, engine, sequence, event_type, stage, completed, None, result,
    )
}

#[allow(clippy::too_many_arguments)]
fn event(
    echo: &Echo,
    engine: &EngineIdentity,
    sequence: u64,
    event_type: EventType,
    stage: Option<StageName>,
    completed: Option<u64>,
    total: Option<u64>,
    result: Option<OperationResult>,
) -> EventEnvelope {
    EventEnvelope {
        engine_protocol_version: ProtocolVersion::current(),
        message_type: EventMessageType::Event,
        request_id: echo.request_id.clone(),
        source_version_id: echo.source_version_id.clone(),
        input_fingerprint: echo.input_fingerprint.clone(),
        derivation_key: echo.derivation_key.clone(),
        operation: echo.operation,
        sequence,
        event_type,
        engine: engine.clone(),
        execution_context: echo.execution_context.clone(),
        occurred_at: now_utc(),
        stage,
        completed,
        total,
        result,
        error: None,
    }
}

/// Current UTC time as an RFC 3339 timestamp without external dependencies.
pub fn now_utc() -> UtcTimestamp {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = remainder / 3600;
    let minute = (remainder % 3600) / 60;
    let second = remainder % 60;
    UtcTimestamp::new(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
    .expect("generated timestamp is valid")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_cover_epoch_and_leap_day() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }
}

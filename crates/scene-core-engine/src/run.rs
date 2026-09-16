//! Host-facing `run` session: validation, event sequence, terminal state and
//! exit codes. Media work is injected through [`MediaBackend`].

use scene_core_media::hash::hash_file;
use scene_core_media::probe::{ProbeError, probe_with_cancel};
use scene_core_media::staging::StagingRoot;
use scene_core_media::toolchain::Toolchain;
use scene_core_protocol::{
    DerivationIdentity, EngineIdentity, ErrorCode, EventEnvelope, EventMessageType, EventType,
    ExecutionContext, Identifier, NormalizedMedia, Operation, OperationResult, ProbeResult,
    ProtocolError, ProtocolVersion, ResourceUsage, Sha256Digest, StageName, StagedSourceFacts,
    StartRequest, UtcTimestamp, verify_staged_source,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Shared cancellation and protocol-violation state for one run.
#[derive(Debug, Default, Clone)]
pub struct RunControl {
    cancelled: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
    violation: Arc<Mutex<Option<ProtocolError>>>,
}

impl RunControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn request_timeout(&self) {
        self.timed_out.store(true, Ordering::Relaxed);
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Records a protocol violation (for example a second start or a cancel
    /// with the wrong requestId) and stops the running operation.
    pub fn record_violation(&self, error: ProtocolError) {
        self.cancelled.store(true, Ordering::Relaxed);
        *self.violation.lock().expect("violation lock") = Some(error);
    }

    pub fn violation(&self) -> Option<ProtocolError> {
        self.violation.lock().expect("violation lock").clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_timed_out(&self) -> bool {
        self.timed_out.load(Ordering::Relaxed)
    }

    pub fn process_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaFailure {
    ToolUnavailable,
    ToolFailed,
    Timeout,
    Cancelled,
    UnsupportedInput,
    CorruptMedia,
    MissingVideoStream,
    ResourceLimit,
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
            MediaFailure::ResourceLimit => ErrorCode::ResourceLimit,
            MediaFailure::Internal => ErrorCode::EngineInternal,
        }
    }
}

/// Media operations used by the state machine. Tests inject fake backends.
pub trait MediaBackend {
    fn probe(&self, input: &Path, control: &RunControl) -> Result<NormalizedMedia, MediaFailure>;
}

/// Backend used when the fixed toolchain is unavailable; the session still
/// emits `accepted` and then a controlled `TOOL_UNAVAILABLE` failure.
pub struct UnavailableBackend;

impl MediaBackend for UnavailableBackend {
    fn probe(&self, _input: &Path, _control: &RunControl) -> Result<NormalizedMedia, MediaFailure> {
        Err(MediaFailure::ToolUnavailable)
    }
}

/// Real backend over the locked toolchain.
pub struct ProbeBackend {
    pub toolchain: Toolchain,
}

impl MediaBackend for ProbeBackend {
    fn probe(&self, input: &Path, control: &RunControl) -> Result<NormalizedMedia, MediaFailure> {
        probe_with_cancel(&self.toolchain, input, Some(control.process_flag()))
            .map_err(|error| classify_probe_error(error, control))
    }
}

fn classify_probe_error(error: ProbeError, control: &RunControl) -> MediaFailure {
    match error {
        ProbeError::ToolUnavailable => MediaFailure::ToolUnavailable,
        ProbeError::ToolFailed => MediaFailure::ToolFailed,
        ProbeError::Timeout => MediaFailure::Timeout,
        ProbeError::Cancelled => {
            if control.is_timed_out() {
                MediaFailure::Timeout
            } else {
                MediaFailure::Cancelled
            }
        }
        ProbeError::CorruptMedia => MediaFailure::CorruptMedia,
        ProbeError::UnsupportedInput => MediaFailure::UnsupportedInput,
        ProbeError::EngineInternal(_) => MediaFailure::Internal,
    }
}

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
    pub input: PathBuf,
}

/// Validates the request, its derivation key, the staging layout and the
/// staged snapshot. Returns the verified input path.
pub fn validate_request(
    request: &StartRequest,
    engine_identity: &EngineIdentity,
    toolchain_fingerprint: &Sha256Digest,
    staging_root: &Path,
) -> Result<VerifiedInput, Box<ProtocolError>> {
    request.validate()?;
    request.validate_derivation(&DerivationIdentity {
        engine_cache_compatibility_id: engine_identity.engine_cache_compatibility_id.clone(),
        toolchain_fingerprint: toolchain_fingerprint.clone(),
    })?;
    if request.operation != Operation::Probe {
        return Err(Box::new(
            ProtocolError::new(
                ErrorCode::OperationUnavailable,
                "the requested operation is not implemented by this build",
            )
            .with_stage("validation")
            .with_next_step("use 'probe' or a newer engine build."),
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
    let facts = StagedSourceFacts {
        byte_size: std::fs::metadata(&input).map(|m| m.len()).unwrap_or(0),
        content_hash: hash_file(&input).map_err(|_| {
            Box::new(
                ProtocolError::new(ErrorCode::InputNotFound, "the staged input is unreadable")
                    .with_stage("staging"),
            )
        })?,
    };
    verify_staged_source(declared, Some(&facts))?;
    Ok(VerifiedInput { input })
}

/// Runs the state machine and emits events in order. Returns the process exit
/// code. `accepted` is sequence 1, `progress` 2 (for `probe`) and the terminal
/// event is the last sequence.
pub fn run_session(
    request: &StartRequest,
    engine: &EngineIdentity,
    backend: &dyn MediaBackend,
    input: &Path,
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
    emit(event(
        &echo,
        engine,
        2,
        EventType::Progress,
        Some(StageName::new("probe").expect("stage")),
        Some(0),
        Some(1),
        None,
    ));

    let outcome = backend.probe(input, control);
    let wall_time_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    if let Some(violation) = control.violation() {
        return terminal_failure(&echo, engine, 3, EventType::Failed, violation, emit);
    }
    if control.is_timed_out() {
        return terminal_failure(&echo, engine, 3, EventType::TimedOut, timeout_error(), emit);
    }

    match outcome {
        Ok(media) => {
            if control.is_cancelled() {
                return terminal_failure(
                    &echo,
                    engine,
                    3,
                    EventType::Cancelled,
                    cancelled_error(),
                    emit,
                );
            }
            let result = OperationResult::Probe(ProbeResult {
                media,
                resource_usage: ResourceUsage {
                    wall_time_ms,
                    cpu_time_ms: None,
                    peak_memory_bytes: None,
                },
            });
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
            let error = failure_error(&failure);
            terminal_failure(&echo, engine, 3, EventType::Failed, error, emit)
        }
    }
}

fn failure_error(failure: &MediaFailure) -> ProtocolError {
    let code = failure.code();
    let message = match failure {
        MediaFailure::ToolUnavailable => "the fixed media tool is unavailable",
        MediaFailure::ToolFailed => "the fixed media tool failed",
        MediaFailure::UnsupportedInput => "the media format is unsupported",
        MediaFailure::CorruptMedia => "the media is corrupt or cannot be parsed",
        MediaFailure::MissingVideoStream => "the media has no usable video stream",
        MediaFailure::ResourceLimit => "a resource limit was reached",
        MediaFailure::Timeout => "the request reached its deadline",
        MediaFailure::Cancelled => "the request was cancelled",
        MediaFailure::Internal => "an internal error occurred",
    };
    ProtocolError::new(code, message).with_stage("probe")
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

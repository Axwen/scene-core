//! Run session contract: event sequence, terminal states and exit codes.

use scene_core_engine::identity::engine_identity;
use scene_core_engine::run::{
    ControlSession, MediaBackend, MediaFailure, PreviewOutcome, RunControl, run_session,
    validate_request,
};
use scene_core_media::staging::StagingRoot;
use scene_core_protocol::{
    Artifact, ArtifactKind, ArtifactRole, CacheCompatibilityId, ContainerInfo,
    DerivationDescriptor, DescriptorVersion, ErrorCode, EventEnvelope, EventType, ExecutionContext,
    ExecutionScope, Identifier, InputDescriptor, InputRef, InputRole, InputSetDescriptor,
    MediaType, NormalizedMedia, Operation, OperationResult, ProtocolError, RelativeRef,
    Sha256Digest, StartRequest,
};
use std::path::Path;
use std::process::{Command, Stdio};

fn digest(byte: u8) -> Sha256Digest {
    Sha256Digest::from_bytes(&[byte])
}

fn request(operation: Operation) -> StartRequest {
    let input = InputDescriptor {
        role: InputRole::new(InputRole::SOURCE_MEDIA).expect("role"),
        input_ref: InputRef::new("input/source.media").expect("ref"),
        content_hash: digest(1),
        byte_size: 5,
    };
    let fingerprint = InputSetDescriptor::from_inputs(vec![input.clone()])
        .input_fingerprint()
        .expect("fingerprint");
    let operation_config_hash = Sha256Digest::from_bytes(b"{}");
    let derivation = DerivationDescriptor {
        derivation_descriptor_version: DescriptorVersion::current(),
        input_fingerprint: fingerprint.clone(),
        operation,
        operation_config_hash: operation_config_hash.clone(),
        output_contract_version: operation.output_contract_version(),
        engine_cache_compatibility_id: CacheCompatibilityId::new("scene-core-output-v1")
            .expect("cache id"),
        toolchain_fingerprint: digest(2),
    };
    StartRequest {
        engine_protocol_version: scene_core_protocol::ProtocolVersion::current(),
        message_type: scene_core_protocol::StartMessageType::Start,
        request_id: Identifier::new("req_01").expect("id"),
        operation,
        source_version_id: Identifier::new("sourcev_01").expect("id"),
        inputs: vec![input],
        input_fingerprint: fingerprint,
        operation_config_hash,
        output_contract_version: operation.output_contract_version(),
        derivation_key: derivation.derive_key().expect("key"),
        execution_context: ExecutionContext {
            run_id: Identifier::new("run_01").expect("id"),
            generation: 0,
            attempt: 1,
            scope: ExecutionScope::Asset,
        },
        deadline_ms: None,
        options: scene_core_protocol::OperationOptions {},
    }
}

fn minimal_media() -> NormalizedMedia {
    NormalizedMedia {
        container: ContainerInfo {
            format_name: "matroska,webm".to_owned(),
            duration_ms: Some(200),
            start_time_ms: Some(0),
            bit_rate_bps: None,
            file_size_bytes: 5,
        },
        streams: Vec::new(),
        primary_video_stream_index: None,
    }
}

enum Mode {
    Success,
    Cancel,
    Timeout,
    Violation,
    ToolUnavailable,
}

struct FakeBackend {
    mode: Mode,
}

impl Copy for Mode {}
impl Clone for Mode {
    fn clone(&self) -> Self {
        *self
    }
}

fn fake_artifact() -> Artifact {
    Artifact {
        artifact_id: Identifier::new("preview-opening").expect("id"),
        kind: ArtifactKind::PreviewFrame,
        role: ArtifactRole::Opening,
        media_type: MediaType::new("image/jpeg").expect("media type"),
        relative_ref: RelativeRef::new("output/preview/opening.jpg").expect("ref"),
        byte_size: 1234,
        content_hash: digest(3),
        requested_time_ms: 0,
        presentation_time_ms: None,
        pixel_width: Some(64),
        pixel_height: Some(48),
    }
}

impl MediaBackend for FakeBackend {
    fn extract_preview(
        &self,
        _staging: &StagingRoot,
        control: &RunControl,
    ) -> Result<PreviewOutcome, MediaFailure> {
        match self.mode {
            Mode::Cancel => {
                control.request_cancel();
                Err(MediaFailure::Cancelled)
            }
            Mode::Timeout => {
                control.request_timeout();
                Err(MediaFailure::Timeout)
            }
            Mode::ToolUnavailable => Err(MediaFailure::ToolUnavailable),
            Mode::Success | Mode::Violation => Ok(PreviewOutcome {
                media: minimal_media(),
                artifacts: vec![fake_artifact()],
            }),
        }
    }

    fn probe(
        &self,
        _staging: &StagingRoot,
        control: &RunControl,
    ) -> Result<NormalizedMedia, MediaFailure> {
        match self.mode {
            Mode::Success => Ok(minimal_media()),
            Mode::Cancel => {
                control.request_cancel();
                Err(MediaFailure::Cancelled)
            }
            Mode::Timeout => {
                control.request_timeout();
                Err(MediaFailure::Timeout)
            }
            Mode::Violation => {
                control.record_violation(ProtocolError::invalid_request("second start"));
                Ok(minimal_media())
            }
            Mode::ToolUnavailable => Err(MediaFailure::ToolUnavailable),
        }
    }
}

#[test]
fn extract_preview_reports_a_manifest() {
    let (events, exit_code) = session_for(Operation::ExtractPreview, Mode::Success);
    assert_eq!(exit_code, 0);
    assert_eq!(events.len(), 3);
    match events[2].result.as_ref().expect("result") {
        OperationResult::ExtractPreview(preview) => {
            assert_eq!(preview.artifact_manifest.artifacts.len(), 1);
            assert_eq!(
                preview.artifact_manifest.output_contract_version.as_str(),
                "extract-preview-result/1"
            );
        }
        other => panic!("unexpected result {other:?}"),
    }
}

fn session_for(operation: Operation, mode: Mode) -> (Vec<EventEnvelope>, u8) {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let request = request(operation);
    let engine = engine_identity();
    let control = RunControl::new();
    let backend = FakeBackend { mode };
    let staging_dir = std::env::temp_dir().join(format!(
        "scene-core-session-{}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        mode as u8
    ));
    std::fs::create_dir_all(&staging_dir).expect("staging");
    let staging = StagingRoot::new(&staging_dir).expect("staging root");
    let mut events = Vec::new();
    let exit_code = run_session(
        &request,
        &engine,
        &digest(2),
        &backend,
        &staging,
        &control,
        &mut |event| events.push(event),
    );
    let _ = std::fs::remove_dir_all(&staging_dir);
    (events, exit_code)
}

fn session(mode: Mode) -> (Vec<EventEnvelope>, u8) {
    session_for(Operation::Probe, mode)
}

#[test]
fn success_emits_accepted_progress_completed() {
    let (events, exit_code) = session(Mode::Success);
    assert_eq!(exit_code, 0);
    assert_eq!(
        events
            .iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>(),
        vec![
            EventType::Accepted,
            EventType::Progress,
            EventType::Completed
        ]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let request = request(Operation::Probe);
    for event in &events {
        assert_eq!(event.request_id, request.request_id);
        assert_eq!(event.source_version_id, request.source_version_id);
        assert_eq!(event.input_fingerprint, request.input_fingerprint);
        assert_eq!(event.derivation_key, request.derivation_key);
        assert_eq!(event.operation, Operation::Probe);
    }
    let completed = &events[2];
    assert!(completed.result.is_some());
    assert!(completed.error.is_none());
}

#[test]
fn cancel_and_timeout_have_their_own_terminals() {
    let (events, exit_code) = session(Mode::Cancel);
    assert_eq!(exit_code, 130);
    assert_eq!(
        events.last().expect("terminal").event_type,
        EventType::Cancelled
    );
    assert_eq!(
        events
            .last()
            .expect("terminal")
            .error
            .as_ref()
            .expect("error")
            .code,
        ErrorCode::Cancelled
    );

    let (events, exit_code) = session(Mode::Timeout);
    assert_eq!(exit_code, 124);
    assert_eq!(
        events.last().expect("terminal").event_type,
        EventType::TimedOut
    );
    assert_eq!(
        events
            .last()
            .expect("terminal")
            .error
            .as_ref()
            .expect("error")
            .code,
        ErrorCode::Timeout
    );
}

#[test]
fn violations_and_tool_failures_end_failed() {
    let (events, exit_code) = session(Mode::Violation);
    assert_eq!(exit_code, 2);
    assert_eq!(
        events.last().expect("terminal").event_type,
        EventType::Failed
    );
    assert!(events[2].result.is_none());

    let (events, exit_code) = session(Mode::ToolUnavailable);
    assert_eq!(exit_code, 2);
    assert_eq!(
        events.last().expect("terminal").event_type,
        EventType::Failed
    );
    assert_eq!(
        events
            .last()
            .expect("terminal")
            .error
            .as_ref()
            .expect("error")
            .code,
        ErrorCode::ToolUnavailable
    );

    let (events, exit_code) = session_for(Operation::ExtractPreview, Mode::ToolUnavailable);
    assert_eq!(exit_code, 2);
    let error = events
        .last()
        .and_then(|event| event.error.as_ref())
        .expect("preview error");
    assert_eq!(error.code, ErrorCode::ToolUnavailable);
    assert_eq!(error.stage.as_deref(), Some("preview"));
}

#[test]
fn validation_rejects_a_tampered_fingerprint() {
    let mut request = request(Operation::Probe);
    request.input_fingerprint = digest(9);
    let staging = std::env::temp_dir().join(format!("scene-core-run-{}", std::process::id()));
    std::fs::create_dir_all(&staging).expect("staging");
    let error = validate_request(
        &request,
        &engine_identity(),
        &digest(2),
        &staging,
        &RunControl::new(),
    )
    .expect_err("fingerprint mismatch");
    assert_eq!(error.code, ErrorCode::InvalidRequest);
    let _ = std::fs::remove_dir_all(&staging);
}

fn run_binary(
    args: &[&str],
    stdin_line: Option<&str>,
    envs: &[(&str, &str)],
) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_scene-core"));
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.env_remove("SCENE_CORE_FFMPEG_DIR");
    command.env_remove("SCENE_CORE_TOOLCHAIN_FINGERPRINT");
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn");
    if let Some(line) = stdin_line {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin");
        writeln!(stdin, "{line}").expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn invalid_request_with_identity_reports_failed_event() {
    let mut request = request(Operation::Probe);
    request.input_fingerprint = digest(9);
    let staging = std::env::temp_dir().join(format!("scene-core-run-cli-{}", std::process::id()));
    std::fs::create_dir_all(&staging).expect("staging");
    let line = serde_json::to_string(&request).expect("serialize");
    let fingerprint = digest(2).as_str().to_owned();
    let (exit_code, stdout, _) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&line),
        &[("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint)],
    );
    assert_eq!(exit_code, 2);
    let event: EventEnvelope = serde_json::from_str(stdout.trim()).expect("failed event JSON");
    assert_eq!(event.event_type, EventType::Failed);
    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.error.as_ref().expect("error").code,
        ErrorCode::InvalidRequest
    );
    let _ = std::fs::remove_dir_all(&staging);
}

#[test]
fn control_session_rejects_a_second_cancel_and_a_foreign_request_id() {
    let request = request(Operation::Probe);
    let cancel = serde_json::json!({
        "engineProtocolVersion": "0.1",
        "messageType": "cancel",
        "requestId": "req_01",
    })
    .to_string();

    let mut session = ControlSession::new(&request, RunControl::new());
    let control = session.control().clone();
    session.handle_line("");
    session.handle_line(&cancel);
    assert!(control.violation().is_none());
    assert!(control.is_cancelled());
    session.handle_line(&cancel);
    let violation = control.violation().expect("second cancel is a violation");
    assert_eq!(violation.code, ErrorCode::InvalidRequest);

    let foreign = serde_json::json!({
        "engineProtocolVersion": "0.1",
        "messageType": "cancel",
        "requestId": "req_other",
    })
    .to_string();
    let mut session = ControlSession::new(&request, RunControl::new());
    let control = session.control().clone();
    session.handle_line(&foreign);
    assert_eq!(
        control.violation().expect("foreign cancel").code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn identity_extras_still_report_a_failed_event() {
    let request = request(Operation::Probe);
    let staging =
        std::env::temp_dir().join(format!("scene-core-run-lenient-{}", std::process::id()));
    std::fs::create_dir_all(&staging).expect("staging");
    let mut value = serde_json::to_value(&request).expect("json");
    value
        .as_object_mut()
        .expect("object")
        .insert("unexpectedField".to_owned(), serde_json::json!(1));
    let line = serde_json::to_string(&value).expect("serialize");
    let fingerprint = digest(2).as_str().to_owned();
    let (exit_code, stdout, _) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&line),
        &[("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint)],
    );
    assert_eq!(exit_code, 2, "stdout: {stdout}");
    let event: EventEnvelope = serde_json::from_str(stdout.trim()).expect("failed event JSON");
    assert_eq!(event.event_type, EventType::Failed);
    assert_eq!(event.sequence, 1);
    assert_eq!(event.request_id.as_str(), "req_01");
    assert_eq!(event.source_version_id.as_str(), "sourcev_01");
    assert_eq!(
        event.error.as_ref().expect("error").code,
        ErrorCode::InvalidRequest
    );
    let _ = std::fs::remove_dir_all(&staging);
}

#[test]
fn oversized_first_line_is_rejected_without_buffering_it() {
    let staging = std::env::temp_dir().join(format!("scene-core-run-big-{}", std::process::id()));
    std::fs::create_dir_all(&staging).expect("staging");
    let line = format!("{{\"padding\":\"{}\"}}", "a".repeat(1024 * 1024 + 16));
    let (exit_code, stdout, stderr) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&line),
        &[],
    );
    assert_eq!(exit_code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("hard limit"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&staging);
}

#[test]
fn unparseable_first_line_keeps_stdout_empty() {
    let staging = std::env::temp_dir().join(format!("scene-core-run-bad-{}", std::process::id()));
    std::fs::create_dir_all(&staging).expect("staging");
    let (exit_code, stdout, stderr) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some("not json"),
        &[],
    );
    assert_eq!(exit_code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.is_empty());
    let _ = std::fs::remove_dir_all(&staging);
}

#[test]
fn probe_run_succeeds_against_staged_media() {
    let Ok(bin_dir) = std::env::var(scene_core_media::toolchain::ENV_FFMPEG_DIR) else {
        eprintln!("skipping: SCENE_CORE_FFMPEG_DIR is not set");
        return;
    };
    let toolchain =
        scene_core_media::toolchain::Toolchain::from_bin_dir(bin_dir).expect("toolchain");
    let staging = std::env::temp_dir().join(format!("scene-core-run-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("input")).expect("input dir");
    let input_path = staging.join("input/source.media");
    let mut args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=0.5:size=64x48:rate=10",
        "-c:v",
        "mjpeg",
        "-f",
        "matroska",
        "-y",
    ]
    .iter()
    .map(|flag| (*flag).to_owned())
    .collect();
    args.push(input_path.to_string_lossy().into_owned());
    let generated = scene_core_media::process::run(&scene_core_media::process::ProcessSpec::new(
        toolchain.ffmpeg(),
        args,
    ))
    .expect("spawn ffmpeg");
    assert!(generated.success, "media generation failed");

    let content_hash = scene_core_media::hash::hash_file(&input_path).expect("hash");
    let byte_size = std::fs::metadata(&input_path).expect("metadata").len();
    let input = InputDescriptor {
        role: InputRole::new(InputRole::SOURCE_MEDIA).expect("role"),
        input_ref: InputRef::new("input/source.media").expect("ref"),
        content_hash,
        byte_size,
    };
    let input_fingerprint = InputSetDescriptor::from_inputs(vec![input.clone()])
        .input_fingerprint()
        .expect("fingerprint");
    let operation = Operation::Probe;
    let operation_config_hash = Sha256Digest::from_bytes(b"{}");
    let toolchain_fingerprint = digest(2);
    let derivation = DerivationDescriptor {
        derivation_descriptor_version: DescriptorVersion::current(),
        input_fingerprint: input_fingerprint.clone(),
        operation,
        operation_config_hash: operation_config_hash.clone(),
        output_contract_version: operation.output_contract_version(),
        engine_cache_compatibility_id: CacheCompatibilityId::new("scene-core-output-v1")
            .expect("cache id"),
        toolchain_fingerprint: toolchain_fingerprint.clone(),
    };
    let request = StartRequest {
        engine_protocol_version: scene_core_protocol::ProtocolVersion::current(),
        message_type: scene_core_protocol::StartMessageType::Start,
        request_id: Identifier::new("req_live").expect("id"),
        operation,
        source_version_id: Identifier::new("sourcev_live").expect("id"),
        inputs: vec![input],
        input_fingerprint,
        operation_config_hash,
        output_contract_version: operation.output_contract_version(),
        derivation_key: derivation.derive_key().expect("key"),
        execution_context: ExecutionContext {
            run_id: Identifier::new("run_live").expect("id"),
            generation: 0,
            attempt: 1,
            scope: ExecutionScope::Asset,
        },
        deadline_ms: Some(60_000),
        options: scene_core_protocol::OperationOptions {},
    };

    let line = serde_json::to_string(&request).expect("serialize");
    let fingerprint_text = toolchain_fingerprint.as_str().to_owned();
    let (exit_code, stdout, stderr) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&line),
        &[
            (
                "SCENE_CORE_FFMPEG_DIR",
                &std::env::var(scene_core_media::toolchain::ENV_FFMPEG_DIR).expect("env"),
            ),
            ("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint_text),
        ],
    );
    assert_eq!(exit_code, 0, "stderr: {stderr}");
    let events: Vec<EventEnvelope> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("event JSON"))
        .collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, EventType::Accepted);
    assert_eq!(events[2].event_type, EventType::Completed);
    let mut validator = scene_core_protocol::EventStreamValidator::new();
    for event in &events {
        validator.accept(event).expect("valid transcript");
    }
    assert!(validator.is_complete());
    let _ = std::fs::remove_dir_all(&staging);
}

fn stage_media(bin_dir: &str, staging: &Path) {
    use scene_core_media::toolchain::Toolchain;
    let toolchain = Toolchain::from_bin_dir(bin_dir).expect("toolchain");
    std::fs::create_dir_all(staging.join("input")).expect("input dir");
    let media = staging.join("input/source.media");
    let args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=1:size=320x240:rate=10",
        "-c:v",
        "mjpeg",
        "-f",
        "matroska",
        "-y",
    ]
    .iter()
    .map(|flag| (*flag).to_owned())
    .chain([media.to_string_lossy().into_owned()])
    .collect();
    let result = scene_core_media::process::run(&scene_core_media::process::ProcessSpec::new(
        toolchain.ffmpeg(),
        args,
    ))
    .expect("spawn");
    assert!(result.success, "media generation failed");
}

fn live_request(operation: Operation, staging: &Path, deadline_ms: u64) -> StartRequest {
    let input_path = staging.join("input/source.media");
    let content_hash = scene_core_media::hash::hash_file(&input_path).expect("hash");
    let byte_size = std::fs::metadata(&input_path).expect("metadata").len();
    let input = InputDescriptor {
        role: InputRole::new(InputRole::SOURCE_MEDIA).expect("role"),
        input_ref: InputRef::new("input/source.media").expect("ref"),
        content_hash,
        byte_size,
    };
    let input_fingerprint = InputSetDescriptor::from_inputs(vec![input.clone()])
        .input_fingerprint()
        .expect("fingerprint");
    let operation_config_hash = Sha256Digest::from_bytes(b"{}");
    let derivation = DerivationDescriptor {
        derivation_descriptor_version: DescriptorVersion::current(),
        input_fingerprint: input_fingerprint.clone(),
        operation,
        operation_config_hash: operation_config_hash.clone(),
        output_contract_version: operation.output_contract_version(),
        engine_cache_compatibility_id: CacheCompatibilityId::new("scene-core-output-v1")
            .expect("cache id"),
        toolchain_fingerprint: digest(2),
    };
    StartRequest {
        engine_protocol_version: scene_core_protocol::ProtocolVersion::current(),
        message_type: scene_core_protocol::StartMessageType::Start,
        request_id: Identifier::new("req_fault").expect("id"),
        operation,
        source_version_id: Identifier::new("sourcev_fault").expect("id"),
        inputs: vec![input],
        input_fingerprint,
        operation_config_hash,
        output_contract_version: operation.output_contract_version(),
        derivation_key: derivation.derive_key().expect("key"),
        execution_context: ExecutionContext {
            run_id: Identifier::new("run_fault").expect("id"),
            generation: 0,
            attempt: 1,
            scope: ExecutionScope::Asset,
        },
        deadline_ms: Some(deadline_ms),
        options: scene_core_protocol::OperationOptions {},
    }
}

#[test]
fn deadline_reaps_the_tool_and_exits_124() {
    let Ok(bin_dir) = std::env::var(scene_core_media::toolchain::ENV_FFMPEG_DIR) else {
        eprintln!("skipping: SCENE_CORE_FFMPEG_DIR is not set");
        return;
    };
    let staging =
        std::env::temp_dir().join(format!("scene-core-fault-timeout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    stage_media(&bin_dir, &staging);
    let request = live_request(Operation::Probe, &staging, 1);
    let fingerprint = digest(2).as_str().to_owned();
    let (exit_code, stdout, _) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&serde_json::to_string(&request).expect("serialize")),
        &[
            ("SCENE_CORE_FFMPEG_DIR", &bin_dir),
            ("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint),
        ],
    );
    assert_eq!(exit_code, 124, "stdout: {stdout}");
    let events: Vec<EventEnvelope> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("event JSON"))
        .collect();
    assert_eq!(
        events.last().expect("terminal").event_type,
        EventType::TimedOut
    );
    assert!(!staging.join("output").exists());
    let _ = std::fs::remove_dir_all(&staging);
}

#[cfg(unix)]
#[test]
fn preview_temporary_symlink_cannot_escape_staging() {
    let Ok(bin_dir) = std::env::var(scene_core_media::toolchain::ENV_FFMPEG_DIR) else {
        eprintln!("skipping: SCENE_CORE_FFMPEG_DIR is not set");
        return;
    };
    let staging = std::env::temp_dir().join(format!("scene-core-fault-tmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    stage_media(&bin_dir, &staging);
    let outside =
        std::env::temp_dir().join(format!("scene-core-escape-{}.jpg", std::process::id()));
    let _ = std::fs::remove_file(&outside);
    std::fs::create_dir_all(staging.join("output/preview")).expect("output dir");
    std::os::unix::fs::symlink(&outside, staging.join("output/preview/opening.tmp"))
        .expect("symlink");

    let request = live_request(Operation::ExtractPreview, &staging, 60_000);
    let fingerprint = digest(2).as_str().to_owned();
    let (exit_code, stdout, stderr) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&serde_json::to_string(&request).expect("serialize")),
        &[
            ("SCENE_CORE_FFMPEG_DIR", &bin_dir),
            ("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint),
        ],
    );
    assert_ne!(exit_code, 0, "stdout: {stdout} stderr: {stderr}");
    assert!(
        !outside.exists(),
        "the preview was written outside staging: {}",
        outside.display()
    );
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_file(&outside);
}

#[cfg(unix)]
#[test]
fn unwritable_output_reports_resource_limit_without_artifacts() {
    let Ok(bin_dir) = std::env::var(scene_core_media::toolchain::ENV_FFMPEG_DIR) else {
        eprintln!("skipping: SCENE_CORE_FFMPEG_DIR is not set");
        return;
    };
    let staging =
        std::env::temp_dir().join(format!("scene-core-fault-disk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    stage_media(&bin_dir, &staging);
    let preview_dir = staging.join("output/preview");
    std::fs::create_dir_all(&preview_dir).expect("preview dir");
    let mut permissions = std::fs::metadata(&preview_dir)
        .expect("metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o555);
    std::fs::set_permissions(&preview_dir, permissions).expect("read-only");

    let request = live_request(Operation::ExtractPreview, &staging, 60_000);
    let fingerprint = digest(2).as_str().to_owned();
    let (exit_code, stdout, stderr) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&serde_json::to_string(&request).expect("serialize")),
        &[
            ("SCENE_CORE_FFMPEG_DIR", &bin_dir),
            ("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint),
        ],
    );
    assert_eq!(exit_code, 3, "stdout: {stdout} stderr: {stderr}");
    let events: Vec<EventEnvelope> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("event JSON"))
        .collect();
    let terminal = events.last().expect("terminal");
    assert_eq!(terminal.event_type, EventType::Failed);
    let error = terminal.error.as_ref().expect("error");
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(!staging.join("output/preview/opening.jpg").exists());

    let mut permissions = std::fs::metadata(&preview_dir)
        .expect("metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&preview_dir, permissions).expect("restore");
    let _ = std::fs::remove_dir_all(&staging);
}

#[test]
fn missing_toolchain_still_reports_accepted_then_failed() {
    let staging =
        std::env::temp_dir().join(format!("scene-core-fault-tool-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("input")).expect("input dir");
    let media = staging.join("input/source.media");
    std::fs::write(&media, b"staged bytes").expect("stage");
    let request = live_request(Operation::Probe, &staging, 60_000);
    let fingerprint = digest(2).as_str().to_owned();
    let empty_dir = staging.join("empty-bin");
    std::fs::create_dir_all(&empty_dir).expect("empty bin");
    let (exit_code, stdout, stderr) = run_binary(
        &["run", "--staging-root", staging.to_str().expect("utf-8")],
        Some(&serde_json::to_string(&request).expect("serialize")),
        &[
            ("SCENE_CORE_FFMPEG_DIR", empty_dir.to_str().expect("utf-8")),
            ("SCENE_CORE_TOOLCHAIN_FINGERPRINT", &fingerprint),
        ],
    );
    assert_eq!(exit_code, 2, "stdout: {stdout} stderr: {stderr}");
    let events: Vec<EventEnvelope> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("event JSON"))
        .collect();
    assert_eq!(events[0].event_type, EventType::Accepted);
    let terminal = events.last().expect("terminal");
    assert_eq!(terminal.event_type, EventType::Failed);
    assert_eq!(
        terminal.error.as_ref().expect("error").code,
        ErrorCode::ToolUnavailable
    );
    let _ = std::fs::remove_dir_all(&staging);
}

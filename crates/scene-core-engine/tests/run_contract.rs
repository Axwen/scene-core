//! Run session contract: event sequence, terminal states and exit codes.

use scene_core_engine::identity::engine_identity;
use scene_core_engine::run::{
    MediaBackend, MediaFailure, RunControl, run_session, validate_request,
};
use scene_core_protocol::{
    CacheCompatibilityId, ContainerInfo, DerivationDescriptor, DescriptorVersion, ErrorCode,
    EventEnvelope, EventType, ExecutionContext, ExecutionScope, Identifier, InputDescriptor,
    InputRef, InputRole, InputSetDescriptor, NormalizedMedia, Operation, ProtocolError,
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

impl MediaBackend for FakeBackend {
    fn probe(&self, _input: &Path, control: &RunControl) -> Result<NormalizedMedia, MediaFailure> {
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

fn session(mode: Mode) -> (Vec<EventEnvelope>, u8) {
    let request = request(Operation::Probe);
    let engine = engine_identity();
    let control = RunControl::new();
    let backend = FakeBackend { mode };
    let mut events = Vec::new();
    let exit_code = run_session(
        &request,
        &engine,
        &backend,
        Path::new("/nonexistent"),
        &control,
        &mut |event| events.push(event),
    );
    (events, exit_code)
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
}

#[test]
fn validation_rejects_a_tampered_fingerprint() {
    let mut request = request(Operation::Probe);
    request.input_fingerprint = digest(9);
    let staging = std::env::temp_dir().join(format!("scene-core-run-{}", std::process::id()));
    std::fs::create_dir_all(&staging).expect("staging");
    let error = validate_request(&request, &engine_identity(), &digest(2), &staging)
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

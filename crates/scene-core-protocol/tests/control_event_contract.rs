//! Control message, event transcript and Manifest contract tests.

mod common;

use common::{
    EMPTY_OPTIONS_HASH, TOOLCHAIN_FINGERPRINT, derivation_descriptor, derivation_identity,
    engine_identity, input_set, midpoint_artifact, minimal_media, preview_artifact, probe_result,
    sha, start_request,
};
use scene_core_protocol::{
    Artifact, ArtifactManifest, CancelMessageType, CancelRequest, ControlMessage, ErrorCode,
    EventEnvelope, EventMessageType, EventStreamValidator, EventType, ExecutionScope,
    ExtractPreviewResult, Identifier, Operation, OutputContractVersion, ProtocolError,
    ProtocolVersion, ResourceUsage, StageName, StartRequest, UtcTimestamp, parse_control_line,
};

fn probe_request() -> StartRequest {
    start_request("req_01", "sourcev_01")
}

fn request_value(request: &StartRequest) -> serde_json::Value {
    serde_json::to_value(request).expect("request serializes")
}

#[test]
fn start_request_roundtrips_strictly() {
    let request = probe_request();
    let line = serde_json::to_string(&request).expect("serializes");
    let parsed = StartRequest::parse_and_validate(&line).expect("parses and validates");
    assert_eq!(parsed, request);
    assert!(parsed.validate_derivation(&derivation_identity()).is_ok());

    let value = request_value(&request);
    assert_eq!(value["engineProtocolVersion"], "0.1");
    assert_eq!(value["messageType"], "start");
    assert_eq!(value["operation"], "probe");
    assert_eq!(value["executionContext"]["runId"], "run_01");
    assert_eq!(value["outputContractVersion"], "probe-result/1");
    assert_eq!(value["options"], serde_json::json!({}));
    assert!(value.get("deadlineMs").is_none());

    let control = parse_control_line(&line).expect("control line");
    assert_eq!(control.message_type(), "start");
    assert_eq!(control.request_id().as_str(), "req_01");
    assert!(matches!(control, ControlMessage::Start(_)));

    let cancel = CancelRequest {
        engine_protocol_version: ProtocolVersion::current(),
        message_type: CancelMessageType::Cancel,
        request_id: Identifier::new("req_01").expect("id"),
    };
    let cancel_line = serde_json::to_string(&cancel).expect("serializes");
    assert_eq!(
        CancelRequest::parse_and_validate(&cancel_line).expect("parses"),
        cancel
    );
    assert!(matches!(
        parse_control_line(&cancel_line).expect("control line"),
        ControlMessage::Cancel(_)
    ));
}

#[test]
fn missing_execution_context_is_rejected() {
    let mut value = request_value(&probe_request());
    value
        .as_object_mut()
        .expect("object")
        .remove("executionContext");
    let error = StartRequest::parse(&value.to_string()).expect_err("missing context");
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[test]
fn unknown_and_duplicate_fields_are_rejected() {
    let mut value = request_value(&probe_request());
    value["futureField"] = serde_json::json!(1);
    assert!(StartRequest::parse(&value.to_string()).is_err());

    let line = serde_json::to_string(&probe_request()).expect("serializes");
    let duplicate_field = line.replacen(
        "\"requestId\":\"req_01\"",
        "\"requestId\":\"req_01\",\"requestId\":\"req_02\"",
        1,
    );
    assert!(StartRequest::parse(&duplicate_field).is_err());

    let duplicate_nested = line.replacen(
        "\"runId\":\"run_01\"",
        "\"runId\":\"run_01\",\"runId\":\"run_02\"",
        1,
    );
    assert!(StartRequest::parse(&duplicate_nested).is_err());
}

#[test]
fn options_reject_arbitrary_and_duplicate_keys_at_parse_time() {
    let line = serde_json::to_string(&probe_request()).expect("serializes");
    let custom = line.replacen("\"options\":{}", "\"options\":{\"profile\":\"custom\"}", 1);
    assert_ne!(line, custom);
    assert!(StartRequest::parse(&custom).is_err());

    let duplicated = line.replacen("\"options\":{}", "\"options\":{\"a\":1,\"a\":2}", 1);
    assert!(StartRequest::parse(&duplicated).is_err());
}

#[test]
fn unsupported_protocol_version_is_unsupported_protocol() {
    let mut value = request_value(&probe_request());
    value["engineProtocolVersion"] = serde_json::json!("0.2");
    let line = value.to_string();
    let parsed = StartRequest::parse(&line).expect("well-formed version parses");
    let error = parsed.validate().expect_err("0.2 is unsupported");
    assert_eq!(error.code, ErrorCode::UnsupportedProtocol);

    let newer = ProtocolVersion::new("0.2").expect("version token");
    assert!(!newer.is_supported());
}

#[test]
fn deadline_bounds_are_enforced() {
    for invalid in [0_u64, 600_001] {
        let mut request = probe_request();
        request.deadline_ms = Some(invalid);
        let error = request.validate().expect_err("deadline out of range");
        assert_eq!(error.code, ErrorCode::InvalidRequest);
        assert_eq!(error.limit, Some(600_000));
        assert_eq!(error.actual, Some(invalid as i64));
    }
    for valid in [1_u64, 600_000] {
        let mut request = probe_request();
        request.deadline_ms = Some(valid);
        assert!(request.validate().is_ok());
        assert_eq!(request.effective_deadline_ms(), valid);
    }
}

#[test]
fn execution_context_and_contract_rules_are_enforced() {
    let mut request = probe_request();
    request.execution_context.attempt = 0;
    assert_eq!(
        request.validate().expect_err("attempt 0").code,
        ErrorCode::InvalidRequest
    );

    request.execution_context.attempt = 2;
    request.execution_context.scope = ExecutionScope::Evaluation;
    assert!(request.validate().is_ok());

    let mut request = probe_request();
    request.operation_config_hash = sha(TOOLCHAIN_FINGERPRINT);
    assert_eq!(
        request.validate().expect_err("config hash").code,
        ErrorCode::InvalidRequest
    );

    let mut request = probe_request();
    request.output_contract_version = Operation::ExtractPreview.output_contract_version();
    assert_eq!(
        request.validate().expect_err("contract").code,
        ErrorCode::InvalidRequest
    );
}

fn accepted_event() -> EventEnvelope {
    let request = probe_request();
    EventEnvelope {
        engine_protocol_version: ProtocolVersion::current(),
        message_type: EventMessageType::Event,
        request_id: request.request_id.clone(),
        source_version_id: request.source_version_id.clone(),
        input_fingerprint: request.input_fingerprint.clone(),
        derivation_key: request.derivation_key.clone(),
        operation: Operation::Probe,
        sequence: 1,
        event_type: EventType::Accepted,
        engine: engine_identity(),
        execution_context: request.execution_context.clone(),
        occurred_at: UtcTimestamp::new("2026-09-07T00:00:00Z").expect("timestamp"),
        stage: None,
        completed: None,
        total: None,
        result: None,
        error: None,
    }
}

fn progress_event(sequence: u64) -> EventEnvelope {
    let mut event = accepted_event();
    event.event_type = EventType::Progress;
    event.sequence = sequence;
    event.stage = Some(StageName::new("normalize").expect("stage"));
    event.completed = Some(1);
    event.total = Some(2);
    event
}

fn completed_probe_event(sequence: u64) -> EventEnvelope {
    let mut event = accepted_event();
    event.event_type = EventType::Completed;
    event.sequence = sequence;
    event.result = Some(scene_core_protocol::OperationResult::Probe(probe_result()));
    event
}

#[test]
fn accepted_event_rules_and_roundtrip() {
    let mut event = accepted_event();
    assert!(event.validate().is_ok());
    assert!(!event.is_terminal());
    assert_eq!(event.exit_code(), None);

    let line = serde_json::to_string(&event).expect("serializes");
    let parsed = EventEnvelope::parse(&line).expect("parses");
    assert_eq!(parsed, event);
    let value: serde_json::Value = serde_json::from_str(&line).expect("value");
    assert_eq!(value["messageType"], "event");
    assert_eq!(value["eventType"], "accepted");
    assert!(value.get("result").is_none());
    assert_eq!(value["engine"]["target"], "x86_64-pc-windows-msvc");
    assert_eq!(value["sequence"], 1);

    event.sequence = 2;
    assert!(event.validate().is_err());

    event.sequence = 1;
    event.stage = Some(StageName::new("probe").expect("stage"));
    assert!(event.validate().is_err());
}

#[test]
fn completed_event_requires_matching_result() {
    let event = completed_probe_event(2);
    assert!(event.validate().is_ok());
    assert!(event.is_terminal());
    assert_eq!(event.exit_code(), Some(0));

    let mut mismatched = event.clone();
    mismatched.operation = Operation::ExtractPreview;
    assert!(mismatched.validate().is_err());

    let mut with_error = event;
    with_error.error = Some(ProtocolError::input_changed("changed"));
    assert!(with_error.validate().is_err());
}

#[test]
fn extract_preview_completed_event_carries_manifest() {
    let request = probe_request();
    let mut event = accepted_event();
    event.operation = Operation::ExtractPreview;
    event.derivation_key =
        derivation_descriptor(request.input_fingerprint.clone(), Operation::ExtractPreview)
            .derive_key()
            .expect("key");
    event.event_type = EventType::Completed;
    event.sequence = 2;
    event.result = Some(scene_core_protocol::OperationResult::ExtractPreview(
        Box::new(ExtractPreviewResult {
            media: minimal_media(),
            artifact_manifest: manifest(vec![preview_artifact(), midpoint_artifact()]),
            resource_usage: ResourceUsage {
                wall_time_ms: 10,
                cpu_time_ms: None,
                peak_memory_bytes: None,
            },
        }),
    ));
    assert!(event.validate().is_ok());

    let line = serde_json::to_string(&event).expect("serializes");
    let parsed = EventEnvelope::parse(&line).expect("parses");
    assert_eq!(parsed, event);
    let value: serde_json::Value = serde_json::from_str(&line).expect("value");
    assert_eq!(value["result"]["operation"], "extract_preview");
    assert_eq!(
        value["result"]["artifactManifest"]["artifacts"][1]["artifactId"],
        "preview-midpoint"
    );
}

#[test]
fn terminal_error_codes_match_their_events() {
    let failed = |code: ErrorCode| {
        let mut event = accepted_event();
        event.event_type = EventType::Failed;
        event.sequence = 2;
        event.error = Some(ProtocolError::new(code, "safe message"));
        event
    };
    assert!(failed(ErrorCode::InputChanged).validate().is_ok());
    assert!(failed(ErrorCode::Timeout).validate().is_err());
    assert!(failed(ErrorCode::Cancelled).validate().is_err());

    let mut cancelled = accepted_event();
    cancelled.event_type = EventType::Cancelled;
    cancelled.sequence = 2;
    cancelled.error = Some(ProtocolError::new(ErrorCode::Cancelled, "cancelled"));
    assert!(cancelled.validate().is_ok());
    assert_eq!(cancelled.exit_code(), Some(130));

    let mut timed_out = accepted_event();
    timed_out.event_type = EventType::TimedOut;
    timed_out.sequence = 2;
    timed_out.error = Some(ProtocolError::new(ErrorCode::Timeout, "timed out"));
    assert!(timed_out.validate().is_ok());
    assert_eq!(timed_out.exit_code(), Some(124));

    let mut wrong = cancelled;
    wrong.error = Some(ProtocolError::new(ErrorCode::InputChanged, "changed"));
    assert!(wrong.validate().is_err());
}

#[test]
fn progress_event_requires_non_negative_totals() {
    let mut event = progress_event(2);
    assert!(event.validate().is_ok());

    event.total = None;
    assert!(event.validate().is_err());
}

#[test]
fn unknown_event_types_are_rejected_by_strict_parse() {
    let line = serde_json::to_string(&accepted_event()).expect("serializes");
    let mutated = line.replacen("\"accepted\"", "\"segment_ready\"", 1);
    assert!(EventEnvelope::parse(&mutated).is_err());
}

#[test]
fn transcript_accepts_accepted_progress_completed() {
    let mut validator = EventStreamValidator::new();
    validator.accept(&accepted_event()).expect("accepted");
    validator.accept(&progress_event(2)).expect("progress");
    let completed = completed_probe_event(3);
    validator.accept(&completed).expect("completed");

    assert!(validator.is_complete());
    assert_eq!(validator.terminal(), Some(EventType::Completed));
    assert_eq!(completed.exit_code(), Some(0));
    assert!(validator.accept(&progress_event(4)).is_err());
}

#[test]
fn transcript_rejects_duplicate_regressed_and_skipped_sequences() {
    let mut validator = EventStreamValidator::new();
    validator.accept(&accepted_event()).expect("accepted");

    assert!(validator.accept(&progress_event(1)).is_err());
    assert!(validator.accept(&progress_event(3)).is_err());

    validator.accept(&progress_event(2)).expect("progress");
    assert!(validator.accept(&progress_event(2)).is_err());
}

#[test]
fn transcript_rejects_events_after_the_terminal() {
    let mut validator = EventStreamValidator::new();
    validator.accept(&accepted_event()).expect("accepted");
    let mut failed = accepted_event();
    failed.event_type = EventType::Failed;
    failed.sequence = 2;
    failed.error = Some(ProtocolError::input_changed("changed"));
    validator.accept(&failed).expect("failed");
    assert_eq!(failed.exit_code(), Some(2));

    assert!(validator.accept(&completed_probe_event(3)).is_err());
}

#[test]
fn transcript_rejects_identity_changes() {
    let mut validator = EventStreamValidator::new();
    validator.accept(&accepted_event()).expect("accepted");

    let mut changed_source = progress_event(2);
    changed_source.source_version_id = Identifier::new("sourcev_other").expect("id");
    assert!(validator.accept(&changed_source).is_err());

    let mut changed_key = progress_event(2);
    changed_key.derivation_key = sha(TOOLCHAIN_FINGERPRINT);
    assert!(validator.accept(&changed_key).is_err());

    let mut changed_context = progress_event(2);
    changed_context.execution_context.attempt = 9;
    assert!(validator.accept(&changed_context).is_err());
}

#[test]
fn transcript_accepts_invalid_request_failed_at_sequence_one() {
    let mut validator = EventStreamValidator::new();
    let mut failed = accepted_event();
    failed.event_type = EventType::Failed;
    failed.error = Some(ProtocolError::invalid_request("bad request"));
    validator.accept(&failed).expect("initial failed");
    assert!(validator.is_complete());
    assert_eq!(failed.exit_code(), Some(2));

    let mut late = EventStreamValidator::new();
    let mut running = accepted_event();
    running.event_type = EventType::Running;
    assert!(late.accept(&running).is_err());

    let mut cancelled = accepted_event();
    cancelled.event_type = EventType::Cancelled;
    cancelled.error = Some(ProtocolError::new(ErrorCode::Cancelled, "cancelled"));
    assert!(late.accept(&cancelled).is_err());
}

fn manifest(artifacts: Vec<Artifact>) -> ArtifactManifest {
    let input_fingerprint = input_set().input_fingerprint().expect("fingerprint");
    let derivation_key =
        derivation_descriptor(input_fingerprint.clone(), Operation::ExtractPreview)
            .derive_key()
            .expect("key");
    ArtifactManifest {
        manifest_version: ProtocolVersion::current(),
        request_id: Identifier::new("req_01").expect("id"),
        source_version_id: Identifier::new("sourcev_01").expect("id"),
        input_fingerprint,
        derivation_key,
        operation: Operation::ExtractPreview,
        operation_config_hash: sha(EMPTY_OPTIONS_HASH),
        output_contract_version: Operation::ExtractPreview.output_contract_version(),
        engine: engine_identity(),
        toolchain_fingerprint: sha(TOOLCHAIN_FINGERPRINT),
        artifacts,
    }
}

#[test]
fn manifest_requires_fixed_preview_order() {
    let opening = manifest(vec![preview_artifact()]);
    assert!(opening.validate().is_ok());

    let full = manifest(vec![preview_artifact(), midpoint_artifact()]);
    assert!(full.validate().is_ok());
    let line = serde_json::to_string(&full).expect("serializes");
    let parsed: ArtifactManifest = serde_json::from_str(&line).expect("parses");
    assert_eq!(parsed, full);

    let value: serde_json::Value = serde_json::from_str(&line).expect("value");
    assert_eq!(value["manifestVersion"], "0.1");
    assert_eq!(
        value["artifacts"][0]["relativeRef"],
        "output/preview/opening.jpg"
    );
    assert_eq!(value["artifacts"][1]["presentationTimeMs"], 30_004);
    assert_eq!(
        value["engine"]["engineCacheCompatibilityId"],
        "scene-core-output-v1"
    );

    assert!(manifest(vec![midpoint_artifact()]).validate().is_err());
    assert!(manifest(vec![]).validate().is_err());
    assert!(
        manifest(vec![
            preview_artifact(),
            midpoint_artifact(),
            midpoint_artifact(),
        ])
        .validate()
        .is_err()
    );
}

#[test]
fn manifest_derivation_key_and_version_are_checked() {
    let mut tampered = manifest(vec![preview_artifact()]);
    tampered.derivation_key = sha(TOOLCHAIN_FINGERPRINT);
    assert!(tampered.validate().is_err());

    let mut wrong_version = manifest(vec![preview_artifact()]);
    wrong_version.manifest_version = ProtocolVersion::new("0.2").expect("version");
    assert!(wrong_version.validate().is_err());

    let mut wrong_operation = manifest(vec![preview_artifact()]);
    wrong_operation.operation = Operation::Probe;
    assert!(wrong_operation.validate().is_err());

    let mut wrong_contract = manifest(vec![preview_artifact()]);
    wrong_contract.output_contract_version =
        OutputContractVersion::new("probe-result/1").expect("contract");
    assert!(wrong_contract.validate().is_err());
}

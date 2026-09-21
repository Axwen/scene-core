//! Control message, event transcript and Manifest contract tests.

mod common;

use common::{
    EMPTY_OPTIONS_HASH, TOOLCHAIN_FINGERPRINT, audio_artifact, audio_manifest,
    derivation_descriptor, derivation_identity, engine_identity, input_set, midpoint_artifact,
    minimal_media, preview_artifact, probe_result, sha, start_request,
};
use scene_core_protocol::{
    Artifact, ArtifactKind, ArtifactManifest, AudioPcmInfo, AudioPcmOptions, CancelMessageType,
    CancelRequest, ControlMessage, DerivationDescriptor, ErrorCode, EventEnvelope,
    EventMessageType, EventStreamValidator, EventType, ExecutionScope, ExtractAudioPcmResult,
    ExtractPreviewResult, Identifier, Operation, OperationOptions, OperationResult,
    OutputContractVersion, ProtocolError, ProtocolVersion, RelativeRef, ResourceUsage, StageName,
    StartRequest, UtcTimestamp, canonical_sha256, parse_control_line,
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
fn progress_values_must_not_regress_or_exceed_total() {
    let mut above_total = progress_event(2);
    above_total.completed = Some(3);
    assert!(above_total.validate().is_err());

    let mut validator = EventStreamValidator::new();
    validator.accept(&accepted_event()).expect("accepted");
    validator.accept(&progress_event(2)).expect("progress");

    let mut regressed = progress_event(3);
    regressed.completed = Some(0);
    assert!(validator.accept(&regressed).is_err());

    let mut next_stage = progress_event(3);
    next_stage.stage = Some(StageName::new("preview").expect("stage"));
    next_stage.completed = Some(0);
    validator.accept(&next_stage).expect("new stage resets");
}

fn completed_preview_event() -> EventEnvelope {
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
    event
}

#[test]
fn completed_manifest_must_echo_the_event_identity() {
    let event = completed_preview_event();
    assert!(event.validate().is_ok());

    let tamper = |event: &EventEnvelope, position: usize| {
        let mut changed = event.clone();
        let scene_core_protocol::OperationResult::ExtractPreview(preview) =
            changed.result.as_mut().expect("result")
        else {
            unreachable!("preview result")
        };
        match position {
            0 => preview.artifact_manifest.request_id = Identifier::new("req_evil").expect("id"),
            1 => {
                preview.artifact_manifest.source_version_id =
                    Identifier::new("sourcev_evil").expect("id")
            }
            2 => preview.artifact_manifest.input_fingerprint = sha(TOOLCHAIN_FINGERPRINT),
            _ => preview.artifact_manifest.engine.engine_version = "9.9.9".to_owned(),
        }
        changed
    };
    for position in 0..4 {
        assert!(
            tamper(&event, position).validate().is_err(),
            "tamper {position} was accepted"
        );
    }
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
fn preview_requested_times_are_pinned_to_their_slots() {
    let mut opening = preview_artifact();
    opening.requested_time_ms = 12_345;
    assert!(manifest(vec![opening]).validate().is_err());

    let mut midpoint = midpoint_artifact();
    midpoint.requested_time_ms = 0;
    assert!(
        manifest(vec![preview_artifact(), midpoint])
            .validate()
            .is_err()
    );
}

#[test]
fn extract_audio_pcm_request_accepts_its_options() {
    let mut request = probe_request();
    request.operation = Operation::ExtractAudioPcm;
    request.output_contract_version = Operation::ExtractAudioPcm.output_contract_version();
    request.options = OperationOptions::ExtractAudioPcm(AudioPcmOptions {
        audio_stream_index: Some(1),
    });
    let config_hash = canonical_sha256(&serde_json::json!({"audioStreamIndex": 1})).expect("hash");
    request.operation_config_hash = config_hash.clone();
    let identity = derivation_identity();
    request.derivation_key = DerivationDescriptor {
        derivation_descriptor_version: scene_core_protocol::DescriptorVersion::current(),
        input_fingerprint: request.input_fingerprint.clone(),
        operation: Operation::ExtractAudioPcm,
        operation_config_hash: config_hash,
        output_contract_version: Operation::ExtractAudioPcm.output_contract_version(),
        engine_cache_compatibility_id: identity.engine_cache_compatibility_id.clone(),
        toolchain_fingerprint: identity.toolchain_fingerprint.clone(),
    }
    .derive_key()
    .expect("key");
    assert!(request.validate().is_ok());
    assert_eq!(request.options.audio_stream_index(), Some(1));
    assert!(request.validate_derivation(&identity).is_ok());

    // A different stream index is a different effective config.
    let mut other = request.clone();
    other.options = OperationOptions::ExtractAudioPcm(AudioPcmOptions {
        audio_stream_index: Some(2),
    });
    other.operation_config_hash =
        canonical_sha256(&serde_json::json!({"audioStreamIndex": 2})).expect("hash");
    assert!(other.validate().is_ok());
    assert!(
        other.validate_derivation(&identity).is_err(),
        "stale derivation key must fail"
    );
}

#[test]
fn audio_options_are_rejected_on_other_operations() {
    let mut request = probe_request();
    request.options = OperationOptions::ExtractAudioPcm(AudioPcmOptions {
        audio_stream_index: Some(1),
    });
    let error = request.validate().expect_err("options mismatch");
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[test]
fn audio_manifest_pins_its_slot_and_mapping_fields() {
    assert!(audio_manifest(vec![audio_artifact()]).validate().is_ok());

    let mut no_mapping = audio_manifest(vec![audio_artifact()]);
    no_mapping.artifacts[0].audio_pcm = None;
    assert!(no_mapping.validate().is_err());

    let mut no_start = audio_manifest(vec![audio_artifact()]);
    no_start.artifacts[0].presentation_time_ms = None;
    assert!(no_start.validate().is_err());

    let mut trimmed = audio_manifest(vec![audio_artifact()]);
    trimmed.artifacts[0].requested_time_ms = 1_000;
    assert!(trimmed.validate().is_err());

    let mut zero_rate = audio_manifest(vec![audio_artifact()]);
    zero_rate.artifacts[0].audio_pcm = Some(AudioPcmInfo {
        sample_rate: 0,
        channels: 2,
        sample_count: 10,
    });
    assert!(zero_rate.validate().is_err());

    let mut wrong_kind = audio_manifest(vec![audio_artifact()]);
    wrong_kind.artifacts[0].kind = ArtifactKind::PreviewFrame;
    assert!(wrong_kind.validate().is_err());

    let mut wrong_ref = audio_manifest(vec![audio_artifact()]);
    wrong_ref.artifacts[0].relative_ref = RelativeRef::new("output/audio/other.wav").expect("ref");
    assert!(wrong_ref.validate().is_err());

    let mut two = audio_manifest(vec![audio_artifact(), audio_artifact()]);
    two.artifacts[1].artifact_id = Identifier::new("audio-pcm-2").expect("id");
    assert!(two.validate().is_err());

    let mut preview_with_audio = manifest(vec![preview_artifact()]);
    preview_with_audio.artifacts[0].audio_pcm = Some(AudioPcmInfo {
        sample_rate: 48_000,
        channels: 2,
        sample_count: 10,
    });
    assert!(preview_with_audio.validate().is_err());
}

#[test]
fn completed_audio_manifest_must_echo_the_event_identity() {
    let audio = audio_manifest(vec![audio_artifact()]);
    let mut event = accepted_event();
    event.operation = Operation::ExtractAudioPcm;
    event.derivation_key = audio.derivation_key.clone();
    event.event_type = EventType::Completed;
    event.sequence = 2;
    event.result = Some(OperationResult::ExtractAudioPcm(Box::new(
        ExtractAudioPcmResult {
            media: minimal_media(),
            artifact_manifest: audio,
            resource_usage: ResourceUsage {
                wall_time_ms: 5,
                cpu_time_ms: None,
                peak_memory_bytes: None,
            },
        },
    )));
    assert!(event.validate().is_ok());

    let mut tampered = event;
    if let Some(OperationResult::ExtractAudioPcm(audio)) = tampered.result.as_mut() {
        audio.artifact_manifest.request_id = Identifier::new("req_evil").expect("id");
    }
    assert!(tampered.validate().is_err());
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

#![allow(dead_code)]

use scene_core_protocol::*;

pub const CONTENT_HASH: &str =
    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const TOOLCHAIN_FINGERPRINT: &str =
    "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
pub const ENGINE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
pub const EMPTY_OPTIONS_HASH: &str =
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";

pub fn sha(value: &str) -> Sha256Digest {
    Sha256Digest::new(value).expect("valid digest")
}

pub fn cache_id() -> CacheCompatibilityId {
    CacheCompatibilityId::new("scene-core-output-v1").expect("valid cache id")
}

pub fn derivation_identity() -> DerivationIdentity {
    DerivationIdentity {
        engine_cache_compatibility_id: cache_id(),
        toolchain_fingerprint: sha(TOOLCHAIN_FINGERPRINT),
    }
}

pub fn source_media_input() -> InputDescriptor {
    InputDescriptor {
        role: InputRole::new(InputRole::SOURCE_MEDIA).expect("role"),
        input_ref: InputRef::new("input/source.media").expect("ref"),
        content_hash: sha(CONTENT_HASH),
        byte_size: 1_048_576,
    }
}

pub fn input_set() -> InputSetDescriptor {
    InputSetDescriptor::from_inputs(vec![source_media_input()])
}

pub fn engine_identity() -> EngineIdentity {
    EngineIdentity {
        engine_version: "0.1.0-alpha.1".to_owned(),
        engine_commit: CommitHash::new(ENGINE_COMMIT).expect("commit"),
        target: "x86_64-pc-windows-msvc".to_owned(),
        engine_cache_compatibility_id: cache_id(),
        supported_protocol_versions: vec![ProtocolVersion::current()],
        implemented_operations: Vec::new(),
    }
}

pub fn minimal_media() -> NormalizedMedia {
    NormalizedMedia {
        container: ContainerInfo {
            format_name: "matroska,webm".to_owned(),
            duration_ms: Some(60_000),
            start_time_ms: Some(0),
            bit_rate_bps: Some(1_000_000),
            file_size_bytes: 1_048_576,
        },
        streams: vec![],
        primary_video_stream_index: None,
    }
}

pub fn preview_artifact() -> Artifact {
    Artifact {
        artifact_id: Identifier::new("preview-opening").expect("id"),
        kind: ArtifactKind::PreviewFrame,
        role: ArtifactRole::Opening,
        media_type: MediaType::new("image/jpeg").expect("media type"),
        relative_ref: RelativeRef::new("output/preview/opening.jpg").expect("ref"),
        byte_size: 12_345,
        content_hash: sha(TOOLCHAIN_FINGERPRINT),
        requested_time_ms: 0,
        presentation_time_ms: None,
        audio_pcm: None,
        pixel_width: Some(512),
        pixel_height: Some(288),
    }
}

pub fn audio_artifact() -> Artifact {
    Artifact {
        artifact_id: Identifier::new("audio-pcm").expect("id"),
        kind: ArtifactKind::AudioPcm,
        role: ArtifactRole::Audio,
        media_type: MediaType::new("audio/wav").expect("media type"),
        relative_ref: RelativeRef::new("output/audio/track.wav").expect("ref"),
        byte_size: 96_044,
        content_hash: sha(TOOLCHAIN_FINGERPRINT),
        requested_time_ms: 0,
        presentation_time_ms: Some(80),
        audio_pcm: Some(AudioPcmInfo {
            sample_rate: 48_000,
            channels: 2,
            sample_count: 48_000,
        }),
        pixel_width: None,
        pixel_height: None,
    }
}

pub fn audio_manifest(artifacts: Vec<Artifact>) -> ArtifactManifest {
    let input_fingerprint = input_set().input_fingerprint().expect("fingerprint");
    let operation = Operation::ExtractAudioPcm;
    let derivation_key = derivation_descriptor(input_fingerprint.clone(), operation)
        .derive_key()
        .expect("key");
    ArtifactManifest {
        manifest_version: ProtocolVersion::current(),
        request_id: Identifier::new("req_01").expect("id"),
        source_version_id: Identifier::new("sourcev_01").expect("id"),
        input_fingerprint,
        derivation_key,
        operation,
        operation_config_hash: sha(EMPTY_OPTIONS_HASH),
        output_contract_version: operation.output_contract_version(),
        engine: engine_identity(),
        toolchain_fingerprint: sha(TOOLCHAIN_FINGERPRINT),
        artifacts,
    }
}

pub fn midpoint_artifact() -> Artifact {
    let mut artifact = preview_artifact();
    artifact.artifact_id = Identifier::new("preview-midpoint").expect("id");
    artifact.role = ArtifactRole::Midpoint;
    artifact.relative_ref = RelativeRef::new("output/preview/midpoint.jpg").expect("ref");
    artifact.requested_time_ms = 30_000;
    artifact.presentation_time_ms = Some(30_004);
    artifact
}

pub fn probe_result() -> ProbeResult {
    ProbeResult {
        media: minimal_media(),
        resource_usage: ResourceUsage {
            wall_time_ms: 125,
            cpu_time_ms: Some(90),
            peak_memory_bytes: Some(64 * 1024 * 1024),
        },
    }
}

pub fn derivation_descriptor(
    input_fingerprint: Sha256Digest,
    operation: Operation,
) -> DerivationDescriptor {
    DerivationDescriptor {
        derivation_descriptor_version: DescriptorVersion::current(),
        input_fingerprint,
        operation,
        operation_config_hash: sha(EMPTY_OPTIONS_HASH),
        output_contract_version: operation.output_contract_version(),
        engine_cache_compatibility_id: cache_id(),
        toolchain_fingerprint: sha(TOOLCHAIN_FINGERPRINT),
    }
}

pub fn start_request(request_id: &str, source_version_id: &str) -> StartRequest {
    let inputs = vec![source_media_input()];
    let input_fingerprint = InputSetDescriptor::from_inputs(inputs.clone())
        .input_fingerprint()
        .expect("input fingerprint");
    let operation = Operation::Probe;
    let derivation_key = derivation_descriptor(input_fingerprint.clone(), operation)
        .derive_key()
        .expect("derivation key");
    StartRequest {
        engine_protocol_version: ProtocolVersion::current(),
        message_type: StartMessageType::Start,
        request_id: Identifier::new(request_id).expect("request id"),
        operation,
        source_version_id: Identifier::new(source_version_id).expect("source version id"),
        inputs,
        input_fingerprint,
        operation_config_hash: sha(EMPTY_OPTIONS_HASH),
        output_contract_version: operation.output_contract_version(),
        derivation_key,
        execution_context: ExecutionContext {
            run_id: Identifier::new("run_01").expect("run id"),
            generation: 0,
            attempt: 1,
            scope: ExecutionScope::Asset,
        },
        deadline_ms: None,
        options: OperationOptions::default(),
    }
}

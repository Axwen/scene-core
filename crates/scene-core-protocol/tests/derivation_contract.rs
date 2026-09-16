//! Derivation and identity contract: one key on both sides, sensitive to
//! every cache identity input and insensitive to request/run identity.

mod common;

use common::{
    CONTENT_HASH, TOOLCHAIN_FINGERPRINT, cache_id, derivation_descriptor, derivation_identity,
    engine_identity, sha, start_request,
};
use scene_core_protocol::{
    CacheCompatibilityId, DerivationDescriptor, ErrorCode, Operation, OutputContractVersion,
    ProtocolVersion, ToolchainIdentity,
};

#[test]
fn derivation_key_matches_frozen_golden() {
    let key = derivation_descriptor(sha(CONTENT_HASH), Operation::Probe)
        .derive_key()
        .expect("key");
    assert_eq!(
        key.as_str(),
        "sha256:28abe45a24b58489ee3b23dfc669618bb9bf02e1009da903a18738c285dcd964"
    );
}

#[test]
fn derivation_key_changes_with_every_cache_identity_input() {
    let base = derivation_descriptor(sha(CONTENT_HASH), Operation::Probe);
    let base_key = base.derive_key().expect("key");

    let mut operation = base.clone();
    operation.operation = Operation::ExtractPreview;
    operation.output_contract_version = Operation::ExtractPreview.output_contract_version();
    assert_ne!(operation.derive_key().expect("key"), base_key);

    let mut input = base.clone();
    input.input_fingerprint = sha(TOOLCHAIN_FINGERPRINT);
    assert_ne!(input.derive_key().expect("key"), base_key);

    let mut config = base.clone();
    config.operation_config_hash = sha(TOOLCHAIN_FINGERPRINT);
    assert_ne!(config.derive_key().expect("key"), base_key);

    let mut contract = base.clone();
    contract.output_contract_version =
        OutputContractVersion::new("probe-result/2").expect("contract version");
    assert_ne!(contract.derive_key().expect("key"), base_key);

    let mut engine = base.clone();
    engine.engine_cache_compatibility_id =
        CacheCompatibilityId::new("scene-core-output-v2").expect("cache id");
    assert_ne!(engine.derive_key().expect("key"), base_key);

    let mut toolchain = base.clone();
    toolchain.toolchain_fingerprint = sha(CONTENT_HASH);
    assert_ne!(toolchain.derive_key().expect("key"), base_key);

    assert_eq!(base.derive_key().expect("key"), base_key);
}

#[test]
fn host_and_engine_derive_the_same_key() {
    let request = start_request("req_01", "sourcev_01");
    let engine_side = request
        .derivation_descriptor(&derivation_identity())
        .expect("descriptor");
    let host_side = derivation_descriptor(request.input_fingerprint.clone(), request.operation);

    assert_eq!(
        engine_side.derive_key().expect("key"),
        host_side.derive_key().expect("key")
    );
    assert_eq!(host_side.derive_key().expect("key"), request.derivation_key);
    assert!(request.validate_derivation(&derivation_identity()).is_ok());
}

#[test]
fn derivation_key_mismatch_is_invalid_request() {
    let mut request = start_request("req_01", "sourcev_01");
    request.derivation_key = sha(TOOLCHAIN_FINGERPRINT);
    let error = request
        .validate_derivation(&derivation_identity())
        .expect_err("mismatch");
    assert_eq!(error.code, ErrorCode::InvalidRequest);

    let different_toolchain = scene_core_protocol::DerivationIdentity {
        engine_cache_compatibility_id: cache_id(),
        toolchain_fingerprint: sha(CONTENT_HASH),
    };
    let error = request
        .validate_derivation(&different_toolchain)
        .expect_err("different toolchain");
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[test]
fn derivation_descriptor_rejects_unregistered_contract() {
    let mut descriptor: DerivationDescriptor =
        derivation_descriptor(sha(CONTENT_HASH), Operation::Probe);
    descriptor.output_contract_version =
        OutputContractVersion::new("extract-preview-result/1").expect("contract version");
    assert!(descriptor.validate().is_err());
}

#[test]
fn engine_identity_validation_rejects_duplicates_and_bad_targets() {
    let identity = engine_identity();
    assert!(identity.validate().is_ok());

    let mut duplicate_versions = identity.clone();
    duplicate_versions
        .supported_protocol_versions
        .push(ProtocolVersion::current());
    assert!(duplicate_versions.validate().is_err());

    let mut duplicate_operations = identity.clone();
    duplicate_operations
        .implemented_operations
        .push(Operation::Probe);
    duplicate_operations
        .implemented_operations
        .push(Operation::Probe);
    assert!(duplicate_operations.validate().is_err());

    let mut bad_target = identity.clone();
    bad_target.target = "WINDOWS".to_owned();
    assert!(bad_target.validate().is_err());

    let mut empty_version = identity.clone();
    empty_version.engine_version = String::new();
    assert!(empty_version.validate().is_err());
}

#[test]
fn toolchain_identity_roundtrips_with_camel_case_fields() {
    let identity = ToolchainIdentity {
        toolchain_fingerprint: sha(TOOLCHAIN_FINGERPRINT),
        target: "x86_64-pc-windows-msvc".to_owned(),
        ffmpeg_version: "7.1.1".to_owned(),
        ffprobe_version: "7.1.1".to_owned(),
        capability_set_fingerprint: sha(CONTENT_HASH),
    };
    assert!(identity.validate().is_ok());

    let value = serde_json::to_value(&identity).expect("serializes");
    assert!(value.get("toolchainFingerprint").is_some());
    assert!(value.get("ffmpegVersion").is_some());

    let parsed: ToolchainIdentity = serde_json::from_value(value).expect("parses");
    assert_eq!(parsed, identity);
}

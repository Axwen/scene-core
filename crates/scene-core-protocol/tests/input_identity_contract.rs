//! Input identity contract: `contentHash`, `inputFingerprint` and staged
//! snapshot verification.

mod common;

use common::{
    TOOLCHAIN_FINGERPRINT, derivation_identity, input_set, sha, source_media_input, start_request,
};
use scene_core_protocol::{
    ErrorCode, Identifier, InputRole, InputSetDescriptor, MAX_INPUT_BYTE_SIZE, StagedSourceFacts,
    StartRequest, verify_input_fingerprint, verify_staged_source,
};

#[test]
fn input_fingerprint_matches_frozen_golden() {
    let fingerprint = input_set().input_fingerprint().expect("fingerprint");
    assert_eq!(
        fingerprint.as_str(),
        "sha256:ceaeaa5c772ab1951c10a0c86f5dd51c5a14987fc8d8ea153c501e645cfc9598"
    );
}

#[test]
fn content_size_and_role_change_the_input_fingerprint() {
    let base = input_set();
    let base_fingerprint = base.input_fingerprint().expect("fingerprint");

    let mut changed_hash = base.clone();
    changed_hash.inputs[0].content_hash = sha(TOOLCHAIN_FINGERPRINT);
    assert_ne!(
        changed_hash.input_fingerprint().expect("fingerprint"),
        base_fingerprint
    );

    let mut changed_size = base.clone();
    changed_size.inputs[0].byte_size += 1;
    assert_ne!(
        changed_size.input_fingerprint().expect("fingerprint"),
        base_fingerprint
    );

    let mut changed_role = base.clone();
    changed_role.inputs[0].role = InputRole::new("sidecar_metadata").expect("role");
    assert_ne!(
        changed_role.input_fingerprint().expect("fingerprint"),
        base_fingerprint
    );

    assert_eq!(base.clone(), base);
}

#[test]
fn request_identity_does_not_change_input_or_derivation_identity() {
    let first = start_request("req_01", "sourcev_01");
    let mut second = start_request("req_02", "sourcev_02");
    second.execution_context.run_id = Identifier::new("run_02").expect("run id");
    second.execution_context.attempt = 7;

    let fingerprint = |request: &StartRequest| {
        InputSetDescriptor::from_inputs(request.inputs.clone())
            .input_fingerprint()
            .expect("fingerprint")
    };
    assert_eq!(fingerprint(&first), fingerprint(&second));

    let key = |request: &StartRequest| {
        request
            .derivation_descriptor(&derivation_identity())
            .expect("descriptor")
            .derive_key()
            .expect("key")
    };
    assert_eq!(key(&first), key(&second));
    assert!(first.validate().is_ok());
    assert!(second.validate().is_ok());
}

#[test]
fn declared_digest_mismatch_is_invalid_request() {
    let request = start_request("req_01", "sourcev_01");
    let descriptor = InputSetDescriptor::from_inputs(request.inputs.clone());
    assert!(verify_input_fingerprint(&descriptor, &request.input_fingerprint).is_ok());

    let wrong = sha(TOOLCHAIN_FINGERPRINT);
    let error = verify_input_fingerprint(&descriptor, &wrong).expect_err("mismatch");
    assert_eq!(error.code, ErrorCode::InvalidRequest);

    let mut tampered = request.clone();
    tampered.input_fingerprint = wrong;
    let error = tampered.validate().expect_err("tampered fingerprint");
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[test]
fn staged_snapshot_mismatch_is_input_changed() {
    let declared = source_media_input();

    let missing = verify_staged_source(&declared, None).expect_err("missing");
    assert_eq!(missing.code, ErrorCode::InputNotFound);

    let changed_size = StagedSourceFacts {
        byte_size: declared.byte_size + 1,
        content_hash: declared.content_hash.clone(),
    };
    let error = verify_staged_source(&declared, Some(&changed_size)).expect_err("size");
    assert_eq!(error.code, ErrorCode::InputChanged);

    let changed_hash = StagedSourceFacts {
        byte_size: declared.byte_size,
        content_hash: sha(TOOLCHAIN_FINGERPRINT),
    };
    let error = verify_staged_source(&declared, Some(&changed_hash)).expect_err("hash");
    assert_eq!(error.code, ErrorCode::InputChanged);

    let matching = StagedSourceFacts {
        byte_size: declared.byte_size,
        content_hash: declared.content_hash.clone(),
    };
    assert!(verify_staged_source(&declared, Some(&matching)).is_ok());
}

#[test]
fn protocol_01_allows_exactly_one_source_media() {
    let mut duplicate = input_set();
    duplicate.inputs.push(source_media_input());
    assert!(duplicate.validate().is_err());

    let mut wrong_role = input_set();
    wrong_role.inputs[0].role = InputRole::new("sidecar_metadata").expect("role");
    assert!(wrong_role.validate().is_err());

    let mut empty = input_set();
    empty.inputs.clear();
    assert!(empty.validate().is_err());

    let mut request = start_request("req_01", "sourcev_01");
    request.inputs.push(source_media_input());
    let error = request.validate().expect_err("two inputs");
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[test]
fn absolute_paths_and_unsafe_refs_are_rejected() {
    for value in [
        "/etc/passwd",
        "C:/media/source.mp4",
        "C:\\media\\source.mp4",
        "\\\\server\\share\\source.mp4",
        "input/../../source.media",
        "input/source.media:stream",
        "input//source.media",
    ] {
        assert!(
            scene_core_protocol::InputRef::new(value).is_err(),
            "{value}"
        );
        assert!(
            scene_core_protocol::RelativeRef::new(value).is_err(),
            "{value}"
        );
    }
    assert!(scene_core_protocol::InputRef::new("input/source.media").is_ok());
    assert!(scene_core_protocol::RelativeRef::new("input/source.media").is_ok());
}

#[test]
fn oversized_input_is_rejected() {
    let mut input = source_media_input();
    input.byte_size = MAX_INPUT_BYTE_SIZE + 1;
    assert!(input.validate().is_err());

    input.byte_size = MAX_INPUT_BYTE_SIZE;
    assert!(input.validate().is_ok());
}

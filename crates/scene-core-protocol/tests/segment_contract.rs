//! Temporal segment contract: half-open ranges, deterministic order,
//! ordinals, parent coverage and acyclic parents.

mod common;

use common::{CONTENT_HASH, TOOLCHAIN_FINGERPRINT, probe_result, sha};
use scene_core_protocol::{
    CacheCompatibilityId, Identifier, OperationResult, PolicyRef, SegmentProvenance,
    TemporalSegment, TemporalSegmentKind, validate_temporal_segments,
};

fn segment(
    id: &str,
    kind: TemporalSegmentKind,
    start_ms: u64,
    end_ms: u64,
    ordinal: u64,
    parent: Option<&str>,
) -> TemporalSegment {
    TemporalSegment {
        segment_id: Identifier::new(id).expect("segment id"),
        kind,
        start_ms,
        end_ms,
        parent_segment_id: parent.map(|value| Identifier::new(value).expect("parent id")),
        ordinal,
        segmentation_policy_ref: PolicyRef::new("shot-boundary/example@1").expect("policy"),
        provenance: SegmentProvenance {
            derivation_key: sha(CONTENT_HASH),
            engine_cache_compatibility_id: CacheCompatibilityId::new("segment-shot-v1")
                .expect("cache id"),
            toolchain_fingerprint: sha(TOOLCHAIN_FINGERPRINT),
        },
    }
}

fn valid_set() -> Vec<TemporalSegment> {
    vec![
        segment(
            "scene_000001",
            TemporalSegmentKind::SceneCandidate,
            0,
            10_000,
            0,
            None,
        ),
        segment(
            "shot_000001",
            TemporalSegmentKind::Shot,
            0,
            4_000,
            0,
            Some("scene_000001"),
        ),
        segment(
            "shot_000002",
            TemporalSegmentKind::Shot,
            4_000,
            10_000,
            1,
            Some("scene_000001"),
        ),
    ]
}

#[test]
fn valid_segments_pass_with_parent_coverage() {
    assert!(validate_temporal_segments(&valid_set()).is_ok());
}

#[test]
fn empty_and_negative_ranges_are_rejected() {
    let mut segments = valid_set();
    segments[1].end_ms = segments[1].start_ms;
    assert!(validate_temporal_segments(&segments).is_err());

    let mut segments = valid_set();
    segments[2].end_ms = segments[2].start_ms - 1;
    assert!(validate_temporal_segments(&segments).is_err());
}

#[test]
fn deterministic_order_and_ordinals_are_enforced() {
    let mut out_of_order = valid_set();
    out_of_order.swap(1, 2);
    assert!(validate_temporal_segments(&out_of_order).is_err());

    let mut bad_ordinal = valid_set();
    bad_ordinal[2].ordinal = 0;
    assert!(validate_temporal_segments(&bad_ordinal).is_err());
}

#[test]
fn duplicate_segment_ids_are_rejected() {
    let mut segments = valid_set();
    segments[2].segment_id = segments[1].segment_id.clone();
    assert!(validate_temporal_segments(&segments).is_err());
}

#[test]
fn parents_must_exist_cover_and_stay_acyclic() {
    let mut missing = valid_set();
    missing[1].parent_segment_id = Some(Identifier::new("scene_missing").expect("id"));
    assert!(validate_temporal_segments(&missing).is_err());

    let mut not_covering = valid_set();
    not_covering[0].end_ms = 3_000;
    assert!(validate_temporal_segments(&not_covering).is_err());

    let mut self_parent = valid_set();
    self_parent[0].parent_segment_id = Some(Identifier::new("scene_000001").expect("id"));
    assert!(validate_temporal_segments(&self_parent).is_err());

    let cyclic = vec![
        segment(
            "shot_a",
            TemporalSegmentKind::Shot,
            0,
            1_000,
            0,
            Some("shot_b"),
        ),
        segment(
            "shot_b",
            TemporalSegmentKind::Shot,
            0,
            1_000,
            1,
            Some("shot_a"),
        ),
    ];
    assert!(validate_temporal_segments(&cyclic).is_err());
}

#[test]
fn probe_and_preview_results_never_carry_segments() {
    let mut value = serde_json::to_value(probe_result()).expect("result serializes");
    value["segments"] = serde_json::json!([]);
    assert!(serde_json::from_value::<OperationResult>(value).is_err());
}

#[test]
fn segment_roundtrips_with_optional_parent() {
    let segments = valid_set();
    let value = serde_json::to_value(&segments[0]).expect("serializes");
    assert_eq!(value["kind"], serde_json::json!("sceneCandidate"));
    assert_eq!(value["parentSegmentId"], serde_json::Value::Null);
    assert_eq!(
        value["segmentationPolicyRef"],
        serde_json::json!("shot-boundary/example@1")
    );
    let parsed: TemporalSegment = serde_json::from_value(value).expect("parses");
    assert_eq!(parsed, segments[0]);
}

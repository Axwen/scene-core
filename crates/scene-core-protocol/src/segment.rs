//! Scene/Shot seam: candidate temporal segments with deterministic ordering
//! and provenance. `probe`/`extract_preview` never produce these.

use crate::error::ValidationError;
use crate::identity::validate_unique;
use crate::values::{CacheCompatibilityId, Identifier, PolicyRef, Sha256Digest};
use serde::{Deserialize, Serialize};

/// Segment kinds frozen by Protocol 0.1.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum TemporalSegmentKind {
    Shot,
    SceneCandidate,
}

impl TemporalSegmentKind {
    pub const ALL: [TemporalSegmentKind; 2] = [
        TemporalSegmentKind::Shot,
        TemporalSegmentKind::SceneCandidate,
    ];
}

/// Half-open `[startMs, endMs)` candidate interval. Public times are
/// non-negative; parents must cover their children.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TemporalSegment {
    pub segment_id: Identifier,
    pub kind: TemporalSegmentKind,
    pub start_ms: u64,
    pub end_ms: u64,
    #[serde(default)]
    pub parent_segment_id: Option<Identifier>,
    pub ordinal: u64,
    pub segmentation_policy_ref: PolicyRef,
    pub provenance: SegmentProvenance,
}

impl TemporalSegment {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_at("segment")
    }

    /// Same rules as [`TemporalSegment::validate`] with a caller-provided
    /// diagnostic path, e.g. `segments[2]`.
    pub fn validate_at(&self, path: &str) -> Result<(), ValidationError> {
        if self.end_ms <= self.start_ms {
            return Err(ValidationError::new(
                format!("{path}.endMs"),
                "must be greater than startMs",
            ));
        }
        Ok(())
    }
}

/// Required reproducibility evidence for a segmentation policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SegmentProvenance {
    pub derivation_key: Sha256Digest,
    pub engine_cache_compatibility_id: CacheCompatibilityId,
    pub toolchain_fingerprint: Sha256Digest,
}

/// Validates one candidate segment set: unique IDs, per-kind deterministic
/// `(startMs, endMs, segmentId)` order, matching `ordinal`, parent coverage and
/// no cycles.
pub fn validate_temporal_segments(segments: &[TemporalSegment]) -> Result<(), ValidationError> {
    for (position, segment) in segments.iter().enumerate() {
        segment.validate_at(&format!("segments[{position}]"))?;
    }
    validate_unique(
        "segments[].segmentId",
        segments.iter().map(|segment| segment.segment_id.as_str()),
    )?;

    for kind in TemporalSegmentKind::ALL {
        let group: Vec<&TemporalSegment> = segments
            .iter()
            .filter(|segment| segment.kind == kind)
            .collect();
        let mut sorted = group.clone();
        sorted.sort_by(|left, right| {
            (left.start_ms, left.end_ms, left.segment_id.as_str()).cmp(&(
                right.start_ms,
                right.end_ms,
                right.segment_id.as_str(),
            ))
        });
        if group != sorted {
            return Err(ValidationError::new(
                "segments",
                "must list same-kind segments in (startMs, endMs, segmentId) order",
            ));
        }
        for (position, segment) in sorted.iter().enumerate() {
            if segment.ordinal != position as u64 {
                return Err(ValidationError::new(
                    format!("segment {} ordinal", segment.segment_id),
                    "must equal the deterministic per-kind sort position",
                ));
            }
        }
    }

    let by_id: std::collections::HashMap<&str, &TemporalSegment> = segments
        .iter()
        .map(|segment| (segment.segment_id.as_str(), segment))
        .collect();
    for segment in segments {
        let Some(parent_id) = &segment.parent_segment_id else {
            continue;
        };
        if parent_id == &segment.segment_id {
            return Err(ValidationError::new(
                format!("segment {} parentSegmentId", segment.segment_id),
                "must not reference itself",
            ));
        }
        let Some(parent) = by_id.get(parent_id.as_str()) else {
            return Err(ValidationError::new(
                format!("segment {} parentSegmentId", segment.segment_id),
                "must reference a segment in the same set",
            ));
        };
        if parent.start_ms > segment.start_ms || parent.end_ms < segment.end_ms {
            return Err(ValidationError::new(
                format!("segment {} parentSegmentId", segment.segment_id),
                "parent must cover the child interval",
            ));
        }
        let mut cursor = parent;
        let mut steps = 0_usize;
        while let Some(next_id) = &cursor.parent_segment_id {
            let Some(next) = by_id.get(next_id.as_str()) else {
                break;
            };
            steps += 1;
            if steps > segments.len() {
                return Err(ValidationError::new(
                    format!("segment {} parentSegmentId", segment.segment_id),
                    "must not form a parent cycle",
                ));
            }
            cursor = next;
        }
    }
    Ok(())
}

//! Operation-specific completed results.
//!
//! `probe` and `extract_preview` never produce `TemporalSegment` values;
//! segmentation can only arrive through a future operation or protocol
//! version.

use crate::error::ValidationError;
use crate::manifest::ArtifactManifest;
use crate::media::NormalizedMedia;
use crate::operation::Operation;
use serde::{Deserialize, Serialize};

/// Resource accounting attached to a completed result. Phase 0 freezes the
/// field boundary; Phase 1 fills the values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResourceUsage {
    pub wall_time_ms: u64,
    pub cpu_time_ms: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProbeResult {
    pub media: NormalizedMedia,
    pub resource_usage: ResourceUsage,
}

impl ProbeResult {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.media.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExtractPreviewResult {
    pub media: NormalizedMedia,
    pub artifact_manifest: ArtifactManifest,
    pub resource_usage: ResourceUsage,
}

impl ExtractPreviewResult {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.media.validate()?;
        self.artifact_manifest.validate()
    }
}

/// Completed result of `extract_audio_pcm`. Same shape as the preview result:
/// the normalized snapshot, the Manifest of the extracted track and resource
/// accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExtractAudioPcmResult {
    pub media: NormalizedMedia,
    pub artifact_manifest: ArtifactManifest,
    pub resource_usage: ResourceUsage,
}

impl ExtractAudioPcmResult {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.media.validate()?;
        self.artifact_manifest.validate()
    }
}

/// Tagged union of completed results, tagged by `operation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum OperationResult {
    Probe(ProbeResult),
    ExtractPreview(Box<ExtractPreviewResult>),
    ExtractAudioPcm(Box<ExtractAudioPcmResult>),
}

impl OperationResult {
    pub const fn operation(&self) -> Operation {
        match self {
            OperationResult::Probe(_) => Operation::Probe,
            OperationResult::ExtractPreview(_) => Operation::ExtractPreview,
            OperationResult::ExtractAudioPcm(_) => Operation::ExtractAudioPcm,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            OperationResult::Probe(result) => result.validate(),
            OperationResult::ExtractPreview(result) => result.validate(),
            OperationResult::ExtractAudioPcm(result) => result.validate(),
        }
    }
}

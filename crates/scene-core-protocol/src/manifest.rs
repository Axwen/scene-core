//! Artifact Manifest for engine-produced files.

use crate::MANIFEST_VERSION;
use crate::error::ValidationError;
use crate::identity::{DerivationDescriptor, DerivationIdentity, EngineIdentity, validate_unique};
use crate::operation::Operation;
use crate::values::{
    DescriptorVersion, Identifier, MediaType, OutputContractVersion, ProtocolVersion, RelativeRef,
    Sha256Digest,
};
use serde::{Deserialize, Serialize};

/// Artifact kinds frozen by Protocol 0.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    PreviewFrame,
    AudioPcm,
}

/// Fixed artifact roles: diagnostic preview frames and the extracted audio
/// track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    Opening,
    Midpoint,
    Audio,
}

/// Audio-specific mapping fields. Present exactly on `audioPcm` artifacts.
/// `presentation_time_ms` carries the material time of the first output
/// sample; these fields complete the mapping
/// `materialTime = presentationTimeMs + sampleIndex / sampleRate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AudioPcmInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub sample_count: u64,
}

impl AudioPcmInfo {
    pub fn validate_at(&self, path: &str) -> Result<(), ValidationError> {
        for (field, value) in [
            ("sampleRate", self.sample_rate),
            ("channels", self.channels),
        ] {
            if value == 0 {
                return Err(ValidationError::new(
                    format!("{path}.{field}"),
                    "must be positive",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Artifact {
    pub artifact_id: Identifier,
    pub kind: ArtifactKind,
    pub role: ArtifactRole,
    pub media_type: MediaType,
    pub relative_ref: RelativeRef,
    pub byte_size: u64,
    pub content_hash: Sha256Digest,
    pub requested_time_ms: u64,
    pub presentation_time_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_pcm: Option<AudioPcmInfo>,
    pub pixel_width: Option<u32>,
    pub pixel_height: Option<u32>,
}

impl Artifact {
    /// Per-artifact structural rules that do not depend on the Manifest order.
    pub fn validate_at(&self, path: &str) -> Result<(), ValidationError> {
        for (field, value) in [
            ("pixelWidth", self.pixel_width),
            ("pixelHeight", self.pixel_height),
        ] {
            if value == Some(0) {
                return Err(ValidationError::new(
                    format!("{path}.{field}"),
                    "must be positive when known, or null",
                ));
            }
        }
        if let Some(audio) = &self.audio_pcm {
            audio.validate_at(path)?;
        }
        Ok(())
    }
}

/// Manifest of one immutable Artifact set. Contains no timestamp or resource
/// statistics; `probe` never produces one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ArtifactManifest {
    pub manifest_version: ProtocolVersion,
    pub request_id: Identifier,
    pub source_version_id: Identifier,
    pub input_fingerprint: Sha256Digest,
    pub derivation_key: Sha256Digest,
    pub operation: Operation,
    pub operation_config_hash: Sha256Digest,
    pub output_contract_version: OutputContractVersion,
    pub engine: EngineIdentity,
    pub toolchain_fingerprint: Sha256Digest,
    pub artifacts: Vec<Artifact>,
}

impl ArtifactManifest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.manifest_version.as_str() != MANIFEST_VERSION {
            return Err(ValidationError::new(
                "manifestVersion",
                "must be '0.1' in Protocol 0.1",
            ));
        }
        self.engine.validate()?;
        if !matches!(
            self.operation,
            Operation::ExtractPreview | Operation::ExtractAudioPcm
        ) {
            return Err(ValidationError::new(
                "operation",
                "Protocol 0.1 Manifests only describe extract_preview and extract_audio_pcm",
            ));
        }
        if !self
            .operation
            .is_registered_output_contract(&self.output_contract_version)
        {
            return Err(ValidationError::new(
                "outputContractVersion",
                "must be the registered result contract of the operation",
            ));
        }
        let descriptor = DerivationDescriptor {
            derivation_descriptor_version: DescriptorVersion::current(),
            input_fingerprint: self.input_fingerprint.clone(),
            operation: self.operation,
            operation_config_hash: self.operation_config_hash.clone(),
            output_contract_version: self.output_contract_version.clone(),
            engine_cache_compatibility_id: self.engine.engine_cache_compatibility_id.clone(),
            toolchain_fingerprint: self.toolchain_fingerprint.clone(),
        };
        descriptor.validate()?;
        let expected = descriptor
            .derive_key()
            .expect("manifest derivation descriptor serializes");
        if expected != self.derivation_key {
            return Err(ValidationError::new(
                "derivationKey",
                "does not match the recomputed derivation descriptor",
            ));
        }
        validate_unique(
            "artifacts[].artifactId",
            self.artifacts
                .iter()
                .map(|artifact| artifact.artifact_id.as_str()),
        )?;
        validate_unique(
            "artifacts[].relativeRef",
            self.artifacts
                .iter()
                .map(|artifact| artifact.relative_ref.as_str()),
        )?;
        for (position, artifact) in self.artifacts.iter().enumerate() {
            artifact.validate_at(&format!("artifacts[{position}]"))?;
        }
        match self.operation {
            Operation::ExtractPreview => self.validate_preview_artifacts(),
            Operation::ExtractAudioPcm => self.validate_audio_artifacts(),
            _ => unreachable!("operation was checked above"),
        }
    }

    /// Engine-side identity pair used to recompute this Manifest's key.
    pub fn derivation_identity(&self) -> DerivationIdentity {
        DerivationIdentity {
            engine_cache_compatibility_id: self.engine.engine_cache_compatibility_id.clone(),
            toolchain_fingerprint: self.toolchain_fingerprint.clone(),
        }
    }

    fn validate_audio_artifacts(&self) -> Result<(), ValidationError> {
        const AUDIO: AudioSlot = AudioSlot {
            artifact_id: "audio-pcm",
            relative_ref: "output/audio/track.wav",
        };
        match self.artifacts.as_slice() {
            [audio] => audio.validate_audio_slot(0, &AUDIO),
            _ => Err(ValidationError::new(
                "artifacts",
                "must contain exactly one PCM audio artifact",
            )),
        }
    }

    fn validate_preview_artifacts(&self) -> Result<(), ValidationError> {
        const OPENING: PreviewSlot = PreviewSlot {
            artifact_id: "preview-opening",
            role: ArtifactRole::Opening,
            relative_ref: "output/preview/opening.jpg",
        };
        const MIDPOINT: PreviewSlot = PreviewSlot {
            artifact_id: "preview-midpoint",
            role: ArtifactRole::Midpoint,
            relative_ref: "output/preview/midpoint.jpg",
        };
        match self.artifacts.as_slice() {
            [opening] => opening.validate_preview_slot(0, &OPENING),
            [opening, midpoint] => {
                opening.validate_preview_slot(0, &OPENING)?;
                midpoint.validate_preview_slot(1, &MIDPOINT)
            }
            _ => Err(ValidationError::new(
                "artifacts",
                "must contain the opening preview and optionally the midpoint preview",
            )),
        }
    }
}

struct PreviewSlot {
    artifact_id: &'static str,
    role: ArtifactRole,
    relative_ref: &'static str,
}

struct AudioSlot {
    artifact_id: &'static str,
    relative_ref: &'static str,
}

impl Artifact {
    fn validate_preview_slot(
        &self,
        position: usize,
        slot: &PreviewSlot,
    ) -> Result<(), ValidationError> {
        let path = format!("artifacts[{position}]");
        if self.artifact_id.as_str() != slot.artifact_id {
            return Err(ValidationError::new(
                format!("{path}.artifactId"),
                format!("must be '{}'", slot.artifact_id),
            ));
        }
        if self.role != slot.role {
            return Err(ValidationError::new(
                format!("{path}.role"),
                "does not match the fixed preview order",
            ));
        }
        if self.kind != ArtifactKind::PreviewFrame {
            return Err(ValidationError::new(
                format!("{path}.kind"),
                "must be 'previewFrame'",
            ));
        }
        if self.media_type.as_str() != "image/jpeg" {
            return Err(ValidationError::new(
                format!("{path}.mediaType"),
                "must be 'image/jpeg'",
            ));
        }
        if self.relative_ref.as_str() != slot.relative_ref {
            return Err(ValidationError::new(
                format!("{path}.relativeRef"),
                format!("must be '{}'", slot.relative_ref),
            ));
        }
        if self.byte_size == 0 {
            return Err(ValidationError::new(
                format!("{path}.byteSize"),
                "must be positive for a preview frame",
            ));
        }
        if self.pixel_width.is_none() || self.pixel_height.is_none() {
            return Err(ValidationError::new(
                format!("{path}.pixelWidth/pixelHeight"),
                "must be known for a preview frame",
            ));
        }
        if self.audio_pcm.is_some() {
            return Err(ValidationError::new(
                format!("{path}.audioPcm"),
                "must be absent from preview artifacts",
            ));
        }
        match slot.role {
            ArtifactRole::Opening if self.requested_time_ms != 0 => {
                return Err(ValidationError::new(
                    format!("{path}.requestedTimeMs"),
                    "must be 0 for the opening preview",
                ));
            }
            ArtifactRole::Midpoint if self.requested_time_ms == 0 => {
                return Err(ValidationError::new(
                    format!("{path}.requestedTimeMs"),
                    "must be positive for the midpoint preview",
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_audio_slot(
        &self,
        position: usize,
        slot: &AudioSlot,
    ) -> Result<(), ValidationError> {
        let path = format!("artifacts[{position}]");
        if self.artifact_id.as_str() != slot.artifact_id {
            return Err(ValidationError::new(
                format!("{path}.artifactId"),
                format!("must be '{}'", slot.artifact_id),
            ));
        }
        if self.role != ArtifactRole::Audio {
            return Err(ValidationError::new(
                format!("{path}.role"),
                "must be 'audio'",
            ));
        }
        if self.kind != ArtifactKind::AudioPcm {
            return Err(ValidationError::new(
                format!("{path}.kind"),
                "must be 'audioPcm'",
            ));
        }
        if self.media_type.as_str() != "audio/wav" {
            return Err(ValidationError::new(
                format!("{path}.mediaType"),
                "must be 'audio/wav'",
            ));
        }
        if self.relative_ref.as_str() != slot.relative_ref {
            return Err(ValidationError::new(
                format!("{path}.relativeRef"),
                format!("must be '{}'", slot.relative_ref),
            ));
        }
        if self.byte_size == 0 {
            return Err(ValidationError::new(
                format!("{path}.byteSize"),
                "must be positive for an audio artifact",
            ));
        }
        if self.requested_time_ms != 0 {
            return Err(ValidationError::new(
                format!("{path}.requestedTimeMs"),
                "must be 0: the audio operation has no trim window in Protocol 0.1",
            ));
        }
        if self.presentation_time_ms.is_none() {
            return Err(ValidationError::new(
                format!("{path}.presentationTimeMs"),
                "must carry the material time of the first output sample",
            ));
        }
        if self.audio_pcm.is_none() {
            return Err(ValidationError::new(
                format!("{path}.audioPcm"),
                "must carry sampleRate, channels and sampleCount",
            ));
        }
        if self.pixel_width.is_some() || self.pixel_height.is_some() {
            return Err(ValidationError::new(
                format!("{path}.pixelWidth/pixelHeight"),
                "must be absent from audio artifacts",
            ));
        }
        Ok(())
    }
}

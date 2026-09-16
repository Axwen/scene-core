//! Shared engine/toolchain identities and the cache derivation descriptor.

use crate::canonical::{CanonicalJsonError, canonical_sha256};
use crate::error::{ValidationError, validate_safe_text};
use crate::operation::Operation;
use crate::values::{
    CacheCompatibilityId, CommitHash, DescriptorVersion, OutputContractVersion, ProtocolVersion,
    Sha256Digest,
};
use serde::{Deserialize, Serialize};

/// Shared identity of the engine build. Reused by events, Manifests, `version`
/// and `doctor`; no consumer may define an approximate variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EngineIdentity {
    pub engine_version: String,
    pub engine_commit: CommitHash,
    pub target: String,
    pub engine_cache_compatibility_id: CacheCompatibilityId,
    pub supported_protocol_versions: Vec<ProtocolVersion>,
    pub implemented_operations: Vec<Operation>,
}

impl EngineIdentity {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text("engine.engineVersion", &self.engine_version, 128)?;
        validate_target("engine.target", &self.target)?;
        if self.supported_protocol_versions.is_empty() {
            return Err(ValidationError::new(
                "engine.supportedProtocolVersions",
                "must not be empty",
            ));
        }
        validate_unique(
            "engine.supportedProtocolVersions",
            self.supported_protocol_versions
                .iter()
                .map(ProtocolVersion::as_str),
        )?;
        validate_unique(
            "engine.implementedOperations",
            self.implemented_operations.iter().map(Operation::as_str),
        )?;
        Ok(())
    }
}

/// Shared fixed-toolchain identity. The fingerprint covers target, FFmpeg and
/// FFprobe versions, build source, configure flags, capabilities and the
/// dynamic library closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolchainIdentity {
    pub toolchain_fingerprint: Sha256Digest,
    pub target: String,
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
    pub capability_set_fingerprint: Sha256Digest,
}

impl ToolchainIdentity {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_target("toolchain.target", &self.target)?;
        validate_safe_text("toolchain.ffmpegVersion", &self.ffmpeg_version, 256)?;
        validate_safe_text("toolchain.ffprobeVersion", &self.ffprobe_version, 256)?;
        Ok(())
    }
}

/// Engine-side identity pair required to recompute `derivationKey`. Kept
/// together so Host- and engine-side code cannot pass one without the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationIdentity {
    pub engine_cache_compatibility_id: CacheCompatibilityId,
    pub toolchain_fingerprint: Sha256Digest,
}

/// Cache derivation input. Excludes request/run/source/execution identity so
/// the same logical media work derives the same key on Host and Core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DerivationDescriptor {
    pub derivation_descriptor_version: DescriptorVersion,
    pub input_fingerprint: Sha256Digest,
    pub operation: Operation,
    pub operation_config_hash: Sha256Digest,
    pub output_contract_version: OutputContractVersion,
    pub engine_cache_compatibility_id: CacheCompatibilityId,
    pub toolchain_fingerprint: Sha256Digest,
}

impl DerivationDescriptor {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self
            .operation
            .is_registered_output_contract(&self.output_contract_version)
        {
            return Err(ValidationError::new(
                "outputContractVersion",
                "must be the registered result contract of the operation",
            ));
        }
        Ok(())
    }

    /// `sha256(RFC8785(DerivationDescriptor))`.
    pub fn derive_key(&self) -> Result<Sha256Digest, CanonicalJsonError> {
        let value = serde_json::to_value(self).expect("derivation descriptor serializes");
        canonical_sha256(&value)
    }
}

pub(crate) fn validate_target(path: &str, target: &str) -> Result<(), ValidationError> {
    if target.is_empty() || target.len() > 64 {
        return Err(ValidationError::new(path, "must be 1 to 64 bytes"));
    }
    if !target.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
    }) {
        return Err(ValidationError::new(
            path,
            "must use lowercase ASCII letters, digits, '-' or '_'",
        ));
    }
    if !target.contains('-') {
        return Err(ValidationError::new(path, "must be a target triple"));
    }
    Ok(())
}

pub(crate) fn validate_unique<'a>(
    path: &str,
    values: impl Iterator<Item = &'a str>,
) -> Result<(), ValidationError> {
    let mut seen = Vec::new();
    for value in values {
        if seen.contains(&value) {
            return Err(ValidationError::new(path, "must not contain duplicates"));
        }
        seen.push(value);
    }
    Ok(())
}

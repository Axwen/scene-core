//! Shared engine/toolchain identities and the cache derivation descriptor.

use crate::canonical::{CanonicalJsonError, canonical_sha256};
use crate::error::{ValidationError, validate_safe_text};
use crate::operation::Operation;
use crate::values::{
    CacheCompatibilityId, CommitHash, DescriptorVersion, OutputContractVersion, ProtocolVersion,
    RelativeRef, Sha256Digest,
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

/// Platform toolchain file shipped inside the bundle. Its SHA-256 over the
/// exact file bytes is the `toolchainFingerprint`; the descriptor itself never
/// carries that fingerprint, so it stays verifiable from the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolchainDescriptor {
    pub descriptor_version: DescriptorVersion,
    pub target: String,
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
    pub toolchain_lock_sha256: Sha256Digest,
    pub configure_flags: Vec<String>,
    pub capability_set_fingerprint: Sha256Digest,
    pub shared_libraries: Vec<ToolchainLibrary>,
}

impl ToolchainDescriptor {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_target("target", &self.target)?;
        validate_safe_text("ffmpegVersion", &self.ffmpeg_version, 256)?;
        validate_safe_text("ffprobeVersion", &self.ffprobe_version, 256)?;
        if self.configure_flags.is_empty() {
            return Err(ValidationError::new(
                "configureFlags",
                "must record the effective configure flags",
            ));
        }
        for flag in &self.configure_flags {
            if !flag.starts_with("--") {
                return Err(ValidationError::new(
                    "configureFlags",
                    "every entry must be a --flag",
                ));
            }
            if matches!(flag.as_str(), "--enable-gpl" | "--enable-nonfree") {
                return Err(ValidationError::new(
                    "configureFlags",
                    "the lgpl distribution profile forbids --enable-gpl and --enable-nonfree",
                ));
            }
        }
        if self.shared_libraries.is_empty() {
            return Err(ValidationError::new(
                "sharedLibraries",
                "must record the dynamic library closure",
            ));
        }
        validate_unique(
            "sharedLibraries[].path",
            self.shared_libraries
                .iter()
                .map(|library| library.path.as_str()),
        )?;
        Ok(())
    }

    /// Fingerprint of the exact descriptor file bytes.
    pub fn fingerprint(descriptor_bytes: &[u8]) -> Sha256Digest {
        Sha256Digest::from_bytes(descriptor_bytes)
    }

    /// Shared identity view consumed by events, Manifests and `version`/`doctor`.
    pub fn to_identity(&self, toolchain_fingerprint: Sha256Digest) -> ToolchainIdentity {
        ToolchainIdentity {
            toolchain_fingerprint,
            target: self.target.clone(),
            ffmpeg_version: self.ffmpeg_version.clone(),
            ffprobe_version: self.ffprobe_version.clone(),
            capability_set_fingerprint: self.capability_set_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolchainLibrary {
    pub path: RelativeRef,
    pub sha256: Sha256Digest,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(character: char) -> Sha256Digest {
        Sha256Digest::new(format!("sha256:{}", character.to_string().repeat(64))).expect("digest")
    }

    fn descriptor() -> ToolchainDescriptor {
        ToolchainDescriptor {
            descriptor_version: DescriptorVersion::current(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            ffmpeg_version: "n9.0.1-30-g9258bacca5".to_owned(),
            ffprobe_version: "n9.0.1-30-g9258bacca5".to_owned(),
            toolchain_lock_sha256: digest('9'),
            configure_flags: vec!["--enable-version3".to_owned(), "--enable-shared".to_owned()],
            capability_set_fingerprint: digest('b'),
            shared_libraries: vec![ToolchainLibrary {
                path: RelativeRef::new("bin/avcodec-63.dll").expect("ref"),
                sha256: digest('1'),
            }],
        }
    }

    #[test]
    fn descriptor_maps_to_the_shared_identity() {
        let descriptor = descriptor();
        assert!(descriptor.validate().is_ok());
        let fingerprint = ToolchainDescriptor::fingerprint(b"descriptor bytes");
        let identity = descriptor.to_identity(fingerprint.clone());
        assert_eq!(identity.toolchain_fingerprint, fingerprint);
        assert_eq!(identity.target, descriptor.target);
        assert_eq!(identity.ffmpeg_version, descriptor.ffmpeg_version);
        assert_eq!(identity.capability_set_fingerprint, digest('b'));
    }

    #[test]
    fn descriptor_rejects_gpl_flags_and_empty_closure() {
        let mut gpl = descriptor();
        gpl.configure_flags.push("--enable-gpl".to_owned());
        assert!(gpl.validate().is_err());

        let mut nonfree = descriptor();
        nonfree.configure_flags = vec!["--enable-nonfree".to_owned()];
        assert!(nonfree.validate().is_err());

        let mut empty = descriptor();
        empty.shared_libraries.clear();
        assert!(empty.validate().is_err());

        let mut bad_flag = descriptor();
        bad_flag.configure_flags = vec!["enable-shared".to_owned()];
        assert!(bad_flag.validate().is_err());
    }

    #[test]
    fn descriptor_fingerprint_is_the_file_digest() {
        let fingerprint = ToolchainDescriptor::fingerprint(b"{}");
        assert_eq!(fingerprint, Sha256Digest::from_bytes(b"{}"));
    }
}

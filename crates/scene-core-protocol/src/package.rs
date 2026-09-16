//! Bundle package manifest: per-file integrity and toolchain references.
//!
//! The manifest only provides integrity checking. The external trust anchor
//! (release metadata digest or signature) is verified outside the bundle and
//! is never part of this document.

use crate::error::{ValidationError, validate_safe_text};
use crate::identity::validate_target;
use crate::identity::validate_unique;
use crate::operation::Operation;
use crate::values::{
    CacheCompatibilityId, CommitHash, DescriptorVersion, LicenseExpression, ProtocolVersion,
    RelativeRef, Sha256Digest, ToolName,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Distribution license profile of the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DistributionProfile {
    Lgpl,
}

/// Engine entry of the package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackagedEngine {
    pub path: RelativeRef,
    pub version: String,
    pub commit: CommitHash,
    pub engine_cache_compatibility_id: CacheCompatibilityId,
    pub supported_protocol_versions: Vec<ProtocolVersion>,
    pub implemented_operations: Vec<Operation>,
}

impl PackagedEngine {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text("engine.version", &self.version, 128)?;
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

/// Fixed tool entry of the package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackagedTool {
    pub name: ToolName,
    pub path: RelativeRef,
    pub version: String,
    pub source_url: String,
    pub source_sha256: Sha256Digest,
    pub license_profile: LicenseExpression,
}

impl PackagedTool {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text("tools[].version", &self.version, 256)?;
        if !self.source_url.starts_with("https://") {
            return Err(ValidationError::new(
                "tools[].sourceUrl",
                "must be an immutable https URL",
            ));
        }
        if self.source_url.len() > 1024 {
            return Err(ValidationError::new("tools[].sourceUrl", "is too long"));
        }
        if self.source_url.chars().any(char::is_control) {
            return Err(ValidationError::new(
                "tools[].sourceUrl",
                "must not contain control characters",
            ));
        }
        Ok(())
    }
}

/// One file listed with its integrity data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackagedFile {
    pub path: RelativeRef,
    pub byte_size: u64,
    pub sha256: Sha256Digest,
}

/// Bundle package manifest. Never lists itself and never carries a timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PackageManifest {
    pub package_manifest_version: DescriptorVersion,
    pub name: String,
    pub package_version: String,
    pub target: String,
    pub distribution_profile: DistributionProfile,
    pub toolchain_fingerprint: Sha256Digest,
    pub toolchain_descriptor_ref: RelativeRef,
    pub capabilities_ref: RelativeRef,
    pub engine: PackagedEngine,
    pub tools: Vec<PackagedTool>,
    pub files: Vec<PackagedFile>,
    pub sbom_ref: RelativeRef,
    pub third_party_licenses_ref: RelativeRef,
}

impl PackageManifest {
    pub const SELF_PATH: &'static str = "package-manifest.json";

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text("name", &self.name, 64)?;
        validate_safe_text("packageVersion", &self.package_version, 64)?;
        validate_target("target", &self.target)?;
        self.engine.validate()?;

        if self.tools.len() != 2 {
            return Err(ValidationError::new(
                "tools",
                "package manifest version 1 lists ffmpeg and ffprobe in order",
            ));
        }
        for (expected, tool) in ["ffmpeg", "ffprobe"].iter().zip(&self.tools) {
            if tool.name.as_str() != *expected {
                return Err(ValidationError::new(
                    "tools[].name",
                    "package manifest version 1 lists ffmpeg and ffprobe in order",
                ));
            }
            tool.validate()?;
        }
        validate_unique(
            "tools[].name",
            self.tools.iter().map(|tool| tool.name.as_str()),
        )?;

        if self.files.is_empty() {
            return Err(ValidationError::new("files", "must not be empty"));
        }
        validate_unique(
            "files[].path",
            self.files.iter().map(|file| file.path.as_str()),
        )?;
        for (position, pair) in self.files.windows(2).enumerate() {
            if pair[0].path.as_str().as_bytes() >= pair[1].path.as_str().as_bytes() {
                return Err(ValidationError::new(
                    format!("files[{position}]"),
                    "files must be sorted by path byte order without duplicates",
                ));
            }
        }
        if self
            .files
            .iter()
            .any(|file| file.path.as_str() == Self::SELF_PATH)
        {
            return Err(ValidationError::new(
                "files[].path",
                "must not list the package manifest itself",
            ));
        }

        let listed: BTreeSet<&str> = self.files.iter().map(|file| file.path.as_str()).collect();
        let required = [
            self.engine.path.as_str(),
            self.tools[0].path.as_str(),
            self.tools[1].path.as_str(),
            self.toolchain_descriptor_ref.as_str(),
            self.capabilities_ref.as_str(),
            self.sbom_ref.as_str(),
        ];
        for path in required {
            if !listed.contains(path) {
                return Err(ValidationError::new(
                    "files",
                    format!("must list the referenced bundle file '{path}'"),
                ));
            }
        }
        Ok(())
    }

    /// Finds a listed file by relative path.
    pub fn file(&self, path: &str) -> Option<&PackagedFile> {
        self.files.iter().find(|file| file.path.as_str() == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(character: char) -> Sha256Digest {
        Sha256Digest::new(format!("sha256:{}", character.to_string().repeat(64))).expect("digest")
    }

    fn file(path: &str) -> PackagedFile {
        PackagedFile {
            path: RelativeRef::new(path).expect("ref"),
            byte_size: 10,
            sha256: digest('1'),
        }
    }

    fn tool(name: &str, path: &str) -> PackagedTool {
        PackagedTool {
            name: ToolName::new(name).expect("tool name"),
            path: RelativeRef::new(path).expect("ref"),
            version: "7.1.1".to_owned(),
            source_url: "https://example.com/ffmpeg-7.1.1.tar.xz".to_owned(),
            source_sha256: digest('2'),
            license_profile: LicenseExpression::new("LGPL-2.1-or-later").expect("license"),
        }
    }

    fn manifest() -> PackageManifest {
        PackageManifest {
            package_manifest_version: DescriptorVersion::current(),
            name: "scene-core".to_owned(),
            package_version: "0.1.0-alpha.1".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            distribution_profile: DistributionProfile::Lgpl,
            toolchain_fingerprint: digest('a'),
            toolchain_descriptor_ref: RelativeRef::new("toolchain-descriptor.json").expect("ref"),
            capabilities_ref: RelativeRef::new("capabilities.json").expect("ref"),
            engine: PackagedEngine {
                path: RelativeRef::new("bin/scene-core.exe").expect("ref"),
                version: "0.1.0-alpha.1".to_owned(),
                commit: CommitHash::new("0123456789abcdef0123456789abcdef01234567")
                    .expect("commit"),
                engine_cache_compatibility_id: CacheCompatibilityId::new("scene-core-output-v1")
                    .expect("cache id"),
                supported_protocol_versions: vec![ProtocolVersion::current()],
                implemented_operations: Vec::new(),
            },
            tools: vec![
                tool("ffmpeg", "bin/ffmpeg.exe"),
                tool("ffprobe", "bin/ffprobe.exe"),
            ],
            files: vec![
                file("SBOM.spdx.json"),
                file("bin/ffmpeg.exe"),
                file("bin/ffprobe.exe"),
                file("bin/scene-core.exe"),
                file("capabilities.json"),
                file("toolchain-descriptor.json"),
            ],
            sbom_ref: RelativeRef::new("SBOM.spdx.json").expect("ref"),
            third_party_licenses_ref: RelativeRef::new("THIRD_PARTY_LICENSES").expect("ref"),
        }
    }

    #[test]
    fn valid_manifest_passes() {
        assert!(manifest().validate().is_ok());
    }

    #[test]
    fn self_reference_is_rejected() {
        let mut value = manifest();
        value.files.push(file(PackageManifest::SELF_PATH));
        value.files.sort_by(|left, right| {
            left.path
                .as_str()
                .as_bytes()
                .cmp(right.path.as_str().as_bytes())
        });
        assert!(value.validate().is_err());
    }

    #[test]
    fn unsorted_files_are_rejected() {
        let mut value = manifest();
        value.files.swap(0, 1);
        assert!(value.validate().is_err());
    }

    #[test]
    fn referenced_engine_must_be_listed() {
        let mut value = manifest();
        value
            .files
            .retain(|entry| entry.path.as_str() != "bin/scene-core.exe");
        assert!(value.validate().is_err());
    }

    #[test]
    fn tool_order_and_source_scheme_are_frozen() {
        let mut swapped = manifest();
        swapped.tools.swap(0, 1);
        assert!(swapped.validate().is_err());

        let mut insecure = manifest();
        insecure.tools[0].source_url = "http://example.com/ffmpeg.tar.xz".to_owned();
        assert!(insecure.validate().is_err());
    }
}

//! Standalone CLI output contracts for `version --json` and `doctor --json`.
//!
//! These outputs never use the Host-facing request protocol. Each command
//! writes exactly one JSON object to stdout; doctor failures map to a stable
//! non-zero exit code derived from the first failed check.

use crate::error::{ErrorCode, ValidationError, validate_safe_text};
use crate::identity::{EngineIdentity, ToolchainIdentity, validate_target};
use crate::operation::Operation;
use crate::values::{
    CacheCompatibilityId, CommitHash, DescriptorVersion, DoctorCheckName, ProtocolVersion,
    Sha256Digest,
};
use serde::{Deserialize, Serialize};

/// `version --json` output. Mirrors the shared `EngineIdentity` plus the
/// toolchain fingerprint and a CLI schema version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VersionOutput {
    pub schema_version: DescriptorVersion,
    pub engine_version: String,
    pub engine_commit: CommitHash,
    pub target: String,
    pub engine_cache_compatibility_id: CacheCompatibilityId,
    pub toolchain_fingerprint: Sha256Digest,
    pub supported_protocol_versions: Vec<ProtocolVersion>,
    pub implemented_operations: Vec<Operation>,
}

impl VersionOutput {
    pub fn from_engine_identity(
        engine: &EngineIdentity,
        toolchain_fingerprint: Sha256Digest,
    ) -> Self {
        Self {
            schema_version: DescriptorVersion::current(),
            engine_version: engine.engine_version.clone(),
            engine_commit: engine.engine_commit.clone(),
            target: engine.target.clone(),
            engine_cache_compatibility_id: engine.engine_cache_compatibility_id.clone(),
            toolchain_fingerprint,
            supported_protocol_versions: engine.supported_protocol_versions.clone(),
            implemented_operations: engine.implemented_operations.clone(),
        }
    }

    /// Rebuilds the shared identity represented by this output.
    pub fn engine_identity(&self) -> EngineIdentity {
        EngineIdentity {
            engine_version: self.engine_version.clone(),
            engine_commit: self.engine_commit.clone(),
            target: self.target.clone(),
            engine_cache_compatibility_id: self.engine_cache_compatibility_id.clone(),
            supported_protocol_versions: self.supported_protocol_versions.clone(),
            implemented_operations: self.implemented_operations.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_target("target", &self.target)?;
        let engine = self.engine_identity();
        engine.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
    Ok,
    Failed,
}

/// Stable doctor failure codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DoctorCheckCode {
    PackageManifestUnreadable,
    PackageManifestInvalid,
    TrustAnchorMissing,
    ManifestDigestMismatch,
    BundleFileInvalid,
    ToolchainDescriptorInvalid,
    EngineExecutableInvalid,
    EngineVersionMismatch,
    ToolExecutableInvalid,
    ToolVersionMismatch,
    CapabilitiesInvalid,
    TempDirUnavailable,
}

impl DoctorCheckCode {
    /// Process exit code for the first failed check.
    pub const fn exit_code(self) -> u8 {
        match self {
            DoctorCheckCode::TempDirUnavailable => 3,
            _ => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DoctorCheck {
    pub name: DoctorCheckName,
    pub status: DoctorStatus,
    #[serde(default)]
    pub code: Option<DoctorCheckCode>,
    pub message: String,
    #[serde(default)]
    pub next_step: Option<String>,
}

impl DoctorCheck {
    pub fn ok(name: DoctorCheckName, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorStatus::Ok,
            code: None,
            message: message.into(),
            next_step: None,
        }
    }

    pub fn failed(
        name: DoctorCheckName,
        code: DoctorCheckCode,
        message: impl Into<String>,
        next_step: impl Into<String>,
    ) -> Self {
        Self {
            name,
            status: DoctorStatus::Failed,
            code: Some(code),
            message: message.into(),
            next_step: Some(next_step.into()),
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text(
            &format!("checks[{}].message", self.name),
            &self.message,
            1024,
        )?;
        if let Some(next_step) = &self.next_step {
            validate_safe_text(&format!("checks[{}].nextStep", self.name), next_step, 1024)?;
        }
        match self.status {
            DoctorStatus::Ok => {
                if self.code.is_some() {
                    return Err(ValidationError::new(
                        format!("checks[{}].code", self.name),
                        "must be null for ok checks",
                    ));
                }
            }
            DoctorStatus::Failed => {
                if self.code.is_none() {
                    return Err(ValidationError::new(
                        format!("checks[{}].code", self.name),
                        "failed checks require a stable code",
                    ));
                }
                if self.next_step.is_none() {
                    return Err(ValidationError::new(
                        format!("checks[{}].nextStep", self.name),
                        "failed checks require an actionable nextStep",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The ten `doctor` check names in their frozen report order (spec §10).
pub const DOCTOR_CHECK_NAMES: [&str; 10] = [
    "package-manifest",
    "manifest-trust-anchor",
    "toolchain-descriptor",
    "bundle-files",
    "engine-executable",
    "engine-version",
    "tool-executables",
    "tool-versions",
    "capabilities",
    "temp-dir",
];

/// `doctor --json` output with ordered, stable checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DoctorOutput {
    pub schema_version: DescriptorVersion,
    pub status: DoctorStatus,
    pub engine: EngineIdentity,
    pub toolchain: Option<ToolchainIdentity>,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorOutput {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.engine.validate()?;
        match (&self.toolchain, self.status) {
            (Some(toolchain), _) => toolchain.validate()?,
            (None, DoctorStatus::Ok) => {
                return Err(ValidationError::new(
                    "toolchain",
                    "must be present when every check passed",
                ));
            }
            (None, DoctorStatus::Failed) => {}
        }
        if self.checks.is_empty() {
            return Err(ValidationError::new("checks", "must not be empty"));
        }
        for (position, check) in self.checks.iter().enumerate() {
            let Some(expected) = DOCTOR_CHECK_NAMES.get(position) else {
                return Err(ValidationError::new(
                    "checks",
                    "must not contain checks beyond the registered ten",
                ));
            };
            if check.name.as_str() != *expected {
                return Err(ValidationError::new(
                    format!("checks[{position}].name"),
                    format!("must be '{expected}' in the frozen doctor order"),
                ));
            }
            check.validate()?;
        }
        let any_failed = self
            .checks
            .iter()
            .any(|check| check.status == DoctorStatus::Failed);
        match (self.status, any_failed) {
            (DoctorStatus::Ok, true) => Err(ValidationError::new(
                "status",
                "must be failed when any check failed",
            )),
            (DoctorStatus::Failed, false) => Err(ValidationError::new(
                "status",
                "must be failed only when a check failed",
            )),
            _ => Ok(()),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status == DoctorStatus::Ok
    }

    /// Exit code of the first failed check, `0` when everything passed.
    pub fn exit_code(&self) -> u8 {
        self.checks
            .iter()
            .find(|check| check.status == DoctorStatus::Failed)
            .and_then(|check| check.code)
            .map(DoctorCheckCode::exit_code)
            .unwrap_or(0)
    }
}

/// Controlled diagnostic failure for the standalone CLI. The engine writes
/// exactly one such object to stdout when it cannot produce a normal command
/// output, then exits non-zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CliErrorOutput {
    pub schema_version: DescriptorVersion,
    pub code: ErrorCode,
    pub message: String,
    pub next_step: String,
}

impl CliErrorOutput {
    pub fn new(code: ErrorCode, message: impl Into<String>, next_step: impl Into<String>) -> Self {
        Self {
            schema_version: DescriptorVersion::current(),
            code,
            message: message.into(),
            next_step: next_step.into(),
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_safe_text("message", &self.message, 1024)?;
        validate_safe_text("nextStep", &self.next_step, 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::DoctorCheckName;

    fn engine() -> EngineIdentity {
        EngineIdentity {
            engine_version: "0.1.0-alpha.1".to_owned(),
            engine_commit: CommitHash::new("0123456789abcdef0123456789abcdef01234567")
                .expect("commit"),
            target: "x86_64-pc-windows-msvc".to_owned(),
            engine_cache_compatibility_id: CacheCompatibilityId::new("scene-core-output-v1")
                .expect("cache id"),
            supported_protocol_versions: vec![ProtocolVersion::current()],
            implemented_operations: Vec::new(),
        }
    }

    fn digest(character: char) -> Sha256Digest {
        Sha256Digest::new(format!("sha256:{}", character.to_string().repeat(64))).expect("digest")
    }

    fn toolchain() -> ToolchainIdentity {
        ToolchainIdentity {
            toolchain_fingerprint: digest('a'),
            target: "x86_64-pc-windows-msvc".to_owned(),
            ffmpeg_version: "7.1.1".to_owned(),
            ffprobe_version: "7.1.1".to_owned(),
            capability_set_fingerprint: digest('b'),
        }
    }

    fn check_name(value: &str) -> DoctorCheckName {
        DoctorCheckName::new(value).expect("check name")
    }

    #[test]
    fn version_output_roundtrips_the_shared_identity() {
        let output = VersionOutput::from_engine_identity(&engine(), digest('c'));
        assert!(output.validate().is_ok());
        assert_eq!(output.engine_identity(), engine());

        let value = serde_json::to_value(&output).expect("serializes");
        assert_eq!(value["schemaVersion"], "1");
        assert_eq!(value["engineVersion"], "0.1.0-alpha.1");
        assert_eq!(value["implementedOperations"], serde_json::json!([]));
        let parsed: VersionOutput = serde_json::from_value(value).expect("parses");
        assert_eq!(parsed, output);
    }

    #[test]
    fn doctor_status_must_match_checks() {
        let ok = DoctorOutput {
            schema_version: DescriptorVersion::current(),
            status: DoctorStatus::Ok,
            engine: engine(),
            toolchain: Some(toolchain()),
            checks: vec![DoctorCheck::ok(check_name("package-manifest"), "readable")],
        };
        assert!(ok.validate().is_ok());
        assert!(ok.is_ok());
        assert_eq!(ok.exit_code(), 0);

        let mut mismatched = ok.clone();
        mismatched.status = DoctorStatus::Failed;
        assert!(mismatched.validate().is_err());
    }

    fn ok_checks_through(position: usize) -> Vec<DoctorCheck> {
        DOCTOR_CHECK_NAMES[..position]
            .iter()
            .map(|name| DoctorCheck::ok(check_name(name), "ok"))
            .collect()
    }

    fn output_with(checks: Vec<DoctorCheck>, status: DoctorStatus) -> DoctorOutput {
        DoctorOutput {
            schema_version: DescriptorVersion::current(),
            status,
            engine: engine(),
            toolchain: Some(toolchain()),
            checks,
        }
    }

    #[test]
    fn failed_checks_require_code_and_next_step() {
        let failed = DoctorCheck::failed(
            check_name("bundle-files"),
            DoctorCheckCode::BundleFileInvalid,
            "a listed file does not match its recorded hash",
            "restore the file from the trusted release archive",
        );
        let mut checks = ok_checks_through(3);
        checks.push(failed.clone());
        let output = output_with(checks, DoctorStatus::Failed);
        assert!(output.validate().is_ok());
        assert_eq!(output.exit_code(), 2);

        let mut missing_next_step = failed.clone();
        missing_next_step.next_step = None;
        assert!(missing_next_step.validate().is_err());

        let mut missing_code = failed;
        missing_code.code = None;
        assert!(missing_code.validate().is_err());

        let mut checks = ok_checks_through(9);
        checks.push(DoctorCheck::failed(
            check_name("temp-dir"),
            DoctorCheckCode::TempDirUnavailable,
            "the temporary directory could not be created",
            "check free space and directory permissions",
        ));
        assert_eq!(output_with(checks, DoctorStatus::Failed).exit_code(), 3);
    }

    #[test]
    fn doctor_checks_must_follow_the_frozen_order() {
        let mut reordered = ok_checks_through(1);
        reordered.push(DoctorCheck::ok(check_name("bundle-files"), "ok"));
        assert!(output_with(reordered, DoctorStatus::Ok).validate().is_err());

        let renamed = output_with(
            vec![DoctorCheck::ok(check_name("manifest-trust-anchor"), "ok")],
            DoctorStatus::Ok,
        );
        assert!(renamed.validate().is_err());

        let mut extended = ok_checks_through(10);
        extended.push(DoctorCheck::ok(check_name("package-manifest"), "ok"));
        assert!(output_with(extended, DoctorStatus::Ok).validate().is_err());

        let mut ordered_failure = ok_checks_through(1);
        ordered_failure.push(DoctorCheck::failed(
            check_name("manifest-trust-anchor"),
            DoctorCheckCode::TrustAnchorMissing,
            "missing",
            "provide the trusted digest",
        ));
        assert!(
            output_with(ordered_failure, DoctorStatus::Failed)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn ok_checks_must_not_carry_codes() {
        let mut check = DoctorCheck::ok(check_name("package-manifest"), "readable");
        check.code = Some(DoctorCheckCode::PackageManifestInvalid);
        assert!(check.validate().is_err());
    }
}

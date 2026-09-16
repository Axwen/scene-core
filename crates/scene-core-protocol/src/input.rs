//! Ordered logical input identity.
//!
//! `contentHash` is a single file's SHA-256; `inputFingerprint` is the
//! versioned logical input-set digest and never a file hash, path or
//! `sourceVersionId`.

use crate::canonical::{CanonicalJsonError, canonical_sha256};
use crate::error::{ProtocolError, ProtocolResult, ValidationError};
use crate::identity::validate_unique;
use crate::values::{DescriptorVersion, InputRef, InputRole, Sha256Digest};
use serde::{Deserialize, Serialize};

/// Maximum accepted `byteSize` for one input, matching the staging contract.
pub const MAX_INPUT_BYTE_SIZE: u64 = 21_474_836_480;

/// One staged logical input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputDescriptor {
    pub role: InputRole,
    #[serde(rename = "ref")]
    pub input_ref: InputRef,
    pub content_hash: Sha256Digest,
    pub byte_size: u64,
}

impl InputDescriptor {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.byte_size > MAX_INPUT_BYTE_SIZE {
            return Err(ValidationError::new(
                "inputs[].byteSize",
                "must not exceed 21474836480 bytes",
            ));
        }
        Ok(())
    }
}

/// Versioned logical input set. `0.1` allows exactly one `source_media`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputSetDescriptor {
    pub input_set_version: DescriptorVersion,
    pub inputs: Vec<InputDescriptor>,
}

impl InputSetDescriptor {
    /// Builds an `inputSetVersion: "1"` descriptor from wire inputs.
    pub fn from_inputs(inputs: Vec<InputDescriptor>) -> Self {
        Self {
            input_set_version: DescriptorVersion::current(),
            inputs,
        }
    }

    /// Protocol 0.1 rules: exactly one `source_media`, unique roles and the
    /// byte-size bound. `inputSetVersion` is fixed by its type.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_unique(
            "inputs[].role",
            self.inputs.iter().map(|input| input.role.as_str()),
        )?;
        for input in &self.inputs {
            input.validate()?;
        }
        if self.inputs.len() != 1 || !self.inputs[0].role.is_source_media() {
            return Err(ValidationError::new(
                "inputs",
                "Protocol 0.1 requires exactly one source_media input",
            ));
        }
        Ok(())
    }

    /// Canonical view with inputs sorted by `role` UTF-8 bytes.
    pub fn canonical_value(&self) -> serde_json::Value {
        let mut inputs: Vec<&InputDescriptor> = self.inputs.iter().collect();
        inputs.sort_by(|left, right| {
            left.role
                .as_str()
                .as_bytes()
                .cmp(right.role.as_str().as_bytes())
        });
        let inputs: Vec<serde_json::Value> = inputs
            .into_iter()
            .map(|input| {
                serde_json::json!({
                    "role": input.role.as_str(),
                    "contentHash": input.content_hash.as_str(),
                    "byteSize": input.byte_size,
                })
            })
            .collect();
        serde_json::json!({
            "inputSetVersion": self.input_set_version.as_str(),
            "inputs": inputs,
        })
    }

    /// `sha256(RFC8785(InputSetDescriptor))`.
    pub fn input_fingerprint(&self) -> Result<Sha256Digest, CanonicalJsonError> {
        canonical_sha256(&self.canonical_value())
    }
}

/// Actual facts of one staged input observed by the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSourceFacts {
    pub byte_size: u64,
    pub content_hash: Sha256Digest,
}

/// Compares declared input identity against the staged snapshot. A missing
/// staging entry is `INPUT_NOT_FOUND`; a size or hash difference is
/// `INPUT_CHANGED`.
pub fn verify_staged_source(
    declared: &InputDescriptor,
    actual: Option<&StagedSourceFacts>,
) -> ProtocolResult<()> {
    let Some(actual) = actual else {
        return Err(Box::new(
            ProtocolError::input_not_found("the staged input/source.media is missing")
                .with_stage("staging")
                .with_next_step("stage the input snapshot again and retry."),
        ));
    };
    if actual.byte_size != declared.byte_size {
        return Err(Box::new(
            ProtocolError::input_changed(
                "the staged input size differs from the declared snapshot",
            )
            .with_stage("staging")
            .with_limit_actual(
                saturating_i64(declared.byte_size),
                saturating_i64(actual.byte_size),
            )
            .with_next_step("recompute the input fingerprint before retrying."),
        ));
    }
    if actual.content_hash != declared.content_hash {
        return Err(Box::new(
            ProtocolError::input_changed(
                "the staged input bytes differ from the declared snapshot",
            )
            .with_stage("staging")
            .with_next_step("recompute the input fingerprint before retrying."),
        ));
    }
    Ok(())
}

/// Recomputes the logical input fingerprint and compares it with the declared
/// value. A digest mismatch is `INVALID_REQUEST`.
pub fn verify_input_fingerprint(
    descriptor: &InputSetDescriptor,
    declared: &Sha256Digest,
) -> ProtocolResult<()> {
    descriptor.validate().map_err(|error| {
        Box::new(
            ProtocolError::invalid_request("the input descriptor is invalid")
                .with_cause(error.to_string()),
        )
    })?;
    let computed = descriptor.input_fingerprint().map_err(|error| {
        Box::new(
            ProtocolError::invalid_request("the input set cannot be canonicalized")
                .with_cause(error.to_string()),
        )
    })?;
    if computed != *declared {
        return Err(Box::new(
            ProtocolError::invalid_request(
                "inputFingerprint does not match the recomputed logical input set digest",
            )
            .with_stage("validation")
            .with_next_step("recompute inputFingerprint as sha256(RFC8785(InputSetDescriptor))."),
        ));
    }
    Ok(())
}

fn saturating_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

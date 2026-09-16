//! Host-facing control messages and the control-line parser.
//!
//! A control line is one UTF-8 JSON object without BOM, at most 1 MiB, with
//! unknown fields, duplicate keys, illegal enums and out-of-range values
//! rejected. Framing primitives live in [`crate::framing`].

use crate::MAX_CONTROL_LINE_BYTES;
use crate::canonical::{CanonicalJsonError, canonical_sha256};
use crate::error::{ProtocolError, ProtocolResult, ValidationError};
use crate::execution::ExecutionContext;
use crate::framing::parse_strict_json;
use crate::identity::{DerivationDescriptor, DerivationIdentity};
use crate::input::{InputDescriptor, InputSetDescriptor, verify_input_fingerprint};
use crate::operation::{Operation, OperationOptions};
use crate::values::{Identifier, OutputContractVersion, ProtocolVersion, Sha256Digest};
use serde::{Deserialize, Serialize};

/// Default `deadlineMs` when the Host omits it.
pub const DEFAULT_DEADLINE_MS: u64 = 120_000;
/// Inclusive upper bound for `deadlineMs`.
pub const MAX_DEADLINE_MS: u64 = 600_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum StartMessageType {
    #[serde(rename = "start")]
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CancelMessageType {
    #[serde(rename = "cancel")]
    Cancel,
}

/// Host-facing start request. `executionContext` is required; `version` and
/// `doctor` never use this DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartRequest {
    pub engine_protocol_version: ProtocolVersion,
    pub message_type: StartMessageType,
    pub request_id: Identifier,
    pub operation: Operation,
    pub source_version_id: Identifier,
    pub inputs: Vec<InputDescriptor>,
    pub input_fingerprint: Sha256Digest,
    pub operation_config_hash: Sha256Digest,
    pub output_contract_version: OutputContractVersion,
    pub derivation_key: Sha256Digest,
    pub execution_context: ExecutionContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_ms: Option<u64>,
    pub options: OperationOptions,
}

impl StartRequest {
    /// Syntactic strict parse against Protocol 0.1.
    pub fn parse(line: &str) -> ProtocolResult<Self> {
        parse_strict_json(line, "StartRequest")
    }

    /// Strict parse plus [`StartRequest::validate`].
    pub fn parse_and_validate(line: &str) -> ProtocolResult<Self> {
        let request = Self::parse(line)?;
        request.validate()?;
        Ok(request)
    }

    /// Effective `deadlineMs` after applying the protocol default.
    pub fn effective_deadline_ms(&self) -> u64 {
        self.deadline_ms.unwrap_or(DEFAULT_DEADLINE_MS)
    }

    /// `sha256(RFC8785(effective options))`. Protocol 0.1 registers no
    /// configurable options, so the effective options are always `{}`.
    pub fn compute_operation_config_hash(&self) -> Result<Sha256Digest, CanonicalJsonError> {
        let options =
            serde_json::to_value(self.options).expect("operation options serialize as an object");
        canonical_sha256(&options)
    }

    /// Pure semantic validation that does not need engine identity.
    pub fn validate(&self) -> ProtocolResult<()> {
        if !self.engine_protocol_version.is_supported() {
            return Err(Box::new(
                ProtocolError::unsupported_protocol(
                    "engineProtocolVersion is not supported by this engine",
                )
                .with_stage("validation")
                .with_next_step(
                    "use one of the supportedProtocolVersions reported by 'version --json'.",
                ),
            ));
        }
        let computed_config_hash = self.compute_operation_config_hash().map_err(|error| {
            Box::new(
                ProtocolError::invalid_request("options cannot be canonicalized")
                    .with_cause(format!("operation config hash is unavailable: {error}"))
                    .with_stage("validation"),
            )
        })?;
        if computed_config_hash != self.operation_config_hash {
            return Err(Box::new(
                ProtocolError::invalid_request(
                    "operationConfigHash does not match the effective operation options",
                )
                .with_stage("validation")
                .with_next_step(
                    "recompute operationConfigHash as sha256(RFC8785(effective options)).",
                ),
            ));
        }
        if !self
            .operation
            .is_registered_output_contract(&self.output_contract_version)
        {
            return Err(Box::new(
                ProtocolError::invalid_request(
                    "outputContractVersion is not registered for the operation in Protocol 0.1",
                )
                .with_stage("validation")
                .with_next_step("use the registered result contract of the requested operation."),
            ));
        }
        self.execution_context.validate().map_err(|error| {
            Box::new(
                ProtocolError::invalid_request("executionContext is invalid")
                    .with_cause(error.to_string())
                    .with_stage("validation")
                    .with_next_step("send runId, generation >= 0, attempt >= 1 and a valid scope."),
            )
        })?;
        if let Some(deadline) = self.deadline_ms {
            if !(1..=MAX_DEADLINE_MS).contains(&deadline) {
                return Err(Box::new(
                    ProtocolError::invalid_request("deadlineMs is out of range")
                        .with_stage("validation")
                        .with_limit_actual(
                            i64::try_from(MAX_DEADLINE_MS).expect("fits"),
                            i64::try_from(deadline).unwrap_or(i64::MAX),
                        )
                        .with_next_step("use a deadlineMs between 1 and 600000."),
                ));
            }
        }
        let descriptor = InputSetDescriptor::from_inputs(self.inputs.clone());
        verify_input_fingerprint(&descriptor, &self.input_fingerprint)
    }

    /// Builds the derivation descriptor from engine-side identity.
    pub fn derivation_descriptor(
        &self,
        identity: &DerivationIdentity,
    ) -> ProtocolResult<DerivationDescriptor> {
        let descriptor = DerivationDescriptor {
            derivation_descriptor_version: crate::values::DescriptorVersion::current(),
            input_fingerprint: self.input_fingerprint.clone(),
            operation: self.operation,
            operation_config_hash: self.operation_config_hash.clone(),
            output_contract_version: self.output_contract_version.clone(),
            engine_cache_compatibility_id: identity.engine_cache_compatibility_id.clone(),
            toolchain_fingerprint: identity.toolchain_fingerprint.clone(),
        };
        descriptor.validate().map_err(|error| {
            Box::new(
                ProtocolError::invalid_request("derivation descriptor is invalid")
                    .with_cause(error.to_string())
                    .with_stage("validation"),
            )
        })?;
        Ok(descriptor)
    }

    /// Recomputes `derivationKey` with this engine's identity and compares it
    /// with the declared value. Call after [`StartRequest::validate`].
    pub fn validate_derivation(&self, identity: &DerivationIdentity) -> ProtocolResult<()> {
        let descriptor = self.derivation_descriptor(identity)?;
        let computed = descriptor
            .derive_key()
            .expect("derivation descriptor serializes");
        if computed != self.derivation_key {
            return Err(Box::new(
                ProtocolError::invalid_request(
                    "derivationKey does not match the recomputed derivation descriptor",
                )
                .with_stage("validation")
                .with_next_step(
                    "recompute derivationKey as sha256(RFC8785(DerivationDescriptor)).",
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CancelRequest {
    pub engine_protocol_version: ProtocolVersion,
    pub message_type: CancelMessageType,
    pub request_id: Identifier,
}

impl CancelRequest {
    pub fn parse(line: &str) -> ProtocolResult<Self> {
        parse_strict_json(line, "CancelRequest")
    }

    pub fn parse_and_validate(line: &str) -> ProtocolResult<Self> {
        let request = Self::parse(line)?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> ProtocolResult<()> {
        if !self.engine_protocol_version.is_supported() {
            return Err(Box::new(
                ProtocolError::unsupported_protocol(
                    "engineProtocolVersion is not supported by this engine",
                )
                .with_stage("validation"),
            ));
        }
        Ok(())
    }
}

/// First control line must be a `StartRequest`; a later line may be a matching
/// `CancelRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlMessage {
    Start(Box<StartRequest>),
    Cancel(CancelRequest),
}

impl ControlMessage {
    pub const fn message_type(&self) -> &'static str {
        match self {
            ControlMessage::Start(_) => "start",
            ControlMessage::Cancel(_) => "cancel",
        }
    }

    pub fn request_id(&self) -> &Identifier {
        match self {
            ControlMessage::Start(request) => &request.request_id,
            ControlMessage::Cancel(request) => &request.request_id,
        }
    }
}

/// Framing error for a JSONL line that exceeds [`MAX_CONTROL_LINE_BYTES`].
/// Shared by the parser and by readers that cap lines while reading.
pub fn control_line_limit_error(actual_bytes: usize) -> ProtocolError {
    ProtocolError::invalid_request("control JSONL line exceeds the 1 MiB hard limit")
        .with_stage("framing")
        .with_limit_actual(
            i64::try_from(MAX_CONTROL_LINE_BYTES).expect("fits"),
            i64::try_from(actual_bytes).unwrap_or(i64::MAX),
        )
}

/// Parses one control JSONL line, rejecting BOM, oversized lines and unknown
/// message types. Semantic validation is separate.
pub fn parse_control_line(line: &str) -> ProtocolResult<ControlMessage> {
    if line.len() > MAX_CONTROL_LINE_BYTES {
        return Err(Box::new(control_line_limit_error(line.len())));
    }
    let probe: MessageTypeProbe = parse_strict_json(line, "control message")?;
    match probe.message_type.as_str() {
        "start" => Ok(ControlMessage::Start(Box::new(StartRequest::parse(line)?))),
        "cancel" => Ok(ControlMessage::Cancel(CancelRequest::parse(line)?)),
        _ => Err(Box::new(
            ProtocolError::invalid_request("messageType must be 'start' or 'cancel'")
                .with_stage("framing")
                .with_next_step(
                    "send one StartRequest first and at most one matching CancelRequest.",
                ),
        )),
    }
}

#[derive(Deserialize)]
struct MessageTypeProbe {
    #[serde(rename = "messageType")]
    message_type: String,
}

/// Pure validator for one request's control stream: the first line must be the
/// single `StartRequest`, followed by at most one `CancelRequest` that reuses
/// the same `requestId`. EOF is not a cancel.
#[derive(Debug, Default)]
pub struct ControlStreamValidator {
    request_id: Option<Identifier>,
    cancelled: bool,
}

impl ControlStreamValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accept(&mut self, message: &ControlMessage) -> Result<(), ValidationError> {
        match message {
            ControlMessage::Start(start) => {
                if self.request_id.is_some() {
                    return Err(ValidationError::new(
                        "messageType",
                        "the first line must be the only StartRequest",
                    ));
                }
                self.request_id = Some(start.request_id.clone());
            }
            ControlMessage::Cancel(cancel) => {
                let Some(expected) = &self.request_id else {
                    return Err(ValidationError::new(
                        "messageType",
                        "the first line must be a StartRequest",
                    ));
                };
                if self.cancelled {
                    return Err(ValidationError::new(
                        "messageType",
                        "at most one CancelRequest is allowed",
                    ));
                }
                if &cancel.request_id != expected {
                    return Err(ValidationError::new(
                        "requestId",
                        "cancel must reuse the StartRequest requestId",
                    ));
                }
                self.cancelled = true;
            }
        }
        Ok(())
    }

    pub fn request_id(&self) -> Option<&Identifier> {
        self.request_id.as_ref()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ExecutionScope;
    use crate::identity::DerivationIdentity;
    use crate::values::{CacheCompatibilityId, InputRef, InputRole};
    use serde_json::json;

    fn probe_request() -> StartRequest {
        let input = InputDescriptor {
            role: InputRole::new(InputRole::SOURCE_MEDIA).expect("role"),
            input_ref: InputRef::new("input/source.media").expect("ref"),
            content_hash: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .parse()
                .expect("digest"),
            byte_size: 1_048_576,
        };
        let descriptor = InputSetDescriptor::from_inputs(vec![input.clone()]);
        let input_fingerprint = descriptor.input_fingerprint().expect("fingerprint");
        let operation = Operation::Probe;
        let identity = DerivationIdentity {
            engine_cache_compatibility_id: CacheCompatibilityId::new("scene-core-output-v1")
                .expect("cache id"),
            toolchain_fingerprint:
                "sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                    .parse()
                    .expect("digest"),
        };
        let derivation_key = DerivationDescriptor {
            derivation_descriptor_version: crate::values::DescriptorVersion::current(),
            input_fingerprint: input_fingerprint.clone(),
            operation,
            operation_config_hash:
                "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                    .parse()
                    .expect("digest"),
            output_contract_version: operation.output_contract_version(),
            engine_cache_compatibility_id: identity.engine_cache_compatibility_id.clone(),
            toolchain_fingerprint: identity.toolchain_fingerprint.clone(),
        }
        .derive_key()
        .expect("key");
        StartRequest {
            engine_protocol_version: ProtocolVersion::current(),
            message_type: StartMessageType::Start,
            request_id: "req_01".parse().expect("id"),
            operation,
            source_version_id: "sourcev_01".parse().expect("id"),
            inputs: vec![input],
            input_fingerprint,
            operation_config_hash:
                "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                    .parse()
                    .expect("digest"),
            output_contract_version: operation.output_contract_version(),
            derivation_key,
            execution_context: ExecutionContext {
                run_id: "run_01".parse().expect("id"),
                generation: 0,
                attempt: 1,
                scope: ExecutionScope::Asset,
            },
            deadline_ms: None,
            options: OperationOptions {},
        }
    }

    #[test]
    fn bom_and_oversized_lines_are_rejected() {
        let bom = "\u{feff}{}";
        assert!(parse_control_line(bom).is_err());
        let oversized = format!("{{{}}}", " ".repeat(MAX_CONTROL_LINE_BYTES));
        assert!(parse_control_line(&oversized).is_err());
    }

    #[test]
    fn unknown_message_type_is_rejected() {
        let line = r#"{"engineProtocolVersion":"0.1","messageType":"probe","requestId":"r"}"#;
        assert!(parse_control_line(line).is_err());
    }

    #[test]
    fn operation_config_hash_is_stable() {
        let request = probe_request();
        assert_eq!(request.effective_deadline_ms(), DEFAULT_DEADLINE_MS);
        assert_eq!(
            request
                .compute_operation_config_hash()
                .expect("hash")
                .as_str(),
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
        let value = serde_json::to_value(&request).expect("serializes");
        assert_eq!(value["messageType"], json!("start"));
        assert_eq!(value["options"], json!({}));
        assert!(value.get("deadlineMs").is_none());
    }

    #[test]
    fn arbitrary_option_keys_fail_while_parsing() {
        let line = serde_json::to_string(&probe_request()).expect("serializes");
        let mutated = line.replacen("\"options\":{}", "\"options\":{\"profile\":\"custom\"}", 1);
        assert!(StartRequest::parse(&mutated).is_err());
    }

    fn cancel_line(request_id: &str) -> String {
        format!(
            r#"{{"engineProtocolVersion":"0.1","messageType":"cancel","requestId":"{request_id}"}}"#
        )
    }

    #[test]
    fn control_stream_accepts_start_then_matching_cancel() {
        let start = serde_json::to_string(&probe_request()).expect("serializes");
        let mut validator = ControlStreamValidator::new();
        let message = parse_control_line(&start).expect("start");
        validator.accept(&message).expect("start accepted");
        assert_eq!(
            validator.request_id().expect("request id").as_str(),
            "req_01"
        );

        let message = parse_control_line(&cancel_line("req_01")).expect("cancel");
        validator
            .accept(&message)
            .expect("matching cancel accepted");
        assert!(validator.is_cancelled());
    }

    #[test]
    fn control_stream_rejects_second_start_and_wrong_cancels() {
        let start = serde_json::to_string(&probe_request()).expect("serializes");
        let start_message = parse_control_line(&start).expect("start");

        let mut second_start = ControlStreamValidator::new();
        second_start.accept(&start_message).expect("start");
        assert!(second_start.accept(&start_message).is_err());

        let mut wrong_id = ControlStreamValidator::new();
        wrong_id.accept(&start_message).expect("start");
        let wrong = parse_control_line(&cancel_line("req_02")).expect("cancel");
        assert!(wrong_id.accept(&wrong).is_err());

        let mut second_cancel = ControlStreamValidator::new();
        second_cancel.accept(&start_message).expect("start");
        let cancel = parse_control_line(&cancel_line("req_01")).expect("cancel");
        second_cancel.accept(&cancel).expect("cancel");
        assert!(second_cancel.accept(&cancel).is_err());

        let mut cancel_first = ControlStreamValidator::new();
        assert!(cancel_first.accept(&cancel).is_err());
    }
}

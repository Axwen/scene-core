//! Controlled protocol errors shared by events, Host requests and fixtures.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Frozen Protocol 0.1 error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedProtocol,
    OperationUnavailable,
    InputNotFound,
    InputChanged,
    UnsupportedInput,
    CorruptMedia,
    MissingVideoStream,
    ResourceLimit,
    ToolUnavailable,
    ToolFailed,
    Timeout,
    Cancelled,
    EngineInternal,
}

impl ErrorCode {
    /// Every code frozen by Protocol 0.1, in contract order.
    pub const ALL: [ErrorCode; 14] = [
        ErrorCode::InvalidRequest,
        ErrorCode::UnsupportedProtocol,
        ErrorCode::OperationUnavailable,
        ErrorCode::InputNotFound,
        ErrorCode::InputChanged,
        ErrorCode::UnsupportedInput,
        ErrorCode::CorruptMedia,
        ErrorCode::MissingVideoStream,
        ErrorCode::ResourceLimit,
        ErrorCode::ToolUnavailable,
        ErrorCode::ToolFailed,
        ErrorCode::Timeout,
        ErrorCode::Cancelled,
        ErrorCode::EngineInternal,
    ];

    /// Terminal kind required for this code by the contract matrix.
    pub const fn terminal(self) -> TerminalKind {
        match self {
            ErrorCode::Timeout => TerminalKind::TimedOut,
            ErrorCode::Cancelled => TerminalKind::Cancelled,
            _ => TerminalKind::Failed,
        }
    }

    /// Process exit code required for this code by the contract matrix.
    pub const fn exit_code(self) -> u8 {
        match self {
            ErrorCode::EngineInternal => 1,
            ErrorCode::UnsupportedInput
            | ErrorCode::CorruptMedia
            | ErrorCode::MissingVideoStream
            | ErrorCode::ResourceLimit
            | ErrorCode::ToolFailed => 3,
            ErrorCode::Timeout => 124,
            ErrorCode::Cancelled => 130,
            _ => 2,
        }
    }

    /// Whether the contract marks this code retryable.
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            ErrorCode::ResourceLimit
                | ErrorCode::ToolUnavailable
                | ErrorCode::ToolFailed
                | ErrorCode::Timeout
                | ErrorCode::EngineInternal
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidRequest => "INVALID_REQUEST",
            ErrorCode::UnsupportedProtocol => "UNSUPPORTED_PROTOCOL",
            ErrorCode::OperationUnavailable => "OPERATION_UNAVAILABLE",
            ErrorCode::InputNotFound => "INPUT_NOT_FOUND",
            ErrorCode::InputChanged => "INPUT_CHANGED",
            ErrorCode::UnsupportedInput => "UNSUPPORTED_INPUT",
            ErrorCode::CorruptMedia => "CORRUPT_MEDIA",
            ErrorCode::MissingVideoStream => "MISSING_VIDEO_STREAM",
            ErrorCode::ResourceLimit => "RESOURCE_LIMIT",
            ErrorCode::ToolUnavailable => "TOOL_UNAVAILABLE",
            ErrorCode::ToolFailed => "TOOL_FAILED",
            ErrorCode::Timeout => "TIMEOUT",
            ErrorCode::Cancelled => "CANCELLED",
            ErrorCode::EngineInternal => "ENGINE_INTERNAL",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Terminal event family a failed request must end with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalKind {
    Failed,
    Cancelled,
    TimedOut,
}

/// Result whose error is boxed: protocol errors are cold-path wire payloads,
/// so keeping them off the `Result` hot shape is intentional.
pub type ProtocolResult<T> = Result<T, Box<ProtocolError>>;

/// Wire error payload. Only the fixed contract fields are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub cause: Option<String>,
    #[serde(default)]
    pub next_step: Option<String>,
    pub retryable: bool,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub actual: Option<i64>,
}

impl ProtocolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            cause: None,
            next_step: None,
            retryable: code.retryable(),
            stage: None,
            limit: None,
            actual: None,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    pub fn unsupported_protocol(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::UnsupportedProtocol, message)
    }

    pub fn input_not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InputNotFound, message)
    }

    pub fn input_changed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InputChanged, message)
    }

    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    pub fn with_next_step(mut self, next_step: impl Into<String>) -> Self {
        self.next_step = Some(next_step.into());
        self
    }

    pub fn with_stage(mut self, stage: impl Into<String>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    pub fn with_limit_actual(mut self, limit: i64, actual: i64) -> Self {
        self.limit = Some(limit);
        self.actual = Some(actual);
        self
    }

    /// Validates the fixed-field payload rules.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.message.is_empty() {
            return Err(ValidationError::new("error.message", "must not be empty"));
        }
        validate_safe_text("error.message", &self.message, 1024)?;
        if let Some(cause) = &self.cause {
            validate_safe_text("error.cause", cause, 1024)?;
        }
        if let Some(next_step) = &self.next_step {
            validate_safe_text("error.nextStep", next_step, 1024)?;
        }
        if let Some(stage) = &self.stage {
            validate_safe_text("error.stage", stage, 64)?;
        }
        if self.retryable != self.code.retryable() {
            return Err(ValidationError::new(
                "error.retryable",
                "must match the frozen error code matrix",
            ));
        }
        Ok(())
    }
}

/// Structural validation failure for engine-produced DTOs and contract
/// invariants. Host-facing request failures use [`ProtocolError`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    path: String,
    message: String,
}

impl ValidationError {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            formatter.write_str(&self.message)
        } else {
            write!(formatter, "{}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for ValidationError {}

pub(crate) fn validate_safe_text(
    path: &str,
    value: &str,
    max_len: usize,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::new(path, "must not be empty"));
    }
    if value.len() > max_len {
        return Err(ValidationError::new(path, "is too long"));
    }
    if value.chars().any(char::is_control) {
        return Err(ValidationError::new(
            path,
            "must not contain control characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_matrix_is_complete_and_consistent() {
        assert_eq!(ErrorCode::ALL.len(), 14);
        for code in ErrorCode::ALL {
            let error = ProtocolError::new(code, "safe message");
            assert_eq!(error.retryable, code.retryable());
            assert!(error.validate().is_ok(), "{code}");
        }
        assert_eq!(ErrorCode::Timeout.exit_code(), 124);
        assert_eq!(ErrorCode::Cancelled.exit_code(), 130);
        assert_eq!(ErrorCode::EngineInternal.exit_code(), 1);
        assert_eq!(ErrorCode::Timeout.terminal(), TerminalKind::TimedOut);
        assert_eq!(ErrorCode::Cancelled.terminal(), TerminalKind::Cancelled);
        assert_eq!(ErrorCode::InputChanged.terminal(), TerminalKind::Failed);
    }

    #[test]
    fn wire_names_match_as_str_for_every_code() {
        for code in ErrorCode::ALL {
            let value = serde_json::to_value(code).expect("serializes");
            assert_eq!(value, serde_json::json!(code.as_str()), "{code:?}");
            let parsed: ErrorCode = serde_json::from_value(value).expect("parses");
            assert_eq!(parsed, code);
        }
    }

    #[test]
    fn error_payload_rejects_unknown_fields_and_bad_retryable() {
        let unknown = r#"{"code":"INVALID_REQUEST","message":"m","retryable":false,"extra":1}"#;
        assert!(serde_json::from_str::<ProtocolError>(unknown).is_err());

        let mismatched = r#"{"code":"INPUT_CHANGED","message":"m","retryable":true}"#;
        let error: ProtocolError = serde_json::from_str(mismatched).expect("parses");
        assert!(error.validate().is_err());
    }
}

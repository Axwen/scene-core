//! Engine event envelope, per-event field rules and the event transcript
//! state machine.

use crate::error::{ErrorCode, ProtocolError, ProtocolResult, ValidationError};
use crate::execution::ExecutionContext;
use crate::framing::parse_strict_json;
use crate::identity::EngineIdentity;
use crate::operation::Operation;
use crate::result::OperationResult;
use crate::values::{Identifier, ProtocolVersion, Sha256Digest, StageName, UtcTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum EventMessageType {
    #[serde(rename = "event")]
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Accepted,
    Running,
    Progress,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl EventType {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            EventType::Completed | EventType::Failed | EventType::Cancelled | EventType::TimedOut
        )
    }
}

/// One engine event. Optional body fields are omitted from the wire form when
/// they do not apply to the event type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EventEnvelope {
    pub engine_protocol_version: ProtocolVersion,
    pub message_type: EventMessageType,
    pub request_id: Identifier,
    pub source_version_id: Identifier,
    pub input_fingerprint: Sha256Digest,
    pub derivation_key: Sha256Digest,
    pub operation: Operation,
    pub sequence: u64,
    pub event_type: EventType,
    pub engine: EngineIdentity,
    pub execution_context: ExecutionContext,
    pub occurred_at: UtcTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<StageName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<OperationResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

impl EventEnvelope {
    pub fn parse(line: &str) -> ProtocolResult<Self> {
        parse_strict_json(line, "engine event")
    }

    pub fn is_terminal(&self) -> bool {
        self.event_type.is_terminal()
    }

    /// Process exit code required for a terminal event, `None` otherwise.
    pub fn exit_code(&self) -> Option<u8> {
        match self.event_type {
            EventType::Completed => Some(0),
            EventType::Failed => self.error.as_ref().map(|error| error.code.exit_code()),
            EventType::Cancelled => Some(ErrorCode::Cancelled.exit_code()),
            EventType::TimedOut => Some(ErrorCode::Timeout.exit_code()),
            _ => None,
        }
    }

    /// Validates the per-event DTO rules.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.engine_protocol_version.is_supported() {
            return Err(ValidationError::new(
                "engineProtocolVersion",
                "must be '0.1' in Protocol 0.1",
            ));
        }
        self.engine.validate()?;
        self.execution_context.validate()?;
        if self.sequence < 1 {
            return Err(ValidationError::new("sequence", "must start at 1"));
        }
        if self.event_type == EventType::Accepted && self.sequence != 1 {
            return Err(ValidationError::new(
                "sequence",
                "accepted must use sequence 1",
            ));
        }
        let has_progress_values =
            self.stage.is_some() || self.completed.is_some() || self.total.is_some();
        match self.event_type {
            EventType::Accepted | EventType::Running => {
                if has_progress_values || self.result.is_some() || self.error.is_some() {
                    return Err(ValidationError::new(
                        "eventType",
                        "accepted and running carry no progress, result or error",
                    ));
                }
            }
            EventType::Progress => {
                if self.stage.is_none() || self.completed.is_none() || self.total.is_none() {
                    return Err(ValidationError::new(
                        "eventType",
                        "progress requires stage, completed and total",
                    ));
                }
                if self.result.is_some() || self.error.is_some() {
                    return Err(ValidationError::new(
                        "eventType",
                        "progress carries no result or error",
                    ));
                }
            }
            EventType::Completed => {
                let Some(result) = &self.result else {
                    return Err(ValidationError::new(
                        "result",
                        "completed requires an operation result",
                    ));
                };
                if has_progress_values || self.error.is_some() {
                    return Err(ValidationError::new(
                        "eventType",
                        "completed carries only the operation result",
                    ));
                }
                if result.operation() != self.operation {
                    return Err(ValidationError::new(
                        "result.operation",
                        "must match the event operation",
                    ));
                }
                result.validate()?;
            }
            EventType::Failed | EventType::Cancelled | EventType::TimedOut => {
                if has_progress_values || self.result.is_some() {
                    return Err(ValidationError::new(
                        "eventType",
                        "terminal failures carry only the controlled error",
                    ));
                }
                let Some(error) = &self.error else {
                    return Err(ValidationError::new(
                        "error",
                        "terminal failures require a controlled error",
                    ));
                };
                error.validate()?;
                match self.event_type {
                    EventType::Cancelled if error.code != ErrorCode::Cancelled => {
                        return Err(ValidationError::new(
                            "error.code",
                            "cancelled must use CANCELLED",
                        ));
                    }
                    EventType::TimedOut if error.code != ErrorCode::Timeout => {
                        return Err(ValidationError::new(
                            "error.code",
                            "timed_out must use TIMEOUT",
                        ));
                    }
                    EventType::Failed
                        if matches!(error.code, ErrorCode::Cancelled | ErrorCode::Timeout) =>
                    {
                        return Err(ValidationError::new(
                            "error.code",
                            "CANCELLED and TIMEOUT require their own terminal event",
                        ));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// Identity that every event of one request must echo unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamIdentity {
    request_id: Identifier,
    source_version_id: Identifier,
    input_fingerprint: Sha256Digest,
    derivation_key: Sha256Digest,
    operation: Operation,
    execution_context: ExecutionContext,
}

impl From<&EventEnvelope> for StreamIdentity {
    fn from(event: &EventEnvelope) -> Self {
        Self {
            request_id: event.request_id.clone(),
            source_version_id: event.source_version_id.clone(),
            input_fingerprint: event.input_fingerprint.clone(),
            derivation_key: event.derivation_key.clone(),
            operation: event.operation,
            execution_context: event.execution_context.clone(),
        }
    }
}

/// Pure validator for one request's event stream: sequence continuity,
/// exactly one terminal event, no events after the terminal, and stable
/// request identity.
///
/// The first event is either `accepted` with sequence 1, or `failed` with
/// sequence 1 when the request was invalid before acceptance.
#[derive(Debug, Default)]
pub struct EventStreamValidator {
    identity: Option<StreamIdentity>,
    last_sequence: u64,
    accepted: bool,
    terminal: Option<EventType>,
}

impl EventStreamValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates `event` against the per-event rules and the stream state so
    /// far, then advances the state.
    pub fn accept(&mut self, event: &EventEnvelope) -> Result<(), ValidationError> {
        event.validate()?;

        if let Some(terminal) = self.terminal {
            return Err(ValidationError::new(
                "eventType",
                format!("no event may follow the terminal {terminal:?} event"),
            ));
        }

        let identity = StreamIdentity::from(event);
        if let Some(expected) = &self.identity {
            if *expected != identity {
                return Err(ValidationError::new(
                    "identity",
                    "must echo the first event's request, source, fingerprint, derivation, operation and execution context unchanged",
                ));
            }
        }

        if self.last_sequence == 0 {
            if event.sequence != 1 {
                return Err(ValidationError::new("sequence", "must start at 1"));
            }
            if !matches!(event.event_type, EventType::Accepted | EventType::Failed) {
                return Err(ValidationError::new(
                    "eventType",
                    "the first event must be accepted, or failed for an invalid request",
                ));
            }
        } else if event.sequence != self.last_sequence + 1 {
            return Err(ValidationError::new(
                "sequence",
                "must increase by exactly 1 without duplicates or regression",
            ));
        }

        match event.event_type {
            EventType::Accepted => {
                if self.accepted {
                    return Err(ValidationError::new(
                        "eventType",
                        "accepted must appear exactly once",
                    ));
                }
                self.accepted = true;
            }
            EventType::Running | EventType::Progress => {
                if !self.accepted {
                    return Err(ValidationError::new(
                        "eventType",
                        "requires a preceding accepted event",
                    ));
                }
            }
            EventType::Failed => {
                self.terminal = Some(event.event_type);
            }
            EventType::Completed | EventType::Cancelled | EventType::TimedOut => {
                if !self.accepted {
                    return Err(ValidationError::new(
                        "eventType",
                        "requires a preceding accepted event",
                    ));
                }
                self.terminal = Some(event.event_type);
            }
        }

        if self.identity.is_none() {
            self.identity = Some(identity);
        }
        self.last_sequence = event.sequence;
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.terminal.is_some()
    }

    pub fn terminal(&self) -> Option<EventType> {
        self.terminal
    }
}

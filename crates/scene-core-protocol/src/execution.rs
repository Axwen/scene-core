//! Host execution context. Required on every Host-facing `StartRequest`.

use crate::error::ValidationError;
use crate::values::Identifier;
use serde::{Deserialize, Serialize};

/// Request origin scope. `cacheScope` never enters the engine protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionScope {
    Asset,
    Query,
    Evaluation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExecutionContext {
    pub run_id: Identifier,
    pub generation: u64,
    pub attempt: u64,
    pub scope: ExecutionScope,
}

impl ExecutionContext {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.attempt < 1 {
            return Err(ValidationError::new(
                "executionContext.attempt",
                "must be at least 1",
            ));
        }
        Ok(())
    }
}

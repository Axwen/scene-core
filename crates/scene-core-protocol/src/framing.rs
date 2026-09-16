//! Strict JSONL framing shared by control messages and engine events.

use crate::error::{ProtocolError, ProtocolResult};
use serde::de::DeserializeOwned;

/// Rejects a UTF-8 BOM at the start of a protocol line.
pub fn check_no_bom(line: &str) -> ProtocolResult<()> {
    if line.starts_with('\u{feff}') {
        return Err(Box::new(
            ProtocolError::invalid_request("UTF-8 BOM is not allowed in protocol JSONL")
                .with_stage("framing")
                .with_next_step("send UTF-8 without BOM."),
        ));
    }
    Ok(())
}

/// Parses one strict JSON value, mapping parser failures to a safe
/// `INVALID_REQUEST` cause that never echoes the rejected value.
pub fn parse_strict_json<T: DeserializeOwned>(line: &str, what: &str) -> ProtocolResult<T> {
    check_no_bom(line)?;
    serde_json::from_str(line).map_err(|error| {
        Box::new(
            ProtocolError::invalid_request(format!("{what} is not a strict JSON object"))
                .with_cause(format!(
                    "{:?} parse error at line {}, column {}",
                    error.classify(),
                    error.line(),
                    error.column()
                ))
                .with_stage("framing")
                .with_next_step(
                    "send one strict JSON object per line; unknown fields, duplicate keys and out-of-range values are rejected.",
                ),
        )
    })
}

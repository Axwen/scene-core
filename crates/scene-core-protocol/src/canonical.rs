//! RFC 8785 (JCS) canonical JSON used by every protocol digest.
//!
//! Protocol 0.1 descriptors contain only strings, integers, booleans, nulls,
//! arrays and objects. Non-integer numbers are rejected instead of being
//! silently formatted like JavaScript, which keeps `inputFingerprint` and
//! `derivationKey` independent of floating-point formatting.

use crate::values::Sha256Digest;
use serde_json::Value;
use std::cmp::Ordering;
use std::fmt;

/// Failure returned when a value cannot be canonicalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalJsonError {
    /// Protocol canonical JSON forbids non-integer numbers.
    NonIntegerNumber { representation: String },
}

impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanonicalJsonError::NonIntegerNumber { representation } => write!(
                formatter,
                "canonical JSON does not allow the non-integer number {representation}"
            ),
        }
    }
}

impl std::error::Error for CanonicalJsonError {}

/// Serializes `value` following RFC 8785.
pub fn to_canonical_json(value: &Value) -> Result<String, CanonicalJsonError> {
    let mut out = String::new();
    write_value(&mut out, value)?;
    Ok(out)
}

/// `sha256(RFC8785(value))` in the protocol `sha256:<hex>` form.
pub fn canonical_sha256(value: &Value) -> Result<Sha256Digest, CanonicalJsonError> {
    let canonical = to_canonical_json(value)?;
    Ok(Sha256Digest::from_bytes(canonical.as_bytes()))
}

fn write_value(out: &mut String, value: &Value) -> Result<(), CanonicalJsonError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => {
            if let Some(unsigned) = number.as_u64() {
                out.push_str(&unsigned.to_string());
            } else if let Some(signed) = number.as_i64() {
                out.push_str(&signed.to_string());
            } else {
                return Err(CanonicalJsonError::NonIntegerNumber {
                    representation: number.to_string(),
                });
            }
        }
        Value::String(string) => write_string(out, string),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(out, item)?;
            }
            out.push(']');
        }
        Value::Object(entries) => {
            let mut keys: Vec<&String> = entries.keys().collect();
            keys.sort_by(|left, right| utf16_code_unit_cmp(left, right));
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, &entries[key.as_str()])?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn write_string(out: &mut String, value: &str) {
    use fmt::Write as _;

    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{0009}' => out.push_str("\\t"),
            '\u{000a}' => out.push_str("\\n"),
            '\u{000c}' => out.push_str("\\f"),
            '\u{000d}' => out.push_str("\\r"),
            character if (character as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", character as u32);
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

fn utf16_code_unit_cmp(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_object_keys_and_keeps_null() {
        let value = json!({"role": "source_media", "byteSize": 7, "contentHash": null});
        assert_eq!(
            to_canonical_json(&value).expect("canonical"),
            r#"{"byteSize":7,"contentHash":null,"role":"source_media"}"#
        );
    }

    #[test]
    fn escapes_strings_like_json_stringify() {
        let value = json!({"text": "line\n\"quoted\"\u{0001}雪"});
        assert_eq!(
            to_canonical_json(&value).expect("canonical"),
            "{\"text\":\"line\\n\\\"quoted\\\"\\u0001雪\"}"
        );
    }

    #[test]
    fn rejects_floats() {
        let error = to_canonical_json(&json!({"ratio": 1.5})).expect_err("float is rejected");
        assert!(matches!(error, CanonicalJsonError::NonIntegerNumber { .. }));
    }

    #[test]
    fn empty_object_golden_hash() {
        let digest = canonical_sha256(&json!({})).expect("canonical");
        assert_eq!(
            digest.as_str(),
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }

    #[test]
    fn utf16_order_differs_from_code_point_order() {
        let value = json!({
            "\u{e000}": 1,
            "\u{10000}": 2,
        });
        assert_eq!(
            to_canonical_json(&value).expect("canonical"),
            "{\"\u{10000}\":2,\"\u{e000}\":1}"
        );
    }
}

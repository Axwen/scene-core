//! Public operation identifiers, their registered result contracts and the
//! caller option boundary.

use crate::values::OutputContractVersion;
use serde::{Deserialize, Serialize};

/// Operations frozen by Protocol 0.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Probe,
    ExtractPreview,
}

impl Operation {
    /// Registered `outputContractVersion` for this operation in 0.1.
    pub fn output_contract_version(self) -> OutputContractVersion {
        let registered = match self {
            Operation::Probe => "probe-result/1",
            Operation::ExtractPreview => "extract-preview-result/1",
        };
        OutputContractVersion::new(registered).expect("registered output contract is well-formed")
    }

    /// Whether `version` is the registered result contract for this operation.
    pub fn is_registered_output_contract(self, version: &OutputContractVersion) -> bool {
        *version == self.output_contract_version()
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Operation::Probe => "probe",
            Operation::ExtractPreview => "extract_preview",
        }
    }
}

/// Caller-provided operation options.
///
/// Protocol 0.1 registers no configurable options: the only accepted wire
/// shape is `{}`. Unknown keys, duplicate keys and non-object shapes are
/// rejected while parsing, so no free-form options map can enter the protocol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct OperationOptions {}

impl<'de> Deserialize<'de> for OperationOptions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EmptyOptionsVisitor;

        impl<'de> serde::de::Visitor<'de> for EmptyOptionsVisitor {
            type Value = OperationOptions;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an empty options object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                if map.next_key::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(<A::Error as serde::de::Error>::custom(
                        "operation options must be empty in Protocol 0.1",
                    ));
                }
                Ok(OperationOptions {})
            }
        }

        deserializer.deserialize_map(EmptyOptionsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_match_as_str() {
        for operation in [Operation::Probe, Operation::ExtractPreview] {
            let value = serde_json::to_value(operation).expect("serializes");
            assert_eq!(
                value,
                serde_json::json!(operation.as_str()),
                "{operation:?}"
            );
        }
    }

    #[test]
    fn registered_contracts_are_wired_to_their_operations() {
        for operation in [Operation::Probe, Operation::ExtractPreview] {
            let version = operation.output_contract_version();
            assert!(operation.is_registered_output_contract(&version));
        }
        let probe = Operation::Probe.output_contract_version();
        assert!(!Operation::ExtractPreview.is_registered_output_contract(&probe));
    }

    #[test]
    fn options_only_accept_an_empty_object() {
        assert_eq!(
            serde_json::from_str::<OperationOptions>("{}").expect("empty options"),
            OperationOptions {}
        );
        assert!(serde_json::from_str::<OperationOptions>(r#"{"profile":"custom"}"#).is_err());
        assert!(serde_json::from_str::<OperationOptions>(r#"{"a":1,"a":2}"#).is_err());
        assert!(serde_json::from_str::<OperationOptions>("[]").is_err());
        assert_eq!(
            serde_json::to_value(OperationOptions {}).expect("serializes"),
            serde_json::json!({})
        );
    }
}

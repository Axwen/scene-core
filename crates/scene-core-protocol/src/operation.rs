//! Public operation identifiers, their registered result contracts and the
//! caller option boundary.

use crate::error::ValidationError;
use crate::values::OutputContractVersion;
use serde::{Deserialize, Serialize};

/// Operations registered by Protocol 0.1.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Probe,
    ExtractPreview,
    ExtractAudioPcm,
}

impl Operation {
    /// Registered `outputContractVersion` for this operation in 0.1.
    pub fn output_contract_version(self) -> OutputContractVersion {
        let registered = match self {
            Operation::Probe => "probe-result/1",
            Operation::ExtractPreview => "extract-preview-result/1",
            Operation::ExtractAudioPcm => "extract-audio-pcm-result/1",
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
            Operation::ExtractAudioPcm => "extract_audio_pcm",
        }
    }
}

/// Options accepted by `probe` and `extract_preview`: exactly `{}`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EmptyOptions {}

/// Options accepted by `extract_audio_pcm`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AudioPcmOptions {
    /// Explicit ffprobe stream index. Omitted means the lowest non-attached
    /// audio stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_stream_index: Option<u32>,
}

/// Operation-specific options. The wire shape must match the request
/// operation; [`OperationOptions::Empty`] is valid for every operation and is
/// the canonical "no options" value.
///
/// Unknown keys, duplicate keys and non-object shapes are rejected while
/// parsing, so no free-form options map can enter the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum OperationOptions {
    Empty(EmptyOptions),
    ExtractAudioPcm(AudioPcmOptions),
}

impl Default for OperationOptions {
    fn default() -> Self {
        Self::Empty(EmptyOptions {})
    }
}

impl<'de> Deserialize<'de> for OperationOptions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct OptionsVisitor;

        impl<'de> serde::de::Visitor<'de> for OptionsVisitor {
            type Value = OperationOptions;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an options object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut audio_stream_index: Option<u32> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "audioStreamIndex" => {
                            if audio_stream_index.is_some() {
                                return Err(<A::Error as serde::de::Error>::duplicate_field(
                                    "audioStreamIndex",
                                ));
                            }
                            audio_stream_index = Some(map.next_value()?);
                        }
                        other => {
                            return Err(<A::Error as serde::de::Error>::unknown_field(
                                other,
                                &["audioStreamIndex"],
                            ));
                        }
                    }
                }
                Ok(match audio_stream_index {
                    Some(index) => OperationOptions::ExtractAudioPcm(AudioPcmOptions {
                        audio_stream_index: Some(index),
                    }),
                    None => OperationOptions::Empty(EmptyOptions {}),
                })
            }
        }

        deserializer.deserialize_map(OptionsVisitor)
    }
}

impl OperationOptions {
    /// Validates that the options shape is registered for `operation`.
    pub fn validate_for(&self, operation: Operation) -> Result<(), ValidationError> {
        match (operation, self) {
            (_, OperationOptions::Empty(_)) => Ok(()),
            (Operation::ExtractAudioPcm, OperationOptions::ExtractAudioPcm(_)) => Ok(()),
            (other, _) => Err(ValidationError::new(
                "options",
                format!("are not registered for operation '{}'", other.as_str()),
            )),
        }
    }

    /// Explicit audio stream index when the audio options variant is used.
    pub const fn audio_stream_index(&self) -> Option<u32> {
        match self {
            OperationOptions::ExtractAudioPcm(options) => options.audio_stream_index,
            OperationOptions::Empty(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPERATIONS: [Operation; 3] = [
        Operation::Probe,
        Operation::ExtractPreview,
        Operation::ExtractAudioPcm,
    ];

    #[test]
    fn wire_names_match_as_str() {
        for operation in OPERATIONS {
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
        for operation in OPERATIONS {
            let version = operation.output_contract_version();
            assert!(operation.is_registered_output_contract(&version));
        }
        let probe = Operation::Probe.output_contract_version();
        assert!(!Operation::ExtractAudioPcm.is_registered_output_contract(&probe));
    }

    #[test]
    fn options_only_accept_their_operation_shape() {
        assert_eq!(
            serde_json::from_str::<OperationOptions>("{}").expect("empty options"),
            OperationOptions::default()
        );
        assert_eq!(
            serde_json::from_str::<OperationOptions>(r#"{"audioStreamIndex":2}"#)
                .expect("audio options"),
            OperationOptions::ExtractAudioPcm(AudioPcmOptions {
                audio_stream_index: Some(2),
            })
        );
        assert!(
            serde_json::from_str::<OperationOptions>(r#"{"audioStreamIndex":"2"}"#).is_err(),
            "wrong type"
        );
        assert!(
            serde_json::from_str::<OperationOptions>(r#"{"profile":"custom"}"#).is_err(),
            "unknown key"
        );
        assert!(
            serde_json::from_str::<OperationOptions>(
                r#"{"audioStreamIndex":1,"audioStreamIndex":2}"#
            )
            .is_err(),
            "duplicate key"
        );
        assert!(serde_json::from_str::<OperationOptions>("[]").is_err());

        let audio = OperationOptions::ExtractAudioPcm(AudioPcmOptions {
            audio_stream_index: Some(1),
        });
        assert!(audio.validate_for(Operation::ExtractAudioPcm).is_ok());
        assert!(audio.validate_for(Operation::Probe).is_err());
        assert!(audio.validate_for(Operation::ExtractPreview).is_err());
        assert!(
            OperationOptions::default()
                .validate_for(Operation::ExtractAudioPcm)
                .is_ok()
        );
        assert_eq!(audio.audio_stream_index(), Some(1));
    }

    #[test]
    fn no_options_round_trips_as_the_empty_variant() {
        let value = serde_json::to_value(OperationOptions::default()).expect("serializes");
        assert_eq!(value, serde_json::json!({}));
        let parsed: OperationOptions = serde_json::from_value(value).expect("parses");
        assert_eq!(parsed, OperationOptions::default());
    }
}

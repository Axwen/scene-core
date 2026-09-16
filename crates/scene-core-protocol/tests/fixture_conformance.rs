//! Fixture conformance runner for `fixtures/protocol/`.
//!
//! `fixture-manifest.json` maps every fixture file to its rule, schema,
//! expected outcome, error code and (for accepted transcripts) exit code.

use scene_core_protocol::{
    ArtifactManifest, ControlMessage, ControlStreamValidator, DerivationDescriptor, EngineIdentity,
    EventEnvelope, EventStreamValidator, InputSetDescriptor, NormalizedMedia, TemporalSegment,
    ToolchainIdentity, parse_control_line, validate_temporal_segments,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FixtureManifest {
    manifest_version: String,
    entrypoints: BTreeMap<String, String>,
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FixtureEntry {
    file: String,
    kind: String,
    entrypoint: String,
    rule: String,
    schema: String,
    expect: Expectation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expectation {
    outcome: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    exit_code: Option<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct Actual {
    accepted: bool,
    error_code: Option<String>,
    exit_code: Option<u8>,
}

fn accept(exit_code: Option<u8>) -> Actual {
    Actual {
        accepted: true,
        error_code: None,
        exit_code,
    }
}

fn reject(error_code: Option<String>) -> Actual {
    Actual {
        accepted: false,
        error_code,
        exit_code: None,
    }
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/protocol")
}

fn schema_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/0.1")
}

fn manifest() -> FixtureManifest {
    let path = fixture_root().join("fixture-manifest.json");
    let text = fs::read_to_string(&path).expect("fixture-manifest.json exists");
    let manifest: FixtureManifest =
        serde_json::from_str(&text).expect("fixture-manifest.json parses");
    assert_eq!(manifest.manifest_version, "1");
    manifest
}

fn run(entrypoint: &str, text: &str) -> Actual {
    match entrypoint {
        "control" => match parse_control_line(text) {
            Err(error) => reject(Some(error.code.as_str().to_owned())),
            Ok(message) => {
                let result = match &message {
                    ControlMessage::Start(request) => request.validate(),
                    ControlMessage::Cancel(request) => request.validate(),
                };
                match result {
                    Ok(()) => accept(None),
                    Err(error) => reject(Some(error.code.as_str().to_owned())),
                }
            }
        },
        "control-stream" => {
            let mut validator = ControlStreamValidator::new();
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let message = match parse_control_line(line) {
                    Ok(message) => message,
                    Err(error) => return reject(Some(error.code.as_str().to_owned())),
                };
                if validator.accept(&message).is_err() {
                    return reject(None);
                }
            }
            accept(None)
        }
        "event" => {
            let event = match EventEnvelope::parse(text) {
                Ok(event) => event,
                Err(error) => return reject(Some(error.code.as_str().to_owned())),
            };
            match event.validate() {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            }
        }
        "transcript" => {
            let mut validator = EventStreamValidator::new();
            let mut exit_code = None;
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let event = match EventEnvelope::parse(line) {
                    Ok(event) => event,
                    Err(error) => return reject(Some(error.code.as_str().to_owned())),
                };
                exit_code = event.exit_code();
                if validator.accept(&event).is_err() {
                    return reject(None);
                }
            }
            if !validator.is_complete() {
                return reject(None);
            }
            accept(exit_code)
        }
        "manifest" => match serde_json::from_str::<ArtifactManifest>(text) {
            Err(_) => reject(None),
            Ok(manifest) => match manifest.validate() {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            },
        },
        "media" => match serde_json::from_str::<NormalizedMedia>(text) {
            Err(_) => reject(None),
            Ok(media) => match media.validate() {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            },
        },
        "segment" => match serde_json::from_str::<Vec<TemporalSegment>>(text) {
            Err(_) => reject(None),
            Ok(segments) => match validate_temporal_segments(&segments) {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            },
        },
        "inputSet" => match serde_json::from_str::<InputSetDescriptor>(text) {
            Err(_) => reject(None),
            Ok(descriptor) => {
                if descriptor.validate().is_err() || descriptor.input_fingerprint().is_err() {
                    reject(None)
                } else {
                    accept(None)
                }
            }
        },
        "derivation" => match serde_json::from_str::<DerivationDescriptor>(text) {
            Err(_) => reject(None),
            Ok(descriptor) => match descriptor.validate() {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            },
        },
        "engineIdentity" => match serde_json::from_str::<EngineIdentity>(text) {
            Err(_) => reject(None),
            Ok(identity) => match identity.validate() {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            },
        },
        "toolchainIdentity" => match serde_json::from_str::<ToolchainIdentity>(text) {
            Err(_) => reject(None),
            Ok(identity) => match identity.validate() {
                Ok(()) => accept(None),
                Err(_) => reject(None),
            },
        },
        other => panic!("{other} is not a registered fixture entrypoint"),
    }
}

#[test]
fn every_fixture_matches_its_expected_outcome() {
    let manifest = manifest();
    for entry in &manifest.fixtures {
        assert!(
            manifest.entrypoints.contains_key(&entry.entrypoint),
            "{}: unregistered entrypoint {}",
            entry.file,
            entry.entrypoint
        );
        let text = fs::read_to_string(fixture_root().join(&entry.file))
            .unwrap_or_else(|_| panic!("{} exists", entry.file));
        let actual = run(&entry.entrypoint, &text);
        let expected_accept = entry.expect.outcome == "accept";
        assert_eq!(
            actual.accepted, expected_accept,
            "{}: outcome mismatch (rule: {})",
            entry.file, entry.rule
        );
        assert_eq!(
            actual.error_code, entry.expect.error_code,
            "{}: error code mismatch (rule: {})",
            entry.file, entry.rule
        );
        assert_eq!(
            actual.exit_code, entry.expect.exit_code,
            "{}: exit code mismatch (rule: {})",
            entry.file, entry.rule
        );
    }
}

#[test]
fn manifest_covers_every_fixture_exactly_once() {
    let manifest = manifest();
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    for entry in &manifest.fixtures {
        assert!(
            listed.insert(entry.file.as_str()),
            "{} is listed more than once",
            entry.file
        );
    }

    let mut present: BTreeSet<String> = BTreeSet::new();
    for kind in ["valid", "invalid", "golden-jsonl"] {
        let dir = fixture_root().join(kind);
        for entry in fs::read_dir(&dir).unwrap_or_else(|_| panic!("{kind}/ exists")) {
            let name = entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            present.insert(format!("{kind}/{name}"));
        }
    }

    assert_eq!(
        present,
        listed.iter().map(|file| (*file).to_owned()).collect(),
        "fixtures/protocol files and fixture-manifest entries diverged"
    );

    let count = |kind: &str| {
        manifest
            .fixtures
            .iter()
            .filter(|entry| entry.kind == kind)
            .count()
    };
    assert!(count("valid") >= 10, "need at least 10 valid fixtures");
    assert!(count("invalid") >= 20, "need at least 20 invalid fixtures");
    assert!(
        count("golden-jsonl") >= 6,
        "need at least 6 golden transcripts"
    );
}

#[test]
fn every_fixture_maps_to_a_committed_schema() {
    let manifest = manifest();
    for entry in &manifest.fixtures {
        let path = schema_root().join(&entry.schema);
        assert!(
            path.is_file(),
            "{}: schema {} is missing",
            entry.file,
            entry.schema
        );
    }
}

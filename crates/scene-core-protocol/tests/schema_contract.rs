//! JSON Schema contract for `schemas/0.1/`.
//!
//! Schemas are generated from the Rust DTOs. `schemas/0.1/` must match the
//! generated output; regenerate with:
//!
//! ```text
//! UPDATE_SCHEMAS=1 cargo test -p scene-core-protocol --test schema_contract
//! ```

use scene_core_protocol::{
    ArtifactManifest, CancelRequest, CliErrorOutput, DerivationDescriptor, DoctorOutput,
    EngineIdentity, EventEnvelope, InputSetDescriptor, NormalizedMedia, PackageManifest,
    StartRequest, TemporalSegment, ToolchainDescriptor, ToolchainIdentity, VersionOutput,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum ControlMessageSchema {
    Start(Box<StartRequest>),
    Cancel(CancelRequest),
}

fn schemas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/0.1")
}

fn schema_value<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).expect("schema serializes")
}

fn generated_schemas() -> Vec<(&'static str, Value)> {
    let mut schemas = vec![
        ("artifact-manifest.json", schema_value::<ArtifactManifest>()),
        ("cli-error.json", schema_value::<CliErrorOutput>()),
        (
            "control-message.json",
            schema_value::<ControlMessageSchema>(),
        ),
        (
            "derivation-descriptor.json",
            schema_value::<DerivationDescriptor>(),
        ),
        ("engine-event.json", schema_value::<EventEnvelope>()),
        ("doctor.json", schema_value::<DoctorOutput>()),
        ("engine-identity.json", schema_value::<EngineIdentity>()),
        (
            "input-set-descriptor.json",
            schema_value::<InputSetDescriptor>(),
        ),
        ("normalized-media.json", schema_value::<NormalizedMedia>()),
        ("package-manifest.json", schema_value::<PackageManifest>()),
        ("temporal-segment.json", schema_value::<TemporalSegment>()),
        (
            "toolchain-descriptor.json",
            schema_value::<ToolchainDescriptor>(),
        ),
        (
            "toolchain-identity.json",
            schema_value::<ToolchainIdentity>(),
        ),
        ("version.json", schema_value::<VersionOutput>()),
    ];
    schemas.sort_by_key(|(name, _)| *name);
    schemas
}

#[test]
fn committed_schemas_match_generated() {
    let dir = schemas_dir();
    let update = std::env::var_os("UPDATE_SCHEMAS").is_some();
    if update {
        fs::create_dir_all(&dir).expect("create schemas dir");
    }

    let generated = generated_schemas();
    let expected_names: Vec<&str> = generated.iter().map(|(name, _)| *name).collect();

    if update {
        for (name, value) in &generated {
            let mut text = serde_json::to_string_pretty(value).expect("pretty schema");
            text.push('\n');
            fs::write(dir.join(name), text).expect("write schema");
        }
    }

    let mut actual_names: Vec<String> = fs::read_dir(&dir)
        .expect("schemas/0.1 exists")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json"))
        .collect();
    actual_names.sort();
    assert_eq!(
        actual_names, expected_names,
        "schemas/0.1 file set drifted; regenerate with UPDATE_SCHEMAS=1 cargo test -p scene-core-protocol --test schema_contract"
    );

    for (name, generated_value) in &generated {
        let committed_text = fs::read_to_string(dir.join(name)).expect("committed schema");
        let committed: Value =
            serde_json::from_str(&committed_text).expect("committed schema JSON");
        assert_eq!(
            &committed, generated_value,
            "{name} is stale; regenerate with UPDATE_SCHEMAS=1 cargo test -p scene-core-protocol --test schema_contract"
        );
    }
}

#[test]
fn normalized_media_schema_encodes_time_sign_rules() {
    let media = schema_value::<NormalizedMedia>();
    let defs = media.get("$defs").expect("$defs");
    let container =
        serde_json::to_value(&defs["ContainerInfo"]).expect("container schema serializes");
    let start = &container["properties"]["startTimeMs"];
    assert_eq!(start["type"], serde_json::json!(["integer", "null"]));
    assert_eq!(start["maximum"], serde_json::json!(0));

    let stream = serde_json::to_value(&defs["MediaStream"]).expect("stream schema serializes");
    let start = &stream["properties"]["startTimeMs"];
    assert_eq!(start["type"], serde_json::json!(["integer", "null"]));
    assert!(start.get("minimum").is_none());
}

#[test]
fn wire_schemas_encode_the_validator_constraints() {
    // The schemas are what consumers validate against, so the string classes
    // must carry the same restrictions as the Rust validators. The patterns
    // are ECMA-262 (lookaheads included); the drift gate keeps them in sync
    // with the generated schemas.
    let manifest = schema_value::<ArtifactManifest>();
    let defs = manifest.get("$defs").expect("$defs");
    let relative = serde_json::to_value(&defs["RelativeRef"]).expect("ref schema serializes");
    let pattern = relative["pattern"].as_str().expect("relative ref pattern");
    for fragment in [
        "(?!/)",
        "(?!.*//)",
        "(?!.*/$)",
        "(^|/)\\.\\.?(/|$)",
        "[^\\\\:",
    ] {
        assert!(
            pattern.contains(fragment),
            "missing {fragment} in {pattern}"
        );
    }
    assert_eq!(relative["maxLength"], serde_json::json!(1024));

    let media_type = serde_json::to_value(&defs["MediaType"]).expect("media type schema");
    assert_eq!(
        media_type["pattern"],
        serde_json::json!("^[a-z0-9.+_-]+/[a-z0-9.+_-]+$")
    );
    assert_eq!(media_type["maxLength"], serde_json::json!(128));

    let start = schema_value::<scene_core_protocol::StartRequest>();
    let deadline = &start["properties"]["deadlineMs"];
    assert_eq!(deadline["minimum"], serde_json::json!(1));
    assert_eq!(deadline["maximum"], serde_json::json!(600_000));
}

#[test]
fn request_artifact_times_are_non_negative() {
    let manifest = schema_value::<ArtifactManifest>();
    let defs = manifest.get("$defs").expect("$defs");
    let artifact = serde_json::to_value(&defs["Artifact"]).expect("artifact schema serializes");
    assert_eq!(
        artifact["properties"]["requestedTimeMs"]["minimum"],
        serde_json::json!(0)
    );
    assert_eq!(
        artifact["properties"]["presentationTimeMs"]["type"],
        serde_json::json!(["integer", "null"])
    );
}

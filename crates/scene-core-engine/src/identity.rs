//! Engine-side identity constants and bundle toolchain discovery.

use scene_core_protocol::{
    CacheCompatibilityId, CommitHash, EngineIdentity, Operation, ProtocolVersion,
    ToolchainDescriptor, ToolchainIdentity,
};
use std::fs;
use std::path::Path;

pub const ENGINE_CACHE_COMPATIBILITY_ID: &str = "scene-core-output-v1";
pub const TOOLCHAIN_DESCRIPTOR_FILE: &str = "toolchain-descriptor.json";

/// Identity baked into this build. Media operations stay empty until Phase 1.
pub fn engine_identity() -> EngineIdentity {
    EngineIdentity {
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        engine_commit: CommitHash::new(
            option_env!("SCENE_CORE_ENGINE_COMMIT")
                .unwrap_or("0000000000000000000000000000000000000000"),
        )
        .expect("engine commit is a 40-digit hex string"),
        target: env!("SCENE_CORE_TARGET").to_owned(),
        engine_cache_compatibility_id: CacheCompatibilityId::new(ENGINE_CACHE_COMPATIBILITY_ID)
            .expect("cache compatibility id is well-formed"),
        supported_protocol_versions: vec![ProtocolVersion::current()],
        implemented_operations: Vec::<Operation>::new(),
    }
}

/// Reads the toolchain descriptor shipped next to the engine and derives the
/// shared identity. The fingerprint is the SHA-256 of the exact file bytes.
pub fn read_toolchain_identity(bundle_root: &Path) -> Result<ToolchainIdentity, String> {
    let path = bundle_root.join(TOOLCHAIN_DESCRIPTOR_FILE);
    let bytes =
        fs::read(&path).map_err(|_| format!("{TOOLCHAIN_DESCRIPTOR_FILE} could not be read"))?;
    let descriptor: ToolchainDescriptor = serde_json::from_slice(&bytes)
        .map_err(|_| format!("{TOOLCHAIN_DESCRIPTOR_FILE} is not a valid toolchain descriptor"))?;
    descriptor
        .validate()
        .map_err(|error| format!("{TOOLCHAIN_DESCRIPTOR_FILE} is invalid: {error}"))?;
    Ok(descriptor.to_identity(ToolchainDescriptor::fingerprint(&bytes)))
}

//! Bundle doctor: ordered integrity checks that stop at the first failure.

use crate::identity::{engine_identity, read_toolchain_identity};
use crate::runner::CommandRunner;
use scene_core_protocol::{
    DescriptorVersion, DoctorCheck, DoctorCheckCode, DoctorCheckName, DoctorOutput, DoctorStatus,
    PackageManifest, Sha256Digest, ToolchainIdentity,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub struct DoctorRequest<'a> {
    pub bundle_root: &'a Path,
    pub trusted_manifest_sha256: Option<&'a Sha256Digest>,
    pub runner: &'a dyn CommandRunner,
}

pub fn run_doctor(request: DoctorRequest<'_>) -> DoctorOutput {
    let engine = engine_identity();
    let mut checks = Vec::new();
    let mut toolchain: Option<ToolchainIdentity> = None;

    let manifest_bytes = match fs::read(request.bundle_root.join(PackageManifest::SELF_PATH)) {
        Ok(bytes) => bytes,
        Err(_) => {
            checks.push(failure(
                "package-manifest",
                DoctorCheckCode::PackageManifestUnreadable,
                "package-manifest.json could not be read",
            ));
            return finish(engine, toolchain, checks);
        }
    };
    let manifest = match serde_json::from_slice::<PackageManifest>(&manifest_bytes) {
        Ok(manifest) => match manifest.validate() {
            Ok(()) => manifest,
            Err(error) => {
                checks.push(failure(
                    "package-manifest",
                    DoctorCheckCode::PackageManifestInvalid,
                    &format!("package-manifest.json is invalid: {error}"),
                ));
                return finish(engine, toolchain, checks);
            }
        },
        Err(_) => {
            checks.push(failure(
                "package-manifest",
                DoctorCheckCode::PackageManifestInvalid,
                "package-manifest.json is not a valid package manifest",
            ));
            return finish(engine, toolchain, checks);
        }
    };
    checks.push(success(
        "package-manifest",
        "package-manifest.json is readable and valid",
    ));

    match request.trusted_manifest_sha256 {
        None => {
            checks.push(failure(
                "manifest-trust-anchor",
                DoctorCheckCode::TrustAnchorMissing,
                "no external manifest digest was provided",
            ));
            return finish(engine, toolchain, checks);
        }
        Some(expected) => {
            if Sha256Digest::from_bytes(&manifest_bytes) != *expected {
                checks.push(failure(
                    "manifest-trust-anchor",
                    DoctorCheckCode::ManifestDigestMismatch,
                    "package-manifest.json does not match the external trust anchor",
                ));
                return finish(engine, toolchain, checks);
            }
            checks.push(success(
                "manifest-trust-anchor",
                "package-manifest.json matches the external digest",
            ));
        }
    }

    match read_toolchain_identity(request.bundle_root) {
        Ok(identity) => {
            if identity.toolchain_fingerprint != manifest.toolchain_fingerprint {
                checks.push(failure(
                    "toolchain-descriptor",
                    DoctorCheckCode::ToolchainDescriptorInvalid,
                    "the toolchain descriptor does not match the package manifest fingerprint",
                ));
                return finish(engine, toolchain, checks);
            }
            checks.push(success(
                "toolchain-descriptor",
                "toolchain descriptor matches the package manifest",
            ));
            toolchain = Some(identity);
        }
        Err(reason) => {
            checks.push(failure(
                "toolchain-descriptor",
                DoctorCheckCode::ToolchainDescriptorInvalid,
                &reason,
            ));
            return finish(engine, toolchain, checks);
        }
    }

    if let Err(check) = check_bundle_files(request.bundle_root, &manifest) {
        checks.push(check);
        return finish(engine, toolchain, checks);
    }
    checks.push(success(
        "bundle-files",
        "every listed file matches its size and hash and no unlisted file is present",
    ));

    match engine.validate() {
        Ok(()) => checks.push(success(
            "engine-executable",
            "the running engine identity is well-formed",
        )),
        Err(_) => {
            checks.push(failure(
                "engine-executable",
                DoctorCheckCode::EngineExecutableInvalid,
                "the running engine identity is invalid",
            ));
            return finish(engine, toolchain, checks);
        }
    }

    if engine.engine_version != manifest.engine.version
        || engine.engine_commit != manifest.engine.commit
        || engine.engine_cache_compatibility_id != manifest.engine.engine_cache_compatibility_id
        || engine.target != manifest.target
        || engine.supported_protocol_versions != manifest.engine.supported_protocol_versions
        || engine.implemented_operations != manifest.engine.implemented_operations
    {
        checks.push(failure(
            "engine-version",
            DoctorCheckCode::EngineVersionMismatch,
            "the running engine does not match the package manifest",
        ));
        return finish(engine, toolchain, checks);
    }
    checks.push(success(
        "engine-version",
        "the running engine matches the package manifest",
    ));

    let mut tool_outputs = Vec::new();
    for tool in &manifest.tools {
        let program = request.bundle_root.join(tool.path.as_str());
        match request.runner.run(&program, &["-version"]) {
            Ok(output) if output.success => tool_outputs.push((tool, output)),
            _ => {
                checks.push(failure(
                    "tool-executables",
                    DoctorCheckCode::ToolExecutableInvalid,
                    "a bundled tool could not be started",
                ));
                return finish(engine, toolchain, checks);
            }
        }
    }
    checks.push(success("tool-executables", "every bundled tool starts"));

    for (tool, output) in &tool_outputs {
        if !output.stdout.contains(tool.version.as_str())
            && !output.stderr.contains(tool.version.as_str())
        {
            checks.push(failure(
                "tool-versions",
                DoctorCheckCode::ToolVersionMismatch,
                "a bundled tool does not report the version recorded in the package manifest",
            ));
            return finish(engine, toolchain, checks);
        }
    }
    checks.push(success(
        "tool-versions",
        "every bundled tool reports the recorded version",
    ));

    let capability_fingerprint = toolchain
        .as_ref()
        .map(|identity| identity.capability_set_fingerprint.clone())
        .expect("toolchain identity is present before the capabilities check");
    if let Err(check) = check_capabilities(request.bundle_root, &manifest, &capability_fingerprint)
    {
        checks.push(check);
        return finish(engine, toolchain, checks);
    }
    checks.push(success(
        "capabilities",
        "capabilities.json is readable and well-formed",
    ));

    if let Err(check) = check_temp_dir() {
        checks.push(check);
        return finish(engine, toolchain, checks);
    }
    checks.push(success(
        "temp-dir",
        "a temporary directory can be created and removed",
    ));

    finish(engine, toolchain, checks)
}

fn finish(
    engine: scene_core_protocol::EngineIdentity,
    toolchain: Option<ToolchainIdentity>,
    checks: Vec<DoctorCheck>,
) -> DoctorOutput {
    let failed = checks
        .iter()
        .any(|check| check.status == DoctorStatus::Failed);
    DoctorOutput {
        schema_version: DescriptorVersion::current(),
        status: if failed {
            DoctorStatus::Failed
        } else {
            DoctorStatus::Ok
        },
        engine,
        toolchain,
        checks,
    }
}

fn check_name(value: &str) -> DoctorCheckName {
    DoctorCheckName::new(value).expect("static doctor check name is well-formed")
}

fn success(name: &str, message: &str) -> DoctorCheck {
    DoctorCheck::ok(check_name(name), message)
}

fn failure(name: &str, code: DoctorCheckCode, message: &str) -> DoctorCheck {
    DoctorCheck::failed(check_name(name), code, message, next_step(code))
}

fn next_step(code: DoctorCheckCode) -> &'static str {
    match code {
        DoctorCheckCode::PackageManifestUnreadable => {
            "extract the bundle again from the trusted release archive"
        }
        DoctorCheckCode::PackageManifestInvalid => {
            "verify the release metadata and extract the bundle again"
        }
        DoctorCheckCode::TrustAnchorMissing => {
            "pass --trusted-manifest-sha256 from the trusted release metadata"
        }
        DoctorCheckCode::ManifestDigestMismatch => {
            "do not use this bundle; download it again from the trusted release metadata"
        }
        DoctorCheckCode::BundleFileInvalid => {
            "extract the bundle again from the trusted release archive"
        }
        DoctorCheckCode::ToolchainDescriptorInvalid => {
            "verify that toolchain-descriptor.json matches the package manifest"
        }
        DoctorCheckCode::EngineExecutableInvalid => {
            "verify the engine binary against the package manifest"
        }
        DoctorCheckCode::EngineVersionMismatch => {
            "use the engine binary recorded in the package manifest"
        }
        DoctorCheckCode::ToolExecutableInvalid => {
            "verify that the bundled tools can run on this target"
        }
        DoctorCheckCode::ToolVersionMismatch => "use the tools recorded in the package manifest",
        DoctorCheckCode::CapabilitiesInvalid => {
            "verify capabilities.json against the recorded capability baseline"
        }
        DoctorCheckCode::TempDirUnavailable => {
            "check free space and temporary directory permissions"
        }
    }
}

fn check_bundle_files(root: &Path, manifest: &PackageManifest) -> Result<(), DoctorCheck> {
    for file in &manifest.files {
        let path = root.join(file.path.as_str());
        if let Err(error) = path_contains_link_or_reparse(root, Path::new(file.path.as_str())) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(failure(
                    "bundle-files",
                    DoctorCheckCode::BundleFileInvalid,
                    "a listed bundle path is a symlink or reparse point",
                ));
            }
        }
        let bytes = fs::read(&path).map_err(|_| {
            failure(
                "bundle-files",
                DoctorCheckCode::BundleFileInvalid,
                "a listed bundle file is missing",
            )
        })?;
        if bytes.len() as u64 != file.byte_size {
            return Err(failure(
                "bundle-files",
                DoctorCheckCode::BundleFileInvalid,
                "a listed bundle file does not match its recorded size",
            ));
        }
        if Sha256Digest::from_bytes(&bytes) != file.sha256 {
            return Err(failure(
                "bundle-files",
                DoctorCheckCode::BundleFileInvalid,
                "a listed bundle file does not match its recorded hash",
            ));
        }
    }

    let mut expected: BTreeSet<String> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str().to_owned())
        .collect();
    expected.insert(PackageManifest::SELF_PATH.to_owned());

    let mut present = BTreeSet::new();
    collect_files(root, root, &mut present).map_err(|_| {
        failure(
            "bundle-files",
            DoctorCheckCode::BundleFileInvalid,
            "the bundle directory could not be walked",
        )
    })?;
    if !present.is_subset(&expected) {
        return Err(failure(
            "bundle-files",
            DoctorCheckCode::BundleFileInvalid,
            "the bundle contains a file that the package manifest does not list",
        ));
    }
    Ok(())
}

fn collect_files(root: &Path, dir: &Path, files: &mut BTreeSet<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if is_link_or_reparse(&metadata) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bundle contains a symlink or reparse point",
            ));
        }
        if file_type.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            if let Ok(relative) = entry.path().strip_prefix(root) {
                let mut parts = Vec::new();
                for component in relative.components() {
                    if let std::path::Component::Normal(part) = component {
                        parts.push(part.to_string_lossy().into_owned());
                    }
                }
                files.insert(parts.join("/"));
            }
        }
    }
    Ok(())
}

fn path_contains_link_or_reparse(root: &Path, relative: &Path) -> std::io::Result<()> {
    let root_metadata = fs::symlink_metadata(root)?;
    if is_link_or_reparse(&root_metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bundle root is a symlink or reparse point",
        ));
    }

    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bundle path contains a non-normal component",
            ));
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current)?;
        if is_link_or_reparse(&metadata) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bundle path contains a symlink or reparse point",
            ));
        }
    }
    Ok(())
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn check_capabilities(
    root: &Path,
    manifest: &PackageManifest,
    expected_fingerprint: &Sha256Digest,
) -> Result<(), DoctorCheck> {
    let path = root.join(manifest.capabilities_ref.as_str());
    let bytes = fs::read(&path).map_err(|_| {
        failure(
            "capabilities",
            DoctorCheckCode::CapabilitiesInvalid,
            "capabilities.json could not be read",
        )
    })?;
    if Sha256Digest::from_bytes(&bytes) != *expected_fingerprint {
        return Err(failure(
            "capabilities",
            DoctorCheckCode::CapabilitiesInvalid,
            "capabilities.json does not match the digest recorded in the toolchain descriptor",
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        failure(
            "capabilities",
            DoctorCheckCode::CapabilitiesInvalid,
            "capabilities.json is not valid JSON",
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        failure(
            "capabilities",
            DoctorCheckCode::CapabilitiesInvalid,
            "capabilities.json must be an object",
        )
    })?;
    for key in ["demuxers", "decoders", "encoders", "filters", "protocols"] {
        let lists = object
            .get(key)
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                failure(
                    "capabilities",
                    DoctorCheckCode::CapabilitiesInvalid,
                    "capabilities.json is missing a capability list",
                )
            })?;
        if lists.is_empty()
            || !lists
                .iter()
                .all(|item| item.as_str().is_some_and(|name| !name.is_empty()))
        {
            return Err(failure(
                "capabilities",
                DoctorCheckCode::CapabilitiesInvalid,
                "capability lists must contain non-empty names",
            ));
        }
    }
    Ok(())
}

fn check_temp_dir() -> Result<(), DoctorCheck> {
    let dir = std::env::temp_dir().join(format!("scene-core-doctor-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|_| {
        failure(
            "temp-dir",
            DoctorCheckCode::TempDirUnavailable,
            "the temporary directory could not be created",
        )
    })?;
    fs::remove_dir_all(&dir).map_err(|_| {
        failure(
            "temp-dir",
            DoctorCheckCode::TempDirUnavailable,
            "the temporary directory could not be removed",
        )
    })
}

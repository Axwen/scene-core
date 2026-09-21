//! Doctor contract tests against synthetic bundles with an injected runner.

use scene_core_engine::doctor::{DoctorRequest, run_doctor};
use scene_core_engine::identity::engine_identity;
use scene_core_engine::runner::{CommandOutput, CommandRunner};
use scene_core_protocol::{
    CacheCompatibilityId, DoctorCheck, DoctorCheckCode, DoctorOutput, DoctorStatus,
    LicenseExpression, PackageManifest, PackagedEngine, PackagedFile, PackagedTool,
    ProtocolVersion, RelativeRef, Sha256Digest, ToolName, ToolchainDescriptor, ToolchainLibrary,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn digest(character: char) -> Sha256Digest {
    Sha256Digest::new(format!("sha256:{}", character.to_string().repeat(64))).expect("digest")
}

struct FakeRunner {
    outputs: HashMap<String, CommandOutput>,
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &Path, _args: &[&str]) -> Result<CommandOutput, String> {
        let name = program
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.outputs
            .get(&name)
            .cloned()
            .ok_or_else(|| "unknown tool".to_owned())
    }
}

fn healthy_runner() -> FakeRunner {
    let mut outputs = HashMap::new();
    outputs.insert(
        "ffmpeg.exe".to_owned(),
        CommandOutput {
            success: true,
            stdout: "ffmpeg version 7.1.1 Copyright (c) 2000-2025".to_owned(),
            stderr: String::new(),
        },
    );
    outputs.insert(
        "ffprobe.exe".to_owned(),
        CommandOutput {
            success: true,
            stdout: "ffprobe version 7.1.1 Copyright (c) 2000-2025".to_owned(),
            stderr: String::new(),
        },
    );
    FakeRunner { outputs }
}

struct Bundle {
    root: PathBuf,
    manifest_bytes: Vec<u8>,
    trusted: Sha256Digest,
}

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("scene-core-test-{}-{name}", std::process::id()))
}

fn write(root: &Path, relative: &str, bytes: &[u8]) -> PackagedFile {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&path, bytes).expect("write fixture file");
    PackagedFile {
        path: RelativeRef::new(relative).expect("ref"),
        byte_size: bytes.len() as u64,
        sha256: Sha256Digest::from_bytes(bytes),
    }
}

fn tool(name: &str, path: &str) -> PackagedTool {
    PackagedTool {
        name: ToolName::new(name).expect("tool name"),
        path: RelativeRef::new(path).expect("ref"),
        version: "7.1.1".to_owned(),
        source_url: "https://example.com/ffmpeg-7.1.1.tar.xz".to_owned(),
        source_sha256: digest('9'),
        license_profile: LicenseExpression::new("LGPL-2.1-or-later").expect("license"),
    }
}

fn build_bundle(name: &str) -> Bundle {
    let root = temp_root(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create bundle root");

    let engine = engine_identity();
    let avcodec = write(&root, "bin/avcodec-63.dll", b"avcodec-dll");
    let avutil = write(&root, "bin/avutil-61.dll", b"avutil-dll");
    let mut files = vec![
        write(&root, "SBOM.spdx.json", b"{\"spdxVersion\":\"SPDX-2.3\"}"),
        write(&root, "THIRD_PARTY_LICENSES/LICENSE.ffmpeg", b"LICENSE"),
        avcodec.clone(),
        avutil.clone(),
        write(&root, "bin/ffmpeg.exe", b"ffmpeg-binary"),
        write(&root, "bin/ffprobe.exe", b"ffprobe-binary"),
        write(&root, "bin/scene-core.exe", b"engine-binary"),
    ];
    let capabilities_bytes = br#"{"capabilitiesVersion":"1","target":"x86_64-pc-windows-msvc","demuxers":["matroska"],"decoders":["h264"],"encoders":["mjpeg"],"filters":["scale"],"protocols":["file"]}"#.to_vec();
    let capability_set_fingerprint = Sha256Digest::from_bytes(&capabilities_bytes);
    files.push(write(&root, "capabilities.json", &capabilities_bytes));

    let descriptor = ToolchainDescriptor {
        descriptor_version: scene_core_protocol::DescriptorVersion::current(),
        target: engine.target.clone(),
        ffmpeg_version: "7.1.1".to_owned(),
        ffprobe_version: "7.1.1".to_owned(),
        toolchain_lock_sha256: digest('9'),
        configure_flags: vec!["--enable-version3".to_owned(), "--enable-shared".to_owned()],
        capability_set_fingerprint,
        shared_libraries: vec![
            ToolchainLibrary {
                path: RelativeRef::new("bin/avcodec-63.dll").expect("ref"),
                sha256: avcodec.sha256.clone(),
            },
            ToolchainLibrary {
                path: RelativeRef::new("bin/avutil-61.dll").expect("ref"),
                sha256: avutil.sha256.clone(),
            },
        ],
    };
    descriptor.validate().expect("fixture descriptor is valid");
    let descriptor_bytes = serde_json::to_vec_pretty(&descriptor).expect("descriptor serializes");
    files.push(write(&root, "toolchain-descriptor.json", &descriptor_bytes));
    files.sort_by(|left, right| {
        left.path
            .as_str()
            .as_bytes()
            .cmp(right.path.as_str().as_bytes())
    });
    let toolchain_fingerprint = ToolchainDescriptor::fingerprint(&descriptor_bytes);

    let manifest = PackageManifest {
        package_manifest_version: scene_core_protocol::DescriptorVersion::current(),
        name: "scene-core".to_owned(),
        package_version: engine.engine_version.clone(),
        target: engine.target.clone(),
        distribution_profile: scene_core_protocol::DistributionProfile::Lgpl,
        toolchain_fingerprint,
        toolchain_descriptor_ref: RelativeRef::new("toolchain-descriptor.json").expect("ref"),
        capabilities_ref: RelativeRef::new("capabilities.json").expect("ref"),
        engine: PackagedEngine {
            path: RelativeRef::new("bin/scene-core.exe").expect("ref"),
            version: engine.engine_version.clone(),
            commit: engine.engine_commit.clone(),
            engine_cache_compatibility_id: engine.engine_cache_compatibility_id.clone(),
            supported_protocol_versions: vec![ProtocolVersion::current()],
            implemented_operations: engine.implemented_operations.clone(),
        },
        tools: vec![
            tool("ffmpeg", "bin/ffmpeg.exe"),
            tool("ffprobe", "bin/ffprobe.exe"),
        ],
        files,
        sbom_ref: RelativeRef::new("SBOM.spdx.json").expect("ref"),
        third_party_licenses_ref: RelativeRef::new("THIRD_PARTY_LICENSES").expect("ref"),
    };
    manifest.validate().expect("fixture manifest is valid");
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest serializes");
    fs::write(root.join(PackageManifest::SELF_PATH), &manifest_bytes).expect("write manifest");
    let trusted = Sha256Digest::from_bytes(&manifest_bytes);

    Bundle {
        root,
        manifest_bytes,
        trusted,
    }
}

fn replace_file(bundle: &mut Bundle, relative: &str, bytes: &[u8]) {
    fs::write(bundle.root.join(relative), bytes).expect("write replacement");
    let mut manifest: PackageManifest =
        serde_json::from_slice(&bundle.manifest_bytes).expect("manifest parses");
    let entry = manifest
        .files
        .iter_mut()
        .find(|file| file.path.as_str() == relative)
        .expect("listed file");
    entry.byte_size = bytes.len() as u64;
    entry.sha256 = Sha256Digest::from_bytes(bytes);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest serializes");
    fs::write(
        bundle.root.join(PackageManifest::SELF_PATH),
        &manifest_bytes,
    )
    .expect("write manifest");
    bundle.trusted = Sha256Digest::from_bytes(&manifest_bytes);
    bundle.manifest_bytes = manifest_bytes;
}

fn doctor<'a>(
    bundle: &'a Bundle,
    trusted: Option<&'a Sha256Digest>,
    runner: &'a dyn CommandRunner,
) -> DoctorOutput {
    run_doctor(DoctorRequest {
        bundle_root: &bundle.root,
        trusted_manifest_sha256: trusted,
        runner,
    })
}

fn check<'a>(output: &'a DoctorOutput, name: &str) -> &'a DoctorCheck {
    output
        .checks
        .iter()
        .find(|check| check.name.as_str() == name)
        .unwrap_or_else(|| panic!("{name} check exists"))
}

fn check_names(output: &DoctorOutput) -> Vec<&str> {
    output
        .checks
        .iter()
        .map(|check| check.name.as_str())
        .collect()
}

#[test]
fn healthy_bundle_passes_with_ordered_checks() {
    let bundle = build_bundle("healthy");
    let runner = healthy_runner();
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);

    output.validate().expect("doctor output validates");
    assert_eq!(output.status, DoctorStatus::Ok);
    assert_eq!(output.exit_code(), 0);
    assert_eq!(
        check_names(&output),
        vec![
            "package-manifest",
            "manifest-trust-anchor",
            "toolchain-descriptor",
            "bundle-files",
            "engine-executable",
            "engine-version",
            "tool-executables",
            "tool-versions",
            "capabilities",
            "temp-dir",
        ]
    );
    assert!(output.toolchain.is_some());
    let _ = fs::remove_dir_all(&bundle.root);
}

#[test]
fn missing_and_tampered_files_fail_bundle_files() {
    let bundle = build_bundle("bundle-files");
    fs::remove_file(bundle.root.join("bin/ffmpeg.exe")).expect("remove tool");
    let runner = healthy_runner();
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(output.status, DoctorStatus::Failed);
    assert_eq!(output.exit_code(), 2);
    assert_eq!(
        check(&output, "bundle-files").code,
        Some(DoctorCheckCode::BundleFileInvalid)
    );
    let _ = fs::remove_dir_all(&bundle.root);

    let bundle = build_bundle("tampered");
    fs::write(bundle.root.join("bin/ffmpeg.exe"), b"tampered-binary").expect("tamper");
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "bundle-files").code,
        Some(DoctorCheckCode::BundleFileInvalid)
    );
    let _ = fs::remove_dir_all(&bundle.root);

    let bundle = build_bundle("unlisted");
    fs::write(bundle.root.join("extra.dll"), b"extra").expect("extra file");
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "bundle-files").code,
        Some(DoctorCheckCode::BundleFileInvalid)
    );
    let _ = fs::remove_dir_all(&bundle.root);
}

#[cfg(unix)]
#[test]
fn listed_symlinked_file_fails_bundle_files() {
    let bundle = build_bundle("symlinked-file");
    let outside = temp_root("symlinked-file-target");
    fs::write(&outside, b"ffmpeg-binary").expect("write target");
    fs::remove_file(bundle.root.join("bin/ffmpeg.exe")).expect("remove regular file");
    std::os::unix::fs::symlink(&outside, bundle.root.join("bin/ffmpeg.exe"))
        .expect("create symlink");

    let output = doctor(&bundle, Some(&bundle.trusted), &healthy_runner());
    assert_eq!(
        check(&output, "bundle-files").code,
        Some(DoctorCheckCode::BundleFileInvalid)
    );

    let _ = fs::remove_file(bundle.root.join("bin/ffmpeg.exe"));
    let _ = fs::remove_dir_all(&bundle.root);
    let _ = fs::remove_file(outside);
}

#[cfg(windows)]
#[test]
fn listed_reparse_file_fails_bundle_files() {
    use std::os::windows::fs::symlink_file;

    let bundle = build_bundle("reparse-file");
    let outside = temp_root("reparse-file-target");
    fs::write(&outside, b"ffmpeg-binary").expect("write target");
    fs::remove_file(bundle.root.join("bin/ffmpeg.exe")).expect("remove regular file");
    symlink_file(&outside, bundle.root.join("bin/ffmpeg.exe"))
        .expect("Windows CI must allow test symlink creation");

    let output = doctor(&bundle, Some(&bundle.trusted), &healthy_runner());
    assert_eq!(
        check(&output, "bundle-files").code,
        Some(DoctorCheckCode::BundleFileInvalid)
    );

    let _ = fs::remove_file(bundle.root.join("bin/ffmpeg.exe"));
    let _ = fs::remove_dir_all(&bundle.root);
    let _ = fs::remove_file(outside);
}

#[test]
fn trust_anchor_is_required_and_verified() {
    let bundle = build_bundle("trust");
    let runner = healthy_runner();

    let output = doctor(&bundle, None, &runner);
    assert_eq!(
        check(&output, "manifest-trust-anchor").code,
        Some(DoctorCheckCode::TrustAnchorMissing)
    );
    assert_eq!(output.exit_code(), 2);

    let wrong = digest('f');
    let output = doctor(&bundle, Some(&wrong), &runner);
    assert_eq!(
        check(&output, "manifest-trust-anchor").code,
        Some(DoctorCheckCode::ManifestDigestMismatch)
    );

    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(output.status, DoctorStatus::Ok);
    let _ = fs::remove_dir_all(&bundle.root);
}

#[test]
fn unreadable_and_invalid_manifests_fail_early() {
    let bundle = build_bundle("manifest-missing");
    fs::remove_file(bundle.root.join(PackageManifest::SELF_PATH)).expect("remove manifest");
    let runner = healthy_runner();
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "package-manifest").code,
        Some(DoctorCheckCode::PackageManifestUnreadable)
    );
    let _ = fs::remove_dir_all(&bundle.root);

    let bundle = build_bundle("manifest-invalid");
    let mut value: serde_json::Value =
        serde_json::from_slice(&bundle.manifest_bytes).expect("manifest value");
    value["tools"] = serde_json::json!([]);
    fs::write(
        bundle.root.join(PackageManifest::SELF_PATH),
        serde_json::to_vec(&value).expect("serialize"),
    )
    .expect("write");
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "package-manifest").code,
        Some(DoctorCheckCode::PackageManifestInvalid)
    );
    let _ = fs::remove_dir_all(&bundle.root);
}

#[test]
fn toolchain_descriptor_must_match_the_manifest() {
    let bundle = build_bundle("toolchain-mismatch");
    let replacement = ToolchainDescriptor {
        descriptor_version: scene_core_protocol::DescriptorVersion::current(),
        target: engine_identity().target,
        ffmpeg_version: "6.0".to_owned(),
        ffprobe_version: "6.0".to_owned(),
        toolchain_lock_sha256: digest('9'),
        configure_flags: vec!["--enable-shared".to_owned()],
        capability_set_fingerprint: digest('d'),
        shared_libraries: vec![ToolchainLibrary {
            path: RelativeRef::new("bin/avcodec-63.dll").expect("ref"),
            sha256: digest('1'),
        }],
    };
    fs::write(
        bundle.root.join("toolchain-descriptor.json"),
        serde_json::to_vec_pretty(&replacement).expect("serialize"),
    )
    .expect("write descriptor");
    let runner = healthy_runner();
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "toolchain-descriptor").code,
        Some(DoctorCheckCode::ToolchainDescriptorInvalid)
    );
    assert!(output.toolchain.is_none());
    output.validate().expect("failed output validates");
    let _ = fs::remove_dir_all(&bundle.root);
}

#[test]
fn tool_and_capability_failures_are_stable() {
    let bundle = build_bundle("tool-start-failure");
    let mut outputs = healthy_runner().outputs;
    outputs.insert(
        "ffmpeg.exe".to_owned(),
        CommandOutput {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
        },
    );
    let runner = FakeRunner { outputs };
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "tool-executables").code,
        Some(DoctorCheckCode::ToolExecutableInvalid)
    );
    let _ = fs::remove_dir_all(&bundle.root);

    let bundle = build_bundle("tool-version-mismatch");
    let mut outputs = healthy_runner().outputs;
    outputs.insert(
        "ffprobe.exe".to_owned(),
        CommandOutput {
            success: true,
            stdout: "ffprobe version 6.0".to_owned(),
            stderr: String::new(),
        },
    );
    let runner = FakeRunner { outputs };
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "tool-versions").code,
        Some(DoctorCheckCode::ToolVersionMismatch)
    );
    let _ = fs::remove_dir_all(&bundle.root);

    let mut bundle = build_bundle("capabilities-invalid");
    replace_file(&mut bundle, "capabilities.json", b"not-json");
    let runner = healthy_runner();
    let output = doctor(&bundle, Some(&bundle.trusted), &runner);
    assert_eq!(
        check(&output, "capabilities").code,
        Some(DoctorCheckCode::CapabilitiesInvalid)
    );
    let _ = fs::remove_dir_all(&bundle.root);
}

#[test]
fn engine_identity_can_report_a_valid_cache_id() {
    let engine = engine_identity();
    let cache_id = CacheCompatibilityId::new("scene-core-output-v1").expect("cache id");
    assert_eq!(engine.engine_cache_compatibility_id, cache_id);
}

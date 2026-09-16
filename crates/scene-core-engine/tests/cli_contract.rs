//! CLI process contract: one JSON object on stdout and a stable exit code.

use scene_core_protocol::{CliErrorOutput, DoctorOutput, DoctorStatus, ErrorCode};
use std::fs;
use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_scene-core"))
        .args(args)
        .output()
        .expect("run scene-core binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn version_without_bundle_reports_controlled_error() {
    let (code, stdout) = run(&["version", "--json"]);
    assert_eq!(code, 2);
    let error: CliErrorOutput = serde_json::from_str(stdout.trim()).expect("cli error JSON");
    assert_eq!(error.code, ErrorCode::ToolUnavailable);
    error.validate().expect("valid cli error output");
}

#[test]
fn doctor_without_manifest_reports_failed_report() {
    let dir = std::env::temp_dir().join(format!("scene-core-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create bundle root");
    let trusted = format!("sha256:{}", "a".repeat(64));
    let (code, stdout) = run(&[
        "doctor",
        "--json",
        "--bundle-root",
        dir.to_str().expect("utf-8 path"),
        "--trusted-manifest-sha256",
        &trusted,
    ]);
    assert_eq!(code, 2);
    let report: DoctorOutput = serde_json::from_str(stdout.trim()).expect("doctor JSON");
    assert_eq!(report.status, DoctorStatus::Failed);
    assert_eq!(report.checks[0].name.as_str(), "package-manifest");
    report.validate().expect("valid doctor report");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn invalid_invocations_report_invalid_request() {
    for args in [
        Vec::new(),
        vec!["version"],
        vec!["nonsense", "--json"],
        vec!["doctor", "--json", "--unknown-flag"],
    ] {
        let (code, stdout) = run(&args);
        assert_eq!(code, 2, "{args:?}");
        let error: CliErrorOutput = serde_json::from_str(stdout.trim()).expect("cli error JSON");
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{args:?}");
        error.validate().expect("valid cli error output");
    }
}

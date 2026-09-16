//! Argument parsing and stdout contract for `version --json` / `doctor --json`.

use crate::doctor::{DoctorRequest, run_doctor};
use crate::identity::{engine_identity, read_toolchain_identity};
use crate::runner::RealCommandRunner;
use scene_core_protocol::{CliErrorOutput, ErrorCode, Sha256Digest, VersionOutput};
use std::path::PathBuf;

pub struct CliOutput {
    pub json: String,
    pub exit_code: u8,
}

/// Runs the standalone CLI and returns the single stdout JSON object.
pub fn run(args: &[String]) -> Result<CliOutput, CliErrorOutput> {
    let Some((command, rest)) = args.split_first() else {
        return Err(invalid("expected 'version' or 'doctor'"));
    };
    match command.as_str() {
        "version" => run_version(rest),
        "doctor" => run_doctor_command(rest),
        other => Err(invalid(&format!("unknown command '{other}'"))),
    }
}

fn run_version(args: &[String]) -> Result<CliOutput, CliErrorOutput> {
    expect_json_flag(args)?;
    let engine = engine_identity();
    let root = executable_root()?;
    let toolchain = read_toolchain_identity(&root).map_err(|reason| {
        CliErrorOutput::new(
            ErrorCode::ToolUnavailable,
            reason,
            "run the engine from an extracted scene-core bundle",
        )
    })?;
    let output = VersionOutput::from_engine_identity(&engine, toolchain.toolchain_fingerprint);
    output.validate().map_err(|_| {
        CliErrorOutput::new(
            ErrorCode::EngineInternal,
            "the engine identity is invalid",
            "report this build as broken",
        )
    })?;
    Ok(serialize(output, 0))
}

fn run_doctor_command(args: &[String]) -> Result<CliOutput, CliErrorOutput> {
    expect_json_flag(args)?;
    let mut bundle_root = None;
    let mut trusted = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {}
            "--bundle-root" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid("--bundle-root requires a path"))?;
                bundle_root = Some(PathBuf::from(value));
                index += 1;
            }
            "--trusted-manifest-sha256" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid("--trusted-manifest-sha256 requires a digest"))?;
                let digest = value.parse::<Sha256Digest>().map_err(|_| {
                    invalid("--trusted-manifest-sha256 must be sha256:<64 lowercase hex>")
                })?;
                trusted = Some(digest);
                index += 1;
            }
            other => return Err(invalid(&format!("unknown argument '{other}'"))),
        }
        index += 1;
    }

    let bundle_root = match bundle_root {
        Some(path) => path,
        None => executable_root()?,
    };
    let runner = RealCommandRunner;
    let output = run_doctor(DoctorRequest {
        bundle_root: &bundle_root,
        trusted_manifest_sha256: trusted.as_ref(),
        runner: &runner,
    });
    output.validate().map_err(|_| {
        CliErrorOutput::new(
            ErrorCode::EngineInternal,
            "the doctor report is invalid",
            "report this build as broken",
        )
    })?;
    let exit_code = output.exit_code();
    Ok(serialize(output, exit_code))
}

fn expect_json_flag(args: &[String]) -> Result<(), CliErrorOutput> {
    if args.iter().any(|argument| argument == "--json") {
        Ok(())
    } else {
        Err(invalid("only --json output is supported"))
    }
}

fn executable_root() -> Result<PathBuf, CliErrorOutput> {
    let executable = std::env::current_exe().map_err(|_| {
        CliErrorOutput::new(
            ErrorCode::EngineInternal,
            "the engine executable path is unavailable",
            "run the engine from an extracted scene-core bundle",
        )
    })?;
    executable.parent().map(PathBuf::from).ok_or_else(|| {
        CliErrorOutput::new(
            ErrorCode::EngineInternal,
            "the engine executable path has no parent directory",
            "run the engine from an extracted scene-core bundle",
        )
    })
}

fn invalid(message: &str) -> CliErrorOutput {
    CliErrorOutput::new(
        ErrorCode::InvalidRequest,
        message,
        "run 'scene-core version --json' or 'scene-core doctor --json'",
    )
}

fn serialize<T: serde::Serialize>(value: T, exit_code: u8) -> CliOutput {
    CliOutput {
        json: serde_json::to_string(&value).expect("CLI output serializes"),
        exit_code,
    }
}

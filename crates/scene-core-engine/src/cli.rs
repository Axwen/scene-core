//! Argument parsing and stdout contract for `version --json` / `doctor --json`.

use crate::doctor::{DoctorRequest, run_doctor};
use crate::identity::{engine_identity, read_toolchain_identity, resolve_bundle_root};
use crate::run::{self, ProbeBackend, RunControl};
use crate::runner::RealCommandRunner;
use scene_core_media::toolchain::Toolchain;
use scene_core_protocol::{
    CliErrorOutput, ControlMessage, ErrorCode, EventEnvelope, EventType, Identifier, Operation,
    ProtocolError, ProtocolVersion, Sha256Digest, StartRequest, VersionOutput, parse_control_line,
};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

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
        "run" => run_run_command(rest),
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

fn run_run_command(args: &[String]) -> Result<CliOutput, CliErrorOutput> {
    let mut staging_root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--staging-root" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid("--staging-root requires a path"))?;
                staging_root = Some(PathBuf::from(value));
                index += 1;
            }
            other => return Err(invalid(&format!("unknown argument '{other}'"))),
        }
        index += 1;
    }
    let staging_root = staging_root.ok_or_else(|| invalid("run requires --staging-root <path>"))?;

    let engine = engine_identity();

    let stdin = std::io::stdin();
    let first = {
        let mut lines = stdin.lock().lines();
        match lines.next() {
            Some(Ok(line)) => line,
            Some(Err(_)) => {
                return Ok(CliOutput {
                    json: String::new(),
                    exit_code: 2,
                });
            }
            None => {
                eprintln!("scene-core run: no StartRequest on stdin");
                return Ok(CliOutput {
                    json: String::new(),
                    exit_code: 2,
                });
            }
        }
    };

    let request = match StartRequest::parse(&first) {
        Ok(request) => request,
        Err(parse_error) => {
            if let Some(echo) = lenient_echo(&first) {
                let error = if echo.version_supported {
                    run::invalid_request_error()
                } else {
                    run::unsupported_protocol_error()
                };
                let exit_code = emit_events(|emit| {
                    emit_failed(&echo.echo, &engine, error, emit);
                    2
                });
                return Ok(CliOutput {
                    json: String::new(),
                    exit_code,
                });
            }
            eprintln!("scene-core run: {}", parse_error.message);
            return Ok(CliOutput {
                json: String::new(),
                exit_code: parse_error.code.exit_code(),
            });
        }
    };

    let toolchain_fingerprint = resolve_toolchain_fingerprint()?;
    let verified =
        match run::validate_request(&request, &engine, &toolchain_fingerprint, &staging_root) {
            Ok(verified) => verified,
            Err(error) => {
                let echo = run::Echo::from_request(&request);
                let exit_code = emit_events(|emit| {
                    emit_failed(&echo, &engine, *error, emit);
                    2
                });
                return Ok(CliOutput {
                    json: String::new(),
                    exit_code,
                });
            }
        };

    let control = RunControl::new();
    {
        let expected = request.request_id.clone();
        let control = control.clone();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                match parse_control_line(&line) {
                    Ok(ControlMessage::Cancel(cancel)) if cancel.request_id == expected => {
                        control.request_cancel();
                    }
                    Ok(_) => control.record_violation(
                        ProtocolError::invalid_request(
                            "only one StartRequest and one matching CancelRequest are allowed",
                        )
                        .with_stage("framing"),
                    ),
                    Err(error) => control.record_violation(*error),
                }
            }
        });
    }
    {
        let control = control.clone();
        let deadline = Duration::from_millis(request.effective_deadline_ms());
        std::thread::spawn(move || {
            std::thread::sleep(deadline);
            control.request_timeout();
        });
    }

    let backend: Box<dyn run::MediaBackend> = match resolve_toolchain_binaries() {
        Ok(toolchain) => Box::new(ProbeBackend { toolchain }),
        Err(_) => Box::new(run::UnavailableBackend),
    };
    let exit_code = emit_events(|emit| {
        run::run_session(
            &request,
            &engine,
            backend.as_ref(),
            &verified.input,
            &control,
            emit,
        )
    });
    Ok(CliOutput {
        json: String::new(),
        exit_code,
    })
}

fn emit_events(action: impl FnOnce(&mut dyn FnMut(EventEnvelope)) -> u8) -> u8 {
    let stdout = std::io::stdout();
    let mut emit = |event: EventEnvelope| {
        let line = serde_json::to_string(&event).expect("event serializes");
        let mut handle = stdout.lock();
        let _ = writeln!(handle, "{line}");
        let _ = handle.flush();
    };
    action(&mut emit)
}

fn emit_failed(
    echo: &run::Echo,
    engine: &scene_core_protocol::EngineIdentity,
    error: ProtocolError,
    emit: &mut dyn FnMut(EventEnvelope),
) {
    let mut envelope = run::event_for(echo, engine, 1, EventType::Failed, None, None, None);
    envelope.error = Some(error);
    emit(envelope);
}

struct LenientEcho {
    echo: run::Echo,
    version_supported: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LenientRequest {
    engine_protocol_version: ProtocolVersion,
    request_id: Identifier,
    operation: Operation,
    source_version_id: Identifier,
    input_fingerprint: Sha256Digest,
    derivation_key: Sha256Digest,
    execution_context: scene_core_protocol::ExecutionContext,
}

fn lenient_echo(line: &str) -> Option<LenientEcho> {
    let parsed: LenientRequest = serde_json::from_str(line).ok()?;
    Some(LenientEcho {
        echo: run::Echo {
            request_id: parsed.request_id,
            source_version_id: parsed.source_version_id,
            input_fingerprint: parsed.input_fingerprint,
            derivation_key: parsed.derivation_key,
            operation: parsed.operation,
            execution_context: parsed.execution_context,
        },
        version_supported: parsed.engine_protocol_version.is_supported(),
    })
}

fn resolve_toolchain_fingerprint() -> Result<Sha256Digest, CliErrorOutput> {
    if let Ok(value) = std::env::var("SCENE_CORE_TOOLCHAIN_FINGERPRINT") {
        return value.parse().map_err(|_| {
            CliErrorOutput::new(
                ErrorCode::ToolUnavailable,
                "SCENE_CORE_TOOLCHAIN_FINGERPRINT is not a valid digest",
                "use the fingerprint of the toolchain descriptor from 'version --json'",
            )
        });
    }
    let root = resolve_bundle_root().map_err(|reason| {
        CliErrorOutput::new(
            ErrorCode::ToolUnavailable,
            reason,
            "run the engine from an extracted scene-core bundle",
        )
    })?;
    read_toolchain_identity(&root)
        .map(|identity| identity.toolchain_fingerprint)
        .map_err(|reason| {
            CliErrorOutput::new(
                ErrorCode::ToolUnavailable,
                reason,
                "run the engine from an extracted scene-core bundle",
            )
        })
}

fn resolve_toolchain_binaries() -> Result<Toolchain, CliErrorOutput> {
    if let Some(bin_dir) = std::env::var_os(scene_core_media::toolchain::ENV_FFMPEG_DIR) {
        return Toolchain::from_bin_dir(bin_dir).map_err(|_| {
            CliErrorOutput::new(
                ErrorCode::ToolUnavailable,
                "the toolchain bin directory is incomplete",
                "run scripts/fetch-toolchain.sh and set SCENE_CORE_FFMPEG_DIR",
            )
        });
    }
    let root = resolve_bundle_root().map_err(|reason| {
        CliErrorOutput::new(
            ErrorCode::ToolUnavailable,
            reason,
            "run the engine from an extracted scene-core bundle",
        )
    })?;
    Toolchain::from_bin_dir(root.join("bin")).map_err(|_| {
        CliErrorOutput::new(
            ErrorCode::ToolUnavailable,
            "the bundle bin directory is incomplete",
            "verify the bundle with doctor before running media work",
        )
    })
}

fn expect_json_flag(args: &[String]) -> Result<(), CliErrorOutput> {
    if args.iter().any(|argument| argument == "--json") {
        Ok(())
    } else {
        Err(invalid("only --json output is supported"))
    }
}

fn executable_root() -> Result<PathBuf, CliErrorOutput> {
    resolve_bundle_root().map_err(|reason| {
        CliErrorOutput::new(
            ErrorCode::EngineInternal,
            reason,
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

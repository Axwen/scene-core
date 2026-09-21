//! Argument parsing and stdout contract for `version --json` / `doctor --json`.

use crate::doctor::{DoctorRequest, run_doctor};
use crate::identity::{engine_identity, read_toolchain_identity, resolve_bundle_root};
use crate::run::{self, ProbeBackend, RunControl};
use crate::runner::RealCommandRunner;
use scene_core_media::staging::StagingRoot;
use scene_core_media::toolchain::Toolchain;
use scene_core_protocol::{
    CliErrorOutput, ErrorCode, EventEnvelope, EventType, Identifier, MAX_CONTROL_LINE_BYTES,
    Operation, ProtocolError, ProtocolVersion, Sha256Digest, StartRequest, VersionOutput,
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

    let first = match read_first_line() {
        Ok(Some(line)) => line,
        Ok(None) => {
            eprintln!("scene-core run: no StartRequest on stdin");
            return Ok(CliOutput {
                json: String::new(),
                exit_code: 2,
            });
        }
        Err(message) => {
            eprintln!("scene-core run: {message}");
            return Ok(CliOutput {
                json: String::new(),
                exit_code: 2,
            });
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

    // A missing fingerprint means the toolchain is unavailable; the request
    // still gets `accepted` followed by a controlled TOOL_UNAVAILABLE failure
    // instead of a non-protocol CLI error.
    let toolchain_fingerprint = resolve_toolchain_fingerprint();
    let fingerprint_for_session = toolchain_fingerprint
        .as_ref()
        .cloned()
        .unwrap_or_else(|_| Sha256Digest::from_bytes(b"toolchain-unavailable"));

    // Cancellation, the deadline and protocol violations are live from here on,
    // so the staged-input hash in validation is covered by the request deadline
    // and can be interrupted.
    let mut session = run::ControlSession::new(&request, RunControl::new());
    let control = session.control().clone();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut lines = CappedLines::new(stdin.lock(), MAX_CONTROL_LINE_BYTES);
        read_control_lines(&mut lines, &mut session);
    });
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
    let validation = match &toolchain_fingerprint {
        Ok(fingerprint) => {
            run::validate_request(&request, &engine, fingerprint, &staging_root, &control)
        }
        // Without the fingerprint the derivation key cannot be recomputed, so
        // skip validation and let the unavailable backend report the failure.
        Err(_) => StagingRoot::new(&staging_root)
            .map(|staging| run::VerifiedInput { staging })
            .map_err(|_| {
                Box::new(
                    ProtocolError::new(
                        ErrorCode::ToolUnavailable,
                        "the fixed media tool is unavailable",
                    )
                    .with_stage("validation"),
                )
            }),
    };
    let echo = run::Echo::from_request(&request);
    let exit_code = emit_events(|emit| match validation {
        Ok(verified) => run::run_session(
            &request,
            &engine,
            &fingerprint_for_session,
            backend.as_ref(),
            &verified.staging,
            &control,
            emit,
        ),
        Err(error) => {
            // Interrupted while hashing the staged input: the session still
            // emits `accepted` followed by the cancelled or timed-out terminal.
            let interrupted = (error.code == ErrorCode::Cancelled)
                .then(|| StagingRoot::new(&staging_root).ok())
                .flatten();
            match interrupted {
                Some(staging) => run::run_session(
                    &request,
                    &engine,
                    &fingerprint_for_session,
                    &run::UnavailableBackend,
                    &staging,
                    &control,
                    emit,
                ),
                None => {
                    let exit_code = error.code.exit_code();
                    emit_failed(&echo, &engine, *error, emit);
                    exit_code
                }
            }
        }
    });
    Ok(CliOutput {
        json: String::new(),
        exit_code,
    })
}

/// Reads control JSONL until the stream ends or the control state is already
/// decided. Any framing violation stops the loop: the violation is terminal for
/// the run, so reading on would only risk spinning on a broken stream.
fn read_control_lines<R: BufRead>(lines: &mut CappedLines<R>, session: &mut run::ControlSession) {
    loop {
        match lines.next_line() {
            Ok(Some(line)) if line.oversized => {
                session
                    .control()
                    .record_violation(scene_core_protocol::control_line_limit_error(line.bytes));
            }
            Ok(Some(line)) if line.invalid_utf8 => session.control().record_violation(
                ProtocolError::invalid_request("control JSONL must be UTF-8").with_stage("framing"),
            ),
            Ok(Some(line)) => {
                session.handle_line(&line.text);
                if session.control().violation().is_none() {
                    continue;
                }
            }
            Ok(None) => return,
            // A failed read can hide a CancelRequest; surface it instead of
            // spinning on the same error.
            Err(error) => session.control().record_violation(
                ProtocolError::invalid_request(format!("the control channel failed: {error}"))
                    .with_stage("framing"),
            ),
        }
        return;
    }
}

/// Reads the first stdin line with the JSONL line cap applied.
fn read_first_line() -> Result<Option<String>, String> {
    let stdin = std::io::stdin();
    let mut lines = CappedLines::new(stdin.lock(), MAX_CONTROL_LINE_BYTES);
    match lines.next_line() {
        Ok(Some(line)) if line.oversized => Err(format!(
            "the StartRequest line exceeds the {} byte hard limit",
            MAX_CONTROL_LINE_BYTES
        )),
        Ok(Some(line)) if line.invalid_utf8 => {
            Err("the StartRequest line is not valid UTF-8".to_owned())
        }
        Ok(Some(line)) => Ok(Some(line.text)),
        Ok(None) => Ok(None),
        Err(error) => Err(format!("stdin could not be read: {error}")),
    }
}

/// One JSONL line read under a hard byte cap.
struct CappedLine {
    text: String,
    bytes: usize,
    oversized: bool,
    /// Protocol JSONL is UTF-8; a line that is not is a framing error.
    invalid_utf8: bool,
}

impl CappedLine {
    fn new(collected: Vec<u8>, bytes: usize, oversized: bool) -> Self {
        let (text, invalid_utf8) = match String::from_utf8(collected) {
            Ok(text) => (text, false),
            Err(error) => (String::from_utf8_lossy(error.as_bytes()).into_owned(), true),
        };
        Self {
            text,
            bytes,
            oversized,
            invalid_utf8,
        }
    }
}

/// JSONL reader that buffers at most `cap + 1` bytes of one line and stops
/// reading a line as soon as it is known to be oversized, so an endless line
/// cannot block the caller.
struct CappedLines<R> {
    reader: R,
    cap: usize,
}

impl<R: BufRead> CappedLines<R> {
    fn new(reader: R, cap: usize) -> Self {
        Self { reader, cap }
    }

    fn next_line(&mut self) -> std::io::Result<Option<CappedLine>> {
        let mut collected: Vec<u8> = Vec::new();
        // Reading one byte past the cap is enough to know the line is over it,
        // and the cap keeps the read bounded even without a newline.
        let read = std::io::Read::take(&mut self.reader, self.cap as u64 + 1)
            .read_until(b'\n', &mut collected)?;
        if read == 0 {
            return Ok(None);
        }
        let complete = collected.ends_with(b"\n");
        if complete {
            collected.pop();
        }
        let bytes = collected.len();
        // The take limit was hit without a newline, so the line is over the cap.
        // Fewer bytes without a newline means the stream simply ended there.
        Ok(Some(CappedLine::new(
            collected,
            bytes,
            !complete && bytes > self.cap,
        )))
    }
}

fn emit_events(action: impl FnOnce(&mut dyn FnMut(EventEnvelope)) -> u8) -> u8 {
    let stdout = std::io::stdout();
    let undelivered = std::cell::Cell::new(false);
    let mut emit = |event: EventEnvelope| {
        let line = serde_json::to_string(&event).expect("event serializes");
        let written = {
            let mut handle = stdout.lock();
            writeln!(handle, "{line}").and_then(|()| handle.flush())
        };
        if written.is_err() {
            undelivered.set(true);
        }
    };
    let exit_code = action(&mut emit);
    if undelivered.get() {
        // The event stream is the contract; a run whose terminal event could
        // not be delivered must not report success.
        ErrorCode::EngineInternal.exit_code()
    } else {
        exit_code
    }
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

/// Best-effort identity recovery for a StartRequest that failed the strict
/// parse. It deliberately accepts a superset of the schema: the point is to
/// echo the identity of a request that carries unknown or invalid extras.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn lines(input: &str, cap: usize) -> Vec<CappedLine> {
        let mut reader = CappedLines::new(Cursor::new(input.as_bytes().to_vec()), cap);
        let mut read = Vec::new();
        while let Some(line) = reader.next_line().expect("read line") {
            read.push(line);
        }
        read
    }

    #[test]
    fn capped_lines_split_lines_and_flag_oversized_ones() {
        let read = lines("one\n\ntwo", 16);
        assert_eq!(read.len(), 3);
        assert_eq!(read[0].text, "one");
        assert!(!read[0].oversized);
        assert_eq!(read[1].text, "");
        assert_eq!(read[2].text, "two");
        assert!(!read[2].oversized);

        // A line over the cap is reported as soon as the take limit is hit.
        let mut reader = CappedLines::new(Cursor::new(b"0123456789\n".to_vec()), 5);
        let oversized = reader.next_line().expect("read").expect("line");
        assert!(oversized.oversized);
        assert_eq!(oversized.bytes, 6);
        assert_eq!(oversized.text, "012345");

        // Exactly `cap` bytes plus the newline still fits.
        let mut reader = CappedLines::new(Cursor::new(b"01234\n".to_vec()), 5);
        let exact = reader.next_line().expect("read").expect("line");
        assert!(!exact.oversized);
        assert_eq!(exact.bytes, 5);
        assert_eq!(exact.text, "01234");
    }

    #[test]
    fn capped_lines_flag_invalid_utf8() {
        let mut reader = CappedLines::new(Cursor::new(vec![b'a', 0xFF, 0xFE, b'\n']), 16);
        let line = reader.next_line().expect("read").expect("line");
        assert!(line.invalid_utf8);
        assert!(!line.oversized);
        assert!(!line.text.is_empty());

        let read = lines("ok\n", 16);
        assert!(!read[0].invalid_utf8);
    }

    #[test]
    fn capped_lines_stop_at_eof() {
        assert!(lines("", 16).is_empty());
    }

    #[test]
    fn oversized_lines_stop_reading_without_a_newline() {
        let (reader, calls) = scripted(vec![vec![b'a'; 8192]]);
        let mut lines = CappedLines::new(reader, 1024);
        let line = lines
            .next_line()
            .expect("read")
            .expect("one oversized line");
        assert!(line.oversized);
        assert_eq!(line.text.len(), 1025);
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the reader must not keep reading an oversized line"
        );
    }

    /// Serves one fixed chunk per `fill_buf` call, then EOF.
    struct ScriptedReader {
        chunks: Vec<Vec<u8>>,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl std::io::Read for ScriptedReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let available = self.fill_buf()?;
            let take = available.len().min(buffer.len());
            buffer[..take].copy_from_slice(&available[..take]);
            self.consume(take);
            Ok(take)
        }
    }

    impl std::io::BufRead for ScriptedReader {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            let index = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.chunks.get(index).map_or(&[][..], Vec::as_slice))
        }

        fn consume(&mut self, _amount: usize) {}
    }

    fn scripted(
        chunks: Vec<Vec<u8>>,
    ) -> (
        ScriptedReader,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        (
            ScriptedReader {
                chunks,
                calls: calls.clone(),
            },
            calls,
        )
    }

    fn probe_request() -> StartRequest {
        StartRequest::parse(include_str!(
            "../../../fixtures/protocol/valid/control-start-probe.json"
        ))
        .expect("fixture parses")
    }

    #[test]
    fn control_reader_stops_on_an_endless_oversized_line() {
        let mut session = run::ControlSession::new(&probe_request(), RunControl::new());
        let (reader, calls) = scripted(vec![vec![b'a'; 8192], vec![b'a'; 8192]]);
        let mut lines = CappedLines::new(reader, 64);
        read_control_lines(&mut lines, &mut session);
        assert!(session.control().violation().is_some());
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "an oversized control line must stop the reader loop"
        );
    }

    #[test]
    fn control_reader_stops_after_an_invalid_utf8_line() {
        let mut session = run::ControlSession::new(&probe_request(), RunControl::new());
        let (reader, calls) = scripted(vec![vec![0xFF, 0xFE, b'\n'], vec![b'{', b'}', b'\n']]);
        let mut lines = CappedLines::new(reader, 64);
        read_control_lines(&mut lines, &mut session);
        assert!(session.control().violation().is_some());
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "a non-UTF-8 control line must stop the reader loop"
        );
    }

    #[test]
    fn control_reader_stops_after_a_protocol_violation() {
        let mut session = run::ControlSession::new(&probe_request(), RunControl::new());
        let cancel =
            br#"{"engineProtocolVersion":"0.1","messageType":"cancel","requestId":"other"}"#;
        let (reader, calls) = scripted(vec![
            [cancel.as_slice(), b"\n"].concat(),
            vec![b'{', b'}', b'\n'],
        ]);
        let mut lines = CappedLines::new(reader, 256);
        read_control_lines(&mut lines, &mut session);
        assert!(session.control().violation().is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn capped_lines_propagate_read_errors() {
        struct FailingReader;

        impl std::io::Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("simulated stdin failure"))
            }
        }

        impl std::io::BufRead for FailingReader {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                Err(std::io::Error::other("simulated stdin failure"))
            }

            fn consume(&mut self, _amount: usize) {}
        }

        let mut reader = CappedLines::new(FailingReader, 16);
        assert!(reader.next_line().is_err());
    }
}

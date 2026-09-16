//! Process execution boundary: no shell, bounded output, deadline and
//! cancellation via kill.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const DEFAULT_STDOUT_LIMIT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_STDERR_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout: Option<Duration>,
    pub stdout_limit_bytes: usize,
    pub stderr_limit_bytes: usize,
    pub cancellation: Option<Arc<AtomicBool>>,
}

impl ProcessSpec {
    pub fn new(
        program: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            timeout: None,
            stdout_limit_bytes: DEFAULT_STDOUT_LIMIT_BYTES,
            stderr_limit_bytes: DEFAULT_STDERR_LIMIT_BYTES,
            cancellation: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_output_limits(mut self, stdout: usize, stderr: usize) -> Self {
        self.stdout_limit_bytes = stdout;
        self.stderr_limit_bytes = stderr;
        self
    }

    pub fn with_cancellation(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancellation = Some(flag);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

impl ProcessOutput {
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

#[derive(Debug)]
pub enum ProcessError {
    Spawn(std::io::Error),
    Wait(std::io::Error),
    Read(std::io::Error),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Spawn(error) => write!(formatter, "could not start the tool: {error}"),
            ProcessError::Wait(error) => write!(formatter, "could not wait for the tool: {error}"),
            ProcessError::Read(error) => {
                write!(formatter, "could not read the tool output: {error}")
            }
        }
    }
}

impl std::error::Error for ProcessError {}

pub fn run(spec: &ProcessSpec) -> Result<ProcessOutput, ProcessError> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(ProcessError::Spawn)?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let stdout_limit = spec.stdout_limit_bytes;
    let stderr_limit = spec.stderr_limit_bytes;
    let (stdout_sender, stdout_receiver) = std::sync::mpsc::channel();
    let (stderr_sender, stderr_receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = stdout_sender.send(read_capped(stdout, stdout_limit));
    });
    std::thread::spawn(move || {
        let _ = stderr_sender.send(read_capped(stderr, stderr_limit));
    });

    let deadline = spec.timeout.map(|timeout| Instant::now() + timeout);
    let mut timed_out = false;
    let mut cancelled = false;
    let status = loop {
        if let Some(flag) = &spec.cancellation {
            if flag.load(Ordering::Relaxed) {
                cancelled = true;
                let _ = child.kill();
            }
        }
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
            }
        }
        match child.try_wait().map_err(ProcessError::Wait)? {
            Some(status) => break status,
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    };

    let (stdout, stdout_truncated) = collect_output(stdout_receiver).map_err(ProcessError::Read)?;
    let (stderr, stderr_truncated) = collect_output(stderr_receiver).map_err(ProcessError::Read)?;
    Ok(ProcessOutput {
        success: status.success() && !timed_out && !cancelled,
        exit_code: status.code(),
        stdout,
        stderr,
        timed_out,
        cancelled,
        stdout_truncated,
        stderr_truncated,
    })
}

type CappedOutput = std::io::Result<(Vec<u8>, bool)>;

fn collect_output(receiver: std::sync::mpsc::Receiver<CappedOutput>) -> CappedOutput {
    match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(value) => value,
        Err(_) => Ok((Vec::new(), true)),
    }
}

fn read_capped<R: Read>(mut reader: R, limit: usize) -> CappedOutput {
    let mut buffer = [0_u8; 8192];
    let mut collected = Vec::new();
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
            Ok(read) => {
                if collected.len() < limit {
                    let take = (limit - collected.len()).min(read);
                    collected.extend_from_slice(&buffer[..take]);
                    if take < read {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
        }
    }
    Ok((collected, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("simulated read failure"))
        }
    }

    #[test]
    fn read_capped_reports_truncation_and_read_errors() {
        let (bytes, truncated) = read_capped(Cursor::new(b"abcdef".to_vec()), 4).expect("read");
        assert_eq!(bytes, b"abcd");
        assert!(truncated);

        let error = read_capped(FailingReader, 4).expect_err("read error must not look like EOF");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
    }
}

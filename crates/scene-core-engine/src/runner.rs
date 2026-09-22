//! Process execution boundary for doctor tool checks.

use scene_core_media::process::{ProcessSpec, run as run_process};
use std::path::Path;
use std::time::Duration;

/// `-version` must answer promptly; doctor is a startup gate, not a hang.
const TOOL_VERSION_TIMEOUT: Duration = Duration::from_secs(30);
/// Doctor only searches the output for the recorded version string.
const TOOL_VERSION_STDOUT_LIMIT_BYTES: usize = 64 * 1024;
const TOOL_VERSION_STDERR_LIMIT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs one bundled tool. Tests inject a fake runner; production uses
/// [`RealCommandRunner`].
pub trait CommandRunner {
    fn run(&self, program: &Path, args: &[&str]) -> Result<CommandOutput, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run(&self, program: &Path, args: &[&str]) -> Result<CommandOutput, String> {
        let spec = ProcessSpec::new(program, args.iter().copied())
            .with_timeout(TOOL_VERSION_TIMEOUT)
            .with_output_limits(
                TOOL_VERSION_STDOUT_LIMIT_BYTES,
                TOOL_VERSION_STDERR_LIMIT_BYTES,
            );
        let output =
            run_process(&spec).map_err(|_| "the bundled tool could not be started".to_owned())?;
        Ok(CommandOutput {
            success: output.success,
            stdout: output.stdout_text(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_tool_is_a_controlled_error() {
        let runner = RealCommandRunner;
        let error = runner.run(
            Path::new("scene-core-tool-that-does-not-exist"),
            &["-version"],
        );
        assert!(
            error.is_err(),
            "a spawn failure must not look like tool output"
        );
    }
}

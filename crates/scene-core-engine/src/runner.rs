//! Process execution boundary for doctor tool checks.

use std::path::Path;
use std::process::Command;

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
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|_| "the bundled tool could not be started".to_owned())?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

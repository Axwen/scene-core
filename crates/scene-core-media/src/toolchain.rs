//! Fixed toolchain resolution.

use std::fmt;
use std::path::{Path, PathBuf};

pub const ENV_FFMPEG_DIR: &str = "SCENE_CORE_FFMPEG_DIR";
pub const FFMPEG: &str = "ffmpeg";
pub const FFPROBE: &str = "ffprobe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolchainError {
    EnvironmentMissing,
    Missing { name: &'static str, path: PathBuf },
    NotAbsolute(PathBuf),
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolchainError::EnvironmentMissing => write!(
                formatter,
                "{ENV_FFMPEG_DIR} is not set to the extracted toolchain bin directory"
            ),
            ToolchainError::Missing { name, path } => {
                write!(formatter, "{name} is missing at {}", path.display())
            }
            ToolchainError::NotAbsolute(path) => write!(
                formatter,
                "the toolchain bin directory must be absolute: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ToolchainError {}

/// Resolved FFmpeg/FFprobe pair. Tools are always addressed by absolute path;
/// `PATH` is never consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    bin_dir: PathBuf,
}

impl Toolchain {
    pub fn from_bin_dir(bin_dir: impl Into<PathBuf>) -> Result<Self, ToolchainError> {
        let bin_dir = bin_dir.into();
        if !bin_dir.is_absolute() {
            return Err(ToolchainError::NotAbsolute(bin_dir));
        }
        let toolchain = Self { bin_dir };
        for (name, path) in [(FFMPEG, toolchain.ffmpeg()), (FFPROBE, toolchain.ffprobe())] {
            if !path.is_file() {
                return Err(ToolchainError::Missing { name, path });
            }
        }
        Ok(toolchain)
    }

    /// Reads [`ENV_FFMPEG_DIR`], the bin directory produced by
    /// `scripts/fetch-toolchain.sh`.
    pub fn from_env() -> Result<Self, ToolchainError> {
        let bin_dir =
            std::env::var(ENV_FFMPEG_DIR).map_err(|_| ToolchainError::EnvironmentMissing)?;
        Self::from_bin_dir(bin_dir)
    }

    pub fn ffmpeg(&self) -> PathBuf {
        self.bin_dir.join(executable_name(FFMPEG))
    }

    pub fn ffprobe(&self) -> PathBuf {
        self.bin_dir.join(executable_name(FFPROBE))
    }

    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_tools_are_reported() {
        let dir = std::env::temp_dir().join(format!("scene-core-toolchain-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let error = Toolchain::from_bin_dir(&dir).expect_err("empty dir has no tools");
        assert!(matches!(error, ToolchainError::Missing { .. }));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relative_bin_dirs_are_rejected() {
        for relative in ["", "ffmpeg", "tools/bin", "./tools"] {
            let error = Toolchain::from_bin_dir(relative).expect_err("relative dir");
            assert!(matches!(error, ToolchainError::NotAbsolute(_)));
        }
    }
}

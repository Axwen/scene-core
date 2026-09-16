//! Per-request staging root containment.

use scene_core_protocol::RelativeRef;
use std::fmt;
use std::path::{Path, PathBuf};

pub const INPUT_REF: &str = "input/source.media";

#[derive(Debug)]
pub enum StagingError {
    Io(std::io::Error),
    InputMissing(PathBuf),
    EscapesRoot(PathBuf),
}

impl fmt::Display for StagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StagingError::Io(error) => write!(formatter, "staging I/O failed: {error}"),
            StagingError::InputMissing(path) => {
                write!(formatter, "staging input is missing at {}", path.display())
            }
            StagingError::EscapesRoot(path) => {
                write!(
                    formatter,
                    "path {} escapes the staging root",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for StagingError {}

/// A canonicalized private staging directory. Inputs resolve to
/// `input/source.media` only; outputs must stay inside the root even when
/// symlinks are involved.
#[derive(Debug, Clone)]
pub struct StagingRoot {
    root: PathBuf,
}

impl StagingRoot {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StagingError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(StagingError::Io)?;
        let root = root.canonicalize().map_err(StagingError::Io)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn input(&self) -> PathBuf {
        self.root.join(INPUT_REF)
    }

    pub fn require_input(&self) -> Result<PathBuf, StagingError> {
        let input = self.input();
        if input.is_file() {
            Ok(input)
        } else {
            Err(StagingError::InputMissing(input))
        }
    }

    pub fn output(&self, reference: &RelativeRef) -> Result<PathBuf, StagingError> {
        let candidate = self.root.join(reference.as_str());
        let mut ancestor = candidate.clone();
        while !ancestor.exists() {
            match ancestor.parent() {
                Some(parent) => ancestor = parent.to_path_buf(),
                None => break,
            }
        }
        let canonical = ancestor.canonicalize().map_err(StagingError::Io)?;
        if !canonical.starts_with(&self.root) {
            return Err(StagingError::EscapesRoot(candidate));
        }
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("scene-core-staging-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn input_requires_the_fixed_relative_ref() {
        let root = StagingRoot::new(temp_root("input")).expect("staging root");
        assert!(matches!(
            root.require_input(),
            Err(StagingError::InputMissing(_))
        ));
        std::fs::create_dir_all(root.root().join("input")).expect("input dir");
        std::fs::write(root.input(), b"media").expect("input file");
        assert_eq!(root.require_input().expect("input"), root.input());
        let _ = std::fs::remove_dir_all(root.root());
    }

    #[test]
    fn outputs_stay_inside_the_root() {
        let root = StagingRoot::new(temp_root("output")).expect("staging root");
        let reference = RelativeRef::new("output/preview/opening.jpg").expect("ref");
        assert_eq!(
            root.output(&reference).expect("output"),
            root.root().join("output/preview/opening.jpg")
        );
        let _ = std::fs::remove_dir_all(root.root());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escapes_are_rejected() {
        let outside = temp_root("outside");
        std::fs::create_dir_all(&outside).expect("outside dir");
        let root = StagingRoot::new(temp_root("symlink")).expect("staging root");
        std::os::unix::fs::symlink(&outside, root.root().join("output")).expect("symlink");
        let reference = RelativeRef::new("output/escape.jpg").expect("ref");
        assert!(matches!(
            root.output(&reference),
            Err(StagingError::EscapesRoot(_))
        ));
        let _ = std::fs::remove_dir_all(root.root());
        let _ = std::fs::remove_dir_all(&outside);
    }
}

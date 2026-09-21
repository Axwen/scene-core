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
        self.reject_symlinks(&input)?;
        match std::fs::metadata(&input) {
            Ok(metadata) if metadata.is_file() => Ok(input),
            _ => Err(StagingError::InputMissing(input)),
        }
    }

    pub fn output(&self, reference: &RelativeRef) -> Result<PathBuf, StagingError> {
        let candidate = self.root.join(reference.as_str());
        self.contain(&candidate)?;
        Ok(candidate)
    }

    /// Rejects a path that is not below the root or that traverses a symlink.
    /// Every path the engine is about to create or write must pass through
    /// here, including temporary files derived from a validated output.
    pub fn contain(&self, candidate: &Path) -> Result<(), StagingError> {
        self.reject_symlinks(candidate)
    }

    /// Rejects any symlink on the path below the root. `symlink_metadata` does
    /// not follow the final component, so both a symlinked input and a
    /// symlinked ancestor directory are caught.
    fn reject_symlinks(&self, candidate: &Path) -> Result<(), StagingError> {
        let escape = || StagingError::EscapesRoot(candidate.to_path_buf());
        let relative = candidate.strip_prefix(&self.root).map_err(|_| escape())?;
        let mut current = self.root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                return Err(escape());
            };
            current.push(part);
            if std::fs::symlink_metadata(&current)
                .map(|metadata| is_link_or_reparse(&metadata))
                .unwrap_or(false)
            {
                return Err(escape());
            }
        }
        Ok(())
    }
}

/// Windows junctions and other reparse points are not `is_symlink()`, so the
/// file attribute is checked there as well.
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
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

    #[cfg(unix)]
    #[test]
    fn symlinked_inputs_are_rejected() {
        let outside = temp_root("outside-input");
        std::fs::create_dir_all(&outside).expect("outside dir");
        let media = outside.join("media.mkv");
        std::fs::write(&media, b"media").expect("media file");

        let root = StagingRoot::new(temp_root("symlink-input")).expect("staging root");
        std::fs::create_dir_all(root.root().join("input")).expect("input dir");
        std::os::unix::fs::symlink(&media, root.input()).expect("symlink");
        assert!(matches!(
            root.require_input(),
            Err(StagingError::EscapesRoot(_))
        ));

        std::fs::remove_file(root.input()).expect("remove symlink");
        std::fs::remove_dir(root.root().join("input")).expect("remove input dir");
        std::os::unix::fs::symlink(&outside, root.root().join("input")).expect("dir symlink");
        assert!(matches!(
            root.require_input(),
            Err(StagingError::EscapesRoot(_))
        ));

        let _ = std::fs::remove_dir_all(root.root());
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[cfg(unix)]
    #[test]
    fn dangling_output_symlinks_are_rejected() {
        let root = StagingRoot::new(temp_root("dangling")).expect("staging root");
        let outside = temp_root("dangling-outside");
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(root.root().join("output")).expect("output dir");
        std::os::unix::fs::symlink(
            outside.join("missing.jpg"),
            root.root().join("output/frame.jpg"),
        )
        .expect("symlink");
        let reference = RelativeRef::new("output/frame.jpg").expect("ref");
        assert!(matches!(
            root.output(&reference),
            Err(StagingError::EscapesRoot(_))
        ));
        let _ = std::fs::remove_dir_all(root.root());
    }

    #[test]
    fn unsafe_reference_forms_are_rejected_before_staging() {
        for reference in [
            "output/frame.jpg:metadata",
            "//server/share/frame.jpg",
            "\\\\server\\share\\frame.jpg",
            "../outside/frame.jpg",
        ] {
            assert!(
                RelativeRef::new(reference).is_err(),
                "unsafe reference was accepted: {reference}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn junction_escapes_are_rejected() {
        use std::process::Command;

        let outside = temp_root("junction-outside");
        std::fs::create_dir_all(&outside).expect("outside dir");
        let root = StagingRoot::new(temp_root("junction")).expect("staging root");
        let link = root.root().join("output");
        let link_string = link.to_string_lossy().into_owned();
        let outside_string = outside.to_string_lossy().into_owned();
        let output = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link_string.as_str(),
                outside_string.as_str(),
            ])
            .output()
            .expect("create junction command");
        assert!(
            output.status.success(),
            "mklink /J failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let reference = RelativeRef::new("output/escape.jpg").expect("ref");
        assert!(matches!(
            root.output(&reference),
            Err(StagingError::EscapesRoot(_))
        ));
        let _ = std::fs::remove_dir_all(root.root());
        let _ = std::fs::remove_dir_all(outside);
    }

    #[cfg(windows)]
    #[test]
    fn directory_symlink_escapes_are_rejected() {
        use std::os::windows::fs::symlink_dir;

        let outside = temp_root("symlink-outside");
        std::fs::create_dir_all(&outside).expect("outside dir");
        let root = StagingRoot::new(temp_root("windows-symlink")).expect("staging root");
        let link = root.root().join("output");
        symlink_dir(&outside, &link).expect("Windows symlink creation requires CI symlink support");

        let reference = RelativeRef::new("output/escape.jpg").expect("ref");
        assert!(matches!(
            root.output(&reference),
            Err(StagingError::EscapesRoot(_))
        ));
        let _ = std::fs::remove_dir_all(root.root());
        let _ = std::fs::remove_dir_all(outside);
    }
}

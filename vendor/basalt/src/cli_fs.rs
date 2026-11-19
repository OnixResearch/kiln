//! Capability-bounded local filesystem roots for the Basalt CLI.
//!
//! This CLI-feature module opens operator-declared policy and output roots with
//! `cap_std::fs::Dir`. Every subsequent read or write is relative to the typed
//! root, so a `../` traversal, absolute path, symlink escape, or output escape
//! attempt fails closed before it can touch an unrelated host path. The
//! library core, UCAN validation, Nickel/Steel backend DTOs, and packaged
//! policy helpers never depend on this module or open ambient paths.

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use std::fmt;
use std::path::Path;

/// A capability root opened around an operator-declared policy input directory.
pub struct PolicyRoot {
    /// Typed capability root.
    dir: Dir,
}

/// A capability root opened around an operator-declared output/receipt directory.
pub struct OutputRoot {
    /// Typed capability root.
    dir: Dir,
}

/// Stable error narration for a capability-bound CLI filesystem operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// The declared root could not be opened.
    OpenRoot,
    /// A relative path was rejected before any filesystem effect.
    InvalidPath,
    /// A read failed inside the bounded root.
    Read,
    /// A write failed inside the bounded root.
    Write,
}

impl fmt::Display for FsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenRoot => formatter.write_str("could not open declared capability root"),
            Self::InvalidPath => formatter.write_str("path escapes the declared capability root"),
            Self::Read => formatter.write_str("could not read file inside capability root"),
            Self::Write => formatter.write_str("could not write file inside capability root"),
        }
    }
}

impl std::error::Error for FsError {}

impl PolicyRoot {
    /// Open an ambient path as a typed capability root.
    ///
    /// The ambient path is trusted only to establish the boundary; all later
    /// resolutions are capability-relative.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::OpenRoot`] when the declared root cannot be opened.
    pub fn open<P: AsRef<Path>>(root: P) -> Result<Self, FsError> {
        let dir = Dir::open_ambient_dir(root.as_ref(), ambient_authority())
            .map_err(|_| FsError::OpenRoot)?;
        Ok(Self { dir })
    }

    /// Read a policy file that must resolve relative to this root.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::InvalidPath`] when `relative` traverses upward or is
    /// absolute, or [`FsError::Read`] when the read fails inside the root.
    pub fn read_policy(&self, relative: &str) -> Result<String, FsError> {
        let path = safe_relative(relative)?;
        self.dir.read_to_string(path).map_err(|_| FsError::Read)
    }
}

impl OutputRoot {
    /// Open an ambient path as a typed output/receipt root.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::OpenRoot`] when the declared root cannot be opened.
    pub fn open<P: AsRef<Path>>(root: P) -> Result<Self, FsError> {
        let dir = Dir::open_ambient_dir(root.as_ref(), ambient_authority())
            .map_err(|_| FsError::OpenRoot)?;
        Ok(Self { dir })
    }

    /// Write a receipt relative to this root.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::InvalidPath`] when `relative` traverses upward or is
    /// absolute, or [`FsError::Write`] when the write fails inside the root.
    pub fn write(&self, relative: &str, contents: &str) -> Result<(), FsError> {
        let path = safe_relative(relative)?;
        self.dir
            .write(path, contents.as_bytes())
            .map_err(|_| FsError::Write)
    }
}

/// Reject upward traversal and absolute paths before any capability-relative effect.
fn safe_relative(relative: &str) -> Result<&Path, FsError> {
    if relative.is_empty() {
        return Err(FsError::InvalidPath);
    }
    if Path::new(relative).is_absolute() {
        return Err(FsError::InvalidPath);
    }
    let mut depth = 0usize;
    for component in Path::new(relative).components() {
        if matches!(component, std::path::Component::ParentDir) {
            if depth == 0 {
                return Err(FsError::InvalidPath);
            }
            depth -= 1;
        } else if matches!(component, std::path::Component::Normal(_)) {
            depth += 1;
        } else if matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        ) {
            return Err(FsError::InvalidPath);
        }
    }
    Ok(Path::new(relative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_dir(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("basalt-cap-{name}-{}_{nanos}", std::process::id()));
        fs::create_dir_all(&dir).expect("fixture dir should create");
        dir
    }

    #[test]
    fn positive_read_inside_declared_root() {
        let dir = unique_dir("read-ok");
        fs::write(dir.join("policy.json"), "{}").expect("policy fixture should write");

        let root = PolicyRoot::open(&dir).expect("root should open");
        assert_eq!(
            root.read_policy("policy.json").expect("read inside root"),
            "{}"
        );

        fs::remove_dir_all(&dir).expect("fixture dir should clean up");
    }

    #[test]
    fn negative_traversal_fails_closed() {
        let dir = unique_dir("traversal");
        let root = PolicyRoot::open(&dir).expect("root should open");

        assert_eq!(
            root.read_policy("../outside.json"),
            Err(FsError::InvalidPath)
        );
        assert_eq!(
            root.read_policy("a/../../outside.json"),
            Err(FsError::InvalidPath)
        );

        fs::remove_dir_all(&dir).expect("fixture dir should clean up");
    }

    #[test]
    fn negative_absolute_path_fails_closed() {
        let dir = unique_dir("absolute");
        let root = PolicyRoot::open(&dir).expect("root should open");

        assert_eq!(root.read_policy("/etc/passwd"), Err(FsError::InvalidPath));

        fs::remove_dir_all(&dir).expect("fixture dir should clean up");
    }

    #[test]
    fn negative_missing_root_fails_closed() {
        let missing = std::env::temp_dir().join("basalt-cap-missing-root-does-not-exist");
        let _ = fs::remove_dir_all(&missing);
        assert!(matches!(PolicyRoot::open(&missing), Err(FsError::OpenRoot)));
    }

    #[test]
    fn negative_output_escape_fails_closed() {
        let dir = unique_dir("output");
        let root = OutputRoot::open(&dir).expect("root should open");

        assert_eq!(root.write("../escape.json", "x"), Err(FsError::InvalidPath));
        assert_eq!(root.write("/abs.json", "x"), Err(FsError::InvalidPath));

        fs::remove_dir_all(&dir).expect("fixture dir should clean up");
    }

    #[test]
    fn positive_output_write_stays_in_root() {
        let dir = unique_dir("write-ok");
        let root = OutputRoot::open(&dir).expect("root should open");

        root.write("receipt.json", r#"{"ok":true}"#)
            .expect("write inside root");
        let contents = fs::read_to_string(dir.join("receipt.json")).expect("written file readable");
        assert_eq!(contents, r#"{"ok":true}"#);

        fs::remove_dir_all(&dir).expect("fixture dir should clean up");
    }

    #[test]
    fn negative_empty_path_fails_closed() {
        let dir = unique_dir("empty");
        let root = PolicyRoot::open(&dir).expect("root should open");
        assert_eq!(root.read_policy(""), Err(FsError::InvalidPath));
        fs::remove_dir_all(&dir).expect("fixture dir should clean up");
    }
}

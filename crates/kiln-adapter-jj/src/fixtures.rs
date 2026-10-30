//! Real-`jj` fixture repositories for integration tests (feature `jj-fixtures`).
//! Fixtures mutate their own scratch repositories through jj; Kiln code under
//! test only observes them. Panics are the failure mode of test support.
#![allow(clippy::missing_panics_doc, clippy::panic, clippy::expect_used)]
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::Workspace;

/// A colocated jj repository with a `base` commit under bookmark `main` and an
/// empty working-copy change on top.
#[derive(Debug)]
pub struct Fixture {
    pub root: PathBuf,
    pub executable: PathBuf,
}

/// The `jj` executable: `KILN_JJ_EXE`, else `jj` resolved on `PATH`.
#[must_use]
pub fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("KILN_JJ_EXE") {
        return PathBuf::from(path);
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join("jj"))
                .find(|candidate| candidate.is_file())
        })
        .expect("jj-fixtures requires KILN_JJ_EXE or jj on PATH")
}

impl Fixture {
    /// Initialise a colocated repository at `root` (which must not exist).
    #[must_use]
    pub fn new(root: &Path) -> Self {
        let parent = root.parent().expect("fixture parent");
        std::fs::create_dir_all(parent).expect("fixture parent");
        let fixture = Self {
            root: root.to_path_buf(),
            executable: executable(),
        };
        fixture.run_in(
            parent,
            &["git", "init", "--colocate", &root.to_string_lossy()],
        );
        fixture.write("README", "base\n");
        fixture.jj(&["describe", "-m", "base"]);
        fixture.jj(&["bookmark", "create", "main", "-r", "@"]);
        fixture.jj(&["new"]);
        fixture
    }

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(dir)
            .env_clear()
            .env("JJ_CONFIG", "/dev/null")
            .env("JJ_USER", "Fixture Author")
            .env("JJ_EMAIL", "author@example.org")
            .args(["--no-pager", "--color", "never"])
            .args(args);
        command
    }

    fn run_in(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.command(dir, args).output().expect("jj spawn");
        assert!(
            output.status.success(),
            "jj {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("jj output utf-8")
    }

    /// Run a mutating fixture command in the workspace.
    pub fn jj(&self, args: &[&str]) -> String {
        self.run_in(&self.root, args)
    }

    pub fn write(&self, path: &str, contents: &str) {
        let target = self.root.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("fixture directory");
        }
        std::fs::write(target, contents).expect("fixture write");
    }

    /// Full change ID of exactly one revision.
    #[must_use]
    pub fn change_id(&self, revision: &str) -> String {
        self.jj(&["log", "--no-graph", "-r", revision, "-T", "change_id"])
    }

    /// Full commit ID of exactly one revision.
    #[must_use]
    pub fn commit_id(&self, revision: &str) -> String {
        self.jj(&["log", "--no-graph", "-r", revision, "-T", "commit_id"])
    }

    /// Describe the working-copy change with one file and start a fresh empty
    /// working copy above it; returns the described change ID.
    #[must_use]
    pub fn commit(&self, path: &str, contents: &str, message: &str) -> String {
        self.write(path, contents);
        self.jj(&["describe", "-m", message]);
        let change = self.change_id("@");
        self.jj(&["new"]);
        change
    }

    /// Rewrite an existing change's file content without moving the working copy.
    pub fn rewrite(&self, change: &str, path: &str, contents: &str) {
        self.jj(&["edit", change]);
        self.write(path, contents);
        // Snapshot the edit (descendants are rebased), then leave the change
        // through a fresh empty working copy so later commands cannot rewrite it.
        self.jj(&["new", change]);
    }

    #[must_use]
    pub fn workspace(&self) -> Workspace {
        Workspace {
            executable: self.executable.clone(),
            workspace: self.root.clone(),
        }
    }

    /// Byte fingerprint of every piece of jj and Git state a mutation would
    /// touch: operation heads, the operation store, Git refs and the working
    /// copy files. jj's derived index cache and the Kiln review provider's own
    /// `refs/kiln/` operation refs (which jj never imports) are excluded.
    #[must_use]
    pub fn fingerprint(&self) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        for relative in [
            ".jj/repo/op_heads",
            ".jj/repo/op_store",
            ".jj/working_copy",
            ".git/refs",
            ".git/packed-refs",
            ".git/HEAD",
        ] {
            collect(&self.root, &self.root.join(relative), &mut files);
        }
        for entry in std::fs::read_dir(&self.root).expect("fixture root") {
            let path = entry.expect("fixture entry").path();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if !matches!(name.as_deref(), Some(".jj" | ".git")) {
                collect(&self.root, &path, &mut files);
            }
        }
        files.retain(|(name, _)| !name.starts_with(".git/refs/kiln"));
        files.sort();
        files
    }
}

fn collect(root: &Path, path: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    let name = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).expect("fixture directory") {
            collect(root, &entry.expect("fixture entry").path(), files);
        }
    } else {
        files.push((name, std::fs::read(path).unwrap_or_default()));
    }
}

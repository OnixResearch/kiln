//! Verify that the vendored verified-logic tree matches the recorded Trellis revision.
//!
//! This is intentionally dependency-free so it can be compiled with plain `rustc`:
//!
//! ```sh
//! rustc scripts/check-verified-logic-josh-sync.rs -o /tmp/check-verified-logic-josh-sync
//! /tmp/check-verified-logic-josh-sync
//! ```

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::process::{self};
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const DEFAULT_SOURCE_REPO: &str = "../trellis";
const DEFAULT_VENDOR_DIR: &str = "vendor/verified-logic";
const DEFAULT_REVISION_FILE: &str = "vendor/verified-logic-revision.txt";
const DEFAULT_FILTER_FILE: &str = "josh/verified-logic.josh";
const REQUIRED_PATHS: &[&str] = &["Cargo.toml", "LICENSE", "src"];
const OPTIONAL_PATHS: &[&str] = &["README.md", "build.rs", ".cargo"];
const REQUIRED_FILTER_FRAGMENTS: &[&str] = &[
    "::Cargo.toml",
    "::LICENSE",
    "::README.md",
    "::build.rs",
    "::.cargo/",
    "::src/",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    source_repo: PathBuf,
    vendor_dir: PathBuf,
    revision_file: PathBuf,
    filter_file: PathBuf,
    revision_override: Option<String>,
    config_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RevisionMetadata {
    revision: String,
    source_repo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EntryKind {
    Directory,
    File,
    Symlink,
}

struct TempDir {
    path: PathBuf,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.path);
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = parse_args(env::args().skip(1))?;
    validate_filter_file(&config.filter_file)?;
    let metadata = read_revision_metadata(&config.revision_file)?;
    if config.config_only {
        println!("verified-logic Josh filter config and revision metadata are valid");
        return Ok(());
    }

    let revision = config.revision_override.clone().unwrap_or_else(|| metadata.revision.clone());

    ensure_git_checkout(&config.source_repo)?;
    ensure_revision_exists(&config.source_repo, &revision)?;

    let selected_paths = selected_paths(&config.source_repo, &revision)?;
    let temp_dir = create_temp_dir()?;
    let expected_dir = temp_dir.path.join("expected");
    fs::create_dir_all(&expected_dir)
        .map_err(|error| format!("failed to create expected tree directory {}: {error}", expected_dir.display()))?;

    export_revision_tree(&config.source_repo, &revision, &selected_paths, &expected_dir)?;
    compare_trees(&expected_dir, &config.vendor_dir)?;

    if let Some(source_repo) = metadata.source_repo.as_deref() {
        println!("recorded source repo: {source_repo}");
    }
    println!(
        "vendor/verified-logic matches Trellis revision {revision} using Josh filter {}",
        config.filter_file.display()
    );
    Ok(())
}

fn parse_args<I>(args: I) -> Result<Config, String>
where I: IntoIterator<Item = String> {
    let mut config = Config {
        source_repo: PathBuf::from(DEFAULT_SOURCE_REPO),
        vendor_dir: PathBuf::from(DEFAULT_VENDOR_DIR),
        revision_file: PathBuf::from(DEFAULT_REVISION_FILE),
        filter_file: PathBuf::from(DEFAULT_FILTER_FILE),
        revision_override: None,
        config_only: false,
    };

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--source-repo" => config.source_repo = PathBuf::from(required_value(&mut iter, "--source-repo")?),
            "--vendor-dir" => config.vendor_dir = PathBuf::from(required_value(&mut iter, "--vendor-dir")?),
            "--revision-file" => config.revision_file = PathBuf::from(required_value(&mut iter, "--revision-file")?),
            "--filter-file" => config.filter_file = PathBuf::from(required_value(&mut iter, "--filter-file")?),
            "--revision" => config.revision_override = Some(required_value(&mut iter, "--revision")?),
            "--config-only" => config.config_only = true,
            "--help" | "-h" => return Err(help_text()),
            unknown => return Err(format!("unknown argument {unknown}\n{}", help_text())),
        }
    }

    Ok(config)
}

fn required_value<I>(iter: &mut I, flag: &str) -> Result<String, String>
where I: Iterator<Item = String> {
    iter.next().filter(|value| !value.is_empty()).ok_or_else(|| format!("{flag} requires a value"))
}

fn help_text() -> String {
    format!(
        "usage: check-verified-logic-josh-sync [--source-repo PATH] [--revision REF] [--vendor-dir PATH] [--revision-file PATH] [--filter-file PATH] [--config-only]\n\nDefaults: source={DEFAULT_SOURCE_REPO}, vendor={DEFAULT_VENDOR_DIR}, revision-file={DEFAULT_REVISION_FILE}, filter={DEFAULT_FILTER_FILE}"
    )
}

fn read_revision_metadata(path: &Path) -> Result<RevisionMetadata, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read revision file {}: {error}", path.display()))?;
    parse_revision_metadata(&text)
}

fn parse_revision_metadata(text: &str) -> Result<RevisionMetadata, String> {
    let mut revision = None;
    let mut source_repo = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("revision=") {
            if value.trim().is_empty() {
                return Err("revision metadata has an empty revision".to_string());
            }
            revision = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("source_repo=") {
            if value.trim().is_empty() {
                return Err("revision metadata has an empty source_repo".to_string());
            }
            source_repo = Some(value.trim().to_string());
        } else {
            return Err(format!("unsupported revision metadata line: {line}"));
        }
    }

    Ok(RevisionMetadata {
        revision: revision.ok_or_else(|| "revision metadata is missing revision=<sha>".to_string())?,
        source_repo,
    })
}

fn missing_filter_fragments(text: &str) -> Vec<&'static str> {
    REQUIRED_FILTER_FRAGMENTS.iter().copied().filter(|fragment| !text.contains(fragment)).collect()
}

fn validate_filter_text(text: &str) -> Result<(), String> {
    let missing = missing_filter_fragments(text);
    if !missing.is_empty() {
        return Err(format!("missing required fragments: {}", missing.join(", ")));
    }
    Ok(())
}

fn validate_filter_file(path: &Path) -> Result<(), String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("failed to read Josh filter {}: {error}", path.display()))?;
    validate_filter_text(&text).map_err(|error| format!("Josh filter {} is {error}", path.display()))
}

fn ensure_git_checkout(repo: &Path) -> Result<(), String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to invoke git for {}: {error}", repo.display()))?;
    if !status.success() {
        return Err(format!("source repo {} is not a git checkout", repo.display()));
    }
    Ok(())
}

fn ensure_revision_exists(repo: &Path, revision: &str) -> Result<(), String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{revision}^{{commit}}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to invoke git for {}: {error}", repo.display()))?;
    if !status.success() {
        return Err(format!("revision {revision} is not present in source repo {}", repo.display()));
    }
    Ok(())
}

fn selected_paths(repo: &Path, revision: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for path in REQUIRED_PATHS {
        if !tree_path_exists(repo, revision, path)? {
            return Err(format!("required path {path} is missing from source repo {} at {revision}", repo.display()));
        }
        paths.push((*path).to_string());
    }
    for path in OPTIONAL_PATHS {
        if tree_path_exists(repo, revision, path)? {
            paths.push((*path).to_string());
        }
    }
    Ok(paths)
}

fn tree_path_exists(repo: &Path, revision: &str, path: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{revision}:{path}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to inspect {path} in {}: {error}", repo.display()))?;
    Ok(status.success())
}

fn create_temp_dir() -> Result<TempDir, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before UNIX_EPOCH: {error}"))?
        .as_nanos();
    let path = env::temp_dir().join(format!("ucan-verified-logic-josh-sync-{}-{timestamp}", process::id()));
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create temporary directory {}: {error}", path.display()))?;
    Ok(TempDir { path })
}

fn export_revision_tree(
    repo: &Path,
    revision: &str,
    selected_paths: &[String],
    destination: &Path,
) -> Result<(), String> {
    let mut git = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("archive")
        .arg("--format=tar")
        .arg(revision)
        .args(selected_paths)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start git archive for {}: {error}", repo.display()))?;

    let git_stdout = git.stdout.take().ok_or_else(|| "failed to capture git archive stdout".to_string())?;

    let mut tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(destination)
        .stdin(Stdio::from(git_stdout))
        .spawn()
        .map_err(|error| format!("failed to start tar extraction into {}: {error}", destination.display()))?;

    let tar_status = tar.wait().map_err(|error| format!("failed to wait for tar extraction: {error}"))?;
    let git_status = git.wait().map_err(|error| format!("failed to wait for git archive: {error}"))?;

    if !git_status.success() {
        return Err(format!("git archive failed for revision {revision}"));
    }
    if !tar_status.success() {
        return Err(format!("tar extraction failed for revision {revision}"));
    }
    Ok(())
}

fn compare_trees(expected: &Path, actual: &Path) -> Result<(), String> {
    let expected_entries = collect_entries(expected)?;
    let actual_entries = collect_entries(actual)?;
    let mut differences = Vec::new();

    for (relative, expected_kind) in &expected_entries {
        match actual_entries.get(relative) {
            Some(actual_kind) if actual_kind == expected_kind => {
                compare_entry_contents(expected, actual, relative, expected_kind, &mut differences)?;
            }
            Some(actual_kind) => differences.push(format!(
                "kind mismatch for {}: expected {:?}, got {:?}",
                relative.display(),
                expected_kind,
                actual_kind
            )),
            None => differences.push(format!("missing vendored path {}", relative.display())),
        }
    }

    for relative in actual_entries.keys() {
        if !expected_entries.contains_key(relative) {
            differences.push(format!("extra vendored path {}", relative.display()));
        }
    }

    if differences.is_empty() {
        Ok(())
    } else {
        Err(format!("vendored verified-logic tree differs from filtered source:\n{}", differences.join("\n")))
    }
}

fn collect_entries(root: &Path) -> Result<BTreeMap<PathBuf, EntryKind>, String> {
    let mut entries = BTreeMap::new();
    collect_entries_from(root, root, &mut entries)?;
    Ok(entries)
}

fn collect_entries_from(root: &Path, current: &Path, entries: &mut BTreeMap<PathBuf, EntryKind>) -> Result<(), String> {
    let mut children = fs::read_dir(current)
        .map_err(|error| format!("failed to read directory {}: {error}", current.display()))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error| format!("failed to collect directory entries from {}: {error}", current.display()))?;
    children.sort_by_key(|entry| entry.path());

    for child in children {
        let path = child.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?
            .to_path_buf();
        let file_type = metadata.file_type();
        if file_type.is_dir() {
            entries.insert(relative, EntryKind::Directory);
            collect_entries_from(root, &path, entries)?;
        } else if file_type.is_file() {
            entries.insert(relative, EntryKind::File);
        } else if file_type.is_symlink() {
            entries.insert(relative, EntryKind::Symlink);
        } else {
            return Err(format!("unsupported file type at {}", path.display()));
        }
    }
    Ok(())
}

fn compare_entry_contents(
    expected_root: &Path,
    actual_root: &Path,
    relative: &Path,
    kind: &EntryKind,
    differences: &mut Vec<String>,
) -> Result<(), String> {
    match kind {
        EntryKind::Directory => Ok(()),
        EntryKind::File => {
            let expected_path = expected_root.join(relative);
            let actual_path = actual_root.join(relative);
            let expected = fs::read(&expected_path)
                .map_err(|error| format!("failed to read {}: {error}", expected_path.display()))?;
            let actual =
                fs::read(&actual_path).map_err(|error| format!("failed to read {}: {error}", actual_path.display()))?;
            if expected != actual {
                differences.push(format!("content mismatch for {}", relative.display()));
            }
            Ok(())
        }
        EntryKind::Symlink => {
            let expected_path = expected_root.join(relative);
            let actual_path = actual_root.join(relative);
            let expected = fs::read_link(&expected_path)
                .map_err(|error| format!("failed to read symlink {}: {error}", expected_path.display()))?;
            let actual = fs::read_link(&actual_path)
                .map_err(|error| format!("failed to read symlink {}: {error}", actual_path.display()))?;
            if expected != actual {
                differences.push(format!(
                    "symlink mismatch for {}: expected {}, got {}",
                    relative.display(),
                    display_os_path(&expected),
                    display_os_path(&actual)
                ));
            }
            Ok(())
        }
    }
}

fn display_os_path(path: &Path) -> String {
    path.as_os_str().to_str().map(str::to_owned).unwrap_or_else(|| display_lossy(path.as_os_str()))
}

fn display_lossy(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_REVISION: &str = "5bd340b63927444ce583164feba57a1fa877c6c5";
    const SAMPLE_SOURCE_REPO: &str = "git@github.com:OnixResearch/trellis.git";

    #[test]
    fn accepts_filter_with_license_and_source_fragments() {
        let filter = REQUIRED_FILTER_FRAGMENTS.join("\n");

        assert!(validate_filter_text(&filter).is_ok());
    }

    #[test]
    fn rejects_filter_without_license_fragment() {
        let filter = REQUIRED_FILTER_FRAGMENTS
            .iter()
            .copied()
            .filter(|fragment| *fragment != "::LICENSE")
            .collect::<Vec<_>>()
            .join("\n");

        let error = validate_filter_text(&filter).expect_err("missing license fragment must fail");
        assert!(error.contains("::LICENSE"), "unexpected error: {error}");
    }

    #[test]
    fn parses_revision_metadata_with_source_repo() {
        let text = format!("revision={SAMPLE_REVISION}\nsource_repo={SAMPLE_SOURCE_REPO}\n");

        let metadata = parse_revision_metadata(&text).expect("metadata should parse");

        assert_eq!(metadata.revision, SAMPLE_REVISION);
        assert_eq!(metadata.source_repo.as_deref(), Some(SAMPLE_SOURCE_REPO));
    }

    #[test]
    fn rejects_revision_metadata_without_revision() {
        let text = format!("source_repo={SAMPLE_SOURCE_REPO}\n");

        let error = parse_revision_metadata(&text).expect_err("missing revision should fail");

        assert!(error.contains("missing revision"), "unexpected error: {error}");
    }

    #[test]
    fn rejects_unsupported_revision_metadata_line() {
        let text = format!("revision={SAMPLE_REVISION}\nunsupported=value\n");

        let error = parse_revision_metadata(&text).expect_err("unsupported line should fail");

        assert!(error.contains("unsupported revision metadata line"), "unexpected error: {error}");
    }

    #[test]
    fn parses_custom_arguments() {
        let args = [
            "--source-repo",
            "../trellis",
            "--vendor-dir",
            "vendor/verified-logic",
            "--revision-file",
            "vendor/verified-logic-revision.txt",
            "--filter-file",
            "josh/verified-logic.josh",
            "--revision",
            SAMPLE_REVISION,
            "--config-only",
        ];

        let config = parse_args(args.iter().map(|arg| (*arg).to_string())).expect("args should parse");

        assert_eq!(config.source_repo, PathBuf::from("../trellis"));
        assert_eq!(config.vendor_dir, PathBuf::from("vendor/verified-logic"));
        assert_eq!(config.revision_file, PathBuf::from("vendor/verified-logic-revision.txt"));
        assert_eq!(config.filter_file, PathBuf::from("josh/verified-logic.josh"));
        assert_eq!(config.revision_override.as_deref(), Some(SAMPLE_REVISION));
        assert!(config.config_only);
    }

    #[test]
    fn rejects_argument_without_required_value() {
        let args = ["--source-repo"];

        let error = parse_args(args.iter().map(|arg| (*arg).to_string())).expect_err("missing value should fail");

        assert!(error.contains("--source-repo requires a value"), "unexpected error: {error}");
    }
}

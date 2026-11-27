//! The `kiln` operator CLI's help, version and refusal surface, observed at the
//! process boundary: which stream receives the text and which status the process
//! returns.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const COMMANDS: [&str; 5] = ["demo", "init", "evidence", "host-qualify", "dashboard"];
const USAGE_PREFIX: &str = "usage: kiln ";
const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDirGuard(PathBuf);

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "kiln-cli-test-{prefix}-{}-{seq}",
            std::process::id()
        ));
        if let Err(error) = fs::create_dir_all(&dir) {
            unreachable!("failed to create test tempdir {}: {error}", dir.display());
        }
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn init_git_repo(path: &Path) {
    let repo = match git2::Repository::init(path) {
        Ok(repo) => repo,
        Err(error) => unreachable!("git2 init failed: {error}"),
    };
    if let Err(error) = fs::write(path.join("README.md"), b"# sample\n") {
        unreachable!("write README failed: {error}");
    }
    let mut index = match repo.index() {
        Ok(index) => index,
        Err(error) => unreachable!("repo index failed: {error}"),
    };
    if let Err(error) = index.add_path(Path::new("README.md")) {
        unreachable!("index add failed: {error}");
    }
    if let Err(error) = index.write() {
        unreachable!("index write failed: {error}");
    }
    let tree_id = match index.write_tree() {
        Ok(id) => id,
        Err(error) => unreachable!("write tree failed: {error}"),
    };
    let tree = match repo.find_tree(tree_id) {
        Ok(tree) => tree,
        Err(error) => unreachable!("find tree failed: {error}"),
    };
    let sig = match git2::Signature::now("Kiln Test", "test@example.invalid") {
        Ok(sig) => sig,
        Err(error) => unreachable!("signature failed: {error}"),
    };
    if let Err(error) = repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[]) {
        unreachable!("commit failed: {error}");
    }
}

fn kiln(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(arguments)
        .output()
        .unwrap_or_else(|error| unreachable!("kiln did not start: {error}"))
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or_else(|error| unreachable!("kiln wrote non-UTF-8: {error}"))
}

/// Each case must write its report to standard error only and exit with its status.
fn assert_refused(cases: &[(&[&str], i32, &str)]) {
    for (arguments, status, report) in cases {
        let output = kiln(arguments);
        assert_eq!(output.status.code(), Some(*status), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        let stderr = text(&output.stderr);
        assert!(stderr.starts_with(report), "{arguments:?}: {stderr}");
    }
}

// r[verify kiln.cli.help]
// r[verify kiln.cli.help.scenario.help]
#[test]
fn every_help_spelling_prints_the_same_usage_on_standard_output() {
    let long = kiln(&["--help"]);
    assert_eq!(long.status.code(), Some(EXIT_SUCCESS));
    assert!(long.stderr.is_empty());
    let usage = text(&long.stdout);
    assert!(usage.starts_with(USAGE_PREFIX), "{usage}");
    for command in COMMANDS {
        assert!(
            usage.contains(&format!("\n  {command}")),
            "usage does not list {command}: {usage}"
        );
    }
    for spelling in ["-h", "help"] {
        let output = kiln(&[spelling]);
        assert_eq!(output.status.code(), Some(EXIT_SUCCESS), "{spelling}");
        assert_eq!(output.stdout, long.stdout, "{spelling}");
        assert!(output.stderr.is_empty(), "{spelling}");
    }
}

// r[verify kiln.cli.help.scenario.refused]
#[test]
fn unusable_input_is_refused_on_standard_error_only() {
    assert_refused(&[
        (&[], EXIT_USAGE, USAGE_PREFIX),
        (&["--helpx"], EXIT_FAILURE, "unsupported command: --helpx"),
        (&["-help"], EXIT_FAILURE, "unsupported command: -help"),
        (&["--help", "demo"], EXIT_USAGE, USAGE_PREFIX),
        (&["help", "dashboard"], EXIT_USAGE, USAGE_PREFIX),
    ]);
}

// r[verify kiln.cli.version]
// r[verify kiln.cli.version.scenario.version]
#[test]
fn both_version_spellings_print_the_name_and_package_version() {
    let expected = format!("kiln {}\n", env!("CARGO_PKG_VERSION"));
    for spelling in ["--version", "-V"] {
        let output = kiln(&[spelling]);
        assert_eq!(output.status.code(), Some(EXIT_SUCCESS), "{spelling}");
        assert_eq!(text(&output.stdout), expected, "{spelling}");
        assert!(output.stderr.is_empty(), "{spelling}");
    }
    let usage = kiln(&["--help"]);
    assert!(
        text(&usage.stdout).contains("\n  --version, -V"),
        "usage does not name the version request"
    );
}

// r[verify kiln.cli.version.scenario.refused]
#[test]
fn a_version_request_takes_no_argument_and_has_no_other_spelling() {
    assert_refused(&[
        (&["--version", "demo"], EXIT_USAGE, USAGE_PREFIX),
        (&["-V", "--help"], EXIT_USAGE, USAGE_PREFIX),
        (&["-v"], EXIT_FAILURE, "unsupported command: -v"),
        (&["version"], EXIT_FAILURE, "unsupported command: version"),
        (&["--ver"], EXIT_FAILURE, "unsupported command: --ver"),
    ]);
}

#[test]
fn help_lists_related_binaries_for_newcomers() {
    let usage = kiln(&["--help"]);
    assert_eq!(usage.status.code(), Some(EXIT_SUCCESS));
    let stdout = text(&usage.stdout);
    assert!(stdout.contains("related binaries:"), "{stdout}");
    for binary in ["kiln-web-service", "kiln-fleet", "kiln-implementer"] {
        assert!(stdout.contains(binary), "missing {binary}: {stdout}");
    }
}

#[test]
fn init_generates_valid_local_workspace_accepted_by_web_service_check() {
    let scratch = TempDirGuard::new("init-valid");
    let repo = scratch.path().join("sample-repo");
    init_git_repo(&repo);
    let out = scratch.path().join("workspace");

    let repo_arg = repo.to_string_lossy().into_owned();
    let out_arg = out.to_string_lossy().into_owned();
    let output = kiln(&[
        "init",
        "--repo",
        &repo_arg,
        "--out",
        &out_arg,
        "--project",
        "demo-proj",
    ]);
    assert_eq!(
        output.status.code(),
        Some(EXIT_SUCCESS),
        "stderr: {}",
        text(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = text(&output.stdout);
    assert!(
        stdout.contains("kiln-web-service check --profile "),
        "{stdout}"
    );
    assert!(
        stdout.contains("kiln-web-service serve --profile "),
        "{stdout}"
    );

    for required in [
        "service.json",
        "access.json",
        "sources.json",
        "START_HERE.txt",
        "cert.pem",
        "key.pem",
        "operator.token",
        "worker.token",
        "state",
    ] {
        assert!(out.join(required).exists(), "missing generated {required}");
    }

    let options = kiln_web_service::server::Options {
        access: Some(out.join("access.json")),
        sources: Some(out.join("sources.json")),
        ..kiln_web_service::server::Options::default()
    };
    if let Err(error) =
        kiln_web_service::server::check_with_options(&out.join("service.json"), &options)
    {
        unreachable!("kiln-web-service check rejected generated workspace: {error}");
    }
}

#[test]
fn init_refuses_existing_output_missing_repo_file_repo_and_invalid_inputs() {
    let scratch = TempDirGuard::new("init-refused");
    let repo = scratch.path().join("repo");
    init_git_repo(&repo);
    let repo_arg = repo.to_string_lossy().into_owned();

    // 1. Existing --out directory must be refused without touching sentinel file.
    let existing_out = scratch.path().join("existing-out");
    if let Err(error) = fs::create_dir_all(&existing_out) {
        unreachable!("mkdir failed: {error}");
    }
    let sentinel = existing_out.join("keep.txt");
    if let Err(error) = fs::write(&sentinel, b"preserve-me") {
        unreachable!("write sentinel failed: {error}");
    }
    let existing_arg = existing_out.to_string_lossy().into_owned();
    let out_exists = kiln(&["init", "--repo", &repo_arg, "--out", &existing_arg]);
    assert_eq!(out_exists.status.code(), Some(EXIT_FAILURE));
    assert!(out_exists.stdout.is_empty());
    assert!(
        text(&out_exists.stderr).contains("already exists"),
        "{}",
        text(&out_exists.stderr)
    );
    assert_eq!(
        fs::read(&sentinel).unwrap_or_else(|e| unreachable!("sentinel lost: {e}")),
        b"preserve-me"
    );
    assert!(!existing_out.join("service.json").exists());

    // 2. Missing --repo must be refused and must not create --out.
    let missing_repo = scratch.path().join("does-not-exist");
    let missing_out = scratch.path().join("out-missing-repo");
    let missing_repo_out = kiln(&[
        "init",
        "--repo",
        &missing_repo.to_string_lossy(),
        "--out",
        &missing_out.to_string_lossy(),
    ]);
    assert_eq!(missing_repo_out.status.code(), Some(EXIT_FAILURE));
    assert!(missing_repo_out.stdout.is_empty());
    assert!(!missing_out.exists());

    // 3. Regular file passed as --repo must be refused.
    let file_repo = scratch.path().join("not-a-dir");
    if let Err(error) = fs::write(&file_repo, b"file") {
        unreachable!("write file failed: {error}");
    }
    let file_out = scratch.path().join("out-file-repo");
    let file_repo_res = kiln(&[
        "init",
        "--repo",
        &file_repo.to_string_lossy(),
        "--out",
        &file_out.to_string_lossy(),
    ]);
    assert_eq!(file_repo_res.status.code(), Some(EXIT_FAILURE));
    assert!(!file_out.exists());

    // 4. Plain non-Git directory passed as --repo must be refused.
    let plain_dir = scratch.path().join("plain-dir");
    if let Err(error) = fs::create_dir_all(&plain_dir) {
        unreachable!("mkdir failed: {error}");
    }
    let plain_out = scratch.path().join("out-plain-dir");
    let plain_res = kiln(&[
        "init",
        "--repo",
        &plain_dir.to_string_lossy(),
        "--out",
        &plain_out.to_string_lossy(),
    ]);
    assert_eq!(plain_res.status.code(), Some(EXIT_FAILURE));
    assert!(!plain_out.exists());

    // 5. Invalid --project name must be refused without creating --out.
    let bad_proj_out = scratch.path().join("out-bad-project");
    let bad_proj = kiln(&[
        "init",
        "--repo",
        &repo_arg,
        "--out",
        &bad_proj_out.to_string_lossy(),
        "--project",
        "bad/name",
    ]);
    assert_eq!(bad_proj.status.code(), Some(EXIT_FAILURE));
    assert!(!bad_proj_out.exists());

    // 6. Control characters in path must be refused without creating --out.
    let ctrl_out = scratch.path().join("out-ctrl");
    let ctrl_res = kiln(&[
        "init",
        "--repo",
        "/tmp/bad\nrepo",
        "--out",
        &ctrl_out.to_string_lossy(),
    ]);
    assert_eq!(ctrl_res.status.code(), Some(EXIT_FAILURE));
    assert!(!ctrl_out.exists());

    // 7. Missing required flag exits with EXIT_USAGE (2) and prints usage to stderr.
    let missing_flag = kiln(&["init", "--repo", &repo_arg]);
    assert_eq!(missing_flag.status.code(), Some(EXIT_USAGE));
    assert!(missing_flag.stdout.is_empty());
    assert!(text(&missing_flag.stderr).starts_with("usage: kiln init "));

    // 8. `kiln init --help` exits 0 and prints usage to stdout.
    let init_help = kiln(&["init", "--help"]);
    assert_eq!(init_help.status.code(), Some(EXIT_SUCCESS));
    assert!(text(&init_help.stdout).starts_with("usage: kiln init "));
    assert!(init_help.stderr.is_empty());
}

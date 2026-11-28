//! The `kiln` operator CLI's help, version and refusal surface, observed at the
//! process boundary: which stream receives the text and which status the process
//! returns.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const COMMANDS: [&str; 11] = [
    "init",
    "doctor",
    "check",
    "serve",
    "fleet",
    "ci",
    "dashboard",
    "demo",
    "evidence",
    "host-qualify",
    "native-git",
];
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
    for section in [
        "Factory & Web Console:",
        "Fleet & CI:",
        "Verification & Core:",
    ] {
        assert!(
            stdout.contains(section),
            "missing section {section}: {stdout}"
        );
    }
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
    assert!(stdout.contains("kiln doctor --config "), "{stdout}");
    assert!(stdout.contains("kiln check --config "), "{stdout}");
    assert!(stdout.contains("kiln serve --config "), "{stdout}");

    for required in [
        "kiln.json",
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

    let kiln_raw = fs::read_to_string(out.join("kiln.json"))
        .unwrap_or_else(|error| unreachable!("failed to read kiln.json: {error}"));
    let kiln_json: serde_json::Value = serde_json::from_str(&kiln_raw)
        .unwrap_or_else(|error| unreachable!("invalid kiln.json: {error}"));
    assert_eq!(
        kiln_json.get("schema").and_then(serde_json::Value::as_str),
        Some("kiln.factory-workspace.v1")
    );
    assert_eq!(
        kiln_json.get("profile").and_then(serde_json::Value::as_str),
        Some("service.json")
    );
    assert_eq!(
        kiln_json.get("access").and_then(serde_json::Value::as_str),
        Some("access.json")
    );
    assert_eq!(
        kiln_json.get("sources").and_then(serde_json::Value::as_str),
        Some("sources.json")
    );

    let start_here = fs::read_to_string(out.join("START_HERE.txt"))
        .unwrap_or_else(|error| unreachable!("failed to read START_HERE.txt: {error}"));
    assert!(start_here.contains("kiln.json"), "{start_here}");
    assert!(start_here.contains("kiln doctor --config "), "{start_here}");
    assert!(start_here.contains("kiln check --config "), "{start_here}");
    assert!(start_here.contains("kiln serve --config "), "{start_here}");

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

#[test]
fn doctor_validates_init_workspace_in_text_and_json() {
    let scratch = TempDirGuard::new("doctor-valid");
    let repo = scratch.path().join("sample-repo");
    init_git_repo(&repo);
    let out = scratch.path().join("workspace");

    let init_out = kiln(&[
        "init",
        "--repo",
        &repo.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--project",
        "doctor-proj",
    ]);
    assert_eq!(init_out.status.code(), Some(EXIT_SUCCESS));

    let kiln_config = out.join("kiln.json");
    let kiln_config_arg = kiln_config.to_string_lossy().into_owned();

    let text_out = kiln(&["doctor", "--config", &kiln_config_arg]);
    assert_eq!(
        text_out.status.code(),
        Some(EXIT_SUCCESS),
        "stdout: {}\nstderr: {}",
        text(&text_out.stdout),
        text(&text_out.stderr)
    );
    assert!(text_out.stderr.is_empty());
    let stdout = text(&text_out.stdout);
    assert!(stdout.contains("[ok] config.schema:"), "{stdout}");
    assert!(stdout.contains("[ok] config.path.profile:"), "{stdout}");
    assert!(stdout.contains("[ok] config.profile.load:"), "{stdout}");
    assert!(stdout.contains("status: ok"), "{stdout}");

    let json_out = kiln(&["doctor", "--config", &kiln_config_arg, "--json"]);
    assert_eq!(json_out.status.code(), Some(EXIT_SUCCESS));
    assert!(json_out.stderr.is_empty());
    let parsed: serde_json::Value = serde_json::from_slice(&json_out.stdout)
        .unwrap_or_else(|error| unreachable!("doctor --json invalid JSON: {error}"));
    assert_eq!(
        parsed.get("schema").and_then(serde_json::Value::as_str),
        Some("kiln.doctor-report.v1")
    );
    assert_eq!(
        parsed.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );

    let host_only = kiln(&["doctor", "--json"]);
    assert_eq!(host_only.status.code(), Some(EXIT_SUCCESS));
    let host_parsed: serde_json::Value = serde_json::from_slice(&host_only.stdout)
        .unwrap_or_else(|error| unreachable!("doctor --json invalid JSON: {error}"));
    assert_eq!(
        host_parsed.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn doctor_fails_cleanly_on_missing_config_broken_schema_missing_profile_and_bad_flags() {
    let scratch = TempDirGuard::new("doctor-invalid");

    // 1. Missing config file.
    let missing = scratch.path().join("missing-kiln.json");
    let missing_out = kiln(&["doctor", "--config", &missing.to_string_lossy(), "--json"]);
    assert_eq!(missing_out.status.code(), Some(EXIT_FAILURE));
    let missing_json: serde_json::Value = serde_json::from_slice(&missing_out.stdout)
        .unwrap_or_else(|error| unreachable!("doctor --json invalid JSON: {error}"));
    assert_eq!(
        missing_json.get("ok").and_then(serde_json::Value::as_bool),
        Some(false)
    );

    // 2. Broken schema in kiln.json.
    let bad_schema = scratch.path().join("bad-schema.json");
    if let Err(error) = fs::write(
        &bad_schema,
        br#"{"schema":"kiln.wrong.v0","profile":"service.json"}"#,
    ) {
        unreachable!("write bad-schema.json failed: {error}");
    }
    let bad_schema_out = kiln(&["doctor", "--config", &bad_schema.to_string_lossy()]);
    assert_eq!(bad_schema_out.status.code(), Some(EXIT_FAILURE));
    assert!(
        text(&bad_schema_out.stdout).contains("[error] config.schema:"),
        "{}",
        text(&bad_schema_out.stdout)
    );

    // 3. Missing referenced profile.
    let missing_profile = scratch.path().join("missing-profile.json");
    if let Err(error) = fs::write(
        &missing_profile,
        br#"{"schema":"kiln.factory-workspace.v1","profile":"does-not-exist.json"}"#,
    ) {
        unreachable!("write missing-profile.json failed: {error}");
    }
    let missing_prof_out = kiln(&[
        "doctor",
        "--config",
        &missing_profile.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(missing_prof_out.status.code(), Some(EXIT_FAILURE));
    let missing_prof_json: serde_json::Value = serde_json::from_slice(&missing_prof_out.stdout)
        .unwrap_or_else(|error| unreachable!("doctor --json invalid JSON: {error}"));
    assert_eq!(
        missing_prof_json
            .get("ok")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    // 4. Unknown flag exits with EXIT_USAGE (2) and prints usage on stderr.
    let bad_flag = kiln(&["doctor", "--unknown"]);
    assert_eq!(bad_flag.status.code(), Some(EXIT_USAGE));
    assert!(bad_flag.stdout.is_empty());
    assert!(text(&bad_flag.stderr).starts_with("usage: kiln doctor "));

    // 5. `kiln doctor --help` exits 0 and prints usage on stdout.
    let help = kiln(&["doctor", "--help"]);
    assert_eq!(help.status.code(), Some(EXIT_SUCCESS));
    assert!(text(&help.stdout).starts_with("usage: kiln doctor "));
    assert!(help.stderr.is_empty());
}

#[test]
fn init_doctor_and_in_process_check_round_trip_with_single_kiln_json() {
    let scratch = TempDirGuard::new("init-check-e2e");
    let repo = scratch.path().join("sample-repo");
    init_git_repo(&repo);
    let out = scratch.path().join("workspace");

    let init_out = kiln(&[
        "init",
        "--repo",
        &repo.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--project",
        "e2e-proj",
    ]);
    assert_eq!(init_out.status.code(), Some(EXIT_SUCCESS));

    let kiln_config = out.join("kiln.json");
    let kiln_config_arg = kiln_config.to_string_lossy().into_owned();

    let check_out = kiln(&["check", "--config", &kiln_config_arg]);
    assert_eq!(
        check_out.status.code(),
        Some(EXIT_SUCCESS),
        "stdout: {}\nstderr: {}",
        text(&check_out.stdout),
        text(&check_out.stderr)
    );

    let missing = scratch.path().join("nope.json");
    let missing_check = kiln(&["check", "--config", &missing.to_string_lossy()]);
    assert_eq!(missing_check.status.code(), Some(EXIT_FAILURE));
    assert!(
        !missing_check.stderr.is_empty(),
        "expected error diagnostic for missing --config"
    );

    let bad_flag = kiln(&["check", "--definitely-not-a-flag"]);
    assert_eq!(bad_flag.status.code(), Some(EXIT_USAGE));
}

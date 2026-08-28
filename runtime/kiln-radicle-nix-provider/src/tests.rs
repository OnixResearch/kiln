use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::admission::hash_bounded_file;
use super::*;

const TEST_UID_FALLBACK: u32 = 1;
const TEST_TIMEOUT_MILLISECONDS: u64 = 5_000;
const TEST_SHORT_TIMEOUT_MILLISECONDS: u64 = 20;
const TEST_OUTPUT_BYTES: u64 = 128;
const TEST_REPORT_BYTES: u64 = 8_192;
const TEST_WRAPPER_BYTES: u64 = 4_096;
const TEST_POLL_MILLISECONDS: u64 = 10;
const TEST_TEARDOWN_MILLISECONDS: u64 = 1_000;
const TEST_COLLISION_ATTEMPTS: usize = 4;
const TEST_FLOOD_ITERATIONS: usize = 4_096;
const TEST_WRAPPER_MODE: u32 = 0o700;
const TEST_REPORT_MODE: u32 = 0o640;
const PERMISSION_MODE_MASK: u32 = 0o777;
const TEST_REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn write_wrapper(root: &Path, body: &str) -> PathBuf {
    let path = root.join("provider-wrapper");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap_or_else(|_| unreachable!());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(TEST_WRAPPER_MODE))
        .unwrap_or_else(|_| unreachable!());
    path
}

fn fixture(body: &str) -> (TempDir, HostedNixProviderProfile) {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let source = temporary.path().join("source.git");
    let work = temporary.path().join("work");
    let reports = temporary.path().join("reports");
    let namespace = "seaglass";
    std::fs::create_dir_all(&source).unwrap_or_else(|_| unreachable!());
    std::fs::create_dir_all(&work).unwrap_or_else(|_| unreachable!());
    std::fs::create_dir_all(reports.join(namespace)).unwrap_or_else(|_| unreachable!());
    let wrapper = write_wrapper(temporary.path(), body);
    let wrapper_blake3 =
        hash_bounded_file(&wrapper, TEST_WRAPPER_BYTES).unwrap_or_else(|_| unreachable!());
    let uid = std::fs::metadata("/proc/self")
        .map(|metadata| metadata.uid())
        .unwrap_or(TEST_UID_FALLBACK);
    let profile = HostedNixProviderProfile {
        schema: HOSTED_NIX_PROVIDER_SCHEMA.to_string(),
        bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
        durable_file_publication_revision: PINNED_DURABLE_FILE_PUBLICATION_REVISION.to_string(),
        allowed_user_id: uid,
        repository: "rad:zseaglass".to_string(),
        source_root: source,
        wrapper_path: wrapper,
        wrapper_blake3,
        working_directory: work,
        report_root: reports,
        report_namespace: namespace.to_string(),
        report_base_url: "https://ci.invalid/reports".to_string(),
        limits: HostedProviderLimits {
            timeout_milliseconds: TEST_TIMEOUT_MILLISECONDS,
            stdin_max_bytes: 1,
            stdout_max_bytes: TEST_OUTPUT_BYTES,
            stderr_max_bytes: TEST_OUTPUT_BYTES,
            poll_interval_milliseconds: TEST_POLL_MILLISECONDS,
            teardown_timeout_milliseconds: TEST_TEARDOWN_MILLISECONDS,
            wrapper_max_bytes: TEST_WRAPPER_BYTES,
            report_max_bytes: TEST_REPORT_BYTES,
            stage_collision_attempts: TEST_COLLISION_ATTEMPTS,
        },
        non_claims: REQUIRED_PROVIDER_NON_CLAIMS.map(str::to_string).to_vec(),
    };
    (temporary, profile)
}

fn request() -> HostedProviderRequest {
    HostedProviderRequest {
        repository: "rad:zseaglass".to_string(),
        revision: TEST_REVISION.to_string(),
    }
}

#[test]
fn exact_provider_succeeds_with_explicit_arguments_and_clear_environment() {
    let body = r#"
[ "${HOME+x}" = x ] && exit 7
[ "$1" = "flake" ] || exit 8
[ "$2" = "check" ] || exit 9
[ "$3" = "--no-update-lock-file" ] || exit 10
case "$4" in git+file://*'?rev='*) ;; *) exit 11 ;; esac
printf 'provider-ok'
"#;
    let (_temporary, profile) = fixture(body);
    let first =
        execute_hosted_nix_provider(&profile, &request()).unwrap_or_else(|_| unreachable!());
    let repeated =
        execute_hosted_nix_provider(&profile, &request()).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, repeated);
    assert_eq!(first.outcome, HostedProviderOutcome::Success);
    assert!(first.log_url.ends_with(".log"));
    let run_leaf = first
        .run_identity
        .strip_prefix("b3:")
        .unwrap_or_else(|| unreachable!());
    for extension in ["log", "json"] {
        let path = profile
            .report_root
            .join(&profile.report_namespace)
            .join(format!("{run_leaf}.{extension}"));
        let mode = std::fs::metadata(path)
            .unwrap_or_else(|_| unreachable!())
            .mode()
            & PERMISSION_MODE_MASK;
        assert_eq!(mode, TEST_REPORT_MODE);
    }
}

#[test]
fn changed_repository_user_and_wrapper_are_rejected() {
    let (_temporary, profile) = fixture("printf 'provider-ok'");
    let mut wrong_repository = request();
    wrong_repository.repository = "rad:zother".to_string();
    assert!(execute_hosted_nix_provider(&profile, &wrong_repository).is_err());

    let mut wrong_user = profile.clone();
    wrong_user.allowed_user_id = wrong_user.allowed_user_id.saturating_add(1);
    assert!(execute_hosted_nix_provider(&wrong_user, &request()).is_err());

    std::fs::write(&profile.wrapper_path, "changed").unwrap_or_else(|_| unreachable!());
    assert!(execute_hosted_nix_provider(&profile, &request()).is_err());
}

#[test]
fn failure_output_flood_and_timeout_do_not_become_success() {
    let (_failed_directory, failed) = fixture("exit 7");
    let failed_receipt =
        execute_hosted_nix_provider(&failed, &request()).unwrap_or_else(|_| unreachable!());
    assert_eq!(failed_receipt.outcome, HostedProviderOutcome::Failure);

    let flood_body = format!(
        "count=0; while [ \"$count\" -lt {TEST_FLOOD_ITERATIONS} ]; do printf x; count=$((count + 1)); done"
    );
    let (_flood_directory, flood) = fixture(&flood_body);
    let flood_receipt =
        execute_hosted_nix_provider(&flood, &request()).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        flood_receipt.outcome,
        HostedProviderOutcome::OutputLimitExceeded
    );

    let (_timeout_directory, mut timeout) = fixture("while :; do :; done");
    timeout.limits.timeout_milliseconds = TEST_SHORT_TIMEOUT_MILLISECONDS;
    let timeout_receipt =
        execute_hosted_nix_provider(&timeout, &request()).unwrap_or_else(|_| unreachable!());
    assert_eq!(timeout_receipt.outcome, HostedProviderOutcome::TimedOut);
}

#[test]
fn malformed_profile_json_and_unknown_fields_fail_closed() {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let malformed = temporary.path().join("malformed.json");
    std::fs::write(&malformed, b"not-json").unwrap_or_else(|_| unreachable!());
    assert!(load_hosted_provider_profile(&malformed).is_err());

    let (_fixture_directory, profile) = fixture("printf 'provider-ok'");
    let mut value = serde_json::to_value(profile).unwrap_or_else(|_| unreachable!());
    let object = value.as_object_mut().unwrap_or_else(|| unreachable!());
    object.insert(
        "ambient_authority".to_string(),
        serde_json::Value::Bool(true),
    );
    let unknown = temporary.path().join("unknown.json");
    std::fs::write(
        &unknown,
        serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(load_hosted_provider_profile(&unknown).is_err());
}

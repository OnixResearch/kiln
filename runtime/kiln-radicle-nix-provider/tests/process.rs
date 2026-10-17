//! Actual provider-process control drills, not Nix, source-trust, or CI qualification.
//! Linux, a non-root UID, /proc, and /bin/sh are explicit prerequisites.

#![cfg(target_os = "linux")]

use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[cfg(test)]
#[path = "process/fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "process/observation.rs"]
mod observation;
#[cfg(test)]
#[path = "process/running.rs"]
mod running;

const REPOSITORY: &str = "rad:zprocess-control-fixture";
const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const EXECUTION_TIMEOUT_MS: u64 = 3_000;
const CANCELLATION_EXECUTION_TIMEOUT_MS: u64 = 30_000;
const TEARDOWN_BOUND_MS: u64 = 1_000;
const READINESS_BOUND_MS: u64 = 2_000;
const PUBLICATION_ALLOWANCE_MS: u64 = 3_000;
const POLL_INTERVAL_MS: u64 = 10;
const OUTPUT_BOUND_BYTES: u64 = 256;
const REPORT_BOUND_BYTES: u64 = 8_192;
const WRAPPER_BOUND_BYTES: u64 = 4_096;
const FLOOD_WRITES: u64 = 4_096;
const CHECK_FAILED_EXIT_CODE: i32 = 3;
const OWNED_PROCESS_COUNT: usize = 2;

#[test]
fn actual_provider_success_reaps_descendant_and_publishes_bounded_report() -> TestResult {
    let fixture = fixture::Harness::new(
        "printf done > release-child; wait \"$descendant\"; printf bounded-control-ok",
        EXECUTION_TIMEOUT_MS,
        std::time::Instant::now,
    )?;
    let mut provider = fixture.start()?;
    provider.await_descendant()?;
    let teardown_deadline = (fixture.clock)() + std::time::Duration::from_millis(TEARDOWN_BOUND_MS);
    provider.proceed()?;
    let status = provider.finish(
        std::time::Duration::from_millis(
            EXECUTION_TIMEOUT_MS + TEARDOWN_BOUND_MS + PUBLICATION_ALLOWANCE_MS,
        ),
        teardown_deadline,
    )?;
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(&provider.stderr_path)?
    );
    let receipt = provider.receipt()?;
    assert_eq!(
        receipt.outcome,
        kiln_radicle_nix_provider::HostedProviderOutcome::Success
    );
    let leaf = receipt
        .run_identity
        .strip_prefix("b3:")
        .ok_or("invalid run identity")?;
    let directory = fixture
        .profile
        .report_root
        .join(&fixture.profile.report_namespace);
    let log = std::fs::read(directory.join(format!("{leaf}.log")))?;
    let report = std::fs::read(directory.join(format!("{leaf}.json")))?;
    assert!(u64::try_from(log.len())? <= REPORT_BOUND_BYTES);
    assert!(u64::try_from(report.len())? <= REPORT_BOUND_BYTES);
    assert_eq!(blake3::hash(&log).to_hex().as_str(), receipt.log_blake3);
    assert_eq!(
        blake3::hash(&report).to_hex().as_str(),
        receipt.report_blake3
    );
    let control_output = b"bounded-control-ok";
    assert!(
        log.windows(control_output.len())
            .any(|bytes| bytes == control_output)
    );
    Ok(())
}

#[test]
fn actual_provider_timeout_terminates_owned_descendant() -> TestResult {
    running::exercise_limit(
        "wait \"$descendant\"",
        kiln_radicle_nix_provider::HostedProviderOutcome::TimedOut,
        std::time::Instant::now,
    )
}

#[test]
fn actual_provider_stdout_limit_terminates_owned_descendant() -> TestResult {
    running::exercise_limit(
        &format!(
            "count=0; while [ \"$count\" -lt {FLOOD_WRITES} ]; do printf x; count=$((count + 1)); done; exit 0"
        ),
        kiln_radicle_nix_provider::HostedProviderOutcome::OutputLimitExceeded,
        std::time::Instant::now,
    )
}

#[test]
fn actual_provider_stderr_limit_terminates_owned_descendant() -> TestResult {
    running::exercise_limit(
        &format!(
            "count=0; while [ \"$count\" -lt {FLOOD_WRITES} ]; do printf x >&2; count=$((count + 1)); done; exit 0"
        ),
        kiln_radicle_nix_provider::HostedProviderOutcome::OutputLimitExceeded,
        std::time::Instant::now,
    )
}

#[test]
fn actual_provider_sigterm_cancellation_terminates_owned_descendant() -> TestResult {
    let fixture = fixture::Harness::new(
        "wait \"$descendant\"",
        CANCELLATION_EXECUTION_TIMEOUT_MS,
        std::time::Instant::now,
    )?;
    let mut provider = fixture.start()?;
    provider.await_descendant()?;
    provider.proceed()?;
    let deadline = (fixture.clock)() + std::time::Duration::from_millis(TEARDOWN_BOUND_MS);
    observation::signal(provider.child.id(), rustix::process::Signal::TERM)?;
    let status = provider.finish(
        std::time::Duration::from_millis(TEARDOWN_BOUND_MS + PUBLICATION_ALLOWANCE_MS),
        deadline,
    )?;
    assert_eq!(status.code(), Some(CHECK_FAILED_EXIT_CODE));
    assert_eq!(
        provider.receipt()?.outcome,
        kiln_radicle_nix_provider::HostedProviderOutcome::Cancelled
    );
    Ok(())
}

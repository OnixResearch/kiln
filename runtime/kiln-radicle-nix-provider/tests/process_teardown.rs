//! Actual provider-process control drills, not Nix, source-trust, or CI qualification.
//! Linux, a non-root UID, /proc, and /bin/sh are explicit prerequisites.

#![cfg(target_os = "linux")]

use std::error::Error;
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use kiln_radicle_nix_provider::{
    HOSTED_NIX_PROVIDER_SCHEMA, HostedNixProviderProfile, HostedProviderLimits,
    HostedProviderOutcome, HostedProviderReceipt, PINNED_BOUNDED_EXEC_REVISION,
    PINNED_DURABLE_FILE_PUBLICATION_REVISION, REQUIRED_PROVIDER_NON_CLAIMS,
};
use rustix::process::{Pid, Signal, kill_process};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

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

#[derive(Debug)]
struct Fixture {
    root: TempDir,
    profile: HostedNixProviderProfile,
}

impl Fixture {
    fn new(operation: &str, timeout_ms: u64) -> TestResult<Self> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source.git");
        let work = root.path().join("work");
        let reports = root.path().join("reports");
        for directory in [&source, &work, &reports.join("process-control")] {
            fs::create_dir_all(directory)?;
        }
        // All PID and release files are relative to the admitted working directory.
        // Only two owned processes exist: this wrapper and its shell descendant.
        let body = format!(
            r#"#!/bin/sh
[ "$#" = 4 ] || exit 41
[ "$1" = flake ] || exit 42
[ "$2" = check ] || exit 43
[ "$3" = --no-update-lock-file ] || exit 44
[ "$4" = 'git+file://{source}?rev={REVISION}' ] || exit 45
[ "${{KILN_PROVIDER_AMBIENT_SECRET+x}}" != x ] || exit 46
printf '%s\n' "$$" > wrapper.pid
/bin/sh -c 'printf "%s\n" "$$" > descendant.pid; while [ ! -f release-child ]; do :; done' &
descendant=$!
while [ ! -f proceed ]; do :; done
{operation}
"#,
            source = source.display(),
        );
        let wrapper = root.path().join("controlled-wrapper");
        fs::write(&wrapper, &body)?;
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700))?;
        let uid = fs::metadata("/proc/self")?.uid();
        assert_ne!(uid, 0, "the provider intentionally refuses root authority");
        let profile = HostedNixProviderProfile {
            schema: HOSTED_NIX_PROVIDER_SCHEMA.to_string(),
            bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
            durable_file_publication_revision: PINNED_DURABLE_FILE_PUBLICATION_REVISION.to_string(),
            allowed_user_id: uid,
            repository: REPOSITORY.to_string(),
            source_root: source,
            wrapper_path: wrapper,
            wrapper_blake3: blake3::hash(body.as_bytes()).to_hex().to_string(),
            working_directory: work,
            report_root: reports,
            report_namespace: "process-control".to_string(),
            report_base_url: "https://ci.invalid/process-control".to_string(),
            limits: HostedProviderLimits {
                timeout_milliseconds: timeout_ms,
                stdin_max_bytes: 1,
                stdout_max_bytes: OUTPUT_BOUND_BYTES,
                stderr_max_bytes: OUTPUT_BOUND_BYTES,
                poll_interval_milliseconds: POLL_INTERVAL_MS,
                teardown_timeout_milliseconds: TEARDOWN_BOUND_MS,
                wrapper_max_bytes: WRAPPER_BOUND_BYTES,
                report_max_bytes: REPORT_BOUND_BYTES,
                stage_collision_attempts: 4,
            },
            non_claims: REQUIRED_PROVIDER_NON_CLAIMS.map(str::to_string).to_vec(),
        };
        Ok(Self { root, profile })
    }

    fn start(&self) -> TestResult<RunningProvider> {
        let profile_path = self.root.path().join("profile.json");
        fs::write(&profile_path, serde_json::to_vec(&self.profile)?)?;
        let stdout_path = self.root.path().join("provider.stdout");
        let stderr_path = self.root.path().join("provider.stderr");
        let started = Instant::now();
        let child = Command::new(env!("CARGO_BIN_EXE_kiln-radicle-nix-provider"))
            .args([
                "--profile",
                profile_path.to_str().ok_or("non-UTF8 fixture path")?,
            ])
            .args(["--repository", REPOSITORY, "--revision", REVISION])
            .env("KILN_PROVIDER_AMBIENT_SECRET", "must-not-reach-wrapper")
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout_path)?)
            .stderr(fs::File::create(&stderr_path)?)
            .spawn()?;
        Ok(RunningProvider {
            child,
            started,
            work: self.profile.working_directory.clone(),
            stdout_path,
            stderr_path,
            owned: Vec::new(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessIdentity {
    pid: u32,
    group: u32,
    start_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessObservation {
    identity: ProcessIdentity,
    state: char,
}

fn observe_process(pid: u32) -> TestResult<Option<ProcessObservation>> {
    let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    // comm may itself contain spaces or parentheses; the last ')' ends it.
    let (_, fields) = stat.rsplit_once(')').ok_or("malformed proc stat")?;
    let mut fields = fields.split_whitespace();
    let state = fields
        .next()
        .ok_or("missing process state")?
        .chars()
        .next()
        .ok_or("empty process state")?;
    let group = fields.nth(1).ok_or("missing process group")?.parse()?;
    let start_ticks = fields
        .nth(16)
        .ok_or("missing process start time")?
        .parse()?;
    Ok(Some(ProcessObservation {
        identity: ProcessIdentity {
            pid,
            group,
            start_ticks,
        },
        state,
    }))
}

#[derive(Debug)]
struct RunningProvider {
    child: Child,
    started: Instant,
    work: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    owned: Vec<ProcessIdentity>,
}

impl RunningProvider {
    fn await_descendant(&mut self) -> TestResult {
        let deadline = Instant::now() + Duration::from_millis(READINESS_BOUND_MS);
        loop {
            if self.work.join("wrapper.pid").exists() && self.work.join("descendant.pid").exists() {
                let wrapper = read_identity(&self.work.join("wrapper.pid"))?;
                let descendant = read_identity(&self.work.join("descendant.pid"))?;
                if let (Some(wrapper), Some(descendant)) = (wrapper, descendant) {
                    assert_ne!(wrapper.pid, descendant.pid);
                    assert_eq!(
                        wrapper.group, wrapper.pid,
                        "wrapper must own its process group"
                    );
                    assert_eq!(
                        descendant.group, wrapper.group,
                        "descendant must belong to owned group"
                    );
                    self.owned = vec![wrapper, descendant];
                    return Ok(());
                }
            }
            if let Some(status) = self.child.try_wait()? {
                return Err(format!(
                    "provider exited before descendant readiness: {status}; {}",
                    fs::read_to_string(&self.stderr_path)?
                )
                .into());
            }
            if Instant::now() >= deadline {
                return Err("descendant readiness exceeded named bound".into());
            }
            thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        }
    }

    fn proceed(&self) -> TestResult {
        fs::write(self.work.join("proceed"), b"run")?;
        Ok(())
    }

    fn finish(&mut self, timeout_ms: u64, teardown_deadline: Instant) -> TestResult<ExitStatus> {
        let completion_deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut observed_terminated = false;
        loop {
            if !observed_terminated {
                let mut survivors = Vec::new();
                let mut observations = Vec::new();
                for identity in &self.owned {
                    let observation = observe_process(identity.pid)?;
                    let same_process = observation.filter(|value| value.identity == *identity);
                    // The wrapper is the provider's direct child and must be
                    // reaped. Its descendant may instead be a terminated zombie
                    // awaiting reaping by PID1; that is not claimed as absent.
                    let is_wrapper = identity.pid == identity.group;
                    let is_terminated = match same_process {
                        None => true,
                        Some(value) => !is_wrapper && value.state == 'Z',
                    };
                    observations.push((*identity, observation));
                    if !is_terminated {
                        survivors.push((*identity, observation));
                    }
                }
                if Instant::now() > teardown_deadline {
                    return Err(format!("direct-child reaping/descendant termination was not observed within named teardown bound: {survivors:?}; all observations: {observations:?}").into());
                }
                observed_terminated = survivors.is_empty();
                if observed_terminated {
                    eprintln!(
                        "provider teardown observations (None means original process absent; Z means terminated but unreaped, not absent): {observations:?}"
                    );
                }
            }
            if let Some(status) = self.child.try_wait()?
                && observed_terminated
            {
                return Ok(status);
            }
            if Instant::now() >= completion_deadline {
                return Err("provider completion exceeded named bound".into());
            }
            thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        }
    }

    fn receipt(&self) -> TestResult<HostedProviderReceipt> {
        Ok(serde_json::from_slice(&fs::read(&self.stdout_path)?)?)
    }
}

fn read_identity(path: &Path) -> TestResult<Option<ProcessIdentity>> {
    let text = fs::read_to_string(path)?;
    let Ok(pid) = text.trim().parse() else {
        // A freshly created PID file may not have received its bytes yet.
        return Ok(None);
    };
    Ok(observe_process(pid)?.map(|observation| observation.identity))
}

fn signal(pid: u32, signal: Signal) -> TestResult {
    let raw_pid = i32::try_from(pid)?;
    let pid = Pid::from_raw(raw_pid).ok_or("provider process ID must be nonzero")?;
    kill_process(pid, signal)?;
    Ok(())
}

impl Drop for RunningProvider {
    fn drop(&mut self) {
        // Emergency cleanup follows, never precedes, the behavioral assertions.
        // Recover PID files even when readiness failed before recording identities.
        for leaf in ["wrapper.pid", "descendant.pid"] {
            if let Ok(Some(identity)) = read_identity(&self.work.join(leaf))
                && !self.owned.contains(&identity)
            {
                self.owned.push(identity);
            }
        }
        for identity in &self.owned {
            if observe_process(identity.pid)
                .ok()
                .flatten()
                .is_some_and(|observation| {
                    observation.identity == *identity && observation.state != 'Z'
                })
            {
                let _ = signal(identity.pid, Signal::KILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn exercise_limit(operation: &str, expected: HostedProviderOutcome) -> TestResult {
    let fixture = Fixture::new(operation, EXECUTION_TIMEOUT_MS)?;
    let mut provider = fixture.start()?;
    provider.await_descendant()?;
    let teardown_deadline = if expected == HostedProviderOutcome::TimedOut {
        provider.started + Duration::from_millis(EXECUTION_TIMEOUT_MS + TEARDOWN_BOUND_MS)
    } else {
        Instant::now() + Duration::from_millis(TEARDOWN_BOUND_MS)
    };
    provider.proceed()?;
    let status = provider.finish(
        EXECUTION_TIMEOUT_MS + TEARDOWN_BOUND_MS + PUBLICATION_ALLOWANCE_MS,
        teardown_deadline,
    )?;
    assert_eq!(
        status.code(),
        Some(CHECK_FAILED_EXIT_CODE),
        "{}",
        fs::read_to_string(&provider.stderr_path)?
    );
    assert_eq!(provider.receipt()?.outcome, expected);
    Ok(())
}

#[test]
fn actual_provider_success_reaps_descendant_and_publishes_bounded_report() -> TestResult {
    let fixture = Fixture::new(
        "printf done > release-child; wait \"$descendant\"; printf bounded-control-ok",
        EXECUTION_TIMEOUT_MS,
    )?;
    let mut provider = fixture.start()?;
    provider.await_descendant()?;
    let teardown_deadline = Instant::now() + Duration::from_millis(TEARDOWN_BOUND_MS);
    provider.proceed()?;
    let status = provider.finish(
        EXECUTION_TIMEOUT_MS + TEARDOWN_BOUND_MS + PUBLICATION_ALLOWANCE_MS,
        teardown_deadline,
    )?;
    assert!(
        status.success(),
        "{}",
        fs::read_to_string(&provider.stderr_path)?
    );
    let receipt = provider.receipt()?;
    assert_eq!(receipt.outcome, HostedProviderOutcome::Success);
    let leaf = receipt
        .run_identity
        .strip_prefix("b3:")
        .ok_or("invalid run identity")?;
    let directory = fixture
        .profile
        .report_root
        .join(&fixture.profile.report_namespace);
    let log = fs::read(directory.join(format!("{leaf}.log")))?;
    let report = fs::read(directory.join(format!("{leaf}.json")))?;
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
    exercise_limit("wait \"$descendant\"", HostedProviderOutcome::TimedOut)
}

#[test]
fn actual_provider_stdout_limit_terminates_owned_descendant() -> TestResult {
    exercise_limit(
        &format!(
            "count=0; while [ \"$count\" -lt {FLOOD_WRITES} ]; do printf x; count=$((count + 1)); done; exit 0"
        ),
        HostedProviderOutcome::OutputLimitExceeded,
    )
}

#[test]
fn actual_provider_stderr_limit_terminates_owned_descendant() -> TestResult {
    exercise_limit(
        &format!(
            "count=0; while [ \"$count\" -lt {FLOOD_WRITES} ]; do printf x >&2; count=$((count + 1)); done; exit 0"
        ),
        HostedProviderOutcome::OutputLimitExceeded,
    )
}

#[test]
fn actual_provider_sigterm_cancellation_terminates_owned_descendant() -> TestResult {
    let fixture = Fixture::new("wait \"$descendant\"", CANCELLATION_EXECUTION_TIMEOUT_MS)?;
    let mut provider = fixture.start()?;
    provider.await_descendant()?;
    provider.proceed()?;
    let deadline = Instant::now() + Duration::from_millis(TEARDOWN_BOUND_MS);
    signal(provider.child.id(), Signal::TERM)?;
    let status = provider.finish(TEARDOWN_BOUND_MS + PUBLICATION_ALLOWANCE_MS, deadline)?;
    assert_eq!(status.code(), Some(CHECK_FAILED_EXIT_CODE));
    assert_eq!(
        provider.receipt()?.outcome,
        HostedProviderOutcome::Cancelled
    );
    Ok(())
}

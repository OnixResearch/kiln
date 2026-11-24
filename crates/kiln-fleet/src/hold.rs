//! Supervised fleet claim holder (`kiln-fleet hold`).
//!
//! Acquires a fresh `attempt: None` claim via `POST /v1/claims` (requiring
//! HTTP `201` with `"outcome": "granted"`, defaulting to a short 15-minute
//! lease; same-holder `"outcome": "existing"` is refused), spawns a child
//! command with inherited stdio and environment inside its own process group,
//! renews the claim at `<= 1/3` of the lease while tracking the current fence,
//! and releases the claim when the child exits or on `SIGINT`/`SIGTERM`.
//!
//! Unreachable-coordinator policy on renew: transport errors and `5xx`
//! responses log a warning and keep the child running while retrying until
//! `lease_deadline_ms` (`min(expires_ms, request_start + lease_ms)`, assuming
//! client and coordinator wall clocks are reasonably synchronized); once
//! `now_after >= lease_deadline_ms`, `hold` warns, terminates the child process
//! group, sends no release, and exits `1`.

use std::io::Write as _;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::client::ApiClient;
use crate::log;

pub const DEFAULT_HOLD_LEASE_MINUTES: u64 = 15;
pub const MIN_HOLD_LEASE_MINUTES: u64 = 1;
pub const MAX_HOLD_LEASE_MINUTES: u64 = 24 * 60;
pub const MS_PER_MINUTE: u64 = 60_000;
pub const RENEW_DIVISOR: u64 = 3;
pub const POLL_INTERVAL_MS: u64 = 50;
pub const UNREACHABLE_RETRY_MS: u64 = 5_000;
pub const STOP_GRACE_MS: u64 = 2_000;
pub const EXIT_REFUSED: i32 = 1;
pub const EXIT_SIGINT: i32 = 130;
pub const EXIT_SIGTERM: i32 = 143;

/// Parameters for a supervised `kiln-fleet hold` execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoldSpec {
    pub scope: String,
    pub tool: String,
    pub session: String,
    pub purpose: String,
    pub lease_minutes: u64,
    pub command: Vec<String>,
}

/// Clock and sleeper seam so tests can exercise renewal and expiry schedules
/// deterministically without sleeping for minutes.
pub trait HoldClock {
    fn now_ms(&self) -> u64;
    fn sleep_ms(&self, ms: u64);
}

/// Wall-clock [`HoldClock`] implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl HoldClock for SystemClock {
    fn now_ms(&self) -> u64 {
        log::now_ms()
    }

    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

/// Signal flags observed by [`run_with`].
#[derive(Clone, Debug, Default)]
pub struct HoldSignals {
    pub sigint: Arc<AtomicBool>,
    pub sigterm: Arc<AtomicBool>,
}

/// RAII unregistration guard for OS signal handlers registered by [`HoldSignals::register`].
#[derive(Debug)]
pub struct SignalGuard {
    registrations: Vec<signal_hook::SigId>,
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        for id in self.registrations.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

impl HoldSignals {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register OS `SIGINT` and `SIGTERM` handlers backed by atomic flags.
    ///
    /// # Errors
    /// Returns a diagnostic if signal registration fails.
    pub fn register() -> Result<(Self, SignalGuard), String> {
        let signals = Self::new();
        let mut registrations = Vec::new();
        for (signal, flag) in [
            (signal_hook::consts::SIGINT, Arc::clone(&signals.sigint)),
            (signal_hook::consts::SIGTERM, Arc::clone(&signals.sigterm)),
        ] {
            match signal_hook::flag::register(signal, flag) {
                Ok(id) => registrations.push(id),
                Err(error) => {
                    for id in registrations {
                        signal_hook::low_level::unregister(id);
                    }
                    return Err(format!("cannot register signal handler: {error}"));
                }
            }
        }
        Ok((signals, SignalGuard { registrations }))
    }
}

/// Acquire a fleet claim, run the wrapped command while renewing, and release on exit or signal.
#[must_use]
pub fn run(client: &ApiClient, spec: &HoldSpec) -> i32 {
    let (signals, _guard) = match HoldSignals::register() {
        Ok(registered) => registered,
        Err(error) => {
            write_stderr(&json!({ "error": error }));
            return EXIT_REFUSED;
        }
    };
    run_with(client, spec, &SystemClock, &signals)
}

/// Run [`run`] with injectable clock and signal flags.
#[must_use]
pub fn run_with(
    client: &ApiClient,
    spec: &HoldSpec,
    clock: &dyn HoldClock,
    signals: &HoldSignals,
) -> i32 {
    let Some((program, args)) = spec.command.split_first() else {
        write_stderr(&json!({ "error": "hold requires a command after --" }));
        return EXIT_REFUSED;
    };
    let acquire_body = json!({
        "scope": spec.scope,
        "tool": spec.tool,
        "session": spec.session,
        "purpose": spec.purpose,
        "lease_minutes": spec.lease_minutes,
    });
    let acquire_now = clock.now_ms();
    let lease_ms = spec.lease_minutes.saturating_mul(MS_PER_MINUTE);
    let (claim_id, mut fence, mut lease_deadline_ms) =
        match client.request("POST", "/v1/claims", Some(&acquire_body)) {
            Ok((201, value)) if value.get("outcome").and_then(Value::as_str) == Some("granted") => {
                let Some(claim_id) = value
                    .pointer("/claim/identity")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    write_stderr(&json!({
                        "error": "claim response missing claim.identity",
                        "response": value,
                    }));
                    return EXIT_REFUSED;
                };
                let Some(fence) = value.pointer("/claim/fence").and_then(Value::as_u64) else {
                    write_stderr(&json!({
                        "error": "claim response missing claim.fence",
                        "response": value,
                    }));
                    return EXIT_REFUSED;
                };
                let local_deadline_ms = acquire_now.saturating_add(lease_ms);
                let expires_ms = value
                    .pointer("/claim/expires_ms")
                    .and_then(Value::as_u64)
                    .map_or(local_deadline_ms, |server_expires| {
                        server_expires.min(local_deadline_ms)
                    });
                (claim_id, fence, expires_ms)
            }
            Ok((status, value)) => {
                write_stderr(&json!({ "status": status, "response": value }));
                return EXIT_REFUSED;
            }
            Err(error) => {
                write_stderr(&json!({ "error": error }));
                return EXIT_REFUSED;
            }
        };
    let acquire_after = clock.now_ms();

    if let Some((sig_name, exit_code)) = check_signal(signals) {
        let note = format!("signal {sig_name}");
        release_claim(client, &claim_id, fence, &note);
        return exit_code;
    }

    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            write_stderr(&json!({ "error": format!("cannot start {program}: {error}") }));
            release_claim(client, &claim_id, fence, "spawn failed");
            return EXIT_REFUSED;
        }
    };

    let renew_interval_ms = (lease_ms / RENEW_DIVISOR).max(1);
    let mut next_renew_ms = acquire_after
        .saturating_add(renew_interval_ms)
        .min(acquire_now.saturating_add(lease_ms.saturating_sub(renew_interval_ms)))
        .min(lease_deadline_ms);

    loop {
        if let Some((sig_name, exit_code)) = check_signal(signals) {
            terminate_child(&mut child);
            let note = format!("signal {sig_name}");
            release_claim(client, &claim_id, fence, &note);
            return exit_code;
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                let (exit_code, note) = exit_outcome(status);
                release_claim(client, &claim_id, fence, &note);
                return exit_code;
            }
            Ok(None) => {}
            Err(error) => {
                log::warn(
                    "hold.wait_failed",
                    json!({ "claim": claim_id, "error": error.to_string() }),
                );
                release_claim(client, &claim_id, fence, "wait failed");
                return EXIT_REFUSED;
            }
        }

        let now = clock.now_ms();
        if now >= next_renew_ms {
            let path = format!("/v1/claims/{claim_id}/renew");
            let body = json!({
                "fence": fence,
                "lease_minutes": spec.lease_minutes,
            });
            let response = client.request("POST", &path, Some(&body));
            let now_after = clock.now_ms();
            match response {
                Ok((status, value)) if (200..300).contains(&status) => {
                    if let Some(new_fence) = value.pointer("/claim/fence").and_then(Value::as_u64) {
                        fence = new_fence;
                    }
                    let local_deadline_ms = now.saturating_add(lease_ms);
                    lease_deadline_ms = value
                        .pointer("/claim/expires_ms")
                        .and_then(Value::as_u64)
                        .map_or(local_deadline_ms, |server_expires| {
                            server_expires.min(local_deadline_ms)
                        });
                    next_renew_ms = now_after
                        .saturating_add(renew_interval_ms)
                        .min(now.saturating_add(lease_ms.saturating_sub(renew_interval_ms)))
                        .min(lease_deadline_ms);
                }
                Ok((status, value)) if (400..500).contains(&status) => {
                    write_stderr(&json!({ "status": status, "response": value }));
                    terminate_child(&mut child);
                    return EXIT_REFUSED;
                }
                Ok((status, value)) => {
                    if now_after >= lease_deadline_ms {
                        log::warn(
                            "hold.renew_expired_unreachable",
                            json!({ "claim": claim_id, "status": status, "response": value }),
                        );
                        write_stderr(&json!({ "status": status, "response": value }));
                        terminate_child(&mut child);
                        return EXIT_REFUSED;
                    }
                    log::warn(
                        "hold.renew_unreachable",
                        json!({ "claim": claim_id, "status": status, "expires_ms": lease_deadline_ms }),
                    );
                    next_renew_ms = now_after
                        .saturating_add(UNREACHABLE_RETRY_MS.min(renew_interval_ms))
                        .min(lease_deadline_ms);
                }
                Err(error) => {
                    if now_after >= lease_deadline_ms {
                        log::warn(
                            "hold.renew_expired_unreachable",
                            json!({ "claim": claim_id, "error": error }),
                        );
                        write_stderr(&json!({ "error": error }));
                        terminate_child(&mut child);
                        return EXIT_REFUSED;
                    }
                    log::warn(
                        "hold.renew_unreachable",
                        json!({ "claim": claim_id, "error": error, "expires_ms": lease_deadline_ms }),
                    );
                    next_renew_ms = now_after
                        .saturating_add(UNREACHABLE_RETRY_MS.min(renew_interval_ms))
                        .min(lease_deadline_ms);
                }
            }
        }

        let remaining = next_renew_ms.saturating_sub(clock.now_ms());
        let sleep_for = POLL_INTERVAL_MS.min(remaining.max(1));
        clock.sleep_ms(sleep_for);
    }
}

fn check_signal(signals: &HoldSignals) -> Option<(&'static str, i32)> {
    if signals.sigint.load(Ordering::SeqCst) {
        Some(("SIGINT", EXIT_SIGINT))
    } else if signals.sigterm.load(Ordering::SeqCst) {
        Some(("SIGTERM", EXIT_SIGTERM))
    } else {
        None
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChildPoll {
    Alive,
    ExitedUnreaped,
    AlreadyReaped,
}

#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    all(target_os = "linux", not(target_env = "uclibc")),
))]
fn poll_child_unreaped(pid: nix::unistd::Pid) -> ChildPoll {
    use nix::errno::Errno;
    use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};

    match waitid(
        Id::Pid(pid),
        WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT | WaitPidFlag::WNOHANG,
    ) {
        Ok(WaitStatus::StillAlive) | Err(Errno::EINTR) => ChildPoll::Alive,
        Ok(_) => ChildPoll::ExitedUnreaped,
        Err(Errno::ECHILD) => ChildPoll::AlreadyReaped,
        Err(_) => ChildPoll::ExitedUnreaped,
    }
}

#[cfg(unix)]
fn terminate_child(child: &mut Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let Ok(raw_pid) = i32::try_from(child.id()) else {
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
            let _ = child.wait();
        }
        return;
    };
    let pgid = Pid::from_raw(raw_pid);

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "haiku",
        all(target_os = "linux", not(target_env = "uclibc")),
    ))]
    {
        match poll_child_unreaped(pgid) {
            ChildPoll::AlreadyReaped => return,
            ChildPoll::ExitedUnreaped => {
                // Direct child is an unreaped zombie (PID/PGID cannot be reused);
                // kill any surviving grandchildren before reaping the leader.
                let _ = killpg(pgid, Signal::SIGKILL);
                let _ = child.wait();
                return;
            }
            ChildPoll::Alive => {}
        }

        // Always terminate the child process group with SIGTERM (even when hold
        // caught SIGINT), because POSIX non-interactive shells set SIG_IGN on
        // SIGINT for backgrounded `&` grandchildren.
        let _ = killpg(pgid, Signal::SIGTERM);

        let deadline = Instant::now() + Duration::from_millis(STOP_GRACE_MS);
        loop {
            match poll_child_unreaped(pgid) {
                ChildPoll::AlreadyReaped => return,
                ChildPoll::ExitedUnreaped => break,
                ChildPoll::Alive => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            std::thread::sleep(remaining.min(Duration::from_millis(20)));
        }

        // Escalate to SIGKILL on the process group BEFORE reaping `child` so the
        // leader's zombie still pins `pgid` (preventing PID reuse) and any
        // SIGTERM-ignoring grandchildren are killed even if the direct child died early.
        let _ = killpg(pgid, Signal::SIGKILL);
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "haiku",
        all(target_os = "linux", not(target_env = "uclibc")),
    )))]
    {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = killpg(pgid, Signal::SIGTERM);
        let deadline = Instant::now() + Duration::from_millis(STOP_GRACE_MS);
        loop {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => return,
                Ok(None) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            std::thread::sleep(remaining.min(Duration::from_millis(20)));
        }
        let _ = killpg(pgid, Signal::SIGKILL);
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(not(unix))]
fn terminate_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn exit_outcome(status: ExitStatus) -> (i32, String) {
    if let Some(code) = status.code() {
        return (code, format!("exit {code}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(sig) = status.signal() {
            return (128_i32.saturating_add(sig), format!("signal {sig}"));
        }
    }
    (EXIT_REFUSED, "exit unknown".to_owned())
}

fn release_claim(client: &ApiClient, claim_id: &str, fence: u64, note: &str) {
    let path = format!("/v1/claims/{claim_id}/release");
    let body = json!({ "fence": fence, "note": note });
    match client.request("POST", &path, Some(&body)) {
        Ok((status, _)) if (200..300).contains(&status) => {}
        Ok((status, value)) => log::warn(
            "hold.release_refused",
            json!({ "claim": claim_id, "status": status, "response": value }),
        ),
        Err(error) => log::warn(
            "hold.release_failed",
            json!({ "claim": claim_id, "error": error }),
        ),
    }
}

fn write_stderr(value: &Value) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{value}");
}

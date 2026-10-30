//! Opt-in debug hold for failed microVM jobs: pure policy admission,
//! eligibility, hold end, single-use capability redemption, the debug step
//! plan and the connection instructions. The hosted shell owns the clock,
//! randomness, SQLite records and the guest session.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::protocol::{GuestStep, StepReport, guest_step};

/// Longest grace an operator may configure (three times Tangled's default).
pub const MAX_GRACE_SECONDS: u32 = 900;
/// Most VMs one runner may hold at once.
pub const MAX_HELD_VMS: u32 = 16;
/// Lifetime of one capability; it also never outlives its hold.
pub const CAPABILITY_TTL_MS: u64 = 120_000;
/// Capabilities issued for one hold.
pub const MAX_CAPABILITIES_PER_HOLD: u32 = 16;
/// Random capability bytes, presented as lowercase hex.
pub const CAPABILITY_BYTES: usize = 32;
/// Bytes of one debug command.
pub const MAX_DEBUG_COMMAND_BYTES: usize = 4096;
/// Output bytes kept per stream of one debug command.
pub const DEBUG_OUTPUT_BYTES: usize = 65_536;
/// Deadline of one debug command inside the guest.
pub const DEBUG_COMMAND_TIMEOUT_MS: u64 = 20_000;
/// BLAKE3 derive-key context for stored capability digests.
pub const CAPABILITY_CONTEXT: &str = "kiln.microvm.debug.capability.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugHoldPolicy {
    pub enabled: bool,
    pub grace_seconds: u32,
    pub max_concurrent: u32,
}

impl DebugHoldPolicy {
    pub fn grace_ms(&self) -> u64 {
        u64::from(self.grace_seconds) * 1000
    }
}

// r[impl kiln.hosted.microvm.debug_hold.policy]
// r[impl kiln.hosted.microvm.debug_hold.policy.scenario.bounds]
/// Bounded operator policy, checked even while disabled.
pub fn admit(policy: &DebugHoldPolicy, maximum_concurrent: u32) -> Result<(), String> {
    if !(1..=MAX_GRACE_SECONDS).contains(&policy.grace_seconds) {
        return Err(format!(
            "debug hold grace must be 1..={MAX_GRACE_SECONDS} seconds"
        ));
    }
    if !(1..=MAX_HELD_VMS).contains(&policy.max_concurrent)
        || policy.max_concurrent > maximum_concurrent
    {
        return Err(format!(
            "debug hold max_concurrent must be 1..={MAX_HELD_VMS} and within maximum_concurrent"
        ));
    }
    Ok(())
}

/// Extra unit and admission time a job may need for a hold; zero unless the
/// runner enables holds and the job could ever be held.
pub fn reserve_ms(policy: Option<&DebugHoldPolicy>, grants: Grants) -> u64 {
    match policy {
        Some(policy) if policy.enabled && grants.permit_hold() => policy.grace_ms(),
        _ => 0,
    }
}

/// Credential and trust facts known before the job runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub secrets: bool,
    pub workload_token: bool,
    pub untrusted: bool,
}

impl Grants {
    fn permit_hold(self) -> bool {
        !self.secrets && !self.workload_token && !self.untrusted
    }
}

/// Every fact the hold decision depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoldFacts {
    pub enabled: bool,
    pub failed: bool,
    pub grants: Grants,
    /// Holds currently active on this runner, excluding this job.
    pub held: u32,
    pub max_concurrent: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Disabled,
    Succeeded,
    Secrets,
    WorkloadToken,
    Untrusted,
    ConcurrencyLimit,
}

impl Refusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Succeeded => "succeeded",
            Self::Secrets => "secrets",
            Self::WorkloadToken => "workload_token",
            Self::Untrusted => "untrusted",
            Self::ConcurrencyLimit => "concurrency_limit",
        }
    }
}

// r[impl kiln.hosted.microvm.debug_hold.lifecycle]
/// The first refusal in a fixed order; there is no override.
pub fn eligibility(facts: &HoldFacts) -> Result<(), Refusal> {
    if !facts.enabled {
        Err(Refusal::Disabled)
    } else if !facts.failed {
        Err(Refusal::Succeeded)
    } else if facts.grants.secrets {
        Err(Refusal::Secrets)
    } else if facts.grants.workload_token {
        Err(Refusal::WorkloadToken)
    } else if facts.grants.untrusted {
        Err(Refusal::Untrusted)
    } else if facts.held >= facts.max_concurrent {
        Err(Refusal::ConcurrencyLimit)
    } else {
        Ok(())
    }
}

/// A completed guest session failed when the guest refused it, stopped
/// early, or any step exited non-zero, timed out or overflowed its output.
pub fn job_failed(steps: usize, reports: &[StepReport], admission_error: Option<&str>) -> bool {
    admission_error.is_some()
        || reports.len() < steps
        || reports.iter().any(|report| !step_succeeded(report))
}

pub fn step_succeeded(report: &StepReport) -> bool {
    report.exit_code == Some(0)
        && !report.timed_out
        && !report.stdout_truncated
        && !report.stderr_truncated
}

/// Signals the shell observes while holding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HoldSignals {
    pub vmm_exited: bool,
    pub guest_lost: bool,
    pub storage_failed: bool,
    pub cancelled: bool,
    pub released: bool,
    pub capacity_pressure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HoldEnd {
    VmmExited,
    GuestLost,
    StorageFailed,
    Cancelled,
    Released,
    CapacityPressure,
    Expired,
}

impl HoldEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VmmExited => "vmm_exited",
            Self::GuestLost => "guest_lost",
            Self::StorageFailed => "storage_failed",
            Self::Cancelled => "cancelled",
            Self::Released => "released",
            Self::CapacityPressure => "capacity_pressure",
            Self::Expired => "expired",
        }
    }
}

// r[impl kiln.hosted.microvm.debug_hold.end]
/// Whether the hold ends now, with fixed precedence. Expiry is checked last
/// but always applies, so no hold outlives its grace.
pub fn hold_end(now_ms: u64, expires_ms: u64, signals: HoldSignals) -> Option<HoldEnd> {
    if signals.vmm_exited {
        Some(HoldEnd::VmmExited)
    } else if signals.guest_lost {
        Some(HoldEnd::GuestLost)
    } else if signals.storage_failed {
        Some(HoldEnd::StorageFailed)
    } else if signals.cancelled {
        Some(HoldEnd::Cancelled)
    } else if signals.released {
        Some(HoldEnd::Released)
    } else if signals.capacity_pressure {
        Some(HoldEnd::CapacityPressure)
    } else if now_ms >= expires_ms {
        Some(HoldEnd::Expired)
    } else {
        None
    }
}

/// Stored digest of a presented capability; the capability itself is never stored.
pub fn capability_digest(capability: &str) -> String {
    blake3::derive_key(CAPABILITY_CONTEXT, capability.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A capability is the hex of exactly `CAPABILITY_BYTES` random bytes.
pub fn capability_well_formed(capability: &str) -> bool {
    capability.len() == CAPABILITY_BYTES * 2
        && capability
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Expiry of a capability issued at `now_ms` for a hold expiring at `hold_expires_ms`.
pub fn capability_expiry(now_ms: u64, hold_expires_ms: u64) -> u64 {
    now_ms
        .saturating_add(CAPABILITY_TTL_MS)
        .min(hold_expires_ms)
}

/// Issuance check for an authenticated write-access principal.
pub fn issue(
    now_ms: u64,
    hold_active: bool,
    hold_expires_ms: u64,
    write_access: bool,
    issued: u32,
) -> Result<u64, &'static str> {
    if !write_access {
        return Err("debug capability requires project write access");
    }
    if !hold_active || now_ms >= hold_expires_ms {
        return Err("no active debug hold for this job");
    }
    if issued >= MAX_CAPABILITIES_PER_HOLD {
        return Err("debug capability issuance bound reached for this hold");
    }
    Ok(capability_expiry(now_ms, hold_expires_ms))
}

/// Stored capability facts at redemption time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityRecord {
    pub peer: String,
    pub expires_ms: u64,
    pub used: bool,
}

// r[impl kiln.hosted.microvm.debug_hold.capability]
/// Redemption check: single use, unexpired, the issuing principal with
/// current write access, and a still-active hold.
pub fn redeem(
    record: Option<&CapabilityRecord>,
    peer: &str,
    write_access: bool,
    hold_active: bool,
    now_ms: u64,
) -> Result<(), &'static str> {
    if !write_access {
        return Err("debug capability requires project write access");
    }
    let Some(record) = record else {
        return Err("debug capability unknown");
    };
    if record.used {
        return Err("debug capability already used");
    }
    if record.peer != peer {
        return Err("debug capability was issued to another principal");
    }
    if now_ms >= record.expires_ms || !hold_active {
        return Err("debug capability expired with its hold");
    }
    Ok(())
}

/// A debug command is bounded, non-empty text without NUL.
pub fn admit_command(command: &str) -> Result<(), String> {
    if command.is_empty() || command.len() > MAX_DEBUG_COMMAND_BYTES || command.contains('\0') {
        return Err(format!(
            "debug command must be 1..={MAX_DEBUG_COMMAND_BYTES} bytes without NUL"
        ));
    }
    Ok(())
}

/// The one guest step a debug command becomes: the runner shell with bash
/// flags in `/work`, the base guest environment and the debug output bound.
pub fn debug_step(shell: &str, command: &str) -> Result<GuestStep, String> {
    admit_command(command)?;
    guest_step(
        shell,
        true,
        command,
        ".",
        &BTreeMap::new(),
        DEBUG_OUTPUT_BYTES,
    )
}

/// Deadline of one command: the fixed bound or the remaining hold.
pub fn command_deadline_ms(now_ms: u64, hold_expires_ms: u64) -> u64 {
    hold_expires_ms
        .saturating_sub(now_ms)
        .min(DEBUG_COMMAND_TIMEOUT_MS)
}

// r[impl kiln.hosted.microvm.debug_hold.sessions]
/// How to connect, appended to the job's log while the hold is active.
pub fn connection_instructions(project: &str, attempt: &str, job: &str, expires_ms: u64) -> String {
    format!(
        "debug hold: this failed job's microVM is held until unix_ms {expires_ms}. \
         With write access to project '{project}': \
         (1) `kiln-hosted rpc` action execution.debug_issue {{\"project\":\"{project}\",\"attempt_id\":\"{attempt}\",\"job\":\"{job}\"}} returns a single-use capability; \
         (2) execution.debug_exec {{\"project\":\"{project}\",\"attempt_id\":\"{attempt}\",\"job\":\"{job}\",\"capability\":CAPABILITY,\"command\":COMMAND}} runs one command in /work over vsock; \
         (3) execution.debug_release with the same project, attempt_id and job ends the hold early."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(grace_seconds: u32, max_concurrent: u32) -> DebugHoldPolicy {
        DebugHoldPolicy {
            enabled: true,
            grace_seconds,
            max_concurrent,
        }
    }

    fn eligible() -> HoldFacts {
        HoldFacts {
            enabled: true,
            failed: true,
            grants: Grants::default(),
            held: 0,
            max_concurrent: 1,
        }
    }

    fn report(exit_code: Option<i32>) -> StepReport {
        StepReport {
            index: 0,
            exit_code,
            signal: None,
            timed_out: false,
            stdout: String::new(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            elapsed_ms: 1,
        }
    }

    // r[verify kiln.hosted.microvm.debug_hold.policy]
    // r[verify kiln.hosted.microvm.debug_hold.policy.scenario.bounds]
    #[test]
    fn policy_bounds_are_named_and_enforced_even_when_disabled() {
        assert_eq!(admit(&policy(1, 1), 1), Ok(()));
        assert_eq!(admit(&policy(MAX_GRACE_SECONDS, MAX_HELD_VMS), 128), Ok(()));
        for (rejected, maximum) in [
            (policy(0, 1), 4),
            (policy(MAX_GRACE_SECONDS + 1, 1), 4),
            (policy(60, 0), 4),
            (policy(60, MAX_HELD_VMS + 1), 128),
            (policy(60, 3), 2),
            (
                DebugHoldPolicy {
                    enabled: false,
                    ..policy(0, 1)
                },
                4,
            ),
        ] {
            assert!(admit(&rejected, maximum).is_err(), "{rejected:?}");
        }
        assert_eq!(reserve_ms(None, Grants::default()), 0);
        let disabled = DebugHoldPolicy {
            enabled: false,
            ..policy(60, 1)
        };
        assert_eq!(reserve_ms(Some(&disabled), Grants::default()), 0);
        assert_eq!(reserve_ms(Some(&policy(60, 1)), Grants::default()), 60_000);
        for grants in [
            Grants {
                secrets: true,
                ..Grants::default()
            },
            Grants {
                workload_token: true,
                ..Grants::default()
            },
            Grants {
                untrusted: true,
                ..Grants::default()
            },
        ] {
            assert_eq!(reserve_ms(Some(&policy(60, 1)), grants), 0, "{grants:?}");
        }
        let json = serde_json::json!({"enabled": true, "grace_seconds": 1, "max_concurrent": 1, "extra": 1});
        assert!(serde_json::from_value::<DebugHoldPolicy>(json).is_err());
    }

    // r[verify kiln.hosted.microvm.debug_hold.lifecycle]
    #[test]
    fn eligibility_refuses_each_fact_in_fixed_order_without_override() {
        assert_eq!(eligibility(&eligible()), Ok(()));
        let cases = [
            (
                HoldFacts {
                    enabled: false,
                    ..eligible()
                },
                Refusal::Disabled,
            ),
            (
                HoldFacts {
                    failed: false,
                    ..eligible()
                },
                Refusal::Succeeded,
            ),
            (
                HoldFacts {
                    grants: Grants {
                        secrets: true,
                        ..Grants::default()
                    },
                    ..eligible()
                },
                Refusal::Secrets,
            ),
            (
                HoldFacts {
                    grants: Grants {
                        workload_token: true,
                        ..Grants::default()
                    },
                    ..eligible()
                },
                Refusal::WorkloadToken,
            ),
            (
                HoldFacts {
                    grants: Grants {
                        untrusted: true,
                        ..Grants::default()
                    },
                    ..eligible()
                },
                Refusal::Untrusted,
            ),
            (
                HoldFacts {
                    held: 1,
                    ..eligible()
                },
                Refusal::ConcurrencyLimit,
            ),
            (
                HoldFacts {
                    enabled: false,
                    failed: false,
                    grants: Grants {
                        secrets: true,
                        workload_token: true,
                        untrusted: true,
                    },
                    held: 9,
                    max_concurrent: 1,
                },
                Refusal::Disabled,
            ),
        ];
        for (facts, refusal) in cases {
            assert_eq!(eligibility(&facts), Err(refusal), "{facts:?}");
        }
    }

    #[test]
    fn job_failure_covers_exit_timeout_truncation_refusal_and_early_stop() {
        assert!(!job_failed(1, &[report(Some(0))], None));
        assert!(job_failed(1, &[report(Some(3))], None));
        assert!(job_failed(1, &[report(None)], None));
        assert!(job_failed(2, &[report(Some(0))], None));
        assert!(job_failed(1, &[], Some("refused")));
        let mut timed_out = report(Some(0));
        timed_out.timed_out = true;
        assert!(job_failed(1, &[timed_out], None));
        let mut truncated = report(Some(0));
        truncated.stderr_truncated = true;
        assert!(job_failed(1, &[truncated], None));
    }

    // r[verify kiln.hosted.microvm.debug_hold.end]
    // r[verify kiln.hosted.microvm.debug_hold.end.scenario.expiry]
    #[test]
    fn hold_end_has_fixed_precedence_and_always_expires() {
        let quiet = HoldSignals::default();
        assert_eq!(hold_end(99, 100, quiet), None);
        assert_eq!(hold_end(100, 100, quiet), Some(HoldEnd::Expired));
        assert_eq!(hold_end(u64::MAX, 100, quiet), Some(HoldEnd::Expired));
        let all = HoldSignals {
            vmm_exited: true,
            guest_lost: true,
            storage_failed: true,
            cancelled: true,
            released: true,
            capacity_pressure: true,
        };
        assert_eq!(hold_end(0, 100, all), Some(HoldEnd::VmmExited));
        for (signals, end) in [
            (
                HoldSignals {
                    guest_lost: true,
                    cancelled: true,
                    ..quiet
                },
                HoldEnd::GuestLost,
            ),
            (
                HoldSignals {
                    storage_failed: true,
                    cancelled: true,
                    ..quiet
                },
                HoldEnd::StorageFailed,
            ),
            (
                HoldSignals {
                    cancelled: true,
                    released: true,
                    ..quiet
                },
                HoldEnd::Cancelled,
            ),
            (
                HoldSignals {
                    released: true,
                    capacity_pressure: true,
                    ..quiet
                },
                HoldEnd::Released,
            ),
            (
                HoldSignals {
                    capacity_pressure: true,
                    ..quiet
                },
                HoldEnd::CapacityPressure,
            ),
        ] {
            assert_eq!(hold_end(0, 100, signals), Some(end));
        }
    }

    // r[verify kiln.hosted.microvm.debug_hold.capability]
    // r[verify kiln.hosted.microvm.debug_hold.capability.scenario.single_use]
    // r[verify kiln.hosted.microvm.debug_hold.capability.scenario.write_access]
    #[test]
    fn capabilities_are_single_use_bound_to_the_principal_and_expire_with_the_hold() {
        assert_eq!(
            issue(10, true, 1_000_000, true, 0),
            Ok(10 + CAPABILITY_TTL_MS)
        );
        assert_eq!(
            issue(10, true, 500, true, 0),
            Ok(500),
            "never outlives the hold"
        );
        assert!(issue(10, true, 500, false, 0).is_err(), "write access");
        assert!(issue(10, false, 500, true, 0).is_err(), "ended hold");
        assert!(issue(500, true, 500, true, 0).is_err(), "expired hold");
        assert!(issue(10, true, 500, true, MAX_CAPABILITIES_PER_HOLD).is_err());

        let fresh = CapabilityRecord {
            peer: "alice".into(),
            expires_ms: 100,
            used: false,
        };
        assert_eq!(redeem(Some(&fresh), "alice", true, true, 99), Ok(()));
        let used = CapabilityRecord {
            used: true,
            ..fresh.clone()
        };
        assert!(
            redeem(Some(&used), "alice", true, true, 1).is_err(),
            "second use"
        );
        assert!(
            redeem(Some(&fresh), "alice", true, true, 100).is_err(),
            "expiry"
        );
        assert!(
            redeem(Some(&fresh), "alice", true, false, 1).is_err(),
            "hold ended"
        );
        assert!(
            redeem(Some(&fresh), "mallory", true, true, 1).is_err(),
            "principal"
        );
        assert!(
            redeem(Some(&fresh), "alice", false, true, 1).is_err(),
            "write access"
        );
        assert!(redeem(None, "alice", true, true, 1).is_err(), "unknown");

        let token = "ab".repeat(CAPABILITY_BYTES);
        assert!(capability_well_formed(&token));
        assert!(!capability_well_formed(&"AB".repeat(CAPABILITY_BYTES)));
        assert!(!capability_well_formed("ab"));
        let digest = capability_digest(&token);
        assert_eq!(digest.len(), 64);
        assert_ne!(digest, token);
        assert_eq!(digest, capability_digest(&token));
        assert_ne!(digest, capability_digest(&"cd".repeat(CAPABILITY_BYTES)));
    }

    // r[verify kiln.hosted.microvm.debug_hold.relay]
    #[test]
    fn debug_steps_are_bounded_and_run_in_the_workspace_without_job_environment() {
        let step = debug_step("/bin/bash", "uname -a").unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(step.working_directory, "/work");
        assert_eq!(step.output_limit, DEBUG_OUTPUT_BYTES);
        assert_eq!(
            step.arguments,
            [
                "--noprofile",
                "--norc",
                "-e",
                "-o",
                "pipefail",
                "-c",
                "uname -a"
            ]
        );
        assert_eq!(
            step.environment.keys().collect::<Vec<_>>(),
            ["HOME", "LANG", "PATH", "TMPDIR"]
        );
        assert!(debug_step("/bin/bash", "").is_err());
        assert!(debug_step("/bin/bash", "a\0b").is_err());
        assert!(debug_step("/bin/bash", &"x".repeat(MAX_DEBUG_COMMAND_BYTES + 1)).is_err());
        assert_eq!(command_deadline_ms(0, u64::MAX), DEBUG_COMMAND_TIMEOUT_MS);
        assert_eq!(command_deadline_ms(90, 100), 10);
        assert_eq!(command_deadline_ms(200, 100), 0);
        let text = connection_instructions("alpha", "a-1", "check[0]", 42);
        assert!(text.contains("execution.debug_issue") && text.contains("unix_ms 42"));
        assert!(text.contains("\"job\":\"check[0]\""));
    }
}

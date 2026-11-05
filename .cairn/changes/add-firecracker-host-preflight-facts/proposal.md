## Why

Issue #28 starts with host preflight facts. The Firecracker path has no pure
representation of host SMT/KSM, kernel CVE status, MPTCP exposure or resctrl
availability, so an untrusted-capable runner cannot make an explicit admission
decision from observed facts.

## What Changes

- Add pure `HostFacts` and `CveStatus` models in `kiln-microvm`.
- Classify CVE-2026-98432 from a checked-in upstream fixed-release table plus
  explicitly reviewed vendor package backports.
- Refuse untrusted admission when the CVE is vulnerable or unknown, SMT is
  active, KSM is running, or listeners remain after MPTCP is disabled.
- Permit refusal only with a non-empty operator override; the caller owns
  recording that override in its receipt.
- Retain SMT, KSM, MPTCP, listener and resctrl facts.

## Impact

- **Files**: `crates/kiln-microvm/src/host.rs`, `src/lib.rs`.
- **Testing**: fixed/vulnerable/vendor/unparseable release classification, SMT,
  KSM, override, and MPTCP-listener cases.
- **Authority**: pure admission only. Receipt wiring, host reads, scheduler
  placement, core partitioning and covert-channel measurement remain follow-up
  work.

## Why

The pure host-preflight facts added by the previous change were not read,
recorded or enforced by the Firecracker shell. An untrusted-capable runner could
still start on an unsafe host and receipts had no host facts.

## What Changes

- Read kernel release/package, SMT, KSM, MPTCP sysctls/listeners and resctrl
  mount state before a hosted execution plan is admitted.
- Record the facts as `vm.host` in every Firecracker attempt receipt.
- Refuse untrusted runs on vulnerable/unknown CVE status, active SMT, running
  KSM, or residual MPTCP listeners.
- Add an optional non-empty `host_preflight_override` to the reviewed
  Firecracker runner profile; the receipt retains the host facts and the
  override remains operator-owned.
- Extend the typed Nickel configuration and hosted documentation.

## Impact

- **Files**: `crates/kiln-hosted/src/execution.rs`,
  `crates/kiln-hosted/src/microvm.rs`, `config/hosted-service.ncl`,
  `config/hosted-execution.ncl`, `docs/hosted-ci.md`.
- **Testing**: host fact reads are shell-owned; focused refusal/override tests,
  hosted library tests and config checks.
- **Authority**: no hostile-isolation claim. The receipt records observations;
  trust-domain scheduling, partitioning and covert-channel measurement remain
  open.

## Why

Issue #15 asks for an opt-in debug hold for failed hosted microVM jobs. Today the per-job lifecycle in `kiln-microvm` goes `Running → Collected → Journaled → Destroying → Destroyed`, so a VM is destroyed as soon as its result is journaled. Failures that only reproduce inside the CI VM can then be debugged only from logs. Tangled's spindle keeps the microVM of a failed step alive for an operator-enabled grace period and prints how to connect; Kiln has no such path.

## What Changes

- Add an operator-level Firecracker runner option `debug_hold = { enabled, grace_seconds, max_concurrent }` to the hosted execution profile and its Nickel contracts. It is absent by default; when absent or disabled, the lifecycle transitions, unit limits, reservations and receipts are unchanged. Grace and concurrency have named bounds.
- Add a `Held` lifecycle state to `kiln-microvm`. It is reachable only from `Journaled`, only after a complete guest session whose job failed, and only when the job had no secret grant, no workload-token audience and was not untrusted (hostile trust, or any future untrusted marker from #12). A hold ends on grace expiry, explicit release, capacity pressure, attempt cancellation or VMM exit, and then proceeds to normal destruction, ending in `Destroyed` or `Abandoned`.
- Add a pure `kiln_microvm::debug_hold` core: policy admission, eligibility, hold end decision, single-use capability redemption, the debug step plan and the connection instructions.
- Extend the bounded vsock protocol with one `debug` request and `debug` report. The guest accepts debug commands only after its single run has failed, on the session the host already accepted. No listener is re-opened and the guest still has no network device.
- Add a host-side relay in the hosted shell. While a VM is held, the job thread relays debug requests from SQLite to the guest session. New customer actions `execution.debug_issue`, `execution.debug_exec`, `execution.debug_release` and `execution.debug_list` issue a short-lived, single-use capability to project principals with write access, redeem it for exactly one command, release the hold and list holds and sessions. Sessions are recorded (who, when, duration) and grant no other authority.
- `execution.get` and the final receipt end with the connection instructions for the hold.
- Document the option, the relay and its limits in `docs/hosted-ci.md`.

## Impact

- **Consumers:** hosted CI operators who enable the option on a Firecracker runner; project members with write access who debug a failed job.
- **Owners:** Kiln hosted-composition maintainers own the shell, the actions and the Nickel contracts; Kiln maintainers own `kiln-microvm` and the guest agent.
- **Affected:**
  - `crates/kiln-microvm` (`lifecycle`, `protocol`, new `debug_hold`, guest agent);
  - `crates/kiln-hosted` (`execution`, `microvm`, new `debug_hold` shell, host test);
  - `config/hosted-service.ncl`, `config/hosted-execution.ncl` and hosted fixtures;
  - `docs/hosted-ci.md`.

## Out of Scope

- No interactive PTY or SSH server. Each capability runs exactly one bounded, non-interactive shell command in the held guest.
- No network device, no new vsock listener and no guest-initiated connection.
- No hold for successful jobs, for jobs with secrets or workload tokens, for untrusted runs, or for namespace and QEMU runners. There is no per-run override.
- No change to journaled results, review, CI or source authority. Debug sessions are audit records only.
- No change to the web-service run console.

## Context

`crates/kiln-microvm/src/lifecycle.rs` is the pure per-job VM state machine. The shell reports observed events and performs the returned effect: `Planned → Starting → Ready → Running → Collected → Journaled → Destroyed`, with `Destroying` after a failed journal and `Abandoned` when teardown cannot be proven. `crates/kiln-hosted/src/microvm.rs` is the Firecracker shell:
- it accepts exactly one guest vsock connection and then removes the host listener;
- it sends one `run` frame, journals the result in `hosted_microvm_job` and stops the attempt's systemd unit.

`crates/kiln-hosted/src/execution.rs` owns admission:
- `check_capacity` bounds reserved, running and unknown attempts by `maximum_concurrent`;
- the admission maximum (`maximum_ms`) sizes the finance reservation and is capped at one hour;
- `credential_grants` marks jobs with secret or workload-audience grants;
- `trust` is `trusted` or `hostile`.

Customer requests run on forked service connections, so a second request can run while `execution.run` is still executing. One RPC exchange is bounded by `EXCHANGE_TIMEOUT_MS` (30 s). Project write access is the `owner`, `admin` and `member` role set used by `execution.run` and `execution.cancel`, with project scope checked by `accounts::require_project`.

## Goals and Success Contract

An operator can enable a bounded debug hold on a Firecracker runner. When a held-eligible job fails, its VM stays up after the result is journaled. A project principal with write access can then obtain a short-lived single-use capability and run exactly one command per capability inside that VM, over the existing vsock session. The hold always ends, and then the VM is destroyed or recorded `Abandoned`.

False completion includes:
- any change to transitions, unit limits, reservations or receipts when the option is absent or disabled;
- `Held` reachable before `Journaled`, for a succeeded job, for an infrastructure failure without a complete guest session, or for a job with secrets, a workload token or untrusted trust;
- a capability that works twice, after the hold or its own expiry, for another principal, or for a principal without write access;
- a hold that can outlive its grace, or that ends in any state other than `Destroyed` or `Abandoned`;
- a debug path that adds a network device, re-opens a vsock listener or lets the guest initiate a connection;
- a debug session that changes the journaled result or grants review, CI or source authority.

## Decisions

### Decision: Operator option on the Firecracker runner

**Choice:** `Backend::Firecracker.debug_hold: Option<DebugHoldPolicy>`, serde-default and omitted when absent, so existing profiles keep their configuration identity. `DebugHoldPolicy { enabled, grace_seconds, max_concurrent }` lives in the pure `kiln_microvm::debug_hold` module, with named bounds:
- `grace_seconds` in `1..=MAX_GRACE_SECONDS` (900);
- `max_concurrent` in `1..=MAX_HELD_VMS` (16), and never above the execution `maximum_concurrent`.

Rust admission and the Nickel contracts (`hosted-service.ncl`, `hosted-execution.ncl`) both enforce the bounds, with positive and negative fixtures.

### Decision: Held only after Journaled, decided by pure eligibility

**Choice:** `State::Held`, `Event::Hold(HoldFacts)`, `Event::HoldEnded` and `Effect::Hold`. `HoldFacts` is a plain-data input:
- `enabled`, `failed`, `secrets`, `workload_token` and `untrusted`;
- `held` (current holds on this runner) and `max_concurrent`.

`debug_hold::eligibility` returns the first refusal in a fixed order: disabled, succeeded, secrets, workload token, untrusted, concurrency limit. The lifecycle accepts `Hold` only in `Journaled`, only when the guest session completed (`GuestDone` was observed), and only when eligibility passes. Every other ordering is an illegal transition that leaves state unchanged.

`Held` accepts only `HoldEnded`, which moves to `Destroying { journaled: true }` with `Effect::Destroy`. Destruction then ends in `Destroyed` or `Abandoned` exactly as today. When the option is disabled the shell never sends `Hold`, so the transition sequence is byte-for-byte today's.

`failed` comes from the pure `debug_hold::job_failed`: an admission error, fewer reports than steps, or a report with a non-zero exit, a timeout or truncated output.

### Decision: Hold end is a pure decision over observed signals

**Choice:** `debug_hold::hold_end(now_ms, expires_ms, signals)` returns `Some(HoldEnd)` with this precedence: VMM exited, cancelled, released, capacity pressure, expired. The shell polls every `HOLD_POLL_MS` (50 ms).

Hold expiry is `started + grace`. The unit's `RuntimeMaxSec` gains the grace (plus `HOLD_UNIT_MARGIN_MS`) only for jobs that can be held, so a held VM is never killed by its own unit before the hold ends. If the unit does stop it, that is a VMM exit and the hold ends.

Admission adds the largest eligible runner grace once to the attempt's `maximum_ms`. At most one job per attempt can fail, because later jobs are skipped. The finance reservation and the one-hour boundary therefore cover the hold. A held VM stays inside its running attempt, so it keeps its `maximum_concurrent` slot.

**Capacity pressure:**
- when `check_capacity` denies an admission while some runner has debug hold enabled, the shell records the denial time in `hosted_capacity_pressure`;
- a hold that started earlier ends at its next poll;
- the denied caller retries after destruction.

### Decision: Relay over the existing vsock session

**Choice:** the guest agent keeps its accepted session open after `done`. If its one run failed, it accepts `debug { nonce, deadline_ms, step }` frames and answers `debug { nonce, report }`.

- **Guest refusal:** before a failed run, a `debug` frame gets `done` with an admission error.
- **Command plan:** the debug step is the pure `debug_hold::debug_step`. It uses the runner guest shell with bash flags, working directory `/work` and the base guest environment (no step or secret variables), and runs as the unprivileged guest user under the same `no_new_privs`, process-group, deadline and output bounds as job steps.
- **Bounds:** a command is at most `MAX_DEBUG_COMMAND_BYTES` (4096). Its deadline is the minimum of `DEBUG_COMMAND_TIMEOUT_MS` (20 s) and the remaining hold. Output is at most `DEBUG_OUTPUT_BYTES` (65,536) per stream.
- **Isolation:** the host never re-binds the listener, so the guest still cannot initiate any vsock connection, and the VM plan still has no network device.

**Shell relay:** while holding, the job thread is the only owner of the guest stream.
- **Mailbox:** it relays requests from `hosted_microvm_debug_session` rows (`requested → running → completed|failed`) and writes the bounded report back.
- **Waiting caller:** the customer request polls its row until a terminal state, or until `DEBUG_COMMAND_TIMEOUT_MS + DEBUG_RELAY_MARGIN_MS`. A compile-time assertion keeps that total below `EXCHANGE_TIMEOUT_MS`.
- **Hold end:** pending rows become `refused`.

### Decision: Single-use capability bound to a write-access principal

**Choice:** `execution.debug_issue { project, attempt_id, job }` requires write access, which is re-checked against durable membership on every call.
- **Token:** it returns a fresh 32-byte random capability as hex. Only its BLAKE3 derive-key digest (`kiln.microvm.debug.capability.v1`) is stored, bound to the organization, attempt, job and issuing peer.
- **Expiry:** the capability expires at the earlier of `issued + CAPABILITY_TTL_MS` (120 s) and the hold expiry.
- **Issuance bound:** at most `MAX_CAPABILITIES_PER_HOLD` (16) are issued per hold.

`execution.debug_exec { project, attempt_id, job, capability, command }` re-checks write access and applies the pure `debug_hold::redeem`:
- the capability is unused and unexpired;
- the presenting peer is the issuing peer;
- the hold is active.

It then marks the capability used with a conditional update, so exactly one concurrent caller wins. Redemption and the session row are written in one transaction. A second use is refused.

`execution.debug_release` requires write access. `execution.debug_list` (read roles) lists holds with their connection instructions and sessions.

### Decision: Records without authority

**Choice:**
- **Hold record:** `hosted_microvm_hold` keeps the runner, start, expiry, end reason, end time and connection instructions.
- **Session record:** each session row records the peer, request, start and end times and duration, the BLAKE3 of the command and the bounded result.
- **Immutable result:** the journaled `hosted_microvm_job.result` is written before `Hold` and never updated afterwards.
- **Receipt:** for enabled runners the receipt's `microvm.debug_hold` holds the outcome, end reason, held milliseconds, session count and instructions. Disabled runners omit the field.
- **No authority:** sessions touch no review, gate or source table.

### Decision: Untrusted marker

**Choice:** `untrusted = trust != "trusted"` today. Once #12 adds an untrusted run marker to hosted execution, it is OR-ed into the same fact; there is no override.

## Risks / Trade-offs

- **Billing:** hold time is attempt time and is billed as elapsed time inside the reservation that admission enlarged. Operators opt in.
- **No interactive shell:** each capability runs one non-interactive command. This keeps the relay bounded under the 30 s RPC exchange.
- **Mailbox latency:** the SQLite mailbox adds up to `HOLD_POLL_MS` latency in each direction, but it gives durable session records without a second socket.
- **Retained responses:** replay protection retains the `debug_issue` response, including the token, in the private service database until the request records rotate. The token is useless after use or expiry.
- **Older guests:** a guest built before this change closes the session after `done`, so debug commands fail and the hold ends with `guest_lost`.

## Verification Strategy

- **Pure tests (`kiln-microvm`):**
  - disabled holds give the exact old transition and effect sequences;
  - `Hold` is rejected before `Journaled`, without `GuestDone`, and for each refusal fact;
  - from `Held`, every event except `HoldEnded` is rejected, and `HoldEnded` always reaches `Destroyed` or `Abandoned`;
  - `hold_end` precedence and expiry;
  - capability redemption covers single use, expiry, the wrong principal, missing write access and an ended hold;
  - policy bounds;
  - protocol round trip and rejection of unbounded debug frames.
- **Shell tests (`kiln-hosted`):**
  - configuration identity is byte-identical with the option absent;
  - Rust admission bounds;
  - with a real SQLite connection and no VM: write-access issuance and refusal, single-use redemption, concurrent double redemption and expiry.
- **Nickel:** positive and negative fixtures.
- **Host test (KVM):** a failing job is held, one command runs through a capability, a second use is refused, a viewer is refused, and grace expiry destroys the VM. Timings are recorded.

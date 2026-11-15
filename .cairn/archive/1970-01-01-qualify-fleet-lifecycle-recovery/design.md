# Context

The fleet host invokes an operator-admitted executable with the single argument
`--kiln-fleet-v1`, a cleared environment, working directory `/`, bounded
stdin/stdout and its own process group. `Executable::verify` admits only an
immutable `/nix/store` path whose bytes match the configured BLAKE3. `mutate`
commits the instance row, the request row and, for provisioning, the finance
reservation before running the driver; the receipt update is a later write
guarded by the instance revision.

## Decisions

### Decision: qualify through the unchanged executable admission

The harness adds the example binary and a per-scenario bash wrapper
(`exec <binary> driver --inventory <dir> "$@"`) to the Nix store with
`nix-store --add`, then configures the operator profile with the wrapper path
and its BLAKE3. No test-only verification bypass exists. Added store paths are
unrooted and remain garbage-collectable.

### Decision: one coordinator process per action

Each account, finance or fleet action runs in a new coordinator process over
the scenario's SQLite file. Killing the coordinator while the driver waits at a
barrier therefore leaves exactly the committed pre-effect intent, the same
state as a crash after the driver replied but before the receipt was persisted.

### Decision: one-shot faults and barriers in the fake provider

The driver reads an optional fault matched by operation and instance and
consumes it atomically: `before_effect` and `after_effect_no_reply` exit without
a receipt; `hang_before_effect` and `hang_after_effect` publish a barrier and
wait, with a named poll interval and maximum wait, for a release or abort file;
`stale_reply` answers with another request identity; `duplicate_create` is a
control that records a second resource for one instance. Malformed argv or
input exits without a receipt.

### Decision: a pure inventory oracle

The oracle receives retained fleet rows, present provider records and the
operation log. It reports driver invocations, created, deleted and present
counts, rows by state and violations: `duplicate_create` (two creations or
records for one instance), `retired_while_present` (a retired row whose
provider resource is present), `untracked_resource` (a provider resource with
no row) and `capacity_exceeded` (more present resources than the admitted
maximum). Each scenario declares its expected violations exactly. The two
controls pass only when their violation is detected. `retired-reappearance`
expects `retired_while_present` because the reappeared resource is real; its
checks prove the row stays retired and cannot be reconciled or reprovisioned.
Every other scenario passes only with none.

### Decision: record the job-secret binding gap

`crates/kiln-hosted/src/secrets.rs` binds secret delivery to jobs, not to fleet
instance generations. The issue forbids adding a secret subsystem, so the gap is
recorded rather than closed.

## Verification

Example unit tests cover the oracle and fault admission with positive and
negative cases (`cargo test -p kiln-hosted --example fleet_qualification`).
Clippy and formatting run for all targets. One real harness run retains its
receipt under `evidence/`.

## Non-claims

The fake provider is not a cloud API. A passing receipt is not production
provider, capacity, availability or secret-binding qualification.

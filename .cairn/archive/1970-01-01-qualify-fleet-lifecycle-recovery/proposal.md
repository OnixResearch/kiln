# Why

Issue #72 asks for retained real-process qualification of the hosted fleet
driver and persistence boundary. `crates/kiln-hosted/src/fleet.rs` already
persists the request before the driver effect, reconciles through `inspect`,
enforces revisions and counts uncertain instances against capacity, but its
tests cover only configuration, capacity and timing. Nothing exercises an
interrupted coordinator, a lost acknowledgement, duplicate delivery, a stale
receipt, a changed operator generation, teardown or capacity conservation
through the real executable admission and SQLite state.

## What Changes

- Add `crates/kiln-hosted/examples/fleet_qualification.rs`, an example that is
  never shipped as a package binary. Its `driver` mode is a strict fail-closed
  fake provider with a durable inventory, an append-only operation log and
  one-shot fault and barrier points. Its `coordinator` mode performs exactly one
  real account, finance or fleet action per process. Its `run` mode admits the
  driver through the unchanged immutable-executable verification, runs every
  scenario with real processes and SQLite, kills the coordinator during driver
  effects, and writes a receipt.
- Add a pure inventory oracle that compares retained fleet rows with provider
  records and the operation log. Deliberate duplicate-create and
  premature-release controls must be detected.
- Document the procedure, the job-secret binding gap and non-claims in
  `docs/hosted-ci.md`.

## Impact

Qualification tooling, evidence and documentation. Production fleet behavior
changes only if the qualification exposes a defect. No cloud capacity is
purchased, no secret subsystem is added and receipts keep
`production_provider_qualified=false`.

## Authority

The operator requested completion of the open issues in this session. No
Radicle or GitHub mutation, no paid capacity and no deployment.

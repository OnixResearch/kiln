# Report implementation attempt outcomes and model turns

## Why

Kiln issue #90 needs an opt-in report joining worker terminal observations with
controller-owned verification, without promoting worker completion or usage into
admission authority. Unknown usage must not become zero.

## What Changes

- Add absent, complete and incomplete model-request counts to attempt usage.
- Retain worker terminal/profile/model observations and the independent verifier
  outcome on the existing exact attempt/fence record.
- Normalize supported trusted Lattice observations; the current OMP JSONL cohort
  has no measured model-request count, so its count remains not instrumented.
- Add a read-only windowed report with raw denominators, failure categories,
  turns and elapsed time per verified pass, and profile/model breakdowns.
- Document subscription cost units and preserve default-off metric receipts.

## Impact

- **Consumer**: operators diagnosing implementation attempts in `kiln-implementer`.
- **Owner**: Kiln maintainers own reporting/admission; Lattice owns worker usage
  measurement and telemetry qualification (Lattice #13).
- **Files**: implementation core, durable implementer, Lattice worker consumer,
  canonical Engine verification, implementer CLI, tests and runbook.
- **Repeatability**: deterministic positive/negative fixtures and actual CLI smoke.
- **Non-claims**: no dollar pricing, profile ranking, model execution attestation,
  live worker qualification, publication or remote mutation.

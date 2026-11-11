# Add canonical candidate verification

## Why

Issue #46 requires an implementation candidate to be admitted only after
independent acceptance, using retained canonical execution evidence rather than
worker claims. The implementation ledger had candidate handoff and verification
phases but no receipt or terminal verified state.

## What Changes

- Add `CandidateVerification` binding the candidate handoff, work/version,
  attempt/fence, candidate/tree/base, profile and canonical `CiFact`.
- Require a terminal successful clean full trusted run at the exact candidate
  revision with nonempty unique passing checks.
- Add a terminal `verified` attempt state and retain the receipt idempotently.
- Add `kiln-implementer verify --verification-file FILE`.
- Document that Kiln does not execute checks or create CI facts.

## Impact

- **Files**: `crates/kiln-core/src/implementation/verification.rs`,
  `attempt.rs`, implementer shell/CLI, docs and specification.
- **Testing**: exact idempotent verification, failed/partial/stale/empty-check
  refusal, terminal state and cleared current claim.
- **Authority**: admission of retained evidence only; no CI execution,
  publication or remote mutation.

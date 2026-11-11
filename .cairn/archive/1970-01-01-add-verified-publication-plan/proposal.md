# Add verified publication preparation

## Why

Issue #47 needs a durable connection between a verified candidate and its branch
and pull request while keeping preparation, publication and integration
distinct. The ledger had no publication state.

## What Changes

- Add a publication plan binding a terminal verified attempt, exact handoff and
  verification receipt, candidate/tree/base, deterministic branch and PR text.
- Add ordered preparation, branch-published and pull-request-published records
  with exact candidate-head checks.
- Keep remote publication external; the core performs no HTTP or Git effect.
- Document the boundary and add prepare/refusal tests.

## Impact

- **Files**: `crates/kiln-core/src/implementation/publication.rs`,
  `attempt.rs`, docs and specification.
- **Testing**: deterministic verified plan, arbitrary branch refusal, unverified
  attempt refusal, terminal state/branch/PR order.
- **Authority**: local preparation and observation recording only. No remote
  publication, merge or landing.

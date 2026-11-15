# Why

Issue #73 asks for a bounded, source-controlled HTTP contract for a finite
GET-only subset of the Kiln web service, exercised independently of the Rust
types that produce responses. Client drift between console and worker clients
is currently invisible.

## What Changes

- Inventory the GET-only family: project list, run list/summary, execution
  binding and artifact metadata, including authentication, status/error bodies,
  nullable/optional fields, limits, unknown outcomes and pagination.
- Commit a deterministic OpenAPI 3.0.x document at
  `contracts/http-api/openapi.json` with explicit version `kiln.http-api.v1`.
- Add independent HTTP golden and negative tests in
  `crates/kiln-web-service/tests/http_contract.rs` and
  `crates/kiln-web-service/tests/http-contract/`.
- Document the contract, compatibility matrix, bounded client behavior,
  lockstep-vs-independent-upgrade decision and Drift/Progenitor evaluation in
  `docs/http-api-contract.md`.
- Evaluate Drift and Progenitor outside the repository; record results in the
  contract documentation.

## Impact

Tests and documentation only for the first pilot. No mutating endpoint is added.
No workspace dependency changes. No server migration.

## Authority

Operator requested issue completion; this package is the pilot scope.

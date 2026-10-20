## Phase 1: Specification foundation

- [x] [serial] Write proposal, design, tasks, and library delta spec for production-readiness gates.
- [x] [depends:proposal] Record proposal/design/tasks gate transcript; `openspec_gate` was unavailable, so evidence captures the tooling blocker.

## Phase 2: Support-level reporting

- [x] [serial] Add operator-facing readiness docs and a deterministic readiness report/check covering controlled-integration, preview, and production-supported labels.
- [x] [depends:support-level-reporting] Wire the readiness check into the aggregate verification path so docs cannot claim unsupported production surfaces.

## Phase 3: Production integration profile

- [x] [depends:support-level-reporting] Add the first explicit production integration profile/example with caller-owned resolver, proof-store, revocation, key-custody, verification-time, limit, and caveat-policy wiring.
- [x] [depends:integration-profile] Add positive and negative fixtures proving resolver errors, proof-store errors/staleness, revocation denial/backend failure, replay attempts, and missing caveat policies fail closed.

## Phase 4: Per-surface promotion gates

- [x] [parallel] Define and test the remote DID-resolution promotion gate, including unsupported method and resolver-freshness failures.
- [x] [parallel] Define and test the proof-transport promotion gate, including missing, oversized, stale, cyclic, and wrong-reference proof failures.
- [x] [parallel] Define and test the registry-backed revocation promotion gate, including revoked leaf/proof, backend failure, stale registry evidence, and duplicate denial behavior.
- [x] [parallel] Define and test the replay/persistence promotion gate, including duplicate invocation receipts, restart recovery, and storage-corruption/freshness failures.
- [x] [parallel] Define and test the expanded key-algorithm promotion gate, including alg/header mismatch, wrong-key-type, malformed-signature, and unsupported-algorithm failures.
- [x] [parallel] Define and test the broader caveat-vocabulary promotion gate, including unknown caveats, malformed payloads, clock/counter provenance, and policy rejection.

## Phase 5: Release evidence

- [x] [depends:promotion-gates] Update README/API/boundary docs so support limits, readiness labels, and verified-boundary claims agree.
- [x] [depends:docs] Run `nix flake check`, targeted readiness checks, OpenSpec validation, and whitespace checks; capture evidence before any support label is promoted.

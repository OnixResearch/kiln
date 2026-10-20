## Context

The repository currently has a clean, pushed `main` and `nix flake check /home/brittonr/git/ucan -L` passes. The supported public surface is intentionally narrow: Ed25519 `did:key` compact-token issuance/verification, explicit proof collections/stores, explicit revocation checker injection, bounded proof traversal, signer abstraction, typed caveat subset, and verified finite bridge predicates. Docs also state that remote DID resolution, network proof fetching, registry revocation, replay stores, persistence, non-Ed25519 key types, and arbitrary caveat semantics are out of scope.

## Goals / Non-Goals

**Goals:**

- Prevent broad production claims from being inferred from green local checks alone.
- Define support levels that can be shown in docs and machine-readable readiness output.
- Specify the first production integration profile as explicit adapter wiring around the existing library, not as hidden global behavior.
- Require negative evidence for resolver/proof/revocation/replay/caveat/key-type failure modes before promotion.

**Non-Goals:**

- Do not implement every backend in this change package.
- Do not make network, storage, ambient clocks, registry lookup, or key custody implicit defaults in the core verifier.
- Do not widen the formally verified claim beyond the finite vendored predicates already documented.
- Do not promote non-Ed25519 or arbitrary caveat support without separate implementation evidence.

## Decisions

### 1. Support labels are evidence-backed, not marketing prose

**Choice:** Use three labels: `controlled-integration`, `preview`, and `production-supported`.

**Rationale:** The current crate is strong for controlled integrations, but production platform status requires deployed integration evidence and negative tests.

**Alternative:** A single `production ready` boolean was rejected because it would either understate the current usable library surface or overstate the missing platform surfaces.

**Implementation:** Add docs plus a deterministic readiness report/check that names each surface, its label, evidence artifacts, blockers, and unsupported boundaries.

### 2. Integration profile remains explicit-shell wiring

**Choice:** The first production profile must compose caller-owned `KeyResolver`, `ProofStore`, `RevocationChecker`, key custody, explicit verification time, limits, and caveat policies through public APIs.

**Rationale:** This preserves the FCIS boundary and avoids silently adding ambient network/storage/clock behavior.

**Alternative:** Built-in global HTTP/IPFS/DID/revocation defaults were rejected because they blur trust boundaries and make backend truth/freshness claims invisible.

**Implementation:** Add an example/profile with deterministic fixtures and optional adapters. The profile must show positive operation and fail-closed behavior for resolver errors, stale proof material, registry revocation, replayed invocation, and missing caveat policy.

### 3. Promotion gates are per-surface

**Choice:** Remote DID resolution, proof transport, revocation registries, replay/persistence, expanded algorithms, and broader caveat vocabularies each require their own evidence before promotion.

**Rationale:** These seams have different threat models and should be drainable independently.

**Alternative:** A single umbrella implementation task was rejected because it would hide which production blockers remain.

**Implementation:** Keep this OpenSpec as the readiness gate; later implementation changes may satisfy one or more gates and update the aggregate readiness report.

## Risks / Trade-offs

**Overly strict labels delay adoption** → Mitigate by clearly naming `controlled-integration` as a positive supported state.

**Example backends become mistaken for normative infrastructure** → Mitigate by marking adapters optional and requiring backend-truth/freshness boundaries in docs and tests.

**Readiness report drifts from docs** → Mitigate with a deterministic consistency check wired into the normal verification gate.

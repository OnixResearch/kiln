# Design: Publish Radicle CI status

## Context

Kiln maps Radicle triggers and dispatches workflows through Lattice. Run outcomes stop at Kiln's journal.

Radicle records CI results in `xyz.radicle.ci-job` collaborative objects under the target repository. Writing there needs a signing key, an expected-head policy, and strict non-authority boundaries.

## Goal and Success Contract

The goal is a versioned, read-only-toward-Kiln status projection with explicit secret authority.

Completion requires these results:

- the cohort pins exact SQLite-free Radicle crates, the SecretSpec CLI, their revisions, licenses, and one platform;
- a typed Nickel status profile admits declared RIDs, the `xyz.radicle.ci-job` type, bounds, and non-claims;
- the signer resolves through one SecretSpec declaration and fails closed when absent;
- each write binds one Kiln attempt, one idempotency key, one RID, and one COB identity;
- the single-writer preflight rejects heads that Kiln did not write;
- ambiguous writes stay unknown until reconciliation succeeds; and
- native fixtures created by the pinned cohort prove repeatable success and every rejection class.

False completion includes ambient key lookup, raw key material in configuration or logs, multi-writer acceptance, status that mutates canonical state, or fixture-only evidence.

## Decisions

### Decision: Resolve the signer through SecretSpec

**Choice:** The typed profile names one SecretSpec declaration: project root, project, profile, secret key, and optional provider restriction. The shell invokes the pinned SecretSpec CLI through Bounded Exec immediately before dispatch.

The CLI uses a cleared environment plus fixed time, input, output, polling, and teardown bounds. The shell verifies its BLAKE3 identity before every invocation.

Missing declarations, unresolved secrets, and provider failures deny the effect before any signing. Resolved key bytes stay in memory, are used for signing, and are dropped without logging. Tests must prove redaction.

**Rationale:** The operator picks where the key lives (OS keyring, file, age, Vault, environment) by editing SecretSpec configuration, not Kiln code. One declarative contract covers development, CI, and production.

### Decision: Single-writer expected heads

**Choice:** The profile enables single-writer mode. The preflight reads current COB heads and accepts exactly two shapes: no prior head (first write) or one head authored by the resolved Kiln signer.

Any other head count or author rejects the plan before signing. A non-idempotent update must also match the exact planned head.

The Git reference mutation uses an atomic expected-target comparison. A race after preflight cannot overwrite a newer same-signer head.

**Rationale:** Single-writer removes merge ambiguity while Radicle collaboration stays out of scope. A later change can widen this policy deliberately.

### Decision: Allowlists bind every target

**Choice:** The profile lists admitted RIDs and admits only `xyz.radicle.ci-job`. Requests outside both allowlists fail before repository access.

**Rationale:** Explicit allowlists prevent a compromised trigger from redirecting status writes to another repository or object type.

### Decision: Status is rebuildable and idempotent

**Choice:** Each write binds the run ID, run outcome, idempotency key, RID, and native object identity. Repeating one key with equal facts produces the same observation. Different facts under one key fail closed.

A first-write plan omits the object ID because Radicle derives it from the signed root change. Kiln returns the derived ID and can discover it later through a bounded key scan.

Any write ambiguity yields unknown and requires reconciliation, never redispatch.

**Rationale:** The accepted `kiln.radicle.status` requirement forbids canonical-state mutation. Native object identity and idempotency make replay safe after crashes.

## Functional Core and Imperative Shell

Pure functions own: profile admission, allowlist checks, plan construction, expected-head comparison, idempotency decisions, and outcome classification.

The shell owns: SecretSpec resolution, key handling, storage handles, COB reads and writes, clocks, and provider error capture.

## Validation

Positive fixtures cover first-write creation, repeatable rewrites of one key, and reconciliation of a recorded write.

Negative fixtures cover wrong RID, unlisted COB type, stale or foreign head, missing SecretSpec declaration, unresolved secret, oversized payload, and secret leakage in logs or errors.

## Risks and Trade-offs

- The `radicle-cob` API can drift; exact pins and cohort checks make drift visible.
- The full `radicle` runtime conflicts with Kiln's SQLite cohort, so this change uses only the SQLite-free COB crates.
- SecretSpec runs as a pinned adapter process; core and standalone stay independent.
- Single-writer cannot represent external collaborators' updates; those reject instead of merging.

## Claim Boundary

Passing checks prove bounded local COB writes under the pinned profile. They do not prove peer convergence, actor trust, remote availability, CI correctness, or that any node serves the written object.

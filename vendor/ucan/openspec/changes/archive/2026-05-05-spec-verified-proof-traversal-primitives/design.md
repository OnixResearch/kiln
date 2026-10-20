## Context

Current UCAN proof verification is intentionally split: the `ucan` shell parses compact tokens, resolves keys and proofs through caller-injected traits, verifies signatures, applies revocation checks, tracks traversal state, folds effective delegation, and maps structured errors. Vendored `verified-logic` provides exact finite helpers for smaller semantic facts, but not yet a complete traversal model.

## Goals / Non-Goals

**Goals:**
- Specify the next verified-logic primitive family needed for proof traversal.
- Keep each primitive finite, deterministic, and data-only.
- Make the shell bridge consume already-resolved, already-parsed structural data or an explicit traversal transcript.
- Preserve fail-closed behavior for parser, resolver, proof-store, revocation, and crypto failures.

**Non-Goals:**
- No compact-token parsing or canonical signing-input verification.
- No network, filesystem, database, resolver, proof-store, revocation registry, replay store, signing, or ambient-clock behavior inside verified primitives.
- No arbitrary caveat-policy verification; opaque caller caveats remain shell-local unless callers opt into typed caveat helpers.
- No claim that runtime crypto/hash/codec internals are formally verified.

## Decisions

### 1. Transcript-first bridge boundary

**Choice:** Future UCAN-side bridge code should prefer a shell-produced finite traversal transcript: visited proof references, resolved entries, parent-child links, budget counters, revocation facts, effective-delegation fold steps, and final authorization target.

**Rationale:** The shell can keep all I/O and parsing while verified logic checks that the observed finite trace is internally consistent, bounded, and semantically valid.

**Alternative:** Verify the shell resolver/traversal loop directly. Rejected because resolver/proof-store/crypto/parser behavior is caller-injected or runtime-only and would blur the trusted boundary.

### 2. Split primitives by proof shape

**Choice:** Model traversal as composable primitives rather than a single broad authorizer:
- proof graph well-formedness
- bounded traversal/cycle rejection
- effective delegation folding
- proof-chain authorization
- revocation-aware chain validity
- canonical transcript consistency

**Rationale:** Small primitives can be proved and consumed incrementally, and the UCAN shell can route exact matching checks without overclaiming.

### 3. Revocation is caller-supplied fact data

**Choice:** Verified primitives may consume revocation references and caller-supplied revoked/not-revoked facts, but MUST NOT query revocation backends.

**Rationale:** Current revocation semantics are caller-injected and fail closed on backend errors. Verified logic can prove denial from finite facts but cannot prove backend freshness or availability.

## Risks / Trade-offs

**Overbroad primitive scope** → Mitigate by keeping each primitive finite and excluding parser/resolver/backend behavior.

**Mismatch between transcript and shell execution** → Mitigate with transcript consistency checks plus UCAN regression tests that produce transcripts from the real traversal path.

**Caveat semantics ambiguity** → Mitigate by requiring opaque caveat preservation/equality only, unless the caller opts into typed caveat primitive semantics.

## Validation Plan

- Validate this OpenSpec strictly.
- Future verified-logic implementation must include Verus proof evidence for the primitive family.
- Future UCAN bridge work must update `scripts/check-ucan-bridge.sh`, focused traversal tests, full cargo checks, OpenSpec validation, and Nix flake checks.

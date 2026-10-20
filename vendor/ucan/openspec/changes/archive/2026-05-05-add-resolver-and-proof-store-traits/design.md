## Context

Current verification takes in-memory `KeyResolutionContext` and `ProofCollection` values. That is deterministic but not extensible enough for reusable crates that store keys and proofs in caches, databases, request envelopes, or blockstores.

## Goals / Non-Goals

**Goals:** introduce explicit resolver/store traits, keep in-memory adapters, bound recursive verification, and preserve deterministic tests.

**Non-Goals:** built-in HTTP/IPFS/database fetchers, ambient global lookup, or async runtime commitment in the first pass.

## Decisions

### 1. Use sync shell traits first

**Choice:** Add synchronous `KeyResolver` and `ProofStore` traits in the std shell.

**Rationale:** The current verifier is synchronous and most tests/adapters can be deterministic. Async can be layered later behind a feature or separate API once a concrete fetcher exists.

**Alternative:** Start async-first. Rejected because it would force runtime choices before any network fetcher exists.

**Implementation:** Keep `KeyResolutionContext` and `ProofCollection` as in-memory implementations or adapters.

### 2. Resolve direct proofs by reference during traversal

**Choice:** Verification reads proof references from the token and asks the proof store for each referenced token as needed.

**Rationale:** This matches real storage/caching shapes and avoids making callers pre-flatten full chains.

**Alternative:** Continue requiring transitive proof vectors. Rejected for the resolver API because it repeats traversal logic downstream.

**Implementation:** Preserve existing cycle detection and move reference-to-token lookup behind the proof store.

### 3. Add explicit verification limits

**Choice:** Add a `VerificationLimits` value covering proof depth, proof count, and token size where the verifier can enforce them.

**Rationale:** Resolver-based recursion needs bounded resource use, especially before async/network fetchers exist.

**Alternative:** Hard-code limits. Rejected because reusable crates need different threat models; defaults can exist but should be visible.

**Implementation:** Wire limits through `VerificationContext` or a verifier builder and produce structured limit errors.

## Risks / Trade-offs

**Trait error types may get complex** → Start with adapter errors mapped into `TokenError` while preserving enough source detail for callers.

**Backwards compatibility** → Provide constructors that wrap existing key/proof collections so current examples keep working during migration.

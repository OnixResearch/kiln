## Context

Today `crates/ucan-core` validates only a tiny capability-document format, while the repo-root `ucan` crate exposes separate wrappers around verified capability containment, temporal checks, `did:key` encoding, and Ed25519 signing/verification. That architecture already has the right FCIS seam, but the pieces are not assembled into the thing consumers actually want: a library that can issue, parse, verify, and authorize UCAN delegation tokens.

The design needs to preserve the existing boundary:

- `ucan-core` stays `#![no_std]` and owns deterministic claim-shape parsing plus structural validation.
- `ucan` stays the `std` shell and owns compact-token codecs, clocks, key material, proof loading, and orchestration.
- UCAN semantics keep flowing through `vendor/verified-logic` instead of being reimplemented locally.

## Goals / Non-Goals

**Goals**
- Give consumers a stable, end-to-end API for issuing, parsing, verifying, and authorizing UCAN tokens.
- Keep deterministic claim-structure parsing and field validation in `ucan-core` while the shell owns compact-token serialization.
- Support the minimum useful interoperability surface first: `did:key` issuers backed by Ed25519 keys and delegation proof chains supplied by the caller.
- Return verified outputs that expose effective capabilities and effective time windows instead of leaving callers to recompute them.
- Replace primitive-heavy public seams with named UCAN concepts and staged verification results so invalid wiring and invalid workflow states become harder to represent.

**Non-Goals**
- Add network DID resolution or built-in remote proof fetching.
- Support every DID method, key type, or UCAN feature in the first change.
- Add storage, revocation registries, or CLI tooling.
- Move verified UCAN logic out of `vendor/verified-logic` into ad-hoc local implementations.

## Decisions

### 1. Introduce first-class token claim types in `ucan-core`

**Choice:** Add pure token and capability-set data structures plus structural validation helpers to `crates/ucan-core`, and make the shell build on those types instead of a bespoke `resource=/ability=` document alone.

**Rationale:** A usable library needs a stable internal model for issuer, audience, capabilities, proofs, and temporal bounds. Putting the deterministic shape rules in the core keeps the FCIS seam intact and gives tests a pure target.

**Alternative:** Keeping the core limited to standalone capability documents was rejected because the shell would then need to invent its own token-claim validation logic, which would blur the core/shell boundary and make the public API harder to reason about.

**Implementation:** Define owned token-claim structs plus borrowed validation views in `ucan-core`, keep parsing/validation pure, and let `ucan` convert validated core values into `verified-logic` inputs. Ability validation must stay aligned with `verified-logic`: `*` remains valid only as a full path segment, never inside another segment.

### 2. Make the shell API token-oriented, not primitive-oriented

**Choice:** Add consumer-facing shell entry points for building, signing, parsing, and verifying token strings/bytes, while keeping low-level primitives available as supporting APIs rather than the main workflow.

**Rationale:** Consumers should not need to manually sequence `sign_ed25519`, `verify_ed25519_signature`, `validate_token_envelope`, and capability checks. A usable library needs a coherent workflow surface.

**Alternative:** Documenting the existing helper sequence was rejected because it still leaves every consumer to rebuild the same orchestration and error handling.

**Implementation:** Add token builder/parser/verifier types or functions in `src/` that accept validated core claims, sign and verify them with explicit inputs, and return verified results with effective delegation data.

### 3. Ship compact token strings first and keep the codec in the shell

**Choice:** The first release will issue and consume a compact `header.payload.signature` token string aligned with JWT/JWS-style UCAN transport and carrying the initial claim set `iss`, `aud`, `att`, `prf`, `exp`, and `nbf`, while keeping the claim model codec-neutral underneath.

**Rationale:** The library needs a real wire format to be usable, but pushing base64url and JSON dependencies into `ucan-core` would complicate the `#![no_std]` boundary for little benefit. Keeping the codec in the shell makes the user-facing flow concrete without burdening the pure core.

**Alternative:** Leaving the wire format unspecified was rejected because the tasks and API surface would otherwise make an implicit choice later. Putting compact-token decoding in `ucan-core` was rejected for the first iteration because the shell can own the codec while the core still owns deterministic claim validation.

**Implementation:** `ucan` owns compact-token encode/decode plus any supporting dependencies such as base64url and JSON crates. `ucan-core` remains dependency-light and validates the decoded claim values it receives from the shell.

### 4. Treat proof traversal, clocks, and key resolution as explicit shell inputs

**Choice:** Require callers to provide proof material, the verification time, and any needed key-resolution context through explicit parameters or traits.

**Rationale:** Proof stores, clocks, and DID resolution are imperative concerns. Making them explicit keeps the core pure, keeps tests deterministic, and avoids hidden ambient state.

**Alternative:** Hidden globals, filesystem lookups, or network calls were rejected because they would make verification non-deterministic and violate the FCIS shell boundary.

**Implementation:** Model proof lookup and time inputs in the shell API, then compose them with `verified-logic` helpers for signature checks, temporal intersection, capability attenuation, and the existing self-delegation rejection.

### 5. Reuse `verified-logic` for semantics and keep local code focused on adaptation

**Choice:** Use local code to translate validated token structures into the vendored verified primitives, then derive effective authorization results from those verified checks.

**Rationale:** The repo already depends on `verified-logic` for capability attenuation, time checks, signature primitives, and token envelope validation. Reusing that source of truth reduces semantic drift.

**Alternative:** Reimplementing attenuation, temporal narrowing, or DID semantics directly in the shell was rejected because it would create two UCAN logic engines that could disagree.

**Implementation:** Keep `ucan-core` responsible for syntax/shape validation only, and keep `ucan` responsible for bridging into `verified-logic` modules such as `ucan_token`, `ucan_chain::ChainEntry`, `ucan_ability`, `ucan_temporal`, and `ucan_signature`.

### 6. Ship docs and examples as part of the usable-library bar

**Choice:** Treat a runnable README/example flow as part of the change instead of optional polish.

**Rationale:** “Usable library” means a new consumer can succeed from the public surface without reverse-engineering internal modules.

**Alternative:** Deferring docs/examples until after the API lands was rejected because it would leave the new surface underspecified and harder to review.

**Implementation:** Add at least one end-to-end consumer example and keep README guidance aligned with the actual supported algorithms, proof model, and current limitations.

### 7. Use named semantic types instead of primitive-heavy public seams

**Choice:** Introduce dedicated UCAN domain types for concepts such as issuer DID, audience DID, compact token strings, proof collections, capability collections, and temporal bounds instead of exposing those roles primarily as `String`, `Vec<_>`, and `u64` parameters.

**Rationale:** This follows the “one behavior, one type” discipline from evermeta. The current primitive-heavy surface makes it too easy to swap issuer and audience, confuse proof material with token bytes, or lose the meaning of a timestamp at the call boundary.

**Alternative:** Keeping raw primitives plus documentation was rejected because documentation does not make bad wiring unrepresentable and does not give the team a stable shared vocabulary for the core UCAN concepts.

**Implementation:** Add owned and borrowed wrappers at the core/shell seam where they buy correctness, keep them zero-cost where possible, and make translations explicit at the boundary to `verified-logic` inputs.

### 8. Model verification as a staged pipeline

**Choice:** Represent parsing, structural validation, signature verification, and authorization as staged result types or typestate-style wrappers instead of one monolithic verifier that accepts and returns mostly untyped data.

**Rationale:** This applies the “state machines the compiler checks” idea from evermeta. Callers should not be able to accidentally authorize undecoded or unsigned material just because every stage currently accepts the same primitive inputs.

**Alternative:** A single verifier with booleans or loosely structured intermediate values was rejected because invalid workflow states remain representable and because it makes both API review and testing less legible.

**Implementation:** Expose a pipeline shaped like `DecodedToken -> StructurallyValidToken -> SignatureVerifiedToken -> AuthorizedToken`, with `StructurallyValidToken` owned by the `ucan-core` validation boundary and the later stages owned by the `ucan` shell via explicit moves across the seam. The shell error surface must keep temporal, attenuation, self-delegation, and signature failures distinct so callers do not lose the reason verification failed.

### 9. Put the architectural standard in the toolchain

**Choice:** Add deterministic verification rails that fail when the FCIS boundary drifts, when semantic checks bypass `vendor/verified-logic`, or when tests stop exercising the intended public workflow surface.

**Rationale:** This follows evermeta’s “the standard is the toolchain” and “one idiom, one verifier” rules. The no-std boundary and verified-logic bridge are core to this change; if they live only in prose, they will erode.

**Alternative:** Relying on code review and contributor discipline alone was rejected because the whole point of this change is to make the library’s usability and correctness story structurally durable.

**Implementation:** Add rails that catch `std`/codec dependency creep in `ucan-core`, bridge-audit checks that confirm semantic routing through `verified-logic`, and test helpers that keep public API coverage readable instead of calcifying around giant shared fixtures.

## Verification Strategy

- `ucan-core` gets pure unit coverage for positive and negative claim validation, including required-field failures, malformed capability sets, and wildcard-segment alignment with `verified-logic`.
- `ucan` gets integration coverage for the public workflow: compact-token round-trip, signature verification, malformed compact tokens, missing proofs, self-delegation rejection, expired parents, and over-broad child delegations.
- Prefer runtime-generated tokens in integration tests so the verification path exercises the same builder/signing surface that consumers use; add targeted tampering of serialized tokens to cover negative cases.
- Build token/proof test helpers as builders so each positive and negative case declares only the preconditions it needs instead of inheriting a general fixture with hidden coupling.
- Keep at least one README/example flow buildable as part of verification so docs cannot drift away from the public API.
- Structure README/example onboarding as architecture snapshot first, end-to-end flow second, and proof-chain deep dive third so the public surface teaches consumers in the same order they need to understand it.
- Add a dedicated no-std verification rail for `ucan-core` plus a deterministic bridge audit that fails if attenuation, token-time, or signature semantics stop routing through `vendor/verified-logic`, or if `ucan-core` picks up shell-only concerns such as `std`, JSON, or compact-token codec dependencies.
- When new files are added, stage them with `git add -N` before Crane-based checks, keep `cargo fmt`/`cargo clippy` scoped to `ucan` and `ucan-core`, and preserve the existing `.gitignore` unignore for `vendor/verified-logic/src/result.rs`.

## Risks / Trade-offs

**Compact token strings may not cover every UCAN ecosystem variant** → Mitigate by keeping the validated claim model separate from the initial shell codec so additional codecs can be added later.

**Proof-chain verification can sprawl if it tries to solve transport/storage too** → Mitigate by requiring proof material as explicit shell inputs and leaving network/storage concerns out of scope.

**The first usable surface may still be narrower than the full UCAN ecosystem** → Mitigate by documenting the initial support matrix clearly: `did:key`, Ed25519, explicit proof inputs, and delegation flows first.

**Integration may duplicate semantics accidentally** → Mitigate by treating local code as adapters around `verified-logic`, with tests that compare high-level outcomes to the underlying verified helpers.

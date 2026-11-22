## Why

Kiln #75's capability, Seqair and Cairn sections and #76's capability and Cairn-workspace sections require that every source view be restricted by the holder's current authority, even when read-only. Today `AdmittedPartialView` is the only view: it is the trusted resolver's complete manifest, and nothing derives an audience-restricted result from it. There is no source-authority vocabulary (Basalt's operations are implementation-only), no opaque handle or scoped cursor, no uniform "unavailable" shape for probes, no restricted Cairn context, and no rule that makes sharing a view an attenuated delegation instead of a copied link. #69 and #70 own authority; this change adds their source-view vocabulary and the pure disclosure and sharing contracts that the routes (#76), projected workspaces (#77–#79) and Lattice scoped context consume.

## What Changes

- A closed source-view vocabulary, `kiln.source-authority-coverage.v1`: discover, read content, read history, prepare workspace, modify source, create source, delegate and retrieve result, one-to-one with the planned Basalt `kiln/source/...` abilities. Submission and publication reuse the protected implementation operations. No operation implies another.
- A `SourceAuthorityRequest` and `SourceAuthority` port mirroring the implementation port, an `EffectiveSourceScope` built only by an adapter from a verified session, per-component path scope with a subtree marker, snapshot rules with explicit live-reference evolution, and the explicit `LocalSourceAuthority` full-source compatibility profile with no role path.
- `restrict`, which filters the trusted view under `selection ∩ ceiling ∩ snapshot ∩ current grant` before anything is counted, paged or reported, into an `AudienceView` that cannot be deserialized or constructed elsewhere. Handles and cursors are BLAKE3 hashes keyed by a shell-held key over a `ViewBinding` that binds source, definition, manifest, ceiling, audience, scope and generations; every use rechecks current authority. Missing and forbidden objects share one `PublicUnavailable` shape; discoverable paths distinguish present, unrequested, absent, unknown and truncated.
- `RestrictedContext`, which discloses Cairn items through `.cairn/changes/<id>/**` path authority (owner decision O4), shows a hidden prerequisite only as an opaque handle and status, binds the audience's view binding and the exact observation, and cannot become a complete `CairnObservation` (compile-fail).
- `ViewCeiling::from_definition` and `admit_share`, which derive an attenuated `SharePlan` no broader than the sharer's current scope and the frozen ceiling in path, operation, snapshot, lifetime and depth, and require the `delegate` ability.
- Typed Nickel contracts for path scopes, grants and share requests, negative share fixtures, and a shared restricted corpus with maintainer, project-reader and agent views, a hidden dependency, equal-byte files, an alias and a hidden Cairn prerequisite.

## Capabilities

### Modified Capabilities

- `source-projection`: audience-restricted views, keyed handles and cursors, restricted Cairn context, the source-view authority vocabulary and port, and attenuated sharing.

## Impact

New `kiln_core::projection::{authority, disclosure, sharing}` modules with tests and a fake verified port for tests. `config/source-projection-contracts.ncl` gains the source-authority contracts; new fixtures under `config/fixtures/` and `fixtures/source-projection/restricted/`. No route, worker, adapter or persisted state changes.

## Non-goals

Wiring the routes, caches and Cairn workspace (#76, S5), issuing delegations through UCAN, the Basalt `kiln://source/v1` adapter (Basalt B2), workspace and writeback admission (S7), durable revocation (S6) and direct-download profiles (#80). Rust 1.88 versus Kiln-owned stores (owner decision O2) is untouched: nothing here persists authority state.

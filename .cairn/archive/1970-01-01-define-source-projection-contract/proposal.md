## Why

Issue [#75](https://github.com/OnixResearch/kiln/issues/75): Kiln has path-scoped monorepo CI but no common contract for several source namespaces over one exact repository snapshot. A saved query alone cannot bind the files an editor, worker or verifier actually sees. Repository identity also depends on the host-local checkout path: `ProjectProfile::repository_key` hashes the canonical `repository_path`, so moving a cache changes identity.

The browser, materializer, build-qualification and mapped-write work in #76–#80 needs one shared vocabulary first: a stable repository identity, a typed projection definition, a frozen manifest with complete path-to-origin mappings, and types that keep a partial view distinct from a complete workspace.

## What Changes

- Add `kiln_core::projection`, a pure `no_std + alloc` module:
  - **Repository identity.** An enrollment binds a Kiln repository handle to the source provider and provider-native repository reference. Its BLAKE3 identity covers only that provider binding. Local and remote locators are separate bindings, so two cache locations share one identity and equal directory names never merge repositories. One handle with two provider bindings is refused unless an explicit reconciliation names both; one provider binding under two handles is refused.
  - **Projection definitions.** `navigation`, `runnable` and `writable` profiles; selection reusing monorepo scope admission and glob semantics; mount layout rules; declared dependencies; writable-only creation destinations; required source capabilities; generated-input policy; and a target filesystem profile. Admission canonicalizes unordered declarations and computes a BLAKE3 definition identity, never from configuration text.
  - **Resolution.** A pure resolver turns supplied exact facts into a `ResolvedProjectionManifest`. It binds the repository identity, exact revision, existing `canonical_tree`, definition, evaluator and adapter versions, target profile, coverage, dependency bindings and every destination's native or generated origin. Unsafe paths, collisions, unsupported capabilities and bound violations become sorted diagnostics before any effect.
  - **Type states.** `AdmittedPartialView` is constructible only by the resolver. `CompleteProjection` is constructible only by a completeness check for a declared build or write operation. Neither deserializes. Serialized manifests can be checked for internal consistency and staleness, but that check never yields an admitted view.
  - **Compatibility.** An explicit conversion turns an admitted monorepo `ProjectScope` into an equivalent labeled projection definition.
- Add a typed Nickel contract for projection definitions with positive and negative fixtures, and a committed export consumed by the Rust tests.

## Impact

- **Consumer and adoption path:** #76 project source views, #77 build-input qualification, #78 materialization and #79 mapped writes build on this vocabulary. Existing monorepo projects enter through the compatibility conversion. The admission authority in #69/#70 and Basalt #3 supplies current access when those consumers apply effects.
- **Immediate outcome:** two projections over one snapshot have reproducible, independently verifiable manifests, and repository identity survives cache moves.
- **Durable capability:** one pure contract and identity vocabulary with diagnostics, type states and fixtures.
- **Owner:** Kiln core maintainers.
- **Files:** `crates/kiln-core/src/{projection.rs,projection/tests.rs,lib.rs}`, `config/source-projection-contracts.ncl`, `config/fixtures/*source-projection*.ncl`, `fixtures/source-projection/definition.json`, `scripts/check-nickel-fixtures.sh`, `docs/operator.md`, `cairn-policy/development-coverage.ncl`, `CHANGELOG.md` and this package.
- **Verification:** focused kiln-core unit tests and doctests, including `compile_fail` cases; Clippy and rustfmt; the Nickel fixture gate; Cairn gates, validation and active-delta traceability.

## Out of Scope

Authority evaluation, grants, effective read/write labels, audience-restricted results, opaque handles, disclosure-equivalent responses, query caches and cursors (#69/#70, Basalt #3, #76). Materialization (#78), writeback (#79) and graph-authoritative storage (#80). Web-service or console wiring. Migrating `ProjectProfile::repository_key`, which feed URNs and automation head keys depend on. A manifest embeds no grants, tokens or proofs, and visibility never implies access.

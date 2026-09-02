# Design: Cut Kiln status storage over to the shared Radicle COB Git store

## Context and Success Contract

Kiln currently binds the git2 0.21.0 COB repository with an ephemeral commit configuration in `status/shell.rs`; `status/storage.rs` owns the borrowed `CobRepository`, the two Radicle storage traits, a BTreeSet-based bounded distinct-object scan, and a caller-supplied expected-target ref update. Patch review mutation calls the same storage. `status/cob.rs` evaluates the CI-only payload and `status/plan.rs` owns pure admission. The existing shared store v0.1.1 is an immutable **0.20** source baseline (`f78168ea4560ce3a3167b5850bc0c1568e1a1abe`, Git archive BLAKE3 `bee9d0616208aba318212f624e5b85a14894c2e9f3f43fee5b7312e21acaa8b0`); it is not a valid Kiln dependency until a separate v0.2.0 adaptation and pin exist.

Success means both status and patch review use that one shared v0.2.0 implementation for Git COB traits and refs, with the same Kiln status and patch decisions and diagnostics, while core and unrelated provider cohorts remain unchanged. Native tests must prove both working and failing local scenarios. r[kiln.radicle.status_store_boundary] r[kiln.radicle.status_store_fixtures]

## Decisions

### Exact local source acquisition

The shared repo is currently local-only and `publish = false`; no remote URL is assumed. After the v0.2.0 implementation and its tests are committed, record its *actual* immutable source commit and BLAKE3 of `git archive --format=tar <commit>` in Kiln's vendor source record. A Kiln-owned import script takes an explicit source checkout and recorded commit, checks the commit/archive digest, imports an in-repository archive under `vendor/radicle-cob-git-store`, preserves notices/licenses, and verifies every imported byte. It refuses an altered existing vendor tree instead of replacing it; before a future pin bump, run the old importer to verify the old tree against its declared prior archive and digest. Cargo uses the **vendored path**, not `../radicle-cob-git-store`; the path is durable because the bytes and their originating source identity are committed in Kiln. The old 0.20 v0.1.1 pin remains unchanged for its own consumers. r[kiln.radicle.status_store_pin]

Use v0.2.0 as the source API/version break; pin Radicle COB 0.21.1, crypto 0.19.0, oid 0.2.2, ref-format 0.2.0, and git2 0.21.0 in the shared v0.2.0 manifest and resolve Kiln's adapter to that cohort. Do not replace Kiln's separate git2 0.20.2 dependency or move Radicle imports to core.

Keep v0.2.0 focused on namespace refs, signed author/type validation, exact expected-head CAS, and streaming bounded discovery. The v0.1.1 `RefLayout::RadLegacy` compatibility path remains at its immutable 0.20 consumer pin; do not carry that version-specific branch into v0.2.0 without a concrete 0.21 consumer and tests. Document both separate immutable pins and do not describe v0.2.0 as interchangeable with 0.20 consumers.

### One shared storage implementation and a borrowed shell handle

The shared v0.2.0 store accepts a borrowed `&git2::Repository` without cloning/reopening it. Kiln retains the opened `Repository` and ephemeral Git config until the entire operation ends. A scoped store value carries the *exact* expected ref target for updates. Product-neutral ref spelling, signed namespace/manifest/author validation, object/change trait implementations, and publication happen only in the shared mechanism. `status/storage.rs` is deleted; Kiln's status and mutation shells retain only their own diagnostic translators, with no duplicate trait implementations, ref walk, or CAS. `status/cob.rs` changes its evaluator binding to the shared store.

Kiln continues to authorize RID/type, resolve SecretSpec, enforce one signer and one head, compare the plan's expected head in the pure admission path, own idempotency keys and the `xyz.radicle.ci-job` payload, and construct provider observations. Shared store does not load Kiln secrets, generate operator keys, create Radicle repos, or publish remotely.

### Exact CAS and bounded replay discovery

Shared update compares the stored exact expected target with the observed current target and performs `git2::Repository::reference_matching` using that expected target; an ancestry-only check is insufficient. First-write uses no-overwrite ref creation. Kiln's initial stale-head admission remains ahead of change creation. If the ref moves after Kiln preflight but before publication, the post-store `Refs` error remains `StatusDispatch::Unknown` with the existing object ID when known; reconciliation is read-only and never redispatches. Do not reclassify that ambiguity as an ordinary failure or a successful write. r[kiln.radicle.status_store_concurrency]

Shared `object_ids_bounded(type,max)` rejects zero, iterates refs without first invoking unbounded `types`, parses and counts *distinct* IDs, stops at overflow, and returns deterministic ordered IDs. Duplicate namespaces for one ID do not spend multiple object slots. Kiln still applies single-writer validation to every discovered object's heads before matching its idempotency key. Shared signed-ref validation must reject malformed namespaces, invalid head signatures and wrong signed authors while Kiln maps shared typed failures to its stable provider diagnostics; avoid silently treating malformed or unreadable refs as absent. Bounds cover distinct IDs, not the total number of refs, elapsed time, or a Git snapshot. r[kiln.radicle.status_store_scan]

### Native fixtures and classification

Preserve the existing nine `status_native` fixture cases: create/replay/reconcile, exact update, foreign head, stale expected head, conflicting replay, missing object, unresolved secret/redaction, scan bound, and ambiguous ref publication. Add status native assertions for malformed refs and signed-author mismatch with stable diagnostic codes. The shared-store fixtures cover two authenticated namespace refs to the same object under a one-object bound, zero bound, tampered signature and symbolic ref, and exact CAS against a ref that moves to another valid descendant. Kiln's native patch-review fixtures prove the same cutover does not bypass its signer or review policy. Use only isolated bare repositories and fixture seeds. Keep explicit negative diagnostic-code assertions and `Unknown` observations; no hosted repo, operator signing key, push or network. Run focused baseline/cutover tests, pure admission tests and the Kiln workspace checks once integration is complete. r[kiln.radicle.status_store_fixtures]

## Risks and Controls

- The shared v0.1.1 `update` accepts a current ref whose target is a parent of the new change; that does **not** enforce Kiln's caller-supplied exact expected target. v0.2.0 must have the exact-match mode and a racing-ref regression before Kiln switches.
- The existing shared `types` builds an unbounded map. It must not replace Kiln's bounded discovery; migrate to the streaming bounded API first.
- Stricter shared signed-ref validation can change failure categories. Review typed shared errors against Kiln's stable `radicle_ref_*`, `radicle_object_scan_bound`, `status_foreign_head`, `status_multi_head`, `status_stale_head`, and unknown classifications, and prove precedence with fixtures.
- Local-only source identity is not a hosted release. Bind a real v0.2.0 commit and archive digest after it exists; do not reuse the v0.1.1 digest or invent a remote. r[kiln.radicle.status_store_pin]

## Non-Claims

The pinned source, local signature validation, CAS fixture, and bounded discovery do not prove actor trust, arbitrary Git concurrency, filesystem snapshots, peer convergence, remote visibility, CI correctness, or release readiness. This change does not grant Radicle initialization, signing, push, sync, or hosted-ref mutation authority. r[kiln.radicle.status_store_nonclaims]

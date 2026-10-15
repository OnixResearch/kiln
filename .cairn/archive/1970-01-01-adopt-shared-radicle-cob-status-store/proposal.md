# Proposal: Adopt the shared Radicle COB Git store for Kiln status

## Why

`kiln-adapter-radicle` currently implements Radicle change/object storage traits and ref updates in `src/status/storage.rs`; both status publication and patch review mutation call that implementation. The independent `onix-radicle-cob-git-store` owns that product-neutral local Git mechanism, but its existing 0.20 cohort cannot implement Kiln's pinned 0.21.1 traits. Keeping two storage implementations makes bounded scans, signed refs, and exact ref updates harder to maintain consistently.

The shared repository's immutable 0.20 v0.1.1 baseline is commit `f78168ea4560ce3a3167b5850bc0c1568e1a1abe` with Git archive BLAKE3 `bee9d0616208aba318212f624e5b85a14894c2e9f3f43fee5b7312e21acaa8b0`. This identifies the baseline, **not** the future 0.21 adaptation. The shared source has no declared publication remote; Kiln must not depend on a mutable sibling checkout.

## What Changes

- Adopt a separately versioned shared store v0.2.0 adapted to Kiln's already pinned Radicle COB 0.21.1, crypto 0.19.0, and git2 0.21.0 cohort. Record its actual source commit and exact archive BLAKE3 only after that source is committed; import a checked, commit-pinned vendored archive rather than a sibling-path or floating dependency. r[kiln.radicle.status_store_pin]
- Cut over `kiln-adapter-radicle/src/status/storage.rs` and its patch-review transport callers from custom storage-trait implementations to the shared mechanism, retaining only Kiln-specific diagnostic translation and the necessary borrowed repository binding for the temporary commit identity. Keep reducers, patch permissions, SecretSpec resolution, RID admission, and status authority in Kiln. r[kiln.radicle.status_store_boundary]
- Preserve positive, early-failing, distinct-object discovery bounds, exact expected-head compare-and-swap, first-write no-overwrite, single-writer admission, and stable unknown/reconciliation behavior on ambiguous reference publication. r[kiln.radicle.status_store_concurrency] r[kiln.radicle.status_store_scan]
- Exercise real local Git/COB fixtures with test-only keys, including both success and negative paths, and record narrow non-claims. r[kiln.radicle.status_store_fixtures] r[kiln.radicle.status_store_nonclaims]

## Impact

- **Adapter**: only `kiln-adapter-radicle` gains the shared storage dependency. No Radicle type or git2 cohort moves into core, standalone builds, or other provider adapters.
- **Packaging**: an exact source commit, archive digest, licensed source snapshot, import recipe, and Cargo lock entry establish the reproducible vendor identity. The old 0.20 source pin remains available to its consumers; this change does not migrate Cairn or Valence.
- **Behavior**: status remains a bounded, single-writer projection of Kiln CI state and patch mutation remains a declared single-writer review action; no actor authorization, provider contract, or operator signing policy is transferred to shared storage.

## Out of Scope

- Radicle initialization, operator-key signing, push, sync, announcement, remote publication, or hosted-ref mutation.
- Changing Kiln's unrelated git2 0.20.2 source cohort, upgrading Cairn/Valence, or making the shared crate a general CI/status authority.
- Treating passing local fixture tests as proof of distributed convergence, remote visibility, filesystem snapshots, actor trust, or release readiness.

## Affected Specs

- `radicle-status`: immutable shared-store source admission, provider boundary, bounded discovery, exact CAS, local conformance, and non-claims; existing accepted CI status semantics remain authoritative.

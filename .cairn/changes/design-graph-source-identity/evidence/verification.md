# Graph source verification

Issue: <https://github.com/OnixResearch/kiln/issues/81>.

## Delivery and ownership

The immediate result is `kiln_core::source_graph`, a pure, optional `no_std`
model of graph-backed source identity, plus a bounded prototype. The durable
capability is the contract that later graph consumers build on: minted logical
identities, versioned relations with distinct outcomes, an exact import ledger,
deterministic exports, one authority mode per repository and evidence binding.
Kiln maintainers own it. The adoption path is #79 (mapped projected writes) and
#80 (durable manifest storage); no runtime, store or provider uses the model
yet, and native-source repositories never depend on it.

## Exercised checks

Commands ran from this checkout through `nix develop path:$PWD -c`.

| Check | Observed result |
| --- | --- |
| `cairn gate proposal|design|tasks design-graph-source-identity ...` | PASS before implementation |
| `cargo clippy --locked -p onix-kiln-core --lib -- -D warnings` | PASS on the first build of the three modules |
| `cargo test --locked -p onix-kiln-core source_graph` | 15 passed |
| `cargo test --locked -p onix-kiln-core --test source_graph_prototype` | 1 passed |
| `cargo test --locked -p onix-kiln-core` | 357 unit, 1 prototype, 1 doctest and 4 compile-fail doctests passed |
| `cargo clippy --locked -p onix-kiln-core --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --locked -p onix-kiln-core --target wasm32-unknown-unknown` | PASS, `no_std` |
| Scoped `rustfmt --check --edition 2024 --config skip_children=true` | PASS for the six touched Rust files |

The unit fixtures pair positive and negative cases: distinct identities for
equal bytes from different operations and mint-not-path derivation; edit and
rename continuity; aliases, copies with provenance and package forks; the
membership, alias and deletion outcomes; pinned dependencies; snapshot
reproduction and refusal of forged revisions, identities, roots, schemas and
relations; name and identity admission; layout admission; ambiguous continuity
and explicit mappings; unsupported, generated, unmapped, duplicate and invalid
entries; a full export/import round trip with a local projected edit, alias
contradiction, preserved deletions and path conflicts; graph- and
native-authoritative publication, reconciliation and migration; and evidence
invalidation for each bound identity. Every refused operation is checked to
leave the snapshot root unchanged.

## Deliberate choices

- A graph-level layout (non-nesting package mounts plus generated entries) has
  its own identity; together with the export record identity it is the
  projection identity for graph sources. #75 manifests remain native-source
  projections.
- Imports return a new graph and record; the input graph never changes. A
  native mode change of an existing path is an edit, not a kind change, because
  only file entries are admitted.
- Minted paths outside the first import of the mounted packages are recorded as
  unresolved continuity; their candidates are that import's removed
  memberships, recorded once instead of per pair. Entities left with no
  membership after an import are deleted with a tombstone.
- `AuthorityState` changes only through its native constructor, publications,
  recorded imports, reconciliations and migrations, so graph authority always
  follows an explicit migration. A reconciliation names the observed native
  revision that the next declared export replaces.

## Boundaries

- No store, persistence adapter, provider adapter, Git import shell or console
  surface exists; facts are synthetic and supplied by tests.
- The capability-restricted graph surfaces in #81 remain open and depend on
  #69/#70 and the restricted disclosure contract of #75.
- BLAKE3 identities detect inconsistency, not authorship; a writer that
  recomputes every identity passes snapshot verification.
- No repository was converted, mirrored or published. Full workspace tests,
  Octet, release-readiness and flake checks were not run.

## Lifecycle

Native `cairn sync design-graph-source-identity ... --execute` added the four
`kiln.source.graph.*` requirements to `.cairn/specs/graph-source/spec.md`; no
archive. The task gate and `cairn validate` passed afterwards. Repository-wide
traceability coverage still exits 1 with the same 71 unrelated dangling IDs and
the two missing GitHub landing/source-status anchors recorded for #90 and #91;
no `kiln.source.graph` requirement or scenario is dangling or missing.

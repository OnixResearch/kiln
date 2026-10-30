## Context

- `radicle` 0.25.1 on crates.io reports git revision
  `71a91042d96bb994c4cd4311d85f5cd1a6311b4e`, the revision the Radicle issues
  cite, and declares `MIT OR Apache-2.0`.
- Adding it to the workspace fails dependency resolution: it depends on `sqlite`
  v0.37, whose `sqlite3-sys` links the native `sqlite3` library already linked by
  rusqlite's `libsqlite3-sys`. Cargo permits one `links` value per graph.
- The published crate also fails to compile for two reasons upstream fixed in
  `e6fe5207`: a borrow-checking failure from an `action @` binding in
  `cob/identity.rs`, and an indexmap default-feature assumption.
- Heartwood's canonical computation lives in `git::canonical` and depends only on
  `radicle_oid`, `radicle_git_ref_format`, `radicle_crypto` (for the DID wrapper),
  `git2`, `thiserror`, `log`, `fast-glob`, `indexmap`, `nonempty` and `serde`.
  None of those links sqlite.

## Goals and Success Contract

The canonical computation is available to Kiln as ordinary Rust with exact
provenance, without linking the node crate. The upstream algorithm files are
byte-identical to the pinned revision except for two removed test modules; every
derived file digest is recorded and checked by the importer. A repository check
builds and tests the crate.

## Decisions

### Decision: vendor only `git::canonical` and its DID/delegates needs

**Choice:** import the nine canonical files and `identity/did.rs`; hand-write five
shim files (`lib.rs`, `git.rs`, `git/raw.rs`, `identity.rs`, `identity/doc.rs`)
that reproduce upstream module paths and carry the `Delegates`/`Threshold` and
`refs::branch` pieces those files use.

**Rationale:** pulling the whole crate would drag the sqlite-dependent node,
storage and COB cache. The shim surface is small, reviewed here, and separated
from the algorithm files so provenance stays legible.

### Decision: remove two fixture-dependent test modules, keep the rest

**Choice:** remove `canonical.rs`'s inline test module and `rules.rs`'s `mod test;`
(they import node storage and `radicle-cob` fixtures), and `symbolic.rs`'s inline
test module (its exact-order deserialization needs `serde_json`'s
`preserve_order`, which unifies globally and changes JSON map ordering for every
workspace crate); retain the quorum, reference-protection and DID tests.

**Rationale:** the retained tests exercise the quorum path Kiln uses. The storage
and order-dependent suites need cohorts or global features this crate must not
impose on Kiln, and the record names each removal.

### Decision: always-validating `refname!`

**Choice:** the shim defines its own `refname!` that validates in every profile
instead of enabling upstream's `macro` feature, which skips validation outside
debug assertions.

**Rationale:** the retained symbolic tests rely on validation; a release-profile
rail would otherwise accept invalid literals and change behaviour.

### Decision: separate workspace and a dedicated check

**Choice:** the crate declares its own `[workspace]` and its own lock file; the
flake gains a `heartwood-canonical` check that builds and tests it.

**Rationale:** the crate is not a Kiln member, so workspace formatting/lint rails
would otherwise neither cover nor be tripped by upstream bytes. The dedicated
check keeps upstream tests in the rail.

## Risks / Trade-offs

- The shims are Kiln-maintained code that must track upstream path changes; the
  importer fails if the pinned files move or their test-module anchors change.
- Removing the storage-fixture tests means upstream's identity-document and
  storage integration is not exercised here; the canonical algorithm itself is.
- The crate is vendored, so upstream fixes after `71a91042` require a recorded
  refresh, not a version bump.
- No Kiln crate consumes the module yet; the named consumer is the canonical
  source mode of #19.

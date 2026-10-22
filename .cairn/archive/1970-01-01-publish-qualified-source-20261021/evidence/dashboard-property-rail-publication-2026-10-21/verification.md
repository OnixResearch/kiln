# Verification

## Current tip published

The operator requested `commit and push` after the tip evidence publication of `ac9916c3`.
The local commit `141ec4d211c021a87da441070a5a6b3cc2378d76`, which carries the operator
dashboard and its property-based terminal rail, was pushed to canonical Radicle `main` and to
the read-only GitHub projection. Both were fast-forwards; no force push, history rewrite,
governance, delegate, threshold, visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `141ec4d211c021a87da441070a5a6b3cc2378d76` |
| Previous `main` on both remotes | `ac9916c3fe7ddbb051f012f84252117f295b65fd` |
| Commits above previous `main` | 3 |
| Source archive | BLAKE3 `ae80aa687eefcc065f7972185d07a1a041d53ec6bd6fe85312c15d1e7f2ad81f`, 63,334,400 bytes, 4,523 members |
| Sorted path manifest | BLAKE3 `615909e72b656c6c9d52bcc683ebf47bf9975c7107d652a4698914b0f7b5f228`, 4,523 entries |
| Signed refs | `3c3d3f350f8b7a559906e6d6b6a4bad75441737c` → `ab9ab4a83e71af4ba23efa98dc5dd16432ea5ebe` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## What the published revision carries

- `kiln-dashboard`: a local, read-only operator surface over one canonical Kiln journal. It
  admits a journal document by recomputing every record's event identity under the encoding
  convention its identity kind declares and by canonical replay, then projects run, attempt,
  and event panels through the accepted `kiln-observe-core` projection.
- A property-based terminal rail over that surface, driven by the pinned Bombadil tool at
  revision `832faf4d953e2c56d1d901aca1bb2c3c3170221d`, with three journal fixtures and the
  licence map row for the separately executed tool.
- The accepted `observation-dashboard` and `bombadil-dashboard-rail` specifications, their
  archived change packages, and the local traceability policy that counts the TypeScript
  specifications and the flake as evidence.

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`141ec4d2`, all 4,523 tracked paths, the path manifest, and the archive identity. The Rosa
repository API reported the same `refs/heads/main` with public visibility and the unchanged
delegate set and threshold.

## Gates

`cargo test --workspace`, strict Clippy, `cargo fmt --check`, `nix flake check` for the
format, clippy, tests, nickel, octet, and bombadil checks, `cairn validate`,
`cairn traceability coverage` (846 of 846), `cairn release-readiness`, and
`cairn policy export --check` all passed on this revision.

## Post-publication gate refresh

The published revision added two traceability evidence roots to the local Cairn policy, so the
checked-in `fixtures/agent-completion/` fixture no longer bound the current generated policy and
`cairn release-readiness` failed on `mcp_agent_smoke`. The fixture's policy binding was refreshed to
the current policy hash; its required checks, command hashes, selection hash, registry hash, subject
hash, evidence hashes, and artifacts are unchanged. `cairn agent-receipt complete`,
`cairn agent-receipt smoke-mcp`, and `cairn release-readiness` pass again on this revision. The
refresh is recorded in `gate-refresh.json`.

## Placement of this receipt

A published revision cannot contain its own publication receipt, because the receipt names the
revision and its archive identity. Each publication receipt therefore lives in the revision that
follows the published one, matching the repository's existing convention for post-push receipts.
This receipt is the local evidence-only child of the current tip; it is not part of the archive it
identifies, and it grants no deployment, activation, binary release or release authority.

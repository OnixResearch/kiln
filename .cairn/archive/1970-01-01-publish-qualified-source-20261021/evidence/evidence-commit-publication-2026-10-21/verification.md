# Verification

## Evidence commit published

The operator requested `commit and push` after the evidence-revision publication of `acc359e7`.
The local evidence commit `acacf2b38f04f462f588215aa7e314e3cdc4ca04`, which carries the
`acc359e7` publication receipt, was pushed to canonical Radicle `main` and to the read-only
GitHub projection. Both were fast-forwards; no force push, history rewrite, governance,
delegate, threshold, visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `acacf2b38f04f462f588215aa7e314e3cdc4ca04` |
| Previous `main` on both remotes | `acc359e769bb3a056279bf5db8ba21e95c294a4c` |
| Source archive | BLAKE3 `840226ed3bcdd9671d26cb5f5e334c49be7270e6ba5fe47040d87e81d7225513`, 61,890,560 bytes, 4,465 members |
| Sorted path manifest | BLAKE3 `6342b8de98a20f992673272f40ca275ed0af5ab91cc339c5223811c68db4badc`, 4,465 entries |
| Signed refs | `0bae3cd81083e8c24a6b1a926c96a409bd584040` → `3993c3be7a257f803a0815620f26664fc3893ede` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`acacf2b3`, all 4,465 tracked paths, the path manifest, and the archive identity. The Rosa
repository API reported the same `refs/heads/main` and public visibility. A credential-isolated
anonymous clone with a cleared home, disabled global and system Git configuration, no credential
helper, no askpass, no inherited SSH agent, and Git protocol v0 ran
`nix develop path:<clone> -c cargo test --locked --workspace` to exit 0 with 527 tests passed and
0 failed.

## Gates

`cargo test --locked --workspace`, strict Clippy, `cargo fmt --check`, `cairn validate`,
`cairn traceability coverage`, `cairn release-readiness`, the Octet check (0 warnings,
0 errors), and the full `nix flake check` all passed on this revision.

## Non-claims

This receipt is a later local evidence descendant and is not inside the source archive it
identifies. Earlier publication receipts retain their original meaning.

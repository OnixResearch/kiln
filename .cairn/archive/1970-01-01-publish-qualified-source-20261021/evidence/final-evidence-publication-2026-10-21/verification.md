# Verification

## Final evidence revision published

The operator requested `commit and push` after the evidence-commit publication of `acacf2b3`.
The local commit `0019ff3cbe1abbc2cca1b109b539be719629cd15`, which carries the `acacf2b3`
publication receipt, was pushed to canonical Radicle `main` and to the read-only GitHub
projection. Both were fast-forwards; no force push, history rewrite, governance, delegate,
threshold, visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `0019ff3cbe1abbc2cca1b109b539be719629cd15` |
| Previous `main` on both remotes | `acacf2b38f04f462f588215aa7e314e3cdc4ca04` |
| Source archive | BLAKE3 `720a251e64e75e0e3606d55574745346801f7b14b2caa13ac438dbfbdacd6eaa`, 62,300,160 bytes, 4,473 members |
| Sorted path manifest | BLAKE3 `dc9695653796cfb5905a437506eab9b90fe4205309abd8f237e64e83cd6b8752`, 4,473 entries |
| Signed refs | `3993c3be7a257f803a0815620f26664fc3893ede` → `641407a59d28466eb2e1f35b313b5082e3313563` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`0019ff3c`, all 4,473 tracked paths, the path manifest, and the archive identity. The Rosa
repository API reported the same `refs/heads/main` with public visibility. A credential-isolated
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

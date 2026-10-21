# Verification

## Current tip published

The operator requested `commit and push` after the final-evidence publication of `0019ff3c`. The
local commit `ac9916c3fe7ddbb051f012f84252117f295b65fd`, which carries the `0019ff3c`
publication receipt, was pushed to canonical Radicle `main` and to the read-only GitHub
projection. Both were fast-forwards; no force push, history rewrite, governance, delegate,
threshold, visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `ac9916c3fe7ddbb051f012f84252117f295b65fd` |
| Previous `main` on both remotes | `0019ff3cbe1abbc2cca1b109b539be719629cd15` |
| Source archive | BLAKE3 `5aeae92760b408de64690669b3d1bce60cc9a379dae293a03ce02e6c7288bf9e`, 62,720,000 bytes, 4,481 members |
| Sorted path manifest | BLAKE3 `8e2584e9c87d73fc3c20488a4e11847a82b0699a800a28d45796feafc5c4b63d`, 4,481 entries |
| Signed refs | `641407a59d28466eb2e1f35b313b5082e3313563` → `3c3d3f350f8b7a559906e6d6b6a4bad75441737c` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`ac9916c3`, all 4,481 tracked paths, the path manifest, and the archive identity. The Rosa
repository API reported the same `refs/heads/main` with public visibility.

## Gates

`cargo test --locked --workspace`, strict Clippy, `cargo fmt --check`, `cairn validate`,
`cairn traceability coverage`, `cairn release-readiness`, the Octet check (0 warnings,
0 errors), and the full `nix flake check` all passed on this revision.

## Placement of this receipt

A published revision cannot contain its own publication receipt, because the receipt names the
revision and its archive identity. Each publication receipt therefore lives in the revision that
follows the published one, matching the repository's existing convention for post-push receipts.
This receipt is the local evidence-only child of the current tip; it is not part of the archive it
identifies, and it grants no deployment, activation, binary release or release authority.

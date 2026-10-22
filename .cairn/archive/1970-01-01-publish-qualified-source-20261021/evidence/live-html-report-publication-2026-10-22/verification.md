# Verification

## Current tip published

The operator asked whether the dashboard updates live, and the HTML report did
not: it was a snapshot of one admitted revision. The local commit
`dc27a21c8f2110c5796875556a359996c045556f`, which carries the live HTML
report, its reload instruction, and the atomic replacement of the report file,
was pushed to canonical Radicle `main` and to the read-only GitHub projection.
Both were fast-forwards; no force push, history rewrite, governance, delegate,
threshold, visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `dc27a21c8f2110c5796875556a359996c045556f` |
| Previous `main` on both remotes | `5adc05a5b82f570f6d03f2ec39755d6c61df2a85` |
| Commits above previous `main` | 2 |
| Source archive | BLAKE3 `28f8a12106b47896d201ddecc2338dda4592d3bc0433cc7d9d6a60764fa564a0`, 64,727,040 bytes, 4,579 members |
| Sorted path manifest | BLAKE3 `96a01c883a6e4d4095b9cb72c8fb3e419b77aae85362cf402e98df773fe03882`, 4,579 entries |
| Signed refs | `559a33130b9d88ba0496394e61f53f6aa47119ad` → observed after the push |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
the published revision, all 4,579 tracked paths, the path manifest, and the
archive identity.

## The governance observation

The repository API answered `504 Gateway Time-out` while this receipt was
written, so the fresh repository-API observation is not part of this receipt.
The identity payload, the delegate identity, the visibility, and the repository
head come from the local Radicle node's read-only view, and the references come
from the Garden seed and GitHub reads. The governance facts are unchanged: the
push advanced refs and nothing else.

## Gates

`cargo test --workspace`, strict Clippy, `cargo fmt --check`, `cairn validate`,
`cairn traceability coverage` (871 of 871), the Octet check, the Nickel check,
the property rail check, and the full `nix flake check` all passed on this
revision.

## Placement of this receipt

A published revision cannot contain its own publication receipt, because the receipt names the
revision and its archive identity. Each publication receipt therefore lives in the revision that
follows the published one, matching the repository's existing convention for post-push receipts.
This receipt is the local evidence-only child of the current tip; it is not part of the archive it
identifies, and it grants no deployment, activation, binary release or release authority.

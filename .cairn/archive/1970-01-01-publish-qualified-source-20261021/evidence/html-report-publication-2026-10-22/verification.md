# Verification

## Current tip published

The operator asked for the dashboard to be made available as HTML, and for the
rest of the work to be finished. The local commit
`5adc05a5b82f570f6d03f2ec39755d6c61df2a85`, which carries the dashboard's
self-contained HTML report, its format and output options, the browser property
rail, and the previous tip's publication receipt, was pushed to canonical
Radicle `main` and to the read-only GitHub projection. Both were
fast-forwards; no force push, history rewrite, governance, delegate, threshold,
visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `5adc05a5b82f570f6d03f2ec39755d6c61df2a85` |
| Previous `main` on both remotes | `298e1e27a2f938bb266fb043382f994c09d72946` |
| Commits above previous `main` | 2 |
| Source archive | BLAKE3 `149082ab0f4ebfbca57adf121db7fd7d057b1d6e4f165018ed9541aa49ee3af0`, 64,296,960 bytes, 4,565 members |
| Sorted path manifest | BLAKE3 `3f951869523b5ac7b3faf391aa5b89c2bee28df90835f41a88dc193cb75f5bb7`, 4,565 entries |
| Signed refs | `3dee70cc53439441780d648f04b1d3e0f0ab9972` → `559a33130b9d88ba0496394e61f53f6aa47119ad` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`5adc05a5`, all 4,565 tracked paths, the path manifest, and the archive
identity.

## Gates

`cargo test --workspace`, strict Clippy, `cargo fmt --check`, `cairn validate`,
`cairn traceability coverage` (866 of 866), `cairn release-readiness`, the
Octet check, the Nickel check, the property rail check, and the full
`nix flake check` all passed on this revision.

## Placement of this receipt

A published revision cannot contain its own publication receipt, because the receipt names the
revision and its archive identity. Each publication receipt therefore lives in the revision that
follows the published one, matching the repository's existing convention for post-push receipts.
This receipt is the local evidence-only child of the current tip; it is not part of the archive it
identifies, and it grants no deployment, activation, binary release or release authority.

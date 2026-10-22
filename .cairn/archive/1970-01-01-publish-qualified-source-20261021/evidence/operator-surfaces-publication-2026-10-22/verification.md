# Verification

## Current tip published

The operator requested `commit and push`, then `do all of these` for the remaining
operator surfaces. The local commit
`298e1e27a2f938bb266fb043382f994c09d72946`, which carries the dashboard store
input, the `kiln dashboard` operator command, the operator CLI property rail, and
a journal written by the OnixOS pilot's own writer, was pushed to canonical
Radicle `main` and to the read-only GitHub projection. Both were fast-forwards; no
force push, history rewrite, governance, delegate, threshold, visibility,
announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `298e1e27a2f938bb266fb043382f994c09d72946` |
| Previous `main` on both remotes | `141ec4d211c021a87da441070a5a6b3cc2378d76` |
| Commits above previous `main` | 2 |
| Source archive | BLAKE3 `c5abd96b6e77427daf16d11a619c05499cc6bced3ab414e0ee94915819e8d4e1`, 63,825,920 bytes, 4,549 members |
| Sorted path manifest | BLAKE3 `6be187bca19349e9839f7ab59ea4f32bb1ea8dc00e72b01ed6cf48b42abc049a`, 4,549 entries |
| Signed refs | `ab9ab4a83e71af4ba23efa98dc5dd16432ea5ebe` → `3dee70cc53439441780d648f04b1d3e0f0ab9972` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`298e1e27`, all 4,549 tracked paths, the path manifest, and the archive identity. The
Rosa repository API reported the same `refs/heads/main` with public visibility and
the unchanged delegate set and threshold.

## Gates

`cargo test --workspace`, strict Clippy, `cargo fmt --check`, `cairn validate`,
`cairn traceability coverage` (855 of 855), `cairn release-readiness`, the Octet
check, the Nickel check, and the full `nix flake check` all passed on this
revision, including the extended property rail over four journal cases and three
operator CLI cases.

## Placement of this receipt

A published revision cannot contain its own publication receipt, because the receipt names the
revision and its archive identity. Each publication receipt therefore lives in the revision that
follows the published one, matching the repository's existing convention for post-push receipts.
This receipt is the local evidence-only child of the current tip; it is not part of the archive it
identifies, and it grants no deployment, activation, binary release or release authority.

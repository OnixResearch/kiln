# Verification

## Evidence revision published

The operator requested `push` after the source-only publication of `c955334f`. The local
evidence revision `acc359e769bb3a056279bf5db8ba21e95c294a4c` was pushed to canonical Radicle
`main` and to the read-only GitHub projection. Both were fast-forwards; no force push, history
rewrite, governance, delegate, threshold, visibility, announcement, or deployment change occurred.

| Fact | Value |
|---|---|
| Published revision | `acc359e769bb3a056279bf5db8ba21e95c294a4c` |
| Previous canonical `main` | `c955334f54bdfe6e155e56c00573ca7ba77c4fb1` |
| Previous GitHub `main` | `305f85d80b9ea05fbd91db73de8a6662a55650ee` |
| Source archive | BLAKE3 `1f72af29b005a7162c45b4fbd60ba94c19698451663e1285c91f6b2496e69a9a`, 61,480,960 bytes, 4,457 members |
| Sorted path manifest | BLAKE3 `e97163d1367f5c76e6541683f06266f9866f28b2261ecb9ee204e19f4a4e7a83`, 4,457 entries |
| Signed refs | `27455a353402ecaa0bd9c6500a953c6d341c1575` → `0bae3cd81083e8c24a6b1a926c96a409bd584040` |
| Governance | one delegate, threshold one, public visibility, unchanged |

## Readbacks

Native Radicle, anonymous Garden HTTPS, and anonymous GitHub HTTPS each reproduced
`acc359e7`, all 4,457 tracked paths, the path manifest, and the archive identity. A
credential-isolated anonymous clone with a cleared home, disabled global and system Git
configuration, no credential helper, no askpass, no inherited SSH agent, and Git protocol v0
ran `nix develop path:<clone> -c cargo test --locked --workspace` to exit 0 with 527 tests
passed and 0 failed.

## Gates

`cargo test --locked --workspace`, strict Clippy, `cargo fmt --check`, `cairn validate`,
`cairn traceability coverage`, `cairn release-readiness`, the Octet check (0 warnings,
0 errors), and the full `nix flake check` all passed on this revision.

## Non-claims

This receipt is a later local evidence descendant and is not inside the source archive it
identifies. The earlier `c955334f` source publication receipt retains its original meaning.

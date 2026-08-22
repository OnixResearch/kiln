# Trigger-to-workflow increment evidence

Date: 2026-08-20

## Published prerequisite

Lattice revision `70496e67c7fd4a8b05914161a8e09de2759bebc8` publishes request schema version 2, durable dispatch recovery, source-bound shell environment injection, instantiated revision authority, and bounded provider-native source values.

## Implemented increment

- The deployed Defelo broker protocol maps exact repository and revision facts.
- The production executable requires an explicit profile path.
- The executable dispatches only through `LatticeAdapter<UnixWorkflowExchange>`.
- The executable contains no direct Nix runner and no fixture fallback.
- Deterministic BLAKE3 trigger identities produce run, effect, attempt, candidate, pipeline, and idempotency identities.
- SQLite reserves replay state before dispatch.
- Restart recovery sends only `RecoverDispatch`; it never automatically redispatches an uncertain request.
- Mixed-case Radicle RIDs remain exact through the Lattice request.
- Source, status, and collaboration capability selection fail closed in the current production profile.

## Checks

- `cargo test --workspace`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo fmt --all -- --check`: pass.
- Positive and negative Radicle Nickel profiles: pass.
- Cairn validation: pass.
- Tracey: 401 of 401 requirements referenced, no missing or dangling IDs; receipt `4cffc7040033ee3c71b5b91937aa78fcc4740f867a0fb92c57a05bf60a331334`.
- `nix flake check path:$PWD -L`: pass, including workspace, Clippy, `no_std`, Nickel, and release-evidence checks.

`cargo octet check` is not an acceptance rail in this repository. It stops because the workspace has no `[metadata]` table. No Octet result is claimed.

## Remaining blocker

The broad Radicle shell change remains blocked for source, status, and collaboration profiles. It needs exact published provider contracts. Collaboration also needs operator-selected signer and expected-head authority.

## Non-claims

This increment does not prove event completeness, source trust, revision availability, checkout correctness, workflow correctness, provider availability, status publication, collaboration mutation, peer convergence, external effect success, or release readiness.

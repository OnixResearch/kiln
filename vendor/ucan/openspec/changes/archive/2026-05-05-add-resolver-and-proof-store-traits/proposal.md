## Why

Verification currently receives a fixed in-memory key list and a fixed in-memory proof collection. That keeps tests deterministic, but it forces every reusable crate to build its own adapter for key lookup, proof lookup, caching, and traversal limits. Modern UCAN-like libraries need explicit resolver/store abstractions while keeping I/O out of the pure core.

This is the third-priority follow-up because real consumers need to verify tokens from request, cache, database, or blockstore contexts without reimplementing proof traversal.

## What Changes

- Introduce key resolver and proof store traits in the std shell.
- Keep the existing in-memory key/proof contexts as adapters implementing those traits.
- Add verification limits for proof depth, proof count, token size, and cycle protection.
- Update verifier orchestration to resolve keys and proofs through explicit abstractions.

## Non-Goals

- Add HTTP/IPFS/database fetchers in this change.
- Make verification use hidden global state.
- Move resolver abstractions into `ucan-core`.

## Capabilities

### New Capabilities
- `key-resolver-abstraction`: Verifiers can resolve issuer keys through an explicit trait.
- `proof-store-abstraction`: Verifiers can load proof tokens through an explicit trait.
- `verification-limits`: Recursive verification has deterministic resource bounds.

### Modified Capabilities
- `library.delegation-chain`: Proof traversal no longer requires preassembled transitive proof collections.
- `library.fcis-boundary`: Resolver and store I/O remains shell-owned and injectable.

## Impact

- **Files**: `src/token.rs` or new resolver/proof modules, tests, examples, README/API docs.
- **APIs**: Adds resolver/store traits, in-memory adapters, verification limits, and verifier context changes.
- **Dependencies**: No required dependency for sync traits; async support can remain a later feature.
- **Testing**: Needs positive tests with in-memory adapters and negative tests for missing keys, missing proofs, cycles, and limit exhaustion.

## Why

The library can verify signatures, proof chains, caveats, and invocations, but callers still have no first-class way to deny a token or any proof in its delegation chain after issuance. Revocation is repeatedly documented as out of scope, which leaves production integrations to wrap verification and risks checking only the leaf token while accepting revoked parents.

## What Changes

- Add caller-supplied revocation-checker traits and an in-memory revocation list keyed by compact-token proof references.
- Add verification entry points that check revocation for the leaf token and every resolved proof before structural parsing or authorization succeeds.
- Preserve existing verification behavior when no revocation checker is supplied.

## Capabilities

### New Capabilities
- `library.revocation.explicit-checker`: Callers can inject revocation state without global registries, network fetchers, or persistence layers.
- `library.revocation.chain-denial`: Verification fails closed when the leaf token or any proof-chain token is revoked.

### Modified Capabilities
- `library.consumer-onboarding`: Documentation moves revocation from unsupported to caller-supplied explicit verification support.

## Impact

- **Files**: `src/token.rs`, `src/lib.rs`, README/docs, examples, OpenSpec library spec.
- **APIs**: Adds revocation checker/list types and revocation-aware verify entry points.
- **Dependencies**: No new dependencies; no network/runtime/storage additions.
- **Testing**: Positive no-revocation compatibility, leaf revocation denial, parent proof revocation denial, backend error propagation, docs/examples build checks.

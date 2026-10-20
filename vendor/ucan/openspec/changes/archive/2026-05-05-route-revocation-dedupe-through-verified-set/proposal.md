## Why

`RevocationList::is_revoked` already routes exact finite-set membership through the vendored verified revocation primitive, but the helper mutation path still used a shell-local iterator equality check to avoid duplicate entries. Keeping both paths duplicates set-membership semantics and weakens the bridge guard.

## What Changes

- Route `RevocationList::revoke` duplicate detection through `revocation_reference_is_listed`.
- Add a regression covering idempotent revocation insertion.
- Strengthen the bridge guard and documentation so exact reference membership for `RevocationList` stays verified-logic-backed.

## Impact

- Files: `src/token.rs`, `scripts/check-ucan-bridge.sh`, `README.md`, `docs/public-api.md`, OpenSpec library spec.
- API: no public API shape changes.
- Verification: focused revocation tests, bridge guard, full Cargo/Nix/OpenSpec gates.

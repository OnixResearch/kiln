## Why

Issue #36 requires crash-safe immutable cache publication, corruption as a measured miss, and receipts that identify verified content and source tier.

## What Changes

- Stage outside the key namespace, verify retained bytes, fsync, atomic no-replace rename and directory fsync.
- Recover orphan stages under exclusive ownership; never overwrite existing keys.
- Add deterministic publish phase hooks and SIGKILL regression fixtures.
- Bind cache observations to entry BLAKE3 and local tier; keep remote storage explicitly absent.

## Impact

- **Files**: hosted cache/execution, web-service cache store and tests, hosted-ci cache documentation.
- **Testing**: parent runs targeted cache tests and KVM regression after concurrent implementation lands.

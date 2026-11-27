## Why

Fleet's legacy agent counts do not expose the enrolled Herdr Core terminal topology or program-reported status.

## What Changes

- Add an opt-in read-only observer of a pinned Core endpoint, without changing placement, claims, dispatch or verification.

## Impact

- **Files**: Fleet observer, typed configuration and console.
- **Testing**: Verify snapshot, reconnect, stale host and redacted terminal projection.

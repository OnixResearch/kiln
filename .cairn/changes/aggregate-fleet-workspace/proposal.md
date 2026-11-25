## Why

The Fleet console lists coordinator activity but omits idle, unobserved and unreachable enrolled hosts. Operators also need to see which source records the work came from.

## What Changes

- Add a machine-first view with observed Herdr sessions, Fleet assignments and independent Cairn and issue coverage.

## Impact

- **Files**: Fleet coordinator, Herdr adapter, and authenticated console projection.
- **Testing**: Verify a multi-machine snapshot and a populated browser fixture, including missing and stale observations.

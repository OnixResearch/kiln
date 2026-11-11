# Add a bounded GitHub polling admission pass

## Why

The authenticated polling transport and normalization exist, but no operator
path turns one pass into durable work. Issue #44 requires bounded polling
recovery that produces one canonical work version without dispatching.

## What Changes

- Add `poll_into_implementer` to normalize a bounded pass and admit it into the
  durable work ledger.
- Report admitted, duplicate, superseded, stale and closed observations.
- Add `kiln-implementer poll` with policy, enrollment, token, page bound, optional
  since timestamp and timeout.
- Keep scheduling, retries, claims and dispatch outside the pass.

## Impact

- **Files**: `crates/kiln-adapter-github/src/poll.rs`, implementer CLI, docs and
  specification.
- **Testing**: repeated pass admits one version and reports duplicate; closed
  observations do not complete work; bound/auth failures fail the pass.
- **Authority**: admission only, through the existing work ledger.

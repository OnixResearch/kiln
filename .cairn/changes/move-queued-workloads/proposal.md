## Why

An idle automatic Herdr reservation can point at the wrong enrolled host. Operators need to redirect queued work without cancelling the attempt or requesting another execution.

## What Changes

- Add operator-only preview, submit and status for rebind of one exact queued attempt reservation to an explicitly selected host.
- Refuse active or potentially started work; quota failover and live migration are distinct capabilities.

## Impact

- **Files**: Fleet routing and Herdr dispatch, Implementer placement persistence and pure core reservation rules.
- **Testing**: Disposable Herdr and ledger fixtures for destination admission, stale bindings, competing dispatch and replay after uncertainty.

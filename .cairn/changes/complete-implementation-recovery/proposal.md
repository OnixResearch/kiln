# Complete implementation recovery and coordinator admission

## Why

Issues #45–48 and #50 require durable exact-attempt authority across source changes, crashes and uncertain provider effects. Current shells retain claims but admit unknown failure as a new attempt and do not atomically apply source supersession.

## What Changes

- Bind durable intake supersession to claim ownership and frozen contracts.
- Refuse stale, cancelled, superseded and unknown-outcome attempts at worker/publication boundaries.
- Preserve explicit reconciliation before retry and expose bounded recovery controls in the existing operator shell.
- Compose independently executing verification and provider-owned execution/publication through existing exact identities.

## Impact

Owned core attempt state, durable implementer shell, implementation CLI and implementation operator documentation. No new merge authority, no live qualification, publication or deployment.

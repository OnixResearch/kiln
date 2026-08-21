# Add a Cairn gate adapter

## Why

Kiln declares `cairn-gate-v1`, but no adapter evaluates a real Cairn policy or consumes its gate receipts.

Cairn owns lifecycle and release-readiness decisions. Kiln must bind each result to one exact candidate without importing lifecycle authority.

## What Changes

- Add optional `kiln-adapter-cairn` support for the `GatePort`.
- Pin one exact Cairn CLI, policy, request, diagnostic, and receipt contract.
- Construct bounded gate requests through a pure core.
- Bind each request to the candidate root, pipeline revision, gate policy, evidence set, and idempotency key.
- Preserve Cairn change, requirement, receipt, policy, and tool identities.
- Reject stale, substituted, incomplete, or ambiguous results.
- Keep release execution and merge authority outside the adapter.
- Add typed Nickel profiles, process bounds, fixtures, and reconciliation.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and a published Cairn gate contract.

## Impact

- **Adapters**: adds `kiln-adapter-cairn`.
- **Core**: adds only provider-neutral gate facts and pure admission decisions.
- **Configuration**: adds a pinned Cairn gate profile.
- **Testing**: covers pass, block, stale candidate, malformed receipt, timeout, and process ambiguity.

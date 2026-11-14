## Why

Issues #52–55 have pure retained/handoff/bridge admission but no host effects. Retained operations must survive crashes without repeating mutation or freeing uncertain capacity. Transport delivery cannot substitute for durable ownership and actual disk/effect fencing.

## What Changes

Extend the existing hosted Cloud Hypervisor shell with durable retained operation execution, protected sockets and bounded state/workspace transfer. Add authenticated single-use Iroh migration stream handling, explicit compatibility/live qualification and evidence-based drain decisions. Preserve per-job no-network behavior.

## Capabilities

### New Capabilities

- `retained-mobility-runtime`: hosted retained effects, verified durable transfer and non-replayable migration operations.

### Modified Capabilities

- Retained lifecycle: retirement requires observed cleanup.

## Impact

Kiln microvm, hosted runner, Iroh adapter and retained/migration runbooks. Aspen durable distributed ownership and enforcement remain provider prerequisites; no new coordinator or VMM supervisor is introduced.

## Why

Kiln can split work across hosts and move a queued reservation, but cannot move an agent that is already running. Interactive Herdr agents are host processes and have no checkpoint. Running the agent inside a Cloud Hypervisor guest makes the whole guest the movable unit.

## What Changes

- Add an operator command that moves one running guest attempt to an explicitly selected host: pause, checkpoint, transfer, restore, reconnect, then retire the source.
- Record every step before its effect and treat a lost reply as uncertain. The source is retired only after Aspen confirms the destination owns the guest and its resources.
- Show the move in the Fleet console with its exact plan, current step and final outcome.
- Keep the existing Herdr split and queued-move controls unchanged.

## Impact

Kiln core placement types, the hosted retained-mobility shell (`integrate-retained-mobility`), Fleet routes, the web bridge, the console, and the authority registry.

Depends on, in their owning repositories:
- Aspen `fence-guest-placement-ownership`: exclusive guest ownership and resource fencing (Aspen #15). Not yet written; the Aspen repository is held by another session.
- Lattice `reconnect-restored-guest-sessions`: epoch-scoped guest reconnect.
- A two-host qualification on operator-owned compatible KVM hosts.

Until all three exist, the command fails closed before any effect.

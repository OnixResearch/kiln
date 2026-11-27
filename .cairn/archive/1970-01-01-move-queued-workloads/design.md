## Context

Automatic Fleet dispatch reserves an interactive plan for a leased implementation attempt and later binds a Herdr session before any launch. The reservation is not evidence of an agent or workspace.

## Decisions

### Decision: Rebind only before a session or transport is reserved

**Choice:** Preview a content-addressed plan over the exact current attempt, fence, old reservation and explicitly admitted destination. Submit re-derives it and atomically swaps the native reservation and persists its receipt in the same store that binds sessions. The dispatcher compares the exact reservation when binding; whichever transaction wins makes the other stale. Check reachability, kind capacity, enrollment, drain and repository checkout through the existing Herdr placement owner. Destination capacity is checked again at launch.

**Rationale:** Preserve work, scope, lease, attempt budget and fence. Reuse guarded Herdr launch and exact observation, recording a single launch intent before external effects. Replay reports retained placement, not a new command. Once any session or launch evidence exists, rebind is unavailable; active transfer would need separate fenced checkpoint, source termination and resume authority.

## Risks / Trade-offs

- Occupancy is an observation, not a provider-side lease. Competing workloads may take the slot; the dispatcher must defer instead of selecting a different destination. Ambiguous launches require reconciliation and cannot be replayed as starts.

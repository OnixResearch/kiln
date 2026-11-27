## Context

Peer admission, Herdr placement, Fleet assignments, Cairn changes and native issue records are distinct facts.

## Decisions

### Decision: Read existing owners without inferring missing facts

**Choice:** Cache the coordinator's bounded Herdr observations and expose the enrolled roster even without successful reads. Keep source aliases on exact Fleet work identities and read the existing authorization-filtered issue and Cairn projections.

**Rationale:** Enrolled does not mean reporting, and a shared title or repository does not bind an issue to a host or attempt.

## Risks / Trade-offs

- Report unsupported, stale, incomplete and unreachable sources independently. Do not disclose provider routes or raw command output.

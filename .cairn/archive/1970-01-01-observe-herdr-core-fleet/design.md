## Context

Herdr Core owns terminal lifecycle; Kiln owns work and authority. Terminal names and program status cannot establish Kiln assignment.

## Decisions

### Decision: Independent read-only Core feed

**Choice:** Use an explicitly configured local Unix socket and exact Core version; read typed lists and subscribe to events to invalidate snapshots. Map Core machine names only to enrolled hosts.

**Rationale:** A disconnected host cannot be inferred idle; observation must not create a launch or terminal-control surface.

## Risks / Trade-offs

- Only closed status and aggregate counts leave the observer. Core terminals without exact Kiln provenance remain unassigned.

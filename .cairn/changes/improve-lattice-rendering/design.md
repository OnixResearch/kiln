## Context

The live workspace retains 19 Lattice-backed runs, all with one admitted `test` node and no edges. Both Completed and Failed observations exist; historical node durations/start times are null and must remain unknown. The current graph has fixed 190px cards and a repeated edge-relaxation loop, with no invalid-cycle warning.

## Decisions

### Decision: Derive layout only from admitted dependencies

Use a deterministic topological traversal over the supplied nodes/edges. Stages are longest dependency depth, not execution time, placement or provider scheduling. Missing edges, duplicate IDs, unknown endpoints and cycles disable diagram topology with an explicit explanation, while retained node records remain inspectable. Preserve exact node states; Completed is not renamed Success and skipped/unknown are not green success.

### Decision: Keep presentation local and read-only

Use existing inert DOM helpers and theme tokens. Show state badges/counts, stage headers, full node names and measured card sizes. Fit/zoom affects only the scrollable diagram. Node selection highlights immediate incoming/outgoing connections and offers explicit prerequisite/dependent links and log navigation. A structured node list remains usable without the diagram; live renders preserve zoom, pan and keyboard focus for the same run. Clearing a session retires all new private views. Do not add provider coupling, infer blocked states/timings, or turn diagram interaction into a mutation.

## Risks / Trade-offs

Fit may make a large diagram small; retain actual-size and bounded zoom controls plus the full node list. Stage labels describe dependency depth only. Missing historical timings remain unknown rather than borrowing the run duration. Live verification cannot claim a real multi-node execution because none is retained in the observed page.

## Verification Strategy

Validate proposal/design/tasks before implementation. Inspect real failed/completed Lattice runs, then exercise branching/join/isolated nodes, long names, state distinctions, keyboard navigation, fit/zoom, selected relationships/log links, refresh/restoration and 390px layout with candidate browser assets. Negative fixtures cover missing edges, invalid endpoints, duplicate IDs, cycles and empty graphs. Check inert text and session retirement. Run JavaScript syntax and focused service checks; update operator guidance and retain bounded observations, without adding source-text or mock-forwarding tests.

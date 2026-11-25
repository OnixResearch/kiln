## Context

P1 delivers the 7-destination Factory Control Plane, Cockpit + Slide-Over Inspector, multiplexed SSE, and 3-pane Code Review workbench. Three high-value gaps remain for long-term differentiation and scale: (1) event history, multi-node log triage, and multi-gate provenance still rely on raw `renderRecorded()` disclosures and disconnected tabs; (2) fair-admission tenant quotas, `AttemptMetrics` burn telemetry, full plan-review step annotations, and publication/integration recovery are still CLI-only; and (3) `server.rs` serves 33 uncompressed uncached static assets on every cold load while `docs/console-port-inventory.md` flags missing Node module tests for `web.js`, `fleet.js`, `work.js`, and `project.js`.

## Decisions

### Decision: Replace raw `renderRecorded()` dumps with a Causal Event Timeline and unified Gate Passport

**Choice:** Render `run.events` and audit records as a chronological state-machine progression (`Admitted -> Gate -> Dispatched -> Observations -> Controls/Unknown -> Terminal`) with copyable 12-char BLAKE3 prefixes and collapsible raw JSON, paired with a Depot-style Cross-Node Failure Digest above `Logs` and a unified **Gate Passport** aggregating CI checks, `cairn-authoritative-completion`, Patchgate receipts, SLSA/in-toto digests, Plan Review annotations, and Semantic Merge O/A/B/M quartets. Keep strict visual provenance borders (`Observed Receipt` solid, `Agent Claim` dotted, `Declared` dashed) in both `kiln-web-service` and read-only `kiln-dashboard`.

**Rationale:** Turns Kiln's deterministic event-sourced core and cryptographic provenance into an immediately legible operator superpower while preserving raw exact records one click away.

### Decision: Surface live Fair-Admission, `AttemptMetrics`, and guarded lifecycle publication/recovery actions

**Choice:** Add `GET /api/implementation/admission` and guarded share/quota adjustment controls in `3.1 Fleet & Workers` alongside live `AttemptMetrics` token/wall-time burn, and expose owner-delegated publication and integration recovery actions (`prepare-publication`, `publish`, `recover-integration`, `close-source`) behind Tier 2/3 safety rails in `2.1 Delivery & Cairn` without moving forge or lifecycle authority into the browser.

**Rationale:** Closes the last major CLI-only operator bottlenecks for running sustained multi-tenant agent factories while strictly preserving Kiln's core/shell authority boundaries.

### Decision: Opt-in inert Atlas topology and HTTP static asset compression + `ETag` caching

**Choice:** Implement opt-in Atlas topology (`docs/atlas-view.md`) strictly bounded to `<=64` nodes using same-origin `dag-canvas` / flat graph fallbacks with explicit declared-vs-observed styling, and add content-addressed `ETag` + `304 Not Modified` + `gzip`/`zstd` encoding (and optional asset grouping) to `server.rs` static routes alongside Node module test suites for `web.js`, `fleet.js`, `work.js`, and `project.js`.

**Rationale:** Keeps cold boot fast over tailnets and remote links without introducing third-party CDNs or JS build-chain sprawl, and closes the test gaps flagged in `docs/console-port-inventory.md`.

## Risks / Trade-offs

- Rendering live publication/integration recovery mutations in the UI must fail closed on stale revisions, mismatched candidates, or missing provider authority (`AGENTS.md` Radicle/GitHub mutation guardrails remain absolute).
- Static asset compression in `server.rs` must respect `Accept-Encoding`, vary headers cleanly, and bound memory/CPU per request without adding heavy external runtime dependencies.
- Atlas views above 64 nodes MUST refuse or fall back to bounded list/table summaries rather than rendering unreadable dense graphs.

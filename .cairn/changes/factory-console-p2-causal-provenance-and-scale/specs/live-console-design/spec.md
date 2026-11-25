## ADDED Requirements

### Requirement: Causal event timeline, cross-node failure digest, and bounded opt-in Atlas topology

r[kiln.web.causal_timeline_and_failure_digest]
Run `Activity` and `Evidence` MUST render canonical events and audit records as a chronological state-machine progression (`Admitted -> Gate -> Dispatched -> Observations -> Controls/Unknown -> Terminal`) showing timestamp, principal/agent, and copyable 12-char BLAKE3 `event_identity` prefixes while preserving raw JSON inside collapsible disclosures. Run `Logs` MUST display a deduplicated Cross-Node Failure Digest above the log viewer for multi-node DAG failures with 1-click navigation to each failed node's `stderr`/`stdout`. Opt-in Atlas topology views (`docs/atlas-view.md`) MUST use same-origin assets only, distinguish declared vs observed edges visually, and enforce a strict `<= 64`-node bound with bounded table fallback when exceeded.

#### Scenario: Multi-node DAG failure surfaces deduplicated failure digest and causal timeline

r[kiln.web.causal_timeline_and_failure_digest.scenario.multi_node]
- GIVEN a retained multi-node CI run where two workflow nodes failed after dispatch
- WHEN the operator opens the run's `Activity` and `Logs` views
- THEN `Activity` MUST display the ordered causal state progression with copyable BLAKE3 event identities
- AND `Logs` MUST surface a deduplicated failure summary listing both failing nodes above the log pane without requiring manual dropdown hunting

#### Scenario: Atlas topology refuses unbounded graphs

r[kiln.web.causal_timeline_and_failure_digest.scenario.atlas_bound]
- GIVEN a topology projection containing more than 64 nodes
- WHEN the operator requests the opt-in Atlas view
- THEN the console MUST enforce the 64-node bound and render an explicit bounded summary fallback rather than an unbounded graph

## ADDED Requirements

### Requirement: The Fleet console distinguishes snapshot composition from freshness

r[kiln.fleet.console_snapshot] The authenticated Fleet console MUST label attempt state counts as a breakdown of the listed attempts, distinguish ranked work from the ready-and-unclaimed count, and distinguish the last accepted snapshot from connection and command feedback. A transport failure MUST NOT make retained records appear current. An unrelated update MUST NOT discard a draft answer to a decision that remains open.

#### Scenario: Changes and outages preserve observation meaning

r[kiln.fleet.console_snapshot.scenario.transitions]
- GIVEN a bounded snapshot with mixed attempt states and an open decision
- WHEN a heartbeat, an unrelated update and a connection failure arrive
- THEN the state breakdown MUST count listed attempts only, the draft answer MUST remain, and the last accepted snapshot MUST be marked stale without becoming an empty register

### Requirement: Fleet attempt visualizations drill down locally

r[kiln.fleet.console_drilldown] The Fleet console MUST let operators use keyboard or pointer to filter listed attempts by coordinator state and recorded session machine. The register MUST show the intersection, distinguish no matches from no attempts, and retain the selection across snapshots until cleared. Bar counts MUST still describe the whole listed snapshot.

#### Scenario: Combined filters outlive a changing snapshot

r[kiln.fleet.console_drilldown.scenario.intersection]
- GIVEN listed attempts on two machines and an attempt without a recorded machine
- WHEN the reader selects a state and a machine and the next snapshot has no matching row
- THEN the selected filters MUST remain visible with no matching rows until cleared
- AND clearing MUST restore all rows of the latest snapshot without a request

### Requirement: Fleet shows current Cairn lifecycle progress

r[kiln.fleet.cairn_progress] Fleet MUST refresh the canonical Cairn index while visible independently of its coordinator stream. It MUST show each listed change's observed lane, unit lane counts and exact detail link, distinguish truncated and unobserved results, and mark retained rows stale after a failed refresh rather than inferring progress.

#### Scenario: Cairn progresses without a Fleet event

r[kiln.fleet.cairn_progress.scenario.refresh]
- GIVEN Fleet shows a Cairn change in progress
- WHEN the Cairn index records a later lifecycle lane and then a refresh fails
- THEN the later lane MUST appear without a Fleet event and remain visible as stale after the failure

### Requirement: Cairn progress attributes workers by exact attempt

r[kiln.fleet.cairn_workers] Fleet MUST show the recorded worker binding or matching Fleet session for listed active attempts on current Cairn work. A Fleet session MUST match both attempt and fence. Missing evidence MUST remain unknown, and a dispatch binding MUST NOT imply live placement.

#### Scenario: A neighboring session cannot be substituted

r[kiln.fleet.cairn_workers.scenario.attribution]
- GIVEN an active Cairn attempt with a retained worker binding and Fleet sessions on the current and a different fence
- WHEN Fleet displays the change
- THEN it MUST show the recorded binding and only the current-fence session
- AND an attempt without either observation MUST remain unattributed

### Requirement: Cairn execution context distinguishes plans from observations

r[kiln.fleet.cairn_execution_context] Fleet MUST distinguish the dispatch provider, harness, requested artifact and bound model services from an observed Fleet machine and agent. The display MUST NOT infer a serving model, a Lattice host or swarm membership from the plan or missing observations.

#### Scenario: A plan is not a runtime observation

r[kiln.fleet.cairn_execution_context.scenario.evidence]
- GIVEN an active attempt with planned model evidence and no matching session
- WHEN Fleet displays the attempt
- THEN the model MUST be identified as requested or bound, and machine and swarm membership MUST remain unobserved
- AND a later exact Fleet session MUST populate its machine and agent, not change the model into a runtime claim

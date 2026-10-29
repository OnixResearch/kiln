## ADDED Requirements

### Requirement: Session-aware log following

r[kiln.dashboard.complete.follow]

Log following SHALL continue for admitted network and bearer sessions, stop on logout/navigation, and preserve exact byte cursors and visible truncation.

#### Scenario: Admitted follow behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented follow path is exercised
- THEN An authenticated network session follows multiple retained chunks without Authorization.

#### Scenario: Rejected or unavailable follow behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the follow boundary is exercised
- THEN A cleared or replaced session cannot publish late log bytes.

### Requirement: Run discovery and navigation

r[kiln.dashboard.complete.navigation]

The live workspace SHALL provide bounded search, project/outcome/source filters, stable pagination, deep links and reload/back restoration without persisting credentials.

#### Scenario: Admitted navigation behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented navigation path is exercised
- THEN A linked run, filters and selected node survive reload and back navigation.

#### Scenario: Rejected or unavailable navigation behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the navigation boundary is exercised
- THEN An absent run or malformed cursor is explicit and never replaced by another run.

### Requirement: Recorded metrics and overview

r[kiln.dashboard.complete.overview]

The service SHALL record host-observed lifecycle/node timestamps outside the pure core and expose bounded counts, project health, duration trends and comparisons; historical unknown timing SHALL remain unknown.

#### Scenario: Admitted overview behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented overview path is exercised
- THEN A completed run contributes its recorded duration and outcome to the overview.

#### Scenario: Rejected or unavailable overview behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the overview boundary is exercised
- THEN A historical run without timestamps does not receive invented timing.

### Requirement: Pipeline inspection

r[kiln.dashboard.complete.pipeline]

The workspace SHALL render admitted dependency edges, stage/node progression, node details and recorded timing without inferring progress from log text.

#### Scenario: Admitted pipeline behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented pipeline path is exercised
- THEN An execution with dependencies renders the actual DAG and per-node state.

#### Scenario: Rejected or unavailable pipeline behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the pipeline boundary is exercised
- THEN Unsupported or absent graph/timing facts remain unavailable.

### Requirement: Continuous bounded worker output

r[kiln.dashboard.complete.streaming]

Worker stdout/stderr SHALL be uploaded while commands execute through the reusable bounded child-process mechanism, preserving cancellation, process-group teardown, byte budgets and cursor integrity. Log search and full retained stream download SHALL operate over admitted bytes.

#### Scenario: Admitted streaming behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented streaming path is exercised
- THEN A running command publishes an initial output chunk before it exits, and a consumer downloads the exact retained stream.

#### Scenario: Rejected or unavailable streaming behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the streaming boundary is exercised
- THEN Overflow, failed ingress, stale bindings and cancellation do not become successful execution.

### Requirement: Explicit source-bound reruns and queueing

r[kiln.dashboard.complete.reruns]

Operators SHALL be able to run the latest admitted reference, select an admitted reference or exact reachable revision, rerun an exact prior revision, and retry a known failed node set as a new candidate/attempt. Requests blocked by current project activity SHALL be durably queued and dispatch once when eligible.

#### Scenario: Admitted reruns behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented reruns path is exercised
- THEN An exact rerun preserves source revision while receiving new run/attempt identities; a queued run survives restart.

#### Scenario: Rejected or unavailable reruns behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the reruns boundary is exercised
- THEN Unknown outcomes are not blindly retried and stale or unauthorized source/node selections have no provider effect.

### Requirement: Structured tests coverage and comparison

r[kiln.dashboard.complete.reports]

Workers SHALL publish bounded declared JUnit and coverage files with exact run/source/attempt/execution/node binding. The workspace SHALL show failure details, coverage, repeated-test instability and comparisons based only on admitted reports.

#### Scenario: Admitted reports behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented reports path is exercised
- THEN Paired pass/fail reports and coverage produce actual test details and a comparison across runs.

#### Scenario: Rejected or unavailable reports behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the reports boundary is exercised
- THEN Malformed, oversized, escaped, entity-bearing or cross-run reports are rejected; absence is not zero failures or full coverage.

### Requirement: Activity and evidence inspection

r[kiln.dashboard.complete.evidence]

The workspace SHALL expose recorded canonical event activity, attributable audit decisions, actual gate/review report records and source/pipeline/artifact provenance links with explicit claim boundaries.

#### Scenario: Admitted evidence behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented evidence path is exercised
- THEN A source-bound report and its exact artifact identity are navigable with recorded verdict and linkage.

#### Scenario: Rejected or unavailable evidence behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the evidence boundary is exercised
- THEN A recorded review is not acceptance authority and stale evidence cannot attach to another attempt.

### Requirement: Triggers schedules and notifications

r[kiln.dashboard.complete.automation]

Operators SHALL configure enabled state and parameters for admitted push/PR triggers, bounded schedules and registered completion/failure notification destinations. The service SHALL execute real scheduling and delivery with durable replay prevention and visible definite/ambiguous outcomes. Destinations and credentials SHALL remain server-owned typed configuration.

#### Scenario: Admitted automation behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented automation path is exercised
- THEN A signed exact-repository event or due schedule starts one run, and its terminal result reaches a registered HTTPS receiver.

#### Scenario: Rejected or unavailable automation behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the automation boundary is exercised
- THEN Invalid signatures, duplicate events, disallowed refs/destinations and ambiguous delivery never cause unsafe repeated effects.

### Requirement: Operational state and retention warnings

r[kiln.dashboard.complete.operations]

The workspace SHALL show actual queue positions, execution capacity, provider observation health and bounded run/request/log/artifact/state usage with warnings before retention admission fails.

#### Scenario: Admitted operations behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented operations path is exercised
- THEN Queued work advances after active work completes and usage warnings reflect real retained bytes/counts.

#### Scenario: Rejected or unavailable operations behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the operations boundary is exercised
- THEN Unavailable providers remain unavailable and warnings neither prune canonical authority nor invent healthy capacity.

### Requirement: Attributable read-only and operator access

r[kiln.dashboard.complete.access]

Private API access SHALL identify a bounded principal, enforce viewer/operator/worker separation before effects and durably record attributable mutations and denials. Tailnet access SHALL remain tokenless in the browser, using trusted server-side peer identity; client-supplied identity headers SHALL not grant authority.

#### Scenario: Admitted access behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented access path is exercised
- THEN A configured operator can mutate, a viewer can inspect but not mutate, and an audit entry identifies the acting principal.

#### Scenario: Rejected or unavailable access behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the access boundary is exercised
- THEN Forged headers, foreign/missing write origins, role substitution and direct localhost requests without credentials are denied.

### Requirement: One dashboard application convention

r[kiln.dashboard.complete.shell]

The live workspace SHALL reuse the existing catalog application shell, navigation vocabulary, appearance/density and accessibility conventions while retaining a separate offline read-only boundary.

#### Scenario: Admitted shell behavior

- GIVEN the exact admitted source, policy and operation facts
- WHEN the implemented shell path is exercised
- THEN Overview, Projects, Runs, Activity, Evidence and Settings use the existing shell conventions on desktop and mobile.

#### Scenario: Rejected or unavailable shell behavior

- GIVEN a missing, stale, malformed, unauthorized or bounded input
- WHEN the shell boundary is exercised
- THEN Unavailable data is not replaced with demo cards or fake controls.

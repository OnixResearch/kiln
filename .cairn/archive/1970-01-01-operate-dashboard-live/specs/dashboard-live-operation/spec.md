# Live dashboard operation

## ADDED Requirements

### Requirement: Bounded read-only candidate collection
r[kiln.dashboard.live.collection]
The collector MUST admit closed versioned configuration, inspect only bounded immediate canonical candidate directories, retain exact candidate/source identities and original journals, and deterministically build a bounded catalog without dispatch, reconciliation, signing, status publication or canonical-state mutation.

#### Scenario: Valid and rejected observation inputs
r[kiln.dashboard.live.collection.scenario.admission]
- GIVEN valid canonical candidate journals or malformed, oversized, symlinked, duplicate, colliding or out-of-bound inputs
- WHEN a finite collection cycle runs
- THEN admitted histories MUST preserve their source identity and deterministic observed-update ordering, and invalid inputs MUST fail visibly without manufacturing an outcome or overwriting canonical inputs.

### Requirement: Supervised finite publication
r[kiln.dashboard.live.supervision]
Matching installed collector/publisher executables and an operator-owned user service/timer MUST provide finite serialized observation/publication cycles to the explicit Aspen3 HTML destination, without transferring canonical journals or CI credentials, queueing missed ticks, or granting provider execution authority.

#### Scenario: Service installation and cadence
r[kiln.dashboard.live.supervision.scenario.cadence]
- GIVEN reviewed explicit configuration and immutable tools on the journal-owning host
- WHEN the new dashboard service/timer starts or is stopped
- THEN only its named units MUST change, each cycle MUST remain bounded, and an overlapping cycle MUST NOT publish concurrently.

### Requirement: Last-good recovery
r[kiln.dashboard.live.recovery]
Collection, renderer or transfer failures MUST preserve last-good self-contained HTML; restart MUST rebuild solely from durable canonical inputs and exact configuration without redispatching CI.

#### Scenario: Failure and restart
r[kiln.dashboard.live.recovery.scenario.restart]
- GIVEN a delivered valid report followed by invalid observation/generation/transfer inputs or a stopped publisher
- WHEN publication fails and the operator restores inputs or restarts supervision
- THEN failure MUST remain visible, the prior report MUST remain readable, and recovery MUST show the admitted current histories without changing CI execution identities.

### Requirement: Fresh real CI proof
r[kiln.dashboard.live.fresh]
Operated verification MUST use newly executed dedicated Kiln fork self-CI success and genuine failure through the maintained broker/controller/worker path and observe their newly written canonical histories on the actual Aspen3 report.

#### Scenario: Exact fresh outcomes
r[kiln.dashboard.live.fresh.scenario.outcomes]
- GIVEN exact new source revisions and a measured runtime/profile cohort
- WHEN the real positive and negative builds finish
- THEN the dashboard MUST display their exact admitted outcomes and revisions, retain unresolved observations, and distinguish these fresh runs from historical retained evidence.

### Requirement: Evidence and authority scope
r[kiln.dashboard.live.claims]
Receipts MUST bind actual source/tool/profile identities, exercised commands, report bytes, failures and unavailable prerequisites. They MUST NOT promote local canary or observation results into canonical governance, arbitrary CI correctness, hostile-worker confinement, newest-source completeness, production rollout or release eligibility.

#### Scenario: Partial prerequisites are not fabricated success
r[kiln.dashboard.live.claims.scenario.scope]
- GIVEN a missing or stale tool, host, credential or provider capability
- WHEN verification is recorded
- THEN the missing prerequisite MUST be named precisely and retained history MUST NOT be substituted for an unexecuted fresh proof; unrelated Patchgate and release holds MUST remain visible.

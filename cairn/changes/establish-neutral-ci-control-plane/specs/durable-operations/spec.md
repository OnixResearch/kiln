# Durable Operations Specification

## ADDED Requirements

### Requirement: Typed Nickel configuration
r[kiln.operations.configuration] Human-authored project, provider, capability, fairness, retry, retention, and operator policy MUST use typed Nickel contracts and deterministic generated projections.

#### Scenario: Policy omits a required bound
r[kiln.operations.configuration.scenario.missing]
- GIVEN a provider policy without its required timeout or attempt bound
- WHEN Nickel contract evaluation runs
- THEN configuration admission MUST fail before service startup.

### Requirement: Versioned append-only journal
r[kiln.operations.journal] Accepted domain events MUST enter a versioned append-only journal with stream identity, expected revision, event identity, and canonical event bytes.

#### Scenario: Writer uses a stale expected revision
r[kiln.operations.journal.scenario.stale]
- GIVEN a stream with a newer committed revision
- WHEN a writer tries to append with an older expected revision
- THEN the append MUST fail without partial journal mutation.

### Requirement: Atomic event and outbox recording
r[kiln.operations.outbox] A shell MUST record accepted events and their requested effects in one local atomic commit before dispatch, and each effect MUST have a lease and idempotency identity.

#### Scenario: Process stops before commit
r[kiln.operations.outbox.scenario.before]
- GIVEN accepted in-memory events and effects
- WHEN the process stops before the local commit succeeds
- THEN neither the events nor the effects MUST appear as committed work.

### Requirement: Deterministic replay without dispatch
r[kiln.operations.replay] Replay MUST validate and fold canonical events through the production evolution relation and MUST emit no new provider effect.

#### Scenario: Historical build request is replayed
r[kiln.operations.replay.scenario.effect]
- GIVEN a valid history that originally requested a build effect
- WHEN recovery replay reconstructs state
- THEN replay MUST reconstruct the state and MUST NOT return a dispatchable build request.

### Requirement: Replay compatibility gate
r[kiln.operations.replay_compatibility] Every supported historical event and transition version MUST replay through the current production evolution path or an explicit deterministic migration, while unsupported or divergent history MUST fail before service admission.

#### Scenario: Transition code changes historical meaning
r[kiln.operations.replay_compatibility.scenario.divergence]
- GIVEN a retained canonical history and a changed transition implementation
- WHEN replay compatibility compares the rebuilt state and recorded checkpoints
- THEN release admission MUST fail if the supported history produces a different result.

### Requirement: Explicit recovery and reconciliation
r[kiln.operations.recovery] Restart recovery MUST classify unclaimed, leased, expired, completed, and ambiguous effects and MUST require provider reconciliation or operator policy before redispatching ambiguous work.

#### Scenario: Lease expires after possible submission
r[kiln.operations.recovery.scenario.expired]
- GIVEN an expired effect lease with no terminal provider observation
- WHEN recovery runs
- THEN the effect MUST enter reconciliation and MUST NOT be treated as never submitted.

### Requirement: Deterministic weighted admission
r[kiln.operations.fairness] Kiln MUST order eligible tenants from recent normalized resource use divided by configured share, using named accounting, decay, rounding, stale-data, and tie-break policy.

#### Scenario: Two tenants have different shares
r[kiln.operations.fairness.scenario.shares]
- GIVEN equal recent normalized use and unequal positive configured shares
- WHEN pure tenant admission runs
- THEN the tenant with the lower use-to-share score MUST rank first.

### Requirement: Named fairness strategy
r[kiln.operations.fairness_strategy] The fairness profile MUST name its resource-dimension aggregation strategy, including weighted scalar or dominant-share behavior, and MUST reject dimensions that the selected strategy cannot compare.

#### Scenario: Dominant-share policy lacks capacity basis
r[kiln.operations.fairness_strategy.scenario.capacity]
- GIVEN a dominant-share profile without an admitted normalization or capacity basis for one dimension
- WHEN fairness admission runs
- THEN it MUST reject that usage fact instead of guessing a comparable share.

### Requirement: Fairness invariants
r[kiln.operations.fairness_invariants] Holding all other admitted facts equal, additional positive usage for one tenant MUST NOT improve that tenant's priority under the selected strategy, and deterministic ties MUST use the configured tie-break rule.

#### Scenario: Tenant usage increases
r[kiln.operations.fairness_invariants.scenario.monotonic]
- GIVEN two equivalent fairness states except for additional admitted usage by one tenant
- WHEN both scores are calculated
- THEN the tenant with additional usage MUST NOT receive a better priority from that difference.

### Requirement: Typed normalized usage facts
r[kiln.operations.usage] Resource usage observations MUST name provider, resource dimensions, units, measurement interval, normalization profile, freshness, and exact producing attempts.

#### Scenario: Provider reports an unsupported unit
r[kiln.operations.usage.scenario.unit]
- GIVEN a usage observation with a unit absent from the selected normalization profile
- WHEN fairness admission runs
- THEN the observation MUST be rejected or handled by the explicit stale-usage policy.

### Requirement: Rebuildable operator projections
r[kiln.operations.projections] CLI, API, status, metrics, logs, and notification views MUST be rebuildable from admitted events and observations and MUST NOT become mutation authority.

#### Scenario: Status database is removed
r[kiln.operations.projections.scenario.rebuild]
- GIVEN an intact canonical journal and a missing derived status database
- WHEN projection rebuild runs
- THEN it MUST reproduce the same bounded status rows without provider dispatch.

### Requirement: Positive and negative operations fixtures
r[kiln.operations.fixtures] The maintained operations suite MUST cover valid replay, recovery, deduplication, fairness, and projection cases plus corruption, stale lease, invalid share, unsupported unit, time regression, and interruption cases.

#### Scenario: Outbox interruption fixture is absent
r[kiln.operations.fixtures.scenario.interruption]
- GIVEN a release candidate without interruption tests around the event and outbox commit
- WHEN operations readiness runs
- THEN the candidate MUST fail.

### Requirement: Operations non-claims
r[kiln.operations.nonclaims] Journal, outbox, lease, deduplication, replay, and fairness checks MUST NOT claim exactly-once external effects, provider durability, global fairness, wall-clock truth, or release eligibility.

#### Scenario: Duplicate callback is suppressed
r[kiln.operations.nonclaims.scenario.dedup]
- GIVEN a duplicate callback suppressed under the retained journal history
- WHEN evidence is reported
- THEN the report MUST claim only bounded duplicate suppression under that store and retention policy.

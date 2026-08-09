# SQLite Event Store Adapter Delta Specification

## ADDED Requirements

### Requirement: Typed SQLite profile
r[kiln.sqlite.profile] Kiln MUST admit SQLite through typed configuration with exact library, schema, transaction, durability, bound, backup, and recovery facts.

#### Scenario: Profile is complete
r[kiln.sqlite.profile.scenario.valid]
- GIVEN a complete supported SQLite profile
- WHEN store admission runs
- THEN Kiln MAY open the configured database with the declared capabilities.

#### Scenario: Durability settings conflict
r[kiln.sqlite.profile.scenario.conflict]
- GIVEN a profile with contradictory transaction or synchronization settings
- WHEN store admission runs
- THEN Kiln MUST reject it before opening the database.

### Requirement: Atomic event and outbox append
r[kiln.sqlite.atomic_append] Accepted events and their requested outbox effects MUST commit in one transaction at one expected stream revision.

#### Scenario: Transaction commits
r[kiln.sqlite.atomic_append.scenario.commit]
- GIVEN an admitted event batch and linked effects at the expected revision
- WHEN the transaction commits
- THEN later reads MUST expose both the events and outbox records.

#### Scenario: Transaction stops after event writes
r[kiln.sqlite.atomic_append.scenario.interrupt]
- GIVEN an interruption before the transaction commit
- WHEN recovery opens the database
- THEN it MUST expose neither the partial events nor their effects.

### Requirement: Identity-checked replay
r[kiln.sqlite.replay] Replay MUST read bounded canonical event bytes in order and validate versions, identities, and sequence continuity.

#### Scenario: History is intact
r[kiln.sqlite.replay.scenario.valid]
- GIVEN a supported continuous event history
- WHEN replay runs
- THEN it MUST rebuild state through the production evolution path without dispatching effects.

#### Scenario: Stored bytes differ from identity
r[kiln.sqlite.replay.scenario.damage]
- GIVEN a row whose canonical bytes do not match its stored identity
- WHEN replay admission runs
- THEN Kiln MUST stop and report store damage.

### Requirement: Durable outbox leases
r[kiln.sqlite.leases] Outbox leases MUST preserve effect, owner, state, bound, attempt, provider, and reconciliation facts across restart.

#### Scenario: Lease remains active
r[kiln.sqlite.leases.scenario.restart]
- GIVEN an active committed lease and a shell restart
- WHEN recovery reads the outbox
- THEN it MUST preserve the lease state without redispatch.

#### Scenario: Lease is ambiguous
r[kiln.sqlite.leases.scenario.ambiguous]
- GIVEN an expired effect that can have reached its provider
- WHEN lease classification runs
- THEN Kiln MUST require reconciliation before redispatch.

### Requirement: Explicit SQLite migrations
r[kiln.sqlite.migrations] Schema migrations MUST use ordered deterministic steps with version preflight, transactional mutation, and postflight checks.

#### Scenario: Supported migration completes
r[kiln.sqlite.migrations.scenario.valid]
- GIVEN a supported prior schema and migration plan
- WHEN migration executes
- THEN the database MUST record the new version and complete history atomically.

#### Scenario: Database version is unknown
r[kiln.sqlite.migrations.scenario.unknown]
- GIVEN an unsupported or partially migrated schema
- WHEN service admission runs
- THEN Kiln MUST fail without destructive repair.

### Requirement: SQLite adapter conformance
r[kiln.sqlite.conformance] Maintained tests MUST include real restart, lock, crash, corruption, migration, backup, and bound fixtures.

#### Scenario: Crash fixture is absent
r[kiln.sqlite.conformance.scenario.crash]
- GIVEN an adapter candidate without interrupted-transaction evidence
- WHEN adapter readiness runs
- THEN the candidate MUST fail.

### Requirement: SQLite adapter non-claims
r[kiln.sqlite.nonclaims] Passing checks MUST NOT claim hardware durability, filesystem correctness, disaster recovery, or exactly-once provider execution.

#### Scenario: Commit returns success
r[kiln.sqlite.nonclaims.scenario.scope]
- GIVEN a successful SQLite commit under the selected policy
- WHEN Kiln records evidence
- THEN it MUST report only the checked database, transaction, setting, and filesystem observations.

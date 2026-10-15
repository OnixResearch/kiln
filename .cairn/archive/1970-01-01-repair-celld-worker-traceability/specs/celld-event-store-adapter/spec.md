# Celld Worker Evidence Delta

## ADDED Requirements

### Requirement: Executable local Worker evidence has bounded authority

r[kiln.celld.local_worker_evidence] Kiln MUST test its owned Worker against actual local SQLite transactions, checked signed requests, and fault-injected rollback before calling those local properties verified. It MUST distinguish local Worker/SQLite observations from a pinned celld fleet's output gate, storage probe, ownership, and remote durability. Accepted Celld scenarios MUST NOT gain source verification markers from model-only or tautological fixtures.

#### Scenario: Transaction aborts before commit

r[kiln.celld.local_worker_evidence.scenario.rollback]
- GIVEN a signed bounded event and linked outbox append against an initialized local Worker object
- WHEN SQLite fails before the Worker transaction commits
- THEN a subsequent read MUST show neither a new event nor a new outbox result.

#### Scenario: Runtime cohort has not been exercised

r[kiln.celld.local_worker_evidence.scenario.runtime_gap]
- GIVEN only local Worker/SQLite or in-memory model results
- WHEN Celld evidence is recorded
- THEN it MUST NOT claim the pinned v0.2.1 executable, output gate, storage probe, fleet takeover or distributed durability was verified.

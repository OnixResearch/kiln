# celld Event Store Adapter Delta Specification

## ADDED Requirements

### Requirement: Optional adapter and dependency boundary

r[kiln.celld.dependency_boundary] Kiln MUST implement celld support as an optional `EventStorePort` adapter. Core crates MUST NOT import celld or Worker runtime types.

#### Scenario: Adapter is not configured

r[kiln.celld.dependency_boundary.scenario.optional]
- GIVEN a Kiln deployment without an admitted celld profile
- WHEN core, provider, and standalone checks run
- THEN they MUST pass without celld, V8, esbuild, or object-store credentials.

#### Scenario: Provider types enter the core

r[kiln.celld.dependency_boundary.scenario.import]
- GIVEN a core or application contract imports a celld, Worker, V8, Wrangler, or object-store implementation type
- WHEN the dependency boundary gate runs
- THEN the gate MUST fail and identify the forbidden dependency edge.

### Requirement: Exact runtime and deployment cohort

r[kiln.celld.cohort] The adapter MUST admit an exact celld release, revision, platform artifact, Worker protocol, deployment, schema, storage cohort, and ingress profile.

It MUST require enabled output gating and a successful conditional-write storage probe. Missing or conflicting observations MUST make the adapter unavailable.

#### Scenario: Exact cohort is available

r[kiln.celld.cohort.scenario.valid]
- GIVEN the celld binary, Worker deployment, schema, storage probe, output gate, and ingress facts match the selected profile
- WHEN runtime admission runs
- THEN the adapter MAY accept event-store requests for that exact cohort.

#### Scenario: Output gating is disabled

r[kiln.celld.cohort.scenario.output_gate]
- GIVEN a celld node reports or declares `CELLD_OUTPUT_GATE=0`
- WHEN runtime admission runs
- THEN the adapter MUST reject the cohort before any canonical append.

### Requirement: Deterministic single-object partitioning

r[kiln.celld.partitioning] Each admitted journal partition MUST map deterministically to one named Durable Object from an explicit versioned projection and BLAKE3 identity.

One mutation MUST NOT span two objects. celld placement and activation order MUST NOT become Kiln fairness or dispatch authority.

#### Scenario: One stream maps to one object

r[kiln.celld.partitioning.scenario.stable]
- GIVEN equal admitted stream and partition-policy facts
- WHEN partition mapping runs twice
- THEN both results MUST select the same Kiln partition identity and celld object name.

#### Scenario: Mutation names two partitions

r[kiln.celld.partitioning.scenario.cross_partition]
- GIVEN an append request that includes events or outbox records for two partitions
- WHEN partition admission runs
- THEN it MUST reject before transport dispatch and MUST create no canonical record.

### Requirement: Atomic event and outbox append

r[kiln.celld.atomic_append] The Worker MUST store an admitted event batch and every linked outbox record in one Durable Object SQLite transaction.

The transaction MUST bind the stream, expected revision, operation, payload, event, effect, protocol, and schema identities.

#### Scenario: Append commits

r[kiln.celld.atomic_append.scenario.commit]
- GIVEN an admitted bounded batch at the current expected revision
- WHEN the Worker transaction commits and the output gate acknowledges it
- THEN later reads MUST expose the events, new revision, outbox records, and idempotency result together.

#### Scenario: Transaction stops after event writes

r[kiln.celld.atomic_append.scenario.interrupt]
- GIVEN the Worker stops or SQLite fails before transaction commit
- WHEN the object recovers
- THEN it MUST expose neither the partial events nor their linked outbox records.

### Requirement: Authenticated bounded transport

r[kiln.celld.transport] The adapter MUST use one admitted Worker origin, path, TLS policy, authentication profile, and named request and response bounds.

The initial contract MUST use HMAC-SHA256 request authentication with shell-owned secret references. Redirects and request-supplied destinations MUST remain disabled.

#### Scenario: Authenticated request is valid

r[kiln.celld.transport.scenario.valid]
- GIVEN a bounded request with an admitted key, timestamp, nonce, body digest, and signature
- WHEN the Worker verifies the envelope
- THEN it MAY resolve the admitted partition and process the operation.

#### Scenario: Destination or signature is invalid

r[kiln.celld.transport.scenario.rejected]
- GIVEN a request changes the configured origin, follows a redirect, replays a nonce, or has an invalid signature
- WHEN transport or Worker admission runs
- THEN it MUST reject before a Durable Object mutation.

### Requirement: Idempotent mutation and explicit ambiguity

r[kiln.celld.ambiguity] Every mutation MUST bind one operation identity and one BLAKE3 payload identity. Ambiguous acceptance MUST require exact operation reconciliation before redispatch.

#### Scenario: Equal operation is repeated

r[kiln.celld.ambiguity.scenario.repeat]
- GIVEN an operation identity already has a stored terminal result for the same payload identity
- WHEN the same request arrives again
- THEN the Worker MUST return the stored result without another canonical mutation.

#### Scenario: Client loses the response

r[kiln.celld.ambiguity.scenario.disconnect]
- GIVEN transport ends after the Worker can have committed the mutation
- WHEN the client classifies the outcome
- THEN it MUST return `Unknown` and reconcile the exact operation before redispatch.

### Requirement: Identity-checked bounded replay

r[kiln.celld.replay] Reads MUST return bounded ordered canonical rows with schema, sequence, revision, and stored identity facts.

The client and core MUST check framing, continuity, canonical identities, and replay versions before production state evolution.

#### Scenario: History is intact

r[kiln.celld.replay.scenario.valid]
- GIVEN a supported continuous history within every page and byte bound
- WHEN replay runs
- THEN it MUST rebuild state through production `evolve` and MUST emit no provider effect.

#### Scenario: Stored bytes differ from identity

r[kiln.celld.replay.scenario.damage]
- GIVEN a stored event row whose canonical bytes do not match its recorded identity
- WHEN replay admission runs
- THEN it MUST stop and report store damage without using a snapshot as replacement history.

### Requirement: Durable leases and reconciliation

r[kiln.celld.leases] The adapter MUST preserve outbox owner, lease, attempt, provider, idempotency, completion, and reconciliation facts across object eviction and node restart.

Time facts and lease decisions MUST come from the Kiln shell and pure core. The Worker MUST only apply an admitted conditional plan.

#### Scenario: Active lease survives restart

r[kiln.celld.leases.scenario.restart]
- GIVEN a committed active lease and a celld node restart or ownership takeover
- WHEN the object restores
- THEN it MUST preserve the lease and MUST NOT redispatch the effect automatically.

#### Scenario: Lease outcome is ambiguous

r[kiln.celld.leases.scenario.ambiguous]
- GIVEN a lease can have reached its provider without a terminal observation
- WHEN recovery classification runs
- THEN Kiln MUST require provider reconciliation before another dispatch.

### Requirement: Explicit Worker schema migration

r[kiln.celld.migrations] Worker schema migrations MUST use versioned pure plans, admitted edges, exact input identities, one-object transactions, and postflight checks.

Unknown, partial, destructive, and cross-object migrations MUST fail closed.

#### Scenario: Supported migration completes

r[kiln.celld.migrations.scenario.valid]
- GIVEN a supported prior schema and an admitted migration plan
- WHEN one object applies the migration
- THEN it MUST record the new schema and migration identity in the same transaction.

#### Scenario: Deployment mixes incompatible schemas

r[kiln.celld.migrations.scenario.mixed]
- GIVEN the fleet can route requests to incompatible Worker or schema cohorts
- WHEN adapter admission or response admission observes the mismatch
- THEN it MUST stop canonical mutations until the operator restores a compatible cohort.

### Requirement: Trust and authority boundaries

r[kiln.celld.authority] The initial profile MUST limit one fleet to one declared trust domain. It MUST NOT claim hostile tenant isolation.

Normal event-store requests MUST NOT use the internal operator API or fleet-bucket credentials. Deployment and fleet mutation remain separate operator authority.

#### Scenario: Untrusted tenants share a profile

r[kiln.celld.authority.scenario.tenant]
- GIVEN a profile declares hostile tenants in one celld alpha fleet
- WHEN security admission runs
- THEN it MUST reject the profile before service startup.

#### Scenario: Adapter tries a private control route

r[kiln.celld.authority.scenario.operator_api]
- GIVEN a normal event-store operation targets `/state`, `/do/NAME`, `/evict/NAME`, or `/shutdown`
- WHEN transport admission runs
- THEN it MUST reject the route as outside `EventStorePort` authority.

### Requirement: Adapter conformance and non-claims

r[kiln.celld.conformance] The adapter MUST pass positive and negative core, client, Worker, transaction, restart, takeover, authentication, bound, migration, and exact-cohort fixtures.

#### Scenario: Negative fixture fails at another boundary

r[kiln.celld.conformance.scenario.wrong_boundary]
- GIVEN an invalid-signature fixture fails only because the object store is unavailable
- WHEN conformance classifies the result
- THEN the fixture MUST NOT count as authentication-rejection evidence.

r[kiln.celld.nonclaims] Passing checks MUST NOT claim object-store correctness, hostile isolation, global serializability, exactly-once effects, CI success, or release eligibility.

#### Scenario: celld acknowledges an append

r[kiln.celld.nonclaims.scenario.acknowledged]
- GIVEN the pinned cohort acknowledges one Worker append
- WHEN Kiln records compatibility evidence
- THEN the evidence MUST stay limited to that request, partition, transaction, deployment, runtime, storage observations, and checked identities.

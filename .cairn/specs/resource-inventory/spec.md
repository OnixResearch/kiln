# Resource Inventory Specification

## Purpose

Defines the `resource-inventory` capability.

## Requirements

### Requirement: Separate observed resource inventories
r[kiln.inventory.separation]
Kiln MUST expose agent definitions, actual worker instances, compute/shared pools and model services as distinct bounded versioned observations. Exact producer-scoped references, freshness, generation and completeness MUST remain visible. Missing, stale, out-of-order, partial or ambiguous facts MUST NOT establish readiness or free capacity. Shared pool capacity MUST NOT be double counted. Revision rollout, applicable qualification, execution grants, inference readiness and request admission MUST remain independent facts.

#### Scenario: Exact task explanation
- GIVEN exact admitted work and authorized producer observations
- WHEN read-only readiness is explained
- THEN unsupported tools, context, backend, sandbox, policy, capacity, evidence and admission reasons MUST be explicit and deterministic; successful explanation MUST NOT grant dispatch

### Requirement: Read-only consistent consumers
r[kiln.inventory.consumers]
Live, CLI and offline HTML MUST consume the same admitted projection, identities and redaction. Offline rendering MUST perform no discovery, network access or mutation. The console MUST preserve accessibility, inert text, unknown states and configured-slot v1 behavior.

#### Scenario: Missing provider
- GIVEN a provider is missing or stale
- WHEN an operator reads inventories
- THEN the surface MUST report unknown or incomplete rather than an empty fleet or zero consumption

### Requirement: Producer observation intake

r[kiln.inventory.producers] Kiln MUST map Lattice `lattice.worker-inventory.v1`, compute `lattice.compute-allocation-observation.v1` and Animus `animus.serving-availability-snapshot.v1` documents into inventory v2 facts only through provider adapters, preserving exact producer identities. Every v2 fact a producer does not publish MUST be listed as unknown. A configured profile MUST NOT become a worker instance, and no producer lifecycle MUST map to idle, healthy or executing unless the producer observed it. Unsupported, malformed, conflicting and ambiguous documents MUST be refused before projection.

#### Scenario: Profiles without instances stay definitions

r[kiln.inventory.producers.scenario.lattice]
- GIVEN Lattice inventory documents for one profile with and without a retained instance
- WHEN Kiln maps them
- THEN Kiln MUST report one definition, only the retained instances as workers, none of them idle, and unpublished facts as unknown

#### Scenario: Shared allocations and pools are counted once

r[kiln.inventory.producers.scenario.compute]
- GIVEN two execution allocations that reference one serving deployment and a pool claimed by both execution and serving
- WHEN Kiln maps the compute observations
- THEN the deployment's claims MUST appear once, the pool MUST appear once with every claim, pool capacity MUST stay unknown, and one accounting authority under two pool identities MUST be refused

#### Scenario: Serving states stay distinct and claimed

r[kiln.inventory.producers.scenario.serving]
- GIVEN serving services that are loading, ready, ready with no request slots, hosted without placement, or stale
- WHEN Kiln maps the snapshot
- THEN their states, request capacity and placement MUST stay distinct, endpoint identity MUST be withheld, a model identity MUST remain a claim, and a stale or unclocked snapshot MUST be unknown

#### Scenario: Invalid producer documents refuse

r[kiln.inventory.producers.scenario.refused]
- GIVEN an unsupported schema, an unknown field, an inconsistent record, a conflicting profile, an ambiguous accounting identity or a replayed sequence with different content
- WHEN Kiln maps it
- THEN mapping MUST fail with a named refusal and no fact

### Requirement: Explicit producer collection

r[kiln.inventory.collection] A collecting shell MUST read only the producer documents a `kiln.inventory.collection-set.v1` manifest names relative to itself, MUST compose them into one v2 snapshot by exact producer-scoped joins and never by host name, and MUST give the CLI, the live API and offline HTML the same projection identity and redaction. Composition MUST perform no discovery, network access or mutation.

#### Scenario: Same projection everywhere

r[kiln.inventory.collection.scenario.parity]
- GIVEN one collected producer snapshot
- WHEN the CLI, the live API reader and offline HTML project it
- THEN they MUST report one projection identity, and the HTML MUST contain no script or remote resource

#### Scenario: Host names never join

r[kiln.inventory.collection.scenario.hostname]
- GIVEN a host-local worker and a compute allocation on hosts with the same name
- WHEN the snapshot is composed
- THEN the worker MUST NOT reference that allocation and its execution MUST remain unknown

#### Scenario: Unsafe manifests refuse

r[kiln.inventory.collection.scenario.refused]
- GIVEN a manifest with an absolute or parent-relative document path, an unsupported producer contract, an unknown field or an oversized document
- WHEN the shell collects it
- THEN collection MUST fail before projection

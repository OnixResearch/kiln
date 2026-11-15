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

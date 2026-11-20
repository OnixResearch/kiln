## MODIFIED Requirements

### Requirement: Producer observation intake

r[kiln.inventory.producers] Kiln MUST map Lattice `lattice.worker-inventory.v2`, compute `lattice.compute-allocation-observation.v1` and Animus `animus.serving-availability-snapshot.v1` documents into inventory v2 facts only through provider adapters, preserving exact producer identities. Every v2 fact a producer does not publish MUST be listed as unknown, and a fact the producer declares unavailable MUST be a known negative. A configured profile MUST NOT become a worker instance, and no producer lifecycle or liveness MUST map to idle, healthy or executing unless the producer observed it. Producer wait reasons MUST keep their exact code, subject and owner and MUST NOT carry free text. Unsupported, malformed, conflicting and ambiguous documents, including `lattice.worker-inventory.v1`, MUST be refused before projection.

#### Scenario: Profiles without instances stay definitions

r[kiln.inventory.producers.scenario.lattice]
- GIVEN Lattice inventory documents for one definition at one revision with and without a retained instance
- WHEN Kiln maps them
- THEN Kiln MUST report one definition keyed by its definition identity apart from its revision, only the retained instances as workers joined by that identity with their attempt, fence and work unchanged, none of them idle or live, and unpublished facts such as revision slots and unobserved liveness as unknown

#### Scenario: Declared capability is known

r[kiln.inventory.producers.scenario.capability]
- GIVEN profiles that declare, mark unavailable or leave undeclared their task families and tools
- WHEN Kiln explains readiness for exact work
- THEN declared names MUST satisfy or block with the exact unmet name, unavailable capability MUST block as a known negative, and undeclared capability MUST stay unknown and never be reported as unsupported

#### Scenario: Producer wait reasons keep their owner

r[kiln.inventory.producers.scenario.wait]
- GIVEN producer wait reasons on a profile or an instance
- WHEN Kiln projects the snapshot and explains readiness
- THEN each reason MUST appear on its fact and as an explanation blocker with its exact code, subject and owner, and the fact it names MUST NOT be observed eligible

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
- GIVEN an unsupported schema such as a v1 Lattice inventory, an unknown field, an out-of-vocabulary code, support value or liveness state, a name over its bound, an inconsistent record, a definition identity that differs between profile and instance, one definition at two revisions, a conflicting profile, an ambiguous accounting identity or a replayed sequence with different content
- WHEN Kiln maps it
- THEN mapping MUST fail with a named refusal and no fact

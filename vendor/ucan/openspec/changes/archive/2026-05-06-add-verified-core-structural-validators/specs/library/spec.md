## ADDED Requirements

### Requirement: Verified core structural validators [r[library.verified-core-structural-validators]]
The library MUST provide verified-logic mirrors or bridge predicates for deterministic `ucan-core` structural validators that admit DID claim shape, capability/proof/caveat list bounds, required temporal fields, and temporal ordering.

#### Scenario: DID and required-claim shape is verified [r[library.verified-core-structural-validators.scenario.did-and-required-claims]]
- GIVEN decoded issuer and audience claim strings plus required claim-presence facts
- WHEN core structural validation admits those claims
- THEN the library MUST route present, non-empty, trimmed, and required-field facts through verified logic
- AND invalid claim shape MUST fail closed before constructing accepted core claims

#### Scenario: List and proof-reference bounds are verified [r[library.verified-core-structural-validators.scenario.list-and-proof-bounds]]
- GIVEN decoded capability, caveat, and proof-reference collections
- WHEN core validation enforces count, emptiness, and encoded-input bounds
- THEN the library MUST route those finite collection-shape facts through verified logic
- AND over-limit or malformed collections MUST fail closed before proof traversal or authorization

#### Scenario: Temporal order admission is verified [r[library.verified-core-structural-validators.scenario.temporal-order]]
- GIVEN decoded `nbf` and `exp` values and required-field presence facts
- WHEN core validation checks temporal order
- THEN the library MUST route `nbf < exp` and required-field admission through verified logic
- AND wall-clock freshness and clock-source correctness MUST remain outside that structural claim

#### Scenario: Core validator verification preserves no_std boundary [r[library.verified-core-structural-validators.scenario.no-std-boundary]]
- GIVEN verified mirrors for core structural validators
- WHEN the repository verifies and tests the core crate
- THEN `ucan-core` MUST remain `#![no_std]`
- AND verified mirrors MUST NOT introduce filesystem, network, clock, allocator, or consumer-specific dependencies into the core crate

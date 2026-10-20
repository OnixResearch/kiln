## MODIFIED Requirements

### Requirement: Delegation proof chains are verified end to end
The library MUST verify delegation proof chains across parent tokens, including issuer/audience linkage, temporal narrowing, capability attenuation, and opaque parent caveat preservation. Resource/ability containment and parent caveat identifier preservation MUST route through vendored verified-logic bridges before a child capability is accepted as delegated.

#### Scenario: Opaque parent caveat preservation uses verified bridge [r[library.delegation-chain.scenario.caveat-preservation-admission]]
- GIVEN a parent delegated capability and a child capability whose resource and ability are contained by that parent
- WHEN the shell decides whether the child preserves every opaque parent caveat
- THEN the shell MUST route the parent and child caveat identifier lists through a vendored verified caveat-preservation bridge
- AND the child MUST be rejected if any parent caveat identifier is absent from the child list
- AND the child MAY include additional caveat identifiers without failing preservation

#### Scenario: Caveat preservation keeps opaque semantics outside the verified claim [r[library.delegation-chain.scenario.caveat-preservation-boundary-limits]]
- GIVEN caveat identifiers admitted by the verified preservation predicate
- WHEN the shell later evaluates caveat policies or relies on caveat payload meaning
- THEN the library MUST continue to document payload parsing, policy semantics, identifier construction provenance, clocks, counters, and backend facts as outside the caveat-preservation admission claim

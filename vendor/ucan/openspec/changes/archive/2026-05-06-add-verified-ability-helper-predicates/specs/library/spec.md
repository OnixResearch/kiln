## ADDED Requirements

### Requirement: Verified ability helper predicates [r[library.verified-ability-helper-predicates]]
The library MUST route wildcard-segment detection and concrete ability classification through vendored verified-logic helpers when those facts affect public validated wrappers or authorization helper behavior.

#### Scenario: Wildcard segment detection is verified [r[library.verified-ability-helper-predicates.scenario.wildcard-segment]]
- GIVEN an ability path represented as finite path segments
- WHEN the shell or a public helper classifies whether the ability contains a wildcard segment
- THEN the classification MUST route through a verified ability helper predicate
- AND only a full `*` segment MUST be treated as a wildcard segment

#### Scenario: Concrete ability classification is verified [r[library.verified-ability-helper-predicates.scenario.concrete-ability]]
- GIVEN an ability path used for invocation or consumer adapter mapping
- WHEN the library classifies the ability as concrete rather than wildcard-containing
- THEN the classification MUST route through a verified helper
- AND malformed wildcard substrings inside non-wildcard segments MUST NOT be treated as valid wildcard segments

#### Scenario: Ability helpers keep consumer policy outside the claim [r[library.verified-ability-helper-predicates.scenario.boundary-limits]]
- GIVEN ability-helper facts admitted by verified logic
- WHEN Aspen, Clankers, or another consumer maps UCAN abilities into local authorization policy
- THEN the library MUST document that consumer-specific resource/action/session semantics remain outside the verified helper claim and outside the `ucan` crate

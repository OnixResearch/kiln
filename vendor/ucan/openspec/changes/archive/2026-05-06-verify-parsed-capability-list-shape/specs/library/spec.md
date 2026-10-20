## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST admit parsed attenuation capability lists only when verified finite list-shape facts show a non-empty list and resource/ability presence for every capability entry before capability construction continues.

#### Scenario: Parsed capability list shape uses verified bridge [r[library.compact-token.scenario.parsed-capability-list-shape-admission]]

- GIVEN serde-decoded UCAN payload claims with a present `att` list
- WHEN the shell constructs capability documents
- THEN it MUST route non-empty-list and per-entry resource/ability presence facts through a vendored verified capability-list-shape bridge
- AND missing resource or ability in any parsed entry MUST reject before authorization

#### Scenario: Parsed capability list shape keeps capability semantics outside the verified claim [r[library.compact-token.scenario.parsed-capability-list-shape-boundary-limits]]

- GIVEN parsed capability-list shape facts admitted by the verified predicate
- WHEN the shell later validates resource, ability, attenuation, caveats, proof traversal, or authorization
- THEN the library MUST continue to document resource string semantics, wildcard ability semantics, caveat payload meaning, parser correctness, and authorization policy as outside the parsed capability-list-shape admission claim

## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST admit parsed opaque caveat lists only when verified finite entry-shape facts preserve the identifier inputs and payload presence used by downstream caveat-preservation checks.

#### Scenario: Parsed caveat list shape uses verified bridge [r[library.compact-token.scenario.parsed-caveat-list-shape-admission]]

- GIVEN parsed capability caveat entries supplied by serde and the shell
- WHEN the shell constructs caveat identifiers or prepares caveat-preservation inputs
- THEN it MUST route finite per-entry identifier-input and payload-presence facts through a vendored verified caveat-list-shape bridge
- AND malformed parsed caveat entries MUST reject before preservation or policy evaluation

#### Scenario: Parsed caveat list shape keeps caveat semantics outside the verified claim [r[library.compact-token.scenario.parsed-caveat-list-shape-boundary-limits]]

- GIVEN parsed caveat-list shape facts admitted by the verified predicate
- WHEN the shell later evaluates caveat policies or relies on caveat payload meaning
- THEN the library MUST continue to document payload parsing, application caveat semantics, clock/count provenance, policy lookup truth, identifier encoding provenance, and backend facts as outside the parsed caveat-list-shape admission claim

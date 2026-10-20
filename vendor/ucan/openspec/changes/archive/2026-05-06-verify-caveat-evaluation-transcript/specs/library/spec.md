## MODIFIED Requirements

### Requirement: Caveat policies fail closed
Caveat evaluation MUST use explicit caller-provided policy hooks and MUST fail closed when caveats are unknown or unevaluated. Caveat aggregation MUST fail closed when a policy is missing or rejects a caveat, and finite aggregation MUST route through a vendored verified transcript bridge.

#### Scenario: Caveat evaluation transcript admits only all-satisfied facts [r[library.invocation.scenario.caveat-evaluation-transcript-admission]]
- GIVEN a matched capability with zero or more caveats
- WHEN the shell emits one caveat evaluation fact per caveat
- THEN authorization MAY proceed only if every fact has `policy_present` true and `satisfied` true according to the vendored verified transcript predicate
- AND the first missing or rejected caveat MUST be reported in caveat order

#### Scenario: Caveat evaluation transcript keeps policy semantics outside the verified claim [r[library.invocation.scenario.caveat-evaluation-transcript-boundary-limits]]
- GIVEN caveat evaluation facts admitted by the verified transcript predicate
- WHEN a caller supplies caveat policies or typed caveat contexts
- THEN the library MUST continue to document policy lookup truth, arbitrary policy correctness, typed payload parsing, clocks, counters, freshness, token provenance, and cryptography as outside the caveat-evaluation transcript claim

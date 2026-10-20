## MODIFIED Requirements

### Requirement: Authorization uses verified capability semantics

Authorization matching and concrete request admission MUST reuse the same verified-logic-backed resource and ability containment semantics used for proof-chain attenuation.

#### Scenario: Authorization requests are concrete operations

- GIVEN an authorization request whose requested ability uses `*` as a full wildcard segment
- WHEN the request is constructed or evaluated
- THEN the library MUST reject the request with a structured validation error
- AND wildcard abilities MUST remain valid only on delegated capabilities, not concrete requested operations
- AND the wildcard check MUST be routed through the verified capability bridge rather than a parallel shell-local ability parser

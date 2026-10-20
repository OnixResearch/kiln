## ADDED Requirements

### Requirement: Verified tokens authorize concrete requests

The `ucan` crate MUST let consumers ask whether a verified token's effective delegation authorizes a concrete resource and ability.
r[library.authorization-decisions.authorize-concrete-request]

#### Scenario: Matching effective capability is allowed
r[library.authorization-decisions.authorize-concrete-request.matching-effective-capability-is-allowed]

- GIVEN a verified token whose effective delegation contains a capability covering a requested resource and ability
- WHEN the consumer evaluates an authorization request against the verified token
- THEN the result MUST be allowed
- AND the result MUST expose the matched effective capability

#### Scenario: Missing effective capability is denied
r[library.authorization-decisions.authorize-concrete-request.missing-effective-capability-is-denied]

- GIVEN a verified token whose effective delegation does not cover a requested resource or ability
- WHEN the consumer evaluates an authorization request against the verified token
- THEN the result MUST be denied
- AND the denial reason MUST identify that no effective capability matched

### Requirement: Authorization uses verified capability semantics

Authorization matching MUST reuse the same verified-logic-backed resource and ability containment semantics used for proof-chain attenuation.
r[library.authorization-decisions.verified-matching]

#### Scenario: Wildcard ability segment authorizes child ability
r[library.authorization-decisions.verified-matching.wildcard-authorizes-child-ability]

- GIVEN an effective capability with ability `store/*`
- AND a request for ability `store/add`
- WHEN authorization matching runs
- THEN the request MUST be allowed if the resource is also covered

#### Scenario: Embedded wildcard remains invalid
r[library.authorization-decisions.verified-matching.embedded-wildcard-is-invalid]

- GIVEN an authorization request whose ability contains `*` inside a non-wildcard segment
- WHEN the request is constructed or evaluated
- THEN the library MUST reject the request with a structured validation error
- AND authorization MUST NOT fall back to string-prefix matching

#### Scenario: Authorization requests are concrete operations
r[library.authorization-decisions.verified-matching.request-must-be-concrete]

- GIVEN an authorization request whose requested ability uses `*` as a full wildcard segment
- WHEN the request is constructed or evaluated
- THEN the library MUST reject the request with a structured validation error
- AND wildcard abilities MUST remain valid only on delegated capabilities, not concrete requested operations

#### Scenario: Broader resource requests are denied
r[library.authorization-decisions.verified-matching.broader-resource-request-denied]

- GIVEN an effective capability delegated for a child resource scope
- AND an authorization request for a broader parent resource scope
- WHEN authorization matching runs
- THEN the request MUST be denied with a no-matching-capability reason

### Requirement: Authorization denials are structured

Authorization checks MUST return machine-readable denial reasons instead of losing detail in a bare boolean.
r[library.authorization-decisions.structured-denials]

#### Scenario: Denial reason distinguishes malformed input from no match
r[library.authorization-decisions.structured-denials.distinguishes-malformed-input-from-no-match]

- GIVEN one request with malformed capability input and one valid request not covered by the token
- WHEN both requests are evaluated
- THEN the malformed request MUST produce a validation error or malformed-input denial
- AND the uncovered valid request MUST produce a no-matching-capability denial

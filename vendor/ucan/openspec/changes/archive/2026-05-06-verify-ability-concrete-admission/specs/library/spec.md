## MODIFIED Requirements

### Requirement: Authorization uses verified capability semantics

Authorization matching and concrete request admission MUST reuse the same verified-logic-backed resource and ability containment semantics used for proof-chain attenuation. Capability concrete/wildcard classification exposed by the UCAN verified adapter MUST also route through vendored verified-logic ability helpers rather than local shell-only segment scans.

#### Scenario: Wildcard ability segment authorizes child ability
- GIVEN an effective capability with ability `store/*`
- AND a request for ability `store/add`
- WHEN authorization matching runs
- THEN the request MUST be allowed if the resource is also covered

#### Scenario: Embedded wildcard remains invalid
- GIVEN a requested ability containing `*` inside a larger segment
- WHEN the authorization request is constructed
- THEN the request MUST fail before matching

#### Scenario: Concrete ability admission uses verified wildcard classification
- GIVEN a verified capability ability split into already-validated segments
- WHEN the shell reports whether that ability contains any wildcard segment or can represent a concrete request
- THEN the shell MUST route wildcard/concrete classification through vendored `verified-logic` ability helpers
- AND the check MUST remain limited to segment bytes without claiming parser, policy, proof-chain, caveat, crypto, backend, clock, or provenance truth

## ADDED Requirements

### Requirement: Verified capability-set authorization bridge [r[lib.verified.capability-set-authorizer]]

The library MUST expose a public advanced helper that delegates finite caller-supplied capability-set authorization to the vendored verified capability-merge authorizer.

#### Scenario: Accept contained exact request [r[lib.verified.capability-set-authorizer.accepted]]

- GIVEN a finite capability set containing a capability whose resource is a byte prefix of a requested resource and whose ability exactly equals the requested ability
- WHEN an advanced caller invokes the capability-set authorization bridge
- THEN the helper returns true from the vendored verified capability-set authorizer

#### Scenario: Deny resource or ability escalation [r[lib.verified.capability-set-authorizer.denied]]

- GIVEN a finite capability set without any entry whose resource prefixes the requested resource and whose ability exactly equals the requested ability
- WHEN an advanced caller invokes the capability-set authorization bridge
- THEN the helper returns false from the vendored verified capability-set authorizer

#### Scenario: Preserve exact data-only boundary [r[lib.verified.capability-set-authorizer.boundary]]

- GIVEN caller-supplied public capability documents and a requested resource/ability pair
- WHEN the capability-set authorization bridge runs
- THEN the helper MUST NOT perform wildcard ability subsumption, compact-token verification, proof resolution, cryptographic signature verification, caveat policy evaluation, revocation backend checks, networking, persistence, replay protection, or ambient clock reads

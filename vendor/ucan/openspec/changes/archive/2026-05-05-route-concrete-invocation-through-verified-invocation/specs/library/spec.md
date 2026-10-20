## MODIFIED Requirements

### Requirement: Invocation authorization verifies concrete operations
The library MUST provide an invocation workflow that verifies token/proof validity, capability coverage, and caveat satisfaction for one concrete requested operation. For allowed invocations, effective constraints MUST include the effective delegation time bounds, the matched effective capability, and the caveat-policy output or effective caveat payloads used to authorize the request. Concrete matched-capability invocation admission MUST pass through the vendored verified invocation bridge without replacing wildcard delegation or caveat policy semantics.

#### Scenario: Invocation allowed by valid delegated capability
r[library.invocations.workflow.valid-invocation-is-allowed]

- GIVEN a token and proof chain that delegate a capability to the invoker
- AND request facts that satisfy all caveats on the matched capability
- WHEN the consumer verifies the invocation
- THEN the invocation MUST be allowed
- AND the result MUST expose issuer, invoker, audience, matched capability, and effective constraints

#### Scenario: Invocation denied when request facts violate caveat
r[library.invocations.workflow.caveat-violation-denies-invocation]

- GIVEN a token and proof chain whose matched capability contains a caveat
- AND request facts that do not satisfy that caveat under the supplied policy
- WHEN the consumer verifies the invocation
- THEN the invocation MUST be denied with a caveat-violation reason

#### Scenario: Concrete invocation admission uses verified invocation semantics [r[library.invocations.workflow.concrete-admission-uses-verified-invocation]]

- GIVEN a verified token whose matched effective capability is concrete
- WHEN invocation authorization admits a concrete requested operation
- THEN the shell MUST check the invoker/audience, resource refinement, and ability equality through the verified invocation bridge helper
- AND wildcard matched capabilities MUST continue to use verified capability containment and caveat policy evaluation rather than the exact-ability invocation helper

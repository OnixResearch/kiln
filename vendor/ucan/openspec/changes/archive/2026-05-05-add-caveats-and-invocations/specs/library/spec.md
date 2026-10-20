## ADDED Requirements

### Requirement: Capability claims can carry caveats

The token model MUST support optional caveat payloads on capabilities while preserving deterministic structural validation in `ucan-core` and codec-specific handling in the `ucan` shell.
r[library.caveats.capability-caveats]

#### Scenario: Caveat payload round-trips through compact tokens
r[library.caveats.capability-caveats.round-trips-through-compact-token]

- GIVEN a capability with an application caveat payload
- WHEN the library issues, parses, and validates a compact token containing that capability
- THEN the caveat payload MUST be preserved for shell-side policy evaluation
- AND `ucan-core` MUST remain free of JSON, filesystem, network, and clock dependencies

#### Scenario: Malformed caveat payload is rejected
r[library.caveats.capability-caveats.malformed-caveat-is-rejected]

- GIVEN a compact token with malformed caveat data for the configured codec
- WHEN the library parses or validates the token
- THEN the library MUST return a structured error
- AND the library MUST NOT panic

### Requirement: Caveat policies fail closed

Caveat evaluation MUST use explicit caller-provided policy hooks and MUST fail closed when caveats are unknown or unevaluated.
r[library.caveats.policy-hooks]

#### Scenario: Unknown caveat denies authorization
r[library.caveats.policy-hooks.unknown-caveat-denies]

- GIVEN a verified token whose matched capability contains a caveat unknown to the supplied policy
- WHEN an authorization or invocation check evaluates that capability
- THEN the decision MUST be denied with an unknown-caveat reason

#### Scenario: Child caveats must attenuate parent caveats
r[library.caveats.policy-hooks.child-caveat-attenuates-parent]

- GIVEN a parent capability caveat and a child capability caveat for the same domain
- WHEN proof-chain attenuation is checked with a caveat policy
- THEN the child caveat MUST be accepted only when the policy says it is no broader than the parent caveat

### Requirement: Caveat-aware authorization decisions are structured

The existing authorization decision API MUST distinguish caveat-specific denials from no-capability-match denials when caveat policy evaluation is requested.
r[library.caveats.authorization-decisions]

#### Scenario: Authorization denied by unknown caveat reports caveat reason
r[library.caveats.authorization-decisions.unknown-caveat-reason]

- GIVEN a verified token whose matched capability contains a caveat unknown to the supplied policy
- WHEN a consumer evaluates a caveat-aware authorization request
- THEN the decision MUST be denied with an unknown-caveat reason
- AND the denial MUST NOT be reported as no-matching-capability

### Requirement: Invocation authorization verifies concrete operations

The library MUST provide an invocation workflow that verifies token/proof validity, capability coverage, and caveat satisfaction for one concrete requested operation. For allowed invocations, effective constraints MUST include the effective delegation time bounds, the matched effective capability, and the caveat-policy output or effective caveat payloads used to authorize the request.
r[library.invocations.workflow]

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

### Requirement: Caveat and invocation APIs preserve the core/shell boundary

Caveat payload preservation in `ucan-core` MUST remain codec-neutral, and policy evaluation, invocation request facts, JSON decoding, clocks, resolvers, storage, and network concerns MUST remain in the `ucan` std shell.
r[library.caveats.fcis-boundary]

#### Scenario: Core remains independent of caveat policy implementations
r[library.caveats.fcis-boundary.core-independent-of-caveat-policy]

- GIVEN caveat and invocation support
- WHEN `ucan-core` is built and audited
- THEN it MUST remain free of policy, JSON, filesystem, network, clock, resolver, storage, and compact-token codec dependencies

### Requirement: Caveat and invocation onboarding is documented

The repository MUST provide runnable onboarding for caveat-aware invocation authorization and document fail-closed caveat behavior.
r[library.invocations.consumer-onboarding]

#### Scenario: Invocation example uses deterministic caveat policy
r[library.invocations.consumer-onboarding.deterministic-caveat-policy-example]

- GIVEN the invocation example
- WHEN it runs through the public crate API
- THEN it MUST verify a request using a deterministic caveat policy
- AND it MUST not use private crate modules

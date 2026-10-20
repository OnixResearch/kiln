## ADDED Requirements

### Requirement: Caveat-preserving attenuation invariant [r[library.verified-caveat-preserving-attenuation]]

The library MUST preserve the invariant that a child capability is authorized by a parent capability only when the same parent both contains the child resource/ability and has every opaque parent caveat preserved by the child capability.

#### Scenario: Same parent supplies containment and caveat preservation [r[library.verified-caveat-preserving-attenuation.scenario.same-parent-required]]

- GIVEN multiple parent capabilities where one parent contains the child resource/ability and another parent has matching caveats
- WHEN the shell computes effective delegation for the child capability
- THEN authorization MUST fail unless a single parent capability satisfies both containment and caveat-preservation requirements

#### Scenario: Missing parent caveat is rejected [r[library.verified-caveat-preserving-attenuation.scenario.missing-caveat-rejected]]

- GIVEN a parent capability with opaque caveats and a child capability that omits one or more of those caveats
- WHEN the shell verifies delegation attenuation
- THEN verification MUST fail with a structured attenuation or caveat-preservation error
- AND the child token MUST NOT be treated as authorized

#### Scenario: Future verified bridge must be exact [r[library.verified-caveat-preserving-attenuation.scenario.verified-bridge-exactness]]

- GIVEN an implementation that claims caveat-preserving attenuation is routed through vendored verified-logic
- WHEN bridge guards and documentation are checked
- THEN they MUST show that the bridge enforces same-parent resource/ability containment plus opaque caveat preservation
- AND the implementation MUST NOT replace the shell check with a non-equivalent helper

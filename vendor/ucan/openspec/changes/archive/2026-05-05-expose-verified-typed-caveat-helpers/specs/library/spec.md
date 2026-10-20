## ADDED Requirements

### Requirement: Verified typed caveat helper bridges [r[library.verified-typed-caveat-helpers]]

The library MUST expose opt-in deterministic typed caveat helper bridges only for caveat semantics that route through exact vendored `ucan_caveat` primitives, while preserving caller-supplied shell caveat policy as the production authorization boundary.

#### Scenario: Typed caveat helper admits supported context [r[library.verified-typed-caveat-helpers.scenario.supported-context]]

- GIVEN a supported typed caveat and explicit caller-supplied context values
- WHEN the caller invokes the public typed caveat helper
- THEN the helper MUST route through the corresponding vendored `ucan_caveat` primitive
- AND the helper MUST return the vendored semantic allow/deny outcome without panicking

#### Scenario: Shell policy remains caller supplied [r[library.verified-typed-caveat-helpers.scenario.shell-policy-remains-caller-supplied]]

- GIVEN a token containing opaque caveat payloads
- WHEN the production shell verifies authorization with a caller-supplied caveat policy set
- THEN verification MUST continue to require explicit caller policy acceptance
- AND the typed caveat helpers MUST NOT automatically allow unknown or unsupported caveats

#### Scenario: Typed caveat helpers preserve runtime boundaries [r[library.verified-typed-caveat-helpers.scenario.boundary-preserved]]

- GIVEN typed caveat inputs and no caller-supplied backend or clock
- WHEN any typed caveat helper runs
- THEN the helper MUST NOT access storage, network, process-global registries, sign data, persist state, replay-protect, or read an ambient clock

## ADDED Requirements

### Requirement: Verified bridge coverage sweep [r[library.verified-bridge-coverage-sweep]]

The library MUST maintain an auditable classification for every UCAN verified bridge export and vendored UCAN verified-logic module that distinguishes live verifier routing, public helper exposure, implementation-ready exact candidates, and intentionally documented boundaries.

#### Scenario: Bridge coverage inventory is complete [r[library.verified-bridge-coverage-sweep.scenario.complete-inventory]]

- GIVEN the public verified exports in `src/lib.rs`, bridge implementations in `src/verified.rs`, live verification calls in `src/token.rs`, and vendored UCAN modules under `vendor/verified-logic/src`
- WHEN the bridge coverage sweep is performed
- THEN every relevant bridge or vendored module MUST be classified as live-path, public-helper, candidate, or documented-boundary
- AND the classification MUST NOT include secret source repository metadata, tokens, private keys, credentials, or compact UCAN token values

#### Scenario: Live verifier claims require exact routing [r[library.verified-bridge-coverage-sweep.scenario.live-claims-require-routing]]

- GIVEN a documentation or specification claim that a verified bridge protects live token verification
- WHEN the claim is checked against the implementation
- THEN the live verifier MUST call the exact executable vendored predicate through the UCAN bridge before accepting the corresponding token fact
- AND parser, codec, cryptographic, resolver, proof-store, revocation backend, freshness, provenance, wall-clock, DID-method, CID, principal-link, arbitrary caveat-policy, and runtime orchestration behavior MUST remain outside the verified claim unless an exact executable bridge covers that behavior

#### Scenario: Public helper coverage is not overclaimed [r[library.verified-bridge-coverage-sweep.scenario.public-helper-boundary]]

- GIVEN a verified helper that consumes caller-supplied finite facts but is not called by the live verifier
- WHEN docs, specs, or guard scripts describe the helper
- THEN they MUST identify it as public-helper coverage rather than live verifier coverage
- AND they MUST state that callers remain responsible for fact truth, freshness, provenance, backend behavior, parser behavior, cryptography, and policy semantics

#### Scenario: Unsupported vendored modules remain documented boundaries [r[library.verified-bridge-coverage-sweep.scenario.unsupported-vendored-boundary]]

- GIVEN a vendored UCAN verified-logic module whose surface is proof-only, spec-only, modeled with uninterpreted hash or identity functions, or otherwise lacks an exact executable runtime bridge for UCAN shell facts
- WHEN the bridge sweep evaluates whether to expose or route it
- THEN the module MUST remain a documented-boundary rather than being exposed or described as live verified coverage
- AND any future routing MUST require a separate exact executable bridge or follow-up OpenSpec

#### Scenario: Bridge guard detects drift [r[library.verified-bridge-coverage-sweep.scenario.guard-detects-drift]]

- GIVEN the expected bridge names and coverage classes after the sweep
- WHEN the bridge guard or equivalent validation runs
- THEN it MUST fail if required bridge exports, live-path calls, or boundary-documentation entries drift from the recorded classification
- AND it MUST preserve explicit non-claims for parser, codec, cryptographic, backend, runtime, and caller-policy behavior

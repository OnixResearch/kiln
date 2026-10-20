## ADDED Requirements

### Requirement: Public wrappers cross string and JSON boundaries ergonomically

Public semantic wrapper types MUST implement standard parsing, formatting, and serde traits where the wrapper trait matrix marks the trait/type pair as included, and MUST NOT implement those traits where doing so exposes secret material or weakens validation.
r[library.ergonomics.wrapper-traits]

#### Scenario: Compact token parses from string
r[library.ergonomics.wrapper-traits.compact-token-from-str]

- GIVEN a trimmed compact token string
- WHEN a consumer parses it with `FromStr`
- THEN the result MUST be a validated `CompactToken` wrapper or a structured parse error

#### Scenario: DID wrappers deserialize with validation
r[library.ergonomics.wrapper-traits.did-serde-validates]

- GIVEN JSON containing issuer or audience DID strings
- WHEN the JSON is deserialized into public DID wrapper types
- THEN valid DID values MUST deserialize successfully
- AND empty or untrimmed DID values MUST fail with structured validation errors

#### Scenario: Public wrappers serialize with canonical string values
r[library.ergonomics.wrapper-traits.serde-serializes-canonical-strings]

- GIVEN a non-secret public wrapper with a canonical string form
- WHEN the wrapper is serialized through serde
- THEN the serialized value SHOULD use that canonical string form
- AND deserializing it MUST validate through the same constructor path

#### Scenario: Display round-trips through validated parsing
r[library.ergonomics.wrapper-traits.display-round-trips]

- GIVEN a non-secret public wrapper with `Display` and `FromStr` implementations
- WHEN a consumer formats the wrapper and parses the formatted value
- THEN the parsed wrapper MUST equal the original value
- AND secret-bearing types MUST NOT gain cleartext formatting through this requirement

### Requirement: Public workflows provide builders

Token issue and verify workflows MUST provide builders or named-argument style constructors that reduce positional argument mistakes. Authorization, resolver/proof-store, and invocation builders are handled by their feature-specific changes.
r[library.ergonomics.builders]

#### Scenario: Issue builder constructs valid request
r[library.ergonomics.builders.issue-builder-valid-request]

- GIVEN a signer, audience, capability, proof collection, and time bounds supplied through named builder methods
- WHEN the builder is finalized
- THEN it MUST produce the same valid issue request as the positional constructor

#### Scenario: Builder reports missing required field
r[library.ergonomics.builders.builder-missing-field]

- GIVEN an issue or verify builder missing a required field
- WHEN the builder is finalized
- THEN it MUST fail with a structured missing-field error naming the absent field

### Requirement: Std shell provides safe time helpers

The std shell MUST provide `SystemTime` and `Duration` helpers while preserving explicit deterministic times in the core verifier.
r[library.ergonomics.time-helpers]

#### Scenario: Verification time can be built from SystemTime
r[library.ergonomics.time-helpers.system-time-verification]

- GIVEN a `SystemTime` value
- WHEN a consumer constructs `VerificationTime` through the std helper
- THEN the resulting timestamp MUST match the Unix-seconds conversion or fail with a structured time-conversion error

#### Scenario: Token bounds can be built from Duration
r[library.ergonomics.time-helpers.duration-token-bounds]

- GIVEN a start time and a positive duration
- WHEN a consumer constructs token time bounds through the std helper
- THEN the resulting bounds MUST have the expected Unix-second interval
- AND duration overflow or invalid non-positive bounds MUST fail with a structured error

### Requirement: Reusable integration examples are runnable

The repository MUST include runnable examples that demonstrate realistic downstream use beyond smoke-token issuance.
r[library.ergonomics.integration-examples]

#### Scenario: HTTP bearer example parses and verifies
r[library.ergonomics.integration-examples.http-bearer-verifies]

- GIVEN an HTTP-style bearer token string and explicit verification context
- WHEN the example parses and verifies the token
- THEN the example MUST produce a verified token or structured verification failure without using private crate modules

#### Scenario: Failure-handling example maps structured errors
r[library.ergonomics.integration-examples.failure-handling-maps-errors]

- GIVEN malformed, expired, missing-proof, and missing-key token cases
- WHEN the example evaluates them
- THEN it MUST show stable application-level handling for parse and verification failures


## ADDED Requirements

### Requirement: Parsed caveat payload admission [r[library.compact-token.parsed-caveat-payload-admission]]

The library MUST verify parsed caveat payload shape facts before decoded-token structural validation accepts caveat payload bytes.

#### Scenario: Parsed caveat payload admission uses verified bridge [r[library.compact-token.scenario.parsed-caveat-payload-admission]]

- GIVEN a compact-token payload whose caveat payload fields have been extracted by the shell
- WHEN decoded-token validation decodes caveat payload bytes before core caveat construction
- THEN the library MUST route payload field presence, encoded non-empty, encoded base64url-no-pad, and decoded non-empty facts through a vendored verified caveat-payload admission bridge
- AND missing, empty, invalidly encoded, or decoded-empty payloads MUST fail closed before the token is structurally accepted

#### Scenario: Parsed caveat payload admission keeps payload semantics outside the verified claim [r[library.compact-token.scenario.parsed-caveat-payload-boundary-limits]]

- GIVEN parsed caveat payload facts admitted by the verified predicate
- WHEN the shell later evaluates caveat policies or typed payloads
- THEN the library MUST continue to document JSON parser correctness, UTF-8/string extraction, duplicate-key behavior, base64url decoder correctness, decoded byte provenance, payload schema semantics, typed payload parsing, policy truth, clock/counter freshness, and token provenance as outside the parsed caveat payload admission claim

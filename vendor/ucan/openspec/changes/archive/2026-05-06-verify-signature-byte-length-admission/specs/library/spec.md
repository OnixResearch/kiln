## ADDED Requirements

### Requirement: Verified signature byte-length admission

The library MUST route Ed25519 secret-key, public-key, and signature byte-length admission through vendored `verified-logic` helpers before constructing typed verified signature primitives. The bridge MUST preserve structured length errors and MUST NOT claim that Ed25519 cryptography, key provenance, DID semantics, parser behavior, token canonicalization, resolver/proof-store truth, revocation, clocks, or authorization policy are formally verified by this length-admission seam.

#### Scenario: malformed Ed25519 byte lengths fail before typed construction

- GIVEN secret-key, public-key, or signature byte slices with lengths other than the fixed Ed25519 widths
- WHEN the UCAN verified shell attempts to construct the corresponding verified signature primitive
- THEN construction MUST fail with the existing structured length error for that primitive
- AND the admission decision MUST be routed through `verified_logic::ucan_signature` rather than a local shell-owned comparison

#### Scenario: fixed Ed25519 byte lengths remain accepted

- GIVEN secret-key, public-key, or signature byte slices with the exact fixed Ed25519 widths
- WHEN the UCAN verified shell validates the byte lengths
- THEN the length admission step MUST accept them
- AND typed construction MAY proceed to the existing verified signature primitive wrappers without adding new crypto or provenance claims

## MODIFIED Requirements

### Requirement: End-to-end token round-trip

The library MUST verify that the exact admitted compact-token `header.payload` bytes are the bytes supplied to signature verification before treating a signature result as applying to the parsed token.

#### Scenario: Signature verification input uses verified bridge [r[library.compact-token.scenario.signing-input-byte-provenance-admission]]

- GIVEN an admitted compact-token envelope with header and payload encoded parts
- WHEN the shell constructs bytes for signature verification
- THEN it MUST route the admitted `header.payload` bytes and the signature-verification input bytes through a vendored verified signing-input provenance bridge
- AND mismatched signing-input bytes MUST reject before authorization

#### Scenario: Signing-input provenance keeps crypto semantics outside the verified claim [r[library.compact-token.scenario.signing-input-byte-provenance-boundary-limits]]

- GIVEN signing-input byte provenance facts admitted by the verified predicate
- WHEN the shell verifies Ed25519 signatures or resolves issuer keys
- THEN the library MUST continue to document Ed25519 correctness, key resolution truth, JSON canonicalization, parser correctness, decoded bytes, duplicate-key behavior, cryptographic strength, and token provenance as outside the signing-input provenance claim

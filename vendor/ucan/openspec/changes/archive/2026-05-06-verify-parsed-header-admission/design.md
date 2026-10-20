## Context

After base64url decoding and decoded JSON object-envelope admission, compact header bytes are deserialized by serde into `DecodedHeader`. The shell then performs ordinary Rust string comparisons against `EdDSA` and `JWT`.

## Goals

- Verify the exact parsed header field admission predicate over shell/serde-supplied bytes.
- Fail closed before structural token validation continues when `alg` or `typ` differ from the supported constants.
- Keep parser and crypto trust boundaries explicit.

## Non-Goals

- Do not verify serde JSON parser correctness, UTF-8 correctness, duplicate-key behavior, canonicalization, base64url decoded bytes, signing-input construction, Ed25519, BLAKE3, resolver/proof-store/revocation behavior, or token provenance.
- Do not add alternate algorithms or token types.

## Decisions

### Parsed field bytes as the bridge boundary

The shell converts parsed `String` fields to bytes and passes them to the bridge. The verified predicate checks exact byte equality to `EdDSA` and `JWT`. This verifies the branch condition used by the shell without claiming how serde produced those strings.

### Header admission stays separate from signature verification

Header admission only proves supported parsed field values. Signature verification remains the existing Ed25519 runtime boundary and still consumes the already-verified signing input framing.

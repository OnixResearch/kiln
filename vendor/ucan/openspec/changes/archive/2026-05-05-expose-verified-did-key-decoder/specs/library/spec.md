## ADDED Requirements

### Requirement: Verified DID-key decode bridge [r[lib.verified.did-key-decode]]

The library MUST expose a public advanced helper that decodes caller-supplied `did:key` bytes through the vendored verified DID-key decoder and returns only public decoded key bytes, the multicodec value, and the original encoded bytes when structural decoding succeeds.

#### Scenario: Decode accepted DID key [r[lib.verified.did-key-decode.accepted]]

- GIVEN DID-key bytes previously produced by the verified encoder
- WHEN an advanced caller invokes the decode bridge
- THEN the helper returns decoded public-key bytes, codec, and original encoded bytes from the vendored decoder

#### Scenario: Reject malformed DID key [r[lib.verified.did-key-decode.reject-malformed]]

- GIVEN bytes without the required DID-key prefix, valid base58btc payload, or multicodec structure
- WHEN an advanced caller invokes the decode bridge
- THEN the helper returns no decoded key and does not perform fallback resolution

#### Scenario: Preserve structural-only boundary [r[lib.verified.did-key-decode.boundary]]

- GIVEN any caller-supplied DID-key bytes
- WHEN the decode bridge runs
- THEN the helper MUST NOT perform remote DID resolution, key lookup, cryptographic signature verification, trust-chain checks, proof fetching, filesystem/network fallback, persistence, or ambient clock reads

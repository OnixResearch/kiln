## Context

`parse_compact_token` decodes base64url header and payload segments and then deserializes them with `serde_json`. Previous slices verify compact split/envelope framing and encoded base64url shape, but decoded JSON byte interpretation remains runtime-only.

## Goals

- Verify a finite, deterministic, byte-level screen for decoded JSON object envelopes.
- Permit ASCII JSON whitespace around the object by verifying shell-supplied first/last significant byte indexes.
- Fail closed before serde-derived semantics when decoded header or payload bytes are not structurally an object.

## Non-Goals

- Do not verify JSON parser correctness, canonical JSON, UTF-8 semantics, duplicate-key handling, claim/header fields, base64url decoded-byte correctness, Ed25519, BLAKE3, resolver/proof-store/revocation behavior, or token provenance.
- Do not add a runtime dependency or replace serde parsing.

## Decisions

### Verified shell-supplied significant-byte facts

The runtime shell computes the first and last non-JSON-whitespace byte indexes, then the bridge checks that all prefix/suffix bytes are JSON whitespace and the significant bytes are `{` and `}`. This avoids proving the Rust index search while still verifying the exact byte facts used for object admission.

### Parse-time integration only for header and payload JSON

The bridge runs after base64url decoding and before `serde_json::from_slice` for compact header and payload parts. Signature bytes, caveat payload bytes, and proof references are not JSON objects and remain outside this screen.

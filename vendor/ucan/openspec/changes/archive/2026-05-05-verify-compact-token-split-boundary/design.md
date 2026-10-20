## Context

`parse_compact_token` currently calls `compact_parts()` with Rust string splitting, builds a signing input from the header and payload parts, then checks `compact_token_envelope_is_valid()`. That envelope check proves exact byte concatenation but intentionally says the compact split and codec layers are outside scope.

## Goals

- Verify the closeable structural part of the split boundary: each emitted part is separator-free and the compact bytes equal `header.payload.signature`.
- Keep all codec/hash/crypto/runtime behaviors explicit trusted boundaries.
- Preserve current public parsing behavior and structured errors.

## Non-Goals

- No verified base64url algorithm.
- No verified JSON parser or canonicalizer.
- No verified BLAKE3 or Ed25519 implementation.
- No network, proof-store, resolver, revocation, replay, persistence, or clock behavior.

## Decisions

### Separator-free parts plus existing framing

The vendored primitive will add a finite byte scanner `bytes_have_no_separator()` and compose it with existing compact framing to define `compact_split_matches_parts()`. This proves an exact byte-level split admission predicate over caller/shell-supplied parts without trusting or reimplementing Rust `str::split` in Verus.

### Parse-time integration

The shell will continue using `compact_parts()` for ordinary parsing and part-count error reporting, then check the resulting bytes with the verified split bridge before decoding any part. Failures use the existing compact framing mismatch error because they are structural compact-token byte mismatches.

### Documentation boundary

Docs and guard scripts must say this closes only separator-free three-part split/framing facts. Base64url, JSON, BLAKE3, Ed25519, and external backends remain trusted runtime boundaries.

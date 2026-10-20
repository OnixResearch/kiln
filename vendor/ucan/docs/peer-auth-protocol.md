# Peer-auth protocol evidence

The optional `ucan-core/peer-auth` feature verifies one authenticated message. It does not change UCAN tokens, proofs, caveats, replay APIs, or authorization.

## Reviewed reference

The bounded reference is `devinautotomic226/celld` revision `c978a2590e875b003ecd860a86cb7551287eba67`.

The review selected `crates/celld/peer_auth.rs` blob `40d6ac4613d293a148e7222e192153eaeaeb29db`. That file binds these values:

- a protocol version;
- opaque request context;
- a SHA-256 body digest;
- a timestamp;
- a nonce;
- an HMAC-SHA-256 tag; and
- a bounded replay decision.

The Celld repository and selected source file use `Apache-2.0`. UCAN copies no Celld code. The reference does not establish parity, equivalence, interoperability, or Celld correctness.

## UCAN framing

UCAN uses its own length-framed domain and protocol version. The HMAC covers the version, timestamp, nonce, opaque routing context, and exact SHA-256 body digest.

The verifier requires an exact caller-selected version and routing context. It also requires caller-supplied time, clock skew, and replay observations.

The message identity and HMAC reference use separate, domain-framed BLAKE3 identities. The HMAC tag remains an HMAC-SHA-256 value. The body digest remains a SHA-256 value.

The focused HMAC test uses RFC 4231 test case 1 for HMAC-SHA-256.

## Dependency evidence

`Cargo.lock` records these exact optional `ucan-core` dependencies:

| Package | Version | Registry checksum | License |
|---|---:|---|---|
| `blake3` | `1.8.4` | `4d2d5991425dfd0785aed03aedcf0b321d61975c9b5b3689c774a2610ae0b51e` | `CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception` |
| `hmac` | `0.12.1` | `6c49c37c09c17a53d937dfbb742eb3a961d65a994e6bcdcf37e7399d0cc8ab5e` | `MIT OR Apache-2.0` |
| `sha2` | `0.10.9` | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` | `MIT OR Apache-2.0` |
| `zeroize` | `1.8.2` | `b97154e67e32c85465826e8bcc1c59429aaaf107c1e4a9e53c8d8ccd5eff88d0` | `MIT OR Apache-2.0` |

The feature disables default dependency features. The core stays compatible with `no_std` and `wasm32-unknown-unknown`.

## Consumer responsibilities

Consumers retain transport, TLS, routing-context construction, key generation and custody, clocks, replay storage, membership, subject resolution, and authorization.

A successful verdict proves only that the supplied key authenticated one exact framed message under the supplied clock and replay observations. It does not grant authority or authenticate an end user.

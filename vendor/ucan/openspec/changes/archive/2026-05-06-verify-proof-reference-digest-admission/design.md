# Design: proof-reference digest admission

## Boundary

The new primitive models a proof reference as caller/shell-supplied digest bytes. It accepts exactly when:

1. the digest is `32` bytes, matching BLAKE3 output width used by the shell; and
2. the stored proof-reference bytes are exactly those digest bytes.

The primitive intentionally treats the hash function as outside the proof. It does not prove that the digest was computed by BLAKE3 for the compact token, that compact-token bytes are canonical, or that resolver/proof-store/revocation backends are correct.

## Shell integration

`proof_reference_for_token` continues to compute `blake3::hash(token.as_str().as_bytes())` in ordinary Rust. Immediately after computing the digest, it calls the bridge helper over the digest bytes and candidate reference bytes. The bridge delegates to vendored `verified-logic`, making the admitted proof-reference shape and byte equality part of the audited bridge surface.

## Validation

Add unit coverage for valid 32-byte digest admission, wrong-length denial, and reference/digest mismatch denial. Update docs and bridge guard checks so future shell changes cannot silently bypass the vendored predicate.

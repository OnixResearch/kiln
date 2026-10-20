# Design

## Decision

The seam is structural base64url-no-pad admission over encoded segment bytes, not codec verification. The verified predicate accepts only ASCII bytes in `[A-Za-z0-9_-]` and rejects unpadded length class `len % 4 == 1`, which cannot represent valid base64url-no-pad data.

## Non-goals

- Do not verify base64url decode output bytes.
- Do not verify JSON canonicalization or serde.
- Do not verify signature, BLAKE3, resolver/proof-store, revocation, or caveat semantics.

## Integration

`decode_part` will call the bridge before `URL_SAFE_NO_PAD.decode`. On a rejected segment it returns the existing `TokenError::Base64Decode` class with a deterministic structural-admission message. The runtime decoder remains responsible for actual decoding.

## Risks

The predicate must match the URL-safe no-padding alphabet and must not reject valid empty segments unless callers impose separate semantic requirements. Length class `1 mod 4` is rejected because unpadded base64url cannot encode that length.

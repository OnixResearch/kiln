## Why

Parsed UCAN claim admission now routes issuer, audience, attenuation, expiration, and not-before presence through a verified bridge, but the decoded `prf` proof-list field still falls through to ordinary core claim validation. `prf` presence is part of the compact UCAN claim shape even when the list is empty, so this leaves one adjacent parsed-claim presence fact outside the current finite admission predicate.

## What Changes

- Extend the vendored parsed-claims presence primitive to include proof-list presence.
- Route shell/serde-supplied `prf` field presence through the UCAN verified bridge before core claim construction.
- Preserve parser, duplicate-key, field semantic, proof-reference decoding, proof-store, resolver, revocation, signing-input, hash, and cryptographic boundaries as non-claims.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, public exports/docs/guards, and this OpenSpec package.
- **APIs**: `VerifiedParsedClaimsPresence` gains a proof-list presence fact.
- **Testing**: focused bridge/unit tests, UCAN workspace checks, bridge guard, OpenSpec validation, and Nix flake check.

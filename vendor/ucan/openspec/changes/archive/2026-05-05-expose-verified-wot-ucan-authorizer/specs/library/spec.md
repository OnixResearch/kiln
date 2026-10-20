## ADDED Requirements

### Requirement: Verified Web-of-Trust UCAN authorizer bridge [r[library.verified-bridges.wot-ucan-authorizer]]

The library MUST expose a deterministic public helper that delegates caller-supplied finite Web-of-Trust UCAN authorization data to the vendored verified Web-of-Trust UCAN authorizer primitive.

#### Scenario: Rooted trust authorizes UCAN [r[library.verified-bridges.wot-ucan-authorizer.authorized]]

- GIVEN a structurally valid UCAN payload, caller-supplied positive ability/caveat/DID-CID linkage facts, satisfied active caveats, root principals, active attestations, no effective matching revocation, and a threshold that the roots satisfy
- WHEN the helper is called with deterministic token and trust evaluation times
- THEN it MUST return an authorized result through the vendored Web-of-Trust UCAN authorizer

#### Scenario: Missing trust facts deny UCAN [r[library.verified-bridges.wot-ucan-authorizer.denied]]

- GIVEN otherwise finite caller-supplied Web-of-Trust UCAN authorization data
- WHEN the token envelope is invalid, any caller-supplied ability/caveat/DID-CID fact is false, active caveats are unsatisfied, trust resolution fails, threshold policy fails, or an effective matching revocation applies
- THEN the helper MUST return a denied result through the vendored Web-of-Trust UCAN authorizer

#### Scenario: Advanced bridge preserves production boundaries [r[library.verified-bridges.wot-ucan-authorizer.boundary]]

- GIVEN production UCAN verification depends on cryptography, DID/CID decoding, resolver/proof-store traversal, caveat policy evaluation, revocation backend behavior, and application trust stores
- WHEN the Web-of-Trust UCAN bridge is used
- THEN the helper MUST remain limited to caller-supplied finite data and explicit caller-supplied facts and MUST NOT perform compact-token verification, cryptographic signature verification, DID/CID decoding, proof fetching, key lookup, revocation backend/freshness checks, caveat policy evaluation, registry/storage/network access, persistence, replay checks, or ambient clock reads

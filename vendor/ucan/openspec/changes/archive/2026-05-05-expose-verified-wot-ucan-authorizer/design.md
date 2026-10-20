## Context

`verified_logic::web_of_trust_ucan_authorizer` consumes already-resolved finite data plus shell-owned boolean facts: ability containment, caveat containment, and DID/CID linkage. It then checks the UCAN envelope, active caveat satisfaction, rooted trust resolution, threshold policy, and revocation effects.

## Decisions

### Data-only wrapper

The shell will define public plain data structs and convert them into vendored verified-logic structs immediately before calling the verified helper. The public helper returns a small authorization enum rather than performing network or storage work.

### Caller-owned facts stay explicit

The wrapper will take `ability_allowed`, `caveats_contained`, and `did_cid_linked` booleans as caller-supplied facts. This preserves existing shell-owned authorization/caveat/DID/CID verification and avoids overclaiming that the Web-of-Trust facade performs production decoding or policy evaluation.

### Digest-width validation

Public principal digest bytes must be exactly the vendored principal digest width. Invalid digest lengths return `VerifiedPrimitiveError` instead of silently constructing malformed principal data.

## Non-Goals

- No built-in Web-of-Trust registry, resolver, persistence, backend freshness, or network lookup.
- No cryptographic signature verification beyond caller-provided fields and verified-logic's deterministic model surface.
- No replacement for `verify_compact_token`, resolver/proof-store traversal, revocation checker fail-closed behavior, caveat policy traits, or invocation verification.

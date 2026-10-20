## Context

`verify_compact_token` is staged: limits/revocation/cycle checks, parse/framing, structural validation, key resolution, signature check, envelope/temporal checks, proof loading, proof-chain attenuation, and final authorization. Previous slices verified exact compact byte framing and finite proof traversal facts, but did not model this stage admission boundary.

## Goals

- Model a finite, deterministic stage transcript over caller-supplied booleans.
- Verify that acceptance is equivalent to every required stage passing.
- Verify that rejection returns the first failed stage in fixed shell order.
- Expose only a data-only helper suitable for shell/caller transcripts.

## Non-Goals

- Verifying base64url, JSON serde/canonicalization, Ed25519, BLAKE3, resolver/proof-store/revocation backend correctness, proof freshness, filesystem/network behavior, caveat policies, or recursive traversal orchestration.
- Replacing structured `TokenError` internals or changing public verification semantics.

## Decisions

### Finite stage booleans

Use a compact record of public boolean facts instead of embedding token bytes or backend details. This keeps the primitive exact and avoids credential/token leakage.

### First-failure stage IDs

Expose stable public stage IDs so callers can compare shell error mapping against a verified fixed order without carrying secret-bearing error payloads.

### Bridge-only integration

Start with a public bridge and tests rather than invasive live error plumbing. Live verification remains ordinary Rust; callers can supply stage facts from their own instrumentation.

## Why

The accepted HTTP observation shell authenticates requests with a pinned
Ed25519 request signature. The observation design records UCAN holder
invocation as the planned capability backend behind the same `ObservationAuth`
port. Operators need capability-based observation grants, proof chains,
audience-bound holder possession, explicit revocation, and bounded replay
without changing the accepted projection or transport contract.

This change begins that deferred follow-up. It adds a second auth variant to
the existing HTTP profile and keeps the Ed25519 variant valid.

## What Changes

- Vendor and pin the exact UCAN cohort, including `ucan`, `ucan-core`, and the
  `verified-logic` snapshot, and record its AGPL/MPL license boundary.
- Add `ucan_holder_invocation_v1` to the typed HTTP observation auth profile:
  root issuer, resource, ability, operation profile, bounded proof tokens,
  revoked proof references, token/nonce/proof bounds, and non-claims.
- Parse a bounded UCAN holder-invocation authorization header and verify the
  compact token, proof chain, revocation, audience key possession, operation
  profile, request binding, time window, caveats, and replay admission before
  projection.
- Map the verified capability to the exact observation request; a denied or
  ambiguous invocation returns a bounded rejection without projecting.
- Add positive and negative fixtures and maintain the existing Ed25519 path.
- Update the license map, reference pins, operator documentation, and evidence.

## Impact

- **Adapters**: `kiln-adapter-http-observe` gains a second auth variant and a
  vendored UCAN dependency.
- **Core**: unchanged; the accepted projection remains pure and unchanged.
- **Configuration**: adds typed Nickel UCAN auth profile fields and fixtures.
- **Licensing**: links AGPL-3.0-or-later `ucan` and MPL-2.0 `verified-logic`;
  combined artifacts are not permissive-only.
- **Testing**: covers token/proof verification, holder signature, capability
  admission, request binding, revocation, replay, time, caveats, malformed
  headers, and the unchanged Ed25519 path.

## Out of Scope

- Importing Basalt policy authority or its full runtime.
- UCAN peer authentication for the iroh observation leg.
- Control intake, dashboards, UI, runtime logging, or metric scraping.

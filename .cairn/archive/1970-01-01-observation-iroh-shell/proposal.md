# Add an iroh observation shell

## Why

The accepted `observation-projection` change adds a pure read-only observation core, and `observation-http-shell` will serve it to standard clients over HTTPS. Peers and distributed operators need the same observation surface over iroh: dial-by-key, no public endpoint, verified content, and relay fallback. Kiln has no iroh dependency today.

Survey of `n0-computer` shows the exact primitives for this: `iroh` (QUIC dial-by-key, hole-punching, relay fallback), `iroh-blobs` and `bao-tree` (BLAKE3-verified streaming), and a WebSocket-to-iroh bridge pattern for browser reach. This change adds the second transport leg: a narrow Kiln-owned iroh shell that serves the same admitted projection to peers under one ALPN protocol.

## What Changes

- Add `kiln-adapter-iroh-observe` as a tokio shell that serves the accepted `kiln.observe` projection over one `kiln.observe.v1` ALPN protocol.
- Pin the consumed n0 cohort: `iroh` core, `iroh-blobs`, and `bao-tree` with exact versions and revisions.
- Dial by public key with stable node identity, alias allowlist and revocation, and visible relay fallback labels.
- Stream log and record ranges as content-addressed, BLAKE3-verified payloads; recipients verify before claiming delivery.
- Prove transport equivalence: the HTTP and iroh legs MUST produce equal logical views over shared fixtures.
- Keep loss, lag, and relay dependence non-authoritative and keep observation read-only.

## Dependencies

This change depends on the accepted `observation-projection` delta (pure view projection) and on `observation-http-shell` (the conformance counterpart for the equal-views rail). It consumes the pinned n0-computer crates as published dependencies and does not copy tile or aspen transport code.

## Impact

- **Adapters**: adds `kiln-adapter-iroh-observe`; no existing crate changes behavior.
- **Core**: consumes the accepted projection unchanged; no core change.
- **Configuration**: adds typed Nickel iroh observation profiles (cohort pins, ALPN, endpoint and relay policy, peer identity and trust, bounds, non-claims).
- **Testing**: covers serving, peer admission and revocation, verified streaming, corrupted-payload rejection, relay fallback, HTTP/iroh view equality, and authority promotion.

## Out of Scope

- The HTTP shell (separate change).
- Control endpoints (submit-command intake).
- A hosted WebSocket bridge deployment; the n0 bridge is a reference pattern, not a dependency.
- Dashboards, UI, runtime logging, and metric scraping.

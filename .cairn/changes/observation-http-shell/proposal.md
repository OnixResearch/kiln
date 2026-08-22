# Add an HTTP observation shell

## Why

The accepted `observation-projection` change adds a pure read-only observation core, but it has no operator-visible surface. Operators need to observe committed Kiln state from standard clients over HTTPS: bounded JSON views and a live event stream. No HTTP transport exists in Kiln today; every network-shaped capability is behind a trait with only test doubles.

This change adds the first transport leg for the observation projection: a bounded HTTP server shell that serves the accepted projection to browsers, curl, and tooling. It is the Buildbot-like operator surface without Buildbot's write coupling: observation stays strictly read-only.

## What Changes

- Add `kiln-adapter-http-observe` as a shell crate that serves admitted `kiln.observe` views as canonical JSON.
- Admit view requests under a typed Nickel HTTP observation profile: registered origin, TLS policy, auth, content contract, and named bounds.
- Serve live event updates as a bounded SSE stream driven by committed journal events, with loss non-authoritative.
- Reject unknown views, unknown targets, unauthorized requests, and oversized requests before projection.
- Keep the shell transport-neutral to the projection: it consumes the accepted observation core and never mutates state.
- Add positive and negative conformance fixtures and non-claim checks.

## Dependencies

This change depends on the accepted `observation-projection` delta (pure view projection, `kiln.observe.view.v1` identity) and on the `ci-domain` and `durable-operations` contracts for the state and journal it serves. It also follows the registered-origin, TLS, DNS-class, and credential discipline of the accepted webhook notification adapter.

## Impact

- **Adapters**: adds `kiln-adapter-http-observe`; no existing crate changes behavior.
- **Core**: consumes the accepted projection unchanged; no core change.
- **Configuration**: adds typed Nickel HTTP observation profiles (origin, TLS, auth, content, bounds, non-claims).
- **Testing**: covers serving, rejection, auth, TLS, live delivery and loss, malformed requests, timeouts, and authority promotion.

## Out of Scope

- The iroh transport leg (separate change).
- Control endpoints (submit-command intake).
- Dashboards, UI, runtime logging, and metric scraping.

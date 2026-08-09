# Add a webhook notification adapter

## Why

Kiln defines notification intents and a `NotificationPort`, but it has no outbound notification adapter.

A bounded webhook adapter provides one reusable integration without adding service-specific notification semantics to the core.

## What Changes

- Add optional `kiln-adapter-webhook` support for the `NotificationPort`.
- Use registered endpoint profiles instead of arbitrary request URLs.
- Project notification intents into bounded versioned bodies through a pure core.
- Bind deliveries to the run, outcome, notification policy, endpoint profile, body identity, and idempotency key.
- Support configured authentication and signature schemes through secret references.
- Disable redirects and dynamic destinations by default.
- Preserve accepted, rejected, rate-limited, timed-out, provider-error, and unknown outcomes.
- Add typed Nickel profiles, retry and reconciliation policy, SSRF controls, and conformance fixtures.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and the accepted durable operations contract.

## Impact

- **Adapters**: adds `kiln-adapter-webhook`.
- **Core**: retains notification meaning and pure projection decisions.
- **Configuration**: adds endpoint, authentication, body, bound, retry, and network policy.
- **Testing**: covers delivery, invalid destinations, redirects, secrets, rate limits, timeouts, and ambiguity.

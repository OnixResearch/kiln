# Add GitHub provider adapters

## Why

Kiln models GitHub collaboration identities and webhook security, but it has no concrete GitHub trigger, status, or collaboration adapter.

GitHub is a declared mirror and optional collaboration provider. It must not replace Radicle repository authority through familiar API behavior.

## What Changes

- Add optional `kiln-adapter-github` with separate trigger, status, and collaboration profiles.
- Pin supported webhook, REST, GraphQL, status, check-run, pull-request, and merge contract cohorts.
- Validate webhook HMAC-SHA256 signatures, delivery identities, repository scope, event types, and replay state.
- Publish statuses as idempotent rebuildable projections.
- Observe pull requests, reviews, commits, checks, and mergeability as native facts.
- Put merge mutation behind a separate capability with fresh target and candidate preflight.
- Preserve installation, account, repository, delivery, pull-request, commit, check, and request identities.
- Add typed Nickel profiles, secret references, rate limits, retries, reconciliation, and conformance fixtures.

## Dependencies

This change depends on `establish-neutral-ci-control-plane`. Direct Git snapshot acquisition remains owned by the separate Git source adapter.

## Impact

- **Adapters**: adds `kiln-adapter-github` with narrow provider profiles.
- **Core**: retains provider-neutral trigger, status, collaboration, and merge decisions.
- **Configuration**: adds API, webhook, secret, capability, rate, and authority policy.
- **Testing**: covers signatures, replay, drift, rate limits, status ambiguity, and stale merge preflight.

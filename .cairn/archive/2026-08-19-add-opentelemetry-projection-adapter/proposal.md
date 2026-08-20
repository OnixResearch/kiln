# Add an OpenTelemetry projection adapter

## Why

Kiln models OpenTelemetry as a rebuildable projection, but it has no concrete span, metric, log, or OTLP adapter.

Operators need telemetry without giving a telemetry backend lifecycle, outcome, or release authority.

## What Changes

- Add `kiln-adapter-opentelemetry` for bounded span, metric, and log projections.
- Pin supported OpenTelemetry data-model, OTLP, and CI/CD semantic-convention cohorts.
- Build telemetry records from admitted Kiln events and observations through pure functions.
- Preserve Kiln and provider identities through declared attributes and links.
- Define stable cardinality, sampling, redaction, timestamp-observation, and resource policy.
- Keep telemetry loss, delay, duplication, and backend failure independent from canonical CI state.
- Add an optional bounded OTLP shell with explicit endpoint and credential profiles.
- Add typed Nickel profiles and deterministic export fixtures.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and the accepted interchange-projections contract.

## Impact

- **Adapters**: adds `kiln-adapter-opentelemetry`.
- **Core**: retains CI state and pure projection inputs.
- **Configuration**: adds schema, resource, cardinality, sampling, redaction, transport, and bound policy.
- **Testing**: covers stable mappings, loss, duplicate export, secret redaction, backend failure, and authority promotion.

# Design: OpenTelemetry projection adapter

## Context

Kiln can rebuild status, metrics, logs, and notifications from canonical events and admitted observations. OpenTelemetry can receive copies of these facts.

Telemetry clocks, sampling, transport, and storage are external observations. They cannot become inputs to canonical transitions without a separate admitted command.

## Goal and Success Contract

The goal is deterministic bounded telemetry projection plus an optional OTLP shell.

Completion requires pinned schemas, pure mappings, stable identity links, cardinality controls, redaction, bounded transport, and backend-loss fixtures.

False completion includes telemetry as an event store, backend success as CI success, unbounded labels, secret export, or hidden ambient endpoint authority.

## Decisions

### Decision: Keep projection pure

Pure functions map committed Kiln events and admitted observations into bounded span, metric, and log records.

The shell supplies observation times, resource facts, sampling decisions, credentials, encoding, batching, and transport results.

### Decision: Pin semantic cohorts

Profiles record OpenTelemetry data-model, OTLP protocol, and CI/CD semantic-convention versions and stability status.

Unknown required fields or semantic drift fail before export.

### Decision: Preserve identities with links

Records carry exact Kiln project, evaluation, run, task, attempt, product, event, and provider references where the profile declares them.

Trace and span identifiers remain telemetry-native. They never replace Kiln identities.

### Decision: Bound cardinality and volume

Profiles declare allowed attributes, value lengths, collection sizes, series keys, batch sizes, queue limits, and drop policy.

Unbounded provider labels, source refs, logs, and diagnostics cannot become metric dimensions.

### Decision: Redact before transport

Pure redaction rules operate on typed fields before encoding. The shell never exports credentials or undeclared raw provider values.

A redaction failure blocks the affected record or batch under explicit policy.

### Decision: Keep loss non-authoritative

Dropped, sampled, delayed, duplicated, or rejected telemetry remains an export observation. It cannot change run outcomes, retries, gates, or release readiness.

### Decision: Constrain OTLP transport

An optional profile defines one registered endpoint, protocol, TLS policy, credentials, deadlines, batching, retry, and response bounds.

Ambiguous export can retry only because telemetry records are non-authoritative and carry stable deduplication facts where supported.

## Configuration

Typed Nickel profiles define schemas, signal mappings, resources, identities, attributes, cardinality, sampling, redaction, batching, queueing, transport, credentials, bounds, and non-claims.

## Validation

Positive fixtures cover spans, metrics, logs, links, redaction, batching, and rebuild. Negative fixtures cover drift, cardinality, secret fields, oversized values, backend loss, duplicate export, and authority promotion.

## Claim Boundary

Passing checks establishes bounded telemetry mapping and export observations. It does not prove complete telemetry, backend durability, CI correctness, or release readiness.

# Design: Mantle build adapter

## Context

Kiln already models build effects, products, metrics, remote execution observations, and provider outcomes. Mantle supplies a stack-native build boundary with stronger native build and evidence facts.

The adapter cannot move Mantle derivations, store paths, runtime handles, or build authority into `kiln-core`.

## Goal and Success Contract

The goal is a real optional `BuildPort` implementation for one exact Mantle cohort.

Completion requires exact request linkage, bounded dispatch, product admission, cancellation, reconciliation, and independent compatibility fixtures.

False completion includes profile-only support, invented success, cache-based completion, builder identity confusion, or an ambient sibling dependency.

## Decisions

### Decision: Consume a pinned contract

The adapter pins a published Mantle revision and versioned request, result, and receipt schema. An ambient sibling checkout cannot satisfy runtime admission.

Compatibility drift makes the profile unavailable before dispatch.

### Decision: Keep plan construction pure

A pure function converts one admitted Kiln build effect into a bounded Mantle request value. It binds the exact candidate or source, pipeline revision, command, environment, platform, outputs, and timeout policy.

The shell performs serialization, transport, clocks, cancellation, credential use, and receipt reads.

### Decision: Preserve identity roles

The adapter preserves builder, worker, provider, signer, store object, product, log, and receipt identities. Kiln remains the control plane unless it performed a named native role.

Protocol-required digest algorithms remain native facts. Kiln uses BLAKE3 only for its own canonical identities.

### Decision: Admit products before success

A Mantle success observation does not complete a Kiln attempt by itself. The pure core checks request linkage, required products, identities, bounds, and terminal classification.

Missing products, mismatched plans, malformed receipts, and stale observations fail without canonical success.

### Decision: Keep cache facts advisory

Cache hits and substitutions remain Mantle observations. They cannot bypass build admission, product checks, or candidate linkage.

### Decision: Preserve unknown outcomes

If transport fails after acceptance, the adapter records `Unknown`. Reconciliation uses the exact native operation or receipt identity.

Cancellation does not imply cancellation until Mantle supplies an admitted terminal observation.

## Configuration

Typed Nickel profiles define the Mantle revision, contract versions, endpoint, identity namespace, capabilities, product bounds, log bounds, timeouts, and reconciliation policy.

## Validation

Positive fixtures cover build success, admitted cache use, products, metrics, logs, cancellation, and reconciliation.

Negative fixtures cover version drift, request substitution, missing products, wrong builder, malformed receipts, timeout, disconnect, duplicate completion, and oversized values.

## Claim Boundary

Passing checks establishes adapter conformance for the selected Mantle contract. It does not prove build correctness, sandbox completeness, cache truth, or release readiness.

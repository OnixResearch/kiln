# Design: Cairn gate adapter

## Context

Kiln forms exact candidate sets and owns their CI evaluation state. Cairn evaluates lifecycle policy and release readiness for declared repository artifacts and evidence.

The adapter must preserve this authority split. A Cairn verdict is a provider observation until Kiln admits its exact applicability.

## Goal and Success Contract

The goal is a versioned `GatePort` that evaluates one exact Kiln candidate through a pinned Cairn contract.

Completion requires deterministic request planning, bounded execution, exact receipt linkage, stable diagnostics, and fail-closed stale-result handling.

False completion includes parsing display text, using ambient policy, accepting a verdict for another candidate, or treating readiness as release execution.

## Decisions

### Decision: Use a versioned machine contract

The first adapter uses a pinned Cairn executable and machine-readable request, result, diagnostic, and receipt formats.

The profile records the executable path and BLAKE3 identity, Cairn revision, contract versions, policy identity, platform, and bounds.

### Decision: Build requests in a pure core

The core binds candidate identity, ordered changes, source snapshots, pipeline revision, required evidence identities, policy identity, and effect identity.

The shell owns files, process execution, environment, clocks, cancellation, and output capture.

### Decision: Require exact applicability

A gate result must repeat or cryptographically bind the exact candidate, policy, evidence set, tool cohort, and request identity.

Moved targets, changed evidence, changed policy, and stale candidates require a new request. Prior success cannot authorize the new candidate.

### Decision: Preserve verdict detail

Pass, block, policy error, provider error, timeout, cancellation, and unknown remain distinct. Findings and missing evidence stay bounded structured facts.

The adapter cannot collapse a process exit into gate failure without the declared result contract.

### Decision: Keep authority narrow

A passing gate can satisfy one configured Kiln gate condition. It cannot merge, publish, release, mutate Cairn artifacts, or override another gate.

### Decision: Reconcile ambiguous execution

If process or transport state is ambiguous after request acceptance, the adapter records unknown. Reconciliation requires an exact receipt or deterministic rerun policy.

## Configuration

Typed Nickel profiles define executable identity, policy identity, supported commands, formats, bounds, deadlines, evidence inputs, rerun policy, and non-claims.

## Validation

Positive fixtures cover pass and blocked verdicts with exact receipts. Negative fixtures cover candidate substitution, stale policy, missing evidence, malformed output, timeout, cancellation, and ambiguous termination.

## Claim Boundary

Passing checks establishes Cairn contract compatibility and exact gate applicability. It does not prove source correctness, test completeness, release execution, or deployment success.

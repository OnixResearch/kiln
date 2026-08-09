# Design: Lattice workflow adapter

## Context

Kiln owns CI meaning and emits workflow effect requests. Lattice executes typed DAGs and records bounded adapter observations.

A direct type import into core would transfer workflow runtime semantics into Kiln. The adapter must use an explicit versioned boundary.

## Goal and Success Contract

The goal is a real optional `WorkflowPort` for one pinned Lattice contract.

Completion requires deterministic lowering, exact identity linkage, capability admission, lifecycle controls, logs, unknown outcomes, and conformance evidence.

False completion includes a profile-only adapter, hidden graph mutation, provider success without linked nodes, or Lattice state used as canonical Kiln state.

## Decisions

### Decision: Lower through a pure adapter core

A pure function maps one admitted Kiln workflow effect into a bounded Lattice request. It receives all values explicitly and performs no transport or file access.

The shell owns encoding, transport, credentials, clocks, streaming, and process or service lifecycle.

### Decision: Bind graph and CI identities

The request binds the Kiln effect, attempt, candidate, pipeline revision, workflow policy, graph identity, and idempotency key.

The result preserves Lattice workflow, execution, node, adapter, receipt, and log identities. Native identities never replace Kiln identities.

### Decision: Admit effect capabilities before dispatch

The selected profile lists supported graph versions, node kinds, effect classes, controls, observation modes, and bounds.

Unsupported credentials, host effects, dynamic graph changes, or adapter kinds fail before Lattice receives the request.

### Decision: Keep lifecycle meanings separate

Lattice pause, cancellation request, or node completion is a provider observation. The Kiln core decides CI outcome changes.

A client timeout changes only the client result. It cannot mark a workflow or attempt timed out.

### Decision: Reconcile ambiguity

A disconnect after acceptance creates `Unknown`. Reconciliation uses the exact Lattice execution identity and rejects substituted or stale results.

### Decision: Keep logs bounded

Log reads use stable provider references and Kiln cursors. Gaps, truncation, and retention remain explicit.

## Configuration

Typed Nickel profiles define contract revisions, endpoints, graph versions, node and effect capabilities, controls, limits, timeouts, log policy, and non-claims.

## Validation

Positive fixtures cover dispatch, node progress, completion, logs, cancellation, pause, resume, and reconciliation.

Negative fixtures cover graph drift, unsupported nodes, hidden credentials, stale results, duplicate callbacks, disconnects, timeouts, log gaps, and oversized graphs.

## Claim Boundary

Passing checks establishes bounded request and observation compatibility. It does not prove workflow correctness, host confinement, external effect success, or release readiness.

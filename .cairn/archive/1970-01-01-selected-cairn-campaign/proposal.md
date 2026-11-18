# Why

Issue #62 needs Kiln to retain the operator's exact selected Cairn set and drive it through native readiness, canonical work/attempt ownership, delivery, and lifecycle observations across restart. Individual intake cannot account for blocked, planning-only, completion-only, or lifecycle obligations.

## What Changes

- Add a pure-core, versioned selected-set campaign with immutable revisions, exact member selectors, profile/policy/instruction/context bindings, endpoint goals, aggregate limits, and owner observation references.
- Add a durable SQLite campaign store/controller that persists action/effect intent before dispatch, resumes only by reconciling pending effects, and exposes bounded start/inspect/cancel/continue/reconcile operations.
- Add typed, versioned Cairn observation adapters that retain source, policy, subject, profile, completion, and lifecycle identity without inventing native authority.
- Add fixtures for selection freeze, blocked and partial membership, dependencies and independent work, overlap/cancellation, restart windows, drift, context, budget lineage, endpoint distinctions, and human-decision backpressure.
- Keep fixtures and operator helper leaves separate from real worker and remote qualification; no remote effects or lifecycle completion are performed.

## Authority and Boundaries

WorkLedger, AttemptLedger, admission, verification, publication/integration, and lifecycle observation remain canonical owners. Campaigns reference their identities and outcomes; they do not create a work database, claim, scheduler, verifier, or lifecycle authority. Cairn profile selection and lifecycle semantics remain native. Unsupported delivery routes remain blocked rather than falling back. Source edits must not change an in-flight attempt contract; material scope or policy change requires an explicit campaign revision/supersession decision.

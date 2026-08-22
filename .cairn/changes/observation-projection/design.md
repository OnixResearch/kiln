# Design: Observation projection core

## Context

Kiln commits every accepted event to an identity-checked journal. All operator-facing facts must rebuild from that journal. The task is the first rung of an observation surface: a pure, bounded, read-only projection core that later HTTP and iroh shells will serve.

Rung scope is deliberately narrow. No transport, no runtime, no logging framework, no control intake. The projection core is a pure crate with no new external dependency.

## Goal and Success Contract

The goal is a deterministic bounded observation projection with full positive and negative fixtures.

Completion requires a pure projection crate, a typed Nickel observation profile, registered view identities, enforced bounds, redaction, unknown-target handling, and conformance evidence.

False completion includes a projection that mutates state, a view without a bound, an unverifiable view identity, a missing negative fixture, or a claim that a view proves transport delivery or control execution.

## Decisions

### Decision: Own observation in a dedicated pure crate

Add `kiln-observe-core`, a pure `no_std + alloc` crate that depends only on existing workspace crates. `kiln-core` state types remain immutable inputs.

A dedicated crate keeps `kiln-core` stable and gives observation a distinct ownership and growth boundary. It mirrors the accepted `kiln-radicle-source-contract` precedent of one pure `no_std + alloc` capability crate.

Rejected alternative: extending `kiln-core`. It would couple every core consumer to the observation vocabulary and grow the core surface for a capability that later gains transport shells.

### Decision: Observation is read-only projection

Every view is a pure function of admitted state, profile, and window facts. Projection never mutates state and never requests effects.

Rejected alternative: serving views from a mutable projection cache. A cache is legal later as a shell-owned observation, but it must never become the source that canonical transitions read.

### Decision: Bound every view through a profile

The profile declares the view vocabulary, page limits, cursor windows, value and attribute caps, and redaction policy. Any request outside the vocabulary fails before projection.

Provider-native and shell-supplied facts cannot enter a view as unbounded attributes or unbounded record volumes.

### Decision: Admit shell-provided windows exactly

Event-tail and log-range views accept only explicit, admitted windows supplied by the shell. The core checks identity, ordering, and bounds; it never reads files, journals, or stores itself.

The store and log store keep their existing owners. Observation consumes their outputs as admitted observations, the same way every other Kiln adapter admits provider observations.

### Decision: Bind views to `kiln.observe.view.v1`

Every view carries a BLAKE3 identity over canonical view bytes, the source state revision, the request, and the window. An unknown state revision fails before projection.

A view identity never grants lifecycle or release authority.

### Decision: Enforce failures as bounded results

Unknown targets return a bounded not-found view or a bounded rejection. Pages are capped under the declared bound with explicit truncation. Out-of-window log cursors report the exact retained window. Redaction omits or masks undeclared fields before encoding.

Rejected alternative: silent truncation without a truncation fact. Truncation must be visible in the returned view.

### Decision: Controls are designed for, not included

A later change adds a submit-command control surface through the existing command and outbox path. This change only guarantees that observation never performs control itself.

## Configuration

Typed Nickel profiles declare the view vocabulary, page and window bounds, attribute and value caps, redaction policy, and non-claims. Positive and negative profile fixtures validate the contract.

## Validation

Positive fixtures cover every supported view, deterministic rebuild, identity parity, read-only enforcement, and retained-window reporting. Negative fixtures cover unknown views, unknown state revisions, unknown targets, page overflow, cursor windows outside retention, redaction, and missing negative cases.

## Claim Boundary

Passing checks establish bounded deterministic projection over admitted facts. They do not prove transport delivery, live freshness, control execution, provider correctness, CI success, or release readiness.

# Design: Operational Radicle provider shell

## Context

The trigger path now dispatches source-bound workflows through Lattice with durable replay recovery.

Status publication and patch-review mutation use separate completed profiles. The remaining work is exact local source acquisition.

## Goal and Success Contract

The goal is separate, concrete source and trigger shells with narrow capability profiles.

Completion requires these results:

- fixture success cannot enter a production provider profile;
- source resolution and acquisition bind one exact Radicle revision;
- trigger intake preserves exact broker event, repository, revision, and replay identities;
- ambiguous effects remain unknown until reconciliation;
- core and standalone builds remain independent from Radicle runtime code; and
- positive and negative fixtures cover each enabled capability.

False completion includes a renamed fixture service, hidden command execution, ambient identity lookup, silent fallback, or status that mutates canonical state.

## Decisions

### Decision: Keep narrow capability profiles

The adapter defines separate source and trigger profiles. Dedicated accepted changes own status and patch-review profiles.

A profile cannot gain another profile's credentials or operations. Provider admission rejects role or capability substitution before effects.

### Decision: Use a pure mapping core

Pure functions admit profile facts, map native observations, form effect plans, classify outcomes, and decide reconciliation.

The shell owns repository handles, processes, local sockets, clocks, credentials, signing, and network transport.

### Decision: Bind exact source facts

A source result preserves the RID, selected signed reference, exact Git OID, provider observation, and canonical BLAKE3 tree identity.

The adapter rejects moved references, missing objects, mixed observations, and tree mismatches. Equal trees do not imply equal history or authority.

### Decision: Keep trigger intake non-authoritative

A trigger can request discovery or evaluation. It cannot grant source acquisition, merge, push, signing, or publication authority.

The deployed broker schema does not carry an actor signature. Kiln records the broker boundary and does not infer an authenticated actor.

Replay state remains shell-owned and durable. Duplicate or malformed native events fail before command admission.

### Decision: Preserve fixture scope

`LocalFixtureService` remains available only for protocol fixtures and examples. The implemented adapter inventory requires a concrete shell and conformance evidence.

## Configuration

Typed Nickel profiles define versions, paths, node or repository identity, capabilities, operations, limits, timeouts, replay retention, status targets, signer policy, and non-claims.

## Validation

Positive fixtures cover exact source acquisition, valid triggers, durable replay, and reconciliation.

Negative fixtures cover moved refs, invalid signatures, replay, wrong RID, namespace substitution, timeout, disconnect, oversized input, and false success.

## Claim Boundary

Passing checks establishes bounded mapping and local provider observations for the selected profile. It does not prove peer convergence, actor trust, remote availability, or mutation success.

# Design: Operational Radicle provider shell

## Context

The current broker parser and response mapper are bounded and deterministic. The executable connects them to `LocalFixtureService`, which returns a fixed successful run.

This path proves protocol compatibility only. It does not prove source acquisition, trigger delivery, status publication, collaboration mutation, or reconciliation.

## Goal and Success Contract

The goal is a concrete optional Radicle shell with narrow capability profiles.

Completion requires these results:

- fixture success cannot enter a production provider profile;
- source resolution and acquisition bind one exact Radicle revision;
- trigger intake preserves native event and actor identities;
- status remains a rebuildable projection;
- collaboration mutation requires explicit operator authority;
- ambiguous effects remain unknown until reconciliation;
- core and standalone builds remain independent from Radicle runtime code; and
- positive and negative fixtures cover each enabled capability.

False completion includes a renamed fixture service, hidden command execution, ambient identity lookup, silent fallback, or status that mutates canonical state.

## Decisions

### Decision: Keep four narrow profiles

The adapter defines separate source, trigger, status, and collaboration profiles. A deployment can enable any subset.

A profile cannot gain another profile's credentials or operations. Provider admission rejects role or capability substitution before effects.

### Decision: Use a pure mapping core

Pure functions admit profile facts, map native observations, form effect plans, classify outcomes, and decide reconciliation.

The shell owns repository handles, processes, local sockets, clocks, credentials, signing, and network transport.

### Decision: Bind exact source facts

A source result preserves the RID, selected signed reference, exact Git OID, provider observation, and canonical BLAKE3 tree identity.

The adapter rejects moved references, missing objects, mixed observations, and tree mismatches. Equal trees do not imply equal history or authority.

### Decision: Keep trigger intake non-authoritative

A trigger can request discovery or evaluation. It cannot grant source acquisition, merge, push, signing, or publication authority.

Replay state remains shell-owned and durable. Invalid signatures and duplicate native events fail before command admission.

### Decision: Keep status rebuildable

Status publication binds one exact Kiln attempt, provider profile, native target, and idempotency key.

A status write cannot complete an attempt or change the journal. Ambiguous writes enter reconciliation.

### Decision: Require explicit mutation authority

Collaboration mutation uses preview-first plans with expected heads and exact targets. The shell reloads current state before signing or writing.

No profile loads an ambient signer. The operator supplies an explicit signer capability for each authorized mutation path.

### Decision: Preserve fixture scope

`LocalFixtureService` remains available only for protocol fixtures and examples. The implemented adapter inventory requires a concrete shell and conformance evidence.

## Configuration

Typed Nickel profiles define versions, paths, node or repository identity, capabilities, operations, limits, timeouts, replay retention, status targets, signer policy, and non-claims.

## Validation

Positive fixtures cover exact source acquisition, valid triggers, idempotent status, authorized preview-first mutation, and reconciliation.

Negative fixtures cover moved refs, invalid signatures, replay, wrong RID, stale heads, missing signer authority, timeout, disconnect, oversized input, and false success.

## Claim Boundary

Passing checks establishes bounded mapping and local provider observations for the selected profile. It does not prove peer convergence, actor trust, remote availability, or mutation success.

# Design: Valence evidence adapter

## Context

Kiln produces canonical CI events and bounded provider observations. Valence can wrap these facts in shared Evidence IR and sidecars.

The adapter must preserve meaning and claim limits. Valence linkage cannot replace Kiln state or strengthen provider observations.

## Goal and Success Contract

The goal is a versioned `EvidencePort` that exports and validates bounded Valence artifacts for exact Kiln subjects.

Completion requires deterministic projection, exact subject identity, explicit roles, non-claims, round-trip fixtures, and schema drift rejection.

False completion includes role promotion, missing subject bytes, Valence IDs replacing Kiln IDs, or imported evidence changing canonical outcomes.

## Decisions

### Decision: Use a pure projection core

A pure function maps admitted Kiln subjects, facts, observations, roles, spans, and non-claims into a bounded adapter value.

The shell owns serialization, files, transport, Valence calls, and receipt handling.

### Decision: Bind exact subjects

Every artifact binds an exact Kiln candidate, pipeline, run, attempt, product, event, or receipt identity. The selected subject kind remains explicit.

A changed subject requires a new projection and evidence identity.

### Decision: Preserve verification roles

Producer, checker, builder, signer, provider, worker, linker, and consumer roles remain distinct. The adapter rejects unsupported or contradictory role assignments.

Kiln is not named as builder, signer, or checker unless the source facts assign that exact role.

### Decision: Carry non-claims

Each profile defines required non-claim classes. The projection carries provider and Kiln claim boundaries without strengthening them.

Missing required non-claims makes the projection incomplete.

### Decision: Keep imports observational

Imported Valence artifacts can supply linkage observations. They cannot create runs, complete attempts, authorize merge, or establish release readiness.

### Decision: Round-trip exact native facts

Round-trip tests preserve every supported identity, role, span, algorithm tag, evidence reference, and non-claim. Lossy mappings require explicit profile policy.

## Configuration

Typed Nickel profiles define Valence revisions, schema versions, subject kinds, roles, span kinds, bounds, transport, lossy policy, and required non-claims.

## Validation

Positive fixtures cover run, attempt, product, build, gate, and source evidence. Negative fixtures cover missing subjects, role promotion, stale links, malformed spans, unsupported versions, and lossy conversion.

## Claim Boundary

Passing checks establishes identity and linkage conformance for the selected artifacts. It does not prove CI behavior, provider correctness, or release eligibility.

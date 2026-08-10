# Design: CDEvents interchange adapter

## Context

Kiln's interchange core models versioned external events and rejects unsupported or lossy mappings. It lacks concrete CDEvents and CloudEvents codecs.

The adapter must preserve native algorithms and identifiers. Imported events cannot replace canonical Kiln events.

## Goal and Success Contract

The goal is bounded deterministic CDEvents import and export for a small pinned event set.

Completion requires exact version pins, pure per-type mappings, duplicate handling, correlation checks, round-trip fixtures, and authority rejection.

False completion includes a generic JSON map, acceptance of unknown event types, silent field loss, imported canonical success, or transport authority hidden in the codec.

## Decisions

### Decision: Support an explicit event registry

The profile lists each supported CDEvents type, version, subject version, CloudEvents binding, required field, optional field, and loss policy.

Unknown types fail as unsupported. A new event type requires a profile and fixtures.

### Decision: Separate envelopes and semantics

A pure CloudEvents codec admits envelope facts. A type-specific pure mapper handles CDEvents subject and data facts.

Successful envelope parsing does not imply semantic mapping success.

### Decision: Preserve correlation and deduplication

Imports preserve native event ID, source, subject, correlation, causation, and observed time facts.

Duplicate decisions use the admitted native source and event identity. Missing required correlation fails without creating a linked observation.

### Decision: Keep imports observational

Imported events can enter a bounded observation store or request explicit discovery under separate policy. They cannot create canonical completion, merge authority, or release readiness.

### Decision: Export from canonical facts

Exports derive from committed Kiln events and admitted observations. Each export binds the exact source identity and mapping profile.

Provider-native details remain extensions only when the profile declares them.

### Decision: Keep transport separate

The first crate encodes and decodes values without network authority. Message buses and webhooks use separate transport profiles or adapters.

## Configuration

Typed Nickel profiles define specification versions, event registry, required fields, extensions, loss policy, bounds, duplicate policy, correlation policy, and non-claims.

## Validation

Positive fixtures cover each supported import and export. Negative fixtures cover unknown types, versions, fields, missing links, duplicates, malformed envelopes, and authority promotion.

## Claim Boundary

Passing checks establishes format and mapping compatibility for the pinned event set. It does not prove external event truth, delivery, provider correctness, or CI outcome.

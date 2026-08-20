# Design: Align provider port ownership

## Context

`kiln-provider-api` defines neutral ports, but current adapter crates do not implement those contracts for several existing paths. They define public provider-specific traits instead.

The same modules also call providers and decide admission, ambiguity, sequencing, or terminal meaning. This makes the adapter an application contract owner and policy owner.

## Goal and Success Contract

Kiln must own provider contracts and deterministic decisions. Adapters must only translate and perform approved external work.

Completion requires these results:

- each external capability has one application-owned contract;
- pure decisions run from ordinary values without adapter calls;
- concrete adapters implement the owned contract;
- infrastructure and domain failures remain distinct;
- one visible composition root selects implementations; and
- positive and negative conformance tests cover every migrated capability.

False completion includes renaming adapter traits, moving files without changing ownership, or wrapping every adapter in the existing generic dispatch method.

## Decisions

### Decision: Put capability contracts in the provider application API

`kiln-provider-api` owns narrow requests, observations, and failures for source observation, exact resolution, acquisition, collaboration mutation, remote execution, and reconciliation.

Contracts use Kiln terminology. Provider-native references remain typed facts inside those contracts.

### Decision: Keep core decisions value-based

`kiln-core` receives explicit profiles, prior state, provider observations, authority facts, and current operation facts.

It returns admission decisions, accepted observations, new state, events, or effect plans. It does not call a repository, signer, transport, or broker service.

Jujutsu ambiguity, COB freshness, remote capability admission, and outcome classification remain deterministic core decisions.

### Decision: Make adapters implement owned ports

The Jujutsu, Radicle, COB, and remote crates translate provider data into application observations. They execute only effects approved by the application shell.

Provider-specific helper traits can remain private implementation details. They must not become competing public application contracts.

### Decision: Compose adapters in an application shell

The standalone service or another explicit application shell loads profiles and external facts. It calls the core, executes approved plans, and revalidates returned observations.

Only the composition root selects concrete providers. Core and provider API crates do not select implementations.

### Decision: Preserve error ownership

Core decisions return typed domain errors. Provider ports return typed application observations or typed infrastructure failures.

Adapters translate SDK, transport, storage, credential, timeout, and cancellation errors. The shell decides whether an infrastructure result is retryable, unknown, denied, or terminal.

## Compatibility

Keep existing canonical identities, provider-native references, accepted request bytes, and outcome classes unchanged. Use explicit compatibility adapters for supported callers during migration.

## Validation

Positive tests cover exact source resolution, current COB mutation, accepted remote execution, broker intake, and successful reconciliation.

Negative tests cover unsupported capability, stale heads, divergent Jujutsu state, moved refs, missing signer, malformed provider data, timeout, cancellation, disconnection, and unknown outcomes.

Dependency checks must reject provider implementations from core and provider API crates. They must also reject new public provider contracts in concrete adapter crates.

## Claim Boundary

Passing these checks proves only contract ownership, deterministic decision behavior, and checked adapter translation for the tested cohort. It does not prove provider correctness or effect completion.

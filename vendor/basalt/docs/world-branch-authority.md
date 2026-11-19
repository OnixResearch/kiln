# World-branch authority policy

Basalt owns portable policy meaning for authority that may accompany a branched world. Molten owns runtime enforcement.

A passing decision does not mint, copy, derive, transfer, activate, promote, or execute a capability.

## Inputs

The pure Rust core consumes normalized supplied facts:

- one closed capability class;
- one branch action;
- source and destination branch identities;
- one capability identity;
- source and destination resource, ability, and limit scopes;
- the selected policy generation;
- a lossless consumer-mapping observation;
- current policy, capability, revocation, replay, and scope observations;
- an optional verified UCAN-composition observation.

The core does not parse tokens, verify signatures, resolve keys, read clocks, query stores, or evaluate Nickel at runtime.

## Closed modes

| Mode | Basalt obligation | Molten enforcement owner |
|---|---|---|
| `copyable` | independent destination grant | grant validation and activation |
| `attenuated` | strictly narrower derivation | UCAN derivation and scope enforcement |
| `linear` | generation-fenced transfer and source deactivation | durable ownership mutation and recovery |
| `simulation-only` | exact deterministic simulation adapter | adapter selection and host-effect denial |
| `promotion-gated` | promotion recheck and release reservation | promotion and effect-release admission |
| `replace-before-activation` | fresh destination grant | key or credential replacement and activation |
| `non-branchable` | deny activation | denial enforcement |

Unknown modes fail Nickel and Rust decoding.

## Scope checks

A destination resource must remain under the source resource prefix. Destination abilities must be a subset of source abilities.

A destination limit cannot exceed the source limit. An attenuated grant must narrow at least one resource, ability, or limit field.

A policy rule cannot widen a consumer request. Human approval or a stored receipt cannot override these checks.

## Currentness

Every rule requires current supplied observations. Basalt denies stale policy, capability, revocation, replay, or scope facts.

Modes that depend on UCAN semantics also require a separately verified composition observation. Basalt does not treat token possession as composition evidence.

Molten must recheck these facts when it realizes obligations. A Basalt receipt cannot authorize later activation or promotion.

## Linear and simulation boundaries

Linear policy describes obligations only. Molten must mark source ownership unavailable before destination activation.

Unknown transfer outcomes require observation-first recovery. Blind retry cannot establish exclusivity.

Simulation-only decisions admit only a named simulation obligation. A live action for the same capability is denied instead of falling back.

## Evidence

Decision receipts contain only domain-separated BLAKE3 identities, selected mode, obligations, diagnostics, and fixed non-claims.

Receipts exclude resources, abilities, capability paths, bearer tokens, private keys, credentials, secret entropy, and private policy bodies.

The frozen Molten mapping is a fixture. It records that Basalt owns policy meaning while Molten retains parsing, currentness, derivation, transfer, adapters, activation, promotion, and effects.

## Non-claims

The policy does not prove:

- capability authenticity;
- current global revocation state;
- replay-store durability;
- successful derivation or transfer;
- source deactivation;
- destination activation;
- simulator parity or confinement;
- effect dispatch;
- release eligibility.

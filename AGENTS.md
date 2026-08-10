# Agent Notes

Read `README.md` before changing this repository.

## Product boundary

Kiln owns provider-neutral CI domain semantics. Keep Aspen, Mantle, Lattice,
GitHub, Radicle, Git, Pijul, and Jujutsu behind versioned adapter ports.
Treat Jujutsu as a source provider, not a forge or collaboration provider.

Core crates must not import provider implementations. Provider identifiers must
not become primary Kiln identifiers. Unsupported capabilities must fail before
external effects.

## Core and shell

Use a pure functional core and thin imperative shells. Pure functions decide
state transitions, admission, ordering, retry classification, and requested
effects. Shells own files, databases, clocks, networks, processes, credentials,
and provider calls.

Every non-trivial function must keep this split. A test must exercise core logic
without starting an external service.

## Configuration and identity

Use typed Nickel for human-authored configuration. Use BLAKE3 for Kiln-owned
content identities and deterministic identifiers. Preserve protocol-mandated
algorithms and provider-native identities where interoperability requires them.

Do not hard-code unexplained numeric values. Use named limits, windows, bounds,
and retry policies.

## Gating and interchange

Bind every gate result to one exact candidate-set identity. A moved target or
failed dependency requires a new candidate and evaluation. Keep merge authority
in collaboration-provider shells.

Keep Remote Execution API, SLSA, in-toto, CDEvents, CloudEvents, and
OpenTelemetry types out of core crates. Their adapters preserve native
algorithms and identities without replacing canonical Kiln state.

Use Kueue and DRF only as fairness references. Kiln owns global tenant ordering,
while execution providers retain placement and preemption.

## Radicle hosting

Radicle is the planned canonical repository host. This choice must not add a
Radicle dependency to core crates, standalone builds, source archives, or local
core tests.

Do not run Radicle initialization, signing, push, synchronization,
announcement, delegate mutation, threshold mutation, or mirror publication
without an explicit operator request. Planning and validation grant no remote
mutation authority.

Treat GitHub, HTTPS, and other mirrors as declared projections. Preserve exact
RID, revision, archive, signed-ref, delegate, threshold, and acquisition
evidence after publication.

## Tests

Write positive and negative tests together. Adapter conformance must test valid
operations, malformed inputs, unsupported capabilities, stale identities,
replayed triggers, timeouts, cancellation, ambiguous outcomes, and provider
failure.

## Lifecycle and VCS

Use native `.cairn/` lifecycle packages. Validate proposal, design, and task
gates before implementation. Use `jj` for version-control operations.

Do not edit `flake.lock` manually. Do not push without an explicit request.

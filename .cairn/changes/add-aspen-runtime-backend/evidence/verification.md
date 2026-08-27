# Verification: Aspen runtime backend

## Baseline

Before implementation, `nix develop path:$PWD -c cargo test --workspace` passed across the existing Kiln workspace.

## Completed implementation evidence

The active change now contains:

- an exact Aspen profile for local revision `7fb4c9de293f815654e1e3b954f4fee3678ac87f`;
- the exact Bounded Exec revision `29dac88ecded94457572db3fdfaaaab95fa91525`;
- typed Nickel contracts, one positive fixture, and five negative fixtures;
- distinct Kiln and Aspen identity types and mapping receipts;
- atomic `EventStorePort` journal, outbox, and revision mapping;
- typed content mapping with missing, corrupt, and overbound denial;
- exact process, transport, content, time, scheduling, and evidence effect plans;
- Lattice workflow-exchange and remote-lifecycle wrappers;
- Radicle and operator ingress client contracts;
- generation-fenced callback state, completion, checkpoint, recovery, drain, and shutdown decisions;
- a one-shot inner Kiln callback protocol and process fixture;
- deterministic local durability, content, service, restart, replay, and uncertainty simulations;
- independent static producer and consumer callback fixtures;
- dependency and ambient-effect architecture tests;
- operator, recovery, ownership, rollout, and non-claim documentation;
- a pinned strict Octet gate for the new adapter and extension source.

## Checks that pass

- `nix develop path:$PWD -c cargo fmt --all -- --check`.
- `nix develop path:$PWD -c cargo test --workspace`.
- `nix develop path:$PWD -c cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `nix develop path:$PWD -c cargo test -p kiln-adapter-aspen -p kiln-aspen-extension -p kiln-adapter-radicle`.
- The adapter suite passes 13 positive and negative tests.
- The callback and hosted-model suites pass 6 positive and negative tests.
- The existing Radicle adapter suite passes 38 tests.
- `nix build path:$PWD#checks.x86_64-linux.aspen-runtime-profile -L --option builders ''`.
- `nix build path:$PWD#checks.x86_64-linux.aspen-runtime-octet-deny-all -L --option builders ''`.
- The exact strict Octet cohort uses Preserves `5.0.0-rc.7` and BLAKE3 `1.8.6`.
- Strict Octet reports `Status: clean`, with zero findings, warnings, and errors.
- Nickel formatting, profile export, positive export, and all five negative fixtures pass.
- Cairn validation, proposal gate, design gate, and structural tasks gate pass under `kiln-default`.

Tracey reports every `kiln.aspen_runtime.*` marker as dangling because `kiln-default` reads requirements only from accepted `.cairn/specs`. Sync is intentionally blocked, so the active delta remains outside the accepted requirement set.

The local Radicle-to-model fixture produces a deterministic run and preserves the provider result as `unknown`. It is simulation evidence, not Aspen-host evidence.

## Exact blocker

Aspen revision `7fb4c9de293f815654e1e3b954f4fee3678ac87f` cannot carry the values required by Kiln's callback.

Its native callback envelope carries `event_ref`, optional `payload_ref`, and optional `state_ref`. Its callback outcome carries output, effect, state, and checkpoint references. It does not materialize the canonical command, prior state, effect-completion body, effect request body, or next state bytes for the one-shot process.

A BLAKE3 reference cannot reconstruct those values. Moving the transition into the ingress client would transfer CI authority out of the hosted callback. Treating `LocalAspenService` as Aspen would create false hosted evidence.

An Aspen prerequisite must add a new bounded value-materialization contract. It must verify every reference against exact bytes and preserve durable intent before content publication or provider effects. It must cover ingress, prior state, effect completion, effect bodies, next state, and checkpoints. The reference-only profile must not be a fallback.

## Incomplete gates

These gates remain incomplete:

- actual Aspen-native callback execution;
- deployed Radicle and operator ingress to an Aspen host;
- visible production Aspen composition;
- exact Aspen-host callback negative tests;
- exact hosted end-to-end success, rejection, restart, and reconciliation;
- final Cairn tasks, traceability, sync, and archive gates.

The Cairn change remains active. It must not be synced or archived yet.

## Claim boundary

Passing checks prove the pure mappings, local models, process protocol, and fail-closed classifications for the tested Kiln source.

They do not prove Aspen integration, CI correctness, provider correctness, sandboxing, exact-once delivery, source trust, network delivery, global durability, production availability, or release eligibility.

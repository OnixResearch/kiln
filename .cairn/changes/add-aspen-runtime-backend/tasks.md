# Tasks: Add the Aspen runtime backend

## Phase 1: Contracts and profiles

- [x] [serial] Pin the immutable Aspen source, callback, transport, durability, content, scheduling, evidence, and bounded-execution contract cohort. r[kiln.aspen_runtime.profile]
- [x] [depends:aspen-bounded-execution-fabric-port] Record the accepted Aspen bounded-execution port and its exact Bounded Exec component revision. r[kiln.aspen_runtime.effects] r[kiln.aspen_runtime.nonclaims]
- [x] [depends:aspen-native-system-extension-host] Record the accepted native-process host contract, execution profile, callback framing, and lifecycle evidence schemas. r[kiln.aspen_runtime.callback] r[kiln.aspen_runtime.lifecycle]
- [x] [parallel] Replace the profile-only `aspen-runtime-v1` declaration with typed Nickel contracts and positive and negative fixtures. r[kiln.aspen_runtime.profile] r[kiln.aspen_runtime.composition]
- [x] [parallel] Add independent producer and consumer fixtures for the Kiln-owned extension protocol. r[kiln.aspen_runtime.callback] r[kiln.aspen_runtime.identity]

## Phase 2: Pure mappings and durable state

- [x] [serial] Add pure request, callback, state, checkpoint, effect, completion, status, and identity mappings outside core dependency graphs. r[kiln.aspen_runtime.boundary] r[kiln.aspen_runtime.callback] r[kiln.aspen_runtime.identity]
- [x] [serial] Implement the Aspen `EventStorePort` mapping with expected revisions and atomic journal, outbox, and revision updates. r[kiln.aspen_runtime.durability]
- [x] [serial] Implement content mappings for source manifests, input roots, outputs, logs, and checkpoints with exact role linkage. r[kiln.aspen_runtime.content]
- [x] [parallel] Add positive and negative pure tests for canonical order, drift, conflicts, stale generations, false success, and identity substitution. r[kiln.aspen_runtime.durability] r[kiln.aspen_runtime.identity] r[kiln.aspen_runtime.conformance]

## Phase 3: Callback shell and runtime composition

- [ ] [depends:aspen-native-callback-value-materialization] Add the bounded native callback executable with cleared environment, canonical standard input and output, named limits, and no provider effects. r[kiln.aspen_runtime.callback]
- [x] [serial] Map approved provider process, transport, content, time, scheduling, and evidence effects to exact Aspen port requests. r[kiln.aspen_runtime.effects]
- [ ] [depends:aspen-native-callback-value-materialization] Add Radicle and operator ingress clients that submit admitted triggers to the hosted service before workflow dispatch. r[kiln.aspen_runtime.ingress]
- [x] [serial] Add generation-fenced activation, checkpoint, recovery, drain, shutdown, and unresolved-effect reconciliation. r[kiln.aspen_runtime.lifecycle] r[kiln.aspen_runtime.failure]
- [ ] [depends:aspen-native-callback-value-materialization] Add a visible application composition root that selects Aspen without changing standalone, Lattice, Mantle, Cairn, or Valence ownership. r[kiln.aspen_runtime.boundary] r[kiln.aspen_runtime.composition]

## Phase 4: Conformance and closeout

- [x] [parallel] Add adapter tests for atomic conflict, post-accept disconnect, timeout, cancellation, missing content, output bounds, and unavailable ports. r[kiln.aspen_runtime.failure] r[kiln.aspen_runtime.conformance]
- [ ] [depends:aspen-native-callback-value-materialization] Add callback tests for valid Aspen-host input, malformed bytes, oversized input, denied authority, deterministic repeat, and effect-free denial. r[kiln.aspen_runtime.callback] r[kiln.aspen_runtime.conformance]
- [ ] [depends:aspen-native-callback-value-materialization] Run simulated, separate native callback, local live Aspen service, restart, replay, and reconciliation fixtures with one accepted and one rejected trigger. r[kiln.aspen_runtime.lifecycle] r[kiln.aspen_runtime.conformance]
- [x] [serial] Add architecture checks that reject Aspen core imports, ambient callback effects, raw runtime handles, mutable sibling dependencies, and hidden fallback. r[kiln.aspen_runtime.boundary] r[kiln.aspen_runtime.composition]
- [x] [serial] Document setup, ownership, port mappings, recovery, operator status, rollout gates, and all non-claims. r[kiln.aspen_runtime.nonclaims]
- [ ] [depends:aspen-native-callback-value-materialization] Run formatting, workspace tests, Clippy, Octet, Nickel, Cairn validation and gates, traceability, and relevant Nix checks after exact Aspen-host conformance. r[kiln.aspen_runtime.conformance] r[kiln.aspen_runtime.nonclaims]

## Verification Coverage

- `Scenario: Exact profile is selected` -> profile and fixture tasks
- `Scenario: Core imports Aspen runtime code` -> dependency-boundary checks
- `Scenario: A valid callback runs` -> callback executable and callback tests
- `Scenario: Expected revision conflicts` -> durable-state mapping and adapter tests
- `Scenario: Commit acknowledgement is lost` -> unknown-outcome and reconciliation tests
- `Scenario: Kiln denies the provider effect` -> effect-free denial tests
- `Scenario: Equal digest text has different roles` -> identity mapping tests
- `Scenario: A trigger is replayed` -> ingress idempotency tests
- `Scenario: Service recovers after committed intent` -> restart and replay fixture
- `Scenario: Aspen is unavailable` -> explicit-selection negative fixture
- `Scenario: Hosted end-to-end fixture passes` -> local live conformance fixture
- `Scenario: A local fixture passes` -> scoped operator status and documentation

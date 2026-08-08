## Phase 1: Repository and canonical model

- [x] [serial] Record exact Hydra, Radicle CI, Radicle native CI, Jujutsu, Choregraph, Aspen, Mantle, Lattice, Valence, Cairn, Zuul, Concourse, Bazel Remote Execution API, SLSA, in-toto, TUF, Kueue, DRF, Temporal, CDEvents, OpenTelemetry CI/CD, and Buildbot revisions or specification versions, licenses, selected concepts, rejected concepts, maturity, and non-claims. r[kiln.domain.boundary] r[kiln.radicle.nonclaims] r[kiln.gating.nonclaims] r[kiln.remote.nonclaims] r[kiln.interchange.nonclaims]
- [x] [serial] Create the Rust workspace, root license decision, Nix development surface, typed Nickel project profile, documentation layout, and adapter dependency policy without manually editing `flake.lock`. r[kiln.providers.core_independence] r[kiln.operations.configuration]
- [x] [depends:workspace-layout] Add `no_std + alloc` domain, canonical model, provider API, codec, standalone shell, CLI, Radicle compatibility, and conformance crate boundaries. r[kiln.domain.model] r[kiln.providers.versioning]
- [x] [parallel] Add positive dependency-boundary checks for allowed core, shell, codec, and adapter edges. r[kiln.providers.core_independence]
- [x] [parallel] Add negative checks that reject Aspen, Mantle, Lattice, provider SDK, filesystem, process, network, clock, credential, and persistence dependencies from the core. r[kiln.providers.core_independence] r[kiln.domain.transition]

## Phase 2: Pure CI domain and identity

- [x] [serial] Define bounded project, pipeline revision, trigger, candidate-set, gate, evaluation, run, task, attempt, product, metric, notification, command, event, effect, decision, and diagnostic types. r[kiln.domain.model] r[kiln.domain.bounds] r[kiln.gating.candidate_set]
- [x] [depends:domain-types] Define canonical packed Preserves profiles, typed BLAKE3 identities, human aliases, and provider-native external references without hashing inside the pure core. r[kiln.domain.identity] r[kiln.providers.external_identity]
- [x] [depends:domain-types] Implement one pure command decision and event evolution kernel with explicit lifecycle invariants and no shell-specific status machine. r[kiln.domain.transition] r[kiln.domain.lifecycle]
- [x] [depends:domain-transition] Implement retry, cancellation, reconciliation, attempt identity, product, metric, and notification decisions with explicit unknown outcomes. r[kiln.domain.attempts] r[kiln.domain.products]
- [x] [parallel] Add positive domain fixtures for project creation, immutable revisions, evaluation, run progression, retry, cancellation, products, metrics, and terminal completion. r[kiln.domain.fixtures]
- [x] [parallel] Add negative domain fixtures for malformed identity, duplicate alias, terminal mutation, stale command, invalid transition, reused attempt, unknown outcome collapse, oversized input, and overclaim. r[kiln.domain.fixtures] r[kiln.domain.boundary]

## Phase 3: Provider contracts and standalone conformance

- [x] [serial] Define separate versioned source, collaboration, trigger, status, workflow, build, event-store, notification, evidence, and gate ports with bounded requests, observations, and outcome classes. r[kiln.providers.composition] r[kiln.providers.versioning]
- [x] [depends:provider-ports] Implement pure capability-profile admission for operations, limits, identity namespaces, cancellation, usage facts, and outcome support. r[kiln.providers.capabilities] r[kiln.providers.no_fallback]
- [x] [depends:provider-capabilities] Define dispatch, completion, cancellation, reconciliation, idempotency, and external-reference contracts that preserve ambiguous outcomes. r[kiln.providers.effects] r[kiln.providers.ambiguity]
- [x] [parallel] Add a reusable provider conformance suite with valid lifecycle, deterministic repeat, cancellation, timeout, and reconciliation cases. r[kiln.providers.conformance]
- [x] [parallel] Add negative conformance cases for unknown versions, absent capabilities, provider substitution, stale completion, duplicate callback, malformed receipt, false success, and hidden fallback. r[kiln.providers.conformance] r[kiln.providers.no_fallback]
- [x] [depends:provider-conformance] Implement deterministic standalone fixture providers and prove an end-to-end run without Aspen, Mantle, or Lattice. r[kiln.providers.standalone]

## Phase 4: Source and collaboration boundaries

- [x] [serial] Define neutral repository, named-reference, source-snapshot, proposed-change, source-event, actor-reference, and source-resolution observation types without universal commit, branch, or pull-request assumptions. r[kiln.source.model]
- [x] [depends:source-model] Implement pure validation for symbolic-to-immutable resolution, canonical tree manifests, BLAKE3 tree identity, and immutable evaluation binding. r[kiln.source.resolution] r[kiln.source.tree_identity]
- [x] [depends:source-model] Define distinct source discovery, exact resolution, acquisition, collaboration, trigger, status, identity-observation, credential-shell, and mutation capability profiles for Git, Pijul, Jujutsu, GitHub, and Radicle mappings. r[kiln.source.port_split] r[kiln.source.native_semantics] r[kiln.source.observation_split]
- [x] [depends:source-resolution] Implement a versioned Jujutsu source-provider shell for admitted repository observation, bookmark and revset discovery, exact-one commit resolution, and snapshot acquisition without forge authority. r[kiln.source.jujutsu] r[kiln.source.native_semantics]
- [x] [depends:source-resolution] Add proposed-change candidate and mirror policy contracts that require exact provider-produced snapshots and explicit disagreement handling. r[kiln.source.proposed_change] r[kiln.source.mirrors]
- [x] [parallel] Add positive Git, Pijul, Jujutsu, GitHub, and Radicle mapping fixtures that preserve native identities and one canonical materialized snapshot. r[kiln.source.fixtures] r[kiln.source.jujutsu]
- [x] [parallel] Add negative moving-reference, missing revision, tree mismatch, unsupported Jujutsu backend, ambiguous Jujutsu revset, conflicted bookmark, divergent change, forged actor, replayed trigger, invalid protocol signature, mirror disagreement, and false-history-equivalence fixtures. r[kiln.source.fixtures] r[kiln.source.jujutsu] r[kiln.source.trigger_security]

## Phase 5: Durable operations and fair admission

- [x] [serial] Define versioned journal event, optimistic revision, outbox effect, lease, idempotency, completion, checkpoint, and projection records. r[kiln.operations.journal] r[kiln.operations.outbox]
- [x] [depends:journal-records] Implement a pure history validator and replay fold over the production evolution kernel with no fresh effect output, plus retained-history compatibility and explicit deterministic migration checks. r[kiln.operations.replay] r[kiln.operations.replay_compatibility]
- [x] [depends:journal-records] Implement the standalone transactional journal and outbox shell with interruption, duplicate, lease-expiry, and restart handling. r[kiln.operations.outbox] r[kiln.operations.recovery]
- [x] [serial] Define typed Nickel contracts for named accounting windows, decay, resource normalization, dimension aggregation, weighted-scalar and dominant-share strategies, shares, stale-usage policy, rounding, tie-breaks, limits, retries, retention, and provider selection. r[kiln.operations.configuration] r[kiln.operations.fairness] r[kiln.operations.fairness_strategy]
- [x] [depends:fairness-config] Implement pure tenant admission and fairness scoring over explicit time and normalized usage facts while leaving worker placement to providers. r[kiln.operations.fairness] r[kiln.operations.fairness_strategy] r[kiln.operations.fairness_invariants] r[kiln.operations.usage]
- [x] [parallel] Add positive deterministic replay, retained-history compatibility, restart recovery, duplicate suppression, weighted ordering, dominant-share ordering, monotonic usage, and tie-break fixtures. r[kiln.operations.fixtures] r[kiln.operations.replay_compatibility] r[kiln.operations.fairness_invariants]
- [x] [parallel] Add negative corrupt or divergent history, lost outbox pairing, stale lease, unknown completion, invalid share, missing capacity basis, stale usage, unsupported unit, time regression, and exactly-once overclaim fixtures. r[kiln.operations.fixtures] r[kiln.operations.replay_compatibility] r[kiln.operations.fairness_strategy] r[kiln.operations.nonclaims]
- [x] [depends:operations-core] Add rebuildable CLI, API, status, metrics, and notification projections with bounded pagination and no mutation authority. r[kiln.operations.projections]

## Phase 6: Radicle repository hosting

- [x] [serial] Define a typed Nickel hosting profile for planned and observed state, canonical branch, RID, delegates, threshold, signed-ref feature policy, seed set, required replica observations, acquisition routes, mirrors, license, rollback, and non-claims. r[kiln.hosting.profile] r[kiln.hosting.canonical_radicle]
- [x] [depends:hosting-profile] Implement pure hosting-profile admission that rejects missing, malformed, stale, contradictory, silently authoritative, or unsupported values. r[kiln.hosting.profile] r[kiln.hosting.mirrors]
- [x] [serial] Add core and standalone dependency gates that build and test from an exact source archive without a Radicle node, `rad`, identity, or network. r[kiln.hosting.runtime_independence]
- [x] [depends:hosting-profile] Define an immutable publication-candidate manifest that binds source revision, archive BLAKE3, license, lifecycle state, release evidence, rollback revision, and profile identity. r[kiln.hosting.candidate]
- [x] [parallel] Add positive planned, candidate, published, consistent-bundle, and current-publication fixtures plus negative RID, license, delegate, threshold, signed-ref, mixed-bundle, rollback, freeze, stale observation, replica, mirror, write-route, bootstrap, and recovery fixtures. r[kiln.hosting.fixtures] r[kiln.hosting.bundle_consistency] r[kiln.hosting.freshness_rollback]
- [x] [serial] Prepare and validate the local publication candidate without Radicle initialization, signing, push, synchronization, announcement, or mirror mutation. r[kiln.hosting.publication_authority] r[kiln.hosting.candidate]
- [x] [depends:publication-candidate] After explicit operator authorization, initialize the Radicle project and publish only the exact validated candidate under the admitted delegate, threshold, branch, and signed-ref policy. r[kiln.hosting.publication_authority] r[kiln.hosting.signed_refs]
- [x] [depends:radicle-publication] Record one consistent publication bundle with the assigned RID, exact source revision, signed refs, monotonic publication sequence, required native replica observations, fresh native acquisitions, strict Git checks, read-only acquisition equality, and explicit time facts. r[kiln.hosting.signed_refs] r[kiln.hosting.acquisition] r[kiln.hosting.bundle_consistency] r[kiln.hosting.freshness_rollback]
- [x] [depends:radicle-acquisition] Configure declared read-only mirrors, test drift and write rejection, and keep mirror updates outside repository authority. r[kiln.hosting.mirrors]
- [x] [parallel] Document source backup, signed-ref capture, seed loss, delegate unavailability, mirror drift, rollback, governance change, and non-circular bootstrap. r[kiln.hosting.bootstrap] r[kiln.hosting.recovery] r[kiln.hosting.nonclaims]

## Phase 7: Radicle CI interoperability

- [x] [serial] Pin the Radicle CI broker protocol, native adapter source, trigger schema, result schema, job COB format, and interoperability limits used by the first profile. r[kiln.radicle.protocol] r[kiln.radicle.nonclaims]
- [x] [depends:radicle-protocol] Implement a strict line-JSON compatibility parser and pure mapping from supported broker triggers to canonical Kiln commands. r[kiln.radicle.trigger]
- [x] [depends:radicle-trigger] Implement the broker adapter shell that returns one Kiln run ID, exposes a bounded log URL when configured, waits or polls under named policy, and writes only protocol responses to standard output. r[kiln.radicle.lifecycle] r[kiln.radicle.result]
- [x] [depends:radicle-lifecycle] Classify malformed input, unsupported event, missing repository or revision, service rejection, timeout, cancellation, provider error, unknown outcome, broken output, and disconnect behavior without false success. r[kiln.radicle.result] r[kiln.radicle.nonclaims]
- [x] [parallel] Add positive broker push, patch, local trusted fixture, run-ID, terminal success, terminal failure, and log-link protocol tests. r[kiln.radicle.fixtures]
- [x] [parallel] Add negative empty input, non-line input, malformed JSON, unknown version, unsupported event, duplicate trigger, stdout noise, timeout, unknown outcome, and unsafe native-shell import tests. r[kiln.radicle.fixtures] r[kiln.radicle.native_runner]
- [x] [depends:radicle-protocol] Add an optional Radicle job COB status projection that cannot mutate canonical Kiln state or grant execution authority. r[kiln.radicle.status]
- [x] [serial] Document broker mode, future direct-node mode, node-local filtering, third-party CI observations, trust limits, and why `.radicle/native.yaml` is not implicit execution authority. r[kiln.radicle.native_runner] r[kiln.radicle.nonclaims]

## Phase 8: Exact change gating

- [x] [serial] Define candidate-set, dependency-edge, frozen-source-universe, gate-policy, gate-result, invalidation, queue-window, and merge-preflight canonical profiles. r[kiln.gating.candidate_set] r[kiln.gating.identity] r[kiln.gating.dependencies] r[kiln.gating.frozen_sources]
- [x] [depends:gating-model] Implement pure bounded dependency validation and exact candidate composition across admitted source and collaboration providers. r[kiln.gating.dependencies] r[kiln.gating.frozen_sources]
- [x] [depends:gating-model] Implement serial and optional speculative candidate admission with named fixed or adaptive window policy and no worker-placement decisions. r[kiln.gating.speculation] r[kiln.gating.window]
- [x] [depends:gating-composition] Implement deterministic failed-prefix invalidation, result applicability, candidate replacement, and reevaluation decisions. r[kiln.gating.invalidation]
- [x] [depends:gating-invalidation] Implement pure merge preflight and a collaboration effect request that denies stale target, approval, capability, candidate, or policy facts. r[kiln.gating.merge_authority]
- [x] [parallel] Add positive serial, speculative, cross-project, frozen-source, adaptive-window, invalidation, reevaluation, and merge-preflight fixtures. r[kiln.gating.fixtures]
- [x] [parallel] Add negative malformed, missing, cyclic, stale, oversized, moving-target, provider-mismatch, stale-result, unadmitted-speculation, and overclaim fixtures. r[kiln.gating.fixtures] r[kiln.gating.nonclaims]

## Phase 9: Standard interchange adapters

- [x] [serial] Pin the supported Remote Execution API capability, CAS, action, operation, cache, cancellation, and log schemas without importing generated protocol code into core. r[kiln.remote.optional] r[kiln.remote.identities] r[kiln.remote.capabilities]
- [x] [depends:remote-schema] Implement pure mappings between exact Kiln build effects and remote action, digest, operation, cache, output, and log facts. r[kiln.remote.action] r[kiln.remote.digests] r[kiln.remote.operations] r[kiln.remote.cache] r[kiln.remote.logs]
- [x] [depends:remote-mapping] Implement the optional remote-execution shell with admitted upload, execute, poll, wait, cancel, download, and bounded-log effects while preserving unknown outcomes. r[kiln.remote.operations] r[kiln.remote.logs]
- [x] [parallel] Add positive and negative capability, digest, action, cache, operation, cancellation, output, log, transport, identity-confusion, and unknown-outcome fixtures. r[kiln.remote.fixtures] r[kiln.remote.nonclaims]
- [x] [serial] Define versioned SLSA, in-toto, CDEvents, CloudEvents, and OpenTelemetry projection profiles that retain canonical Kiln authority. r[kiln.interchange.authority] r[kiln.interchange.provenance] r[kiln.interchange.builder] r[kiln.interchange.algorithms]
- [x] [depends:interchange-profiles] Implement pure bounded CDEvents conversion, correlation, duplicate, unknown-field, downgrade, and lossy-mapping decisions. r[kiln.interchange.cdevents] r[kiln.interchange.correlation] r[kiln.interchange.evolution]
- [x] [depends:interchange-profiles] Implement rebuildable evidence and telemetry projections through declared ports without lifecycle or release authority. r[kiln.interchange.telemetry] r[kiln.interchange.authority]
- [x] [parallel] Add positive and negative provenance, builder, signer, algorithm, CDEvents, correlation, duplicate, evolution, telemetry, authority-promotion, and non-claim fixtures. r[kiln.interchange.fixtures] r[kiln.interchange.nonclaims]

## Phase 10: Optional stack adapters and release evidence

- [x] [serial] Add exact optional adapter profiles for Mantle build facts, Lattice workflow execution, Aspen/Molten runtime services, Valence evidence projection, Cairn gates, and Choregraph machine intake without changing core types. r[kiln.providers.composition] r[kiln.providers.core_independence]
- [x] [parallel] Run each available stack adapter through the common positive and negative conformance suite and record unsupported capabilities. r[kiln.providers.conformance]
- [x] [serial] Add architecture, operator, provider-authoring, source-mapping, change-gating, recovery, fairness, remote-execution, interchange, Radicle interoperability, security, and non-claim documentation. r[kiln.domain.boundary] r[kiln.operations.nonclaims] r[kiln.gating.nonclaims] r[kiln.remote.nonclaims] r[kiln.interchange.nonclaims] r[kiln.radicle.nonclaims]
- [x] [serial] Add deterministic release evidence that binds source, schemas, Nickel contracts, canonical fixtures, test results, adapter profiles, protocol pins, hosting state, gating profiles, interchange profiles, toolchains, Cairn gates, and non-claims with BLAKE3 identities. r[kiln.domain.fixtures] r[kiln.providers.conformance] r[kiln.operations.fixtures] r[kiln.gating.fixtures] r[kiln.remote.fixtures] r[kiln.interchange.fixtures] r[kiln.radicle.fixtures] r[kiln.hosting.nonclaims]
- [x] [serial] Run formatting, workspace tests, strict Clippy, `no_std`, Nickel, dependency audits, gating, remote, interchange, protocol, and hosting fixtures, Cairn validation, Tracey coverage, and focused Nix checks. r[kiln.domain.fixtures] r[kiln.source.fixtures] r[kiln.operations.fixtures] r[kiln.gating.fixtures] r[kiln.remote.fixtures] r[kiln.interchange.fixtures] r[kiln.radicle.fixtures] r[kiln.hosting.fixtures]
- [x] [serial] Sync accepted specifications, confirm complete requirement coverage, archive the change with evidence, and rerun post-archive validation. r[kiln.domain.boundary] r[kiln.providers.no_fallback] r[kiln.operations.nonclaims] r[kiln.gating.nonclaims] r[kiln.remote.nonclaims] r[kiln.interchange.nonclaims] r[kiln.radicle.nonclaims] r[kiln.hosting.nonclaims]

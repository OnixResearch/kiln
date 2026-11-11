// Accepted-spec traceability index. The paths identify the implementation or conformance surface.
//
// Iroh native exchange conformance: crates/kiln-adapter-iroh/tests/workflow.rs.
// r[verify kiln.iroh.workflow.transport.scenario.valid]
// Actual native Lattice execution and bridge-restart observations:
// .cairn/archive/1970-01-01-add-iroh-workflow-transport/evidence/verification.json.
// r[verify kiln.iroh.workflow.conformance.scenario.local]
//
// Radicle COB model and adapters: crates/kiln-core/src/cob.rs, crates/kiln-adapter-cob/src/lib.rs.
// r[impl kiln.lifecycle.native_store]
// r[verify kiln.lifecycle.native_store.scenario.active_paths]
// r[verify kiln.lifecycle.native_store.scenario.parity]
// r[verify kiln.lifecycle.native_store.scenario.stale_plan]

// r[impl kiln.radicle_cobs.model]
// r[verify kiln.radicle_cobs.model.scenario.identity]
// r[verify kiln.radicle_cobs.model.scenario.history]
// r[impl kiln.radicle_cobs.profile_registry]
// r[verify kiln.radicle_cobs.profile_registry.scenario.supported]
// r[verify kiln.radicle_cobs.profile_registry.scenario.unknown]
// r[impl kiln.radicle_cobs.initial_profiles]
// r[verify kiln.radicle_cobs.initial_profiles.scenario.adr]
// r[verify kiln.radicle_cobs.initial_profiles.scenario.missing]
// r[impl kiln.radicle_cobs.semantic_boundary]
// r[verify kiln.radicle_cobs.semantic_boundary.scenario.reduced]
// r[verify kiln.radicle_cobs.semantic_boundary.scenario.import]
// r[impl kiln.radicle_cobs.capabilities]
// r[verify kiln.radicle_cobs.capabilities.scenario.read_only]
// r[verify kiln.radicle_cobs.capabilities.scenario.event_authority]
// r[impl kiln.radicle_cobs.effects]
// r[verify kiln.radicle_cobs.effects.scenario.fresh]
// r[verify kiln.radicle_cobs.effects.scenario.stale]
// r[impl kiln.radicle_cobs.conflicts]
// r[verify kiln.radicle_cobs.conflicts.scenario.concurrent]
// r[verify kiln.radicle_cobs.conflicts.scenario.partial]
// r[impl kiln.radicle_cobs.applicability]
// r[verify kiln.radicle_cobs.applicability.scenario.release]
// r[verify kiln.radicle_cobs.applicability.scenario.advance]
// r[impl kiln.radicle_cobs.status_projection]
// r[verify kiln.radicle_cobs.status_projection.scenario.disagreement]
// r[impl kiln.radicle_cobs.configuration]
// r[verify kiln.radicle_cobs.configuration.scenario.bound]
// r[verify kiln.radicle_cobs.configuration.scenario.drift]
// r[impl kiln.radicle_cobs.interop]
// r[verify kiln.radicle_cobs.interop.scenario.cairn]
// r[verify kiln.radicle_cobs.interop.scenario.malformed]
// r[impl kiln.radicle_cobs.nonclaims]
// r[verify kiln.radicle_cobs.nonclaims.scenario.local]
//
// Task operations core and shell: crates/kiln-core/src/task_ops.rs, crates/kiln-standalone/src/lib.rs.
// r[impl kiln.task_operations.reference_boundary]
// r[verify kiln.task_operations.reference_boundary.scenario.import]
// r[verify kiln.task_operations.reference_boundary.scenario.documentation]
// r[impl kiln.task_operations.status]
// r[verify kiln.task_operations.status.scenario.rebuild]
// r[verify kiln.task_operations.status.scenario.mutation]
// r[impl kiln.task_operations.lifecycle_facets]
// r[verify kiln.task_operations.lifecycle_facets.scenario.capacity]
// r[verify kiln.task_operations.lifecycle_facets.scenario.pause]
// r[impl kiln.task_operations.logs]
// r[verify kiln.task_operations.logs.scenario.restart]
// r[verify kiln.task_operations.logs.scenario.expired]
// r[impl kiln.task_operations.follow]
// r[verify kiln.task_operations.follow.scenario.complete]
// r[verify kiln.task_operations.follow.scenario.mismatch]
// r[impl kiln.task_operations.wait]
// r[verify kiln.task_operations.wait.scenario.terminal]
// r[verify kiln.task_operations.wait.scenario.deadline]
// r[impl kiln.task_operations.capacity]
// r[verify kiln.task_operations.capacity.scenario.available]
// r[verify kiln.task_operations.capacity.scenario.fairness]
// r[impl kiln.task_operations.readiness]
// r[verify kiln.task_operations.readiness.scenario.ready]
// r[verify kiln.task_operations.readiness.scenario.shell_mutation]
// r[impl kiln.task_operations.blocking_facts]
// r[verify kiln.task_operations.blocking_facts.scenario.release]
// r[verify kiln.task_operations.blocking_facts.scenario.clock]
// r[impl kiln.task_operations.controls]
// r[verify kiln.task_operations.controls.scenario.hold]
// r[verify kiln.task_operations.controls.scenario.unsupported]
// r[impl kiln.task_operations.redaction]
// r[verify kiln.task_operations.redaction.scenario.safe]
// r[verify kiln.task_operations.redaction.scenario.secret]
// r[impl kiln.task_operations.fixtures]
// r[verify kiln.task_operations.fixtures.scenario.missing_negative]
// r[impl kiln.task_operations.nonclaims]
// r[verify kiln.task_operations.nonclaims.scenario.logs]
//
// Neutral CI domain: crates/kiln-core/src/domain.rs and crates/kiln-core/src/types.rs.
// r[verify kiln.domain.attempts]
// r[verify kiln.domain.attempts.scenario.unknown]
// r[verify kiln.domain.boundary]
// r[verify kiln.domain.boundary.scenario.success]
// r[verify kiln.domain.bounds]
// r[verify kiln.domain.bounds.scenario.exceeded]
// r[verify kiln.domain.fixtures]
// r[verify kiln.domain.fixtures.scenario.missing_negative]
// r[verify kiln.domain.identity]
// r[verify kiln.domain.identity.scenario.alias]
// r[verify kiln.domain.lifecycle]
// r[verify kiln.domain.lifecycle.scenario.terminal]
// r[verify kiln.domain.model]
// r[verify kiln.domain.model.scenario.evaluation]
// r[verify kiln.domain.products]
// r[verify kiln.domain.products.scenario.orphan]
// r[verify kiln.domain.transition]
// r[verify kiln.domain.transition.scenario.deterministic]
//
// Exact change gating: crates/kiln-core/src/gating.rs.
// r[verify kiln.gating.candidate_set]
// r[verify kiln.gating.candidate_set.scenario.mixed]
// r[verify kiln.gating.dependencies]
// r[verify kiln.gating.dependencies.scenario.cycle]
// r[verify kiln.gating.fixtures]
// r[verify kiln.gating.fixtures.scenario.missing]
// r[verify kiln.gating.frozen_sources]
// r[verify kiln.gating.frozen_sources.scenario.branch_move]
// r[verify kiln.gating.identity]
// r[verify kiln.gating.identity.scenario.dependency_change]
// r[verify kiln.gating.invalidation]
// r[verify kiln.gating.invalidation.scenario.first_fails]
// r[verify kiln.gating.merge_authority]
// r[verify kiln.gating.merge_authority.scenario.target_moved]
// r[verify kiln.gating.nonclaims]
// r[verify kiln.gating.nonclaims.scenario.passed]
// r[verify kiln.gating.speculation]
// r[verify kiln.gating.speculation.scenario.disabled]
// r[verify kiln.gating.window]
// r[verify kiln.gating.window.scenario.shrink]
//
// Radicle hosting: crates/kiln-core/src/hosting.rs, config/default.ncl, and evidence/publication-receipt.ncl.
// r[verify kiln.hosting.acquisition]
// r[verify kiln.hosting.acquisition.scenario.replica]
// r[verify kiln.hosting.bootstrap]
// r[verify kiln.hosting.bootstrap.scenario.unavailable]
// r[verify kiln.hosting.bundle_consistency]
// r[verify kiln.hosting.bundle_consistency.scenario.mix]
// r[verify kiln.hosting.candidate]
// r[verify kiln.hosting.candidate.scenario.drift]
// r[verify kiln.hosting.canonical_radicle]
// r[verify kiln.hosting.canonical_radicle.scenario.mirror_tip]
// r[verify kiln.hosting.fixtures]
// r[verify kiln.hosting.fixtures.scenario.missing]
// r[verify kiln.hosting.freshness_rollback]
// r[verify kiln.hosting.freshness_rollback.scenario.older]
// r[verify kiln.hosting.mirrors]
// r[verify kiln.hosting.mirrors.scenario.write]
// r[verify kiln.hosting.nonclaims]
// r[verify kiln.hosting.nonclaims.scenario.scope]
// r[verify kiln.hosting.profile]
// r[verify kiln.hosting.profile.scenario.missing_rid]
// r[verify kiln.hosting.publication_authority]
// r[verify kiln.hosting.publication_authority.scenario.validation]
// r[verify kiln.hosting.recovery]
// r[verify kiln.hosting.recovery.scenario.seed_loss]
// r[verify kiln.hosting.runtime_independence]
// r[verify kiln.hosting.runtime_independence.scenario.absent]
// r[verify kiln.hosting.signed_refs]
// r[verify kiln.hosting.signed_refs.scenario.threshold]
// r[verify kiln.hosting.source_only_publication]
// r[verify kiln.hosting.source_only_publication.scenario.accepted]
// r[verify kiln.hosting.source_only_publication.scenario.rejected]
//
// Interchange projections: crates/kiln-core/src/interchange.rs.
// r[verify kiln.interchange.algorithms]
// r[verify kiln.interchange.algorithms.scenario.sha256]
// r[verify kiln.interchange.authority]
// r[verify kiln.interchange.authority.scenario.import_success]
// r[verify kiln.interchange.builder]
// r[verify kiln.interchange.builder.scenario.mantle]
// r[verify kiln.interchange.cdevents]
// r[verify kiln.interchange.cdevents.scenario.unknown]
// r[verify kiln.interchange.correlation]
// r[verify kiln.interchange.correlation.scenario.replay]
// r[verify kiln.interchange.evolution]
// r[verify kiln.interchange.evolution.scenario.missing]
// r[verify kiln.interchange.fixtures]
// r[verify kiln.interchange.fixtures.scenario.missing]
// r[verify kiln.interchange.nonclaims]
// r[verify kiln.interchange.nonclaims.scenario.schema]
// r[verify kiln.interchange.provenance]
// r[verify kiln.interchange.provenance.scenario.attempt_mismatch]
// r[verify kiln.interchange.telemetry]
// r[verify kiln.interchange.telemetry.scenario.loss]
//
// Durable operations and fairness: crates/kiln-core/src/operations.rs and crates/kiln-standalone/src/lib.rs.
// r[verify kiln.operations.configuration]
// r[verify kiln.operations.configuration.scenario.missing]
// r[verify kiln.operations.fairness]
// r[verify kiln.operations.fairness.scenario.shares]
// r[verify kiln.operations.fairness_invariants]
// r[verify kiln.operations.fairness_invariants.scenario.monotonic]
// r[verify kiln.operations.fairness_strategy]
// r[verify kiln.operations.fairness_strategy.scenario.capacity]
// r[verify kiln.operations.fixtures]
// r[verify kiln.operations.fixtures.scenario.interruption]
// r[verify kiln.operations.journal]
// r[verify kiln.operations.journal.scenario.stale]
// r[verify kiln.operations.nonclaims]
// r[verify kiln.operations.nonclaims.scenario.dedup]
// r[verify kiln.operations.outbox]
// r[verify kiln.operations.outbox.scenario.before]
// r[verify kiln.operations.projections]
// r[verify kiln.operations.projections.scenario.rebuild]
// r[verify kiln.operations.recovery]
// r[verify kiln.operations.recovery.scenario.expired]
// r[verify kiln.operations.replay]
// r[verify kiln.operations.replay.scenario.effect]
// r[verify kiln.operations.replay_compatibility]
// r[verify kiln.operations.replay_compatibility.scenario.divergence]
// r[verify kiln.operations.usage]
// r[verify kiln.operations.usage.scenario.unit]
//
// Provider contracts and conformance: crates/kiln-core/src/provider.rs, crates/kiln-provider-api/src/lib.rs, and this crate.
// r[verify kiln.providers.ambiguity]
// r[verify kiln.providers.ambiguity.scenario.lookup]
// r[verify kiln.providers.capabilities]
// r[verify kiln.providers.capabilities.scenario.cancel]
// r[verify kiln.providers.composition]
// r[verify kiln.providers.composition.scenario.mixed]
// r[verify kiln.providers.conformance]
// r[verify kiln.providers.conformance.scenario.status]
// r[verify kiln.providers.core_independence]
// r[verify kiln.providers.core_independence.scenario.import]
// r[verify kiln.providers.effects]
// r[verify kiln.providers.effects.scenario.mismatch]
// r[verify kiln.providers.external_identity]
// r[verify kiln.providers.external_identity.scenario.namespace]
// r[verify kiln.providers.no_fallback]
// r[verify kiln.providers.no_fallback.scenario.unavailable]
// r[verify kiln.providers.standalone]
// r[verify kiln.providers.standalone.scenario.absent]
// r[verify kiln.providers.versioning]
// r[verify kiln.providers.versioning.scenario.unknown]
//
// Radicle CI compatibility: crates/kiln-adapter-radicle/src/lib.rs.
// r[verify kiln.radicle.fixtures]
// r[verify kiln.radicle.fixtures.scenario.stdout]
// r[verify kiln.radicle.lifecycle]
// r[verify kiln.radicle.lifecycle.scenario.triggered]
// r[verify kiln.radicle.native_runner]
// r[verify kiln.radicle.native_runner.scenario.shell]
// r[verify kiln.radicle.nonclaims]
// r[verify kiln.radicle.nonclaims.scenario.scope]
// r[verify kiln.radicle.protocol]
// r[verify kiln.radicle.protocol.scenario.version]
// r[verify kiln.radicle.result]
// r[verify kiln.radicle.result.scenario.unknown]
// r[verify kiln.radicle.status]
// r[verify kiln.radicle.status.scenario.success]
// r[verify kiln.radicle.trigger]
// r[verify kiln.radicle.trigger.scenario.push]
//
// Remote execution: crates/kiln-core/src/remote.rs and crates/kiln-adapter-remote/src/lib.rs.
// r[verify kiln.remote.action]
// r[verify kiln.remote.action.scenario.input_drift]
// r[verify kiln.remote.cache]
// r[verify kiln.remote.cache.scenario.missing_output]
// r[verify kiln.remote.capabilities]
// r[verify kiln.remote.capabilities.scenario.digest]
// r[verify kiln.remote.digests]
// r[verify kiln.remote.digests.scenario.native]
// r[verify kiln.remote.fixtures]
// r[verify kiln.remote.fixtures.scenario.missing]
// r[verify kiln.remote.identities]
// r[verify kiln.remote.identities.scenario.equal_text]
// r[verify kiln.remote.logs]
// r[verify kiln.remote.logs.scenario.truncate]
// r[verify kiln.remote.nonclaims]
// r[verify kiln.remote.nonclaims.scenario.success]
// r[verify kiln.remote.operations]
// r[verify kiln.remote.operations.scenario.wait_failure]
// r[verify kiln.remote.optional]
// r[verify kiln.remote.optional.scenario.absent]
//
// Source and collaboration: crates/kiln-core/src/source.rs and crates/kiln-adapter-jj/src/lib.rs.
// r[verify kiln.source.fixtures]
// r[verify kiln.source.fixtures.scenario.jujutsu_missing]
// r[verify kiln.source.fixtures.scenario.missing]
// r[verify kiln.source.jujutsu]
// r[verify kiln.source.jujutsu.scenario.backend]
// r[verify kiln.source.jujutsu.scenario.resolve]
// r[verify kiln.source.mirrors]
// r[verify kiln.source.mirrors.scenario.disagreement]
// r[verify kiln.source.model]
// r[verify kiln.source.model.scenario.jujutsu]
// r[verify kiln.source.model.scenario.pijul]
// r[verify kiln.source.native_semantics]
// r[verify kiln.source.native_semantics.scenario.history]
// r[verify kiln.source.native_semantics.scenario.jujutsu_ambiguous]
// r[verify kiln.source.observation_split]
// r[verify kiln.source.observation_split.scenario.webhook]
// r[verify kiln.source.port_split]
// r[verify kiln.source.port_split.scenario.jujutsu]
// r[verify kiln.source.port_split.scenario.local_git]
// r[verify kiln.source.proposed_change]
// r[verify kiln.source.proposed_change.scenario.conflict]
// r[verify kiln.source.resolution]
// r[verify kiln.source.resolution.scenario.jujutsu_rewrite]
// r[verify kiln.source.resolution.scenario.moving]
// r[verify kiln.source.tree_identity]
// r[verify kiln.source.tree_identity.scenario.mismatch]
// r[verify kiln.source.trigger_security]
// r[verify kiln.source.trigger_security.scenario.github]

//
// Accepted change anchors. Each block names the implementation or conformance
// surface recorded in that completed change package's verification evidence,
// which was synced and archived into the accepted specifications.

// Agent access declarations and derived review agents:
// crates/kiln-web-service/src/access.rs, crates/kiln-web-service/src/code_review.rs.
// r[impl kiln.access.agent.declaration]
// r[verify kiln.access.agent.declaration.scenario.invalid]
// r[impl kiln.access.agent.derivation]
// r[verify kiln.access.agent.derivation.scenario.merge]
// r[verify kiln.access.agent.derivation.scenario.non_writer]

// Agent attribution on audited admission denials:
// crates/kiln-web-service/src/engine/admission.rs, crates/kiln-web-service/tests/engine.rs.
// r[impl kiln.access.agent.audit]
// r[verify kiln.access.agent.audit.scenario.denial]
// r[verify kiln.access.agent.audit.scenario.human]
// r[verify kiln.access.agent.audit.scenario.surface]

// Attested agent submissions in review-friction observation:
// crates/kiln-observe-core/src/review.rs, crates/kiln-dashboard/src/render.rs.
// r[impl kiln.observe.review.agent]
// r[impl kiln.observe.review.agent.boundary]
// r[verify kiln.observe.review.agent.boundary.scenario.authority]
// r[verify kiln.observe.review.agent.scenario.count]
// r[verify kiln.observe.review.agent.scenario.unattested]

// Declared agent harness aggregation in review friction:
// crates/kiln-observe-core/src/review.rs, crates/kiln-dashboard/src/render.rs.
// r[impl kiln.observe.review.agent.harness]
// r[impl kiln.observe.review.agent.harness.boundary]
// r[verify kiln.observe.review.agent.harness.boundary.scenario.authority]
// r[verify kiln.observe.review.agent.harness.scenario.count]
// r[verify kiln.observe.review.agent.harness.scenario.missing]

// Agent declaration on the admitted session principal:
// crates/kiln-web-service/src/access.rs, crates/kiln-web-service/src/server.rs.
// r[impl kiln.access.agent.session]
// r[verify kiln.access.agent.session.scenario.agent]
// r[verify kiln.access.agent.session.scenario.human]
// r[impl kiln.access.agent.session.surface]
// r[verify kiln.access.agent.session.surface.scenario.label]

// Anchored agent-thread outcomes:
// crates/kiln-core/src/change_review.rs, crates/kiln-web-service/src/code_review/outcome_tests.rs.
// r[impl kiln.review.agent.outcomes]

// Agent vote policy admission and effect:
// crates/kiln-core/src/change_review.rs, crates/kiln-web-service/tests/code_review.rs.
// r[impl kiln.review.agent.profile]
// r[verify kiln.review.agent.profile.scenario.unknown]
// r[impl kiln.review.agent.votes]
// r[verify kiln.review.agent.votes.scenario.advisory]
// r[verify kiln.review.agent.votes.scenario.binding]
// r[verify kiln.review.agent.votes.scenario.veto]

// Attested agent metadata on revisions and review surfaces:
// crates/kiln-core/src/change_review.rs, crates/kiln-web-service/src/code_review/commands.rs.
// r[impl kiln.review.agent.metadata]
// r[verify kiln.review.agent.metadata.scenario.attest]
// r[verify kiln.review.agent.metadata.scenario.conflict]
// r[verify kiln.review.agent.metadata.scenario.invalid]
// r[impl kiln.review.agent.metadata.surface]
// r[verify kiln.review.agent.metadata.surface.scenario.display]

// Advisory agent objections:
// crates/kiln-core/src/change_review.rs, crates/kiln-web-service/tests/code_review.rs.
// r[impl kiln.review.agent.objections]

// Firecracker cache-partition declaration:
// crates/kiln-hosted/src/execution.rs, crates/kiln-hosted/src/microvm.rs.
// r[impl kiln.hosted.microvm.cache_partition]

// Firecracker CPU affinity declaration:
// crates/kiln-hosted/src/execution.rs, crates/kiln-hosted/src/microvm.rs.
// r[impl kiln.hosted.microvm.cpu_affinity]

// Firecracker host preflight facts:
// crates/kiln-microvm/src, crates/kiln-hosted/src/microvm.rs.
// r[impl kiln.hosted.microvm.host_facts]

// Firecracker trust-domain placement:
// crates/kiln-hosted/src/execution.rs, crates/kiln-hosted/src/microvm.rs.
// r[impl kiln.hosted.microvm.trust_domain]

// Host preflight receipt integration:
// crates/kiln-hosted/src/microvm.rs, crates/kiln-hosted/src/execution.rs.
// r[impl kiln.hosted.microvm.host_receipt]

// Change-class ownership policy and classification:
// crates/kiln-core/src/change_review.rs, crates/kiln-core/src/change_review_advisory_tests.rs.
// r[impl kiln.review.class.authority]
// r[verify kiln.review.class.authority.scenario.docs]
// r[verify kiln.review.class.authority.scenario.logic]
// r[impl kiln.review.class.policy]
// r[verify kiln.review.class.policy.scenario.explicit]
// r[verify kiln.review.class.policy.scenario.missing_evidence]
// r[impl kiln.review.class.select]
// r[verify kiln.review.class.select.scenario.rename]

// Typed check evidence:
// crates/kiln-core/src/change_review.rs, crates/kiln-web-service/src/engine/change_review.rs.
// r[impl kiln.review.checks]

// Repository feed and OPML fuzz rails:
// crates/kiln-web-service/fuzz, crates/kiln-highlight/fuzz.
// r[impl kiln.fuzz.feeds]
// r[verify kiln.fuzz.feeds.smoke]
// r[impl kiln.fuzz.highlight]
// r[verify kiln.fuzz.highlight.smoke]
// r[impl kiln.fuzz.nonclaims]

// Patchgate atomic run receipt:
// crates/kiln-adapter-patchgate/src/bin/kiln-patchgate-review.rs.
// r[impl kiln.patchgate.receipt.publication]

// Patchgate worker job:
// config/patchgate-review-job.ncl, scripts/patchgate-review-job.py.
// r[impl kiln.patchgate.worker.job]

// Radicle broker patch identity:
// crates/kiln-adapter-radicle/src/defelo.rs.
// r[impl kiln.radicle.broker_031_wire.patch]

// Lattice review workflow node:
// crates/kiln-adapter-lattice/src/profile.rs, crates/kiln-adapter-github/src/profile.rs.
// r[impl kiln.lattice.review_node]

// Structural corpus line baseline:
// crates/kiln-highlight/src/structural_tests.rs, crates/kiln-web-service/src/code_review/structural.rs.
// r[impl kiln.review.structural.corpus]

// Tag-bound triggers:
// crates/kiln-core/src/trigger.rs, crates/kiln-web-service/src/engine/triggers.rs.
// r[impl kiln.triggers.tag]
// r[impl kiln.triggers.tag.boundary]
// r[verify kiln.triggers.tag.boundary.scenario.bounded]
// r[impl kiln.triggers.tag.filter]
// r[verify kiln.triggers.tag.filter.scenario.invalid]
// r[verify kiln.triggers.tag.scenario.dispatch]

// VMM backend seam:
// crates/kiln-microvm/src/backend.rs, crates/kiln-hosted/src/microvm.rs.
// r[impl kiln.microvm.backend.seam]

// Developer-loop worker data mount roots and read-only mounts:
// crates/kiln-web-service/src/execution.rs, config/web-worker.ncl.
// r[impl kiln.web.developer_loop.data_mount_roots]
// r[impl kiln.web.developer_loop.data_mounts]

// Admitted journal proof:
// crates/kiln-dashboard/src/journal.rs, crates/kiln-dashboard/src/report.rs.
// r[impl kiln.observe.dashboard.proof]
// r[verify kiln.observe.dashboard.proof.scenario.construct]
// r[verify kiln.observe.dashboard.proof.scenario.read]

// Admitted Aspen profile proof:
// crates/kiln-adapter-aspen/src/profile/mod.rs, crates/kiln-adapter-aspen/src/tests/contracts.rs.
// r[impl kiln.aspen.proof]
// r[verify kiln.aspen.proof.scenario.construct]
// r[verify kiln.aspen.proof.scenario.read]

// Admitted GitHub profile proof:
// crates/kiln-adapter-github/src/profile.rs, crates/kiln-adapter-github/src/export.rs.
// r[impl kiln.github.profile.proof]
// r[verify kiln.github.profile.proof.scenario.construct]
// r[verify kiln.github.profile.proof.scenario.read]

// Admitted Radicle configuration proof:
// crates/kiln-adapter-radicle/src/source/shell.rs, crates/kiln-adapter-radicle/src/profile.rs.
// r[impl kiln.radicle.proof]
// r[verify kiln.radicle.proof.scenario.construct]
// r[verify kiln.radicle.proof.scenario.read]

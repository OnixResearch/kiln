use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::*;
use crate::source::{
    JujutsuBackend, JujutsuFacts, PijulFacts, SourceCapability, SourceNativeFacts,
    SourceProviderProfile, SourceResolution, compute_pijul_observation_identity,
};
use crate::types::{Alias, ContentId, Diagnostic, EvaluationId, ExternalRef, PolicyId};
use crate::workflow::WorkflowBounds;

const PROFILE: &str = "pijul-source-v1";
const SUPPLIER: &str = "fixture-proposal-supplier";
const CHANGE_BOUND: usize = 64;
const WORKFLOW_LIMIT: usize = 8;
const WORKFLOW_LOG_BYTES: usize = 4_096;
const MEMORY_BYTES: u64 = 1 << 30;
const TIMEOUT_MS: u64 = 60_000;
const OUTPUT_BYTES: u64 = 1 << 20;
const MAX_PRODUCTS: usize = 4;
const RETRY_ATTEMPTS: u32 = 2;

fn ok<T, E>(value: Result<T, E>) -> T {
    value.unwrap_or_else(|_| unreachable!())
}

fn id(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().as_str(),
    ))
}

fn pijul(kind: &str, value: &str) -> ExternalRef {
    ok(ExternalRef::new("pijul", kind, value))
}

fn alias(value: &str) -> Alias {
    ok(Alias::new(value))
}

fn profile_id(value: &str) -> ProviderProfileId {
    ok(ProviderProfileId::new(value))
}

fn acquisition_profile() -> SourceProviderProfile {
    SourceProviderProfile {
        id: profile_id(PROFILE),
        native_system: ok(ExternalRef::new("pijul", "source-provider", "fixture")),
        capabilities: BTreeSet::from([
            SourceCapability::Discover,
            SourceCapability::ResolveExact,
            SourceCapability::Acquire,
            SourceCapability::ObserveIdentity,
        ]),
        identity_namespaces: BTreeSet::from([alias("pijul-state"), alias("pijul-change")]),
        immutable_resolution: true,
    }
}

fn resolution(channel: &str, state: &str, changes: &[&str], tree: &str) -> SourceResolution {
    let mut facts = PijulFacts {
        provider_profile: profile_id(PROFILE),
        repository: pijul("repository", "fixture"),
        channel: pijul("channel", channel),
        state: pijul("state", state),
        observed_state_before: pijul("state", state),
        observed_state_after: pijul("state", state),
        change_sequence: changes
            .iter()
            .map(|change| pijul("change", change))
            .collect(),
        change_bound: CHANGE_BOUND,
        format_cohort: pijul("format-cohort", "sanakirja-pristine-v1"),
        observation_identity: id("pending"),
    };
    facts.observation_identity = ok(compute_pijul_observation_identity(&facts));
    SourceResolution {
        source_provider: profile_id(PROFILE),
        repository: pijul("repository", "fixture"),
        symbolic_reference: pijul("channel", channel),
        exact_revision: pijul("state", state),
        canonical_tree: id(tree),
        native_facts: SourceNativeFacts::Pijul(facts),
    }
}

fn role(role: MergeRole, resolution: SourceResolution) -> RoleSource {
    RoleSource {
        role,
        resolution,
        acquisition_profile: acquisition_profile(),
    }
}

fn checker(label: &str) -> CheckerIdentity {
    CheckerIdentity {
        producer_profile: profile_id("fixture-semantic-checker-v1"),
        checker: id(label),
        toolchain: id("fixture-toolchain"),
        semantic_profile: alias("fixture-rust-merge-profile"),
        receipt_profile: alias("fixture-receipt-v1"),
    }
}

fn bounds() -> CheckBounds {
    CheckBounds {
        timeout_ms: TIMEOUT_MS,
        max_memory_bytes: MEMORY_BYTES,
        stdout_max_bytes: OUTPUT_BYTES,
        stderr_max_bytes: OUTPUT_BYTES,
        max_products: MAX_PRODUCTS,
    }
}

fn policy(mode: CheckMode) -> SemanticMergePolicy {
    let mut admitted_checkers = vec![checker("fixture-checker"), checker("other-checker")];
    admitted_checkers.sort();
    SemanticMergePolicy {
        id: ok(PolicyId::new("semantic-merge-fixture")),
        mode,
        lineage: LineagePolicy::PijulChangeContainment {
            require_greatest_common_base: true,
            max_candidate_extra_changes: 0,
        },
        admitted_checkers,
        admitted_source_profiles: vec![profile_id(PROFILE)],
        admitted_candidate_suppliers: vec![profile_id(SUPPLIER)],
        retry: RetryPolicy {
            max_attempts: RETRY_ATTEMPTS,
            retry_tool_error: true,
            retry_exhausted: false,
        },
        bound_ceilings: bounds(),
    }
}

fn request_with(mode: CheckMode) -> SemanticMergeRequest {
    SemanticMergeRequest {
        contract_version: SEMANTIC_MERGE_CONTRACT_VERSION,
        evaluation: ok(EvaluationId::new("evaluation-1")),
        base: role(
            MergeRole::Base,
            resolution("main", "state-o", &["c1"], "tree-o"),
        ),
        variant_a: role(
            MergeRole::VariantA,
            resolution("feature-a", "state-a", &["c1", "ca"], "tree-a"),
        ),
        variant_b: role(
            MergeRole::VariantB,
            resolution("feature-b", "state-b", &["c1", "cb"], "tree-b"),
        ),
        candidate: role(
            MergeRole::Candidate,
            resolution("merge", "state-m", &["c1", "ca", "cb"], "tree-m"),
        ),
        candidate_supply: CandidateSupply {
            supplier: profile_id(SUPPLIER),
            proposal: ok(ExternalRef::new("fixture", "proposal", "proposal-1")),
            supplied_state: pijul("state", "state-m"),
        },
        subject: SemanticSubject {
            language: alias("rust"),
            unit: ok(ExternalRef::new("fixture", "crate", "fixture-crate")),
            required_coverage: vec![alias("crate::add"), alias("crate::sub")],
        },
        property: SemanticProperty {
            id: alias("semantic-conflict-freedom"),
            revision: id("property-v1"),
            assumptions: {
                let mut assumptions = vec![id("assume-total"), id("assume-pure")];
                assumptions.sort();
                assumptions
            },
        },
        inputs: SemanticInputs {
            dependencies: id("deps-v1"),
            model: id("model-v1"),
        },
        checker: checker("fixture-checker"),
        policy: policy(mode),
        bounds: bounds(),
    }
}

fn request() -> SemanticMergeRequest {
    request_with(CheckMode::Required)
}

fn admitted() -> AdmittedRequest {
    ok(admit_request(request()))
}

fn expect(result: CoreResult<impl core::fmt::Debug>, expected_code: &str, expected_field: &str) {
    let (actual_code, actual_field) = match result {
        Ok(value) => (
            alloc::format!("unexpected success: {value:?}"),
            String::new(),
        ),
        Err(Diagnostic { code, field, .. }) => (code, field),
    };
    assert_eq!(
        (actual_code.as_str(), actual_field.as_str()),
        (expected_code, expected_field)
    );
}

fn apply(state: &SemanticCheckState, decision: &SemanticCheckDecision) -> SemanticCheckState {
    decision
        .events
        .iter()
        .fold(state.clone(), |current, event| ok(evolve(&current, event)))
}

fn run(state: &SemanticCheckState, command: SemanticCheckCommand) -> SemanticCheckState {
    apply(state, &ok(decide(state, &command)))
}

fn dispatched(request: AdmittedRequest) -> (SemanticCheckState, CheckerRequest) {
    let state = SemanticCheckState::new(request);
    let decision = ok(decide(&state, &SemanticCheckCommand::Dispatch));
    let Some(SemanticCheckEffect::DispatchChecker(checker_request)) = decision.effects.first()
    else {
        unreachable!()
    };
    let checker_request = checker_request.as_ref().clone();
    (apply(&state, &decision), checker_request)
}

fn artifact(
    checker_request: &CheckerRequest,
    kind: ArtifactKind,
    name: &str,
    content: &str,
) -> BoundArtifact {
    let content = id(content);
    BoundArtifact {
        kind,
        name: alias(name),
        binding: ok(artifact_binding(
            &checker_request.request.binding().identity,
            &checker_request.attempt.identity,
            kind,
            name,
            &content,
        )),
        content,
    }
}

fn observation(checker_request: &CheckerRequest, verdict: CheckerVerdict) -> CheckerObservation {
    CheckerObservation {
        attempt: checker_request.attempt.identity.clone(),
        binding: checker_request.request.binding().clone(),
        checker: checker_request.request.request().checker.clone(),
        evidence_kind: EvidenceKind::SemanticCheckerReceipt,
        verdict,
        coverage: checker_request
            .request
            .request()
            .subject
            .required_coverage
            .clone(),
        output_complete: true,
        receipt: Some(id("receipt")),
        artifacts: vec![
            artifact(checker_request, ArtifactKind::Log, "checker-log", "log"),
            artifact(
                checker_request,
                ArtifactKind::Product,
                "proof-summary",
                "summary",
            ),
        ],
    }
}

fn observe(state: &SemanticCheckState, observation: CheckerObservation) -> SemanticCheckState {
    run(
        state,
        SemanticCheckCommand::ObserveResult(Box::new(observation)),
    )
}

fn completed(verdict: CheckerVerdict) -> SemanticCheckState {
    let (state, checker_request) = dispatched(admitted());
    observe(&state, observation(&checker_request, verdict))
}

fn gate(state: &SemanticCheckState) -> SemanticGateVerdict {
    ok(decide_gate(state, state.request().binding()))
}

// r[verify kiln.semantic_merge.request.scenario.frozen]
// r[verify kiln.semantic_merge.lineage.scenario.contained]
#[test]
fn frozen_quartet_admits_with_stable_role_tagged_binding() {
    let first = admitted();
    let second = admitted();
    assert_eq!(first.binding(), second.binding());
    let roles: BTreeSet<&ContentId> = MergeRole::ALL
        .into_iter()
        .map(|role| first.binding().component(BindingComponent::for_role(role)))
        .collect();
    assert_eq!(roles.len(), MergeRole::ALL.len());
    let SourceNativeFacts::Pijul(facts) = &first.request().variant_b.resolution.native_facts else {
        unreachable!()
    };
    assert_eq!(facts.channel, pijul("channel", "feature-b"));
    assert_eq!(
        facts.change_sequence,
        vec![pijul("change", "c1"), pijul("change", "cb")]
    );

    // Equal trees under different roles stay distinct role-tagged identities.
    let mut same_trees = request();
    same_trees.candidate.resolution.canonical_tree = id("tree-o");
    let same_trees = ok(admit_request(same_trees));
    assert_ne!(same_trees.binding().base, same_trees.binding().candidate);
}

// r[verify kiln.semantic_merge.request.scenario.swapped]
#[test]
fn swapped_roles_fail_on_the_variant_binding() {
    let mut swapped = request();
    core::mem::swap(
        &mut swapped.variant_a.resolution,
        &mut swapped.variant_b.resolution,
    );
    let swapped = ok(admit_request(swapped));
    assert_eq!(
        admitted().binding().first_mismatch(swapped.binding()),
        Some(BindingComponent::VariantA)
    );

    let (state, checker_request) = dispatched(admitted());
    let mut result = observation(&checker_request, CheckerVerdict::Verified);
    result.binding = swapped.binding().clone();
    expect(
        decide(
            &state,
            &SemanticCheckCommand::ObserveResult(Box::new(result)),
        ),
        "semantic_binding_mismatch",
        "semantic_merge.binding.variant_a",
    );

    let mut mislabeled = request();
    mislabeled.variant_a.role = MergeRole::VariantB;
    expect(
        admit_request(mislabeled),
        "semantic_role_slot",
        "semantic_merge.variant_a.role",
    );
}

// r[verify kiln.semantic_merge.request.scenario.malformed]
#[test]
fn malformed_requests_fail_before_effects() {
    let mut version = request();
    version.contract_version = SEMANTIC_MERGE_CONTRACT_VERSION + 1;
    expect(
        admit_request(version),
        "semantic_contract_version",
        "semantic_merge.contract_version",
    );

    let mut empty = request();
    empty.subject.required_coverage.clear();
    expect(
        admit_request(empty),
        "semantic_subject_coverage",
        "semantic_merge.subject.required_coverage",
    );

    let mut unsorted = request();
    unsorted.subject.required_coverage.reverse();
    expect(
        admit_request(unsorted),
        "semantic_subject_coverage",
        "semantic_merge.subject.required_coverage",
    );

    let mut assumptions = request();
    assumptions.property.assumptions.reverse();
    expect(
        admit_request(assumptions),
        "semantic_property_assumptions",
        "semantic_merge.property.assumptions",
    );

    let mut zero = request();
    zero.bounds.timeout_ms = 0;
    expect(
        admit_request(zero),
        "semantic_bounds",
        "semantic_merge.bounds.timeout_ms",
    );

    let mut above = request();
    above.bounds.max_products = MAX_PRODUCTS + 1;
    expect(
        admit_request(above),
        "semantic_bounds",
        "semantic_merge.bounds.max_products",
    );

    let mut unadmitted = request();
    unadmitted.checker = checker("unreviewed-checker");
    expect(
        admit_request(unadmitted),
        "semantic_checker_unadmitted",
        "semantic_merge.checker",
    );
}

// r[verify kiln.semantic_merge.lineage.scenario.refused]
#[test]
fn ambiguous_or_unavailable_lineage_is_refused() {
    let mut base_outside = request();
    base_outside.base.resolution = resolution("main", "state-o", &["c0"], "tree-o");
    expect(
        admit_request(base_outside),
        "semantic_lineage_base",
        "semantic_merge.variant_a.native_facts.change_sequence",
    );

    let mut ambiguous = request();
    ambiguous.variant_a.resolution =
        resolution("feature-a", "state-a", &["c1", "ca", "shared"], "tree-a");
    ambiguous.variant_b.resolution =
        resolution("feature-b", "state-b", &["c1", "cb", "shared"], "tree-b");
    ambiguous.candidate.resolution =
        resolution("merge", "state-m", &["c1", "ca", "cb", "shared"], "tree-m");
    expect(
        admit_request(ambiguous),
        "semantic_lineage_ambiguous",
        "semantic_merge.base.native_facts.change_sequence",
    );

    let mut missing = request();
    missing.candidate.resolution = resolution("merge", "state-m", &["c1", "ca"], "tree-m");
    expect(
        admit_request(missing),
        "semantic_lineage_candidate",
        "semantic_merge.candidate.native_facts.change_sequence",
    );

    let mut extra = request();
    extra.candidate.resolution =
        resolution("merge", "state-m", &["c1", "ca", "cb", "fix"], "tree-m");
    expect(
        admit_request(extra),
        "semantic_lineage_candidate_extra",
        "semantic_merge.candidate.native_facts.change_sequence",
    );

    let mut unavailable = request();
    let exact = ok(ExternalRef::new("jj", "commit", "commit-b"));
    unavailable.variant_b.resolution = SourceResolution {
        source_provider: profile_id(PROFILE),
        repository: ok(ExternalRef::new("jj", "repository", "fixture")),
        symbolic_reference: ok(ExternalRef::new("jj", "bookmark", "feature-b")),
        exact_revision: exact.clone(),
        canonical_tree: id("tree-b"),
        native_facts: SourceNativeFacts::Jujutsu(JujutsuFacts {
            backend: JujutsuBackend::Git,
            operation_identity: ok(ExternalRef::new("jj", "operation", "op")),
            change_id: ok(ExternalRef::new("jj", "change", "change-b")),
            bookmark: None,
            resolved_commits: vec![exact],
            conflicted_bookmark: false,
            divergent_change: false,
        }),
    };
    expect(
        admit_request(unavailable),
        "semantic_lineage_unavailable",
        "semantic_merge.variant_b.native_facts",
    );

    // One change identity cannot stand for a whole channel state.
    let mut single_change = request();
    single_change.candidate.resolution.exact_revision = pijul("change", "cb");
    expect(
        admit_request(single_change),
        "pijul_resolution_facts",
        "source_resolution.native_facts",
    );
}

// r[verify kiln.semantic_merge.candidate.scenario.unauthorized]
#[test]
fn candidate_must_come_from_authorized_read_only_acquisition() {
    let mut supplier = request();
    supplier.candidate_supply.supplier = profile_id("unknown-supplier");
    expect(
        admit_request(supplier),
        "semantic_candidate_supplier",
        "semantic_merge.candidate_supply.supplier",
    );

    let mut supplied = request();
    supplied.candidate_supply.supplied_state = pijul("state", "state-other");
    expect(
        admit_request(supplied),
        "semantic_candidate_supplied_state",
        "semantic_merge.candidate_supply.supplied_state",
    );

    for forbidden in [
        SourceCapability::Mutate,
        SourceCapability::Collaborate,
        SourceCapability::ReceiveTrigger,
        SourceCapability::PublishStatus,
        SourceCapability::UseCredentials,
    ] {
        let mut authority = request();
        authority
            .candidate
            .acquisition_profile
            .capabilities
            .insert(forbidden);
        expect(
            admit_request(authority),
            "semantic_acquisition_authority",
            "semantic_merge.candidate.acquisition_profile.capabilities",
        );
    }

    let mut moving = request();
    moving.base.acquisition_profile.immutable_resolution = false;
    expect(
        admit_request(moving),
        "semantic_acquisition_capability",
        "semantic_merge.base.acquisition_profile.capabilities",
    );

    let mut unlisted = request();
    unlisted.policy.admitted_source_profiles = vec![profile_id("git-source-v1")];
    expect(
        admit_request(unlisted),
        "semantic_acquisition_profile",
        "semantic_merge.base.acquisition_profile",
    );
}

// r[verify kiln.semantic_merge.dispatch]
#[test]
fn checker_attempt_maps_onto_a_valid_lattice_workflow_plan() {
    let (_, checker_request) = dispatched(admitted());
    let dispatch = CheckerDispatchIdentity {
        effect_id: ok(EffectId::new("effect-1")),
        attempt_id: ok(AttemptId::new("attempt-1")),
        candidate_id: ok(CandidateId::new("candidate-1")),
        pipeline_revision_id: ok(PipelineRevisionId::new("pipeline-1")),
        provider_profile: profile_id("lattice-workflow-v1"),
        platform: alias("x86_64-linux"),
        bounds: WorkflowBounds {
            max_nodes: WORKFLOW_LIMIT,
            max_edges: WORKFLOW_LIMIT,
            max_log_references: WORKFLOW_LIMIT,
            max_log_read_bytes: WORKFLOW_LOG_BYTES,
            max_total_log_bytes: WORKFLOW_LOG_BYTES,
        },
    };
    let plan = ok(checker_workflow_plan(&checker_request, &dispatch));
    assert_eq!(plan.idempotency_key, checker_request.attempt.identity);
    assert_eq!(
        plan.nodes.first().map(|node| &node.operation_identity),
        Some(&checker_request.request.binding().identity)
    );

    let mut foreign = checker_request;
    foreign.attempt = ok(attempt_binding(&id("other-request"), 1));
    expect(
        checker_workflow_plan(&foreign, &dispatch),
        "semantic_attempt_binding",
        "semantic_merge.attempt.request",
    );
}

// r[verify kiln.semantic_merge.outcomes.scenario.nonverified]
#[test]
fn only_fresh_verified_satisfies_a_required_gate() {
    let verified = gate(&completed(CheckerVerdict::Verified));
    assert!(verified.satisfies_required && !verified.blocks);
    assert_eq!(verified.outcome, Some(SemanticOutcome::Verified));
    assert!(!verified.revalidation.grants_mutation_authority());

    for (verdict, outcome) in [
        (
            CheckerVerdict::Counterexample,
            SemanticOutcome::Counterexample,
        ),
        (CheckerVerdict::Unknown, SemanticOutcome::Unknown),
        (CheckerVerdict::Unsupported, SemanticOutcome::Unsupported),
        (
            CheckerVerdict::Exhausted(ExhaustionBound::Memory),
            SemanticOutcome::Exhausted(ExhaustionBound::Memory),
        ),
        (CheckerVerdict::Cancelled, SemanticOutcome::Cancelled),
        (CheckerVerdict::ToolError, SemanticOutcome::ToolError),
    ] {
        let verdict = gate(&completed(verdict));
        assert_eq!(verdict.outcome, Some(outcome));
        assert!(!verdict.satisfies_required && verdict.blocks);
        assert_eq!(
            verdict.conflict_demonstrated,
            outcome == SemanticOutcome::Counterexample
        );
    }

    let pending = gate(&dispatched(admitted()).0);
    assert_eq!(pending.outcome, None);
    assert!(pending.blocks);

    let (state, checker_request) = dispatched(admitted());
    let mut moved = request();
    moved.base.resolution = resolution("main", "state-o2", &["c1"], "tree-o");
    let fresh = ok(reobserve_binding(&moved));
    let stale = observe(
        &state,
        observation(&checker_request, CheckerVerdict::Verified),
    );
    let stale = ok(decide_gate(&stale, &fresh));
    assert_eq!(
        stale.outcome,
        Some(SemanticOutcome::Stale(BindingComponent::Base))
    );
    assert!(!stale.satisfies_required && stale.blocks && !stale.conflict_demonstrated);
}

// r[verify kiln.semantic_merge.outcomes.scenario.advisory]
#[test]
fn advisory_mode_reports_without_blocking_or_satisfying() {
    for verdict in [
        CheckerVerdict::Verified,
        CheckerVerdict::Counterexample,
        CheckerVerdict::Unknown,
    ] {
        let (state, checker_request) =
            dispatched(ok(admit_request(request_with(CheckMode::Advisory))));
        let state = observe(&state, observation(&checker_request, verdict));
        let verdict = gate(&state);
        assert_eq!(verdict.mode, CheckMode::Advisory);
        assert!(verdict.outcome.is_some());
        assert!(!verdict.satisfies_required && !verdict.blocks);
        let combined = ok(combine_gates(&verdict, &[]));
        assert!(combined.passed && !combined.semantic_blocking);
    }
}

fn changed_requests() -> Vec<(BindingComponent, SemanticMergeRequest)> {
    let mut base = request();
    base.base.resolution = resolution("main", "state-o2", &["c1"], "tree-o");
    let mut candidate = request();
    candidate.candidate.resolution = resolution("merge", "state-m2", &["cb", "ca", "c1"], "tree-m");
    candidate.candidate_supply.supplied_state = pijul("state", "state-m2");
    let mut dependencies = request();
    dependencies.inputs.dependencies = id("deps-v2");
    let mut policy = request();
    policy.policy.retry.max_attempts = 1;
    let mut checker_change = request();
    checker_change.checker = checker("other-checker");
    let mut property = request();
    property.property.revision = id("property-v2");
    vec![
        (BindingComponent::Base, base),
        (BindingComponent::Candidate, candidate),
        (BindingComponent::Dependencies, dependencies),
        (BindingComponent::Policy, policy),
        (BindingComponent::Checker, checker_change),
        (BindingComponent::Property, property),
    ]
}

// r[verify kiln.semantic_merge.binding.scenario.changed]
#[test]
fn results_for_changed_inputs_name_the_failed_binding() {
    let (state, checker_request) = dispatched(admitted());
    for (component, changed) in changed_requests() {
        let changed = ok(admit_request(changed));
        let mut result = observation(&checker_request, CheckerVerdict::Verified);
        result.binding = changed.binding().clone();
        let field = alloc::format!("semantic_merge.binding.{}", component.name());
        expect(
            decide(
                &state,
                &SemanticCheckCommand::ObserveResult(Box::new(result)),
            ),
            "semantic_binding_mismatch",
            &field,
        );
    }

    let mut forged = observation(&checker_request, CheckerVerdict::Verified);
    forged.binding.identity = id("forged");
    expect(
        decide(
            &state,
            &SemanticCheckCommand::ObserveResult(Box::new(forged)),
        ),
        "semantic_binding_identity",
        "semantic_merge.binding.identity",
    );

    let mut substituted = observation(&checker_request, CheckerVerdict::Verified);
    substituted.checker = checker("other-checker");
    expect(
        decide(
            &state,
            &SemanticCheckCommand::ObserveResult(Box::new(substituted)),
        ),
        "semantic_checker_substitution",
        "semantic_merge.binding.checker",
    );
}

// r[verify kiln.semantic_merge.binding.scenario.coverage]
#[test]
fn coverage_partial_output_and_artifacts_are_bound() {
    let (state, checker_request) = dispatched(admitted());
    let command = |observation: CheckerObservation| {
        decide(
            &state,
            &SemanticCheckCommand::ObserveResult(Box::new(observation)),
        )
    };

    let mut missing = observation(&checker_request, CheckerVerdict::Verified);
    missing.coverage.pop();
    expect(
        command(missing),
        "semantic_coverage_missing",
        "semantic_merge.result.coverage",
    );

    let mut outside = observation(&checker_request, CheckerVerdict::Unknown);
    outside.coverage.push(alias("crate::zzz"));
    expect(
        command(outside),
        "semantic_coverage_scope",
        "semantic_merge.result.coverage",
    );

    let mut foreign_product = observation(&checker_request, CheckerVerdict::Verified);
    let mut other_attempt = checker_request.clone();
    other_attempt.attempt = ok(attempt_binding(
        &checker_request.request.binding().identity,
        2,
    ));
    foreign_product.artifacts = vec![artifact(
        &other_attempt,
        ArtifactKind::Product,
        "proof-summary",
        "summary",
    )];
    expect(
        command(foreign_product),
        "semantic_artifact_binding",
        "semantic_merge.result.artifacts",
    );

    let mut too_many = observation(&checker_request, CheckerVerdict::Verified);
    too_many.artifacts = (0..=MAX_PRODUCTS)
        .map(|index| {
            artifact(
                &checker_request,
                ArtifactKind::Product,
                &alloc::format!("product-{index}"),
                "bytes",
            )
        })
        .collect();
    expect(
        command(too_many),
        "semantic_product_bound",
        "semantic_merge.result.artifacts",
    );

    let mut no_receipt = observation(&checker_request, CheckerVerdict::Verified);
    no_receipt.receipt = None;
    expect(
        command(no_receipt),
        "semantic_receipt_missing",
        "semantic_merge.result.receipt",
    );

    let mut partial = observation(&checker_request, CheckerVerdict::Verified);
    partial.output_complete = false;
    let partial = observe(&state, partial);
    let verdict = gate(&partial);
    assert_eq!(verdict.outcome, Some(SemanticOutcome::ToolError));
    assert!(verdict.blocks);
    assert_eq!(
        verdict.diagnostic.map(|diagnostic| diagnostic.code),
        Some(String::from("semantic_partial_output"))
    );
}

// r[verify kiln.semantic_merge.binding.scenario.cache]
#[test]
fn cache_reuse_and_retry_follow_the_exact_binding() {
    let verified = completed(CheckerVerdict::Verified);
    let entry = cache_entry(&verified);
    assert!(entry.is_some());
    let entry = entry.unwrap_or_else(|| unreachable!());
    let fresh = SemanticCheckState::new(admitted());
    let reused = run(
        &fresh,
        SemanticCheckCommand::UseCache(Box::new(entry.clone())),
    );
    assert!(gate(&reused).satisfies_required);

    let changed = ok(admit_request(
        changed_requests()
            .into_iter()
            .find(|(component, _)| *component == BindingComponent::Dependencies)
            .map(|(_, request)| request)
            .unwrap_or_else(|| unreachable!()),
    ));
    expect(
        admit_cache_entry(&changed, &entry),
        "semantic_cache_mismatch",
        "semantic_merge.binding.dependencies",
    );

    assert_eq!(cache_entry(&completed(CheckerVerdict::Unknown)), None);
    let mut unknown = entry;
    unknown.result.outcome = SemanticOutcome::Unknown;
    expect(
        admit_cache_entry(&admitted(), &unknown),
        "semantic_cache_nondeterministic",
        "semantic_merge.cache.outcome",
    );

    let first = completed(CheckerVerdict::ToolError);
    let retry = ok(decide(&first, &SemanticCheckCommand::Retry));
    let Some(SemanticCheckEvent::AttemptStarted(second)) = retry.events.first() else {
        unreachable!()
    };
    assert_eq!(second.ordinal, 2);
    assert_ne!(
        Some(&second.identity),
        first
            .attempts()
            .first()
            .map(|record| &record.attempt.identity)
    );
    let retried = apply(&first, &retry);
    let second_request = CheckerRequest {
        request: admitted(),
        attempt: second.clone(),
    };
    let exhausted = observe(
        &retried,
        observation(&second_request, CheckerVerdict::ToolError),
    );
    expect(
        decide(&exhausted, &SemanticCheckCommand::Retry),
        "semantic_retry_refused",
        "semantic_merge.policy.retry",
    );

    let retry_policy = request().policy.retry;
    for (outcome, used, decision) in [
        (SemanticOutcome::ToolError, 1, RetryDecision::Retry),
        (SemanticOutcome::ToolError, 2, RetryDecision::Terminal),
        (
            SemanticOutcome::Exhausted(ExhaustionBound::WallTime),
            1,
            RetryDecision::Terminal,
        ),
        (SemanticOutcome::Unknown, 1, RetryDecision::Terminal),
        (SemanticOutcome::Counterexample, 1, RetryDecision::Terminal),
        (SemanticOutcome::Cancelled, 1, RetryDecision::Terminal),
    ] {
        assert_eq!(classify_retry(&retry_policy, outcome, used), decision);
    }
}

// r[verify kiln.semantic_merge.freshness.scenario.late]
#[test]
fn late_replayed_and_cancelled_results_are_rejected() {
    let (state, checker_request) = dispatched(admitted());
    let cancel = ok(decide(&state, &SemanticCheckCommand::Cancel));
    assert_eq!(
        cancel.effects,
        vec![SemanticCheckEffect::CancelChecker(
            checker_request.attempt.identity.clone()
        )]
    );
    let cancelled = apply(&state, &cancel);
    expect(
        decide(
            &cancelled,
            &SemanticCheckCommand::ObserveResult(Box::new(observation(
                &checker_request,
                CheckerVerdict::Verified,
            ))),
        ),
        "semantic_result_late",
        "semantic_merge.attempt",
    );
    let verdict = gate(&cancelled);
    assert_eq!(verdict.outcome, Some(SemanticOutcome::Cancelled));
    assert!(verdict.blocks);

    let first = observe(
        &state,
        observation(&checker_request, CheckerVerdict::ToolError),
    );
    let retried = run(&first, SemanticCheckCommand::Retry);
    expect(
        decide(
            &retried,
            &SemanticCheckCommand::ObserveResult(Box::new(observation(
                &checker_request,
                CheckerVerdict::Verified,
            ))),
        ),
        "semantic_result_late",
        "semantic_merge.attempt",
    );
    assert_eq!(retried.result(), None);

    let mut replay = observation(&checker_request, CheckerVerdict::Verified);
    replay.attempt = ok(attempt_binding(&id("another-evaluation"), 1)).identity;
    expect(
        decide(
            &state,
            &SemanticCheckCommand::ObserveResult(Box::new(replay)),
        ),
        "semantic_attempt_replay",
        "semantic_merge.attempt",
    );
}

// r[verify kiln.semantic_merge.freshness.scenario.concurrent]
#[test]
fn concurrent_source_change_marks_the_evaluation_stale() {
    let (state, checker_request) = dispatched(admitted());
    let mut moved = request();
    moved.variant_b.resolution = resolution("feature-b", "state-b2", &["cb", "c1"], "tree-b");
    let fresh = ok(reobserve_binding(&moved));
    let decision = ok(decide(
        &state,
        &SemanticCheckCommand::ObserveInputs(Box::new(fresh.clone())),
    ));
    assert_eq!(
        decision.effects,
        vec![SemanticCheckEffect::CancelChecker(
            checker_request.attempt.identity.clone()
        )]
    );
    let stale = apply(&state, &decision);
    assert_eq!(stale.stale(), Some(BindingComponent::VariantB));
    expect(
        decide(
            &stale,
            &SemanticCheckCommand::ObserveResult(Box::new(observation(
                &checker_request,
                CheckerVerdict::Verified,
            ))),
        ),
        "semantic_result_stale",
        "semantic_merge.binding.variant_b",
    );
    expect(
        decide(&stale, &SemanticCheckCommand::Dispatch),
        "semantic_evaluation_stale",
        "semantic_merge.binding.variant_b",
    );
    let verdict = ok(decide_gate(&stale, &fresh));
    assert_eq!(
        verdict.outcome,
        Some(SemanticOutcome::Stale(BindingComponent::VariantB))
    );
    assert!(verdict.blocks);

    let unchanged = ok(decide(
        &state,
        &SemanticCheckCommand::ObserveInputs(Box::new(state.request().binding().clone())),
    ));
    assert!(unchanged.events.is_empty() && unchanged.effects.is_empty());
}

// r[verify kiln.semantic_merge.independence.scenario.substitution]
#[test]
fn semantic_and_independent_gates_never_substitute() {
    let others = |passed: bool| {
        [
            ("build", IndependentGateKind::Build),
            ("tests", IndependentGateKind::Test),
            ("lints", IndependentGateKind::Lint),
            ("review", IndependentGateKind::Review),
        ]
        .map(|(name, kind)| IndependentGate {
            name: alias(name),
            kind,
            required: true,
            passed: passed || kind != IndependentGateKind::Build,
        })
    };
    let unknown = gate(&completed(CheckerVerdict::Unknown));
    let combined = ok(combine_gates(&unknown, &others(true)));
    assert!(!combined.passed && combined.semantic_blocking);

    let verified = gate(&completed(CheckerVerdict::Verified));
    let combined = ok(combine_gates(&verified, &others(false)));
    assert!(!combined.passed && !combined.semantic_blocking);
    assert_eq!(combined.failing_gates, vec![alias("build")]);
    assert!(ok(combine_gates(&verified, &others(true))).passed);

    let (state, checker_request) = dispatched(admitted());
    for kind in [
        EvidenceKind::CleanTextMerge,
        EvidenceKind::EqualSourceTrees,
        EvidenceKind::PassingTests,
        EvidenceKind::EvidenceEnvelope,
    ] {
        let mut substitute = observation(&checker_request, CheckerVerdict::Verified);
        substitute.evidence_kind = kind;
        expect(
            decide(
                &state,
                &SemanticCheckCommand::ObserveResult(Box::new(substitute)),
            ),
            "semantic_evidence_substitution",
            "semantic_merge.result.evidence_kind",
        );
    }
}

// r[verify kiln.semantic_merge.independence.scenario.revalidation]
#[test]
fn downstream_operations_must_revalidate_exact_context() {
    let verdict = gate(&completed(CheckerVerdict::Verified));
    assert!(verdict.satisfies_required);
    let obligation = &verdict.revalidation;
    assert!(!obligation.grants_mutation_authority());
    assert_eq!(
        check_revalidation(obligation, &obligation.expectations),
        Ok(())
    );

    for moved_role in [MergeRole::Candidate, MergeRole::Base] {
        let observed: Vec<ContextExpectation> = obligation
            .expectations
            .iter()
            .cloned()
            .map(|mut expectation| {
                if expectation.role == moved_role {
                    expectation.state = pijul("state", "moved");
                }
                expectation
            })
            .collect();
        let field = alloc::format!(
            "semantic_merge.{}.revalidation",
            BindingComponent::for_role(moved_role).name()
        );
        expect(
            check_revalidation(obligation, &observed),
            "semantic_revalidation_moved",
            &field,
        );
    }
    let partial: Vec<ContextExpectation> =
        obligation.expectations.iter().skip(1).cloned().collect();
    expect(
        check_revalidation(obligation, &partial),
        "semantic_revalidation_context",
        "semantic_merge.revalidation",
    );
}

#[test]
fn producer_failures_record_preserved_outcomes() {
    for (failure, outcome) in [
        (
            ProducerFailure::TimedOut,
            SemanticOutcome::Exhausted(ExhaustionBound::WallTime),
        ),
        (ProducerFailure::Cancelled, SemanticOutcome::Cancelled),
        (ProducerFailure::ToolError, SemanticOutcome::ToolError),
    ] {
        let (state, checker_request) = dispatched(admitted());
        let state = run(
            &state,
            SemanticCheckCommand::ObserveFailure {
                attempt: checker_request.attempt.identity.clone(),
                failure,
            },
        );
        let verdict = gate(&state);
        assert_eq!(verdict.outcome, Some(outcome));
        assert!(verdict.blocks);
    }
}

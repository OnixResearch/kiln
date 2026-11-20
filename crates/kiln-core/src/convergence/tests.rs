use super::*;
use crate::domain::Outcome;
use crate::gating::CandidateChange;
use crate::provider::{ProviderLimits, ProviderRole};
use crate::types::{
    CandidateId, ContentId, ExternalRef, PipelineRevisionId, PolicyId, ProviderProfileId,
};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;

fn leading(requests: &[EvidenceBuildRequest]) -> &EvidenceBuildRequest {
    requests.first().unwrap_or_else(|| unreachable!("request"))
}

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn row(requirement: &str, state: EvidenceState) -> EvidenceRow {
    EvidenceRow {
        requirement_id: requirement.to_string(),
        specification_path: alloc::format!(".cairn/specs/{requirement}/spec.md"),
        row_blake3: content('a'),
        state,
        contract_id: Some("contract-a".to_string()),
        job_id: Some("job-a".to_string()),
    }
}

fn row_with_job(requirement: &str, state: EvidenceState, job: &str) -> EvidenceRow {
    let mut value = row(requirement, state);
    value.job_id = Some(job.to_string());
    value
}

fn mixed_rows() -> Vec<EvidenceRow> {
    vec![
        row("a-untargeted", EvidenceState::Untargeted),
        row("b-current", EvidenceState::Current),
        row("c-unrealized", EvidenceState::Unrealized),
        row("d-not-projected", EvidenceState::NotProjected),
        row("e-stale", EvidenceState::Stale),
        row("f-failed", EvidenceState::Failed),
        row("g-unsupported", EvidenceState::Unsupported),
        row("h-incomplete", EvidenceState::Incomplete),
    ]
}

fn candidate() -> CandidateSet {
    let change =
        ExternalRef::new("source", "change", "change-a").unwrap_or_else(|_| unreachable!());
    CandidateSet {
        id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        ordered_changes: vec![CandidateChange {
            change: change.clone(),
            target_revision: ExternalRef::new("source", "revision", "revision-a")
                .unwrap_or_else(|_| unreachable!()),
            source_snapshot: content('b'),
            pipeline_revision: PipelineRevisionId::new("pipeline-a")
                .unwrap_or_else(|_| unreachable!()),
            collaboration_provider: ProviderProfileId::new("collaboration-a")
                .unwrap_or_else(|_| unreachable!()),
        }],
        dependencies: Vec::new(),
        frozen_sources: BTreeMap::from([(change, content('b'))]),
        policy_id: PolicyId::new("policy-a").unwrap_or_else(|_| unreachable!()),
    }
}

fn binding() -> ConvergenceCampaignBinding {
    ConvergenceCampaignBinding {
        candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        candidate_set_identity: candidate_set_identity(&candidate())
            .unwrap_or_else(|_| unreachable!()),
        receipt_blake3: content('c'),
        revision_kind: "git".to_string(),
        revision: "d5b34c2d64b1227417a46df8ed8427f2d10ed3c8".to_string(),
        registry_blake3: content('d'),
        case_blake3: content('e'),
        policy_blake3: content('f'),
    }
}

fn revision() -> EvidenceCampaignRevision {
    let selection = select_unsatisfied(&[
        row_with_job("one", EvidenceState::Failed, "job-one"),
        row_with_job("two", EvidenceState::Stale, "job-two"),
    ])
    .unwrap_or_else(|_| unreachable!());
    mint_evidence_campaign(&binding(), &selection)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!())
}

fn plan(name: &str) -> EvidencePlanBinding {
    EvidencePlanBinding {
        plan_identity: content(name.chars().next().unwrap_or('a')),
        required_products: vec![Alias::new("bin").unwrap_or_else(|_| unreachable!())],
    }
}

fn build_selection() -> EvidenceBuildSelection {
    EvidenceBuildSelection {
        provider_profile: ProviderProfileId::new("mantle-build-v1")
            .unwrap_or_else(|_| unreachable!()),
        policy_identity: content('1'),
        pipeline_revision: PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!()),
        platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
        realization: EvidenceRealization::BuildFromPlan,
        plans: BTreeMap::from([
            ("job-one".to_string(), plan("a")),
            ("job-two".to_string(), plan("b")),
        ]),
    }
}

fn capability() -> CapabilityProfile {
    CapabilityProfile {
        contract_version: CONTRACT_VERSION_V1,
        profile_id: ProviderProfileId::new("mantle-build-v1").unwrap_or_else(|_| unreachable!()),
        role: ProviderRole::Build,
        namespace: ExternalRef::new("mantle", "build-provider", "d75158b0")
            .unwrap_or_else(|_| unreachable!()),
        operations: BTreeSet::from([Operation::Dispatch, Operation::Observe]),
        outcomes: BTreeSet::from([
            Outcome::Success,
            Outcome::DomainFailure,
            Outcome::Cancelled,
            Outcome::TimedOut,
            Outcome::ProviderError,
            Outcome::Unknown,
        ]),
        limits: ProviderLimits {
            max_request_bytes: 1_048_576,
            max_result_bytes: 1_048_576,
            max_in_flight: 4,
        },
    }
}

#[test]
fn selects_only_unsatisfied_targeted_rows_in_registry_order() {
    let selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    let selected: Vec<_> = selection
        .items
        .iter()
        .map(|item| item.requirement_id.as_str())
        .collect();
    assert_eq!(
        selected,
        vec![
            "c-unrealized",
            "e-stale",
            "f-failed",
            "g-unsupported",
            "h-incomplete"
        ]
    );
}

#[test]
fn exclusions_retain_their_reason() {
    let selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        selection.exclusions,
        vec![
            ExcludedRow {
                requirement_id: "a-untargeted".to_string(),
                state: EvidenceState::Untargeted,
                reason: ExclusionReason::Untargeted,
            },
            ExcludedRow {
                requirement_id: "b-current".to_string(),
                state: EvidenceState::Current,
                reason: ExclusionReason::Current,
            },
            ExcludedRow {
                requirement_id: "d-not-projected".to_string(),
                state: EvidenceState::NotProjected,
                reason: ExclusionReason::NotProjected,
            },
        ]
    );
}

#[test]
fn empty_rows_are_refused() {
    assert_eq!(
        select_unsatisfied(&[]).map(|_| ()),
        Err(Diagnostic::new(
            "convergence_rows_empty",
            "convergence.rows",
            "a convergence receipt must carry registry rows",
        ))
    );
}

#[test]
fn duplicate_requirement_rows_are_refused() {
    let rows = vec![
        row("same", EvidenceState::Failed),
        row("same", EvidenceState::Current),
    ];
    assert_eq!(
        select_unsatisfied(&rows).map(|_| ()),
        Err(Diagnostic::new(
            "convergence_duplicate_requirement",
            "convergence.rows.requirement_id",
            "a requirement identity appears more than once",
        ))
    );
}

#[test]
fn row_count_bound_is_enforced() {
    let rows: Vec<_> = (0..=MAX_CONVERGENCE_ROWS)
        .map(|index| row(&alloc::format!("r{index}"), EvidenceState::Failed))
        .collect();
    assert_eq!(
        select_unsatisfied(&rows).map(|_| ()),
        Err(Diagnostic::new(
            "convergence_row_bound",
            "convergence.rows",
            "convergence row count exceeds its named bound",
        ))
    );
}

#[test]
fn oversized_requirement_text_is_refused() {
    let mut value = row("ok", EvidenceState::Failed);
    value.requirement_id = "x".repeat(MAX_CONVERGENCE_ID_BYTES + 1);
    assert_eq!(
        select_unsatisfied(&[value]).map(|_| ()),
        Err(Diagnostic::new(
            "convergence_text_bound",
            "convergence.rows.requirement_id",
            "value is empty, oversized or contains control characters",
        ))
    );
}

#[test]
fn mints_one_frozen_revision_from_the_selection() {
    let selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    let revision = mint_evidence_campaign(&binding(), &selection)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(revision.schema, EVIDENCE_CAMPAIGN_SCHEMA_V1);
    assert_eq!(revision.items.len(), 5);
    assert_eq!(
        revision.candidate_set_identity,
        binding().candidate_set_identity
    );
    assert_eq!(revision.receipt_blake3, binding().receipt_blake3);
    assert_eq!(
        revision
            .items
            .iter()
            .map(|item| item.requirement_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "c-unrealized",
            "e-stale",
            "f-failed",
            "g-unsupported",
            "h-incomplete"
        ]
    );
}

#[test]
fn repeated_mint_is_identical() {
    let selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    let first = mint_evidence_campaign(&binding(), &selection).unwrap_or_else(|_| unreachable!());
    let second = mint_evidence_campaign(&binding(), &selection).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
}

#[test]
fn moved_candidate_or_row_changes_the_campaign_identity() {
    let selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    let base = mint_evidence_campaign(&binding(), &selection)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());

    let mut moved_candidate = binding();
    moved_candidate.candidate_set_identity = content('2');
    let moved = mint_evidence_campaign(&moved_candidate, &selection)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_ne!(base.campaign_id, moved.campaign_id);

    let mut rows = mixed_rows();
    rows.get_mut(2)
        .unwrap_or_else(|| unreachable!("row"))
        .row_blake3 = content('3');
    let changed_row = select_unsatisfied(&rows).unwrap_or_else(|_| unreachable!());
    let changed = mint_evidence_campaign(&binding(), &changed_row)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_ne!(base.campaign_id, changed.campaign_id);
}

#[test]
fn changed_item_order_changes_the_campaign_identity() {
    let selection = select_unsatisfied(&[
        row_with_job("one", EvidenceState::Failed, "job-one"),
        row_with_job("two", EvidenceState::Stale, "job-two"),
    ])
    .unwrap_or_else(|_| unreachable!());
    let reordered = select_unsatisfied(&[
        row_with_job("two", EvidenceState::Stale, "job-two"),
        row_with_job("one", EvidenceState::Failed, "job-one"),
    ])
    .unwrap_or_else(|_| unreachable!());
    let first = mint_evidence_campaign(&binding(), &selection)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let second = mint_evidence_campaign(&binding(), &reordered)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_ne!(first.campaign_id, second.campaign_id);
}

#[test]
fn satisfied_or_empty_selection_mints_none() {
    let satisfied = select_unsatisfied(&[
        row("a", EvidenceState::Current),
        row("b", EvidenceState::NotProjected),
        row("c", EvidenceState::Untargeted),
    ])
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(mint_evidence_campaign(&binding(), &satisfied), Ok(None));
}

#[test]
fn a_current_item_cannot_enter_a_campaign() {
    let mut selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    selection.items.push(row("current", EvidenceState::Current));
    assert_eq!(
        mint_evidence_campaign(&binding(), &selection).map(|_| ()),
        Err(Diagnostic::new(
            "campaign_item_state",
            "convergence.campaign.items",
            "a campaign item must name an unsatisfied evidence state",
        ))
    );
}

#[test]
fn a_duplicate_item_is_refused() {
    let mut selection = select_unsatisfied(&mixed_rows()).unwrap_or_else(|_| unreachable!());
    selection.items.push(
        selection
            .items
            .first()
            .cloned()
            .unwrap_or_else(|| unreachable!()),
    );
    assert_eq!(
        mint_evidence_campaign(&binding(), &selection).map(|_| ()),
        Err(Diagnostic::new(
            "campaign_duplicate_requirement",
            "convergence.campaign.items.requirement_id",
            "a campaign repeats a requirement identity",
        ))
    );
}

#[test]
fn a_missing_evidence_job_is_refused() {
    let mut rows = mixed_rows();
    rows.get_mut(2)
        .unwrap_or_else(|| unreachable!("row"))
        .job_id = None;
    let selection = select_unsatisfied(&rows).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        mint_evidence_campaign(&binding(), &selection).map(|_| ()),
        Err(Diagnostic::new(
            "campaign_item_job_missing",
            "convergence.campaign.items.job_id",
            "a campaign item must name its evidence job",
        ))
    );
}

#[test]
fn candidate_set_identity_binds_the_existing_candidate_value() {
    let base = candidate_set_identity(&candidate()).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        candidate_set_identity(&candidate()).unwrap_or_else(|_| unreachable!()),
        base
    );
    let mut moved = candidate();
    moved.policy_id = PolicyId::new("policy-b").unwrap_or_else(|_| unreachable!());
    assert_ne!(
        candidate_set_identity(&moved).unwrap_or_else(|_| unreachable!()),
        base
    );
    let mut moved_source = candidate();
    moved_source.frozen_sources = BTreeMap::from([(
        ExternalRef::new("source", "change", "change-a").unwrap_or_else(|_| unreachable!()),
        content('9'),
    )]);
    assert_ne!(
        candidate_set_identity(&moved_source).unwrap_or_else(|_| unreachable!()),
        base
    );
}

#[test]
fn plans_one_typed_request_per_item() {
    let requests = plan_evidence_builds(&revision(), &build_selection(), &capability())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(requests.len(), 2);
    assert_eq!(leading(&requests).job_id, "job-one");
    assert_eq!(
        requests
            .get(1)
            .unwrap_or_else(|| unreachable!("request"))
            .job_id,
        "job-two"
    );
    assert_eq!(leading(&requests).campaign_id, revision().campaign_id);
    assert_eq!(
        leading(&requests).candidate_set_identity,
        revision().candidate_set_identity
    );
}

#[test]
fn requests_are_deterministic_and_lower_to_build_plans() {
    let first = plan_evidence_builds(&revision(), &build_selection(), &capability())
        .unwrap_or_else(|_| unreachable!());
    let second = plan_evidence_builds(&revision(), &build_selection(), &capability())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
    let attempt = AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!());
    for request in &first {
        let plan = request
            .into_build_effect_plan(attempt.clone())
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(plan.effect_id, request.effect_id);
        assert_eq!(plan.idempotency_key, request.idempotency_key);
        assert_eq!(plan.plan_identity, request.plan_identity);
    }
}

#[test]
fn provider_without_dispatch_is_unsupported() {
    let mut profile = capability();
    profile.operations = BTreeSet::from([Operation::Observe]);
    assert_eq!(
        plan_evidence_builds(&revision(), &build_selection(), &profile).map(|_| ()),
        Err(Diagnostic::new(
            "unsupported_capability",
            "provider.operation",
            "provider does not declare the requested operation",
        ))
    );
}

#[test]
fn another_role_or_profile_is_refused() {
    let mut wrong_role = capability();
    wrong_role.role = ProviderRole::Gate;
    assert_eq!(
        plan_evidence_builds(&revision(), &build_selection(), &wrong_role).map(|_| ()),
        Err(Diagnostic::new(
            "role_mismatch",
            "provider.role",
            "provider role does not match the requested port",
        ))
    );
    let mut wrong_profile = capability();
    wrong_profile.profile_id =
        ProviderProfileId::new("other-v1").unwrap_or_else(|_| unreachable!());
    assert_eq!(
        plan_evidence_builds(&revision(), &build_selection(), &wrong_profile).map(|_| ()),
        Err(Diagnostic::new(
            "provider_substitution",
            "provider.profile_id",
            "selected provider profile does not match",
        ))
    );
}

#[test]
fn a_missing_outcome_class_is_refused() {
    let mut profile = capability();
    profile.outcomes.remove(&Outcome::Unknown);
    assert_eq!(
        plan_evidence_builds(&revision(), &build_selection(), &profile).map(|_| ()),
        Err(Diagnostic::new(
            "outcome_support_missing",
            "provider.outcomes",
            "provider does not preserve all required outcome classes",
        ))
    );
}

#[test]
fn preserve_realization_fails_before_effects() {
    let mut selection = build_selection();
    selection.realization = EvidenceRealization::PreserveCacheOnly;
    assert_eq!(
        plan_evidence_builds(&revision(), &selection, &capability()).map(|_| ()),
        Err(Diagnostic::new(
            "unsupported_capability",
            "convergence.verify.realization",
            "the pinned Mantle adapter exposes only build --plan, not a preserve or realize operation",
        ))
    );
}

#[test]
fn a_missing_plan_is_refused() {
    let mut selection = build_selection();
    selection.plans.remove("job-two");
    assert_eq!(
        plan_evidence_builds(&revision(), &selection, &capability()).map(|_| ()),
        Err(Diagnostic::new(
            "evidence_plan_missing",
            "convergence.verify.plans",
            "a campaign evidence job has no exact plan",
        ))
    );
}

#[test]
fn an_unexpected_plan_is_refused() {
    let mut selection = build_selection();
    selection.plans.insert("job-other".to_string(), plan("c"));
    assert_eq!(
        plan_evidence_builds(&revision(), &selection, &capability()).map(|_| ()),
        Err(Diagnostic::new(
            "evidence_plan_unexpected",
            "convergence.verify.plans",
            "a selected plan does not belong to this campaign",
        ))
    );
}

#[test]
fn duplicate_empty_and_unsorted_products_are_refused() {
    for products in [
        vec![],
        vec![
            Alias::new("bin").unwrap_or_else(|_| unreachable!()),
            Alias::new("bin").unwrap_or_else(|_| unreachable!()),
        ],
        vec![
            Alias::new("z").unwrap_or_else(|_| unreachable!()),
            Alias::new("a").unwrap_or_else(|_| unreachable!()),
        ],
    ] {
        let mut selection = build_selection();
        if let Some(plan) = selection.plans.get_mut("job-one") {
            plan.required_products = products.clone();
        }
        assert!(
            plan_evidence_builds(&revision(), &selection, &capability()).is_err(),
            "products must be refused: {products:?}"
        );
    }
}

#[test]
fn a_platform_change_changes_the_request_identity() {
    let first = plan_evidence_builds(&revision(), &build_selection(), &capability())
        .unwrap_or_else(|_| unreachable!());
    let mut moved = build_selection();
    moved.platform = Alias::new("aarch64-linux").unwrap_or_else(|_| unreachable!());
    let second =
        plan_evidence_builds(&revision(), &moved, &capability()).unwrap_or_else(|_| unreachable!());
    assert_ne!(
        leading(&first).idempotency_key,
        leading(&second).idempotency_key
    );
    assert_ne!(leading(&first).effect_id, leading(&second).effect_id);
}

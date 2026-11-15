//! Synthetic selected-campaign data in the disposable exploration workspace.
//! Canonical stores/projection are real; source observations and policies are fictional.
use std::path::{Path, PathBuf};

use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, EnumerationState, Freshness, MemberBinding,
    MemberObservation, MemberSelector, MemberStage, WorkflowProfile, selector_key,
};
use kiln_core::types::ContentId;
use kiln_implementer::campaign::CampaignStore;
use kiln_web_service::engine::ApiResponse;
use kiln_web_service::inventory::read_campaign_projection_with_inventory;

use super::{Result, error, inventory, json};

pub const CAMPAIGN_ID: &str = "synthetic-selected-cairn";
const EXAMPLE_CONCURRENCY: u16 = 1;
const EXAMPLE_OUTSTANDING_DECISIONS: u16 = 2;

fn identity(label: &str) -> Result<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(label.as_bytes()).to_hex().to_string())
        .map_err(|error| format!("synthetic identity invalid: {error:?}").into())
}

fn member(
    selector: MemberSelector,
    profile: WorkflowProfile,
    goal: CompletionGoal,
) -> Result<MemberBinding> {
    Ok(MemberBinding {
        selector,
        profile,
        profile_version: identity(&format!("synthetic-profile:{profile:?}"))?,
        repository_instructions: identity("synthetic-repository-instructions")?,
        policy: identity("synthetic-campaign-policy")?,
        required_checks: vec!["synthetic-independent-check".into()],
        required_reviews: if profile == WorkflowProfile::NoSpecDelta {
            Vec::new()
        } else {
            vec!["synthetic-independent-review".into()]
        },
        integration_policy: identity("synthetic-unqualified-integration-policy")?,
        goal,
        prerequisite_inclusion: false,
        context: Vec::new(),
    })
}

pub fn seed(root: &Path) -> Result<PathBuf> {
    let database = root.join("synthetic-campaign.sqlite");
    let mut store = CampaignStore::open(&database)?;
    let state = store.start(CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: CAMPAIGN_ID.into(),
        revision: 1,
        repository: "synthetic/disconnected-cairn".into(),
        members: vec![
            member(
                MemberSelector::WholeChange {
                    change: "synthetic-planning".into(),
                },
                WorkflowProfile::SpecDriven,
                CompletionGoal::NativeLifecycleComplete,
            )?,
            member(
                MemberSelector::Tasks {
                    change: "synthetic-assurance".into(),
                    tasks: vec!["task-review".into()],
                },
                WorkflowProfile::Assurance,
                CompletionGoal::VerifiedPullRequest,
            )?,
            member(
                MemberSelector::WholeChange {
                    change: "synthetic-lifecycle".into(),
                },
                WorkflowProfile::NoSpecDelta,
                CompletionGoal::NativeLifecycleComplete,
            )?,
        ],
        max_concurrency: EXAMPLE_CONCURRENCY,
        max_outstanding_decisions: EXAMPLE_OUTSTANDING_DECISIONS,
        parent_budget_id: identity("synthetic-parent-budget-not-observed")?,
        agent_definition: identity("synthetic-agent-definition")?,
        require_one_writer_per_repository: true,
    })?;
    let prerequisite = state
        .contract
        .members
        .first()
        .map(|binding| selector_key(&binding.selector))
        .ok_or("synthetic selection has no prerequisite")?;
    for (binding, (stage, enumeration, blocker)) in state.contract.members.iter().zip([
        (
            MemberStage::Planning,
            EnumerationState::IncompleteLimit,
            "Synthetic planning artifact missing; enumeration is partial",
        ),
        (
            MemberStage::Blocked,
            EnumerationState::Complete,
            "Synthetic independent <review> and current checks are missing",
        ),
        (
            MemberStage::Lifecycle,
            EnumerationState::Complete,
            "Synthetic completion-only work; no qualified integration/lifecycle route",
        ),
    ]) {
        let observation = MemberObservation {
            member: selector_key(&binding.selector),
            stage,
            enumeration,
            source_identity: Some(identity("synthetic-cairn-source-observation")?),
            policy_identity: binding.policy.clone(),
            subject_identity: None,
            integration_target_identity: None,
            profile_identity: binding.profile_version.clone(),
            dependencies: if stage == MemberStage::Blocked {
                vec![prerequisite.clone()]
            } else {
                Vec::new()
            },
            remaining_obligations: Default::default(),
            dependency_reason: (stage == MemberStage::Blocked)
                .then(|| "Synthetic planning prerequisite is unfinished".into()),
            canonical_work_key: None,
            work_identity: None,
            attempt_identity: None,
            attempt_fence: None,
            context_identity: None,
            native_context_identity: None,
            context_freshness: Freshness::NotApplicable,
            candidate_freshness: if stage == MemberStage::Blocked {
                Freshness::Stale
            } else {
                Freshness::Unknown
            },
            policy_freshness: Freshness::Current,
            verifier_freshness: Freshness::Unknown,
            target_freshness: Freshness::Unknown,
            evidence: Vec::new(),
            blockers: vec![blocker.into()],
            required_checks_satisfied: false,
            required_reviews_satisfied: false,
            delivery_route_supported: Some(false),
            recovery: None,
            consumed_budget: 0,
            reserved_budget: 0,
            unknown_budget: 0,
            cancelled: false,
        };
        let version = store
            .version(CAMPAIGN_ID)?
            .ok_or("synthetic campaign disappeared")?;
        store.observe(CAMPAIGN_ID, version, observation)?;
    }
    Ok(database)
}

pub fn response(database: &Path, method: &str, target: &str) -> Result<ApiResponse> {
    if method != "GET" {
        return Ok(error(405, "Campaign examples are read-only"));
    }
    if target != format!("/api/implementation/campaigns?campaign={CAMPAIGN_ID}") {
        return Ok(error(
            404,
            "Only the named synthetic campaign exists in this disconnected workspace",
        ));
    }
    let resources = inventory::projection(false)?;
    let projection =
        read_campaign_projection_with_inventory(database, CAMPAIGN_ID, Some(&resources))?
            .ok_or("synthetic campaign disappeared")?;
    Ok(json(200, serde_json::to_value(projection)?))
}

//! Versioned, read-only operator projection of admitted selected campaigns.
use std::{collections::BTreeMap, path::Path};

use kiln_core::implementation::campaign::{
    CampaignDecision, CampaignState, CompletionGoal, DecisionKind, DecisionRequestState,
    EnumerationState, Freshness, MemberObservation, MemberStage, SelectionBasis, selector_key,
};
use kiln_core::implementation::selected_work::{IntakeProfile, SelectedWorkSurface};
use kiln_core::inventory::{AttemptBinding, InventoryProjection};
use kiln_implementer::campaign::CampaignObservation;
use kiln_implementer::observation::ImplementationObservation;
use serde::Serialize;

pub const SCHEMA: &str = "kiln.selected-campaign-projection.v1";
pub const MAX_MEMBERS: usize = 1_000;

pub const MAX_PROJECTION_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, Serialize)]
pub struct WaitCategory {
    pub category: &'static str,
    pub state: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignProjection {
    pub schema: &'static str,
    pub identity: String,
    pub campaign_identity: String,
    pub resource_inventory_identity: Option<String>,
    pub campaign_id: String,
    pub store_version: u64,
    pub selection_identity: String,
    pub selection_revision: u64,
    pub repository_identity: String,
    pub members: Vec<MemberProjection>,
    pub pending_effects: Vec<serde_json::Value>,
    pub decisions: Vec<serde_json::Value>,
    pub budget: Option<serde_json::Value>,
    pub next_action: Option<CampaignDecision>,
    /// Why `next_action` was selected; member holds remain on their members.
    pub selection_basis: SelectionBasis,
    pub source: &'static str,
    pub non_claims: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemberProjection {
    pub member: String,
    pub change: String,
    pub tasks: Vec<String>,
    pub dependencies: Vec<String>,
    pub goal: CompletionGoal,
    pub stage: MemberStage,
    pub observed: bool,
    pub enumeration: Option<EnumerationState>,
    pub owner_snapshot: &'static str,
    pub work_identity: Option<String>,
    pub canonical_work_state: &'static str,
    pub canonical_work_revision: Option<u64>,
    pub attempt_identity: Option<String>,
    pub attempt_fence: Option<u64>,
    pub resource_inventory_identity: Option<String>,
    pub resource_binding_state: &'static str,
    pub resource_binding: Option<AttemptBinding>,
    pub candidate_identity: Option<String>,
    pub integration_target_identity: Option<String>,
    pub pending_effect_ids: Vec<String>,
    pub evidence: Vec<String>,
    pub input_freshness: Vec<(&'static str, Freshness)>,
    pub required_checks: Vec<String>,
    pub required_reviews: Vec<String>,
    pub checks_satisfied: Option<bool>,
    pub reviews_satisfied: Option<bool>,
    pub required_context_binding: bool,
    pub remaining_obligations: Vec<&'static str>,
    pub waits: Vec<&'static str>,
    pub wait_categories: Vec<WaitCategory>,
    pub blockers: Vec<String>,
    /// This member's retained wait, blocker or replan from campaign selection.
    pub selection_hold: Option<CampaignDecision>,
    /// Explicit intake profile of this member's Cairn observations.
    pub intake: IntakeProfile,
    /// Typed selected-work surface; the same value worker context and
    /// campaign inspection carry. Absent under the legacy intake.
    pub selected_work: Option<SelectedWorkSurface>,
}

pub(crate) fn identify(state: &CampaignState, version: u64) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(state, version))
        .map_err(|_| "campaign_projection_encoding_invalid")?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

/// Repository identity shared by campaign and Cairn projections: BLAKE3 of
/// the canonical repository binding, never the raw repository path.
#[must_use]
pub fn repository_identity(repository: &str) -> String {
    blake3::hash(repository.as_bytes()).to_hex().to_string()
}

fn member_projection(
    state: &CampaignState,
    member: &kiln_core::implementation::campaign::MemberBinding,
) -> MemberProjection {
    use kiln_core::implementation::campaign::MemberSelector;
    let key = selector_key(&member.selector);
    let (change, tasks) = match &member.selector {
        MemberSelector::WholeChange { change } => (change.clone(), Vec::new()),
        MemberSelector::Tasks { change, tasks } => (change.clone(), tasks.clone()),
    };
    let observation = state.observations.get(&key);
    let mut remaining = Vec::new();
    let mut waits = Vec::new();
    let (stage, enumeration, observed) = match observation {
        Some(o) => (o.stage, Some(o.enumeration.clone()), true),
        None => {
            remaining.push("owner_observation");
            waits.push("missing_source_or_owner_observation");
            (MemberStage::Missing, None, false)
        }
    };
    let mut item = MemberProjection {
        member: key.clone(),
        change,
        tasks,
        dependencies: observation
            .map(|o| o.dependencies.clone())
            .unwrap_or_default(),
        goal: member.goal,
        stage,
        observed,
        enumeration,
        owner_snapshot: if observed {
            "campaign_member_observation"
        } else {
            "not_observed"
        },
        work_identity: None,
        canonical_work_state: "not_joined",
        canonical_work_revision: None,
        attempt_identity: None,
        attempt_fence: None,
        resource_inventory_identity: None,
        resource_binding_state: "not_joined",
        resource_binding: None,
        candidate_identity: None,
        integration_target_identity: None,
        pending_effect_ids: Vec::new(),
        evidence: Vec::new(),
        input_freshness: Vec::new(),
        required_checks: member.required_checks.clone(),
        required_reviews: member.required_reviews.clone(),
        checks_satisfied: None,
        reviews_satisfied: None,
        required_context_binding: member.context.iter().any(|context| context.required),
        remaining_obligations: remaining,
        waits,
        wait_categories: Vec::new(),
        blockers: Vec::new(),
        selection_hold: None,
        intake: state
            .selected_work
            .get(&key)
            .map_or(IntakeProfile::LegacyReadinessCompatibility, |history| {
                history.intake
            }),
        selected_work: state
            .selected_work
            .get(&key)
            .and_then(|history| history.surface().ok()),
    };
    if let Some(o) = observation {
        fill_observation(&mut item, o);
    }
    let mut uncertain_effect = false;
    for effect in state
        .pending_effects
        .values()
        .filter(|e| e.member == key && !e.progress_applied)
    {
        item.pending_effect_ids
            .push(effect.effect_id.as_str().to_owned());
        uncertain_effect |= matches!(
            effect.state,
            kiln_core::implementation::campaign::EffectState::Unknown
                | kiln_core::implementation::campaign::EffectState::IntentPersisted
        );
    }
    if uncertain_effect {
        item.waits.push("uncertain_owner_effect");
    }
    let check_status = |required: bool, satisfied: Option<bool>| {
        if !required {
            "not_required"
        } else {
            match satisfied {
                Some(true) => "satisfied_by_retained_observation",
                Some(false) => "unsatisfied",
                None => "unknown",
            }
        }
    };
    item.wait_categories = vec![
        WaitCategory {
            category: "prerequisite_wait",
            state: if matches!(
                item.stage,
                MemberStage::Selected | MemberStage::Planning | MemberStage::Missing
            ) {
                "waiting"
            } else {
                "not_indicated"
            },
        },
        WaitCategory {
            category: "missing_artifact",
            state: if matches!(item.stage, MemberStage::Selected | MemberStage::Planning) {
                "planning_artifact_obligation"
            } else {
                "not_indicated"
            },
        },
        WaitCategory {
            category: "required_review",
            state: check_status(!item.required_reviews.is_empty(), item.reviews_satisfied),
        },
        WaitCategory {
            category: "verification_failure_or_missing_evidence",
            state: check_status(!item.required_checks.is_empty(), item.checks_satisfied),
        },
        WaitCategory {
            category: "worker_capability",
            state: "unknown_not_joined_to_campaign_snapshot",
        },
        WaitCategory {
            category: "execution_capacity",
            state: "unknown_not_joined_to_campaign_snapshot",
        },
        WaitCategory {
            category: "model_admission",
            state: "unknown_not_joined_to_campaign_snapshot",
        },
        WaitCategory {
            category: "uncertain_effect",
            state: if uncertain_effect {
                "pending_effect_visible"
            } else {
                "none_observed"
            },
        },
        WaitCategory {
            category: "aggregate_budget_exhaustion",
            state: "unknown_limit_not_exposed_by_campaign_snapshot",
        },
    ];
    item
}

fn fill_observation(item: &mut MemberProjection, o: &MemberObservation) {
    item.work_identity = o
        .work_identity
        .as_ref()
        .map(|identity| identity.as_str().to_owned());
    item.attempt_identity = o
        .attempt_identity
        .as_ref()
        .map(|identity| identity.as_str().to_owned());
    item.attempt_fence = o.attempt_fence;
    item.candidate_identity = o
        .subject_identity
        .as_ref()
        .map(|identity| identity.as_str().to_owned());
    item.integration_target_identity = o
        .integration_target_identity
        .as_ref()
        .map(|identity| identity.as_str().to_owned());
    item.evidence = o
        .evidence
        .iter()
        .map(|identity| identity.as_str().to_owned())
        .collect();
    item.input_freshness = vec![
        ("context", o.context_freshness),
        ("candidate", o.candidate_freshness),
        ("policy", o.policy_freshness),
        ("verifier", o.verifier_freshness),
        ("integration_target", o.target_freshness),
    ];
    item.checks_satisfied =
        (!item.required_checks.is_empty()).then_some(o.required_checks_satisfied);
    item.reviews_satisfied =
        (!item.required_reviews.is_empty()).then_some(o.required_reviews_satisfied);
    item.blockers = o.blockers.clone();
    if !o.remaining_obligations.is_empty() {
        item.remaining_obligations.push("selected_native_units");
        item.blockers.extend(
            o.remaining_obligations
                .iter()
                .map(|unit| format!("remaining selected native unit: {unit}")),
        );
    }
    if o.enumeration != EnumerationState::Complete {
        item.waits.push("partial_or_incomplete_enumeration");
    }
    if matches!(o.stage, MemberStage::Selected | MemberStage::Planning) {
        item.remaining_obligations.push("planning_artifact");
        item.waits.push("prerequisite_or_artifact");
    }
    if matches!(o.stage, MemberStage::Candidate | MemberStage::Verifying) {
        item.remaining_obligations.push("verification");
    }
    if !item.required_checks.is_empty() && !o.required_checks_satisfied {
        item.remaining_obligations.push("required_checks");
        item.waits.push("verification_failure_or_missing_evidence");
    }
    if !item.required_reviews.is_empty() && !o.required_reviews_satisfied {
        item.remaining_obligations.push("required_review");
        item.waits.push("required_review");
    }
    if matches!(
        o.stage,
        MemberStage::VerifiedCandidate | MemberStage::VerifiedPullRequest
    ) && item.goal != CompletionGoal::VerifiedCandidate
        && item.goal != CompletionGoal::VerifiedPullRequest
    {
        item.remaining_obligations.push("integration");
    }
    if matches!(o.stage, MemberStage::Integrated | MemberStage::Lifecycle)
        && item.goal == CompletionGoal::NativeLifecycleComplete
    {
        item.remaining_obligations
            .push("native_lifecycle_completion");
    }
    if [
        o.context_freshness,
        o.candidate_freshness,
        o.policy_freshness,
        o.verifier_freshness,
        o.target_freshness,
    ]
    .contains(&Freshness::Stale)
    {
        item.waits.push("stale_input_replan");
    }
    if item.required_context_binding && o.context_identity.is_none() {
        item.remaining_obligations.push("required_decision_context");
        item.waits.push("prerequisite_wait");
    }
    if o.stage == MemberStage::Ready {
        item.waits.push("owner_admission_not_implied");
    }
    item.waits.sort_unstable();
    item.waits.dedup();
    item.remaining_obligations.sort_unstable();
    item.remaining_obligations.dedup();
}

pub fn project(state: &CampaignState, store_version: u64) -> Result<CampaignProjection, String> {
    if state.contract.members.len() > MAX_MEMBERS {
        return Err("campaign_projection_member_bound".into());
    }
    let mut members: Vec<_> = state
        .contract
        .members
        .iter()
        .map(|m| member_projection(state, m))
        .collect();
    members.sort_by(|a, b| a.member.cmp(&b.member));
    let pending_effects = serde_json::to_value(state.pending_effects.values().collect::<Vec<_>>())
        .map_err(|_| "campaign_projection_encoding_invalid")?;
    let decisions = serde_json::to_value(state.decisions.values().collect::<Vec<_>>())
        .map_err(|_| "campaign_projection_encoding_invalid")?;
    // r[impl kiln.implementation.campaign.independent_progress.scenario.projection]
    let evaluation = state.evaluate();
    let mut holds: BTreeMap<String, CampaignDecision> = evaluation
        .holds
        .into_iter()
        .filter_map(|hold| Some((hold.member.clone()?, hold)))
        .collect();
    for member in &mut members {
        member.selection_hold = holds.remove(&member.member);
    }
    let mut budget = state
        .budget
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| "campaign_projection_encoding_invalid")?;
    if state.budget.is_none() {
        budget = Some(
            serde_json::json!({"state":"not_observed","parent_budget_identity":state.contract.parent_budget_id}),
        );
    }
    for request in state.decisions.values().filter(|d| {
        matches!(
            d.state,
            DecisionRequestState::Unissued | DecisionRequestState::Outstanding
        )
    }) {
        if request.kind == DecisionKind::RequiredReview
            && let Some(member) = members.iter_mut().find(|m| m.member == request.member)
            && !member.waits.contains(&"required_review")
        {
            member.waits.push("required_review");
        }
    }
    let campaign_identity = identify(state, store_version)?;
    let projection = CampaignProjection {
        schema: SCHEMA,
        identity: String::new(),
        campaign_identity,
        resource_inventory_identity: None,
        campaign_id: state.contract.campaign_id.clone(),
        store_version,
        selection_identity: state.selection_identity.as_str().to_owned(),
        selection_revision: state.contract.revision,
        repository_identity: repository_identity(&state.contract.repository),
        members,
        pending_effects: pending_effects.as_array().cloned().unwrap_or_default(),
        decisions: decisions.as_array().cloned().unwrap_or_default(),
        budget,
        next_action: Some(evaluation.decision),
        selection_basis: evaluation.basis,
        source: "existing_campaign_store_read_only",
        non_claims: vec![
            "display is not campaign authority",
            "no reservation, dispatch, provider discovery, or lifecycle mutation",
            "member observations are not independent proof of provider readiness",
        ],
    };
    let mut projection = projection;
    finalize_identity(&mut projection)?;
    Ok(projection)
}
pub fn enforce_bound(projection: &CampaignProjection) -> Result<(), String> {
    if serde_json::to_vec(projection)
        .map_err(|_| "campaign_projection_encoding_invalid")?
        .len()
        > MAX_PROJECTION_BYTES
    {
        return Err("campaign_projection_byte_bound".into());
    }
    Ok(())
}
/// Hashes the final redacted semantic projection with its own identity cleared.
/// Call only after canonical work and optional resource joins have completed.
pub fn finalize_identity(projection: &mut CampaignProjection) -> Result<(), String> {
    projection.identity.clear();
    let bytes =
        serde_json::to_vec(projection).map_err(|_| "campaign_projection_encoding_invalid")?;
    if bytes.len() > MAX_PROJECTION_BYTES {
        return Err("campaign_projection_byte_bound".into());
    }
    projection.identity = blake3::hash(&bytes).to_hex().to_string();
    enforce_bound(projection)
}

/// Join only exact campaign work/attempt/fence identities from a separate resource projection.
/// This copies admitted observation bindings; it does not confer readiness or hide inventory failure.
// r[impl kiln.dashboard.campaign.projection.scenario.unknown]
fn join_resource_inventory(projection: &mut CampaignProjection, inventory: &InventoryProjection) {
    for member in &mut projection.members {
        member.resource_inventory_identity = Some(inventory.identity.clone());
        let (Some(work), Some(attempt), Some(fence)) = (
            member.work_identity.as_deref(),
            member.attempt_identity.as_deref(),
            member.attempt_fence,
        ) else {
            member.resource_binding_state = "not_bound";
            continue;
        };
        let mut matches = inventory.snapshot.attempts.iter().filter(|binding| {
            binding.work_version == work && binding.attempt == attempt && binding.fence == fence
        });
        if let Some(binding) = matches.next() {
            if matches.next().is_none() {
                member.resource_binding = Some(binding.clone());
                member.resource_binding_state = "exact_match";
            } else {
                member.resource_binding_state = "ambiguous_duplicate";
            }
        } else {
            member.resource_binding_state = "no_exact_match";
        }
    }
}
/// Shared composition point for authenticated, CLI and inert offline consumers.
pub fn compose_resource_inventory(
    mut projection: CampaignProjection,
    inventory: Option<&InventoryProjection>,
) -> Result<CampaignProjection, String> {
    projection.resource_inventory_identity = inventory.map(|snapshot| snapshot.identity.clone());
    if let Some(snapshot) = inventory {
        join_resource_inventory(&mut projection, snapshot);
    } else {
        for member in &mut projection.members {
            member.resource_inventory_identity = None;
            member.resource_binding = None;
            member.resource_binding_state = "not_joined";
        }
    }
    finalize_identity(&mut projection)?;
    Ok(projection)
}

/// Enriches work identities with their exact canonical current-revision observation.
pub fn join_current_work(
    projection: &mut CampaignProjection,
    observation: &ImplementationObservation,
) {
    for member in &mut projection.members {
        let Some(work) = member.work_identity.as_deref() else {
            member.canonical_work_state = "not_bound";
            continue;
        };
        let Ok(identity) = kiln_core::types::ContentId::from_blake3_hex(work) else {
            member.canonical_work_state = "malformed_binding";
            continue;
        };
        match observation.current_work(&identity) {
            Ok(current) => {
                member.canonical_work_revision = Some(current.revision);
                member.canonical_work_state = "current";
                member
                    .input_freshness
                    .push(("canonical_work", Freshness::Current));
            }
            Err(kiln_provider_api::inventory::InventoryReadFailure::StaleWork) => {
                member.canonical_work_state = "stale";
                member
                    .input_freshness
                    .push(("canonical_work", Freshness::Stale));
                member.waits.push("stale_work_identity");
                member.remaining_obligations.push("current_work_version");
            }
            Err(kiln_provider_api::inventory::InventoryReadFailure::MalformedObservation) => {
                member.canonical_work_state = "malformed_source";
                member
                    .input_freshness
                    .push(("canonical_work", Freshness::Unknown));
            }
            Err(_) => {
                member.canonical_work_state = "unavailable";
                member
                    .input_freshness
                    .push(("canonical_work", Freshness::Unknown));
            }
        }
    }
}

pub fn read_campaign(path: &Path, campaign_id: &str) -> Result<Option<CampaignProjection>, String> {
    let observation = CampaignObservation::open(path)?;
    let Some((state, version)) = observation.inspect(campaign_id)? else {
        return Ok(None);
    };
    let mut projection = project(&state, version)?;
    match ImplementationObservation::open(path) {
        Ok(work_observation) => join_current_work(&mut projection, &work_observation),
        Err(_) => {
            for member in &mut projection.members {
                if member.work_identity.is_some() {
                    member.canonical_work_state = "unavailable";
                    member
                        .input_freshness
                        .push(("canonical_work", Freshness::Unknown));
                }
            }
        }
    }
    finalize_identity(&mut projection)?;
    Ok(Some(projection))
}
pub fn read_campaign_with_inventory(
    path: &Path,
    campaign_id: &str,
    inventory: Option<&InventoryProjection>,
) -> Result<Option<CampaignProjection>, String> {
    read_campaign(path, campaign_id)?
        .map(|projection| compose_resource_inventory(projection, inventory))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::campaign::{
        CAMPAIGN_SCHEMA, CampaignContract, CampaignOperation, HumanDecisionRequest, MemberBinding,
        MemberSelector, WorkflowProfile,
    };
    use kiln_core::types::ContentId;
    use kiln_implementer::campaign::CampaignStore;

    fn id() -> ContentId {
        ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|error| unreachable!("{error:?}"))
    }
    fn contract() -> CampaignContract {
        CampaignContract {
            schema: CAMPAIGN_SCHEMA.into(),
            campaign_id: "campaign<&".into(),
            revision: 2,
            repository: "repo".into(),
            members: vec![MemberBinding {
                selector: MemberSelector::Tasks {
                    change: "change-a".into(),
                    tasks: vec!["one".into()],
                },
                profile: WorkflowProfile::Unknown,
                profile_version: id(),
                repository_instructions: id(),
                policy: id(),
                required_checks: vec!["check".into()],
                required_reviews: vec!["review".into()],
                integration_policy: id(),
                goal: CompletionGoal::NativeLifecycleComplete,
                prerequisite_inclusion: false,
                context: Vec::new(),
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 1,
            parent_budget_id: id(),
            agent_definition: id(),
            require_one_writer_per_repository: true,
        }
    }

    fn observed(member: &str, stage: MemberStage, dependencies: Vec<String>) -> MemberObservation {
        let verified = stage == MemberStage::VerifiedCandidate;
        MemberObservation {
            member: member.into(),
            stage,
            enumeration: EnumerationState::Complete,
            source_identity: Some(id()),
            policy_identity: id(),
            subject_identity: Some(id()),
            integration_target_identity: None,
            profile_identity: id(),
            dependencies,
            remaining_obligations: Default::default(),
            dependency_reason: None,
            canonical_work_key: Some(id()),
            work_identity: Some(id()),
            attempt_identity: None,
            attempt_fence: None,
            context_identity: None,
            native_context_identity: None,
            context_freshness: Freshness::NotApplicable,
            candidate_freshness: Freshness::Current,
            policy_freshness: Freshness::Current,
            verifier_freshness: if verified {
                Freshness::Current
            } else {
                Freshness::Unknown
            },
            target_freshness: Freshness::Unknown,
            evidence: Vec::new(),
            blockers: Vec::new(),
            required_checks_satisfied: true,
            required_reviews_satisfied: !verified,
            delivery_route_supported: Some(true),
            recovery: None,
            consumed_budget: 0,
            reserved_budget: 0,
            unknown_budget: 0,
            cancelled: false,
        }
    }

    #[test]
    // r[verify kiln.implementation.campaign.independent_progress.scenario.projection]
    fn waiting_members_keep_their_holds_while_independent_work_is_next() {
        let member = |change: &str, goal: CompletionGoal, reviews: Vec<String>| MemberBinding {
            selector: MemberSelector::WholeChange {
                change: change.into(),
            },
            profile: WorkflowProfile::SpecDriven,
            profile_version: id(),
            repository_instructions: id(),
            policy: id(),
            required_checks: Vec::new(),
            required_reviews: reviews,
            integration_policy: id(),
            goal,
            prerequisite_inclusion: false,
            context: Vec::new(),
        };
        let mut selected = contract();
        selected.members = vec![
            member(
                "change-a",
                CompletionGoal::VerifiedPullRequest,
                vec!["review".into()],
            ),
            member("change-b", CompletionGoal::VerifiedCandidate, Vec::new()),
            member("change-c", CompletionGoal::VerifiedCandidate, Vec::new()),
        ];
        let mut state =
            CampaignState::start(selected).unwrap_or_else(|error| unreachable!("{error:?}"));
        for observation in [
            observed("change-a/*", MemberStage::VerifiedCandidate, Vec::new()),
            observed("change-b/*", MemberStage::Ready, vec!["change-a/*".into()]),
            observed("change-c/*", MemberStage::Ready, Vec::new()),
        ] {
            state
                .observe(observation)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
        }
        state
            .add_decision(HumanDecisionRequest {
                identity: id(),
                kind: DecisionKind::RequiredReview,
                member: "change-a/*".into(),
                work_identity: Some(id()),
                decision_revision: None,
                policy_identity: id(),
                candidate_identity: None,
                state: DecisionRequestState::Outstanding,
                created_ms: 1,
                question: "review the verified candidate".into(),
                options: vec!["approve".into(), "reject".into()],
                recommendation: None,
                priority: 1,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            })
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let projection = project(&state, 4).unwrap_or_else(|error| unreachable!("{error:?}"));
        let next = projection
            .next_action
            .as_ref()
            .unwrap_or_else(|| unreachable!("next action"));
        assert_eq!(next.operation, CampaignOperation::Code);
        assert_eq!(next.member.as_deref(), Some("change-c/*"));
        assert_eq!(projection.selection_basis, SelectionBasis::MemberAction);
        let member = |key: &str| {
            projection
                .members
                .iter()
                .find(|member| member.member == key)
                .unwrap_or_else(|| unreachable!("projected member {key}"))
        };
        let review = member("change-a/*");
        assert!(review.waits.contains(&"required_review"));
        let hold = review
            .selection_hold
            .as_ref()
            .unwrap_or_else(|| unreachable!("review hold"));
        assert_eq!(hold.owner.as_deref(), Some("review-owner"));
        assert_eq!(hold.effect_intent, Some(id()));
        let dependent = member("change-b/*")
            .selection_hold
            .as_ref()
            .unwrap_or_else(|| unreachable!("dependency hold"));
        assert!(dependent.reason.contains("dependency"));
        assert!(member("change-c/*").selection_hold.is_none());
        let json = crate::inventory::render_campaign_json(&projection)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(json.contains("\"selection_basis\": \"member_action\""));
        let html = crate::inventory::render_campaign_html(&projection)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(html.contains("required review response is outstanding"));
    }

    #[test]
    // r[verify kiln.dashboard.campaign.projection.scenario.partial]
    fn absent_campaign_store_does_not_create_database_and_keeps_selection_obligation() {
        let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error:?}"));
        let path = directory.path().join("absent.db");
        assert!(read_campaign(&path, "not-there").is_err());
        assert!(!path.exists());
        let state =
            CampaignState::start(contract()).unwrap_or_else(|error| unreachable!("{error:?}"));
        let projection = project(&state, 7).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(projection.members.len(), 1);
        let member = projection
            .members
            .first()
            .unwrap_or_else(|| unreachable!("projection has one member"));
        assert_eq!(member.stage, MemberStage::Missing);
        assert_eq!(member.goal, CompletionGoal::NativeLifecycleComplete);
        assert!(
            member
                .waits
                .contains(&"missing_source_or_owner_observation")
        );
        assert_eq!(
            member
                .wait_categories
                .iter()
                .find(|w| w.category == "aggregate_budget_exhaustion")
                .map(|w| w.state),
            Some("unknown_limit_not_exposed_by_campaign_snapshot")
        );
        let encoded =
            serde_json::to_string(&projection).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(encoded.contains(&projection.identity));
        let html = crate::inventory::render_campaign_html(&projection)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(html.contains("campaign&lt;&amp;"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("fetch("));
    }

    #[test]
    // r[verify kiln.dashboard.campaign.projection.scenario.identity]
    // r[verify kiln.dashboard.campaign.projection.scenario.readonly]
    fn canonical_campaign_projection_is_read_only_and_identity_tracks_stored_version() {
        let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error:?}"));
        let path = directory.path().join("campaign.db");
        let mut store =
            CampaignStore::open(&path).unwrap_or_else(|error| unreachable!("{error:?}"));
        store
            .start(contract())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        drop(store);
        let before = std::fs::read(&path).unwrap_or_else(|error| unreachable!("{error:?}"));
        let first = read_campaign(&path, "campaign<&")
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .unwrap_or_else(|| unreachable!());
        let second = read_campaign(&path, "campaign<&")
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .unwrap_or_else(|| unreachable!());
        let after = std::fs::read(&path).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(before, after);
        assert_eq!(first.identity, second.identity);
        assert_eq!(first.store_version, second.store_version);
        let cli_json = crate::inventory::run_campaign_cli_args(&[
            path.display().to_string(),
            "campaign<&".into(),
            "json".into(),
        ])
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let cli_html = crate::inventory::run_campaign_cli_args(&[
            path.display().to_string(),
            "campaign<&".into(),
            "html".into(),
        ])
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(cli_json.contains(&first.identity));
        assert!(cli_html.contains(&first.identity));
        assert!(cli_html.contains("campaign&lt;&amp;"));
        assert!(!cli_html.contains("<script"));
    }
    #[test]
    // r[verify kiln.dashboard.campaign.projection.scenario.current_work]
    fn unchanged_campaign_row_gets_new_identity_when_current_work_observation_changes() {
        let state =
            CampaignState::start(contract()).unwrap_or_else(|error| unreachable!("{error:?}"));
        let original = project(&state, 9).unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut current_work = original.clone();
        let member = current_work
            .members
            .first_mut()
            .unwrap_or_else(|| unreachable!("projection has one member"));
        member.canonical_work_state = "current";
        member.canonical_work_revision = Some(7);
        member
            .input_freshness
            .push(("canonical_work", Freshness::Current));
        finalize_identity(&mut current_work).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(original.store_version, current_work.store_version);
        assert_eq!(original.campaign_identity, current_work.campaign_identity);
        assert_ne!(original.identity, current_work.identity);
    }
}

//! Synthetic selected-campaign data in the disposable exploration workspace.
//! Canonical stores/projection are real; source observations and policies are fictional.
use std::path::{Path, PathBuf};

use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, DecisionKind, DecisionRequestState,
    EnumerationState, Freshness, HumanDecisionRequest, MemberBinding, MemberObservation,
    MemberSelector, MemberStage, WorkflowProfile, selector_key,
};
use kiln_core::types::ContentId;
use kiln_implementer::campaign::CampaignStore;
use kiln_web_service::campaign_controls::Controls;
use kiln_web_service::engine::ApiResponse;
use kiln_web_service::inventory::read_campaign_projection_with_inventory;

use super::{ACTOR, Result, error, inventory, json};

pub const CAMPAIGN_ID: &str = "synthetic-selected-cairn";
/// A startable synthetic definition: native lifecycle completion on a native route.
pub const START_PREVIEW_ID: &str = "synthetic-start-preview";
/// A verified pull request goal that the native no-PR route does not offer.
pub const UNSUPPORTED_ID: &str = "synthetic-unsupported-endpoint";
const EXAMPLE_CONCURRENCY: u16 = 1;
const EXAMPLE_OUTSTANDING_DECISIONS: u16 = 2;
const REVIEW_ROLE: &str = "synthetic-independent-review";

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
            member(
                MemberSelector::WholeChange {
                    change: "synthetic-ready".into(),
                },
                WorkflowProfile::NoSpecDelta,
                CompletionGoal::VerifiedCandidate,
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
            Some("Synthetic planning artifact missing; enumeration is partial"),
        ),
        (
            MemberStage::Blocked,
            EnumerationState::Complete,
            Some("Synthetic independent <review> and current checks are missing"),
        ),
        (
            MemberStage::Lifecycle,
            EnumerationState::Complete,
            Some("Synthetic completion-only work; no qualified integration/lifecycle route"),
        ),
        // Depends on no held member, so it is the proposed next action.
        (MemberStage::Ready, EnumerationState::Complete, None),
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
            blockers: blocker.map(Into::into).into_iter().collect(),
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
    // Two outstanding questions fill the bound, so the third waits unissued and
    // the independent ready member stays the proposed next action.
    let key = |index: usize| {
        state
            .contract
            .members
            .get(index)
            .map(|binding| selector_key(&binding.selector))
            .ok_or("synthetic member missing")
    };
    let decisions = [
        (
            key(1)?,
            DecisionKind::RequiredReview,
            "Does the synthetic assurance plan satisfy the independent review?",
            vec!["approve", "request_changes"],
            None,
            2,
        ),
        (
            key(2)?,
            DecisionKind::Preference,
            "Which native lifecycle receipt should the completion-only member record?",
            vec!["sync", "archive"],
            Some("archive"),
            1,
        ),
        (
            key(3)?,
            DecisionKind::EmpiricalUncertainty,
            "Is the ready member's intermittent synthetic check worth a separate experiment?",
            vec!["propose_experiment", "accept_current_evidence"],
            None,
            1,
        ),
    ];
    for (index, (member, kind, question, options, recommendation, priority)) in
        decisions.into_iter().enumerate()
    {
        let version = store
            .version(CAMPAIGN_ID)?
            .ok_or("synthetic campaign disappeared")?;
        store.add_decision(
            CAMPAIGN_ID,
            version,
            HumanDecisionRequest {
                identity: identity(&format!("synthetic-decision:{index}"))?,
                kind,
                member,
                work_identity: None,
                decision_revision: None,
                policy_identity: identity("synthetic-campaign-policy")?,
                candidate_identity: None,
                state: DecisionRequestState::Outstanding,
                created_ms: u64::try_from(index)? + 1,
                question: question.into(),
                options: options.into_iter().map(Into::into).collect(),
                recommendation: recommendation.map(Into::into),
                priority,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            },
        )?;
    }
    Ok(database)
}

/// A control profile granting the synthetic operator every command over the
/// synthetic campaign and two frozen synthetic definitions.
pub fn controls(root: &Path, database: &Path) -> Result<Controls> {
    let mut definitions = Vec::new();
    for (campaign_id, change, goal) in [
        (
            START_PREVIEW_ID,
            "synthetic-next",
            CompletionGoal::NativeLifecycleComplete,
        ),
        (
            UNSUPPORTED_ID,
            "synthetic-pull-request",
            CompletionGoal::VerifiedPullRequest,
        ),
    ] {
        let contract = CampaignContract {
            schema: CAMPAIGN_SCHEMA.into(),
            campaign_id: campaign_id.into(),
            revision: 1,
            repository: "synthetic/disconnected-cairn".into(),
            members: vec![member(
                MemberSelector::WholeChange {
                    change: change.into(),
                },
                WorkflowProfile::NoSpecDelta,
                goal,
            )?],
            max_concurrency: EXAMPLE_CONCURRENCY,
            max_outstanding_decisions: EXAMPLE_OUTSTANDING_DECISIONS,
            parent_budget_id: identity("synthetic-parent-budget-not-observed")?,
            agent_definition: identity("synthetic-agent-definition")?,
            require_one_writer_per_repository: true,
        };
        let path = root.join(format!("{campaign_id}.contract.json"));
        std::fs::write(&path, serde_json::to_vec(&contract)?)?;
        definitions.push(serde_json::json!({"contract": path, "delivery_route": "native_git"}));
    }
    let profile = root.join("synthetic-campaign-controls.json");
    std::fs::write(
        &profile,
        serde_json::to_vec(&serde_json::json!({
            "schema": "kiln.web-campaign-controls.v1",
            "campaign_database": database,
            "grants": [{
                "principal": ACTOR,
                "campaigns": [CAMPAIGN_ID, START_PREVIEW_ID, UNSUPPORTED_ID],
                "commands": ["start", "continue", "pause", "cancel", "issue_decision", "resolve_decision"],
                "review_roles": [REVIEW_ROLE],
            }],
            "definitions": definitions,
        }))?,
    )?;
    Ok(Controls::new(&profile, Some(database))?)
}

pub fn response(database: &Path, method: &str, target: &str) -> Result<ApiResponse> {
    if method != "GET" {
        return Ok(error(405, "Campaign projections are read-only"));
    }
    let Some(campaign_id) = target
        .strip_prefix("/api/implementation/campaigns?campaign=")
        .filter(|id| [CAMPAIGN_ID, START_PREVIEW_ID, UNSUPPORTED_ID].contains(id))
    else {
        return Ok(error(
            404,
            "Only the named synthetic campaigns exist in this disconnected workspace",
        ));
    };
    let resources = inventory::projection(false)?;
    let Some(projection) =
        read_campaign_projection_with_inventory(database, campaign_id, Some(&resources))?
    else {
        return Ok(error(404, "This synthetic campaign has not been started"));
    };
    Ok(json(200, serde_json::to_value(projection)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // r[verify kiln.implementation.campaign.independent_progress.scenario.projection]
    #[test]
    fn synthetic_campaign_proposes_independent_work_beside_member_holds() -> Result {
        let root = tempfile::tempdir()?;
        let database = seed(root.path())?;
        let target = format!("/api/implementation/campaigns?campaign={CAMPAIGN_ID}");
        let projection: Value = serde_json::from_slice(&response(&database, "GET", &target)?.body)?;
        assert_eq!(projection["next_action"]["operation"], "code");
        assert_eq!(projection["next_action"]["member"], "synthetic-ready/*");
        assert_eq!(projection["selection_basis"], "member_action");
        let members = projection["members"]
            .as_array()
            .ok_or("projection has no members")?;
        assert_eq!(members.len(), 4);
        for member in members {
            let hold = &member["selection_hold"];
            if member["member"] == "synthetic-ready/*" {
                assert!(hold.is_null());
            } else {
                assert!(hold["reason"].is_string(), "{member}");
            }
        }
        Ok(())
    }

    // r[verify kiln.implementation.campaign.controls.scenario.console]
    #[test]
    fn synthetic_controls_offer_answers_and_previews_without_moving_the_next_action() -> Result {
        use kiln_web_service::campaign_controls::ControlRequest;
        use kiln_web_service::engine::Role;
        let root = tempfile::tempdir()?;
        let database = seed(root.path())?;
        let controls = controls(root.path(), &database)?;
        let read = |target: &str| -> Result<Value> {
            let result = controls.response(&ControlRequest {
                method: "GET",
                target,
                body: b"",
                principal: ACTOR,
                role: Role::Operator,
                now_ms: 1,
            });
            assert_eq!(result.response.status, 200, "{target}");
            Ok(serde_json::from_slice(&result.response.body)?)
        };
        let inbox = read(&format!(
            "/api/implementation/campaigns/inbox?campaign={CAMPAIGN_ID}"
        ))?;
        let decisions = inbox
            .pointer("/decisions")
            .and_then(Value::as_array)
            .ok_or("inbox has no decisions")?;
        let states: Vec<_> = decisions
            .iter()
            .map(|decision| {
                (
                    decision.pointer("/state").and_then(Value::as_str),
                    decision.pointer("/actions").cloned(),
                )
            })
            .collect();
        assert_eq!(
            states,
            [
                (Some("outstanding"), Some(serde_json::json!(["resolve"]))),
                (Some("outstanding"), Some(serde_json::json!(["resolve"]))),
                (Some("unissued"), Some(serde_json::json!([]))),
            ]
        );
        assert_eq!(
            inbox.pointer("/operations"),
            Some(&serde_json::json!(["pause", "cancel", "resolve_decision"]))
        );
        let definitions = read("/api/implementation/campaign-definitions")?;
        assert_eq!(
            (
                definitions.pointer("/definitions/0/can_start"),
                definitions.pointer("/definitions/1/refusals"),
            ),
            (
                Some(&Value::Bool(true)),
                Some(&serde_json::json!(["endpoint_unsupported"]))
            )
        );
        let target = format!("/api/implementation/campaigns?campaign={CAMPAIGN_ID}");
        let projection: Value = serde_json::from_slice(&response(&database, "GET", &target)?.body)?;
        assert_eq!(
            projection.pointer("/next_action/member"),
            Some(&serde_json::json!("synthetic-ready/*"))
        );
        Ok(())
    }
}

//! Synthetic selected-campaign data in the disposable exploration workspace.
//! Canonical stores/projection are real; source observations and policies are fictional.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, DecisionKind, DecisionRequestState,
    EnumerationState, Freshness, HumanDecisionRequest, MemberBinding, MemberObservation,
    MemberSelector, MemberStage, WorkflowProfile, selector_key,
};
use kiln_core::implementation::{
    AcceptanceObligation, DEFAULT_LEASE_MS, Enrollment, FailureClass, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, SourceRef, SupersessionRule, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
use kiln_implementer::campaign::CampaignStore;
use kiln_web_service::campaign_controls::Controls;
use kiln_web_service::engine::ApiResponse;
use kiln_web_service::inventory::campaign_projection::repository_identity;
use kiln_web_service::inventory::read_campaign_projection_with_inventory;

use super::{ACTOR, Example, Result, error, inventory, json, now_ms};

pub const CAMPAIGN_ID: &str = "synthetic-selected-cairn";
/// A startable synthetic definition: native lifecycle completion on a native route.
pub const START_PREVIEW_ID: &str = "synthetic-start-preview";
/// A verified pull request goal that the native no-PR route does not offer.
pub const UNSUPPORTED_ID: &str = "synthetic-unsupported-endpoint";
const EXAMPLE_CONCURRENCY: u16 = 1;
const EXAMPLE_OUTSTANDING_DECISIONS: u16 = 2;
const REVIEW_ROLE: &str = "synthetic-independent-review";
const REPOSITORY: &str = "synthetic/disconnected-cairn";
/// The planning task's budget allows one attempt, so its failure is final.
const PLAN_ATTEMPTS: u8 = 1;
const TASK_ATTEMPTS: u8 = 2;
const ATTEMPT_MINUTES: u32 = 30;
const TASK_PRIORITY: u8 = 1;

fn identity(label: &str) -> Result<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(label.as_bytes()).to_hex().to_string())
        .map_err(|error| format!("synthetic identity invalid: {error:?}").into())
}

fn core<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| format!("synthetic canonical work invalid: {error:?}").into())
}

/// One synthetic Cairn task as canonical implementation work.
fn task(
    native_id: &str,
    revision: u64,
    dependencies: Vec<ContentId>,
    attempts: u8,
) -> Result<ImplementationWork> {
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision,
        actor: ACTOR.into(),
        repository: REPOSITORY.into(),
        project: "synthetic-cairn".into(),
        base: None,
        title: format!("Synthetic Cairn task {native_id}"),
        requested_outcome: "Synthetic implementation; no provider is contacted".into(),
        acceptance: vec![AcceptanceObligation {
            id: format!("{native_id}#checks"),
            statement: "The synthetic task's checks pass".into(),
        }],
        dependencies,
        permitted_paths: vec!["synthetic".into()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: identity("synthetic-verification-profile")?,
        budget: WorkBudget {
            attempts,
            minutes_per_attempt: ATTEMPT_MINUTES,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: TASK_PRIORITY,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: core(ExternalRef::new("cairn", "task", native_id))?,
            native_id: native_id.into(),
            version: identity(&format!("synthetic-cairn-snapshot:{native_id}:{revision}"))?,
        }],
    })
}

/// Canonical Cairn task work beside the synthetic campaign: A (planning) fails
/// its one attempt, B (assurance) depends on A's exact work identity, and C
/// (ready) is claimed at revision 1, superseded by revision 2 and claimed again
/// while the campaign member still binds revision 1. D (lifecycle) stays a
/// completion-only selection without canonical work.
struct SyntheticWork {
    plan: ImplementationWork,
    review: ImplementationWork,
    build_v1: ImplementationWork,
    build_v2: ImplementationWork,
}

impl SyntheticWork {
    fn new() -> Result<Self> {
        let plan = task("synthetic-planning/task-plan", 1, Vec::new(), PLAN_ATTEMPTS)?;
        let review = task(
            "synthetic-assurance/task-review",
            1,
            vec![core(plan.identity())?],
            TASK_ATTEMPTS,
        )?;
        Ok(Self {
            plan,
            review,
            build_v1: task("synthetic-ready/task-build", 1, Vec::new(), TASK_ATTEMPTS)?,
            build_v2: task("synthetic-ready/task-build", 2, Vec::new(), TASK_ATTEMPTS)?,
        })
    }

    /// Admit and drive the work through the real canonical owner. Admission
    /// order decides each claim: the failed plan's budget is spent, and the
    /// review task is admitted last so nothing claims it.
    fn seed(&self, database: &Path) -> Result {
        let now = now_ms()?;
        let mut implementer = Implementer::open_with_local_operator(
            database,
            Enrollment {
                repositories: BTreeSet::from([REPOSITORY.to_string()]),
                actors: BTreeSet::from([ACTOR.to_string()]),
            },
            "example-operator",
        )?;
        let claim = |implementer: &mut Implementer, work: &ImplementationWork| -> Result<_> {
            let attempt = implementer
                .claim_next(now, DEFAULT_LEASE_MS)?
                .ok_or("synthetic work was not claimed")?;
            if attempt.work_identity != core(work.identity())? {
                return Err("synthetic claim selected other work".into());
            }
            Ok(attempt)
        };
        implementer.admit(&self.plan, None)?;
        let plan = claim(&mut implementer, &self.plan)?;
        implementer.fail(
            &plan.identity,
            plan.fence,
            now,
            FailureClass::Implementation,
            "synthetic plan attempt failed its own checks",
        )?;
        implementer.admit(&self.build_v1, None)?;
        claim(&mut implementer, &self.build_v1)?;
        implementer.admit_with_supersession(
            &self.build_v2,
            None,
            SupersessionRule::AbortActive,
            now,
        )?;
        claim(&mut implementer, &self.build_v2)?;
        implementer.admit(&self.review, None)?;
        Ok(())
    }
}

/// Catalog entries for the Cairn changes page over the synthetic stores.
pub fn examples() -> Vec<Example> {
    let change = |name: &str| {
        format!(
            "/?page=cairn&cairn_repository={}&cairn_change={name}",
            repository_identity(REPOSITORY)
        )
    };
    vec![
        Example::new(
            "Cairn changes: filter, board and inspect next steps",
            "/?page=cairn",
            "Switch the change register between List and Board, filter by lane, blockers or open decisions, and open a change's next steps. Lanes summarize the least-advanced unit, not lifecycle authority. The canonical stores are disposable; no Cairn, worker or provider is contacted.",
        ),
        Example::new(
            "Prerequisite with a failed attempt",
            change("synthetic-planning"),
            "A needs attention: its attempt failed and its planning member waits with partial enumeration. Open its unit row for the attempt timeline and follow the dependency graph to B. All five stages stay separate.",
        ),
        Example::new(
            "Dependent task and its review wait",
            change("synthetic-assurance"),
            "B is ready to start and depends on A's exact work identity. Its next steps name the review owner; inspect the graph, open the prerequisite, or answer only the review decision the campaign inbox admits.",
        ),
        Example::new(
            "Superseded attempt and stale campaign binding",
            change("synthetic-ready"),
            "C is in progress on revision 2. Expand its unit to compare the historical revision 1 attempt with the active attempt. Its stale campaign binding names the replan owner; its empirical question stays read-only until the campaign can issue it.",
        ),
        Example::new(
            "Completion-only selection without canonical work",
            change("synthetic-lifecycle"),
            "D awaits work: selected for native lifecycle completion only, it has no canonical work or attempts. Inspect its member and open decision; All changes and Browse changes keep your register filters.",
        ),
    ]
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
    let work = SyntheticWork::new()?;
    // The ready member keeps binding revision 1 after revision 2 supersedes it.
    let stale_binding = (
        core(work.build_v1.canonical_key())?,
        core(work.build_v1.identity())?,
    );
    let mut store = CampaignStore::open(&database)?;
    let state = store.start(CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: CAMPAIGN_ID.into(),
        revision: 1,
        repository: REPOSITORY.into(),
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
            canonical_work_key: (stage == MemberStage::Ready).then(|| stale_binding.0.clone()),
            work_identity: (stage == MemberStage::Ready).then(|| stale_binding.1.clone()),
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
    drop(store);
    work.seed(&database)?;
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
            repository: REPOSITORY.into(),
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

    // r[verify kiln.cairn.workspace.scenario.example]
    // r[verify kiln.cairn.workspace.agents.scenario.source]
    #[test]
    fn synthetic_cairn_changes_join_real_work_attempts_and_members() -> Result {
        use kiln_web_service::cairn_workspace::agents::InventoryObservation;
        use kiln_web_service::cairn_workspace::http::{
            DETAIL_ROUTE, INDEX_ROUTE, response as cairn,
        };
        use kiln_web_service::engine::Role;
        use serde_json::json;
        let root = tempfile::tempdir()?;
        let database = seed(root.path())?;
        let inventory_reads = std::cell::Cell::new(0_usize);
        let observe = || {
            inventory_reads.set(inventory_reads.get() + 1);
            crate::inventory::cairn_observation()
        };
        let read = |target: &str| -> Result<Value> {
            let response = cairn(Some(&database), "GET", target, Role::Viewer, &observe);
            assert_eq!(response.status, 200, "{target}");
            Ok(serde_json::from_slice(&response.body)?)
        };
        let index = read(INDEX_ROUTE)?;
        assert_eq!(inventory_reads.get(), 0, "the index reads no inventory");
        let changes: Vec<&str> = index["changes"]
            .as_array()
            .ok_or("index has no changes")?
            .iter()
            .filter_map(|change| change["change"].as_str())
            .collect();
        assert_eq!(
            changes,
            [
                "synthetic-assurance",
                "synthetic-lifecycle",
                "synthetic-planning",
                "synthetic-ready"
            ]
        );
        // A needs attention, B is ready, C is in progress and D awaits work;
        // only A has no open decision.
        let lanes: Vec<(&str, &str, u64)> = index["changes"]
            .as_array()
            .ok_or("index has no changes")?
            .iter()
            .filter_map(|change| {
                Some((
                    change["change"].as_str()?,
                    change["lane"].as_str()?,
                    change["open_decisions"].as_u64()?,
                ))
            })
            .collect();
        assert_eq!(
            lanes,
            [
                ("synthetic-assurance", "ready", 1),
                ("synthetic-lifecycle", "awaiting", 1),
                ("synthetic-planning", "attention", 0),
                ("synthetic-ready", "in_progress", 1)
            ]
        );
        let detail = |change: &str| {
            read(&format!(
                "{DETAIL_ROUTE}?repository={}&change={change}",
                repository_identity(REPOSITORY)
            ))
        };
        // A: the prerequisite's one attempt failed.
        let planning = detail("synthetic-planning")?;
        assert_eq!(
            planning.pointer("/units/0/native_id"),
            Some(&json!("synthetic-planning/task-plan"))
        );
        assert_eq!(
            planning.pointer("/units/0/attempts/0/phase"),
            Some(&json!("failed"))
        );
        assert_eq!(planning.pointer("/units/0/attempts/1"), None);
        // The agents section reads the synthetic inventory beside the change
        // and keeps the captured Lattice workflow revisions apart.
        assert_eq!(inventory_reads.get(), 1);
        assert_eq!(planning.pointer("/agents/source"), Some(&json!("admitted")));
        assert_eq!(
            planning.pointer("/agents/workflow_revisions/source"),
            Some(&json!("admitted"))
        );
        assert_eq!(
            planning.pointer(
                "/agents/workflow_revisions/definitions/0/selected_for_new_sessions/state"
            ),
            Some(&json!("recorded"))
        );
        assert_eq!(
            planning.pointer("/agents/definitions/0/selected_for_new_sessions/state"),
            Some(&json!("recorded"))
        );
        let text = planning.to_string();
        assert!(!text.contains("worker-a-session") && !text.contains("worker-a-workspace"));
        for withheld in ["session-a", "session-b", "verify-change", "definition_name"] {
            assert!(!text.contains(withheld), "{withheld} leaked");
        }
        // An inventory refusal leaves the canonical change readable.
        let refused = cairn(
            Some(&database),
            "GET",
            &format!(
                "{DETAIL_ROUTE}?repository={}&change=synthetic-planning",
                repository_identity(REPOSITORY)
            ),
            Role::Viewer,
            &|| InventoryObservation::Unavailable("inventory_input_unavailable".into()),
        );
        assert_eq!(refused.status, 200);
        let refused: Value = serde_json::from_slice(&refused.body)?;
        assert_eq!(
            (
                refused.pointer("/agents/source"),
                refused.pointer("/agents/reason"),
                refused.pointer("/agents/definitions"),
                refused.pointer("/units/0/native_id"),
            ),
            (
                Some(&json!("unavailable")),
                Some(&json!("inventory_input_unavailable")),
                Some(&json!([])),
                Some(&json!("synthetic-planning/task-plan")),
            )
        );
        assert_ne!(refused["identity"], planning["identity"]);
        // B: depends on A's exact current work identity and waits for its review.
        let assurance = detail("synthetic-assurance")?;
        assert_eq!(
            assurance.pointer("/units/0/current_work/0/dependencies/0/state"),
            Some(&json!("current"))
        );
        assert_eq!(
            assurance.pointer("/units/0/current_work/0/dependencies/0/native_ids"),
            Some(&json!(["synthetic-planning/task-plan"]))
        );
        assert!(
            assurance["decisions"]
                .as_array()
                .ok_or("detail has no decisions")?
                .iter()
                .any(|decision| decision["category"] == "required_review")
        );
        // C: revision 1's attempt is historical; the member still binds it.
        let ready = detail("synthetic-ready")?;
        assert_eq!(
            (
                ready.pointer("/units/0/attempts/0/applicability"),
                ready.pointer("/units/0/attempts/1/applicability"),
                ready.pointer("/units/0/facets/execution"),
            ),
            (
                Some(&json!("historical")),
                Some(&json!("current")),
                Some(&json!("claimed"))
            )
        );
        assert_eq!(
            ready.pointer("/memberships/0/work_binding"),
            Some(&json!("stale"))
        );
        assert!(
            ready["blockers"]
                .as_array()
                .ok_or("detail has no blockers")?
                .iter()
                .any(|blocker| blocker["condition"] == "stale_work_binding")
        );
        // D: completion-only, a campaign selection without canonical work.
        let lifecycle = detail("synthetic-lifecycle")?;
        assert_eq!(
            (
                lifecycle.pointer("/units"),
                lifecycle.pointer("/memberships/0/goal"),
                lifecycle.pointer("/memberships/0/stage"),
            ),
            (
                Some(&json!([])),
                Some(&json!("native_lifecycle_complete")),
                Some(&json!("lifecycle"))
            )
        );
        Ok(())
    }
}

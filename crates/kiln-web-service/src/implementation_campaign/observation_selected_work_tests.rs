//! Worker context, campaign inspection and the UI projection carry one
//! obligation projection for one admitted snapshot. Synthetic retained
//! producer/consumer fixtures; no live campaign.
use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, CampaignContract, CampaignState, CompletionGoal, EnumerationState, Freshness,
    MemberBinding, MemberObservation, MemberSelector, MemberStage, WorkflowProfile, selector_key,
};
use kiln_core::implementation::selected_work::{
    AdmittedSelectedWork, IntakeProfile, SelectedWorkObservation, SurfaceObservation,
    TransitionKind, bind_attempt,
};
use kiln_core::types::ContentId;
use kiln_implementer::campaign::{CampaignObservation, CampaignStore};

use super::worker_surface;

const FIXTURES: &str = "../../fixtures/cairn/selected-work";

fn id(c: char) -> ContentId {
    ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn snapshot(name: &str) -> AdmittedSelectedWork {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES)
        .join(format!("{name}.admitted.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap_or_default())
        .unwrap_or_else(|_| unreachable!())
}

fn contract() -> CampaignContract {
    CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: "selected-work".into(),
        revision: 1,
        repository: "repo".into(),
        members: vec![MemberBinding {
            selector: MemberSelector::Tasks {
                change: "sample".into(),
                tasks: vec!["t1".into()],
            },
            profile: WorkflowProfile::SpecDriven,
            profile_version: id('a'),
            repository_instructions: id('a'),
            policy: id('a'),
            required_checks: Vec::new(),
            required_reviews: Vec::new(),
            integration_policy: id('a'),
            goal: CompletionGoal::VerifiedCandidate,
            prerequisite_inclusion: false,
            context: Vec::new(),
        }],
        max_concurrency: 1,
        max_outstanding_decisions: 1,
        parent_budget_id: id('a'),
        agent_definition: id('a'),
        require_one_writer_per_repository: true,
    }
}

fn executing(member: &str) -> MemberObservation {
    MemberObservation {
        member: member.into(),
        stage: MemberStage::Executing,
        enumeration: EnumerationState::Complete,
        source_identity: Some(id('a')),
        policy_identity: id('a'),
        subject_identity: Some(id('a')),
        integration_target_identity: Some(id('c')),
        profile_identity: id('a'),
        dependencies: Vec::new(),
        remaining_obligations: Default::default(),
        dependency_reason: None,
        canonical_work_key: Some(id('a')),
        work_identity: Some(id('b')),
        attempt_identity: Some(id('d')),
        attempt_fence: Some(1),
        context_identity: None,
        native_context_identity: None,
        context_freshness: Freshness::NotApplicable,
        candidate_freshness: Freshness::Unknown,
        policy_freshness: Freshness::Current,
        verifier_freshness: Freshness::Unknown,
        target_freshness: Freshness::Current,
        evidence: Vec::new(),
        blockers: Vec::new(),
        required_checks_satisfied: false,
        required_reviews_satisfied: false,
        delivery_route_supported: Some(true),
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}

fn inspect(path: &std::path::Path) -> (CampaignState, u64) {
    CampaignObservation::open(path)
        .unwrap_or_else(|_| unreachable!())
        .inspect("selected-work")
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!())
}

#[test]
// r[verify kiln.selected_work.completion.scenario.surfaces]
// r[verify kiln.selected_work.attempts]
fn worker_inspection_and_ui_carry_one_obligation_projection() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("campaign.db");
    let mut store = CampaignStore::open(&path).unwrap_or_else(|_| unreachable!());
    let state = store.start(contract()).unwrap_or_else(|_| unreachable!());
    let member = state
        .contract
        .members
        .first()
        .map(|binding| selector_key(&binding.selector))
        .unwrap_or_else(|| unreachable!());
    let version = store
        .version("selected-work")
        .ok()
        .flatten()
        .unwrap_or_default();
    store
        .observe("selected-work", version, executing(&member))
        .unwrap_or_else(|error| unreachable!("{error}"));
    let planning = snapshot("planning");
    let version = store
        .version("selected-work")
        .ok()
        .flatten()
        .unwrap_or_default();
    store
        .observe_selected_work(
            "selected-work",
            version,
            &member,
            IntakeProfile::TypedSnapshotV1,
            SelectedWorkObservation::Admitted {
                snapshot: Box::new(planning.clone()),
                target: Some(id('c')),
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let attempt =
        bind_attempt(id('d'), id('b'), Some(id('c')), planning).unwrap_or_else(|_| unreachable!());
    let version = store
        .version("selected-work")
        .ok()
        .flatten()
        .unwrap_or_default();
    store
        .bind_selected_work_attempt("selected-work", version, &member, attempt.clone())
        .unwrap_or_else(|error| unreachable!("{error}"));

    let (inspected, version) = inspect(&path);
    let inspection = inspected
        .selected_work
        .get(&member)
        .and_then(|history| history.surface().ok())
        .unwrap_or_else(|| unreachable!());
    let worker = worker_surface(&inspected, &member).unwrap_or_else(|_| unreachable!());
    let ui = crate::inventory::campaign_projection::project(&inspected, version)
        .unwrap_or_else(|_| unreachable!())
        .members
        .into_iter()
        .next()
        .and_then(|item| item.selected_work)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(worker, inspection);
    assert_eq!(ui, inspection);
    assert_eq!(
        worker.guidance.as_ref().map(|guidance| &guidance.identity),
        Some(&attempt.guidance.identity)
    );

    // A later evaluation is a refresh; every surface keeps the bound guidance.
    let mut store = CampaignStore::open(&path).unwrap_or_else(|_| unreachable!());
    store
        .observe_selected_work(
            "selected-work",
            version,
            &member,
            IntakeProfile::TypedSnapshotV1,
            SelectedWorkObservation::Admitted {
                snapshot: Box::new(snapshot("passing")),
                target: Some(id('c')),
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let (inspected, version) = inspect(&path);
    let worker = worker_surface(&inspected, &member).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        worker.pending_transition.as_ref().map(|t| t.kind),
        Some(TransitionKind::Refresh)
    );
    assert_eq!(worker.current_contract, Some(attempt.identity.clone()));
    assert_eq!(
        worker.guidance.as_ref().map(|guidance| &guidance.identity),
        Some(&attempt.guidance.identity)
    );
    assert_eq!(
        inspected
            .selected_work
            .get(&member)
            .and_then(|history| history.current.as_ref()),
        Some(&attempt)
    );

    // An unavailable read withholds worker guidance but keeps history.
    store
        .observe_selected_work(
            "selected-work",
            version,
            &member,
            IntakeProfile::TypedSnapshotV1,
            SelectedWorkObservation::Unavailable {
                reason: "selected-work read timed out".into(),
                refusal: None,
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let (inspected, version) = inspect(&path);
    assert!(worker_surface(&inspected, &member).is_err());
    let ui = crate::inventory::campaign_projection::project(&inspected, version)
        .unwrap_or_else(|_| unreachable!())
        .members
        .into_iter()
        .next()
        .and_then(|item| item.selected_work)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(ui.observation, SurfaceObservation::Unavailable);
    assert_eq!(ui.current_contract, Some(attempt.identity));
    // The legacy intake is explicit and cannot be silently switched.
    assert!(
        store
            .observe_selected_work(
                "selected-work",
                version,
                &member,
                IntakeProfile::LegacyReadinessCompatibility,
                SelectedWorkObservation::NotObserved,
            )
            .is_err()
    );
}

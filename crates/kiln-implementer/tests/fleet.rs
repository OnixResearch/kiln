use kiln_core::fleet::{
    AgentStatus, AttemptViewState, ClaimCommand, ClaimConflict, ClaimEffect, ClaimOutcome,
    ClaimState, DEFAULT_CLAIM_LEASE_MS, DEFAULT_NUDGE_AFTER_MS, DEFAULT_RENEW_BEFORE_MS,
    FleetAgent, FleetEffect, FleetPolicy, ForeignClaim, MirrorStatus, Observation, Placement,
    SessionState, SessionWorkspace, parse_scope, parse_scope_text,
};
use kiln_core::implementation::{
    AcceptanceObligation, DEFAULT_LEASE_MS, DISPATCH_PLAN_SCHEMA, DispatchControls,
    DispatchWorkspace, Enrollment, FailureClass, IMPLEMENTATION_WORK_SCHEMA,
    INTERACTIVE_AGENT_EFFECT, INTERACTIVE_PROVIDER_PROFILE, ImplementationAttempt,
    ImplementationDispatchPlan, ImplementationWork, SourceRef, TerminalPolicy, WorkBudget,
    WorkSourceKind, WorkspaceIsolation,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::{FLEET_STATE_SCHEMA, Implementer};
use std::collections::BTreeSet;
use std::path::Path;

type TestResult = Result<(), String>;

const COORDINATOR: &str = "britton-desktop";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn core<T, E: std::fmt::Debug>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}

fn content(character: char) -> Result<ContentId, String> {
    core(ContentId::from_blake3_hex(character.to_string().repeat(64)))
}

fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    }
}

fn work(issue: &str, paths: &[&str]) -> Result<ImplementationWork, String> {
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "onix/kiln".to_string(),
        project: "kiln".to_string(),
        base: Some(core(ExternalRef::new("git", "commit", &"1".repeat(40)))?),
        title: "Fleet work".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: paths.iter().map(|path| (*path).to_string()).collect(),
        permitted_effects: BTreeSet::from([
            "write_source".to_string(),
            INTERACTIVE_AGENT_EFFECT.to_string(),
        ]),
        verification_profile: content('9')?,
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: core(ExternalRef::new("github", "issue", issue))?,
            native_id: issue.to_string(),
            version: content('1')?,
        }],
    })
}

fn open(path: &Path) -> Result<Implementer, String> {
    Implementer::open_with_local_operator(path, enrollment(), "test-operator")
}

/// A store with one admitted work item claimed at `now_ms`.
fn claimed(
    path: &Path,
    paths: &[&str],
    now_ms: u64,
) -> Result<(Implementer, ImplementationWork, ImplementationAttempt), String> {
    let mut implementer = open(path)?;
    let work = work("onix/kiln#45", paths)?;
    implementer.admit(&work, None)?;
    let attempt = implementer
        .claim_next(now_ms, DEFAULT_LEASE_MS)?
        .ok_or("work was not claimed")?;
    Ok((implementer, work, attempt))
}

fn acquire(scope: &str, session: &str) -> Result<ClaimCommand, String> {
    Ok(ClaimCommand::Acquire {
        scope: core(parse_scope_text(scope))?,
        holder: FleetAgent {
            tool: "omp".to_string(),
            host: "aspen3".to_string(),
            session: session.to_string(),
        },
        purpose: "fleet store test".to_string(),
        lease_ms: DEFAULT_CLAIM_LEASE_MS,
        attempt: None,
    })
}

fn placement(attempt: &ImplementationAttempt) -> Placement {
    Placement {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        work: attempt.work_identity.clone(),
        work_revision: attempt.work_revision,
        machine: "aspen3".to_string(),
        kind: "omp".to_string(),
    }
}

fn started() -> Result<Observation, String> {
    Ok(Observation::Started {
        agent_name: "kiln-omp-1".to_string(),
        workspace: core(SessionWorkspace::new(
            "kiln-1",
            "pane-1",
            "/srv/worktrees/attempt",
            "kiln/attempt",
            &"a".repeat(40),
        ))?,
    })
}

fn status(status: AgentStatus) -> Observation {
    Observation::Status { status }
}

fn refused<T: std::fmt::Debug>(result: Result<T, String>, code: &str) -> TestResult {
    match result {
        Err(error) if error.starts_with(code) => Ok(()),
        other => Err(format!("expected refusal {code}, got {other:?}")),
    }
}

fn interactive_plan(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
) -> Result<ImplementationDispatchPlan, String> {
    Ok(ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.to_string(),
        work_identity: core(work.identity())?,
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: content('a')?,
            isolation: WorkspaceIsolation::InteractiveWorktree,
            network: true,
            credentials: true,
            projection: None,
        },
        provider_profile: core(ProviderProfileId::new(INTERACTIVE_PROVIDER_PROFILE))?,
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 60,
        token_budget: 1_000,
        invocation_budget: 1,
        expected_output: content('b')?,
        execution: None,
    })
}

// r[verify kiln.fleet.claims] r[verify kiln.fleet.claims.scenario.conflict]
#[test]
fn fleet_claims_conflict_with_live_attempt_paths_and_persist() -> TestResult {
    let directory = core(tempfile::tempdir())?;
    let path = directory.path().join("implementation.db");
    let mut implementer = open(&path)?;
    assert!(implementer.fleet_attempt_scopes(0)?.is_empty());
    assert_eq!(implementer.fleet_state()?.schema, FLEET_STATE_SCHEMA);
    drop(implementer);

    let (mut implementer, _, attempt) = claimed(&path, &["crates"], 0)?;
    let scopes = implementer.fleet_attempt_scopes(1)?;
    assert_eq!(scopes.len(), 1);
    let scope = scopes.first().ok_or("scope missing")?;
    assert_eq!(scope.attempt, attempt.identity);
    assert_eq!(scope.scope, core(parse_scope("onix/kiln", "crates"))?);
    assert!(
        implementer
            .fleet_attempt_scopes(attempt.lease_until_ms)?
            .is_empty()
    );

    let (outcome, effects) = implementer.fleet_claim(
        acquire("onix/kiln crates/kiln-core", "s1")?,
        1,
        COORDINATOR,
        &[],
    )?;
    assert!(
        matches!(&outcome, ClaimOutcome::Conflict(ClaimConflict::Attempt { attempt: held, .. }) if *held == attempt.identity),
        "{outcome:?}"
    );
    assert!(effects.is_empty());
    let (outcome, _) =
        implementer.fleet_claim(acquire("ONIX/kiln .", "s1")?, 1, COORDINATOR, &[])?;
    assert!(
        matches!(
            outcome,
            ClaimOutcome::Conflict(ClaimConflict::Attempt { .. })
        ),
        "{outcome:?}"
    );

    let (outcome, effects) =
        implementer.fleet_claim(acquire("onix/kiln docs", "s1")?, 1, COORDINATOR, &[])?;
    let ClaimOutcome::Granted(granted) = outcome else {
        return Err(format!("disjoint scope was not granted: {outcome:?}"));
    };
    assert_eq!(effects.len(), 1);
    assert!(
        matches!(effects.first(), Some(ClaimEffect::PostClaim { identity, .. }) if *identity == granted.identity)
    );
    assert_eq!(implementer.fleet_pending_mirror(COORDINATOR)?, effects);

    let foreign = [ForeignClaim {
        scope: core(parse_scope_text("onix/kiln nix/modules"))?,
        message_id: "<claim@aspen3>".to_string(),
        author: "aspen3 <aspen3@swarm.local>".to_string(),
        date_unix_ms: 1,
    }];
    let (outcome, _) =
        implementer.fleet_claim(acquire("onix/kiln nix", "s2")?, 2, COORDINATOR, &foreign)?;
    assert!(
        matches!(
            outcome,
            ClaimOutcome::Conflict(ClaimConflict::Foreign { .. })
        ),
        "{outcome:?}"
    );

    // Once the attempt ends its paths no longer conflict.
    implementer.fail(
        &attempt.identity,
        attempt.fence,
        3,
        FailureClass::Implementation,
        "agent gave up",
    )?;
    assert!(implementer.fleet_attempt_scopes(4)?.is_empty());
    let (outcome, _) = implementer.fleet_claim(
        acquire("onix/kiln crates/kiln-core", "s1")?,
        4,
        COORDINATOR,
        &[],
    )?;
    assert!(matches!(outcome, ClaimOutcome::Granted(_)), "{outcome:?}");
    drop(implementer);

    let mut reopened = open(&path)?;
    let state = reopened.fleet_state()?;
    assert_eq!(state.claims.claims.len(), 2);
    assert_eq!(state.claims.next_fence, 3);
    assert_eq!(state.claims.claims.get(&granted.identity), Some(&granted));
    assert_eq!(reopened.fleet_pending_mirror(COORDINATOR)?.len(), 2);
    Ok(())
}

// r[verify kiln.fleet.claims] r[verify kiln.fleet.claims.scenario.conflict]
#[test]
fn unparseable_attempt_paths_widen_to_the_whole_repository() -> TestResult {
    let directory = core(tempfile::tempdir())?;
    let (mut implementer, _, attempt) = claimed(
        &directory.path().join("implementation.db"),
        &["docs/my notes"],
        0,
    )?;
    let scopes = implementer.fleet_attempt_scopes(1)?;
    assert_eq!(scopes.len(), 1);
    let scope = scopes.first().ok_or("scope missing")?;
    assert_eq!(scope.attempt, attempt.identity);
    assert_eq!(scope.scope, core(parse_scope("onix/kiln", "."))?);
    let (outcome, _) =
        implementer.fleet_claim(acquire("onix/kiln crates", "s1")?, 1, COORDINATOR, &[])?;
    assert!(
        matches!(
            outcome,
            ClaimOutcome::Conflict(ClaimConflict::Attempt { .. })
        ),
        "{outcome:?}"
    );
    Ok(())
}

// r[verify kiln.fleet.claims] r[verify kiln.fleet.claims.scenario.lease]
#[test]
fn fleet_claim_fences_are_checked_and_persist_across_reopen() -> TestResult {
    let directory = core(tempfile::tempdir())?;
    let path = directory.path().join("implementation.db");
    let mut implementer = open(&path)?;
    let (outcome, _) =
        implementer.fleet_claim(acquire("onix/kiln docs", "s1")?, 10, COORDINATOR, &[])?;
    let ClaimOutcome::Granted(claim) = outcome else {
        return Err(format!("claim was not granted: {outcome:?}"));
    };
    let stale = claim.fence.saturating_add(1);
    refused(
        implementer.fleet_claim(
            ClaimCommand::Renew {
                identity: claim.identity.clone(),
                fence: stale,
                lease_ms: DEFAULT_CLAIM_LEASE_MS,
            },
            20,
            COORDINATOR,
            &[],
        ),
        "fleet_claim_stale",
    )?;
    refused(
        implementer.fleet_claim(
            ClaimCommand::Release {
                identity: claim.identity.clone(),
                fence: stale,
                note: "done".to_string(),
            },
            20,
            COORDINATOR,
            &[],
        ),
        "fleet_claim_stale",
    )?;
    let reopened_claim = open(&path)?
        .fleet_state()?
        .claims
        .claims
        .get(&claim.identity)
        .cloned();
    assert_eq!(reopened_claim, Some(claim.clone()));

    let (outcome, _) = implementer.fleet_claim(
        ClaimCommand::Renew {
            identity: claim.identity.clone(),
            fence: claim.fence,
            lease_ms: DEFAULT_CLAIM_LEASE_MS,
        },
        20,
        COORDINATOR,
        &[],
    )?;
    assert!(matches!(outcome, ClaimOutcome::Renewed(_)), "{outcome:?}");
    let (outcome, effects) = implementer.fleet_claim(
        ClaimCommand::Release {
            identity: claim.identity.clone(),
            fence: claim.fence,
            note: "done".to_string(),
        },
        30,
        COORDINATOR,
        &[],
    )?;
    assert!(matches!(outcome, ClaimOutcome::Released(_)), "{outcome:?}");
    assert!(effects.is_empty(), "release waits for the claim receipt");
    assert!(matches!(
        implementer.fleet_pending_mirror(COORDINATOR)?.as_slice(),
        [ClaimEffect::PostClaim { .. }]
    ));
    drop(implementer);

    let mut reopened = open(&path)?;
    let state = reopened.fleet_state()?;
    let released = state
        .claims
        .claims
        .get(&claim.identity)
        .ok_or("claim missing")?;
    assert!(matches!(
        released.state,
        ClaimState::Released { at_ms: 30, .. }
    ));
    assert!(matches!(
        reopened.fleet_pending_mirror(COORDINATOR)?.as_slice(),
        [ClaimEffect::PostClaim { .. }]
    ));
    reopened.fleet_claim(
        ClaimCommand::MirrorObserved {
            identity: claim.identity.clone(),
            release: false,
            status: MirrorStatus::Posted {
                message_id: Some("<claim@kiln>".to_string()),
            },
        },
        35,
        COORDINATOR,
        &[],
    )?;
    drop(reopened);
    let mut reopened = open(&path)?;
    assert!(matches!(
        reopened.fleet_pending_mirror(COORDINATOR)?.as_slice(),
        [ClaimEffect::PostRelease { references: Some(reference), .. }] if reference == "<claim@kiln>"
    ));
    let (outcome, _) =
        reopened.fleet_claim(acquire("onix/kiln docs", "s2")?, 40, COORDINATOR, &[])?;
    let ClaimOutcome::Granted(next) = outcome else {
        return Err(format!("released scope was not granted again: {outcome:?}"));
    };
    assert!(next.fence > claim.fence);
    Ok(())
}

// r[verify kiln.fleet.dispatch]
#[test]
fn sessions_bind_only_to_current_attempts_and_observations_persist() -> TestResult {
    let directory = core(tempfile::tempdir())?;
    let path = directory.path().join("implementation.db");
    let (mut implementer, _, attempt) = claimed(&path, &["crates"], 0)?;

    let mut stale = placement(&attempt);
    stale.fence = attempt.fence.saturating_add(1);
    refused(
        implementer.fleet_bind_session(stale, 1),
        "implementation_fence_stale",
    )?;
    refused(
        implementer.fleet_bind_session(placement(&attempt), attempt.lease_until_ms),
        "implementation_fence_stale",
    )?;
    let mut other_work = placement(&attempt);
    other_work.work_revision = attempt.work_revision.saturating_add(1);
    refused(
        implementer.fleet_bind_session(other_work, 1),
        "fleet_session_bind",
    )?;
    assert!(implementer.fleet_state()?.sessions.sessions.is_empty());

    let session = implementer.fleet_bind_session(placement(&attempt), 1)?;
    assert_eq!(session.state, SessionState::Placing);
    refused(
        implementer.fleet_bind_session(placement(&attempt), 2),
        "fleet_session_bind",
    )?;
    refused(
        implementer.fleet_bind_session(placement(&attempt), 1),
        "fleet_session_bind",
    )?;
    let views = implementer.fleet_attempt_views(2)?;
    assert!(matches!(
        views.as_slice(),
        [view] if view.attempt == attempt.identity && view.held_by_fleet && view.state == AttemptViewState::Claimed
    ));

    let identity = session.identity.clone();
    refused(
        implementer.fleet_observe(&identity, Observation::Captured, 3),
        "fleet_session_transition",
    )?;
    refused(
        implementer.fleet_observe(&attempt.identity, Observation::Prompted, 3),
        "fleet_session_unknown",
    )?;
    implementer.fleet_observe(&identity, started()?, 3)?;
    refused(
        implementer.fleet_observe(&identity, started()?, 4),
        "fleet_session_transition",
    )?;
    implementer.fleet_observe(&identity, Observation::Prompted, 4)?;
    let working = implementer.fleet_observe(&identity, status(AgentStatus::Working), 5)?;
    assert_eq!(working.state, SessionState::Working);
    drop(implementer);

    let mut reopened = open(&path)?;
    let persisted = reopened
        .fleet_state()?
        .sessions
        .sessions
        .get(&identity)
        .cloned()
        .ok_or("session missing after reopen")?;
    assert_eq!(persisted, working);
    assert!(persisted.worked);
    assert_eq!(persisted.agent_name.as_deref(), Some("kiln-omp-1"));
    refused(
        reopened.fleet_observe(
            &identity,
            Observation::Handoff {
                commit: "ABC".to_string(),
            },
            6,
        ),
        "fleet_session_handoff",
    )?;
    let handed = reopened.fleet_observe(
        &identity,
        Observation::Handoff {
            commit: COMMIT.to_string(),
        },
        6,
    )?;
    assert_eq!(
        handed.state,
        SessionState::HandoffReceived {
            commit: COMMIT.to_string()
        }
    );
    reopened.fleet_observe(&identity, Observation::Captured, 7)?;
    drop(reopened);
    let captured = open(&path)?
        .fleet_state()?
        .sessions
        .sessions
        .get(&identity)
        .map(|session| session.state.clone());
    assert_eq!(captured, Some(SessionState::Captured));
    Ok(())
}

// r[verify kiln.fleet.decisions] r[verify kiln.fleet.decisions.scenario.answer]
#[test]
fn decisions_are_raised_answered_by_operators_and_delivered_once() -> TestResult {
    let directory = core(tempfile::tempdir())?;
    let path = directory.path().join("implementation.db");
    let (mut implementer, _, attempt) = claimed(&path, &["crates"], 0)?;
    let session = implementer.fleet_bind_session(placement(&attempt), 1)?;
    let identity = session.identity.clone();
    implementer.fleet_observe(&identity, started()?, 2)?;
    implementer.fleet_observe(&identity, Observation::Prompted, 3)?;
    implementer.fleet_observe(&identity, status(AgentStatus::Blocked), 4)?;

    refused(
        implementer.fleet_raise_decision(&attempt.identity, "Which crate?".to_string(), 5),
        "fleet_session_unknown",
    )?;
    let decision = implementer.fleet_raise_decision(&identity, "Which crate?".to_string(), 5)?;
    assert_eq!(decision.session, identity);
    assert_eq!(decision.attempt, attempt.identity);
    assert!(decision.is_open());
    refused(
        implementer.fleet_raise_decision(&identity, "Again?".to_string(), 6),
        "fleet_decision_open",
    )?;

    refused(
        implementer.fleet_answer_decision(
            &decision.identity,
            "kiln-core".to_string(),
            "aspen3",
            false,
            7,
        ),
        "fleet_decision_operator",
    )?;
    let prompt = implementer.fleet_answer_decision(
        &decision.identity,
        "kiln-core".to_string(),
        "britton-desktop",
        true,
        8,
    )?;
    assert!(prompt.contains("kiln-core"));
    refused(
        implementer.fleet_answer_decision(
            &decision.identity,
            "other".to_string(),
            "britton-desktop",
            true,
            9,
        ),
        "fleet_decision_answered",
    )?;
    drop(implementer);

    let mut reopened = open(&path)?;
    let answered = reopened
        .fleet_state()?
        .decisions
        .decisions
        .get(&decision.identity)
        .cloned()
        .ok_or("decision missing after reopen")?;
    assert_eq!(
        answered.answer.as_ref().map(|answer| answer.by.as_str()),
        Some("britton-desktop")
    );

    // The tick delivers the answer as a prompt exactly once.
    let policy = FleetPolicy::default();
    let effects = reopened.fleet_plan(10, 0, &policy)?;
    let delivery = FleetEffect::Prompt {
        session: identity.clone(),
        text: prompt,
    };
    assert!(effects.contains(&delivery), "{effects:?}");
    reopened.fleet_record_issued(10, 0, &effects)?;
    assert!(!reopened.fleet_plan(11, 0, &policy)?.contains(&delivery));

    // A decision for an ended session closes without delivery.
    reopened.fleet_observe(&identity, Observation::Prompted, 12)?;
    let open_decision =
        reopened.fleet_raise_decision(&identity, "Still blocked?".to_string(), 13)?;
    reopened.fleet_observe(
        &identity,
        Observation::Cancelled {
            cause: "operator".to_string(),
        },
        14,
    )?;
    let closed = reopened
        .fleet_state()?
        .decisions
        .decisions
        .get(&open_decision.identity)
        .cloned()
        .ok_or("decision missing")?;
    assert_eq!(closed.closed_ms, Some(14));
    refused(
        reopened.fleet_answer_decision(
            &open_decision.identity,
            "go on".to_string(),
            "britton-desktop",
            true,
            15,
        ),
        "fleet_decision_closed",
    )?;
    refused(
        reopened.fleet_raise_decision(&identity, "Anyone?".to_string(), 16),
        "fleet_decision_session",
    )?;
    Ok(())
}

// r[verify kiln.fleet.dispatch] r[verify kiln.fleet.dispatch.scenario.lifecycle]
#[test]
fn planned_effects_recorded_as_issued_are_not_repeated() -> TestResult {
    let directory = core(tempfile::tempdir())?;
    let path = directory.path().join("implementation.db");
    let policy = FleetPolicy::default();
    let mut implementer = open(&path)?;
    let work = work("onix/kiln#45", &["crates"])?;
    implementer.admit(&work, None)?;
    assert_eq!(implementer.fleet_ready_work(0)?, 1);
    assert_eq!(
        implementer.fleet_plan(0, 1, &policy)?,
        vec![FleetEffect::ClaimNext]
    );
    assert!(implementer.fleet_plan(0, 0, &policy)?.is_empty());
    refused(
        implementer.fleet_plan(
            0,
            1,
            &FleetPolicy {
                tick_ms: 0,
                ..policy
            },
        ),
        "fleet_policy",
    )?;

    let attempt = implementer
        .claim_next(0, DEFAULT_LEASE_MS)?
        .ok_or("work was not claimed")?;
    assert_eq!(implementer.fleet_ready_work(0)?, 0);
    let views = implementer.fleet_attempt_views(0)?;
    assert!(matches!(views.as_slice(), [view] if !view.held_by_fleet));
    assert!(implementer.fleet_plan(1_000, 1, &policy)?.is_empty());
    implementer.reserve_dispatch_plan(
        &work,
        &attempt.identity,
        attempt.fence,
        1,
        &interactive_plan(&work, &attempt)?,
    )?;
    let views = implementer.fleet_attempt_views(1_000)?;
    assert!(matches!(views.as_slice(), [view] if view.held_by_fleet));

    let place = FleetEffect::Place {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    let before = implementer.fleet_state()?;
    let effects = implementer.fleet_plan(1_000, 1, &policy)?;
    assert_eq!(effects, vec![place.clone()]);
    assert_eq!(
        implementer.fleet_state()?,
        before,
        "planning must not write"
    );
    implementer.fleet_record_issued(1_000, 1, &effects)?;
    assert!(implementer.fleet_plan(1_001, 1, &policy)?.is_empty());
    implementer.fleet_clear_placing(&attempt.identity)?;
    assert_eq!(implementer.fleet_plan(1_002, 1, &policy)?, vec![place]);
    implementer.fleet_record_issued(1_002, 1, &effects)?;

    let session = implementer.fleet_bind_session(placement(&attempt), 1_003)?;
    let identity = session.identity.clone();
    assert!(implementer.fleet_state()?.sessions.placing.is_empty());
    implementer.fleet_observe(&identity, started()?, 1_004)?;
    implementer.fleet_observe(&identity, Observation::Prompted, 1_005)?;
    implementer.fleet_observe(&identity, status(AgentStatus::Working), 2_000)?;
    implementer.fleet_observe(&identity, status(AgentStatus::Idle), 3_000)?;
    drop(implementer);

    let mut reopened = open(&path)?;
    let nudge_at = 3_000 + DEFAULT_NUDGE_AFTER_MS;
    let effects = reopened.fleet_plan(nudge_at, 1, &policy)?;
    assert!(
        matches!(effects.as_slice(), [FleetEffect::Prompt { session, .. }] if *session == identity),
        "{effects:?}"
    );
    reopened.fleet_record_issued(nudge_at, 1, &effects)?;
    assert!(reopened.fleet_plan(nudge_at, 1, &policy)?.is_empty());

    let renew_at = attempt.lease_until_ms - DEFAULT_RENEW_BEFORE_MS / 2;
    let renew = FleetEffect::Renew {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    // Without a fresh observation for 54 minutes, the stale guard stops
    // renewing and raises a decision instead.
    let stale_effects = reopened.fleet_plan(renew_at, 1, &policy)?;
    let stale_question = match stale_effects.as_slice() {
        [FleetEffect::RaiseDecision { session, question }]
            if *session == identity
                && kiln_core::fleet::is_stale_observation_question(question) =>
        {
            question.clone()
        }
        other => return Err(format!("expected stale decision, got {other:?}")),
    };
    let stale_decision = reopened.fleet_raise_decision(&identity, stale_question, renew_at)?;
    assert!(stale_decision.is_open());
    // A fresh observation closes the open stale decision automatically and
    // resumes lease renewal without operator action.
    reopened.fleet_observe(&identity, status(AgentStatus::Working), renew_at)?;
    assert!(
        reopened
            .fleet_state()?
            .decisions
            .open_for_session(&identity)
            .is_none()
    );
    let effects = reopened.fleet_plan(renew_at, 1, &policy)?;
    assert_eq!(effects, vec![renew.clone()]);
    reopened.fleet_record_issued_with_policy(renew_at, 1, &effects, &policy)?;
    drop(reopened);
    assert!(
        !open(&path)?
            .fleet_plan(renew_at, 1, &policy)?
            .contains(&renew)
    );
    Ok(())
}

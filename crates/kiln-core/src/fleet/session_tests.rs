use super::*;
use crate::fleet::decision::{answer, raise};
use alloc::vec;

const NOW: u64 = 10_000_000_000;
const MINUTE: u64 = 60 * 1_000;
const HOUR: u64 = 60 * MINUTE;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn placing(attempt: char, fence: u64) -> FleetSession {
    FleetSession::placing(
        Placement {
            attempt: content(attempt),
            fence,
            work: content('f'),
            work_revision: 1,
            machine: "aspen3".to_string(),
            kind: "omp".to_string(),
        },
        NOW - HOUR,
    )
    .unwrap_or_else(|_| unreachable!())
}

fn workspace() -> SessionWorkspace {
    SessionWorkspace::new(
        "kiln-attempt",
        "pane-1",
        "/srv/worktrees/attempt",
        "kiln/impl-ffffffffffff-111111111111",
        &"3".repeat(GIT_SHA1_HEX_BYTES),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn started() -> Observation {
    Observation::Started {
        agent_name: "kiln-omp-1".to_string(),
        workspace: workspace(),
    }
}

fn status(status: AgentStatus) -> Observation {
    Observation::Status { status }
}

/// Bind a session and apply observations at the given times.
fn session_with(
    ledger: &SessionLedger,
    attempt: char,
    observations: &[(Observation, u64)],
) -> (SessionLedger, ContentId) {
    let session = placing(attempt, 1);
    let identity = session.identity.clone();
    let mut ledger = bind(ledger, session).unwrap_or_else(|_| unreachable!());
    for (observation, at_ms) in observations {
        ledger = observe(&ledger, &identity, observation.clone(), *at_ms)
            .unwrap_or_else(|diagnostic| unreachable!("{diagnostic:?}"));
    }
    (ledger, identity)
}

fn working(at_ms: u64) -> Vec<(Observation, u64)> {
    vec![
        (started(), at_ms),
        (Observation::Prompted, at_ms),
        (status(AgentStatus::Working), at_ms),
    ]
}

fn view(attempt: char, state: AttemptViewState, lease_expires_ms: u64) -> AttemptView {
    AttemptView {
        attempt: content(attempt),
        fence: 1,
        state,
        lease_expires_ms,
        held_by_fleet: true,
    }
}

fn session_of<'a>(ledger: &'a SessionLedger, identity: &ContentId) -> &'a FleetSession {
    ledger
        .sessions
        .get(identity)
        .unwrap_or_else(|| unreachable!())
}

fn refused(ledger: &SessionLedger, identity: &ContentId, observation: Observation) -> String {
    observe(ledger, identity, observation, NOW)
        .err()
        .map(|diagnostic| diagnostic.code)
        .unwrap_or_else(|| unreachable!())
}

#[test]
// r[verify kiln.fleet.dispatch] r[verify kiln.fleet.dispatch.scenario.lifecycle]
fn lifecycle_effects_are_requested_once() {
    let ledger = SessionLedger::default();
    let (ledger, renewing) = session_with(&ledger, '1', &working(NOW - 30 * MINUTE));
    let mut blocked_steps = working(NOW - 30 * MINUTE);
    blocked_steps.push((status(AgentStatus::Blocked), NOW - 6 * MINUTE));
    let (ledger, blocked) = session_with(&ledger, '2', &blocked_steps);
    let mut idle_steps = working(NOW - 30 * MINUTE);
    idle_steps.push((status(AgentStatus::Idle), NOW - 11 * MINUTE));
    let (ledger, idle) = session_with(&ledger, '3', &idle_steps);
    let mut gone_steps = working(NOW - 30 * MINUTE);
    gone_steps.push((Observation::Gone, NOW - MINUTE));
    let (ledger, gone) = session_with(&ledger, '4', &gone_steps);
    let (ledger, finished) = session_with(&ledger, '5', &working(NOW - 30 * MINUTE));

    let attempts = [
        view('1', AttemptViewState::Executing, NOW + 5 * MINUTE),
        view('2', AttemptViewState::Executing, NOW + HOUR),
        view('3', AttemptViewState::Executing, NOW + HOUR),
        view('4', AttemptViewState::Executing, NOW + HOUR),
        view('5', AttemptViewState::Terminal, NOW + HOUR),
        view('6', AttemptViewState::Claimed, NOW + HOUR),
    ];
    let decisions = DecisionLedger::default();
    let input = TickInput {
        now_ms: NOW,
        ledger: &ledger,
        attempts: &attempts,
        free_slots: 2,
        ready_work: 3,
        decisions: &decisions,
    };
    let policy = FleetPolicy::default();
    let effects = plan_tick(&input, &policy);

    let expected = [
        FleetEffect::ClaimNext,
        FleetEffect::Place {
            attempt: content('6'),
            fence: 1,
        },
        FleetEffect::Renew {
            attempt: content('1'),
            fence: 1,
        },
        FleetEffect::Cancel {
            attempt: content('4'),
            fence: 1,
            cause: "agent gone before handoff".to_string(),
        },
        FleetEffect::Stop {
            session: gone.clone(),
        },
        FleetEffect::Stop {
            session: finished.clone(),
        },
    ];
    for effect in &expected {
        assert!(effects.contains(effect), "{effect:?} in {effects:?}");
    }
    assert!(effects.iter().any(|effect| matches!(
        effect,
        FleetEffect::RaiseDecision { session, question }
            if *session == blocked && question.contains("blocked") && question.contains("6 minutes")
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        FleetEffect::Prompt { session, text }
            if *session == idle && text.starts_with("Kiln reminder 1 of 2")
    )));
    assert_eq!(effects.len(), expected.len() + 2, "{effects:?}");
    // Planning is pure: the same input gives the same effects.
    assert_eq!(plan_tick(&input, &policy), effects);

    let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
    assert_eq!(session_of(&recorded, &idle).nudges, 1);
    assert!(session_of(&recorded, &blocked).decision_raised);
    assert_eq!(recorded.placing.get(&content('6')), Some(&1));
    let repeated = TickInput {
        ledger: &recorded,
        ..input
    };
    assert_eq!(plan_tick(&repeated, &policy), vec![FleetEffect::ClaimNext]);
    let later = TickInput {
        now_ms: NOW + 2 * MINUTE,
        ledger: &recorded,
        ..input
    };
    assert_eq!(plan_tick(&later, &policy), vec![FleetEffect::ClaimNext]);
    assert!(
        session_of(&recorded, &renewing)
            .renew_requested_for
            .is_some()
    );
}

#[test]
// r[verify kiln.fleet.dispatch]
fn renewal_follows_the_lease_and_live_sessions_only() {
    let (ledger, session) = session_with(&SessionLedger::default(), '1', &working(NOW - HOUR));
    let decisions = DecisionLedger::default();
    let policy = FleetPolicy::default();
    let tick = |ledger: &SessionLedger, lease: u64| {
        let attempts = [view('1', AttemptViewState::Executing, lease)];
        let input = TickInput {
            now_ms: NOW,
            ledger,
            attempts: &attempts,
            free_slots: 0,
            ready_work: 0,
            decisions: &decisions,
        };
        let effects = plan_tick(&input, &policy);
        let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
        (effects, recorded)
    };
    let (effects, _) = tick(&ledger, NOW + policy.renew_before_ms + 1);
    assert!(effects.is_empty());
    let (effects, recorded) = tick(&ledger, NOW + policy.renew_before_ms);
    assert_eq!(
        effects,
        vec![FleetEffect::Renew {
            attempt: content('1'),
            fence: 1
        }]
    );
    let (effects, _) = tick(&recorded, NOW + policy.renew_before_ms);
    assert!(effects.is_empty());
    // A renewed lease that is again near expiry is renewed again.
    let (effects, _) = tick(&recorded, NOW + MINUTE);
    assert_eq!(effects.len(), 1);
    // An expired lease means the attempt ended: stop, never renew.
    let (effects, _) = tick(&ledger, NOW);
    assert_eq!(effects, vec![FleetEffect::Stop { session }]);

    // A placing session is not renewed.
    let (placing_ledger, _) = session_with(&SessionLedger::default(), '1', &[]);
    let (effects, _) = tick(&placing_ledger, NOW + MINUTE);
    assert!(effects.is_empty());
}

#[test]
// r[verify kiln.fleet.dispatch]
fn nudges_are_capped_before_one_decision() {
    let policy = FleetPolicy::default();
    let decisions = DecisionLedger::default();
    let attempts = [view('1', AttemptViewState::Executing, NOW + 10 * HOUR)];
    let mut steps = working(NOW - 2 * HOUR);
    steps.push((status(AgentStatus::Idle), NOW - HOUR));
    let (mut ledger, session) = session_with(&SessionLedger::default(), '1', &steps);
    let mut now = NOW;
    for nudge in 1..=policy.max_nudges {
        let input = TickInput {
            now_ms: now,
            ledger: &ledger,
            attempts: &attempts,
            free_slots: 0,
            ready_work: 0,
            decisions: &decisions,
        };
        let effects = plan_tick(&input, &policy);
        assert!(matches!(
            effects.as_slice(),
            [FleetEffect::Prompt { text, .. }] if text.starts_with(&format!("Kiln reminder {nudge} of"))
        ));
        ledger = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
        ledger = observe(&ledger, &session, Observation::Prompted, now)
            .unwrap_or_else(|_| unreachable!());
        ledger = observe(&ledger, &session, status(AgentStatus::Done), now)
            .unwrap_or_else(|_| unreachable!());
        now += policy.nudge_after_ms;
    }
    let input = TickInput {
        now_ms: now,
        ledger: &ledger,
        attempts: &attempts,
        free_slots: 0,
        ready_work: 0,
        decisions: &decisions,
    };
    let effects = plan_tick(&input, &policy);
    assert!(matches!(
        effects.as_slice(),
        [FleetEffect::RaiseDecision { question, .. }] if question.contains("after 2 nudges")
    ));
    let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
    let again = TickInput {
        ledger: &recorded,
        now_ms: now + HOUR,
        ..input
    };
    assert!(plan_tick(&again, &policy).is_empty());
    // An open decision alone also suppresses a second one.
    let (open, _) =
        raise(&decisions, &session, &content('1'), "idle", now).unwrap_or_else(|_| unreachable!());
    let with_open = TickInput {
        decisions: &open,
        ..input
    };
    assert!(plan_tick(&with_open, &policy).is_empty());
    // Idle without any observed work is never nudged.
    let mut unworked = vec![(started(), NOW - HOUR), (Observation::Prompted, NOW - HOUR)];
    unworked.push((status(AgentStatus::Idle), NOW - HOUR));
    let (quiet, _) = session_with(&SessionLedger::default(), '1', &unworked);
    let quiet_input = TickInput {
        now_ms: NOW,
        ledger: &quiet,
        ..input
    };
    assert!(plan_tick(&quiet_input, &policy).is_empty());
}

#[test]
// r[verify kiln.fleet.dispatch] r[verify kiln.fleet.decisions]
fn answered_decisions_are_prompted_once() {
    let policy = FleetPolicy::default();
    let attempts = [view('1', AttemptViewState::Executing, NOW + 10 * HOUR)];
    let mut steps = working(NOW - HOUR);
    steps.push((status(AgentStatus::Blocked), NOW - HOUR));
    let (ledger, session) = session_with(&SessionLedger::default(), '1', &steps);
    let (decisions, decision) = raise(
        &DecisionLedger::default(),
        &session,
        &content('1'),
        "Which store?",
        NOW - 30 * MINUTE,
    )
    .unwrap_or_else(|_| unreachable!());
    let input = TickInput {
        now_ms: NOW,
        ledger: &ledger,
        attempts: &attempts,
        free_slots: 0,
        ready_work: 0,
        decisions: &decisions,
    };
    // Blocked past the window, but a decision is already open.
    assert!(plan_tick(&input, &policy).is_empty());

    let (answered, _) = answer(
        &decisions,
        &decision.identity,
        "SQLite",
        "britton-desktop",
        true,
        NOW,
    )
    .unwrap_or_else(|_| unreachable!());
    let input = TickInput {
        decisions: &answered,
        ..input
    };
    let effects = plan_tick(&input, &policy);
    assert!(matches!(
        effects.as_slice(),
        [FleetEffect::Prompt { session: target, text }]
            if *target == session && text.contains("SQLite") && text.contains("Which store?")
    ));
    let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        session_of(&recorded, &session).delivered_decision,
        Some(decision.identity.clone())
    );
    assert_eq!(session_of(&recorded, &session).nudges, 0);
    let repeated = TickInput {
        ledger: &recorded,
        ..input
    };
    assert!(plan_tick(&repeated, &policy).is_empty());
    // After the prompt lands and the agent works, nothing is re-delivered.
    let prompted =
        observe(&recorded, &session, Observation::Prompted, NOW).unwrap_or_else(|_| unreachable!());
    let worked = observe(&prompted, &session, status(AgentStatus::Working), NOW)
        .unwrap_or_else(|_| unreachable!());
    assert!(!session_of(&worked, &session).prompt_pending);
    let settled = TickInput {
        ledger: &worked,
        ..input
    };
    assert!(plan_tick(&settled, &policy).is_empty());
}

#[test]
// r[verify kiln.fleet.dispatch]
fn handoffs_are_captured_once_and_survive_a_gone_agent() {
    let policy = FleetPolicy::default();
    let decisions = DecisionLedger::default();
    let attempts = [view('1', AttemptViewState::Executing, NOW + 10 * HOUR)];
    let commit = "a".repeat(GIT_SHA1_HEX_BYTES);
    let mut steps = working(NOW - HOUR);
    steps.push((
        Observation::Handoff {
            commit: commit.clone(),
        },
        NOW - MINUTE,
    ));
    steps.push((Observation::Gone, NOW - MINUTE));
    let (ledger, session) = session_with(&SessionLedger::default(), '1', &steps);
    assert_eq!(
        session_of(&ledger, &session).state,
        SessionState::HandoffReceived {
            commit: commit.clone()
        }
    );
    let input = TickInput {
        now_ms: NOW,
        ledger: &ledger,
        attempts: &attempts,
        free_slots: 0,
        ready_work: 0,
        decisions: &decisions,
    };
    let effects = plan_tick(&input, &policy);
    assert_eq!(
        effects,
        vec![FleetEffect::Capture {
            session: session.clone(),
            commit: commit.clone()
        }]
    );
    let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
    assert!(
        plan_tick(
            &TickInput {
                ledger: &recorded,
                ..input
            },
            &policy
        )
        .is_empty()
    );
    // The same handoff again changes nothing; another commit is refused.
    assert_eq!(
        observe(
            &recorded,
            &session,
            Observation::Handoff {
                commit: commit.clone()
            },
            NOW
        ),
        Ok(recorded.clone())
    );
    assert_eq!(
        refused(
            &recorded,
            &session,
            Observation::Handoff {
                commit: "b".repeat(GIT_SHA1_HEX_BYTES)
            }
        ),
        "fleet_session_transition"
    );
    let captured =
        observe(&recorded, &session, Observation::Captured, NOW).unwrap_or_else(|_| unreachable!());
    assert!(!session_of(&captured, &session).capture_pending);
    let captured_input = TickInput {
        ledger: &captured,
        ..input
    };
    assert!(plan_tick(&captured_input, &policy).is_empty());
    // Once the attempt ends after verification, the agent is stopped.
    let ended = [view('1', AttemptViewState::Terminal, NOW + 10 * HOUR)];
    let ended_input = TickInput {
        attempts: &ended,
        ..captured_input
    };
    assert_eq!(
        plan_tick(&ended_input, &policy),
        vec![FleetEffect::Stop {
            session: session.clone()
        }]
    );

    // A failed capture cancels the attempt and stops the agent.
    let failed = observe(
        &recorded,
        &session,
        Observation::CaptureFailed {
            reason: "tree export refused".to_string(),
        },
        NOW,
    )
    .unwrap_or_else(|_| unreachable!());
    let failed_effects = plan_tick(
        &TickInput {
            ledger: &failed,
            ..input
        },
        &policy,
    );
    assert_eq!(
        failed_effects,
        vec![
            FleetEffect::Cancel {
                attempt: content('1'),
                fence: 1,
                cause: "capture failed: tree export refused".to_string(),
            },
            FleetEffect::Stop { session }
        ]
    );
}

#[test]
// r[verify kiln.fleet.dispatch]
fn start_failures_cancel_and_stop() {
    let policy = FleetPolicy::default();
    let decisions = DecisionLedger::default();
    let attempts = [view('1', AttemptViewState::Claimed, NOW + HOUR)];
    let (ledger, session) = session_with(
        &SessionLedger::default(),
        '1',
        &[(
            Observation::StartFailed {
                reason: "unsupported kind".to_string(),
            },
            NOW,
        )],
    );
    let input = TickInput {
        now_ms: NOW,
        ledger: &ledger,
        attempts: &attempts,
        free_slots: 0,
        ready_work: 0,
        decisions: &decisions,
    };
    let effects = plan_tick(&input, &policy);
    assert_eq!(
        effects,
        vec![
            FleetEffect::Cancel {
                attempt: content('1'),
                fence: 1,
                cause: "start failed: unsupported kind".to_string(),
            },
            FleetEffect::Stop {
                session: session.clone()
            }
        ]
    );
    let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
    assert!(session_of(&recorded, &session).cancel_requested);
    assert!(
        plan_tick(
            &TickInput {
                ledger: &recorded,
                ..input
            },
            &policy
        )
        .is_empty()
    );
}

#[test]
// r[verify kiln.fleet.dispatch]
fn claims_are_requested_only_within_free_slots() {
    let policy = FleetPolicy::default();
    let decisions = DecisionLedger::default();
    let (ledger, _) = session_with(&SessionLedger::default(), '1', &[(started(), NOW)]);
    let attempts = [view('1', AttemptViewState::Executing, NOW + HOUR)];
    let tick = |free_slots: u32, ready_work: u32| {
        plan_tick(
            &TickInput {
                now_ms: NOW,
                ledger: &ledger,
                attempts: &attempts,
                free_slots,
                ready_work,
                decisions: &decisions,
            },
            &policy,
        )
    };
    assert!(tick(1, 5).is_empty());
    assert_eq!(tick(2, 5), vec![FleetEffect::ClaimNext]);
    assert!(tick(2, 0).is_empty());
    assert!(tick(0, 5).is_empty());

    // Attempts held elsewhere or ended are not placed.
    let mut foreign = view('2', AttemptViewState::Claimed, NOW + HOUR);
    foreign.held_by_fleet = false;
    let others = [
        foreign,
        view('3', AttemptViewState::Terminal, NOW + HOUR),
        view('4', AttemptViewState::Verifying, NOW + HOUR),
        view('5', AttemptViewState::Claimed, NOW),
    ];
    let empty = SessionLedger::default();
    assert!(
        plan_tick(
            &TickInput {
                now_ms: NOW,
                ledger: &empty,
                attempts: &others,
                free_slots: 0,
                ready_work: 0,
                decisions: &decisions,
            },
            &policy
        )
        .is_empty()
    );
}

#[test]
// r[verify kiln.fleet.dispatch]
fn placement_markers_clear_on_bind_or_failure() {
    let policy = FleetPolicy::default();
    let decisions = DecisionLedger::default();
    let attempts = [view('1', AttemptViewState::Claimed, NOW + HOUR)];
    let empty = SessionLedger::default();
    let input = TickInput {
        now_ms: NOW,
        ledger: &empty,
        attempts: &attempts,
        free_slots: 0,
        ready_work: 0,
        decisions: &decisions,
    };
    let effects = plan_tick(&input, &policy);
    assert_eq!(
        effects,
        vec![FleetEffect::Place {
            attempt: content('1'),
            fence: 1
        }]
    );
    let recorded = record_issued(&input, &effects).unwrap_or_else(|_| unreachable!());
    let cleared = clear_placing(&recorded, &content('1'));
    assert_eq!(
        plan_tick(
            &TickInput {
                ledger: &cleared,
                ..input
            },
            &policy
        ),
        effects
    );
    let bound = bind(&recorded, placing('1', 1)).unwrap_or_else(|_| unreachable!());
    assert!(bound.placing.is_empty());
    // Markers for attempts that are no longer placeable are dropped.
    let gone = [view('1', AttemptViewState::Terminal, NOW + HOUR)];
    let pruned = record_issued(
        &TickInput {
            ledger: &recorded,
            attempts: &gone,
            ..input
        },
        &[],
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(pruned.placing.is_empty());
}

#[test]
// r[verify kiln.fleet.dispatch]
fn binding_is_idempotent_and_refuses_conflicts() {
    let session = placing('1', 1);
    let ledger =
        bind(&SessionLedger::default(), session.clone()).unwrap_or_else(|_| unreachable!());
    assert_eq!(bind(&ledger, session.clone()), Ok(ledger.clone()));
    let code = |result: CoreResult<SessionLedger>| result.err().map(|diagnostic| diagnostic.code);

    let mut moved = session.clone();
    moved.machine = "aspen1".to_string();
    assert_eq!(
        code(bind(&ledger, moved)),
        Some("fleet_session_bind".to_string())
    );
    let mut forged = placing('1', 2);
    forged.identity = content('9');
    assert_eq!(
        code(bind(&SessionLedger::default(), forged)),
        Some("fleet_session_bind".to_string())
    );
    assert_eq!(
        code(bind(&ledger, placing('1', 2))),
        Some("fleet_session_bind".to_string())
    );
    let mut advanced = placing('2', 1);
    advanced.state = SessionState::Working;
    assert_eq!(
        code(bind(&ledger, advanced)),
        Some("fleet_session_bind".to_string())
    );
    let mut bad_kind = placing('2', 1);
    bad_kind.kind = "o m p".to_string();
    assert!(bind(&ledger, bad_kind).is_err());

    // A new fence may bind once the earlier session ended.
    let ended = observe(&ledger, &session.identity, Observation::Gone, NOW)
        .unwrap_or_else(|_| unreachable!());
    assert!(bind(&ended, placing('1', 2)).is_ok());

    let mut full = SessionLedger::default();
    for index in 0..MAX_LIVE_FLEET_SESSIONS {
        let attempt =
            ContentId::from_blake3_hex(format!("{index:064x}")).unwrap_or_else(|_| unreachable!());
        let candidate = FleetSession::placing(
            Placement {
                attempt,
                fence: 1,
                work: content('f'),
                work_revision: 1,
                machine: "aspen3".to_string(),
                kind: "omp".to_string(),
            },
            NOW,
        )
        .unwrap_or_else(|_| unreachable!());
        full = bind(&full, candidate).unwrap_or_else(|_| unreachable!());
    }
    assert_eq!(
        code(bind(&full, placing('e', 1))),
        Some("fleet_session_limit".to_string())
    );
}

#[test]
// r[verify kiln.fleet.dispatch]
fn senseless_transitions_are_refused() {
    let (placing_ledger, placing_id) = session_with(&SessionLedger::default(), '1', &[]);
    assert_eq!(
        refused(&placing_ledger, &placing_id, Observation::Prompted),
        "fleet_session_transition"
    );
    assert_eq!(
        refused(
            &placing_ledger,
            &placing_id,
            Observation::Handoff {
                commit: "a".repeat(GIT_SHA1_HEX_BYTES)
            }
        ),
        "fleet_session_transition"
    );
    assert_eq!(
        refused(&placing_ledger, &content('9'), Observation::Gone),
        "fleet_session_unknown"
    );
    let (started_ledger, started_id) =
        session_with(&SessionLedger::default(), '1', &[(started(), NOW)]);
    assert_eq!(
        refused(&started_ledger, &started_id, started()),
        "fleet_session_transition"
    );
    assert_eq!(
        refused(&started_ledger, &started_id, Observation::Captured),
        "fleet_session_transition"
    );

    let (working_ledger, working_id) = session_with(&SessionLedger::default(), '1', &working(NOW));
    let upper = "A".repeat(GIT_SHA1_HEX_BYTES);
    let long = "a".repeat(GIT_SHA1_HEX_BYTES + 1);
    for commit in ["xyz", upper.as_str(), long.as_str()] {
        assert_eq!(
            refused(
                &working_ledger,
                &working_id,
                Observation::Handoff {
                    commit: commit.to_string()
                }
            ),
            "fleet_session_handoff"
        );
    }
    assert_eq!(
        refused(
            &working_ledger,
            &working_id,
            Observation::StartFailed {
                reason: "late".to_string()
            }
        ),
        "fleet_session_transition"
    );
    assert_eq!(
        refused(
            &working_ledger,
            &working_id,
            Observation::CaptureFailed {
                reason: "x".to_string()
            }
        ),
        "fleet_session_transition"
    );
    assert_eq!(
        refused(
            &working_ledger,
            &working_id,
            Observation::Cancelled {
                cause: String::new()
            }
        ),
        "fleet_session_cause"
    );

    let handoff = Observation::Handoff {
        commit: "c".repeat(GIT_SHA256_HEX_BYTES),
    };
    let ended = [
        Observation::Gone,
        Observation::Cancelled {
            cause: "operator".to_string(),
        },
    ];
    for end in ended {
        let ended_ledger =
            observe(&working_ledger, &working_id, end, NOW).unwrap_or_else(|_| unreachable!());
        assert!(session_of(&ended_ledger, &working_id).state.is_ended());
        assert_eq!(
            refused(&ended_ledger, &working_id, handoff.clone()),
            "fleet_session_transition"
        );
        assert_eq!(
            refused(&ended_ledger, &working_id, Observation::Prompted),
            "fleet_session_transition"
        );
        // Status and Gone after the end are recorded as seen only.
        let seen = observe(
            &ended_ledger,
            &working_id,
            status(AgentStatus::Working),
            NOW + 1,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            session_of(&seen, &working_id).state,
            session_of(&ended_ledger, &working_id).state
        );
    }
    let captured = [(handoff.clone(), NOW), (Observation::Captured, NOW)];
    let mut steps = working(NOW);
    steps.extend(captured);
    let (captured_ledger, captured_id) = session_with(&SessionLedger::default(), '1', &steps);
    assert_eq!(
        refused(&captured_ledger, &captured_id, handoff),
        "fleet_session_transition"
    );
    assert_eq!(
        refused(
            &captured_ledger,
            &captured_id,
            Observation::Cancelled {
                cause: "late".to_string()
            }
        ),
        "fleet_session_transition"
    );
    assert_eq!(
        observe(&captured_ledger, &captured_id, Observation::Captured, NOW),
        Ok(captured_ledger.clone())
    );

    let mut tampered = workspace();
    tampered.branch = "other".to_string();
    assert_eq!(
        refused(
            &placing_ledger,
            &placing_id,
            Observation::Started {
                agent_name: "kiln-omp-1".to_string(),
                workspace: tampered,
            }
        ),
        "fleet_session_workspace"
    );
    assert_eq!(
        refused(
            &placing_ledger,
            &placing_id,
            Observation::Started {
                agent_name: "two words".to_string(),
                workspace: workspace(),
            }
        ),
        "fleet_session"
    );
}

#[test]
// r[verify kiln.fleet.dispatch]
fn status_reports_never_invent_liveness() {
    let (ledger, session) = session_with(&SessionLedger::default(), '1', &working(NOW));
    let unknown = observe(&ledger, &session, status(AgentStatus::Unknown), NOW + 5)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(session_of(&unknown, &session).state, SessionState::Working);
    assert_eq!(session_of(&unknown, &session).observed_ms, NOW + 5);
    let blocked = observe(&ledger, &session, status(AgentStatus::Blocked), NOW + 10)
        .unwrap_or_else(|_| unreachable!());
    let still = observe(&blocked, &session, status(AgentStatus::Blocked), NOW + 20)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        session_of(&still, &session).state,
        SessionState::Blocked { since_ms: NOW + 10 }
    );
    // A starting agent's status does not advance it before its prompt.
    let (starting, starting_id) = session_with(&SessionLedger::default(), '2', &[(started(), NOW)]);
    let seen = observe(&starting, &starting_id, status(AgentStatus::Working), NOW)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        session_of(&seen, &starting_id).state,
        SessionState::Starting
    );
    assert!(!session_of(&seen, &starting_id).worked);
}

#[test]
// r[verify kiln.fleet.dispatch]
fn record_issued_refuses_unknown_targets() {
    let decisions = DecisionLedger::default();
    let ledger = SessionLedger::default();
    let input = TickInput {
        now_ms: NOW,
        ledger: &ledger,
        attempts: &[],
        free_slots: 0,
        ready_work: 0,
        decisions: &decisions,
    };
    for effect in [
        FleetEffect::Stop {
            session: content('1'),
        },
        FleetEffect::Renew {
            attempt: content('1'),
            fence: 1,
        },
        FleetEffect::Capture {
            session: content('1'),
            commit: "a".repeat(GIT_SHA1_HEX_BYTES),
        },
    ] {
        assert_eq!(
            record_issued(&input, &[effect])
                .err()
                .map(|diagnostic| diagnostic.code),
            Some("fleet_session_effect".to_string())
        );
    }
}

#[test]
// r[verify kiln.fleet.dispatch]
fn policy_defaults_and_bounds() {
    let policy = FleetPolicy::default();
    assert_eq!(policy.tick_ms, 15_000);
    assert_eq!(policy.renew_before_ms, 10 * MINUTE);
    assert_eq!(policy.blocked_decision_after_ms, 5 * MINUTE);
    assert_eq!(policy.nudge_after_ms, 10 * MINUTE);
    assert_eq!(policy.max_nudges, 2);
    assert_eq!(policy.attempt_lease_ms, HOUR);
    assert!(policy.validate().is_ok());
    for invalid in [
        FleetPolicy {
            tick_ms: MIN_TICK_MS - 1,
            ..policy
        },
        FleetPolicy {
            renew_before_ms: policy.attempt_lease_ms,
            ..policy
        },
        FleetPolicy {
            attempt_lease_ms: MAX_LEASE_MS + 1,
            ..policy
        },
        FleetPolicy {
            max_nudges: MAX_NUDGES_BOUND + 1,
            ..policy
        },
        FleetPolicy {
            nudge_after_ms: 0,
            ..policy
        },
    ] {
        assert!(invalid.validate().is_err(), "{invalid:?}");
    }
}

#[test]
// r[verify kiln.fleet.dispatch]
fn ledgers_round_trip_and_phases_map() {
    let mut steps = working(NOW);
    steps.push((status(AgentStatus::Blocked), NOW));
    let (ledger, _) = session_with(&SessionLedger::default(), '1', &steps);
    let encoded = serde_json::to_string(&ledger).unwrap_or_else(|_| unreachable!());
    let decoded: SessionLedger = serde_json::from_str(&encoded).unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded, ledger);
    assert_eq!(
        AttemptViewState::from_phase(AttemptPhase::Verifying),
        AttemptViewState::Verifying
    );
    assert_eq!(
        AttemptViewState::from_phase(AttemptPhase::Superseded),
        AttemptViewState::Terminal
    );
    assert_eq!(
        session_identity(&content('1'), 1),
        Ok(placing('1', 1).identity)
    );
    assert_ne!(
        session_identity(&content('1'), 2),
        Ok(placing('1', 1).identity)
    );
}

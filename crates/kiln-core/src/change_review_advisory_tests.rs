//! Advisory approvals: separate from votes, bound to the exact current
//! revision, and never able to change any submit checklist item.
use super::*;
use alloc::format;
use alloc::vec;

const OWNER: &str = "alice";
const REVIEWERS: [&str; 2] = ["bob", "carol"];
/// Read principals outside the policy; they may advise but never vote.
const READERS: [&str; 2] = ["dave", "erin"];

fn policy(ownership: bool, minimum_approvals: u32) -> Policy {
    let mut rules = Vec::new();
    if ownership {
        rules.push(OwnershipRule {
            id: "core".into(),
            paths: vec!["src/**".into()],
            owners: ["carol".into()].into_iter().collect(),
            teams: BTreeSet::new(),
            target_refs: BTreeSet::new(),
            required_evidence: Vec::new(),
            approval: BTreeMap::new(),
            project: None,
            interface: Vec::new(),
        });
    }
    Policy {
        id: "policy-1".into(),
        eligible_reviewers: REVIEWERS.iter().map(|p| String::from(*p)).collect(),
        submitters: [OWNER.into()].into_iter().collect(),
        minimum_approvals,
        require_ci: true,
        ownership: OwnershipPolicy {
            teams: BTreeMap::new(),
            rules,
            classes: Vec::new(),
            dependents: BTreeMap::new(),
        },
        optional_ci: Vec::new(),
        agent_votes: AgentVotes::Advisory,

        required_checks: Vec::new(),
    }
}

fn ci(success: bool) -> CiFact {
    CiFact {
        project: "project".into(),
        repository: "repository".into(),
        revision: "commit-1".into(),
        run_id: "run-1".into(),
        ordinal: 1,
        success,
        full_run: true,
        clean: true,
        scope_identity: None,
        terminal: true,
        untrusted: None,

        checks: Vec::new(),
    }
}

fn facts(policy: Policy, changed_files: bool) -> Facts {
    Facts {
        policy: Some(policy),
        source: Some(SourceFact {
            project: "project".into(),
            repository: "repository".into(),
            target_ref: "refs/heads/main".into(),
            commit: "commit-1".into(),
            base: "base".into(),
            current_target: "base".into(),
            immutable: true,
            target_contains_commit: false,
            target_is_ancestor: true,
            target_ancestors: BTreeSet::new(),
            changed_files: changed_files.then(|| ChangedFiles {
                files: vec![ChangedFile {
                    path: "src/lib.rs".into(),
                    old_path: None,
                }],
                complete: true,
            }),
        }),
        ci: vec![ci(true)],
        anchors: vec![Anchor {
            revision: "r1".into(),
            base_commit: "base".into(),
            file: "src/lib.rs".into(),
            side: Side::Revision,
            start_line: 1,
            end_line: 1,
        }],
        ..Facts::default()
    }
}

fn revision(id: &str, number: u32, commit: &str, author: &str) -> Revision {
    Revision {
        id: id.into(),
        number,
        commit: commit.into(),
        base: "base".into(),
        files: vec!["src/lib.rs".into()],
        author: author.into(),
        created_ms: 0,
        authorship: Authorship::Undeclared,
        submitted_by: None,
    }
}

fn send(
    state: &mut ReviewState,
    facts: &mut Facts,
    actor: &str,
    mut command: Command,
) -> ReviewResult<()> {
    facts.now_ms += 1;
    facts.event_id = format!("event-{}", facts.now_ms);
    if let Command::Create { revision, .. } | Command::Upload { revision, .. } = &mut command {
        revision.created_ms = facts.now_ms;
    }
    apply(state, actor, command, facts)
}

fn ok(result: ReviewResult<()>) {
    if let Err(error) = result {
        unreachable!("fixture command rejected: {error}");
    }
}

fn rejected(result: ReviewResult<()>, code: &str) {
    match result {
        Err(error) => assert_eq!(error.code, code),
        Ok(()) => unreachable!("expected {code}"),
    }
}

fn change(state: &ReviewState) -> &Change {
    state
        .changes
        .get("c")
        .unwrap_or_else(|| unreachable!("fixture change"))
}

fn current(state: &ReviewState) -> String {
    change(state)
        .current()
        .map(|revision| revision.id.clone())
        .unwrap_or_else(|_| unreachable!("fixture revision"))
}

fn setup(facts: &mut Facts) -> ReviewState {
    let mut state = ReviewState::default();
    ok(send(
        &mut state,
        facts,
        OWNER,
        Command::Create {
            id: "c".into(),
            project: "project".into(),
            repository: "repository".into(),
            target_ref: "refs/heads/main".into(),
            title: "Review".into(),
            description: "Fix".into(),
            wip: false,
            revision: revision("r1", 1, "commit-1", OWNER),
        },
    ));
    for principal in REVIEWERS {
        let rev = current(&state);
        ok(send(
            &mut state,
            facts,
            OWNER,
            Command::SetReviewer {
                change: "c".into(),
                expected_revision: rev.clone(),
                principal: principal.into(),
                role: Some(ReviewerRole::Reviewer),
            },
        ));
    }
    state
}

/// Upload revision r2 authored by `author`, observing its exact source commit.
fn upload(state: &mut ReviewState, facts: &mut Facts, author: &str) {
    if let Some(source) = facts.source.as_mut() {
        source.commit = "commit-2".into();
    }
    ok(send(
        state,
        facts,
        OWNER,
        Command::Upload {
            change: "c".into(),
            expected_revision: "r1".into(),
            revision: revision("r2", 2, "commit-2", author),
        },
    ));
}

fn advise(
    state: &mut ReviewState,
    facts: &mut Facts,
    actor: &str,
    approve: bool,
) -> ReviewResult<()> {
    let expected_revision = current(state);
    let command = if approve {
        Command::AdvisoryApprove {
            change: "c".into(),
            expected_revision,
        }
    } else {
        Command::AdvisoryWithdraw {
            change: "c".into(),
            expected_revision,
        }
    };
    send(state, facts, actor, command)
}

fn vote(state: &mut ReviewState, facts: &mut Facts, actor: &str, value: VoteValue) {
    let expected_revision = current(state);
    ok(send(
        state,
        facts,
        actor,
        Command::Vote {
            change: "c".into(),
            expected_revision,
            value,
        },
    ));
}

const VOTES: [Option<VoteValue>; 5] = [
    None,
    Some(VoteValue::Approve),
    Some(VoteValue::NeedsRevision),
    Some(VoteValue::Veto),
    Some(VoteValue::Abstain),
];

// r[verify kiln.review.advisory.scenario.checklist]
// r[verify kiln.review.advisory.authority]
#[test]
fn advisory_approvals_never_change_any_checklist_item() {
    let mut cases = 0usize;
    for ownership in [false, true] {
        for minimum in [1, 2] {
            for changed_files in [false, true] {
                for bob in VOTES {
                    for carol in VOTES {
                        for wip in [false, true] {
                            for blocking in [false, true] {
                                for ci_state in [None, Some(false), Some(true)] {
                                    cases += 1;
                                    let mut facts = facts(policy(ownership, minimum), true);
                                    let mut state = setup(&mut facts);
                                    if !changed_files {
                                        if let Some(source) = facts.source.as_mut() {
                                            source.changed_files = None;
                                        }
                                    }
                                    if let Some(value) = bob {
                                        vote(&mut state, &mut facts, "bob", value);
                                    }
                                    if let Some(value) = carol {
                                        vote(&mut state, &mut facts, "carol", value);
                                    }
                                    if wip {
                                        let rev = current(&state);
                                        ok(send(
                                            &mut state,
                                            &mut facts,
                                            OWNER,
                                            Command::SetWip {
                                                change: "c".into(),
                                                expected_revision: rev.clone(),
                                                wip: true,
                                            },
                                        ));
                                    }
                                    if blocking {
                                        let anchor = facts
                                            .anchors
                                            .first()
                                            .cloned()
                                            .unwrap_or_else(|| unreachable!());
                                        let rev = current(&state);
                                        ok(send(
                                            &mut state,
                                            &mut facts,
                                            "bob",
                                            Command::StartThread {
                                                change: "c".into(),
                                                expected_revision: rev.clone(),
                                                id: "t1".into(),
                                                comment_id: "t1-c1".into(),
                                                anchor,
                                                body: "Please fix".into(),
                                                blocking: true,
                                            },
                                        ));
                                    }
                                    facts.ci = ci_state.map(ci).into_iter().collect();
                                    let before = checklist(&state, "c", &facts);
                                    let votes = change(&state).votes.clone();
                                    let candidate = change(&state).candidate.clone();
                                    let attention = change(&state).attention.clone();
                                    let inbox: Vec<_> = [OWNER, "bob", "carol"]
                                        .iter()
                                        .map(|p| project(&state, p).inbox.len())
                                        .collect();
                                    // Every principal allowed to advise does so.
                                    for advisor in REVIEWERS.iter().chain(READERS.iter()) {
                                        ok(advise(&mut state, &mut facts, advisor, true));
                                    }
                                    assert_eq!(current_advisory(change(&state)).count(), 4);
                                    assert_eq!(checklist(&state, "c", &facts), before);
                                    assert_eq!(change(&state).votes, votes);
                                    assert_eq!(change(&state).candidate, candidate);
                                    assert_eq!(change(&state).attention, attention);
                                    let after: Vec<_> = [OWNER, "bob", "carol"]
                                        .iter()
                                        .map(|p| project(&state, p).inbox.len())
                                        .collect();
                                    assert_eq!(after, inbox);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 2 * 2 * 2 * 5 * 5 * 2 * 2 * 3);
}

// r[verify kiln.review.advisory.authority]
#[test]
fn advisory_approval_never_satisfies_approvals_or_ownership_alone() {
    let mut facts = facts(policy(true, 1), true);
    let mut state = setup(&mut facts);
    for advisor in REVIEWERS.iter().chain(READERS.iter()) {
        ok(advise(&mut state, &mut facts, advisor, true));
    }
    let checks = checklist(&state, "c", &facts).unwrap_or_else(|_| unreachable!("checklist"));
    assert!(!checks.ready);
    for key in ["approvals", "owner:core"] {
        assert!(
            checks
                .items
                .iter()
                .any(|item| item.key == key && !item.satisfied),
            "{key} must stay unmet"
        );
    }
    let rev = current(&state);
    rejected(
        send(
            &mut state,
            &mut facts,
            OWNER,
            Command::PrepareSubmit {
                change: "c".into(),
                expected_revision: rev.clone(),
                candidate_id: "candidate-1".into(),
            },
        ),
        "submit_blocked",
    );
    // The owner's eligible vote, not the advisory record, satisfies both items.
    vote(&mut state, &mut facts, "carol", VoteValue::Approve);
    let checks = checklist(&state, "c", &facts).unwrap_or_else(|_| unreachable!("checklist"));
    assert!(checks.ready);
    // A prepared candidate is neither cleared nor changed by advisory activity.
    let rev = current(&state);
    ok(send(
        &mut state,
        &mut facts,
        OWNER,
        Command::PrepareSubmit {
            change: "c".into(),
            expected_revision: rev.clone(),
            candidate_id: "candidate-1".into(),
        },
    ));
    let candidate = change(&state).candidate.clone();
    assert!(candidate.is_some());
    ok(advise(&mut state, &mut facts, "dave", false));
    ok(advise(&mut state, &mut facts, "dave", true));
    assert_eq!(change(&state).candidate, candidate);
}

// r[verify kiln.review.advisory.scenario.votes]
#[test]
fn eligible_votes_behave_exactly_as_without_advisory_approvals() {
    let script: [(&str, VoteValue); 5] = [
        ("bob", VoteValue::NeedsRevision),
        ("carol", VoteValue::Approve),
        ("bob", VoteValue::Approve),
        ("carol", VoteValue::Veto),
        ("carol", VoteValue::Approve),
    ];
    let run = |with_advisory: bool| {
        let mut facts = facts(policy(true, 2), true);
        let mut state = setup(&mut facts);
        let mut checks = Vec::new();
        for (actor, value) in script {
            if with_advisory {
                for advisor in READERS {
                    ok(advise(&mut state, &mut facts, advisor, true));
                }
            }
            vote(&mut state, &mut facts, actor, value);
            checks.push(checklist(&state, "c", &facts));
        }
        let votes: Vec<_> = change(&state)
            .votes
            .iter()
            .map(|v| (v.actor.clone(), v.revision.clone(), v.value))
            .collect();
        (votes, checks)
    };
    assert_eq!(run(true), run(false));
    // Advisory records never make a reader an eligible voter.
    let mut facts = facts(policy(false, 1), false);
    let mut state = setup(&mut facts);
    ok(advise(&mut state, &mut facts, "dave", true));
    let rev = current(&state);
    rejected(
        send(
            &mut state,
            &mut facts,
            "dave",
            Command::Vote {
                change: "c".into(),
                expected_revision: rev.clone(),
                value: VoteValue::Approve,
            },
        ),
        "ineligible_vote",
    );
}

// r[verify kiln.review.advisory.scenario.lifecycle]
#[test]
fn advisory_binds_the_current_revision_and_becomes_historical() {
    let mut facts = facts(policy(false, 1), false);
    let mut state = setup(&mut facts);
    ok(advise(&mut state, &mut facts, "dave", true));
    // Re-casting replaces the actor's record of the same revision.
    ok(advise(&mut state, &mut facts, "dave", true));
    assert_eq!(change(&state).advisory.len(), 1);
    let recorded = change(&state)
        .advisory
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(recorded.revision, "r1");
    assert_eq!(recorded.created_ms, facts.now_ms);
    rejected(
        send(
            &mut state,
            &mut facts,
            "erin",
            Command::AdvisoryApprove {
                change: "c".into(),
                expected_revision: "r0".into(),
            },
        ),
        "stale_revision",
    );
    ok(advise(&mut state, &mut facts, "erin", true));
    ok(advise(&mut state, &mut facts, "erin", false));
    rejected(
        advise(&mut state, &mut facts, "erin", false),
        "advisory_missing",
    );
    upload(&mut state, &mut facts, OWNER);
    assert_eq!(current_advisory(change(&state)).count(), 0);
    assert_eq!(change(&state).advisory.len(), 1, "history is retained");
    rejected(
        advise(&mut state, &mut facts, "dave", false),
        "advisory_missing",
    );
    let query = |advisory| SearchQuery {
        kind: SearchKind::All,
        text: String::new(),
        status: None,
        advisory: Some(advisory),
        agent: None,
        agent_harness: None,
        agent_model: None,
    };
    assert!(search(&state, "dave", &query(true)).is_empty());
    assert_eq!(search(&state, "dave", &query(false)).len(), 1);
    ok(advise(&mut state, &mut facts, "dave", true));
    assert_eq!(search(&state, "dave", &query(true)).len(), 1);
    assert!(search(&state, "dave", &query(false)).is_empty());
}

// r[verify kiln.review.advisory.scenario.forbidden]
#[test]
fn owners_authors_and_closed_changes_cannot_advise() {
    let mut facts = facts(policy(false, 1), false);
    let mut state = setup(&mut facts);
    rejected(
        advise(&mut state, &mut facts, OWNER, true),
        "advisory_forbidden",
    );
    // An author of any revision (for example a provider-completed revision
    // authored by another principal) is an author of the change.
    upload(&mut state, &mut facts, OWNER);
    if let Some(change) = state.changes.get_mut("c") {
        if let Some(first) = change.revisions.first_mut() {
            first.author = "frank".into();
        }
    }
    rejected(
        advise(&mut state, &mut facts, "frank", true),
        "advisory_forbidden",
    );
    rejected(
        send(
            &mut state,
            &mut facts,
            "dave",
            Command::AdvisoryApprove {
                change: "missing".into(),
                expected_revision: "r2".into(),
            },
        ),
        "not_found",
    );
    let rev = current(&state);
    ok(send(
        &mut state,
        &mut facts,
        OWNER,
        Command::Abandon {
            change: "c".into(),
            expected_revision: rev.clone(),
        },
    ));
    rejected(advise(&mut state, &mut facts, "dave", true), "closed");
    assert!(change(&state).advisory.is_empty());
}

#[test]
fn advisory_approvals_are_bounded_per_revision() {
    let mut facts = facts(policy(false, 1), false);
    let mut state = setup(&mut facts);
    for index in 0..MAX_ADVISORY_PER_REVISION {
        ok(advise(
            &mut state,
            &mut facts,
            &format!("reader-{index}"),
            true,
        ));
    }
    rejected(
        advise(&mut state, &mut facts, "one-more", true),
        "advisory_bound",
    );
    // An existing advisor may still refresh within the bound.
    ok(advise(&mut state, &mut facts, "reader-0", true));
    assert_eq!(
        current_advisory(change(&state)).count(),
        MAX_ADVISORY_PER_REVISION
    );
}

// r[verify kiln.review.advisory.legacy]
#[test]
fn pre_advisory_documents_replay_byte_identically() {
    let mut facts = facts(policy(false, 1), false);
    let state = setup(&mut facts);
    let encoded = serde_json::to_string(change(&state)).unwrap_or_default();
    assert!(!encoded.contains("advisory"));
    let decoded: Change = serde_json::from_str(&encoded).unwrap_or_else(|_| unreachable!("change"));
    assert_eq!(serde_json::to_string(&decoded).unwrap_or_default(), encoded);
    let saved =
        r#"{"id":"v","name":"Mine","query":{"kind":"awaiting_review","text":"x","status":"open"}}"#;
    let query: SavedQuery = serde_json::from_str(saved).unwrap_or_else(|_| unreachable!("query"));
    assert_eq!(query.query.advisory, None);
    assert_eq!(serde_json::to_string(&query).unwrap_or_default(), saved);
    let command = r#"{"op":"vote","change":"c","expected_revision":"r1","value":"approve"}"#;
    let parsed: Command = serde_json::from_str(command).unwrap_or_else(|_| unreachable!("command"));
    assert_eq!(serde_json::to_string(&parsed).unwrap_or_default(), command);
    // The new command is a distinct operation, never a vote value.
    let advisory = r#"{"op":"advisory_approve","change":"c","expected_revision":"r1"}"#;
    assert!(matches!(
        serde_json::from_str::<Command>(advisory),
        Ok(Command::AdvisoryApprove { .. })
    ));
    assert!(serde_json::from_str::<VoteValue>(r#""advisory_approve""#).is_err());
    assert!(serde_json::from_str::<VoteValue>(r#""advisory""#).is_err());
}

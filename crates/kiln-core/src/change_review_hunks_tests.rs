//! Hunk viewed markers: mark-all, per-hunk toggles, carry-forward on upload and
//! byte-identical replay of documents written before hunk markers.
use super::*;
use alloc::string::ToString;
use alloc::{format, vec};

fn base_facts() -> Facts {
    Facts {
        now_ms: 0,
        event_id: String::new(),
        policy: Some(Policy {
            id: "policy-1".into(),
            eligible_reviewers: ["bob".into(), "carol".into()].into_iter().collect(),
            submitters: ["alice".into()].into_iter().collect(),
            minimum_approvals: 1,
            require_ci: false,
            ownership: OwnershipPolicy::default(),
            optional_ci: Vec::new(),
            agent_votes: AgentVotes::Advisory,
        }),
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
            changed_files: None,
        }),
        ci: Vec::new(),
        anchors: Vec::new(),
        integration: None,
        ci_required: None,
        hunks: None,
        agents: BTreeSet::new(),
    }
}

fn hunks(items: &[(&str, &str)]) -> Option<Vec<HunkFact>> {
    Some(
        items
            .iter()
            .map(|(file, digest)| HunkFact {
                file: (*file).into(),
                digest: (*digest).into(),
            })
            .collect(),
    )
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

fn revision(id: &str, number: u32, commit: &str, files: &[&str]) -> Revision {
    Revision {
        id: id.into(),
        number,
        commit: commit.into(),
        base: "base".into(),
        files: files.iter().map(|file| (*file).to_string()).collect(),
        author: "alice".into(),
        created_ms: 0,
        authorship: Authorship::Undeclared,
        submitted_by: None,
    }
}

fn change_state() -> ReviewResult<(ReviewState, Facts)> {
    let mut state = ReviewState::default();
    let mut facts = base_facts();
    send(
        &mut state,
        &mut facts,
        "alice",
        Command::Create {
            id: "c".into(),
            project: "project".into(),
            repository: "repository".into(),
            target_ref: "refs/heads/main".into(),
            title: "Change".into(),
            description: String::new(),
            wip: false,
            revision: revision("c-r1", 1, "commit-1", &["a.rs", "b.rs"]),
        },
    )?;
    for principal in ["bob", "carol"] {
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetReviewer {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                principal: principal.into(),
                role: Some(ReviewerRole::Reviewer),
            },
        )?;
    }
    Ok((state, facts))
}

fn mark_file(
    state: &mut ReviewState,
    facts: &mut Facts,
    actor: &str,
    revision: &str,
    file: &str,
    reviewed: bool,
) -> ReviewResult<()> {
    send(
        state,
        facts,
        actor,
        Command::MarkReviewed {
            change: "c".into(),
            expected_revision: revision.into(),
            file: file.into(),
            reviewed,
        },
    )
}

fn mark_hunk(
    state: &mut ReviewState,
    facts: &mut Facts,
    actor: &str,
    file: &str,
    digest: &str,
    reviewed: bool,
) -> ReviewResult<()> {
    send(
        state,
        facts,
        actor,
        Command::MarkHunk {
            change: "c".into(),
            expected_revision: "c-r1".into(),
            file: file.into(),
            hunk_digest: digest.into(),
            reviewed,
        },
    )
}

fn viewed(state: &ReviewState, actor: &str) -> Vec<(String, String, String)> {
    state
        .changes
        .get("c")
        .map(|change| {
            reviewed_hunks(change, actor)
                .map(|m| (m.revision.clone(), m.file.clone(), m.hunk_digest.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn fail_code(result: ReviewResult<()>) -> String {
    result.err().map(|error| error.code).unwrap_or_default()
}

// r[verify kiln.review.hunks]
#[test]
fn file_mark_all_writes_hunk_markers_and_hunk_toggles_are_private() -> ReviewResult<()> {
    let (mut state, mut facts) = change_state()?;
    facts.hunks = hunks(&[("a.rs", "a1"), ("a.rs", "a2"), ("b.rs", "b1")]);
    let checklist_before = checklist(&state, "c", &facts)?;
    mark_file(&mut state, &mut facts, "bob", "c-r1", "a.rs", true)?;
    assert_eq!(
        viewed(&state, "bob"),
        vec![
            ("c-r1".into(), "a.rs".into(), "a1".into()),
            ("c-r1".into(), "a.rs".into(), "a2".into())
        ]
    );
    // Private: another principal sees nothing, and markers never move approval state.
    assert!(viewed(&state, "carol").is_empty());
    assert_eq!(checklist(&state, "c", &facts)?, checklist_before);
    let change = state.changes.get("c").ok_or_else(|| ReviewError {
        code: "missing".into(),
        message: String::new(),
    })?;
    assert_eq!(
        change.last_viewed.get("bob").map(String::as_str),
        Some("c-r1")
    );
    assert!(
        change.reviewed.is_empty(),
        "no legacy file markers are written"
    );
    // Unmark one hunk, then mark it again without duplicating.
    mark_hunk(&mut state, &mut facts, "bob", "a.rs", "a2", false)?;
    assert_eq!(
        viewed(&state, "bob"),
        vec![("c-r1".into(), "a.rs".into(), "a1".into())]
    );
    mark_hunk(&mut state, &mut facts, "bob", "a.rs", "a1", true)?;
    assert_eq!(viewed(&state, "bob").len(), 1);
    // Unmarking the file removes every hunk marker of that file.
    mark_file(&mut state, &mut facts, "bob", "c-r1", "a.rs", false)?;
    assert!(viewed(&state, "bob").is_empty());
    Ok(())
}

// r[verify kiln.review.hunks]
#[test]
fn hunk_marks_fail_closed_without_facts_or_for_unknown_hunks() -> ReviewResult<()> {
    let (mut state, mut facts) = change_state()?;
    let before = state.clone();
    assert_eq!(
        fail_code(mark_file(
            &mut state, &mut facts, "bob", "c-r1", "a.rs", true
        )),
        "hunks_unavailable"
    );
    facts.hunks = hunks(&[("a.rs", "a1")]);
    assert_eq!(
        fail_code(mark_hunk(
            &mut state, &mut facts, "bob", "a.rs", "forged", true
        )),
        "invalid_hunk"
    );
    assert_eq!(
        fail_code(mark_hunk(&mut state, &mut facts, "bob", "b.rs", "a1", true)),
        "invalid_file"
    );
    assert_eq!(
        fail_code(mark_file(
            &mut state,
            &mut facts,
            "bob",
            "c-r1",
            "missing.rs",
            true
        )),
        "invalid_file"
    );
    assert_eq!(
        fail_code(mark_file(
            &mut state, &mut facts, "bob", "stale", "a.rs", true
        )),
        "stale_revision"
    );
    assert_eq!(state, before);
    Ok(())
}

// r[verify kiln.review.hunks.scenario.carry_forward]
#[test]
fn upload_carries_forward_only_unchanged_hunks_per_principal() -> ReviewResult<()> {
    let (mut state, mut facts) = change_state()?;
    // a1 unchanged, a2 edited, b1 moved with an edit; carol viewed only a2.
    facts.hunks = hunks(&[("a.rs", "a1"), ("a.rs", "a2"), ("b.rs", "b1")]);
    mark_file(&mut state, &mut facts, "bob", "c-r1", "a.rs", true)?;
    mark_file(&mut state, &mut facts, "bob", "c-r1", "b.rs", true)?;
    mark_hunk(&mut state, &mut facts, "carol", "a.rs", "a2", true)?;
    if let Some(source) = facts.source.as_mut() {
        source.commit = "commit-2".into();
    }
    facts.hunks = hunks(&[
        ("a.rs", "a1"),
        ("a.rs", "a2-edited"),
        ("b.rs", "b1-moved-edited"),
        ("b.rs", "new"),
    ]);
    send(
        &mut state,
        &mut facts,
        "alice",
        Command::Upload {
            change: "c".into(),
            expected_revision: "c-r1".into(),
            revision: revision("c-r2", 2, "commit-2", &["a.rs", "b.rs"]),
        },
    )?;
    assert_eq!(
        viewed(&state, "bob"),
        vec![("c-r2".into(), "a.rs".into(), "a1".into())]
    );
    assert!(viewed(&state, "carol").is_empty());
    let change = state.changes.get("c").ok_or_else(|| ReviewError {
        code: "missing".into(),
        message: String::new(),
    })?;
    // The last viewed revision is the principal's own history, not the upload.
    assert_eq!(
        change.last_viewed.get("bob").map(String::as_str),
        Some("c-r1")
    );
    assert!(hunk_viewed(change, "bob", "c-r2", "a.rs", "a1"));
    assert!(!hunk_viewed(change, "bob", "c-r2", "b.rs", "new"));
    // An upload without observed hunks drops every marker (never guesses).
    if let Some(source) = facts.source.as_mut() {
        source.commit = "commit-3".into();
    }
    facts.hunks = None;
    send(
        &mut state,
        &mut facts,
        "alice",
        Command::Upload {
            change: "c".into(),
            expected_revision: "c-r2".into(),
            revision: revision("c-r3", 3, "commit-3", &["a.rs"]),
        },
    )?;
    assert!(viewed(&state, "bob").is_empty());
    Ok(())
}

// r[verify kiln.review.hunks.scenario.replay]
#[test]
fn legacy_file_markers_replay_byte_identically_and_bind_their_revision()
-> Result<(), serde_json::Error> {
    let (state, _) = change_state().map_err(|error| serde::de::Error::custom(error.message))?;
    let fresh = serde_json::to_string(&state)?;
    assert!(!fresh.contains("reviewed_hunks") && !fresh.contains("last_viewed"));
    let legacy = fresh.replacen(
        "\"reviewed\":[]",
        "\"reviewed\":[{\"actor\":\"bob\",\"revision\":\"c-r1\",\"file\":\"a.rs\"}]",
        1,
    );
    assert_ne!(legacy, fresh);
    let replayed: ReviewState = serde_json::from_str(&legacy)?;
    assert_eq!(serde_json::to_string(&replayed)?, legacy);
    let change = replayed
        .changes
        .get("c")
        .ok_or_else(|| serde::de::Error::custom("change"))?;
    assert!(hunk_viewed(change, "bob", "c-r1", "a.rs", "any-hunk"));
    assert!(!hunk_viewed(change, "bob", "c-r2", "a.rs", "any-hunk"));
    assert!(!hunk_viewed(change, "carol", "c-r1", "a.rs", "any-hunk"));
    // Unknown marker fields are rejected rather than silently dropped.
    let forged = fresh.replacen(
        "\"reviewed\":[]",
        "\"reviewed\":[],\"reviewed_hunks\":[{\"actor\":\"bob\",\"revision\":\"c-r1\",\"file\":\"a.rs\",\"hunk_digest\":\"d\",\"approved\":true}]",
        1,
    );
    assert!(serde_json::from_str::<ReviewState>(&forged).is_err());
    Ok(())
}

// r[verify kiln.review.hunks.scenario.replay]
#[test]
fn unmarking_one_hunk_of_a_legacy_file_marker_keeps_the_others() -> ReviewResult<()> {
    let (state, mut facts) = change_state()?;
    let text = serde_json::to_string(&state).map_err(|_| ReviewError {
        code: "encode".into(),
        message: String::new(),
    })?;
    let legacy = text.replacen(
        "\"reviewed\":[]",
        "\"reviewed\":[{\"actor\":\"bob\",\"revision\":\"c-r1\",\"file\":\"a.rs\"}]",
        1,
    );
    let mut state: ReviewState = serde_json::from_str(&legacy).map_err(|_| ReviewError {
        code: "decode".into(),
        message: String::new(),
    })?;
    facts.hunks = hunks(&[("a.rs", "a1"), ("a.rs", "a2")]);
    mark_hunk(&mut state, &mut facts, "bob", "a.rs", "a1", false)?;
    let change = state.changes.get("c").ok_or_else(|| ReviewError {
        code: "missing".into(),
        message: String::new(),
    })?;
    assert!(change.reviewed.is_empty());
    assert!(!hunk_viewed(change, "bob", "c-r1", "a.rs", "a1"));
    assert!(hunk_viewed(change, "bob", "c-r1", "a.rs", "a2"));
    Ok(())
}

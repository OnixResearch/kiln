use super::*;
use kiln_core::change_review::CiFact;
use kiln_core::implementation::campaign::{EnumerationState, MemberStage};
use kiln_core::implementation::{
    ATTEMPT_SCHEMA, AcceptanceObligation, AttemptLedger, CandidateVerification, DEFAULT_LEASE_MS,
    Enrollment, FailureClass, IMPLEMENTATION_WORK_SCHEMA, SourceRef, TerminalPolicy,
    VERIFICATION_SCHEMA, WorkBudget, WorkLedger,
};
use kiln_core::types::ExternalRef;

const REPOSITORY: &str = "onix/kiln";
const CHANGE: &str = "chg";

fn id(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn reference(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
}

fn work(unit: &str) -> ImplementationWork {
    let native_id = format!("{CHANGE}/{unit}");
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".into(),
        repository: REPOSITORY.into(),
        project: "kiln".into(),
        base: None,
        title: native_id.clone(),
        requested_outcome: "implement".into(),
        acceptance: vec![AcceptanceObligation {
            id: "a1".into(),
            statement: "verified".into(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: id('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedIntegrated,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: reference("cairn", "task", &native_id),
            native_id,
            version: id('1'),
        }],
    }
}

fn check(name: &str, kind: &str, outcome: &str, digest: Option<char>) -> CiCheck {
    CiCheck {
        name: name.into(),
        kind: kind.into(),
        outcome: outcome.into(),
        artifact_digest: digest.map(|byte| byte.to_string().repeat(64)),
    }
}

fn completion() -> CiCheck {
    check(COMPLETION_CHECK, COMPLETION_KIND, PASSING, Some('f'))
}

/// A verified attempt on `work` whose run holds `checks`.
fn verified(work: &ImplementationWork, ordinal: u8, checks: Vec<CiCheck>) -> ImplementationAttempt {
    let work_identity = work.identity().unwrap_or_else(|_| unreachable!());
    let identity = id(char::from(b'a' + ordinal));
    ImplementationAttempt {
        schema: ATTEMPT_SCHEMA.to_string(),
        identity: identity.clone(),
        canonical_key: work.canonical_key().unwrap_or_else(|_| unreachable!()),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        base: None,
        ordinal,
        fence: u64::from(ordinal),
        lease_until_ms: DEFAULT_LEASE_MS,
        claimed_at_ms: 0,
        ended_at_ms: Some(1),
        phase: AttemptPhase::Verified,
        dispatch: None,
        dispatch_request_binding: None,
        workflow: None,
        candidate: None,
        handoff: None,
        verification: Some(CandidateVerification {
            schema: VERIFICATION_SCHEMA.to_string(),
            handoff: id('3'),
            work_identity,
            work_revision: work.revision,
            attempt: identity,
            fence: u64::from(ordinal),
            candidate: reference("git", "commit", &"c".repeat(40)),
            tree: reference("git", "tree", &"d".repeat(40)),
            base: None,
            verification_profile: work.verification_profile.clone(),
            run: CiFact {
                project: work.project.clone(),
                repository: work.repository.clone(),
                revision: "c".repeat(40),
                run_id: "verification".into(),
                ordinal: 1,
                success: true,
                full_run: true,
                clean: true,
                scope_identity: None,
                terminal: true,
                untrusted: None,
                checks,
            },
        }),
        publication: None,
        metrics: None,
        worker_observation: None,
        verification_outcome: None,
        failure: None,
        cause: None,
        superseded_by: None,
        authority: None,
    }
}

fn integration(commit: &str, evidence: char) -> Integration {
    Integration {
        commit: commit.into(),
        evidence: id(evidence),
    }
}

fn unit<'a>(
    work: &'a ImplementationWork,
    attempt: Option<&'a ImplementationAttempt>,
    integrated: Option<Integration>,
) -> UnitEvidence<'a> {
    UnitEvidence {
        native_id: work
            .aliases
            .first()
            .map(|alias| alias.native_id.clone())
            .unwrap_or_default(),
        work: Some((work, work.identity().unwrap_or_else(|_| unreachable!()))),
        attempt,
        integration: integrated,
    }
}

/// A scripted target: which integrated results it contains, which leave the
/// change source unchanged, and which results descend from which.
#[derive(Default)]
struct Target {
    contained: BTreeSet<&'static str>,
    unchanged: BTreeSet<&'static str>,
    ancestry: BTreeSet<(&'static str, &'static str)>,
    unreadable: bool,
}

impl Target {
    fn containing(commits: &[&'static str]) -> Self {
        Self {
            contained: commits.iter().copied().collect(),
            unchanged: commits.iter().copied().collect(),
            ..Self::default()
        }
    }
}

impl Applicability for Target {
    fn contained(&self, commit: &str) -> Result<bool, String> {
        if self.unreadable {
            return Err("object missing".into());
        }
        Ok(self.contained.contains(commit))
    }
    fn unchanged_since(&self, commit: &str) -> Result<bool, String> {
        Ok(self.unchanged.contains(commit))
    }
    fn descends(&self, later: &str, earlier: &str) -> Result<bool, String> {
        Ok(later == earlier
            || self
                .ancestry
                .iter()
                .any(|(descendant, ancestor)| *descendant == later && *ancestor == earlier))
    }
}

fn reason(adoption: Adoption) -> String {
    match adoption {
        Adoption::Blocked(reason) => reason,
        Adoption::Adopted { binding } => unreachable!("adopted unit {binding}"),
    }
}

#[test]
// r[verify kiln.implementation.campaign.adoption] r[verify kiln.implementation.campaign.adoption.scenario.adopted]
fn integrated_completion_evidence_is_adopted_at_the_result_containing_the_others() {
    let (a, b) = (work("task:a"), work("task:b"));
    let (first, second) = (
        verified(&a, 1, vec![completion()]),
        verified(&b, 1, vec![completion()]),
    );
    let units = [
        unit(&a, Some(&first), Some(integration("c1", 'e'))),
        unit(&b, Some(&second), Some(integration("c2", 'e'))),
    ];
    let mut target = Target::containing(&["c1", "c2"]);
    assert!(
        matches!(decide(&units, &target), Adoption::Blocked(reason) if reason.contains("diverge"))
    );
    target.ancestry.insert(("c2", "c1"));
    assert_eq!(decide(&units, &target), Adoption::Adopted { binding: 1 });
    // One integration covering both units binds the first.
    let shared = [
        unit(&a, Some(&first), Some(integration("c1", 'e'))),
        unit(&b, Some(&second), Some(integration("c1", 'e'))),
    ];
    assert_eq!(
        decide(&shared, &Target::containing(&["c1"])),
        Adoption::Adopted { binding: 0 }
    );
}

#[test]
// r[verify kiln.implementation.campaign.adoption.scenario.blocked]
fn each_missing_or_stale_fact_blocks_adoption_with_its_reason() {
    let a = work("task:a");
    let mut newer = work("task:a");
    newer.revision = 2;
    newer.title.push_str(" revised");
    let ready = verified(&a, 1, vec![completion()]);
    let target = Target::containing(&["c1"]);
    assert_eq!(
        decide(
            &[unit(&a, Some(&ready), Some(integration("c1", 'e')))],
            &target
        ),
        Adoption::Adopted { binding: 0 }
    );

    let mut not_verified = ready.clone();
    not_verified.phase = AttemptPhase::CandidateProduced;
    let mut failed_run = ready.clone();
    if let Some(verification) = failed_run.verification.as_mut() {
        verification.run.success = false;
    }
    let missing_completion = verified(&a, 1, vec![check("test", "test", PASSING, None)]);
    let doubled_completion = verified(&a, 1, vec![completion(), completion()]);
    let failed_completion = verified(
        &a,
        1,
        vec![check(COMPLETION_CHECK, COMPLETION_KIND, "fail", None)],
    );
    let wrong_kind = verified(&a, 1, vec![check(COMPLETION_CHECK, "other", PASSING, None)]);
    let other_version = verified(&newer, 1, vec![completion()]);
    let controls: [(UnitEvidence<'_>, &Target, &str); 12] = [
        (
            UnitEvidence {
                native_id: "chg/task:a".into(),
                work: None,
                attempt: None,
                integration: None,
            },
            &target,
            "not admitted through Kiln",
        ),
        (unit(&a, None, None), &target, "has no attempt"),
        (
            unit(&a, Some(&other_version), Some(integration("c1", 'e'))),
            &target,
            "another work version",
        ),
        (
            unit(&a, Some(&not_verified), Some(integration("c1", 'e'))),
            &target,
            "not independently verified",
        ),
        (
            unit(&a, Some(&failed_run), Some(integration("c1", 'e'))),
            &target,
            "not independently verified",
        ),
        (
            unit(&a, Some(&missing_completion), Some(integration("c1", 'e'))),
            &target,
            "completion check",
        ),
        (
            unit(&a, Some(&doubled_completion), Some(integration("c1", 'e'))),
            &target,
            "completion check",
        ),
        (
            unit(&a, Some(&failed_completion), Some(integration("c1", 'e'))),
            &target,
            "completion check",
        ),
        (
            unit(&a, Some(&wrong_kind), Some(integration("c1", 'e'))),
            &target,
            "completion check",
        ),
        (unit(&a, Some(&ready), None), &target, "no integration"),
        (
            unit(&a, Some(&ready), Some(integration("c9", 'e'))),
            &target,
            "not contained",
        ),
        (
            unit(&a, Some(&ready), Some(integration("c1", 'e'))),
            &Target {
                unchanged: BTreeSet::new(),
                ..Target::containing(&["c1"])
            },
            "change source moved",
        ),
    ];
    for (evidence, target, expected) in controls {
        let reason = reason(decide(&[evidence], target));
        assert!(reason.contains(expected), "{expected}: {reason}");
        assert!(reason.starts_with("chg/task:a: "), "{reason}");
    }
    assert!(reason(decide(&[], &target)).contains("no required native unit"));
    let unreadable = Target {
        unreadable: true,
        ..Target::containing(&["c1"])
    };
    assert!(
        reason(decide(
            &[unit(&a, Some(&ready), Some(integration("c1", 'e')))],
            &unreadable
        ))
        .contains("could not be read")
    );
    // One unadoptable unit blocks the whole member.
    let b = work("task:b");
    assert!(
        reason(decide(
            &[
                unit(&a, Some(&ready), Some(integration("c1", 'e'))),
                unit(&b, None, None)
            ],
            &target
        ))
        .starts_with("chg/task:b: ")
    );
}

fn member() -> MemberObservation {
    MemberObservation {
        member: format!("{CHANGE}/*"),
        stage: MemberStage::Selected,
        enumeration: EnumerationState::Complete,
        source_identity: None,
        policy_identity: id('a'),
        subject_identity: None,
        integration_target_identity: None,
        profile_identity: id('a'),
        dependencies: Vec::new(),
        remaining_obligations: BTreeSet::from(["chg/task:a".to_string(), "chg/task:b".to_string()]),
        dependency_reason: None,
        canonical_work_key: None,
        work_identity: None,
        attempt_identity: None,
        attempt_fence: None,
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

#[test]
// r[verify kiln.implementation.campaign.adoption.scenario.adopted]
fn the_adopted_binding_records_owner_facts_and_clears_adopted_units() {
    let (a, b) = (work("task:a"), work("task:b"));
    let first = verified(
        &a,
        1,
        vec![
            completion(),
            check("acceptance", "test", PASSING, Some('7')),
        ],
    );
    let second = verified(&b, 2, vec![completion()]);
    let units = [
        unit(&a, Some(&first), Some(integration("c1", 'e'))),
        unit(&b, Some(&second), Some(integration("c2", 'd'))),
    ];
    let adopted = Adopted::new(&units, 0).unwrap_or_else(|error| unreachable!("{error}"));
    let mut observed = member();
    adopted.identify(&mut observed);
    adopted.bind(&mut observed, &["acceptance".to_string()]);
    assert_eq!(observed.work_identity, a.identity().ok());
    assert_eq!(observed.canonical_work_key, a.canonical_key().ok());
    assert_eq!(observed.source_identity, Some(id('1')));
    assert_eq!(
        (observed.attempt_identity.clone(), observed.attempt_fence),
        (Some(first.identity.clone()), Some(1))
    );
    assert_eq!(
        (observed.candidate_freshness, observed.verifier_freshness),
        (Freshness::Current, Freshness::Current)
    );
    assert!(observed.required_checks_satisfied);
    assert!(observed.remaining_obligations.is_empty());
    assert!(observed.evidence.contains(&id('e')) && observed.evidence.contains(&id('7')));
    // Binding again changes nothing; a required check the run lacks is unmet.
    let before = observed.clone();
    adopted.bind(&mut observed, &["acceptance".to_string()]);
    assert_eq!(observed, before);
    adopted.bind(&mut observed, &["lint".to_string()]);
    assert!(!observed.required_checks_satisfied);
    assert!(Adopted::new(&units, 2).is_err());
    assert!(Adopted::new(&[unit(&a, None, None)], 0).is_err());
}

#[test]
// r[verify kiln.implementation.campaign.adoption]
fn extraction_reads_the_latest_attempt_of_each_unit_and_only_then_its_integration() {
    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let (a, b) = (work("task:a"), work("task:b"));
    let mut ledgers = CanonicalCampaignLedgers {
        work: WorkLedger::default(),
        attempts: AttemptLedger::default(),
    };
    for row in [&a, &b] {
        ledgers
            .work
            .admit(row, None, &enrollment)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    }
    let first = ledgers
        .attempts
        .claim(&a, 0, DEFAULT_LEASE_MS)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    ledgers
        .attempts
        .fail(
            &first.identity,
            first.fence,
            1,
            FailureClass::Implementation,
            "tests failed",
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let second = ledgers
        .attempts
        .claim(&a, 2, DEFAULT_LEASE_MS)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let units = BTreeSet::from([
        "chg/task:a".to_string(),
        "chg/task:b".to_string(),
        "chg/task:c".to_string(),
    ]);
    let mut asked = Vec::new();
    let evidence = unit_evidence(&ledgers, REPOSITORY, &units, |attempt| {
        asked.push(attempt.identity.clone());
        Ok(Some(integration("c1", 'e')))
    })
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(asked, vec![second.identity.clone()]);
    let facts: Vec<(&str, bool, Option<u8>, bool)> = evidence
        .iter()
        .map(|unit| {
            (
                unit.native_id.as_str(),
                unit.work.is_some(),
                unit.attempt.map(|attempt| attempt.ordinal),
                unit.integration.is_some(),
            )
        })
        .collect();
    assert_eq!(
        facts,
        vec![
            ("chg/task:a", true, Some(2), true),
            ("chg/task:b", true, None, false),
            ("chg/task:c", false, None, false),
        ]
    );
    let elsewhere = unit_evidence(&ledgers, "onix/other", &units, |_| Ok(None))
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(elsewhere.iter().all(|unit| unit.work.is_none()));
}

/// Commit `files` on `parent`, moving HEAD only when asked.
fn commit(
    repository: &git2::Repository,
    parent: Option<git2::Oid>,
    files: &[(&str, Option<&str>)],
    head: bool,
) -> git2::Oid {
    let root = repository
        .workdir()
        .unwrap_or_else(|| unreachable!())
        .to_path_buf();
    let mut index = repository.index().unwrap_or_else(|_| unreachable!());
    if let Some(parent) = parent {
        let tree = repository
            .find_commit(parent)
            .and_then(|commit| commit.tree())
            .unwrap_or_else(|_| unreachable!());
        index.read_tree(&tree).unwrap_or_else(|_| unreachable!());
    }
    for (path, content) in files {
        let full = root.join(path);
        match content {
            Some(content) => {
                std::fs::create_dir_all(full.parent().unwrap_or_else(|| unreachable!()))
                    .unwrap_or_else(|_| unreachable!());
                std::fs::write(&full, content).unwrap_or_else(|_| unreachable!());
                index
                    .add_path(Path::new(path))
                    .unwrap_or_else(|_| unreachable!());
            }
            None => index
                .remove_path(Path::new(path))
                .unwrap_or_else(|_| unreachable!()),
        }
    }
    let tree = index
        .write_tree()
        .and_then(|tree| repository.find_tree(tree))
        .unwrap_or_else(|_| unreachable!());
    let signature =
        git2::Signature::new("fixture", "fixture@example.invalid", &git2::Time::new(0, 0))
            .unwrap_or_else(|_| unreachable!());
    let parents: Vec<git2::Commit<'_>> = parent
        .map(|parent| {
            repository
                .find_commit(parent)
                .unwrap_or_else(|_| unreachable!())
        })
        .into_iter()
        .collect();
    let parents: Vec<&git2::Commit<'_>> = parents.iter().collect();
    repository
        .commit(
            head.then_some("HEAD"),
            &signature,
            &signature,
            "fixture",
            &tree,
            &parents,
        )
        .unwrap_or_else(|_| unreachable!())
}

#[test]
// r[verify kiln.implementation.campaign.adoption.scenario.blocked]
fn target_history_requires_containment_and_an_unchanged_change_source() {
    let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let repository = git2::Repository::init(root.path()).unwrap_or_else(|_| unreachable!());
    let tasks = ".cairn/changes/chg/tasks.md";
    let base = commit(
        &repository,
        None,
        &[(tasks, Some("- [x] a")), ("README", Some("one"))],
        true,
    );
    let unrelated = commit(&repository, Some(base), &[("README", Some("two"))], true);
    let moved = commit(
        &repository,
        Some(unrelated),
        &[(tasks, Some("- [x] a\n- [ ] b"))],
        true,
    );
    let removed = commit(&repository, Some(unrelated), &[(tasks, None)], false);
    let side = commit(&repository, Some(base), &[("OTHER", Some("side"))], false);
    let open = |target: git2::Oid| {
        TargetHistory::open(root.path(), &target.to_string(), CHANGE)
            .unwrap_or_else(|error| unreachable!("{error}"))
    };
    let at_unrelated = open(unrelated);
    assert_eq!(at_unrelated.contained(&base.to_string()), Ok(true));
    assert_eq!(at_unrelated.contained(&unrelated.to_string()), Ok(true));
    assert_eq!(at_unrelated.contained(&moved.to_string()), Ok(false));
    assert_eq!(at_unrelated.contained(&side.to_string()), Ok(false));
    assert_eq!(at_unrelated.unchanged_since(&base.to_string()), Ok(true));
    assert_eq!(
        at_unrelated.descends(&unrelated.to_string(), &base.to_string()),
        Ok(true)
    );
    assert_eq!(
        at_unrelated.descends(&base.to_string(), &unrelated.to_string()),
        Ok(false)
    );
    assert_eq!(open(moved).unchanged_since(&base.to_string()), Ok(false));
    assert_eq!(open(removed).unchanged_since(&base.to_string()), Ok(false));
    assert!(at_unrelated.contained("not-a-commit").is_err());
    assert!(at_unrelated.contained(&"f".repeat(40)).is_err());
    assert!(TargetHistory::open(&root.path().join("absent"), &base.to_string(), CHANGE).is_err());
}

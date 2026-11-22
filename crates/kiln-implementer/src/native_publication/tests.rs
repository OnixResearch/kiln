use super::*;

fn id(value: char) -> ContentId {
    ContentId::from_blake3_hex(value.to_string().repeat(64))
        .unwrap_or_else(|error| unreachable!("fixture identity failed: {}", error.message))
}

#[test]
fn retained_intent_resume_matches_only_the_same_operation_scope() {
    let external = |native_id: char, object_type: &str| {
        kiln_core::types::ExternalRef::new("git", object_type, &native_id.to_string().repeat(40))
            .unwrap_or_else(|error| unreachable!("fixture reference failed: {}", error.message))
    };
    let plan = NativeGitPlan {
        schema: kiln_core::implementation::NATIVE_GIT_SCHEMA.into(),
        work_identity: id('1'),
        work_revision: 2,
        attempt: id('2'),
        fence: 3,
        handoff: id('3'),
        verification: id('4'),
        review: id('5'),
        checks: id('6'),
        policy: id('7'),
        repository: "repo".into(),
        remote_identity: id('8'),
        provider_profile: "native-git-v1".into(),
        branch_expected_old: None,
        branch_ref: "refs/heads/impl/work".into(),
        target_ref: "refs/heads/main".into(),
        expected_old_target: external('a', "commit"),
        candidate: external('b', "commit"),
        tree: external('c', "tree"),
        branch_operation: id('9'),
        integration_operation: id('a'),
    };
    let mut resume = plan.clone();
    resume.review = id('d');
    resume.checks = id('e');
    resume.branch_operation = id('f');
    resume.integration_operation = id('0');
    assert!(same_native_git_request(&resume, &plan));
    resume.remote_identity = id('0');
    assert!(!same_native_git_request(&resume, &plan));
}

use crate::admission::AdmissionCommand;
use kiln_core::change_review::{
    self, AgentVotes, CiCheck, CiFact, Command as ReviewCommand, Facts, OwnershipPolicy,
    Policy as ReviewPolicy, ReviewState, ReviewerRole, Revision, SourceFact, VoteValue,
};
use kiln_core::fair_admission::AdmissionPolicy;
use kiln_core::implementation::{
    AcceptanceObligation, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff,
    CandidateVerification, Enrollment, EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, SourceRef, TerminalPolicy, VERIFICATION_SCHEMA, WorkBudget, WorkSourceKind,
    acceptance_digest,
};
use kiln_core::operations::FairnessStrategy;
use kiln_core::types::{Alias, ExternalRef, ProviderProfileId};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use git2::{Oid, Repository, Signature, Time};
use kiln_adapter_git::native_publication::{
    NativeRemoteProfile, NativeTransport, PushObservation, push_exact_ref, read_remote_ref,
    remote_identity,
};
fn external(kind: &str, native_id: &str) -> ExternalRef {
    ExternalRef::new("git", kind, native_id)
        .unwrap_or_else(|error| unreachable!("fixture reference failed: {}", error.message))
}

fn commit(repo: &Repository, parent: Option<Oid>, path: &str, content: &[u8]) -> Oid {
    let mut object = repo
        .blob(content)
        .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
    let mut mode = 0o100644;
    for component in path.rsplit('/') {
        let mut builder = repo
            .treebuilder(None)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        builder
            .insert(component, object, mode)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        object = builder
            .write()
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        mode = 0o040000;
    }
    let tree = repo
        .find_tree(object)
        .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )
    .unwrap_or_else(|error| unreachable!("fixture signature failed: {error}"));
    let parents = parent
        .iter()
        .map(|oid| {
            repo.find_commit(*oid)
                .unwrap_or_else(|error| unreachable!("fixture parent failed: {error}"))
        })
        .collect::<Vec<_>>();
    let parents = parents.iter().collect::<Vec<_>>();
    repo.commit(None, &signature, &signature, "fixture", &tree, &parents)
        .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"))
}

fn native_enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".into()]),
        actors: BTreeSet::from(["operator".into()]),
    }
}

fn native_work(base: ExternalRef) -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: "onix/kiln".into(),
        project: "kiln".into(),
        base: Some(base),
        title: "Native integration test".into(),
        requested_outcome: "verified native integration".into(),
        acceptance: vec![AcceptanceObligation {
            id: "exact".into(),
            statement: "exact candidate is integrated".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: id('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedIntegrated,
        priority: 1,
        aliases: vec![
            SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "OnixResearch/kiln#63")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "OnixResearch/kiln#63".into(),
                version: id('1'),
            },
            SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: ExternalRef::new("cairn", "task", "native-git/change/task")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "native-git/change/task".into(),
                version: id('2'),
            },
        ],
    }
}

fn native_policy() -> AdmissionPolicy {
    AdmissionPolicy {
        version: 1,
        accounting_basis: id('a'),
        max_items: 16,
        max_bytes: 100_000,
        max_wait_ms: 1000,
        max_observation_age_ms: 100,
        max_retry_after_ms: 100,
        max_children: 4,
        max_active: 2,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(Alias::new("native").unwrap_or_else(|_| unreachable!()), 1)]),
        group_limits: vec![],
        serving: None,
    }
}

fn send_review(
    state: &mut ReviewState,
    facts: &mut Facts,
    actor: &str,
    mut command: ReviewCommand,
) {
    facts.now_ms += 1;
    facts.event_id = format!("event-{}", facts.now_ms);
    if let ReviewCommand::Create { revision, .. } | ReviewCommand::Upload { revision, .. } =
        &mut command
    {
        revision.created_ms = facts.now_ms;
    }
    change_review::apply(state, actor, command, facts)
        .unwrap_or_else(|error| unreachable!("review fixture rejected: {error}"));
}

fn review_authority(candidate: &ExternalRef, base: &ExternalRef) -> NativeGitReviewAuthority {
    let mut facts = Facts {
        policy: Some(ReviewPolicy {
            id: "native-policy".into(),
            eligible_reviewers: BTreeSet::from(["reviewer".into()]),
            submitters: BTreeSet::from(["operator".into()]),
            minimum_approvals: 1,
            require_ci: true,
            ownership: OwnershipPolicy::default(),
            optional_ci: vec![],
            agent_votes: AgentVotes::Advisory,
            required_checks: vec![],
        }),
        source: Some(SourceFact {
            project: "kiln".into(),
            repository: "onix/kiln".into(),
            target_ref: "refs/heads/main".into(),
            commit: candidate.native_id.as_str().into(),
            base: base.native_id.as_str().into(),
            current_target: base.native_id.as_str().into(),
            immutable: true,
            target_contains_commit: false,
            target_is_ancestor: true,
            target_ancestors: BTreeSet::new(),
            changed_files: None,
        }),
        ci: vec![CiFact {
            project: "kiln".into(),
            repository: "onix/kiln".into(),
            revision: candidate.native_id.as_str().into(),
            run_id: "native-run".into(),
            ordinal: 1,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,
            checks: vec![],
        }],
        ..Facts::default()
    };
    let mut state = ReviewState::default();
    send_review(
        &mut state,
        &mut facts,
        "operator",
        ReviewCommand::Create {
            id: "native".into(),
            project: "kiln".into(),
            repository: "onix/kiln".into(),
            target_ref: "refs/heads/main".into(),
            title: "Native Git".into(),
            description: "Exact test candidate".into(),
            wip: false,
            revision: Revision {
                id: "native-r1".into(),
                number: 1,
                commit: candidate.native_id.as_str().into(),
                base: base.native_id.as_str().into(),
                files: vec!["crates/kiln/src/lib.rs".into()],
                author: "operator".into(),
                created_ms: 0,
                authorship: change_review::Authorship::Undeclared,
                submitted_by: None,
                session_traces: None,
            },
        },
    );
    send_review(
        &mut state,
        &mut facts,
        "operator",
        ReviewCommand::SetReviewer {
            change: "native".into(),
            expected_revision: "native-r1".into(),
            principal: "reviewer".into(),
            role: Some(ReviewerRole::Reviewer),
        },
    );
    send_review(
        &mut state,
        &mut facts,
        "reviewer",
        ReviewCommand::Vote {
            change: "native".into(),
            expected_revision: "native-r1".into(),
            value: VoteValue::Approve,
        },
    );
    send_review(
        &mut state,
        &mut facts,
        "operator",
        ReviewCommand::PrepareSubmit {
            change: "native".into(),
            expected_revision: "native-r1".into(),
            candidate_id: "native-candidate".into(),
        },
    );
    assert!(
        change_review::checklist(&state, "native", &facts)
            .unwrap_or_else(|error| unreachable!("review checklist failed: {error}"))
            .ready
    );
    NativeGitReviewAuthority {
        state,
        facts,
        profile_identity: id('d'),
        source_identity: id('e'),
        ci_identity: id('f'),
        change_id: "native".into(),
        revision_id: "native-r1".into(),
        candidate_id: "native-candidate".into(),
        actor: "operator".into(),
    }
}

struct NativeFixture {
    path: PathBuf,
    remote_root: PathBuf,
    repository: Repository,
    remote: Repository,
    profile: NativeRemoteProfile,
    base_oid: Oid,
    candidate_oid: Oid,
    work: ImplementationWork,
    attempt: kiln_core::implementation::ImplementationAttempt,
    review: NativeGitReviewAuthority,
    plan: NativeGitPlan,
    plan_id: ContentId,
}

fn no_credentials(
    _: &str,
    _: Option<&str>,
    _: git2::CredentialType,
) -> Result<git2::Cred, git2::Error> {
    Err(git2::Error::from_str(
        "bare fixture unexpectedly requested credentials",
    ))
}

impl NativeFixture {
    fn new(path: &Path) -> (Implementer, Self) {
        let remote_root = path.with_extension("native-remote");
        std::fs::create_dir_all(&remote_root)
            .unwrap_or_else(|error| unreachable!("fixture directory failed: {error}"));
        let bare_path = remote_root.join("remote.git");
        let local_path = remote_root.join("local");
        let remote = Repository::init_bare(&bare_path)
            .unwrap_or_else(|error| unreachable!("bare fixture failed: {error}"));
        let repository = Repository::init(&local_path)
            .unwrap_or_else(|error| unreachable!("local fixture failed: {error}"));
        let url = bare_path.to_string_lossy().into_owned();
        repository
            .remote("origin", &url)
            .unwrap_or_else(|error| unreachable!("remote fixture failed: {error}"));
        let profile = NativeRemoteProfile {
            remote_name: "origin".into(),
            remote_url: url,
            transport: NativeTransport::LocalBareFixture,
            ssh_host_key_sha256: None,
        };
        let remote_id = remote_identity(&profile)
            .unwrap_or_else(|error| unreachable!("remote identity failed: {error}"));
        let base_oid = commit(&repository, None, "crates/kiln/src/lib.rs", b"base\n");
        let candidate_oid = commit(
            &repository,
            Some(base_oid),
            "crates/kiln/src/lib.rs",
            b"candidate\n",
        );
        assert!(matches!(
            push_exact_ref(
                &repository,
                &profile,
                &remote_id,
                base_oid,
                "refs/heads/main",
                Oid::ZERO_SHA1,
                no_credentials
            ),
            Ok(PushObservation::Confirmed { .. })
        ));
        let base = external("commit", &base_oid.to_string());
        let candidate = external("commit", &candidate_oid.to_string());
        let tree_oid = repository
            .find_commit(candidate_oid)
            .unwrap_or_else(|error| unreachable!("candidate lookup failed: {error}"))
            .tree_id();
        let tree = external("tree", &tree_oid.to_string());
        let work = native_work(base.clone());
        let mut engine =
            Implementer::open_with_local_operator(path, native_enrollment(), "test-operator")
                .unwrap_or_else(|error| unreachable!("fixture open failed: {error}"));
        engine.set_clock_ms(1);
        engine
            .admit(&work, None)
            .unwrap_or_else(|error| unreachable!("work admission failed: {error}"));
        engine
            .admission_apply(
                AdmissionCommand::Configure {
                    policy: native_policy(),
                },
                10,
            )
            .unwrap_or_else(|error| unreachable!("policy setup failed: {error}"));
        let claim = engine
            .claim_next(0, 60_000)
            .unwrap_or_else(|error| unreachable!("claim failed: {error}"))
            .unwrap_or_else(|| unreachable!("fixture claim absent"));
        engine
            .reserve_dispatch(
                &claim.identity,
                claim.fence,
                1,
                &id('d'),
                &ProviderProfileId::new("native-worker").unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|error| unreachable!("dispatch reservation failed: {error}"));
        engine
            .start_execution(
                &claim.identity,
                claim.fence,
                2,
                &ExternalRef::new("lattice", "run", "native-run")
                    .unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|error| unreachable!("execution start failed: {error}"));
        let handoff = CandidateHandoff {
            schema: CANDIDATE_HANDOFF_SCHEMA.into(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt_identity: claim.identity.clone(),
            fence: claim.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            candidate: candidate.clone(),
            tree: tree.clone(),
            changed_paths: vec!["crates/kiln/src/lib.rs".into()],
            acceptance_digest: acceptance_digest(&work).unwrap_or_else(|_| unreachable!()),
            verification_profile: work.verification_profile.clone(),
            toolchain: id('7'),
            environment: id('8'),
            evidence: vec![CandidateEvidence {
                identity: id('e'),
                kind: EvidenceKind::Check,
                outcome: EvidenceOutcome::Passed,
                reference: ExternalRef::new("ci", "run", "native-check")
                    .unwrap_or_else(|_| unreachable!()),
            }],
        };
        engine
            .admit_candidate_handoff(&work, &claim.identity, claim.fence, 3, &handoff)
            .unwrap_or_else(|error| unreachable!("handoff admission failed: {error}"));
        engine
            .begin_verification(&claim.identity, claim.fence, 4)
            .unwrap_or_else(|error| unreachable!("verification start failed: {error}"));
        let handoff_id =
            kiln_core::implementation::admit_candidate_handoff(&work, &claim, &handoff)
                .unwrap_or_else(|_| unreachable!());
        let verification = CandidateVerification {
            schema: VERIFICATION_SCHEMA.into(),
            handoff: handoff_id,
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt: claim.identity.clone(),
            fence: claim.fence,
            candidate: candidate.clone(),
            tree: tree.clone(),
            base: work.base.clone(),
            verification_profile: work.verification_profile.clone(),
            run: CiFact {
                project: work.project.clone(),
                repository: work.repository.clone(),
                revision: candidate.native_id.as_str().into(),
                run_id: "native-verify".into(),
                ordinal: 1,
                success: true,
                full_run: true,
                clean: true,
                scope_identity: None,
                terminal: true,
                untrusted: None,
                checks: vec![CiCheck {
                    name: "cairn-authoritative-completion".into(),
                    kind: "cairn-agent-completion-v2".into(),
                    outcome: "pass".into(),
                    artifact_digest: Some(id('f').as_str().into()),
                }],
            },
        };
        engine
            .record_verification(
                &work,
                &claim.identity,
                claim.fence,
                5,
                &handoff,
                &verification,
            )
            .unwrap_or_else(|error| unreachable!("verification admission failed: {error}"));
        let attempt = engine
            .attempt(&claim.identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let handoff_id =
            kiln_core::implementation::admit_candidate_handoff(&work, &attempt, &handoff)
                .unwrap_or_else(|error| {
                    unreachable!("retained handoff identity failed: {}", error.message)
                });
        let verification_id = kiln_core::implementation::admit_candidate_verification(
            &work,
            &attempt,
            &handoff,
            &verification,
        )
        .unwrap_or_else(|error| {
            unreachable!("retained verification identity failed: {}", error.message)
        });
        let review = review_authority(&candidate, &base);
        let mut plan = NativeGitPlan {
            schema: kiln_core::implementation::NATIVE_GIT_SCHEMA.into(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: work.revision,
            attempt: claim.identity.clone(),
            fence: claim.fence,
            handoff: handoff_id,
            verification: verification_id,
            review: id('0'),
            checks: id('0'),
            policy: native_policy()
                .identity()
                .unwrap_or_else(|_| unreachable!()),
            repository: work.repository.clone(),
            remote_identity: remote_id,
            provider_profile: "native-git-v1".into(),
            branch_expected_old: None,
            branch_ref: "refs/heads/impl/native".into(),
            target_ref: "refs/heads/main".into(),
            expected_old_target: base,
            candidate,
            tree,
            branch_operation: id('b'),
            integration_operation: id('c'),
        };
        let plan_id = engine
            .prepare_native_git(&work, &claim.identity, claim.fence, &mut plan, &review)
            .unwrap_or_else(|error| unreachable!("native plan preparation failed: {error}"));
        let canonical_plan = serde_json::to_vec(&plan)
            .unwrap_or_else(|error| unreachable!("plan encoding failed: {error}"));
        let expected_plan_id =
            ContentId::from_blake3_hex(blake3::hash(&canonical_plan).to_hex().to_string())
                .unwrap_or_else(|error| unreachable!("plan identity failed: {}", error.message));
        assert_eq!(plan_id, expected_plan_id);
        (
            engine,
            Self {
                path: path.to_path_buf(),
                remote_root,
                repository,
                remote,
                profile,
                base_oid,
                candidate_oid,
                work,
                attempt,
                review,
                plan,
                plan_id,
            },
        )
    }

    fn reopen(&self) -> Implementer {
        let mut implementer =
            Implementer::open_with_local_operator(&self.path, native_enrollment(), "test-operator")
                .unwrap_or_else(|error| unreachable!("fixture reopen failed: {error}"));
        implementer.set_clock_ms(1);
        implementer
    }
}

impl Drop for NativeFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.remote_root);
    }
}

fn push_branch(fixture: &NativeFixture) -> Result<PushObservation, git2::Error> {
    push_exact_ref(
        &fixture.repository,
        &fixture.profile,
        &fixture.plan.remote_identity,
        fixture.candidate_oid,
        &fixture.plan.branch_ref,
        Oid::ZERO_SHA1,
        no_credentials,
    )
}

fn read_branch(
    fixture: &NativeFixture,
    plan: &NativeGitPlan,
) -> Result<Option<NativeGitObservation>, String> {
    let oid = read_remote_ref(
        &fixture.repository,
        &fixture.profile,
        &plan.remote_identity,
        &plan.branch_ref,
        no_credentials,
    )
    .map_err(|error| error.to_string())?;
    Ok(oid.map(|oid| NativeGitObservation::Branch {
        remote: plan.remote_identity.clone(),
        branch_ref: plan.branch_ref.clone(),
        commit: external("commit", &oid.to_string()),
    }))
}

fn read_target(
    fixture: &NativeFixture,
    plan: &NativeGitPlan,
) -> Result<Option<NativeGitObservation>, String> {
    let oid = read_remote_ref(
        &fixture.repository,
        &fixture.profile,
        &plan.remote_identity,
        &plan.target_ref,
        no_credentials,
    )
    .map_err(|error| error.to_string())?;
    Ok(oid.map(|oid| NativeGitObservation::Target {
        remote: plan.remote_identity.clone(),
        target_ref: plan.target_ref.clone(),
        previous: plan.expected_old_target.clone(),
        resulting: external("commit", &oid.to_string()),
    }))
}

fn publish_branch(fixture: &NativeFixture, engine: &mut Implementer) -> NativeGitEffectReservation {
    engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::BranchPublication,
            },
            &fixture.review,
            |mode, plan| {
                assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                let pushed = push_branch(fixture).map_err(|error| error.to_string())?;
                match pushed {
                    PushObservation::Confirmed { .. } => Ok(Some(NativeGitObservation::Branch {
                        remote: plan.remote_identity.clone(),
                        branch_ref: plan.branch_ref.clone(),
                        commit: plan.candidate.clone(),
                    })),
                    other => Err(format!("local bare branch publication failed: {other:?}")),
                }
            },
        )
        .unwrap_or_else(|error| unreachable!("branch publication failed: {error}"))
}

fn valid_lifecycle(
    fixture: &NativeFixture,
) -> crate::publication_effects::CairnLifecycleObservation {
    crate::publication_effects::CairnLifecycleObservation {
        binding: crate::publication_effects::CairnLifecycleBinding {
            source: fixture
                .work
                .aliases
                .iter()
                .find(|alias| alias.kind == kiln_core::implementation::WorkSourceKind::CairnTask)
                .cloned()
                .unwrap_or_else(|| unreachable!("Cairn task alias absent")),
            handoff: fixture.plan.handoff.clone(),
            verification: fixture.plan.verification.clone(),
            authoritative_completion: id('f'),
            candidate: fixture.plan.candidate.clone(),
            action: crate::publication_effects::CairnLifecycleAction::ObserveReviewedSync,
        },
        observed_tree: fixture.plan.tree.clone(),
        lifecycle_policy: fixture.plan.policy.clone(),
        lifecycle_receipt: ExternalRef::new("cairn", "lifecycle-receipt", "native-observation")
            .unwrap_or_else(|_| unreachable!()),
    }
}

#[test]
fn lost_branch_and_target_acknowledgments_reconcile_after_reopen() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    let mut branch_pushes = 0;
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::BranchPublication,
                },
                &fixture.review,
                |mode, _| {
                    assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                    branch_pushes += 1;
                    assert!(matches!(
                        push_branch(&fixture),
                        Ok(PushObservation::Confirmed { .. })
                    ));
                    Err("simulated lost branch acknowledgment".into())
                }
            )
            .is_err()
    );
    drop(engine);

    let mut engine = fixture.reopen();
    let branch = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::BranchPublication,
            },
            &fixture.review,
            |mode, _| {
                assert_eq!(mode, NativeGitDispatch::ReconcileOnly);
                read_branch(&fixture, &fixture.plan)
            },
        )
        .unwrap_or_else(|error| unreachable!("branch reconciliation failed: {error}"));
    assert!(matches!(
        branch.observed,
        Some(NativeGitObservation::Branch { .. })
    ));
    assert_eq!(branch_pushes, 1);

    let mut target_pushes = 0;
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::TargetIntegration,
                },
                &fixture.review,
                |mode, plan| {
                    assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                    target_pushes += 1;
                    match push_exact_ref(
                        &fixture.repository,
                        &fixture.profile,
                        &plan.remote_identity,
                        fixture.candidate_oid,
                        &plan.target_ref,
                        fixture.base_oid,
                        no_credentials,
                    )
                    .map_err(|error| error.to_string())?
                    {
                        PushObservation::Confirmed { .. } => {
                            Err("simulated lost target acknowledgment".into())
                        }
                        other => Err(format!("target push did not land: {other:?}")),
                    }
                }
            )
            .is_err()
    );
    drop(engine);

    let mut engine = fixture.reopen();
    let integration = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::TargetIntegration,
            },
            &fixture.review,
            |mode, plan| {
                assert_eq!(mode, NativeGitDispatch::ReconcileOnly);
                read_target(&fixture, plan)
            },
        )
        .unwrap_or_else(|error| unreachable!("target reconciliation failed: {error}"));
    assert!(integration.receipt.is_some());
    assert_eq!(target_pushes, 1);
    assert_eq!(
        fixture.remote.refname_to_id("refs/heads/main").ok(),
        Some(fixture.candidate_oid)
    );
    assert_eq!(
        engine
            .native_git_receipt(&fixture.attempt.identity)
            .unwrap_or_else(|_| unreachable!()),
        integration.receipt
    );
}

#[test]
fn unknown_branch_recovery_reads_real_remote_without_a_second_push() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    let mut pushes = 0;
    let unknown = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::BranchPublication,
            },
            &fixture.review,
            |mode, plan| {
                assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                pushes += 1;
                assert!(matches!(
                    push_branch(&fixture),
                    Ok(PushObservation::Confirmed { .. })
                ));
                Ok(Some(NativeGitObservation::Unknown {
                    operation: plan.branch_operation.clone(),
                    reason: "ack lost".into(),
                }))
            },
        )
        .unwrap_or_else(|error| unreachable!("unknown observation was not retained: {error}"));
    assert!(matches!(
        unknown.observed,
        Some(NativeGitObservation::Unknown { .. })
    ));
    let branch = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::BranchPublication,
            },
            &fixture.review,
            |mode, _| {
                assert_eq!(mode, NativeGitDispatch::ReconcileOnly);
                read_branch(&fixture, &fixture.plan)
            },
        )
        .unwrap_or_else(|error| unreachable!("unknown reconciliation failed: {error}"));
    assert!(matches!(
        branch.observed,
        Some(NativeGitObservation::Branch { .. })
    ));
    assert_eq!(pushes, 1);
}

fn bump_fence(engine: &mut Implementer, identity: &ContentId) {
    let tx = engine
        .connection
        .transaction()
        .unwrap_or_else(|_| unreachable!());
    let (mut ledger, version) =
        load::<serde_json::Value>(&tx, ATTEMPT_KIND).unwrap_or_else(|_| unreachable!());
    ledger
        .get_mut("attempts")
        .and_then(|attempts| attempts.get_mut(identity.as_str()))
        .and_then(serde_json::Value::as_object_mut)
        .unwrap_or_else(|| unreachable!("retained attempt ledger entry"))
        .insert("fence".into(), serde_json::json!(u64::MAX));
    save(&tx, ATTEMPT_KIND, version, &ledger).unwrap_or_else(|_| unreachable!());
    tx.commit().unwrap_or_else(|_| unreachable!());
}

// Configuration is immutable through the owner API. Inject a changed durable
// snapshot to ensure a retained plan still cannot dispatch under stale authority.
fn replace_policy_snapshot(engine: &mut Implementer, policy: AdmissionPolicy) {
    let tx = engine
        .connection
        .transaction()
        .unwrap_or_else(|_| unreachable!());
    let (mut ledger, version) = load::<AdmissionLedger>(&tx, POLICY_KIND)
        .unwrap_or_else(|error| unreachable!("policy snapshot failed: {error}"));
    ledger.policy = Some(policy);
    save(&tx, POLICY_KIND, version, &ledger)
        .unwrap_or_else(|error| unreachable!("policy snapshot failed: {error}"));
    tx.commit().unwrap_or_else(|_| unreachable!());
}

#[test]
fn new_target_effect_refuses_stale_review_policy_fence_and_claim() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    publish_branch(&fixture, &mut engine);
    let mut calls = 0;
    let mut stale_review = fixture.review.clone();
    stale_review
        .facts
        .source
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .current_target = id('d').as_str().into();
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::TargetIntegration,
                },
                &stale_review,
                |_, _| {
                    calls += 1;
                    Ok(None)
                }
            )
            .is_err()
    );

    let mut changed_policy = native_policy();
    changed_policy.accounting_basis = id('8');
    replace_policy_snapshot(&mut engine, changed_policy);
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::TargetIntegration,
                },
                &fixture.review,
                |_, _| {
                    calls += 1;
                    Ok(None)
                }
            )
            .is_err()
    );
    replace_policy_snapshot(&mut engine, native_policy());

    bump_fence(&mut engine, &fixture.attempt.identity);
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::TargetIntegration,
                },
                &fixture.review,
                |_, _| {
                    calls += 1;
                    Ok(None)
                }
            )
            .is_err()
    );
    assert_eq!(calls, 0);

    let mut superseding = fixture.work.clone();
    superseding.revision += 1;
    superseding.title = "Superseding authority".into();
    superseding
        .aliases
        .first_mut()
        .unwrap_or_else(|| unreachable!("work has an alias"))
        .version = id('3');
    engine
        .admit(&superseding, None)
        .unwrap_or_else(|error| unreachable!("claim supersession failed: {error}"));
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::TargetIntegration,
                },
                &fixture.review,
                |_, _| {
                    calls += 1;
                    Ok(None)
                }
            )
            .is_err()
    );
    assert_eq!(calls, 0);
}

#[test]
fn wrong_remote_and_candidate_observations_cannot_be_retained_or_accepted() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    let mut wrong_candidate = false;
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::BranchPublication,
                },
                &fixture.review,
                |mode, plan| {
                    assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                    assert!(matches!(
                        push_branch(&fixture),
                        Ok(PushObservation::Confirmed { .. })
                    ));
                    wrong_candidate = true;
                    Ok(Some(NativeGitObservation::Branch {
                        remote: plan.remote_identity.clone(),
                        branch_ref: plan.branch_ref.clone(),
                        commit: external("commit", &"d".repeat(40)),
                    }))
                }
            )
            .is_err()
    );
    assert!(wrong_candidate);

    let mut wrong_remote = false;
    assert!(
        engine
            .consume_native_git_effect(
                NativeGitEffectSelection {
                    work: &fixture.work,
                    attempt: &fixture.attempt.identity,
                    fence: fixture.attempt.fence,
                    plan_identity: &fixture.plan_id,
                    kind: NativeGitEffectKind::BranchPublication,
                },
                &fixture.review,
                |mode, plan| {
                    assert_eq!(mode, NativeGitDispatch::ReconcileOnly);
                    wrong_remote = true;
                    Ok(Some(NativeGitObservation::Branch {
                        remote: id('d'),
                        branch_ref: plan.branch_ref.clone(),
                        commit: plan.candidate.clone(),
                    }))
                }
            )
            .is_err()
    );
    assert!(wrong_remote);

    let recovered = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::BranchPublication,
            },
            &fixture.review,
            |mode, _| {
                assert_eq!(mode, NativeGitDispatch::ReconcileOnly);
                read_branch(&fixture, &fixture.plan)
            },
        )
        .unwrap_or_else(|error| unreachable!("exact remote read-back failed: {error}"));
    assert!(
        matches!(recovered.observed, Some(NativeGitObservation::Branch { ref commit, .. }) if commit == &fixture.plan.candidate)
    );

    let mut wrong_plan = fixture.plan.clone();
    wrong_plan.remote_identity = id('e');
    assert!(
        engine
            .prepare_native_git(
                &fixture.work,
                &fixture.attempt.identity,
                fixture.attempt.fence,
                &mut wrong_plan,
                &fixture.review
            )
            .is_err()
    );
}

#[test]
fn external_target_advance_is_explicit_and_creates_no_receipt() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    publish_branch(&fixture, &mut engine);
    let advanced_oid = commit(
        &fixture.repository,
        Some(fixture.base_oid),
        "crates/kiln/src/lib.rs",
        b"independent target advance\n",
    );
    assert!(matches!(
        push_exact_ref(
            &fixture.repository,
            &fixture.profile,
            &fixture.plan.remote_identity,
            advanced_oid,
            "refs/heads/main",
            fixture.base_oid,
            no_credentials
        ),
        Ok(PushObservation::Confirmed { .. })
    ));

    let integration = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::TargetIntegration,
            },
            &fixture.review,
            |mode, plan| {
                assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                let observed = push_exact_ref(
                    &fixture.repository,
                    &fixture.profile,
                    &plan.remote_identity,
                    fixture.candidate_oid,
                    &plan.target_ref,
                    fixture.base_oid,
                    no_credentials,
                )
                .map_err(|error| error.to_string())?;
                match observed {
                    PushObservation::Rejected { reason } => {
                        Ok(Some(NativeGitObservation::Rejected {
                            operation: plan.integration_operation.clone(),
                            code: "target_race".into(),
                            reason,
                        }))
                    }
                    other => Err(format!("target race was not rejected: {other:?}")),
                }
            },
        )
        .unwrap_or_else(|error| unreachable!("target rejection was not retained: {error}"));
    assert!(matches!(
        integration.observed,
        Some(NativeGitObservation::Rejected { .. })
    ));
    assert!(integration.receipt.is_none());
    assert_eq!(
        fixture.remote.refname_to_id("refs/heads/main").ok(),
        Some(advanced_oid)
    );
}

#[test]
fn exact_native_integration_consumes_cairn_lifecycle_receipt() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    publish_branch(&fixture, &mut engine);
    let integration = engine
        .consume_native_git_effect(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::TargetIntegration,
            },
            &fixture.review,
            |mode, plan| {
                assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                match push_exact_ref(
                    &fixture.repository,
                    &fixture.profile,
                    &plan.remote_identity,
                    fixture.candidate_oid,
                    &plan.target_ref,
                    fixture.base_oid,
                    no_credentials,
                )
                .map_err(|error| error.to_string())?
                {
                    PushObservation::Confirmed { .. } => Ok(Some(NativeGitObservation::Target {
                        remote: plan.remote_identity.clone(),
                        target_ref: plan.target_ref.clone(),
                        previous: plan.expected_old_target.clone(),
                        resulting: plan.candidate.clone(),
                    })),
                    other => Err(format!("exact target integration failed: {other:?}")),
                }
            },
        )
        .unwrap_or_else(|error| unreachable!("native integration failed: {error}"));
    assert!(integration.receipt.is_some());

    let mut malformed = valid_lifecycle(&fixture);
    malformed.binding.candidate = external("commit", &"d".repeat(40));
    assert!(
        engine
            .record_native_cairn_lifecycle(
                &fixture.work,
                &fixture.attempt.identity,
                fixture.attempt.fence,
                &malformed,
                &fixture.review
            )
            .is_err()
    );

    let observation = valid_lifecycle(&fixture);
    let receipt = engine
        .record_native_cairn_lifecycle(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &observation,
            &fixture.review,
        )
        .unwrap_or_else(|error| unreachable!("Cairn lifecycle consumption failed: {error}"));
    assert_eq!(
        receipt.integration,
        id_from_receipt(
            &integration
                .receipt
                .clone()
                .unwrap_or_else(|| unreachable!())
        )
    );
    drop(engine);
    let mut reopened = fixture.reopen();
    assert_eq!(
        reopened
            .native_cairn_lifecycle(&fixture.attempt.identity)
            .unwrap_or_else(|_| unreachable!()),
        Some(receipt)
    );
}

#[test]
fn current_authority_is_re_resolved_after_intent_before_push_and_ack() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("native.db");
    let (mut engine, fixture) = NativeFixture::new(&path);
    let mut observations = 0;
    let reservation = engine
        .consume_native_git_effect_with_current_authority(
            NativeGitEffectSelection {
                work: &fixture.work,
                attempt: &fixture.attempt.identity,
                fence: fixture.attempt.fence,
                plan_identity: &fixture.plan_id,
                kind: NativeGitEffectKind::BranchPublication,
            },
            || {
                observations += 1;
                Ok(fixture.review.clone())
            },
            |mode, plan| {
                assert_eq!(mode, NativeGitDispatch::FirstDispatch);
                match push_branch(&fixture).map_err(|error| error.to_string())? {
                    PushObservation::Confirmed { .. } => Ok(Some(NativeGitObservation::Branch {
                        remote: plan.remote_identity.clone(),
                        branch_ref: plan.branch_ref.clone(),
                        commit: plan.candidate.clone(),
                    })),
                    other => Err(format!("branch fixture failed: {other:?}")),
                }
            },
        )
        .unwrap_or_else(|error| unreachable!("authority re-resolution failed: {error}"));
    assert_eq!(observations, 3);
    assert!(matches!(
        reservation.observed,
        Some(NativeGitObservation::Branch { .. })
    ));
}

fn id_from_receipt(receipt: &NativeGitReceipt) -> ContentId {
    native_receipt_identity(receipt)
        .unwrap_or_else(|error| unreachable!("native receipt identity failed: {error}"))
}

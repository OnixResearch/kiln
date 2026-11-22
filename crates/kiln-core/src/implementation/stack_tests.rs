//! Route, layer verification, plan, gate, retained publication and prefix
//! integration of an ordered stack.
use super::*;
use crate::change_review::CiCheck;
use crate::implementation::{
    ATTEMPT_SCHEMA, AcceptanceObligation, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence,
    EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA, SourceRef, VERIFICATION_SCHEMA,
    WorkBudget, WorkSourceKind, acceptance_digest, admit_candidate_handoff,
    admit_candidate_verification,
};
use alloc::collections::BTreeSet;
use alloc::vec;

const BASE: char = '1';
const LOWER: char = 'a';
const LOWER_TREE: char = 'b';
const TOP: char = 'c';
const TOP_TREE: char = 'd';

fn reference(kind: &str, character: char) -> ExternalRef {
    ExternalRef::new("git", kind, &character.to_string().repeat(40))
        .unwrap_or_else(|_| unreachable!())
}

fn commit(character: char) -> ExternalRef {
    reference("commit", character)
}

fn tree(character: char) -> ExternalRef {
    reference("tree", character)
}

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn work() -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "onix/kiln".to_string(),
        project: "kiln".to_string(),
        base: Some(commit(BASE)),
        title: "Stacked work".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#89")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#89".to_string(),
            version: content('1'),
        }],
    }
}

fn handoff(work: &ImplementationWork) -> CandidateHandoff {
    CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
        work_revision: work.revision,
        attempt_identity: content('a'),
        fence: 4,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: commit(TOP),
        tree: tree(TOP_TREE),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: acceptance_digest(work).unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: content('7'),
        environment: content('8'),
        evidence: vec![CandidateEvidence {
            identity: content('e'),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: ExternalRef::new("kiln", "run", "candidate-check")
                .unwrap_or_else(|_| unreachable!()),
        }],
    }
}

fn run(work: &ImplementationWork, revision: char) -> CiFact {
    CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: revision.to_string().repeat(40),
        run_id: format!("run-{revision}"),
        ordinal: 1,
        success: true,
        full_run: true,
        clean: true,
        scope_identity: None,
        terminal: true,
        untrusted: None,
        checks: vec![CiCheck {
            name: "test".to_string(),
            kind: "test".to_string(),
            outcome: "pass".to_string(),
            artifact_digest: Some("f".repeat(64)),
        }],
    }
}

fn attempt(
    work: &ImplementationWork,
    verification: Option<CandidateVerification>,
) -> ImplementationAttempt {
    let handoff = handoff(work);
    ImplementationAttempt {
        schema: ATTEMPT_SCHEMA.to_string(),
        identity: content('a'),
        canonical_key: content('b'),
        work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
        work_revision: work.revision,
        base: work.base.clone(),
        ordinal: 1,
        fence: 4,
        lease_until_ms: 100,
        claimed_at_ms: 10,
        ended_at_ms: Some(20),
        phase: AttemptPhase::Verified,
        dispatch: None,
        dispatch_request_binding: None,
        workflow: None,
        candidate: Some(handoff.candidate.clone()),
        handoff: Some(handoff),
        verification,
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

struct Fixture {
    work: ImplementationWork,
    attempt: ImplementationAttempt,
    handoff: CandidateHandoff,
    verification: CandidateVerification,
    lower: Vec<LayerVerification>,
    plan: StackPlan,
}

impl Fixture {
    fn new() -> Self {
        let work = work();
        let handoff = handoff(&work);
        let handoff_identity = admit_candidate_handoff(&work, &attempt(&work, None), &handoff)
            .unwrap_or_else(|_| unreachable!());
        let verification = CandidateVerification {
            schema: VERIFICATION_SCHEMA.to_string(),
            handoff: handoff_identity.clone(),
            work_identity: handoff.work_identity.clone(),
            work_revision: handoff.work_revision,
            attempt: handoff.attempt_identity.clone(),
            fence: handoff.fence,
            candidate: handoff.candidate.clone(),
            tree: handoff.tree.clone(),
            base: handoff.base.clone(),
            verification_profile: work.verification_profile.clone(),
            run: run(&work, TOP),
        };
        let attempt = attempt(&work, Some(verification.clone()));
        let lower = LayerVerification {
            schema: LAYER_VERIFICATION_SCHEMA.to_string(),
            handoff: handoff_identity,
            work_identity: handoff.work_identity.clone(),
            work_revision: handoff.work_revision,
            attempt: handoff.attempt_identity.clone(),
            fence: handoff.fence,
            position: 1,
            candidate: commit(LOWER),
            tree: tree(LOWER_TREE),
            parent: commit(BASE),
            verification_profile: work.verification_profile.clone(),
            run: run(&work, LOWER),
        };
        let lower_identity = admit_layer_verification(&work, &attempt, &handoff, &lower)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let top_identity = admit_candidate_verification(&work, &attempt, &handoff, &verification)
            .unwrap_or_else(|_| unreachable!());
        let work_identity = work.identity().unwrap_or_else(|_| unreachable!());
        let bottom_branch = layer_branch(&work_identity, &attempt.identity, 1);
        let plan = StackPlan {
            schema: STACK_SCHEMA.to_string(),
            work_identity: work_identity.clone(),
            work_revision: work.revision,
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            handoff: lower.handoff.clone(),
            verification: top_identity.clone(),
            repository: work.repository.clone(),
            base: commit(BASE),
            trunk_branch: "main".to_string(),
            layers: vec![
                StackLayerPlan {
                    position: 1,
                    candidate: commit(LOWER),
                    tree: tree(LOWER_TREE),
                    parent: commit(BASE),
                    verification: lower_identity,
                    branch: bottom_branch.clone(),
                    base_branch: "main".to_string(),
                    title: "Add the shared types".to_string(),
                    body: "Bottom layer".to_string(),
                },
                StackLayerPlan {
                    position: 2,
                    candidate: commit(TOP),
                    tree: tree(TOP_TREE),
                    parent: commit(LOWER),
                    verification: top_identity,
                    branch: layer_branch(&work_identity, &attempt.identity, 2),
                    base_branch: bottom_branch,
                    title: "Use the shared types".to_string(),
                    body: "Top layer".to_string(),
                },
            ],
        };
        Self {
            work,
            attempt,
            handoff,
            verification,
            lower: vec![lower],
            plan,
        }
    }

    fn admit(&self) -> CoreResult<ContentId> {
        admit_stack_plan(
            &self.work,
            &self.attempt,
            &self.handoff,
            &self.verification,
            &self.lower,
            &self.plan,
        )
    }

    fn publication(&self) -> StackPublication {
        StackPublication {
            schema: STACK_PUBLICATION_SCHEMA.to_string(),
            stack: stack_identity(&self.plan).unwrap_or_else(|_| unreachable!()),
            provider_stack: github("stack", 42),
            pull_requests: vec![github("pull_request", 101), github("pull_request", 102)],
        }
    }
}

fn github(kind: &str, number: u64) -> ExternalRef {
    ExternalRef::new("github", kind, &format!("onix/kiln#{number}"))
        .unwrap_or_else(|_| unreachable!())
}

fn code(result: CoreResult<ContentId>) -> Option<String> {
    result.err().map(|diagnostic| diagnostic.code)
}

// r[verify kiln.implementation.stack.scenario.route]
#[test]
fn routes_default_to_one_pull_request_and_refuse_origin() {
    assert_eq!(PublicationRoute::default(), PublicationRoute::PullRequest);
    for supported in [false, true] {
        assert_eq!(
            resolve_route(PublicationRoute::PullRequest, supported),
            Ok(PublicationRoute::PullRequest)
        );
    }
    assert_eq!(
        resolve_route(PublicationRoute::GithubStack, true),
        Ok(PublicationRoute::GithubStack)
    );
    assert_eq!(
        resolve_route(PublicationRoute::GithubStack, false),
        Ok(PublicationRoute::PullRequest)
    );
    for supported in [false, true] {
        assert_eq!(
            resolve_route(PublicationRoute::OriginStack, supported).map_err(|error| error.code),
            Err("implementation_stack_destination_unsupported".to_string())
        );
    }
}

// r[verify kiln.implementation.stack]
#[test]
fn layer_verification_binds_one_exact_lower_layer_under_the_ci_rules() {
    let fixture = Fixture::new();
    let lower = fixture
        .lower
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let admit = |layer: &LayerVerification| {
        admit_layer_verification(&fixture.work, &fixture.attempt, &fixture.handoff, layer)
    };
    let identity = admit(&lower).unwrap_or_else(|_| unreachable!());
    type Change = fn(&mut LayerVerification);
    let changes: [(&str, Change); 10] = [
        ("schema", |layer| layer.schema = "kiln.other.v1".to_string()),
        ("top position", |layer| {
            layer.position = MAX_STACK_LAYERS as u32
        }),
        ("zero position", |layer| layer.position = 0),
        ("top candidate", |layer| layer.candidate = commit(TOP)),
        ("own parent", |layer| layer.parent = commit(LOWER)),
        ("profile", |layer| layer.verification_profile = content('7')),
        ("fence", |layer| layer.fence = 5),
        ("failed run", |layer| layer.run.success = false),
        ("other revision", |layer| {
            layer.run.revision = TOP.to_string().repeat(40)
        }),
        ("failing check", |layer| {
            if let Some(check) = layer.run.checks.first_mut() {
                check.outcome = "fail".to_string();
            }
        }),
    ];
    for (name, change) in changes {
        let mut changed = lower.clone();
        change(&mut changed);
        assert!(admit(&changed).is_err(), "{name} admitted");
    }
    let mut renamed = lower;
    renamed.run.run_id = "run-again".to_string();
    assert_ne!(admit(&renamed).ok(), Some(identity), "the run is bound");
}

// r[verify kiln.implementation.stack.scenario.plan]
#[test]
fn stack_plan_binds_every_layer_exactly() {
    let fixture = Fixture::new();
    let identity = fixture
        .admit()
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!(
        identity,
        stack_identity(&fixture.plan).unwrap_or_else(|_| unreachable!())
    );
    type Change = fn(&mut Fixture);
    let changes: [(&str, Change); 16] = [
        ("swapped positions", |f| f.plan.layers.swap(0, 1)),
        ("skipped position", |f| {
            if let Some(top) = f.plan.layers.last_mut() {
                top.position = 3;
            }
        }),
        ("broken parent", |f| {
            if let Some(top) = f.plan.layers.last_mut() {
                top.parent = commit(BASE);
            }
        }),
        ("bottom off base", |f| {
            if let Some(bottom) = f.plan.layers.first_mut() {
                bottom.parent = commit('2');
            }
        }),
        ("broken branch chain", |f| {
            if let Some(top) = f.plan.layers.last_mut() {
                top.base_branch = "main".to_string();
            }
        }),
        ("foreign branch", |f| {
            if let Some(bottom) = f.plan.layers.first_mut() {
                bottom.branch = "kiln/stack-other-1".to_string();
            }
        }),
        ("top not verified candidate", |f| {
            if let Some(top) = f.plan.layers.last_mut() {
                top.candidate = commit('e');
            }
        }),
        ("lower verification identity", |f| {
            if let Some(bottom) = f.plan.layers.first_mut() {
                bottom.verification = content('5');
            }
        }),
        ("missing layer verification", |f| f.lower.clear()),
        ("lower evidence for other tree", |f| {
            if let Some(evidence) = f.lower.first_mut() {
                evidence.tree = tree('6');
            }
        }),
        ("single layer", |f| {
            f.plan.layers.truncate(1);
            f.lower.clear();
        }),
        ("empty title", |f| {
            if let Some(top) = f.plan.layers.last_mut() {
                top.title.clear();
            }
        }),
        ("not verified", |f| {
            f.attempt.phase = AttemptPhase::Verifying
        }),
        ("other fence", |f| f.plan.fence = 5),
        ("unsupported policy", |f| {
            f.work.terminal_policy = TerminalPolicy::VerifiedOrAlreadySatisfied;
        }),
        ("other trunk", |f| f.plan.trunk_branch = String::new()),
    ];
    for (name, change) in changes {
        let mut changed = Fixture::new();
        change(&mut changed);
        assert!(changed.admit().is_err(), "{name} admitted");
    }
    let mut seventeen = Fixture::new();
    let bottom = seventeen
        .plan
        .layers
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    while seventeen.plan.layers.len() <= MAX_STACK_LAYERS {
        seventeen.plan.layers.insert(0, bottom.clone());
    }
    assert_eq!(
        code(seventeen.admit()),
        Some("implementation_stack_binding".to_string())
    );
}

fn refused(code: &str) -> LayerState {
    LayerState::Refused {
        code: code.to_string(),
    }
}

fn unknown(code: &str) -> LayerState {
    LayerState::Unknown {
        code: code.to_string(),
    }
}

// r[verify kiln.implementation.stack.gate] r[verify kiln.implementation.stack.gate.scenario.lower]
#[test]
fn lower_layers_gate_higher_layers_from_the_bottom() {
    let blocked = |layer, code: &str| LayerGate::Blocked {
        layer,
        code: code.to_string(),
    };
    let waiting = |layer, code: &str| LayerGate::Unknown {
        layer,
        code: code.to_string(),
    };
    let cases = [
        (
            vec![LayerState::Ready, LayerState::Ready],
            vec![LayerGate::MayLand, LayerGate::MayLand],
        ),
        (
            vec![
                refused("github_landing_check_failed"),
                LayerState::Ready,
                LayerState::Ready,
            ],
            vec![
                blocked(1, "github_landing_check_failed"),
                blocked(1, "github_landing_check_failed"),
                blocked(1, "github_landing_check_failed"),
            ],
        ),
        (
            vec![unknown("github_landing_pagination"), LayerState::Ready],
            vec![
                waiting(1, "github_landing_pagination"),
                waiting(1, "github_landing_pagination"),
            ],
        ),
        (
            vec![
                LayerState::Ready,
                refused("github_landing_approvals"),
                LayerState::Ready,
            ],
            vec![
                LayerGate::MayLand,
                blocked(2, "github_landing_approvals"),
                blocked(2, "github_landing_approvals"),
            ],
        ),
        (
            vec![LayerState::Integrated, LayerState::Ready],
            vec![LayerGate::Integrated, LayerGate::MayLand],
        ),
        (
            vec![LayerState::Ready, LayerState::Integrated],
            vec![LayerGate::MayLand, waiting(2, OUT_OF_ORDER)],
        ),
        (
            vec![
                refused("github_stack_layer_head_changed"),
                LayerState::Integrated,
            ],
            vec![
                blocked(1, "github_stack_layer_head_changed"),
                blocked(1, "github_stack_layer_head_changed"),
            ],
        ),
    ];
    for (states, expected) in cases {
        assert_eq!(stack_gate(&states), Ok(expected), "{states:?}");
    }
    for states in [
        vec![LayerState::Ready],
        vec![LayerState::Ready; MAX_STACK_LAYERS + 1],
        vec![LayerState::Ready, refused("")],
        vec![
            LayerState::Ready,
            unknown(&"x".repeat(MAX_STATE_CODE_BYTES + 1)),
        ],
    ] {
        assert!(stack_gate(&states).is_err(), "{states:?}");
    }
}

// r[verify kiln.implementation.stack]
#[test]
fn retained_publication_binds_one_pull_request_per_layer() {
    let fixture = Fixture::new();
    assert!(admit_stack_publication(&fixture.plan, &fixture.publication()).is_ok());
    type Change = fn(&mut StackPublication);
    let changes: [(&str, Change); 6] = [
        ("schema", |publication| {
            publication.schema = "kiln.other.v1".to_string()
        }),
        ("other plan", |publication| publication.stack = content('5')),
        ("missing pull request", |publication| {
            publication.pull_requests.pop();
        }),
        ("duplicate pull request", |publication| {
            publication.pull_requests =
                vec![github("pull_request", 101), github("pull_request", 101)];
        }),
        ("other repository", |publication| {
            if let Some(pull) = publication.pull_requests.first_mut() {
                *pull = ExternalRef::new("github", "pull_request", "other/kiln#1")
                    .unwrap_or_else(|_| unreachable!());
            }
        }),
        ("stack kind", |publication| {
            publication.provider_stack = github("pull_request", 42)
        }),
    ];
    for (name, change) in changes {
        let mut publication = fixture.publication();
        change(&mut publication);
        assert!(
            admit_stack_publication(&fixture.plan, &publication).is_err(),
            "{name} admitted"
        );
    }
}

fn landed(
    position: u32,
    candidate: char,
    tree_id: char,
    first_parent: ExternalRef,
    merge: char,
) -> StackLayerIntegration {
    StackLayerIntegration {
        position,
        pull_request: github("pull_request", 100 + u64::from(position)),
        candidate: commit(candidate),
        merge_commit: commit(merge),
        mapping: IntegrationMapping {
            expected_base: first_parent,
            candidate_tree: tree(tree_id),
            landed_tree: tree(tree_id),
        },
    }
}

// r[verify kiln.implementation.stack.gate.scenario.integration]
#[test]
fn integration_admits_only_an_exact_bottom_prefix() {
    let fixture = Fixture::new();
    let publication = fixture.publication();
    let integration = |layers: Vec<StackLayerIntegration>| StackIntegration {
        schema: STACK_INTEGRATION_SCHEMA.to_string(),
        stack: publication.stack.clone(),
        layers,
    };
    let bottom = landed(1, LOWER, LOWER_TREE, commit(BASE), '3');
    let top = landed(2, TOP, TOP_TREE, commit('3'), '4');
    let prefix = admit_stack_integration(
        &fixture.plan,
        &publication,
        &integration(vec![bottom.clone()]),
    )
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!((prefix.integrated_layers, prefix.complete), (1, false));
    let whole = admit_stack_integration(
        &fixture.plan,
        &publication,
        &integration(vec![bottom.clone(), top.clone()]),
    )
    .unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!((whole.integrated_layers, whole.complete), (2, true));
    assert_ne!(prefix.identity, whole.identity);
    let refusals = [
        (
            "top only",
            vec![landed(1, TOP, TOP_TREE, commit(BASE), '4')],
        ),
        (
            "top on lower candidate",
            vec![bottom.clone(), landed(2, TOP, TOP_TREE, commit(LOWER), '4')],
        ),
        (
            "rewritten top head",
            vec![bottom.clone(), landed(2, 'e', TOP_TREE, commit('3'), '4')],
        ),
        (
            "other landed tree",
            vec![bottom.clone(), landed(2, TOP, '6', commit('3'), '4')],
        ),
        (
            "bottom off base",
            vec![landed(1, LOWER, LOWER_TREE, commit('2'), '3'), top.clone()],
        ),
        (
            "merge is candidate",
            vec![landed(1, LOWER, LOWER_TREE, commit(BASE), LOWER)],
        ),
        (
            "extra layer",
            vec![
                bottom.clone(),
                top.clone(),
                landed(3, 'e', 'f', commit('4'), '5'),
            ],
        ),
        ("no layers", Vec::new()),
    ];
    for (name, layers) in refusals {
        assert!(
            admit_stack_integration(&fixture.plan, &publication, &integration(layers)).is_err(),
            "{name} admitted"
        );
    }
    let mut foreign = top;
    foreign.pull_request = github("pull_request", 7);
    assert!(
        admit_stack_integration(
            &fixture.plan,
            &publication,
            &integration(vec![bottom, foreign])
        )
        .is_err()
    );
}

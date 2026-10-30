use super::*;
use alloc::string::ToString;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const SHA256: &str = "3333333333333333333333333333333333333333333333333333333333333333";

fn gate(commit: &str) -> UntrustedGate {
    UntrustedGate::new(
        UntrustedReason::Fork,
        7,
        "outside/source".to_string(),
        commit.to_string(),
        false,
    )
    .unwrap_or_else(|_| unreachable!())
}

fn command(authority: ApproverAuthority, run: &str, commit: &str) -> ApproveUntrustedRun {
    ApproveUntrustedRun {
        actor: Approver {
            id: "operator".to_string(),
            label: "Operator".to_string(),
            authority,
        },
        run: run.to_string(),
        source_commit: commit.to_string(),
    }
}

// r[verify kiln.untrusted.classification]
#[test]
fn classification_matrix_keeps_deny_and_fails_closed_under_approve() {
    let facts = |same_repository, author_association| PullRequestTrustFacts {
        same_repository,
        author_association,
    };
    // Deny: exactly today's behaviour, association ignored.
    assert_eq!(
        classify(UntrustedPullRequests::Deny, facts(true, None)),
        TrustDecision::Trusted
    );
    assert_eq!(
        classify(UntrustedPullRequests::Deny, facts(true, Some("NONE"))),
        TrustDecision::Trusted
    );
    assert_eq!(
        classify(UntrustedPullRequests::Deny, facts(false, Some("OWNER"))),
        TrustDecision::Refused
    );
    // Approve: forks are always untrusted, even from owners.
    assert_eq!(
        classify(UntrustedPullRequests::Approve, facts(false, Some("OWNER"))),
        TrustDecision::Untrusted(UntrustedReason::Fork)
    );
    for association in PUSH_ACCESS_ASSOCIATIONS {
        assert_eq!(
            classify(
                UntrustedPullRequests::Approve,
                facts(true, Some(association))
            ),
            TrustDecision::Trusted
        );
    }
    for association in [
        None,
        Some("CONTRIBUTOR"),
        Some("FIRST_TIME_CONTRIBUTOR"),
        Some("FIRST_TIMER"),
        Some("NONE"),
        Some("owner"),
        Some(""),
    ] {
        assert_eq!(
            classify(UntrustedPullRequests::Approve, facts(true, association)),
            TrustDecision::Untrusted(UntrustedReason::AuthorWithoutPushAccess)
        );
    }
    assert!(UntrustedPullRequests::default().is_deny());
}

#[test]
fn gate_admission_requires_exact_commit_pull_request_and_head() {
    assert!(
        UntrustedGate::new(UntrustedReason::Fork, 1, "a/b".into(), SHA256.into(), false).is_ok()
    );
    assert_eq!(
        UntrustedGate::new(UntrustedReason::Fork, 0, "a/b".into(), A.into(), false),
        Err(GateRejection::ZeroPullRequest)
    );
    assert_eq!(
        UntrustedGate::new(UntrustedReason::Fork, 1, "".into(), A.into(), false),
        Err(GateRejection::InvalidHeadRepository)
    );
    for commit in [
        &A[..39],
        "G111111111111111111111111111111111111111",
        "ABCDEF1111111111111111111111111111111111",
    ] {
        assert_eq!(
            UntrustedGate::new(UntrustedReason::Fork, 1, "a/b".into(), commit.into(), false),
            Err(GateRejection::InvalidCommit)
        );
    }
}

// r[verify kiln.untrusted.approval]
// r[verify kiln.untrusted.approval.scenario.unauthorized]
#[test]
fn only_an_operator_approves_the_exact_commit_once() {
    let blocked = gate(A);
    assert_eq!(blocked.state(), UntrustedState::AwaitingApproval);
    assert!(!blocked.may_dispatch());
    for authority in [
        ApproverAuthority::Viewer,
        ApproverAuthority::Worker,
        ApproverAuthority::Automation,
    ] {
        assert_eq!(
            decide_approval("run", &blocked, &command(authority, "run", A), 5),
            Err(ApprovalRejection::Unauthorized)
        );
        // Authorization precedes state disclosure.
        assert_eq!(
            decide_approval("run", &blocked, &command(authority, "other", "bad"), 5),
            Err(ApprovalRejection::Unauthorized)
        );
    }
    let operator = |run, commit| command(ApproverAuthority::Operator, run, commit);
    assert_eq!(
        decide_approval("run", &blocked, &operator("other", A), 5),
        Err(ApprovalRejection::RunMismatch)
    );
    assert_eq!(
        decide_approval("run", &blocked, &operator("run", "abc"), 5),
        Err(ApprovalRejection::InvalidCommit)
    );
    assert_eq!(
        decide_approval("run", &blocked, &operator("run", B), 5),
        Err(ApprovalRejection::StaleCommit)
    );
    let mut invalid = operator("run", A);
    invalid.actor.label = "line\nbreak".into();
    assert_eq!(
        decide_approval("run", &blocked, &invalid, 5),
        Err(ApprovalRejection::InvalidActor)
    );
    let approval =
        decide_approval("run", &blocked, &operator("run", A), 5).unwrap_or_else(|_| unreachable!());
    assert_eq!(approval.source_commit, A);
    assert_eq!(approval.actor_id, "operator");
    let mut approved = blocked.clone();
    approved.approval = Some(approval);
    assert_eq!(approved.state(), UntrustedState::Approved);
    assert!(approved.may_dispatch());
    assert_eq!(
        decide_approval("run", &approved, &operator("run", A), 6),
        Err(ApprovalRejection::AlreadyApproved)
    );
    // SHA-256 object ids are exact commits too.
    let wide = gate(SHA256);
    assert!(decide_approval("run", &wide, &operator("run", SHA256), 1).is_ok());
}

// r[verify kiln.untrusted.blocked.scenario.new_commit]
#[test]
fn new_commit_supersedes_unapproved_gate_but_never_an_approved_one() {
    let older = gate(A);
    let newer = gate(B);
    assert!(supersedes(&newer, &older));
    assert!(!supersedes(&older, &older));
    let mut other_pr = gate(B);
    other_pr.pull_request = 8;
    assert!(!supersedes(&other_pr, &older));
    let mut other_head = gate(B);
    other_head.head_repository = "someone/else".into();
    assert!(!supersedes(&other_head, &older));
    let mut superseded = older.clone();
    superseded.superseded_by = Some("newer".into());
    assert_eq!(superseded.state(), UntrustedState::Superseded);
    assert_eq!(
        decide_approval(
            "run",
            &superseded,
            &command(ApproverAuthority::Operator, "run", A),
            1
        ),
        Err(ApprovalRejection::Superseded)
    );
    let mut approved = older;
    approved.approval = Some(UntrustedApproval {
        actor_id: "operator".into(),
        actor_label: "Operator".into(),
        source_commit: A.into(),
        approved_at_ms: 1,
    });
    assert!(!supersedes(&newer, &approved));
}

// r[verify kiln.untrusted.execution]
#[test]
fn untrusted_grants_are_empty_and_trusted_grants_unchanged() {
    let untrusted = execution_grants(true);
    assert!(!untrusted.secrets && !untrusted.workload_token);
    assert!(!untrusted.cache_restore && !untrusted.cache_publish);
    assert!(untrusted.requires_untrusted_capable_runner);
    assert_eq!(execution_grants(false), TRUSTED_GRANTS);
    let provenance = UntrustedProvenance {
        reason: UntrustedReason::Fork,
        source_commit: A.into(),
        approver_id: "operator".into(),
    };
    assert!(provenance.admit());
    assert!(
        !UntrustedProvenance {
            source_commit: "HEAD".into(),
            ..provenance.clone()
        }
        .admit()
    );
    assert!(
        !UntrustedProvenance {
            approver_id: String::new(),
            ..provenance
        }
        .admit()
    );
}

// r[verify kiln.untrusted.review]
#[test]
fn required_ci_needs_policy_approval_and_exact_commit() {
    let mut approved = gate(A);
    approved.approval = Some(UntrustedApproval {
        actor_id: "operator".into(),
        actor_label: "Operator".into(),
        source_commit: A.into(),
        approved_at_ms: 1,
    });
    assert!(!approved.satisfies_required_ci(A), "policy off");
    approved.satisfies_required_ci = true;
    assert!(approved.satisfies_required_ci(A));
    assert!(!approved.satisfies_required_ci(B), "other commit");
    let mut blocked = gate(A);
    blocked.satisfies_required_ci = true;
    assert!(!blocked.satisfies_required_ci(A), "not approved");
    let summary = approved.summary();
    assert_eq!(summary.state, UntrustedState::Approved);
    assert_eq!(summary.approved_commit.as_deref(), Some(A));
    assert!(summary.counts_for_required_ci);
}

#[test]
fn default_gate_fields_are_omitted_from_durable_bytes() {
    let encoded = serde_json::to_string(&gate(A)).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        encoded,
        "{\"reason\":\"fork\",\"pull_request\":7,\"head_repository\":\"outside/source\",\"source_commit\":\"1111111111111111111111111111111111111111\"}"
    );
    let decoded: UntrustedGate = serde_json::from_str(&encoded).unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded, gate(A));
    assert!(serde_json::from_str::<UntrustedGate>("{\"reason\":\"fork\",\"pull_request\":7,\"head_repository\":\"a\",\"source_commit\":\"x\",\"extra\":1}").is_err());
}

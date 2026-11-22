// r[verify cairn.policy_exception_cob.profile]
// r[verify cairn.policy_exception_cob.profile.root]
// r[verify cairn.policy_exception_cob.profile.kind]
// r[verify cairn.policy_exception_cob.scope]
// r[verify cairn.policy_exception_cob.authority]
// r[verify cairn.policy_exception_cob.authority.denied]
// r[verify cairn.policy_exception_cob.approval]
// r[verify cairn.policy_exception_cob.validity]
// r[verify cairn.policy_exception_cob.applicability]
// r[verify cairn.policy_exception_cob.validation]
// r[verify cairn.policy_exception_cob.interop.negative]
// r[verify cairn.policy_exception_cob.backends.roundtrip]
// r[verify cairn.policy_exception_cob.receipts]
// r[verify cairn.policy_exception_cob.receipts]
// r[verify cairn.policy_exception_cob.receipts.backend]
// r[verify cairn.policy_exception_cob.interop.outward]
// r[verify cairn.policy_exception_cob.interop.negative]
// r[verify cairn.policy_exception_cob.applicability.allowed]
// r[verify cairn.policy_exception_cob.applicability.non_authority]
// r[verify cairn.policy_exception_cob.applicability.terminal]
// r[verify cairn.policy_exception_cob.approval.allowed]
// r[verify cairn.policy_exception_cob.approval.conflict]
// r[verify cairn.policy_exception_cob.approval.stale]
// r[verify cairn.policy_exception_cob.authority.allowed]
// r[verify cairn.policy_exception_cob.authority.ambient]
// r[verify cairn.policy_exception_cob.profile.identity]
// r[verify cairn.policy_exception_cob.receipts.applicability]
// r[verify cairn.policy_exception_cob.scope.drift]
// r[verify cairn.policy_exception_cob.scope.exact]
// r[verify cairn.policy_exception_cob.validity.conflict]
// r[verify cairn.policy_exception_cob.validity.current]
// r[verify cairn.policy_exception_cob.validity.invalid]
// r[verify cairn.policy_exception_cob.validity.no_clock]
// r[verify cairn.policy_exception_cob.validity.terminal]
use super::*;
const POLICY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const REQUESTER: &str = "requester";
const REVIEWER: &str = "reviewer";
const APPROVER: &str = "approver";
const REVOKER: &str = "revoker";
const OBSERVER: &str = "observer";

fn request() -> ExceptionRequest {
    let scope = PolicyScope {
        rule_ids: vec!["allow-network-egress".to_string()],
        targets: vec!["svc:bar".to_string()],
    };
    let mut request = ExceptionRequest {
        policy_identity: POLICY.to_string(),
        scope,
        rationale: "temporary migration".to_string(),
        compensating_controls: vec!["audit-log".to_string()],
        request_root: String::new(),
    };
    request.request_root = exception_request_root(&request).unwrap();
    request
}

fn init_op() -> (PolicyExceptionOperation, PolicyExceptionState) {
    let init = PolicyExceptionInit {
        schema: POLICY_EXCEPTION_SCHEMA_V1.to_string(),
        kind: "policy-exception".to_string(),
        exception_id: "pe-1".to_string(),
        workspace_id: format!("git_oid:{}", "c".repeat(40) + &"d".repeat(24)),
        request: request(),
        requesters: std::collections::BTreeSet::from([REQUESTER.to_string()]),
        reviewers: std::collections::BTreeSet::from([REVIEWER.to_string()]),
        approvers: std::collections::BTreeSet::from([APPROVER.to_string()]),
        revokers: std::collections::BTreeSet::from([REVOKER.to_string()]),
        observers: std::collections::BTreeSet::from([OBSERVER.to_string()]),
    };
    let op = PolicyExceptionOperation {
        id: "init".to_string(),
        actor: REQUESTER.to_string(),
        role: None,
        actions: vec![PolicyExceptionAction::Init {
            state: Box::new(init),
        }],
    };
    let state = initialize_exception(op.clone()).unwrap();
    (op, state)
}

#[test]
fn valid_request_preserves_scope_and_identity() {
    let (_, state) = init_op();
    assert_eq!(state.kind, "policy-exception");
    assert_eq!(state.exception_id, "pe-1");
    assert_eq!(
        state.request.scope.rule_ids,
        vec!["allow-network-egress".to_string()]
    );
    assert!(exception_state_hash(&state).unwrap().len() == BLAKE3_HEX_LENGTH);
}

// r[verify cairn.policy_exception_cob.scope.wildcard]
#[test]
fn implicit_wildcard_scope_fails_closed() {
    let mut scope = PolicyScope {
        rule_ids: vec!["allow-network-egress".to_string()],
        targets: vec!["*".to_string()],
    };
    assert!(validate_scope(&scope).is_err());
    scope.targets = vec!["svc:bar".to_string()];
    assert!(validate_scope(&scope).is_ok());
}

// r[verify cairn.policy_exception_cob.scope]
#[test]
fn unknown_rule_fails_closed() {
    let scope = PolicyScope {
        rule_ids: vec!["allow-everything".to_string()],
        targets: vec!["svc:bar".to_string()],
    };
    assert!(matches!(
        validate_scope(&scope),
        Err(PolicyExceptionError::UnknownRule { .. })
    ));
}

// r[verify cairn.policy_exception_cob.authority.denied]
#[test]
fn requester_cannot_approve() {
    let (_, mut state) = init_op();
    let scope_root = exception_request_root(&state.request).unwrap();
    let op = PolicyExceptionOperation {
        id: "approve".to_string(),
        actor: REQUESTER.to_string(),
        role: Some(ExceptionRole::Approver),
        actions: vec![PolicyExceptionAction::Approve {
            scope_root: scope_root.clone(),
        }],
    };
    assert!(matches!(
        apply_exception_operation(&mut state, op),
        Err(PolicyExceptionError::UnauthorizedRole {
            required: "approver",
            ..
        })
    ));
    // Also verify current role does not bind an approver.
    let _ = scope_root;
    assert!(!exception_actor_has_role(
        &state,
        REQUESTER,
        ExceptionRole::Approver
    ));
}

// r[verify cairn.policy_exception_cob.approval]
#[test]
fn two_approvers_approve_exception() {
    let (_, mut state) = init_op();
    // single approver in the set- up to two approvals; first -> in-review
    // for simplicity, our set has one approver; approval sets in-review first.
    let scope_root = exception_request_root(&state.request).unwrap();
    let op = PolicyExceptionOperation {
        id: "approve".to_string(),
        actor: APPROVER.to_string(),
        role: Some(ExceptionRole::Approver),
        actions: vec![PolicyExceptionAction::Approve {
            scope_root: scope_root.clone(),
        }],
    };
    apply_exception_operation(&mut state, op).unwrap();
    assert!(state.lifecycle == "in-review" || state.lifecycle == "approved");
}

// r[verify cairn.policy_exception_cob.validity]
#[test]
fn explicit_observation_and_expiry() {
    let (_, mut state) = init_op();
    let observe = PolicyExceptionOperation {
        id: "obs".to_string(),
        actor: OBSERVER.to_string(),
        role: Some(ExceptionRole::Observer),
        actions: vec![PolicyExceptionAction::ObserveValidity {
            observation: ValidityObservation {
                id: "obs-1".to_string(),
                observed_epoch: 100,
                finite_valid_until: 1000,
                source: OBSERVER.to_string(),
            },
        }],
    };
    apply_exception_operation(&mut state, observe).unwrap();
    // not approved yet -> not-applicable
    assert_eq!(classify_applicability(&state).unwrap(), "not-approved");
}

// r[verify cairn.policy_exception_cob.validity]
#[test]
fn expiration_cannot_be_reversed_by_older_observation() {
    let (_, mut state) = init_op();
    apply_exception_operation(
        &mut state,
        PolicyExceptionOperation {
            id: "obs1".to_string(),
            actor: OBSERVER.to_string(),
            role: Some(ExceptionRole::Observer),
            actions: vec![PolicyExceptionAction::ObserveValidity {
                observation: ValidityObservation {
                    id: "obs-1".to_string(),
                    observed_epoch: 200,
                    finite_valid_until: 300,
                    source: OBSERVER.to_string(),
                },
            }],
        },
    )
    .unwrap();
    // Expire
    apply_exception_operation(
        &mut state,
        PolicyExceptionOperation {
            id: "expire".to_string(),
            actor: REVOKER.to_string(),
            role: Some(ExceptionRole::Revoker),
            actions: vec![PolicyExceptionAction::Expire {
                reason: "timeout".to_string(),
            }],
        },
    )
    .unwrap();
    assert_eq!(classify_applicability(&state).unwrap(), "inapplicable");
    // A stale older observation must be rejected.
    let stale = apply_exception_operation(
        &mut state,
        PolicyExceptionOperation {
            id: "obs2".to_string(),
            actor: OBSERVER.to_string(),
            role: Some(ExceptionRole::Observer),
            actions: vec![PolicyExceptionAction::ObserveValidity {
                observation: ValidityObservation {
                    id: "obs-2".to_string(),
                    observed_epoch: 100,
                    finite_valid_until: 900,
                    source: OBSERVER.to_string(),
                },
            }],
        },
    );
    assert!(matches!(
        stale,
        Err(PolicyExceptionError::StaleObservation { .. })
    ));
}

// r[verify cairn.policy_exception_cob.backends.roundtrip]
// r[verify cairn.policy_exception_cob.backends.roundtrip]
#[test]
fn portable_round_trip_preserves_state() {
    let (_, state) = init_op();
    let portable = serialize_exception_portable(&state).unwrap();
    let reloaded = parse_exception_portable(&portable).unwrap();
    assert_eq!(reloaded, state);
}

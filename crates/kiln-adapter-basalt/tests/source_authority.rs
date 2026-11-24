//! Positive and negative tests for `BasaltSourceAuthority` (`kiln://source/v1/`).

use std::collections::BTreeSet;

use kiln_adapter_basalt::test_support::{
    self, AUDIENCE, COORDINATOR_SEED, DELEGATE_SEED, EXPIRED_AT_S, EXPIRES_S, GENERATION,
    NOT_BEFORE_S, ROOT_SEED, SourceFixture, VERIFIER_SEED, VERIFY_AT_S,
};
use kiln_adapter_basalt::{
    BasaltSourceAuthority, SourceSigner, encode_snapshot_rule, issue_delegated_source_grant,
};
use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityProfile, AuthorityProvenance,
};
use kiln_core::projection::authority::{
    AttemptFence, MAX_DELEGATION_DEPTH, OriginScope, PathScope, RefEvolution, SOURCE_DIGEST_BYTES,
    SnapshotObservation, SnapshotRule, SourceAuthority, SourceAuthorityRequest, SourceCurrentness,
    SourceDenialReason, SourceViewOperation,
};
use kiln_core::projection::disclosure::{OpaqueHandle, ViewCeiling};
use kiln_core::projection::sharing::{ShareRequest, admit_share};
use kiln_core::projection::{
    GeneratedInputs, Mount, ProjectionDefinition, ProjectionProfile, Selection, SourceCapability,
    TargetProfile, admit_definition,
};
use kiln_core::types::{ContentId, ExternalRef};
use ucan::token::TokenSigner;
use ucan::{
    AudienceDid, CapabilityDocument, CapabilitySet, ProofReferences, TokenTimeBounds,
    issue_token_with_signer,
};

fn digest(label: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(label.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn revision(ch: char) -> ExternalRef {
    let hex = std::iter::repeat_n(ch, 40).collect::<String>();
    ExternalRef::new("git", "commit", &hex).unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn path(value: &str, subtree: bool) -> PathScope {
    PathScope::from_path(value, subtree).unwrap_or_else(|| unreachable!("path {value}"))
}

fn current(rev: &ExternalRef) -> SourceCurrentness {
    SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev.clone(),
            ref_name: Some("refs/heads/main".into()),
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    }
}

fn request(
    repo: &ContentId,
    rev: &ExternalRef,
    operation: SourceViewOperation,
    scope: OriginScope,
    attempt: Option<AttemptFence>,
) -> SourceAuthorityRequest {
    SourceAuthorityRequest {
        operation,
        repository: repo.clone(),
        snapshot: SnapshotRule::Exact {
            revision: rev.clone(),
        },
        view_binding: digest("view-binding"),
        scope,
        request_digest: [9; SOURCE_DIGEST_BYTES],
        attempt,
    }
}

fn signed(signer: &SourceSigner, req: &SourceAuthorityRequest) -> AuthorityProfile {
    AuthorityProfile::VerifiedUcan(
        signer
            .sign(req)
            .unwrap_or_else(|error| unreachable!("{error:?}")),
    )
}

fn ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("{error:?}"))
}

fn err<T: std::fmt::Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => unreachable!("expected Err, got {value:?}"),
        Err(error) => error,
    }
}

fn some<T>(option: Option<T>) -> T {
    option.unwrap_or_else(|| unreachable!("expected Some"))
}

#[test]
fn all_eight_source_operations_admit_and_revalidate_with_exact_scope() {
    let repo = digest("repo");
    let rev = revision('a');
    let cur = current(&rev);
    let attempt = AttemptFence {
        attempt: digest("attempt-1"),
        fence: 3,
    };

    // Non-attempt operations: Discover, ReadContent, ReadHistory, PrepareWorkspace, Delegate, RetrieveResult
    let non_attempt_ops = [
        SourceViewOperation::Discover,
        SourceViewOperation::ReadContent,
        SourceViewOperation::ReadHistory,
        SourceViewOperation::PrepareWorkspace,
        SourceViewOperation::Delegate,
        SourceViewOperation::RetrieveResult,
    ];
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(path("crates/app", true), non_attempt_ops.to_vec())],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let mut authority = fixture.authority();
    for op in non_attempt_ops {
        let scope = if matches!(op, SourceViewOperation::ReadContent) {
            OriginScope::Components {
                path: path("crates/app/src/main.rs", false),
            }
        } else {
            OriginScope::Components {
                path: path("crates/app", true),
            }
        };
        let req = request(&repo, &rev, op, scope, None);
        let decision = authority
            .admit(&req, &signed(&signer, &req), &cur, VERIFY_AT_S)
            .unwrap_or_else(|err| unreachable!("{op:?}: {err:?}"));
        assert!(decision.allowed);
        assert_eq!(decision.provenance, AuthorityProvenance::VerifiedUcan);
        assert_eq!(decision.principal, signer.did());
        let session = decision.session.unwrap_or_else(|| unreachable!());
        let eff = authority
            .effective_scope(session)
            .unwrap_or_else(|| unreachable!());
        // Invariant D: effective_scope contains ONLY the admitted request grant,
        // never sibling operations or unchecked paths.
        assert_eq!(eff.grants().len(), 1);
        assert_eq!(
            eff.grants().first().map(|grant| &grant.operations),
            Some(&BTreeSet::from([op])),
            "{op:?} must not widen to sibling ops"
        );
        authority
            .revalidate(session, &req, &cur, VERIFY_AT_S)
            .unwrap_or_else(|err| unreachable!("{op:?} revalidate: {err:?}"));
    }
    assert_eq!(fixture.replay.admitted(), non_attempt_ops.len());

    // Attempt-bound mutating operations: ModifySource, CreateSource
    let mut_ops = [
        SourceViewOperation::ModifySource,
        SourceViewOperation::CreateSource,
    ];
    let mut_fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        Some(&attempt),
        &[(path("crates/app", true), mut_ops.to_vec())],
        EXPIRES_S,
    );
    let mut_signer = mut_fixture.signer();
    let mut mut_authority = mut_fixture.authority();
    for op in mut_ops {
        let req = request(
            &repo,
            &rev,
            op,
            OriginScope::Components {
                path: path("crates/app/src/lib.rs", false),
            },
            Some(attempt.clone()),
        );
        let decision = mut_authority
            .admit(&req, &signed(&mut_signer, &req), &cur, VERIFY_AT_S)
            .unwrap_or_else(|err| unreachable!("{op:?}: {err:?}"));
        let session = decision.session.unwrap_or_else(|| unreachable!());
        mut_authority
            .revalidate(session, &req, &cur, VERIFY_AT_S)
            .unwrap_or_else(|err| unreachable!("{op:?} revalidate: {err:?}"));
    }
    assert_eq!(mut_fixture.replay.admitted(), 2);
}

#[test]
fn sibling_path_boundary_and_subtree_terminal_never_widen() {
    let repo = digest("repo");
    let rev = revision('b');
    let cur = current(&rev);
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(
            path("crates/app", true),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let mut authority = fixture.authority();

    // `crates/apple/README.md` shares byte prefix `crates/app` but is a sibling!
    let sibling = request(
        &repo,
        &rev,
        SourceViewOperation::ReadContent,
        OriginScope::Components {
            path: path("crates/apple/README.md", false),
        },
        None,
    );
    let denied = err(authority.admit(&sibling, &signed(&signer, &sibling), &cur, VERIFY_AT_S));
    assert_eq!(denied.reason, SourceDenialReason::CredentialDenied);
    // Cheap probe_holder catches it before ReplayLedger!
    assert_eq!(fixture.replay.admitted(), 0);

    // An `s/` terminal grant covers `Discover` at that exact node ONLY,
    // never an entry below it!
    let scope_s = basalt::kiln::SourceScope {
        audience: AUDIENCE,
        generation: GENERATION,
        repository: repo.as_str(),
        attempt: None,
        path: &["crates".into(), "app".into()],
        terminal: Some(basalt::kiln::PathScope::Subtree),
    }
    .prefix()
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let recipient = AudienceDid::from(
        test_support::signer(COORDINATOR_SEED)
            .issuer()
            .unwrap_or_else(|_| unreachable!()),
    );
    let cap_set = ok(CapabilitySet::new(vec![ok(CapabilityDocument::new(
        scope_s,
        "kiln/source/discover".into(),
    ))]));
    let token_s = ok(issue_token_with_signer(
        &test_support::signer(ROOT_SEED),
        &recipient,
        &cap_set,
        &ProofReferences::from_references(vec![]),
        ok(TokenTimeBounds::new(NOT_BEFORE_S, EXPIRES_S)),
    ))
    .as_str()
    .to_string();

    let mut s_fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(path("crates", true), vec![SourceViewOperation::Discover])],
        EXPIRES_S,
    );
    s_fixture.token = token_s;
    let s_signer = s_fixture.signer();
    let mut s_auth = s_fixture.authority();

    // Exact node `crates/app` subtree Discover succeeds:
    let req_node = request(
        &repo,
        &rev,
        SourceViewOperation::Discover,
        OriginScope::Components {
            path: path("crates/app", true),
        },
        None,
    );
    let ok_node = ok(s_auth.admit(&req_node, &signed(&s_signer, &req_node), &cur, VERIFY_AT_S));
    let eff = some(s_auth.effective_scope(some(ok_node.session)));
    assert!(eff.permits_scope(SourceViewOperation::Discover, &path("crates/app", true)));

    // Descendant subtree `crates/app/src` is denied by `s/`!
    let req_desc = request(
        &repo,
        &rev,
        SourceViewOperation::Discover,
        OriginScope::Components {
            path: path("crates/app/src", true),
        },
        None,
    );
    let err_desc = err(s_auth.admit(&req_desc, &signed(&s_signer, &req_desc), &cur, VERIFY_AT_S));
    assert_eq!(err_desc.reason, SourceDenialReason::CredentialDenied);
}

#[test]
fn cheap_checks_and_handle_refusal_never_burn_replay() {
    let repo = digest("repo");
    let rev = revision('c');
    let cur = current(&rev);
    let attempt = AttemptFence {
        attempt: digest("attempt-1"),
        fence: 2,
    };
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        Some(&attempt),
        &[(
            path("crates/app", true),
            vec![
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
            ],
        )],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let mut authority = fixture.authority();

    // 1. OriginScope::Handle -> HandleConfersNothing (before replay!)
    let valid_req = request(
        &repo,
        &rev,
        SourceViewOperation::ReadContent,
        OriginScope::Components {
            path: path("crates/app/lib.rs", false),
        },
        Some(attempt.clone()),
    );
    let valid_cred = signed(&signer, &valid_req);
    let handle_req = request(
        &repo,
        &rev,
        SourceViewOperation::ReadContent,
        OriginScope::Handle {
            handle: some(OpaqueHandle::from_hex(&"ab".repeat(32))),
        },
        Some(attempt.clone()),
    );
    assert_eq!(
        err(authority.admit(&handle_req, &valid_cred, &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::HandleConfersNothing
    );

    // 2. Missing AttemptFence on ModifySource -> RequestInvalid (before replay!)
    let no_attempt_req = request(
        &repo,
        &rev,
        SourceViewOperation::ModifySource,
        OriginScope::Components {
            path: path("crates/app/lib.rs", false),
        },
        None,
    );
    assert_eq!(
        err(authority.admit(&no_attempt_req, &valid_cred, &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::RequestInvalid
    );

    // 3. Cancelled / Superseded AttemptCurrentness -> AttemptNotCurrent (before replay!)
    let mut cancelled = cur.clone();
    cancelled.attempt = AttemptCurrentness::Cancelled;
    assert_eq!(
        err(authority.admit(&valid_req, &valid_cred, &cancelled, VERIFY_AT_S)).reason,
        SourceDenialReason::AttemptNotCurrent
    );

    // 4. Stale generation -> StaleGeneration (before replay!)
    let mut stale_gen = cur.clone();
    stale_gen.generation += 1;
    assert_eq!(
        err(authority.admit(&valid_req, &valid_cred, &stale_gen, VERIFY_AT_S)).reason,
        SourceDenialReason::StaleGeneration
    );

    // 5. Snapshot mismatch -> SnapshotNotAdmitted (before replay!)
    let wrong_rev = current(&revision('d'));
    assert_eq!(
        err(authority.admit(&valid_req, &valid_cred, &wrong_rev, VERIFY_AT_S)).reason,
        SourceDenialReason::SnapshotNotAdmitted
    );

    // 6. LocalOperator & Missing -> VerifierUnavailable / MissingEvidence
    assert_eq!(
        err(authority.admit(
            &valid_req,
            &AuthorityProfile::LocalOperator {
                operator: "op".into()
            },
            &cur,
            VERIFY_AT_S
        ))
        .reason,
        SourceDenialReason::VerifierUnavailable
    );
    assert_eq!(
        err(authority.admit(&valid_req, &AuthorityProfile::Missing, &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::MissingEvidence
    );

    // None of those 7 denials burned replay! Now the valid credential still succeeds once:
    assert_eq!(fixture.replay.calls(), 0);
    assert_eq!(fixture.replay.admitted(), 0);
    ok(authority.admit(&valid_req, &valid_cred, &cur, VERIFY_AT_S));
    assert_eq!(fixture.replay.admitted(), 1);

    // Invariant C: replaying the EXACT SAME credential on the SAME adapter
    // instance MUST fail replay (no admit-level session cache)!
    let replayed = err(authority.admit(&valid_req, &valid_cred, &cur, VERIFY_AT_S));
    assert_eq!(replayed.reason, SourceDenialReason::CredentialDenied);
    assert_eq!(fixture.replay.calls(), 2);
    assert_eq!(fixture.replay.admitted(), 1);
}

#[test]
fn delegation_depth_attenuation_and_escalation_refusal() {
    let repo = digest("repo");
    let rev = revision('e');
    let cur = current(&rev);

    // Root delegates crates/app/** with Discover, ReadContent, Delegate to DELEGATE_SEED,
    // which delegates crates/app/src/** with ReadContent ONLY to COORDINATOR_SEED.
    let fixture = SourceFixture::delegated(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(
            path("crates/app", true),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::Delegate,
            ],
        )],
        &[(
            path("crates/app/src", true),
            vec![SourceViewOperation::ReadContent],
        )],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let mut authority = fixture.authority();

    let allowed_req = request(
        &repo,
        &rev,
        SourceViewOperation::ReadContent,
        OriginScope::Components {
            path: path("crates/app/src/main.rs", false),
        },
        None,
    );
    let dec = ok(authority.admit(
        &allowed_req,
        &signed(&signer, &allowed_req),
        &cur,
        VERIFY_AT_S,
    ));
    let eff = some(authority.effective_scope(some(dec.session)));
    // Chain depth is 1, so remaining depth = MAX_DELEGATION_DEPTH - 1.
    assert_eq!(eff.depth(), MAX_DELEGATION_DEPTH - 1);

    // Escalation: child tries to claim Discover (which parent held, child did not)
    let esc_op = request(
        &repo,
        &rev,
        SourceViewOperation::Discover,
        OriginScope::Components {
            path: path("crates/app/src", true),
        },
        None,
    );
    assert_eq!(
        err(authority.admit(&esc_op, &signed(&signer, &esc_op), &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::CredentialDenied
    );

    // Escalation: child issues itself a token covering crates/app/Cargo.toml
    // (outside child's crates/app/src/ grant; also if delegate had no Delegate
    // ability, Basalt rejects!).
    let no_del_fixture = SourceFixture::delegated(
        VERIFIER_SEED,
        &repo,
        None,
        &[(
            path("crates/app", true),
            vec![SourceViewOperation::ReadContent], // missing Delegate!
        )],
        &[(
            path("crates/app/src", true),
            vec![SourceViewOperation::ReadContent],
        )],
        EXPIRES_S,
    );
    let no_del_signer = no_del_fixture.signer();
    let mut no_del_auth = no_del_fixture.authority();
    assert_eq!(
        err(no_del_auth.admit(
            &allowed_req,
            &signed(&no_del_signer, &allowed_req),
            &cur,
            VERIFY_AT_S
        ))
        .reason,
        SourceDenialReason::CredentialDenied
    );

    // SharePlan -> real delegated UCAN via issue_delegated_source_grant
    let def = ok(admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "def".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["crates/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    }));
    let ceiling = ViewCeiling::from_definition(&def);

    // First admit Delegate on crates/app for DELEGATE_SEED
    let del_fixture = SourceFixture::direct(
        DELEGATE_SEED,
        &repo,
        None,
        &[(
            path("crates/app", true),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::Delegate,
            ],
        )],
        EXPIRES_S,
    );
    let del_signer = del_fixture.signer();
    let mut del_auth = del_fixture.authority();
    let del_req = request(
        &repo,
        &rev,
        SourceViewOperation::Delegate,
        OriginScope::Components {
            path: path("crates/app", true),
        },
        None,
    );
    let del_dec = ok(del_auth.admit(&del_req, &signed(&del_signer, &del_req), &cur, VERIFY_AT_S));
    let del_scope = some(del_auth.effective_scope(some(del_dec.session))).clone();
    // Join with Discover/ReadContent scope to share ReadContent:
    let read_req = request(
        &repo,
        &rev,
        SourceViewOperation::ReadContent,
        OriginScope::Components {
            path: path("crates/app/src/main.rs", false),
        },
        None,
    );
    let read_dec = ok(del_auth.admit(
        &read_req,
        &signed(&del_signer, &read_req),
        &cur,
        VERIFY_AT_S,
    ));
    let read_scope = some(del_auth.effective_scope(some(read_dec.session)));
    let joined = ok(
        kiln_core::projection::authority::EffectiveSourceScope::join(&[
            del_scope.clone(),
            read_scope.clone(),
        ]),
    );
    let share_plan = ok(admit_share(
        &joined,
        &ceiling,
        &ShareRequest {
            recipient: test_support::did(VERIFIER_SEED),
            paths: vec![path("crates/app/src/main.rs", false)],
            operations: BTreeSet::from([SourceViewOperation::ReadContent]),
            snapshot: SnapshotRule::Exact {
                revision: rev.clone(),
            },
            not_after_s: EXPIRES_S - 100,
            depth: 0,
        },
        VERIFY_AT_S,
    ));
    let child_token = ok(issue_delegated_source_grant(
        &share_plan,
        test_support::seed(DELEGATE_SEED),
        AUDIENCE,
        None,
        &[&del_fixture.token],
        NOT_BEFORE_S,
    ));
    let mut child_fixture = SourceFixture::direct(
        VERIFIER_SEED,
        &repo,
        None,
        &[(path("crates", true), vec![SourceViewOperation::ReadContent])],
        EXPIRES_S,
    );
    child_fixture.token = child_token;
    child_fixture.proofs = vec![del_fixture.token];
    let child_signer = child_fixture.signer();
    let mut child_auth = child_fixture.authority();
    let child_dec = ok(child_auth.admit(
        &read_req,
        &signed(&child_signer, &read_req),
        &cur,
        VERIFY_AT_S,
    ));
    assert!(child_dec.allowed);
    let child_eff = some(child_auth.effective_scope(some(child_dec.session)));
    assert_eq!(child_eff.depth(), MAX_DELEGATION_DEPTH - 1);
}

#[test]
fn revocation_stale_view_expiry_and_revalidation_denials() {
    let repo = digest("repo");
    let rev = revision('f');
    let cur = current(&rev);
    let fixture = SourceFixture::delegated(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(
            path("crates/app", true),
            vec![
                SourceViewOperation::ReadContent,
                SourceViewOperation::Delegate,
            ],
        )],
        &[(
            path("crates/app/src", true),
            vec![SourceViewOperation::ReadContent],
        )],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let mut authority = fixture.authority();
    let req = request(
        &repo,
        &rev,
        SourceViewOperation::ReadContent,
        OriginScope::Components {
            path: path("crates/app/src/main.rs", false),
        },
        None,
    );
    let dec = ok(authority.admit(&req, &signed(&signer, &req), &cur, VERIFY_AT_S));
    let session = some(dec.session);

    // 1. Revocation generation bump denies revalidate & discards session
    let mut bumped = cur.clone();
    bumped.revocation_generation = 1;
    assert_eq!(
        err(authority.revalidate(session, &req, &bumped, VERIFY_AT_S)).reason,
        SourceDenialReason::Revoked
    );
    assert!(authority.effective_scope(session).is_none());

    // 2. Revoking parent proof denies both revalidate and fresh admit
    let dec2 = ok(authority.admit(&req, &signed(&signer, &req), &cur, VERIFY_AT_S));
    let session2 = some(dec2.session);
    fixture
        .revocations
        .revoke(some(fixture.proofs.first()).as_str());
    assert_eq!(
        err(authority.revalidate(session2, &req, &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::Revoked
    );
    assert_eq!(
        err(authority.admit(&req, &signed(&signer, &req), &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::Revoked
    );

    // 3. Stale revocation view denies
    let fresh_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(path("crates", true), vec![SourceViewOperation::ReadContent])],
        EXPIRES_S,
    );
    let fresh_signer = fresh_fix.signer();
    let mut fresh_auth = fresh_fix.authority();
    fresh_fix.revocations.set_stale(true);
    assert_eq!(
        err(fresh_auth.admit(&req, &signed(&fresh_signer, &req), &cur, VERIFY_AT_S)).reason,
        SourceDenialReason::Revoked
    );
    fresh_fix.revocations.set_stale(false);
    let s3 =
        some(ok(fresh_auth.admit(&req, &signed(&fresh_signer, &req), &cur, VERIFY_AT_S)).session);
    assert_eq!(
        err(fresh_auth.revalidate(s3, &req, &cur, EXPIRED_AT_S)).reason,
        SourceDenialReason::Expired
    );
}

#[test]
fn snapshot_rule_encoding_is_bounded_ascii_and_injective() {
    let r1 = SnapshotRule::Exact {
        revision: revision('1'),
    };
    let r2 = SnapshotRule::Exact {
        revision: revision('2'),
    };
    let r3 = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::FastForward,
    };
    let r4 = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::Rewrite,
    };
    let encoded = [&r1, &r2, &r3, &r4].map(encode_snapshot_rule);
    for value in &encoded {
        assert!(!value.is_empty() && value.len() <= 512);
        assert!(value.is_ascii() && value.trim() == value);
        assert!(!value.bytes().any(|b| b.is_ascii_control()));
    }
    let unique: BTreeSet<&String> = encoded.iter().collect();
    assert_eq!(unique.len(), 4);

    // Ref evolution check in BasaltSourceAuthority
    let repo = digest("repo");
    let rev1 = revision('1');
    let rev2 = revision('2');
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(path("crates", true), vec![SourceViewOperation::ReadContent])],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let mut authority = fixture.authority();
    let mut req = request(
        &repo,
        &rev1,
        SourceViewOperation::ReadContent,
        OriginScope::Components {
            path: path("crates/app/main.rs", false),
        },
        None,
    );
    req.snapshot = r3;
    let cur1 = current(&rev1);
    let session =
        some(ok(authority.admit(&req, &signed(&signer, &req), &cur1, VERIFY_AT_S)).session);
    // Moving ref invalidates existing session on revalidate
    let cur2 = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev2,
            ref_name: Some("refs/heads/main".into()),
            previous: Some(rev1),
            fast_forward: true,
        },
        attempt: AttemptCurrentness::Current,
    };
    assert_eq!(
        err(authority.revalidate(session, &req, &cur2, VERIFY_AT_S)).reason,
        SourceDenialReason::SnapshotNotAdmitted
    );
    // Fresh admit under fast-forward succeeds
    ok(authority.admit(&req, &signed(&signer, &req), &cur2, VERIFY_AT_S));
    // Non-fast-forward rewrite under FastForward rule is refused
    let mut non_ff = cur2;
    non_ff.snapshot.fast_forward = false;
    assert_eq!(
        err(authority.admit(&req, &signed(&signer, &req), &non_ff, VERIFY_AT_S)).reason,
        SourceDenialReason::SnapshotNotAdmitted
    );
}

#[test]
fn max_delegation_depth_above_eight_is_refused() {
    let repo = digest("repo");
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        &repo,
        None,
        &[(path("crates", true), vec![SourceViewOperation::Discover])],
        EXPIRES_S,
    );
    assert!(
        BasaltSourceAuthority::verified(
            fixture.config(),
            fixture.services(),
            MAX_DELEGATION_DEPTH + 1
        )
        .is_err()
    );
}

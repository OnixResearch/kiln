use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::*;
use crate::projection::disclosure::OpaqueHandle;
use crate::projection::restricted_corpus::{
    Corpus, FakeGrant, FakeVerifiedSourceAuthority, GENERATION, digest, ok, verified,
};

fn ops(operations: &[SourceViewOperation]) -> BTreeSet<SourceViewOperation> {
    operations.iter().copied().collect()
}

fn path(value: &str, subtree: bool) -> PathScope {
    PathScope::from_path(value, subtree).unwrap_or_else(|| unreachable!("valid scope {value}"))
}

fn request(
    corpus: &Corpus,
    operation: SourceViewOperation,
    scope: OriginScope,
) -> SourceAuthorityRequest {
    SourceAuthorityRequest {
        operation,
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        view_binding: digest("view-binding"),
        scope,
        request_digest: [3; SOURCE_DIGEST_BYTES],
        attempt: None,
    }
}

fn components(value: &str) -> OriginScope {
    OriginScope::Components {
        path: path(value, false),
    }
}

fn local(operations: &[SourceViewOperation], corpus: &Corpus) -> LocalSourceAuthority {
    ok(LocalSourceAuthority::new(
        "operator@host",
        LocalSourceGrant {
            repositories: [corpus.repository().identity().clone()]
                .into_iter()
                .collect(),
            operations: ops(operations),
        },
    ))
}

fn denied<T: core::fmt::Debug>(result: Result<T, SourceAuthorityDecision>) -> SourceDenialReason {
    match result {
        Ok(value) => unreachable!("unexpectedly admitted: {value:?}"),
        Err(decision) => {
            assert!(!decision.allowed);
            assert!(decision.session.is_none() && decision.scope_identity.is_none());
            assert!(decision.principal.is_empty());
            decision.reason
        }
    }
}

// r[verify kiln.source.projection.authority.vocabulary.scenario.closed]
#[test]
fn coverage_is_closed_unique_and_namespaced() {
    assert_eq!(
        SOURCE_AUTHORITY_COVERAGE_SCHEMA,
        "kiln.source-authority-coverage.v1"
    );
    ok(validate_source_coverage(&SOURCE_COVERAGE));
    let abilities: Vec<&str> = SourceViewOperation::ALL
        .iter()
        .map(|operation| source_coverage(*operation).ability)
        .collect();
    assert_eq!(
        abilities,
        [
            "kiln/source/discover",
            "kiln/source/read",
            "kiln/source/history",
            "kiln/source/prepare",
            "kiln/source/modify",
            "kiln/source/create",
            "kiln/source/delegate",
            "kiln/source/result",
        ]
    );
    for operation in SourceViewOperation::ALL {
        assert_eq!(
            operation_for_ability(source_coverage(operation).ability),
            Some(operation)
        );
    }
    // Submission and publication stay protected implementation operations.
    for unknown in [
        "kiln/source/publish",
        "kiln/source/submit",
        "kiln/source/*",
        "",
    ] {
        assert_eq!(operation_for_ability(unknown), None, "{unknown}");
    }
    assert_eq!(
        REUSED_PROTECTED_OPERATIONS,
        [
            ProtectedOperation::CandidateSubmit,
            ProtectedOperation::PublicationPublish
        ]
    );

    let mut duplicate = SOURCE_COVERAGE;
    duplicate[1] = duplicate[0];
    assert_eq!(
        ok_err(validate_source_coverage(&duplicate)),
        "source_coverage_operation"
    );
    let missing: Vec<SourceCoverageEntry> = SOURCE_COVERAGE.iter().copied().skip(1).collect();
    assert_eq!(
        ok_err(validate_source_coverage(&missing)),
        "source_coverage_operation"
    );
    let mut foreign = SOURCE_COVERAGE;
    foreign[0].ability = "kiln/implementation/discover";
    assert_eq!(
        ok_err(validate_source_coverage(&foreign)),
        "source_coverage_namespace"
    );
    let mut shared = SOURCE_COVERAGE;
    shared[1].ability = shared[0].ability;
    assert_eq!(
        ok_err(validate_source_coverage(&shared)),
        "source_coverage_ability"
    );
    let mut attemptless = SOURCE_COVERAGE;
    attemptless[4].attempt = AttemptRule::Optional;
    assert_eq!(
        ok_err(validate_source_coverage(&attemptless)),
        "source_coverage_attempt"
    );
}

fn ok_err(result: CoreResult<()>) -> String {
    match result {
        Ok(()) => unreachable!("inconsistent coverage was accepted"),
        Err(diagnostic) => diagnostic.code,
    }
}

// r[verify kiln.source.projection.authority.vocabulary.scenario.no_implication]
#[test]
fn operations_never_imply_each_other() {
    let corpus = Corpus::load();
    for granted in SourceViewOperation::ALL {
        let scope = corpus.scope_with(
            vec![SourceGrant {
                path: PathScope::root(),
                operations: ops(&[granted]),
            }],
            "did:key:z6MkSingle",
            &corpus.revision,
        );
        for asked in SourceViewOperation::ALL {
            assert_eq!(
                scope.permits(asked, "crates/app/src/main.rs"),
                asked == granted,
                "{granted:?} must not imply {asked:?}"
            );
        }
    }
    // A reader holds read and history, never share, edit or publish.
    let reader = corpus.scope("project-reader");
    let main = "crates/app/src/main.rs";
    assert!(reader.permits(SourceViewOperation::ReadContent, main));
    for withheld in [
        SourceViewOperation::Delegate,
        SourceViewOperation::ModifySource,
        SourceViewOperation::CreateSource,
        SourceViewOperation::PrepareWorkspace,
        SourceViewOperation::RetrieveResult,
    ] {
        assert!(!reader.permits(withheld, main), "{withheld:?}");
    }
}

// r[verify kiln.source.projection.authority.scope.scenario.components]
#[test]
fn path_scope_is_framed_per_component() {
    let app = path("crates/app", true);
    assert!(app.covers_path("crates/app"));
    assert!(app.covers_path("crates/app/src/main.rs"));
    assert!(!app.covers_path("crates/apple/README.md"));
    assert!(!app.covers_path("crates/ap"));
    assert!(!app.covers_path("crates"));
    let exact = path("crates/app/Cargo.toml", false);
    assert!(exact.covers_path("crates/app/Cargo.toml"));
    assert!(!exact.covers_path("crates/app/Cargo.toml/x"));
    assert!(!exact.covers_path("crates/app/Cargo.tom"));
    assert!(PathScope::root().covers_path("docs/guide.md"));
    assert!(!PathScope::root().covers_path(""));

    assert!(app.contains(&exact));
    assert!(app.contains(&path("crates/app/src", true)));
    assert!(!exact.contains(&app));
    assert!(!app.contains(&path("crates/apple", true)));
    assert!(!exact.contains(&path("crates/app/Cargo.toml", true)));
    assert!(PathScope::root().contains(&app));

    for unsafe_path in [
        "",
        "crates/../secret",
        "./crates",
        "crates//app",
        "crates/*",
        "a\\b",
    ] {
        assert!(
            PathScope::from_path(unsafe_path, false).is_none(),
            "{unsafe_path:?}"
        );
    }
    assert!(PathScope::from_path("", true).is_some());
    let mut slashed = app.clone();
    slashed.components = vec!["crates/app".into()];
    assert!(slashed.validate().is_err());

    assert_eq!(
        PathScope::cairn_change("app-feature"),
        Some(path(".cairn/changes/app-feature", true))
    );
    for invalid in ["", "..", "a/b"] {
        assert!(PathScope::cairn_change(invalid).is_none(), "{invalid:?}");
    }
}

// r[verify kiln.source.projection.authority.request.scenario.binding]
#[test]
fn request_validates_attempt_rule_and_binds_every_field() {
    let corpus = Corpus::load();
    let base = request(
        &corpus,
        SourceViewOperation::ReadContent,
        components("crates/app/src/main.rs"),
    );
    ok(base.validate());
    let mut modify = base.clone();
    modify.operation = SourceViewOperation::ModifySource;
    assert!(modify.validate().is_err(), "mutation needs an attempt");
    modify.attempt = Some(AttemptFence {
        attempt: digest("attempt"),
        fence: 0,
    });
    assert!(modify.validate().is_err(), "a bound attempt needs a fence");
    modify.attempt = Some(AttemptFence {
        attempt: digest("attempt"),
        fence: 4,
    });
    ok(modify.validate());

    type Change = fn(&mut SourceAuthorityRequest);
    let changes: [Change; 8] = [
        |r| r.operation = SourceViewOperation::ReadHistory,
        |r| r.repository = digest("other-repository"),
        |r| {
            r.snapshot = SnapshotRule::Ref {
                name: "refs/heads/main".into(),
                evolution: RefEvolution::FastForward,
            }
        },
        |r| r.view_binding = digest("other-binding"),
        |r| r.scope = components("crates/app/src/util.rs"),
        |r| {
            r.scope = OriginScope::Handle {
                handle: OpaqueHandle::from_hex(&"ab".repeat(32))
                    .unwrap_or_else(|| unreachable!("valid handle")),
            }
        },
        |r| r.request_digest = [4; SOURCE_DIGEST_BYTES],
        |r| {
            r.attempt = Some(AttemptFence {
                attempt: digest("attempt"),
                fence: 1,
            })
        },
    ];
    let mut seen = BTreeSet::from([base.digest_hex()]);
    for change in changes {
        let mut changed = base.clone();
        change(&mut changed);
        assert!(seen.insert(changed.digest_hex()), "{changed:?}");
    }
}

// r[verify kiln.source.projection.authority.port.scenario.evolution]
#[test]
fn snapshot_rules_admit_only_their_evolution() {
    let corpus = Corpus::load();
    let first = corpus.revision.clone();
    let second = ok(ExternalRef::new("git", "commit", &"2".repeat(40)));
    let observed =
        |revision: &ExternalRef, previous: Option<&ExternalRef>, ff: bool| SnapshotObservation {
            revision: revision.clone(),
            ref_name: Some("refs/heads/main".into()),
            previous: previous.cloned(),
            fast_forward: ff,
        };
    let exact = SnapshotRule::Exact {
        revision: first.clone(),
    };
    let follow = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::FastForward,
    };
    let rewrite = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::Rewrite,
    };
    assert!(exact.admits(&observed(&first, None, false)));
    assert!(!exact.admits(&observed(&second, Some(&first), true)));
    assert!(follow.admits(&observed(&second, Some(&first), true)));
    assert!(!follow.admits(&observed(&second, Some(&first), false)));
    assert!(rewrite.admits(&observed(&second, Some(&first), false)));
    let mut other_ref = observed(&second, None, true);
    other_ref.ref_name = Some("refs/heads/other".into());
    assert!(!follow.admits(&other_ref));

    assert!(exact.within(&exact, &first));
    assert!(exact.within(&follow, &first));
    assert!(!exact.within(&follow, &second));
    assert!(follow.within(&rewrite, &first));
    assert!(!rewrite.within(&follow, &first));
    assert!(!follow.within(&exact, &first));
    let tag = SnapshotRule::Ref {
        name: "refs/tags/v2".into(),
        evolution: RefEvolution::FastForward,
    };
    assert!(!tag.within(&follow, &first));
}

// r[verify kiln.source.projection.authority.compatibility.scenario.explicit]
#[test]
fn local_profile_needs_an_explicit_operator_grant() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let current = corpus.current();
    let now = corpus.now_s;
    let operator = AuthorityProfile::LocalOperator {
        operator: "operator@host".into(),
    };
    let mut authority = local(&[Op::Discover, Op::ReadContent], &corpus);
    let read = request(
        &corpus,
        Op::ReadContent,
        components("crates/secret/src/key.rs"),
    );
    let decision = ok(authority.admit(&read, &operator, &current, now));
    assert!(decision.allowed);
    assert_eq!(decision.provenance, AuthorityProvenance::LocalOperator);
    assert_eq!(decision.ability, "kiln/source/read");
    let session = decision
        .session
        .unwrap_or_else(|| unreachable!("allowed decisions hold a session"));
    let scope = authority
        .effective_scope(session)
        .cloned()
        .unwrap_or_else(|| unreachable!("live session"));
    // The session holds the admitted request only, not the whole grant.
    assert_eq!(scope.grants().len(), 1);
    assert!(scope.permits(Op::ReadContent, "crates/secret/src/key.rs"));
    assert!(!scope.permits(Op::ReadContent, "docs/guide.md"));
    assert!(!scope.permits(Op::Discover, "crates/secret/src/key.rs"));
    // A root discovery is admitted on its own.
    let browse = request(
        &corpus,
        Op::Discover,
        OriginScope::Components {
            path: PathScope::root(),
        },
    );
    let browsing = ok(authority.admit(&browse, &operator, &current, now))
        .session
        .unwrap_or_else(|| unreachable!("allowed decisions hold a session"));
    let root = authority
        .effective_scope(browsing)
        .unwrap_or_else(|| unreachable!("live session"));
    assert!(root.permits(Op::Discover, "docs/guide.md"));
    assert!(!root.permits(Op::Delegate, "docs/guide.md"));
    assert_eq!(decision.scope_identity.as_ref(), Some(scope.identity()));
    ok(authority.revalidate(session, &read, &current, now));

    assert_eq!(
        denied(authority.admit(&read, &AuthorityProfile::Missing, &current, now)),
        SourceDenialReason::MissingEvidence
    );
    assert_eq!(
        denied(authority.admit(&read, &verified("did:key:z6MkMaintainer"), &current, now)),
        SourceDenialReason::VerifierUnavailable
    );
    let other = AuthorityProfile::LocalOperator {
        operator: "viewer".into(),
    };
    assert_eq!(
        denied(authority.admit(&read, &other, &current, now)),
        SourceDenialReason::PrincipalMismatch
    );
    let mut foreign = read.clone();
    foreign.repository = digest("other-repository");
    assert_eq!(
        denied(authority.admit(&foreign, &operator, &current, now)),
        SourceDenialReason::RepositoryOutOfScope
    );
    let history = request(&corpus, Op::ReadHistory, components("docs/guide.md"));
    assert_eq!(
        denied(authority.admit(&history, &operator, &current, now)),
        SourceDenialReason::OperationNotGranted
    );
    let handle = request(
        &corpus,
        Op::ReadContent,
        OriginScope::Handle {
            handle: OpaqueHandle::from_hex(&"cd".repeat(32))
                .unwrap_or_else(|| unreachable!("valid handle")),
        },
    );
    assert_eq!(
        denied(authority.admit(&handle, &operator, &current, now)),
        SourceDenialReason::HandleConfersNothing
    );
    let decisions = authority.decisions();
    assert!(decisions.len() >= 7);
    let log = ok(serde_json::to_string(decisions));
    assert!(!log.contains("fake.compact.token"));

    for grant in [
        LocalSourceGrant {
            repositories: BTreeSet::new(),
            operations: ops(&[Op::Discover]),
        },
        LocalSourceGrant {
            repositories: [digest("repository")].into_iter().collect(),
            operations: BTreeSet::new(),
        },
        LocalSourceGrant {
            repositories: [digest("repository")].into_iter().collect(),
            operations: ops(&[Op::Discover, Op::Delegate]),
        },
    ] {
        assert!(LocalSourceAuthority::new("operator@host", grant).is_err());
    }
    assert!(
        LocalSourceAuthority::new(
            " ",
            LocalSourceGrant {
                repositories: [digest("repository")].into_iter().collect(),
                operations: ops(&[Op::Discover]),
            }
        )
        .is_err()
    );
}

// r[verify kiln.source.projection.authority.port.scenario.verified]
#[test]
fn fake_verified_port_scopes_each_holder() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let current = corpus.current();
    let now = corpus.now_s;
    let mut port = FakeVerifiedSourceAuthority::default();
    let agent = &corpus.view("agent").holder;
    port.grant(
        &agent.audience,
        FakeGrant {
            repository: corpus.repository().identity().clone(),
            snapshot: SnapshotRule::Exact {
                revision: corpus.revision.clone(),
            },
            grants: agent.grants.clone(),
            not_after_s: agent.not_after_s,
            depth: agent.depth,
            generation: crate::projection::restricted_corpus::GENERATION,
        },
    );
    let profile = verified(&agent.audience);
    let main = request(
        &corpus,
        Op::ReadContent,
        components("crates/app/src/main.rs"),
    );
    let decision = ok(port.admit(&main, &profile, &current, now));
    assert_eq!(decision.provenance, AuthorityProvenance::VerifiedUcan);
    assert_eq!(decision.principal, agent.audience);

    let shared = request(
        &corpus,
        Op::ReadContent,
        components("crates/shared/src/lib.rs"),
    );
    assert_eq!(
        denied(port.admit(&shared, &profile, &current, now)),
        SourceDenialReason::ResourceOutOfScope
    );
    let apple = request(&corpus, Op::Discover, components("crates/apple/README.md"));
    assert_eq!(
        denied(port.admit(&apple, &profile, &current, now)),
        SourceDenialReason::ResourceOutOfScope
    );
    let share = request(
        &corpus,
        Op::Delegate,
        OriginScope::Components {
            path: path("crates/app", true),
        },
    );
    assert_eq!(
        denied(port.admit(&share, &profile, &current, now)),
        SourceDenialReason::OperationNotGranted
    );
    let history = request(
        &corpus,
        Op::ReadHistory,
        components("crates/app/src/main.rs"),
    );
    assert_eq!(
        denied(port.admit(&history, &profile, &current, now)),
        SourceDenialReason::OperationNotGranted
    );
    let mut edit = request(
        &corpus,
        Op::ModifySource,
        components("crates/app/src/main.rs"),
    );
    assert_eq!(
        denied(port.admit(&edit, &profile, &current, now)),
        SourceDenialReason::RequestInvalid
    );
    edit.attempt = Some(AttemptFence {
        attempt: digest("attempt"),
        fence: 2,
    });
    ok(port.admit(&edit, &profile, &current, now));
    let mut stale_attempt = current.clone();
    stale_attempt.attempt = AttemptCurrentness::Superseded;
    assert_eq!(
        denied(port.admit(&edit, &profile, &stale_attempt, now)),
        SourceDenialReason::AttemptNotCurrent
    );
    assert_eq!(
        denied(port.admit(&main, &profile, &current, agent.not_after_s)),
        SourceDenialReason::Expired
    );
    assert_eq!(
        denied(port.admit(&main, &verified("did:key:z6MkStranger"), &current, now)),
        SourceDenialReason::CredentialDenied
    );
}

// r[verify kiln.source.projection.authority.port.scenario.revalidate]
#[test]
fn revalidation_denies_revocation_generation_and_moved_snapshots() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let reader = &corpus.view("project-reader").holder;
    let follow = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::FastForward,
    };
    let mut port = FakeVerifiedSourceAuthority::default();
    port.grant(
        &reader.audience,
        FakeGrant {
            repository: corpus.repository().identity().clone(),
            snapshot: follow.clone(),
            grants: reader.grants.clone(),
            not_after_s: reader.not_after_s,
            depth: reader.depth,
            generation: crate::projection::restricted_corpus::GENERATION,
        },
    );
    let profile = verified(&reader.audience);
    let mut current = corpus.current();
    current.snapshot.ref_name = Some("refs/heads/main".into());
    let mut live = request(
        &corpus,
        Op::ReadContent,
        components("crates/app/src/main.rs"),
    );
    live.snapshot = follow.clone();
    let admit = |port: &mut FakeVerifiedSourceAuthority, current: &SourceCurrentness| {
        ok(port.admit(&live, &profile, current, now))
            .session
            .unwrap_or_else(|| unreachable!("session"))
    };

    let session = admit(&mut port, &current);
    let first_scope = port
        .effective_scope(session)
        .unwrap_or_else(|| unreachable!("live"))
        .identity()
        .clone();
    ok(port.revalidate(session, &live, &current, now));

    let mut revoked = current.clone();
    revoked.revocation_generation += 1;
    assert_eq!(
        denied(port.revalidate(session, &live, &revoked, now)),
        SourceDenialReason::Revoked
    );
    assert!(port.effective_scope(session).is_none(), "denial discards");

    let session = admit(&mut port, &current);
    let mut regenerated = current.clone();
    regenerated.generation += 1;
    assert_eq!(
        denied(port.revalidate(session, &live, &regenerated, now)),
        SourceDenialReason::StaleGeneration
    );

    // The branch moves: the old session never follows it silently.
    let session = admit(&mut port, &current);
    let second = ok(ExternalRef::new("git", "commit", &"2".repeat(40)));
    let mut moved = current.clone();
    moved.snapshot.previous = Some(corpus.revision.clone());
    moved.snapshot.revision = second.clone();
    moved.snapshot.fast_forward = true;
    assert_eq!(
        denied(port.revalidate(session, &live, &moved, now)),
        SourceDenialReason::SnapshotNotAdmitted
    );
    // A fresh admission along the admitted evolution gets a new scope.
    let refreshed = admit(&mut port, &moved);
    let scope = port
        .effective_scope(refreshed)
        .unwrap_or_else(|| unreachable!("live"));
    assert_eq!(scope.revision(), &second);
    assert_ne!(scope.identity(), &first_scope);
    // A rewrite is outside fast-forward evolution.
    let mut rewritten = moved.clone();
    rewritten.snapshot.fast_forward = false;
    assert_eq!(
        denied(port.admit(&live, &profile, &rewritten, now)),
        SourceDenialReason::SnapshotNotAdmitted
    );

    port.revoke(&reader.audience);
    assert_eq!(
        denied(port.revalidate(refreshed, &live, &moved, now)),
        SourceDenialReason::Revoked
    );
    assert_eq!(
        denied(port.admit(&live, &profile, &moved, now)),
        SourceDenialReason::Revoked
    );

    // A session admits one exact request; another payload is a new request.
    let mut fresh = port_without_revocation(&corpus, &follow);
    let fresh_session = admit(&mut fresh, &current);
    let mut other = live.clone();
    other.request_digest = [9; SOURCE_DIGEST_BYTES];
    assert_eq!(
        denied(fresh.revalidate(fresh_session, &other, &current, now)),
        SourceDenialReason::BindingMismatch
    );
}

fn port_without_revocation(
    corpus: &Corpus,
    snapshot: &SnapshotRule,
) -> FakeVerifiedSourceAuthority {
    let reader = &corpus.view("project-reader").holder;
    let mut port = FakeVerifiedSourceAuthority::default();
    port.grant(
        &reader.audience,
        FakeGrant {
            repository: corpus.repository().identity().clone(),
            snapshot: snapshot.clone(),
            grants: reader.grants.clone(),
            not_after_s: reader.not_after_s,
            depth: reader.depth,
            generation: crate::projection::restricted_corpus::GENERATION,
        },
    );
    port
}

// r[verify kiln.source.projection.authority.scope.scenario.identity]
#[test]
fn effective_scope_identity_binds_every_field() {
    let corpus = Corpus::load();
    let reader = &corpus.view("project-reader").holder;
    let mut facts = EffectiveScopeFacts {
        audience: reader.audience.clone(),
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        revision: corpus.revision.clone(),
        view_binding: digest("view"),
        admitted: reader.grants.clone(),
        capabilities: Vec::new(),
        not_after_s: reader.not_after_s,
        depth: 1,
        generation: 1,
        revocation_generation: 0,
        provenance: AuthorityProvenance::VerifiedUcan,
    };
    facts.capabilities = SessionCapability::under(&facts.context(), &reader.grants);
    let base = ok(EffectiveSourceScope::new(facts.clone()));
    let mut reordered = facts.clone();
    reordered.admitted.reverse();
    reordered.capabilities.reverse();
    assert_eq!(
        ok(EffectiveSourceScope::new(reordered)).identity(),
        base.identity()
    );

    type Change = fn(&mut EffectiveScopeFacts);
    let changes: [Change; 10] = [
        |f| f.audience = "did:key:z6MkOther".into(),
        |f| f.repository = digest("other"),
        |f| {
            f.revision = ok(ExternalRef::new("git", "commit", &"3".repeat(40)));
        },
        |f| f.view_binding = digest("other-view"),
        |f| {
            f.admitted.pop();
        },
        |f| f.not_after_s += 1,
        |f| f.depth = 0,
        |f| f.generation += 1,
        |f| f.revocation_generation += 1,
        |f| f.provenance = AuthorityProvenance::LocalOperator,
    ];
    for change in changes {
        let mut changed = facts.clone();
        change(&mut changed);
        assert_ne!(
            ok(EffectiveSourceScope::new(changed)).identity(),
            base.identity()
        );
    }
    let mut invalid = facts.clone();
    invalid.audience = String::new();
    assert!(EffectiveSourceScope::new(invalid).is_err());
    let mut deep = facts.clone();
    deep.depth = MAX_DELEGATION_DEPTH + 1;
    assert!(EffectiveSourceScope::new(deep).is_err());
    let mut unsafe_grant = facts;
    unsafe_grant.admitted.push(SourceGrant {
        path: PathScope {
            components: vec!["..".into()],
            subtree: true,
        },
        operations: ops(&[SourceViewOperation::Discover]),
    });
    assert!(EffectiveSourceScope::new(unsafe_grant).is_err());
}

// r[verify kiln.source.projection.authority.scope.scenario.admitted_only]
#[test]
fn capability_sets_confirm_but_never_extend_admitted_grants() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let grant = |path: &str, operations: &[Op]| SourceGrant {
        path: PathScope::from_path(path, true).unwrap_or_else(|| unreachable!("{path}")),
        operations: ops(operations),
    };
    let base = || EffectiveScopeFacts {
        audience: "did:key:z6MkReader".into(),
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        revision: corpus.revision.clone(),
        view_binding: digest("view"),
        admitted: Vec::new(),
        capabilities: Vec::new(),
        not_after_s: 2_000,
        depth: 0,
        generation: 1,
        revocation_generation: 0,
        provenance: AuthorityProvenance::VerifiedUcan,
    };
    let scope = |admitted: Vec<SourceGrant>, capabilities: Vec<SourceGrant>| {
        let mut facts = base();
        facts.admitted = admitted;
        facts.capabilities = SessionCapability::under(&facts.context(), &capabilities);
        ok(EffectiveSourceScope::new(facts))
    };
    // A capability set reporting an unchecked sibling adds nothing.
    let clipped = scope(
        vec![grant("crates/app", &[Op::Discover])],
        vec![
            grant("crates/app", &[Op::Discover, Op::ReadContent]),
            grant("crates/secret", &[Op::Discover, Op::ReadContent]),
        ],
    );
    assert_eq!(clipped.grants(), &[grant("crates/app", &[Op::Discover])]);
    assert!(!clipped.permits(Op::Discover, "crates/secret/src/key.rs"));
    assert!(!clipped.permits(Op::ReadContent, "crates/app/src/main.rs"));
    // An admitted grant the capability set does not confirm is dropped, and
    // its operations narrow to the confirmed ones.
    let narrowed = scope(
        vec![
            grant("crates/app", &[Op::Discover, Op::Delegate]),
            grant("docs", &[Op::Discover]),
        ],
        vec![grant("crates", &[Op::Discover])],
    );
    assert_eq!(narrowed.grants(), &[grant("crates/app", &[Op::Discover])]);

    // A capability observed under any other context confirms nothing.
    type Rekey = fn(&mut CapabilityContext);
    let rekeys: [Rekey; 7] = [
        |c| c.audience = "did:key:z6MkOther".into(),
        |c| c.repository = digest("other-repository"),
        |c| {
            c.snapshot = SnapshotRule::Ref {
                name: "refs/heads/main".into(),
                evolution: RefEvolution::Rewrite,
            }
        },
        |c| c.view_binding = digest("other-view"),
        |c| c.generation += 1,
        |c| c.generation -= 1,
        |c| c.revocation_generation += 1,
    ];
    for rekey in rekeys {
        let mut facts = base();
        facts.admitted = vec![grant("crates/app", &[Op::Discover])];
        let mut context = facts.context();
        rekey(&mut context);
        facts.capabilities = SessionCapability::under(&context, &facts.admitted.clone());
        let unconfirmed = ok(EffectiveSourceScope::new(facts));
        assert!(unconfirmed.grants().is_empty(), "{context:?}");
    }

    // Individually admitted scopes join; a scope differing in any key does not.
    let read = scope(
        vec![grant("crates/app", &[Op::ReadContent])],
        vec![grant("crates/app", &[Op::ReadContent])],
    );
    let joined = ok(EffectiveSourceScope::join(&[clipped.clone(), read]));
    assert!(joined.permits(Op::Discover, "crates/app/src/main.rs"));
    assert!(joined.permits(Op::ReadContent, "crates/app/src/main.rs"));
    assert!(!joined.permits(Op::Discover, "crates/secret/src/key.rs"));
    assert!(EffectiveSourceScope::join(&[]).is_err());
    type Change = fn(&mut EffectiveScopeFacts);
    let mismatches: [Change; 8] = [
        |f| f.audience = "did:key:z6MkOther".into(),
        |f| f.repository = digest("other-repository"),
        |f| {
            f.snapshot = SnapshotRule::Ref {
                name: "refs/heads/main".into(),
                evolution: RefEvolution::Rewrite,
            }
        },
        |f| f.revision = ok(ExternalRef::new("git", "commit", &"4".repeat(40))),
        |f| f.view_binding = digest("other-view"),
        |f| f.generation += 1,
        |f| f.revocation_generation += 1,
        |f| f.provenance = AuthorityProvenance::LocalOperator,
    ];
    for change in mismatches {
        let mut facts = base();
        change(&mut facts);
        facts.admitted = vec![grant("crates/app", &[Op::ReadContent])];
        facts.capabilities = SessionCapability::under(&facts.context(), &facts.admitted.clone());
        let other = ok(EffectiveSourceScope::new(facts));
        assert!(
            EffectiveSourceScope::join(&[clipped.clone(), other]).is_err(),
            "a mismatched scope joined"
        );
    }
}

// r[verify kiln.source.projection.authority.scope.scenario.admitted_only]
// r[verify kiln.source.projection.authority.port.scenario.verified]
#[test]
fn fake_port_confirms_only_capabilities_of_the_current_context() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let current = corpus.current();
    let agent = &corpus.view("agent").holder;
    let exact = SnapshotRule::Exact {
        revision: corpus.revision.clone(),
    };
    let fake = |repository: ContentId, generation: u64| FakeGrant {
        repository,
        snapshot: exact.clone(),
        grants: agent.grants.clone(),
        not_after_s: agent.not_after_s,
        depth: agent.depth,
        generation,
    };
    let main = request(
        &corpus,
        Op::ReadContent,
        components("crates/app/src/main.rs"),
    );
    let profile = verified(&agent.audience);
    let repository = corpus.repository().identity().clone();

    // A grant of another repository confirms nothing for this one.
    let mut port = FakeVerifiedSourceAuthority::default();
    port.grant(
        &agent.audience,
        fake(digest("other-repository"), GENERATION),
    );
    assert_eq!(
        denied(port.admit(&main, &profile, &current, now)),
        SourceDenialReason::RepositoryOutOfScope
    );

    // A grant issued under an older generation confirms nothing, nor do raw
    // capabilities of another audience, repository or generation; one of the
    // exact current context does.
    let mut port = FakeVerifiedSourceAuthority::default();
    port.grant(&agent.audience, fake(repository.clone(), GENERATION - 1));
    assert_eq!(
        denied(port.admit(&main, &profile, &current, now)),
        SourceDenialReason::ResourceOutOfScope
    );
    let context = CapabilityContext {
        audience: agent.audience.clone(),
        repository: repository.clone(),
        snapshot: exact.clone(),
        view_binding: main.view_binding.clone(),
        generation: GENERATION,
        revocation_generation: current.revocation_generation,
    };
    let mut other_audience = context.clone();
    other_audience.audience = "did:key:z6MkSharer".into();
    let mut other_repository = context.clone();
    other_repository.repository = digest("other-repository");
    let mut stale = context.clone();
    stale.generation = GENERATION - 1;
    for raw in [other_audience, other_repository, stale] {
        for capability in SessionCapability::under(&raw, &agent.grants) {
            port.report_raw(&agent.audience, capability);
        }
        assert_eq!(
            denied(port.admit(&main, &profile, &current, now)),
            SourceDenialReason::ResourceOutOfScope
        );
    }
    for capability in SessionCapability::under(&context, &agent.grants) {
        port.report_raw(&agent.audience, capability);
    }
    ok(port.admit(&main, &profile, &current, now));

    // A live reference request is outside an exact grant.
    let mut live = main.clone();
    live.snapshot = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::FastForward,
    };
    let mut on_ref = current.clone();
    on_ref.snapshot.ref_name = Some("refs/heads/main".into());
    assert_eq!(
        denied(port.admit(&live, &profile, &on_ref, now)),
        SourceDenialReason::SnapshotNotAdmitted
    );
}

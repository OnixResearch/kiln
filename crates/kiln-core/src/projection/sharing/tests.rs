use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::*;
use crate::projection::authority::{
    OriginScope, RefEvolution, SourceAuthority, SourceAuthorityRequest, SourceDenialReason,
};
use crate::projection::disclosure::restrict;
use crate::projection::restricted_corpus::{
    Corpus, FakeGrant, FakeVerifiedSourceAuthority, digest, key, ok, verified,
};
use crate::projection::{Mount, admit_definition};

fn outcome(result: &Result<SharePlan, ShareRefusal>) -> String {
    match result {
        Ok(_) => "admitted".into(),
        Err(refusal) => ok(serde_json::to_value(refusal))
            .as_str()
            .unwrap_or_default()
            .into(),
    }
}

fn path(value: &str, subtree: bool) -> PathScope {
    PathScope::from_path(value, subtree).unwrap_or_else(|| unreachable!("{value}"))
}

fn base_request(corpus: &Corpus) -> ShareRequest {
    ShareRequest {
        recipient: "did:key:z6MkReviewer".into(),
        paths: vec![path("crates/app", true)],
        operations: [
            SourceViewOperation::Discover,
            SourceViewOperation::ReadContent,
        ]
        .into_iter()
        .collect(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        not_after_s: 1_500,
        depth: 0,
    }
}

// r[verify kiln.source.projection.sharing.attenuation.scenario.corpus]
#[test]
fn corpus_share_cases_attenuate_or_refuse() {
    let corpus = Corpus::load();
    let mut seen = BTreeSet::new();
    for case in &corpus.shares {
        let parent = corpus.scope(&case.sharer);
        let ceiling = corpus.ceiling(&case.view);
        let result = admit_share(&parent, &ceiling, &case.request, corpus.now_s);
        assert_eq!(outcome(&result), case.expect, "{}", case.name);
        seen.insert(case.expect.clone());
        if let Ok(plan) = result {
            assert_eq!(plan.issuer(), "did:key:z6MkMaintainer");
            assert_eq!(plan.recipient(), case.request.recipient);
            assert_eq!(plan.ceiling(), &ceiling);
            assert_eq!(plan.parent_scope(), parent.identity());
            assert!(plan.not_after_s() <= parent.not_after_s());
            assert!(plan.depth() < parent.depth());
        }
    }
    for expected in [
        "admitted",
        "delegation_not_permitted",
        "widen_resource",
        "widen_operation",
        "outlives_parent",
        "depth_exceeded",
        "widen_snapshot",
    ] {
        assert!(seen.contains(expected), "corpus lacks {expected}");
    }
}

// r[verify kiln.source.projection.sharing.attenuation.scenario.bounds]
#[test]
fn malformed_and_expired_shares_are_refused() {
    let corpus = Corpus::load();
    let parent = corpus.scope("maintainer");
    let ceiling = corpus.ceiling("project-reader");
    let now = corpus.now_s;
    ok(admit_share(&parent, &ceiling, &base_request(&corpus), now));
    type Change = fn(&mut ShareRequest);
    let invalid: [Change; 7] = [
        |r| r.recipient = "did:*".into(),
        |r| r.recipient = "reviewer".into(),
        |r| r.recipient = "did:key:a b".into(),
        |r| r.paths.clear(),
        |r| r.operations.clear(),
        |r| r.not_after_s = 1_000,
        |r| r.depth = MAX_DELEGATION_DEPTH + 1,
    ];
    for change in invalid {
        let mut request = base_request(&corpus);
        change(&mut request);
        assert_eq!(
            admit_share(&parent, &ceiling, &request, now),
            Err(ShareRefusal::RequestInvalid),
            "{request:?}"
        );
    }
    let mut late = base_request(&corpus);
    late.not_after_s = 2_500;
    assert_eq!(
        admit_share(&parent, &ceiling, &late, 2_000),
        Err(ShareRefusal::ParentExpired)
    );
    let mut delegating = base_request(&corpus);
    delegating.operations.insert(SourceViewOperation::Delegate);
    assert_eq!(
        admit_share(&parent, &ceiling, &delegating, now),
        Err(ShareRefusal::DepthExceeded)
    );
    delegating.depth = 1;
    let plan = ok(admit_share(&parent, &ceiling, &delegating, now));
    assert_eq!(plan.depth(), 1);
    // A live reference cannot be shared from an exact grant; an exact
    // revision can be shared from a live grant only at its admitted revision.
    let mut follow = base_request(&corpus);
    follow.snapshot = SnapshotRule::Ref {
        name: "refs/heads/main".into(),
        evolution: RefEvolution::FastForward,
    };
    assert_eq!(
        admit_share(&parent, &ceiling, &follow, now),
        Err(ShareRefusal::WidenSnapshot)
    );
    let mut other = base_request(&corpus);
    other.snapshot = SnapshotRule::Exact {
        revision: ok(crate::types::ExternalRef::new(
            "git",
            "commit",
            &"2".repeat(40),
        )),
    };
    assert_eq!(
        admit_share(&parent, &ceiling, &other, now),
        Err(ShareRefusal::WidenSnapshot)
    );
}

// r[verify kiln.source.projection.sharing.attenuation.scenario.property]
#[test]
fn a_child_never_exceeds_its_parent_or_ceiling() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let candidates = [
        path("", true),
        path("crates", true),
        path("crates/app", true),
        path("crates/apple", true),
        path("crates/app/src/main.rs", false),
        path("crates/shared", true),
        path("aliases/shared.rs", false),
        path("docs", true),
        path(".cairn/changes/app-feature", true),
    ];
    let parents = [
        corpus.scope("maintainer"),
        corpus.scope_with(
            vec![
                SourceGrant {
                    path: path("crates/app", true),
                    operations: [Op::Discover, Op::ReadContent, Op::Delegate]
                        .into_iter()
                        .collect(),
                },
                SourceGrant {
                    path: path("crates/shared", true),
                    operations: [Op::Discover, Op::ReadContent].into_iter().collect(),
                },
            ],
            "did:key:z6MkMaintainer",
            &corpus.revision,
        ),
    ];
    let ceilings = [
        corpus.ceiling("project-reader"),
        corpus.ceiling("agent"),
        corpus.ceiling("maintainer"),
    ];
    let mut admitted = 0_usize;
    let mut refused = 0_usize;
    for parent in &parents {
        for ceiling in &ceilings {
            for scope in &candidates {
                for mask in 1_u16..(1 << SourceViewOperation::ALL.len()) {
                    let operations: BTreeSet<Op> = SourceViewOperation::ALL
                        .iter()
                        .enumerate()
                        .filter(|(bit, _)| mask & (1 << bit) != 0)
                        .map(|(_, operation)| *operation)
                        .collect();
                    for (not_after_s, depth) in [(1_500, 0), (1_500, 1), (2_000, 1), (2_001, 0)] {
                        let request = ShareRequest {
                            recipient: "did:key:z6MkChild".into(),
                            paths: vec![scope.clone()],
                            operations: operations.clone(),
                            snapshot: SnapshotRule::Exact {
                                revision: corpus.revision.clone(),
                            },
                            not_after_s,
                            depth,
                        };
                        let Ok(plan) = admit_share(parent, ceiling, &request, now) else {
                            refused += 1;
                            continue;
                        };
                        admitted += 1;
                        assert!(plan.not_after_s() <= parent.not_after_s());
                        assert!(plan.depth() < parent.depth());
                        for granted in plan.grants() {
                            assert!(ceiling.contains(&granted.path), "{granted:?}");
                            for operation in &granted.operations {
                                assert!(ceiling.operations().contains(operation));
                                assert!(
                                    parent.permits_scope(*operation, &granted.path),
                                    "{operation:?} over {granted:?}"
                                );
                            }
                            assert!(parent.permits_scope(Op::Delegate, &granted.path));
                        }
                    }
                }
            }
        }
    }
    assert!(admitted > 0 && refused > admitted, "{admitted} / {refused}");
}

// r[verify kiln.source.projection.sharing.ceiling.scenario.saved_view]
#[test]
fn expanding_a_saved_view_does_not_expand_shared_access() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let ceiling = corpus.ceiling("project-reader");
    let maintainer = corpus.scope("maintainer");
    let mut request = base_request(&corpus);
    request.operations.insert(Op::Delegate);
    request.depth = 1;
    let plan = ok(admit_share(&maintainer, &ceiling, &request, now));

    let mut port = FakeVerifiedSourceAuthority::default();
    port.issue(&plan);
    let recipient = verified(plan.recipient());
    let request = |operation: Op, scope: OriginScope| SourceAuthorityRequest {
        operation,
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        view_binding: ceiling.identity().clone(),
        scope,
        request_digest: [6; 32],
        attempt: None,
    };
    let read = |scope: OriginScope| request(Op::ReadContent, scope);
    // The recipient admits each shared operation over its crate on its own.
    let mut admitted = Vec::new();
    for operation in [Op::Discover, Op::ReadContent, Op::Delegate] {
        let session = ok(port.admit(
            &request(
                operation,
                OriginScope::Components {
                    path: path("crates/app", true),
                },
            ),
            &recipient,
            &corpus.current(),
            now,
        ))
        .session
        .unwrap_or_else(|| unreachable!("session"));
        admitted.push(
            port.effective_scope(session)
                .cloned()
                .unwrap_or_else(|| unreachable!("live")),
        );
    }
    let child = ok(crate::projection::authority::EffectiveSourceScope::join(
        &admitted,
    ));
    match port.admit(
        &read(OriginScope::Components {
            path: path("crates/shared/src/lib.rs", false),
        }),
        &recipient,
        &corpus.current(),
        now,
    ) {
        Ok(decision) => unreachable!("widened: {decision:?}"),
        Err(decision) => assert_eq!(decision.reason, SourceDenialReason::ResourceOutOfScope),
    }

    // The saved view is expanded to the whole repository.
    let mut expanded = corpus.view("project-reader").definition.clone();
    expanded.selection.include = vec!["**".into()];
    expanded.mounts.push(Mount {
        source_prefix: "docs".into(),
        destination_prefix: "crates/app/docs".into(),
    });
    let expanded = ok(admit_definition(&expanded));
    let facts = corpus.facts();
    for ceiling in [
        plan.ceiling().clone(),
        ViewCeiling::from_definition(&expanded),
    ] {
        let trusted = corpus.enumerate(&expanded, &facts, &child, &ceiling);
        let view = ok(restrict(&trusted, &child, &ceiling, &key(), now));
        let shown: Vec<&str> = view
            .entries()
            .iter()
            .filter_map(|entry| entry.display_path.as_deref())
            .collect();
        assert_eq!(
            shown,
            [
                "crates/app/Cargo.toml",
                "crates/app/src/main.rs",
                "crates/app/src/util.rs"
            ]
        );
    }
    // The child may share on only within what it holds, at lower depth.
    let mut grandchild = base_request(&corpus);
    grandchild.recipient = "did:key:z6MkGrandchild".into();
    grandchild.operations = [Op::Discover].into_iter().collect();
    let expanded_ceiling = ViewCeiling::from_definition(&expanded);
    ok(admit_share(&child, &expanded_ceiling, &grandchild, now));
    grandchild.paths = vec![path("docs", true)];
    assert_eq!(
        admit_share(&child, &expanded_ceiling, &grandchild, now),
        Err(ShareRefusal::WidenResource)
    );
    grandchild.paths = vec![path("crates/app", true)];
    grandchild.operations = [Op::ReadHistory].into_iter().collect();
    assert_eq!(
        admit_share(&child, &expanded_ceiling, &grandchild, now),
        Err(ShareRefusal::WidenOperation)
    );
    grandchild.operations = [Op::Discover].into_iter().collect();
    grandchild.depth = 1;
    assert_eq!(
        admit_share(&child, &expanded_ceiling, &grandchild, now),
        Err(ShareRefusal::DepthExceeded)
    );
    grandchild.depth = 0;
    grandchild.not_after_s = 1_600;
    assert_eq!(
        admit_share(&child, &expanded_ceiling, &grandchild, now),
        Err(ShareRefusal::OutlivesParent)
    );
}

// r[verify kiln.source.projection.sharing.attenuation.scenario.reference]
#[test]
fn a_view_reference_alone_confers_nothing() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    // The reader passes its view reference and handles along; the recipient
    // holds no grant, so the port admits nothing.
    let reader = ok(restrict(
        &corpus.trusted("project-reader"),
        &corpus.scope("project-reader"),
        &corpus.ceiling("project-reader"),
        &key(),
        now,
    ));
    let mut port = FakeVerifiedSourceAuthority::default();
    let holder = &corpus.view("project-reader").holder;
    port.grant(
        &holder.audience,
        FakeGrant {
            repository: corpus.repository().identity().clone(),
            snapshot: SnapshotRule::Exact {
                revision: corpus.revision.clone(),
            },
            grants: holder.grants.clone(),
            not_after_s: holder.not_after_s,
            depth: holder.depth,
            generation: crate::projection::restricted_corpus::GENERATION,
        },
    );
    let handle = reader
        .entries()
        .first()
        .map(|entry| entry.handle)
        .unwrap_or_else(|| unreachable!("visible entry"));
    for scope in [
        OriginScope::Handle {
            handle: reader.reference(),
        },
        OriginScope::Handle { handle },
        OriginScope::Components {
            path: path("crates/app/src/main.rs", false),
        },
    ] {
        let request = SourceAuthorityRequest {
            operation: Op::ReadContent,
            repository: corpus.repository().identity().clone(),
            snapshot: SnapshotRule::Exact {
                revision: corpus.revision.clone(),
            },
            view_binding: reader.binding().identity().clone(),
            scope,
            request_digest: [8; 32],
            attempt: None,
        };
        match port.admit(
            &request,
            &verified("did:key:z6MkFriend"),
            &corpus.current(),
            now,
        ) {
            Ok(decision) => unreachable!("a reference conferred access: {decision:?}"),
            Err(decision) => assert_eq!(decision.reason, SourceDenialReason::CredentialDenied),
        }
    }
    // Nor can the reader itself turn its read into a share.
    assert_eq!(
        admit_share(
            &corpus.scope("project-reader"),
            reader.ceiling(),
            &base_request(&corpus),
            now
        ),
        Err(ShareRefusal::DelegationNotPermitted)
    );
    // The plan identity binds the sharer's scope and generations.
    let plan = ok(admit_share(
        &corpus.scope("maintainer"),
        &corpus.ceiling("project-reader"),
        &base_request(&corpus),
        now,
    ));
    let rebound = corpus.scope_with(
        corpus.view("maintainer").holder.grants.clone(),
        "did:key:z6MkMaintainer",
        &corpus.revision,
    );
    assert_eq!(rebound.identity(), corpus.scope("maintainer").identity());
    let narrowed = corpus.scope_with(
        vec![SourceGrant {
            path: path("crates", true),
            operations: [Op::Discover, Op::ReadContent, Op::Delegate]
                .into_iter()
                .collect(),
        }],
        "did:key:z6MkMaintainer",
        &corpus.revision,
    );
    let other = ok(admit_share(
        &narrowed,
        &corpus.ceiling("project-reader"),
        &base_request(&corpus),
        now,
    ));
    assert_ne!(other.identity(), plan.identity());
    assert_eq!(other.grants(), plan.grants());
    assert_ne!(plan.identity(), &digest("plan"));
}

// r[verify kiln.source.projection.authority.scope.scenario.admitted_only]
#[test]
fn an_issued_share_is_current_only_in_its_generation() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let ceiling = corpus.ceiling("project-reader");
    let plan = ok(admit_share(
        &corpus.scope("maintainer"),
        &ceiling,
        &base_request(&corpus),
        now,
    ));
    let mut port = FakeVerifiedSourceAuthority::default();
    port.issue(&plan);
    let request = SourceAuthorityRequest {
        operation: Op::ReadContent,
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        view_binding: ceiling.identity().clone(),
        scope: OriginScope::Components {
            path: path("crates/app/src/main.rs", false),
        },
        request_digest: [7; 32],
        attempt: None,
    };
    let recipient = verified(plan.recipient());
    ok(port.admit(&request, &recipient, &corpus.current(), now));
    let mut next = corpus.current();
    next.generation = plan.generation() + 1;
    match port.admit(&request, &recipient, &next, now) {
        Ok(decision) => unreachable!("stale share admitted: {decision:?}"),
        Err(decision) => assert_eq!(decision.reason, SourceDenialReason::ResourceOutOfScope),
    }
}

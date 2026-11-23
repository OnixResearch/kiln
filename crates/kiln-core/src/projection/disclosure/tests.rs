use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use super::*;
use crate::projection::authority::{
    OriginScope, SnapshotRule, SourceAuthority, SourceAuthorityRequest, SourceDenialReason,
    SourceGrant,
};
use crate::projection::restricted_corpus::{
    Corpus, FakeGrant, FakeVerifiedSourceAuthority, digest, key, ok, other_key, verified,
};
use crate::projection::{
    DependencyStatus, EntryKind, Mount, ProjectionDefinition, ProjectionFacts, SourceEntry,
    admit_definition,
};

fn view_of(corpus: &Corpus, name: &str) -> AudienceView {
    ok(restrict(
        &corpus.trusted(name),
        &corpus.scope(name),
        &corpus.ceiling(name),
        &key(),
        corpus.now_s,
    ))
}

fn shown(view: &AudienceView) -> Vec<&str> {
    view.entries()
        .iter()
        .filter_map(|entry| entry.display_path.as_deref())
        .collect()
}

fn refused(result: Result<AudienceView, DisclosureRefusal>) -> DisclosureRefusal {
    match result {
        Ok(view) => unreachable!("unexpectedly disclosed: {view:?}"),
        Err(refusal) => refusal,
    }
}

fn unavailable<T: core::fmt::Debug>(result: Result<T, Unavailable>) -> UnavailableReason {
    match result {
        Ok(value) => unreachable!("unexpectedly available: {value:?}"),
        Err(refusal) => {
            assert_eq!(refusal.public(), PublicUnavailable::SHAPE);
            refusal.reason()
        }
    }
}

fn handle_of(view: &AudienceView, destination: &str) -> OpaqueHandle {
    view.trusted
        .iter()
        .zip(view.entries())
        .find(|(slot, _)| slot.destination == destination)
        .map(|(_, entry)| entry.handle)
        .unwrap_or_else(|| unreachable!("{destination} is visible"))
}

fn grant(path: &str, operations: &[SourceViewOperation]) -> SourceGrant {
    SourceGrant {
        path: PathScope::from_path(path, true).unwrap_or_else(|| unreachable!("{path}")),
        operations: operations.iter().copied().collect(),
    }
}

/// Deterministic SplitMix64 for property cases without a dev-dependency.
struct Cases(u64);

impl Cases {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

const PROPERTY_CASES: u64 = 64;

// r[verify kiln.source.projection.disclosure.restricted.scenario.audiences]
#[test]
fn corpus_views_disclose_only_their_effective_scope() {
    let corpus = Corpus::load();
    let observation = corpus.observation();
    let mut identities = BTreeSet::new();
    for case in &corpus.views {
        let view = view_of(&corpus, &case.name);
        assert_eq!(shown(&view), case.expect.visible, "{}", case.name);
        assert_eq!(view.coverage(), VisibleCoverage::Complete);
        let scope = corpus.scope(&case.name);
        let context = ok(restrict_context(
            &view,
            &observation,
            &scope,
            &key(),
            corpus.now_s,
        ));
        let ids: Vec<&str> = context
            .items()
            .iter()
            .map(|item| item.native_id.as_str())
            .collect();
        assert_eq!(ids, case.expect.cairn, "{}", case.name);
        assert_eq!(
            context.hidden_prerequisites().len(),
            case.expect.hidden_prerequisites,
            "{}",
            case.name
        );
        assert!(identities.insert(view.content_identity()), "{}", case.name);
    }

    // The maintainer may edit everything; the reader nothing; the agent only
    // its crate.
    let status = |name: &str, destination: &str| {
        view_of(&corpus, name)
            .entries()
            .iter()
            .find(|entry| entry.display_path.as_deref() == Some(destination))
            .map(|entry| (entry.status, entry.readable))
    };
    assert_eq!(
        status("maintainer", "crates/secret/src/key.rs"),
        Some((VisibleStatus::Present, true))
    );
    assert_eq!(
        status("project-reader", "crates/app/src/main.rs"),
        Some((VisibleStatus::ReadOnly, true))
    );
    assert_eq!(
        status("agent", "crates/app/src/main.rs"),
        Some((VisibleStatus::Present, true))
    );
    // The agent discovers its change but may not read its text.
    assert_eq!(
        status("agent", ".cairn/changes/app-feature/tasks.md"),
        Some((VisibleStatus::ReadOnly, false))
    );
    // Only the full-source maintainer learns the dependency's identity.
    for (name, disclosed) in [
        ("maintainer", true),
        ("project-reader", false),
        ("agent", false),
    ] {
        let view = view_of(&corpus, name);
        let dependency = view
            .dependencies()
            .first()
            .unwrap_or_else(|| unreachable!("declared"));
        assert!(dependency.available);
        assert_eq!(dependency.identity.is_some(), disclosed, "{name}");
    }
}

// r[verify kiln.source.projection.disclosure.restricted.scenario.filter_first]
#[test]
fn effective_view_is_selection_ceiling_snapshot_and_current_grant() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    // A full-source holder through the reader's view sees only that view's
    // ceiling: no documentation, though its grant covers the root.
    let maintainer = corpus.scope("maintainer");
    let facts = corpus.facts();
    let reader_definition = corpus.definition("project-reader");
    let reader_ceiling = corpus.ceiling("project-reader");
    let through_reader = ok(restrict(
        &corpus.enumerate(&reader_definition, &facts, &maintainer, &reader_ceiling),
        &maintainer,
        &reader_ceiling,
        &key(),
        now,
    ));
    assert!(!shown(&through_reader).contains(&"docs/guide.md"));
    assert!(shown(&through_reader).contains(&"crates/secret/src/key.rs"));
    // A ceiling from a narrower definition hides what the trusted view holds.
    let agent_ceiling = corpus.ceiling("agent");
    let narrowed = ok(restrict(
        &corpus.enumerate(
            &corpus.definition("maintainer"),
            &facts,
            &maintainer,
            &agent_ceiling,
        ),
        &maintainer,
        &agent_ceiling,
        &key(),
        now,
    ));
    assert!(
        shown(&narrowed)
            .iter()
            .all(|path| path.starts_with(".cairn/changes/")
                || path.starts_with("crates/app/")
                || path.starts_with("crates/apple/")
                || path.starts_with("crates/shared/"))
    );
    // The per-component scope never leaks a sibling with a longer name.
    let agent = view_of(&corpus, "agent");
    assert!(!shown(&agent).contains(&"crates/apple/README.md"));

    let trusted = corpus.trusted("agent");
    let ceiling = corpus.ceiling("agent");
    let scope = corpus.scope("agent");
    assert_eq!(
        refused(restrict(&trusted, &scope, &ceiling, &key(), 2_000)),
        DisclosureRefusal::Expired
    );
    let moved = ok(ExternalRef::new("git", "commit", &"2".repeat(40)));
    let other_revision = corpus.scope_with(
        corpus.view("agent").holder.grants.clone(),
        "did:key:z6MkAgent",
        &moved,
    );
    assert_eq!(
        refused(restrict(&trusted, &other_revision, &ceiling, &key(), now)),
        DisclosureRefusal::SnapshotMismatch
    );
    let write_only = corpus.scope_with(
        vec![grant("crates/app", &[Op::ModifySource, Op::Delegate])],
        "did:key:z6MkAgent",
        &corpus.revision,
    );
    assert_eq!(
        refused(restrict(&trusted, &write_only, &ceiling, &key(), now)),
        DisclosureRefusal::NotAuthorized
    );
    let mut foreign = corpus.facts();
    let other_repository = ok(crate::projection::admit_repositories(
        &[crate::projection::RepositoryEnrollment {
            repository_id: "other".into(),
            binding: crate::projection::ProviderBinding {
                source_provider: ok(crate::types::ProviderProfileId::new("git-source-v1")),
                native_repository: ok(ExternalRef::new("github", "repository", "Other/other")),
            },
            locators: Vec::new(),
        }],
        &[],
    ))
    .pop()
    .unwrap_or_else(|| unreachable!("one repository"));
    foreign.repository = other_repository.identity().clone();
    let selection = ScopedSelection::for_holder(&scope, &ceiling);
    let foreign_view = ok(ScopedEnumeration::new(
        &selection,
        ok(crate::projection::resolve(
            &corpus.definition("agent"),
            &other_repository,
            &ok(selection.filter_facts(&foreign)),
        )),
    ));
    assert_eq!(
        refused(restrict(&foreign_view, &scope, &ceiling, &key(), now)),
        DisclosureRefusal::RepositoryMismatch
    );
    for refusal in [
        DisclosureRefusal::Expired,
        DisclosureRefusal::RepositoryMismatch,
        DisclosureRefusal::SnapshotMismatch,
        DisclosureRefusal::NotAuthorized,
        DisclosureRefusal::Stale,
        DisclosureRefusal::OutsideSelection,
    ] {
        assert_eq!(refusal.public(), PublicUnavailable::SHAPE);
    }
}

/// Facts at a new revision where only entries the reader cannot see changed.
fn forbidden_only_change(corpus: &Corpus, cases: &mut Cases, round: u64) -> ProjectionFacts {
    let revision = ok(ExternalRef::new(
        "git",
        "commit",
        &alloc::format!("{round:040x}"),
    ));
    let mut facts = corpus.facts_at(&revision);
    let forbidden = |path: &str| {
        path.starts_with("crates/secret/")
            || path.starts_with("crates/apple/")
            || path.starts_with("docs/")
            || path.starts_with(".cairn/changes/secret-rotation/")
    };
    for _ in 0..=cases.below(4) {
        match cases.below(4) {
            0 => facts.entries.push(SourceEntry {
                path: alloc::format!("crates/secret/src/gen{}.rs", cases.below(1_000)),
                object: ok(ExternalRef::new(
                    "git",
                    "blob",
                    &alloc::format!("blob-{}", cases.next()),
                )),
                kind: EntryKind::File,
            }),
            1 => {
                let positions: Vec<usize> = facts
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| forbidden(&entry.path))
                    .map(|(index, _)| index)
                    .collect();
                let count = u64::try_from(positions.len()).unwrap_or(u64::MAX).max(1);
                let chosen = usize::try_from(cases.below(count)).unwrap_or(0);
                if let Some(index) = positions.get(chosen).copied() {
                    facts.entries.remove(index);
                }
            }
            2 => {
                for entry in facts.entries.iter_mut().filter(|e| forbidden(&e.path)) {
                    entry.object = ok(ExternalRef::new(
                        "git",
                        "blob",
                        &alloc::format!("changed-{}", cases.next()),
                    ));
                }
            }
            _ => {
                for entry in facts.entries.iter_mut().filter(|e| forbidden(&e.path)) {
                    entry.kind = EntryKind::Executable;
                }
            }
        }
    }
    facts.entries.sort_by(|a, b| a.path.cmp(&b.path));
    facts.entries.dedup_by(|a, b| a.path == b.path);
    // Mutations can cancel out (an added file removed again); keep one.
    if facts.entries == corpus.facts_at(&revision).entries {
        facts.entries.push(SourceEntry {
            path: alloc::format!("crates/secret/src/round{round}.rs"),
            object: ok(ExternalRef::new("git", "blob", "round")),
            kind: EntryKind::File,
        });
        facts.entries.sort_by(|a, b| a.path.cmp(&b.path));
    }
    facts
}

// r[verify kiln.source.projection.disclosure.restricted.scenario.forbidden_invariance]
#[test]
fn forbidden_only_changes_leave_the_visible_result_equal() {
    let corpus = Corpus::load();
    let baseline = view_of(&corpus, "project-reader");
    type Seen = (Option<String>, Option<EntryKind>, VisibleStatus, bool);
    let baseline_entries: Vec<Seen> = baseline
        .entries()
        .iter()
        .map(|e| (e.display_path.clone(), e.kind, e.status, e.readable))
        .collect();
    let definition = corpus.definition("project-reader");
    let ceiling = corpus.ceiling("project-reader");
    let grants = corpus.view("project-reader").holder.grants.clone();
    let mut cases = Cases(0x5EED_0075);
    for round in 1..=PROPERTY_CASES {
        let facts = forbidden_only_change(&corpus, &mut cases, round);
        assert_ne!(
            facts.entries,
            corpus.facts().entries,
            "round {round} changed nothing hidden"
        );
        let scope = corpus.scope_with(grants.clone(), "did:key:z6MkReader", &facts.exact_revision);
        let trusted = corpus.enumerate(&definition, &facts, &scope, &ceiling);
        let view = ok(restrict(&trusted, &scope, &ceiling, &key(), corpus.now_s));
        let entries: Vec<Seen> = view
            .entries()
            .iter()
            .map(|e| (e.display_path.clone(), e.kind, e.status, e.readable))
            .collect();
        assert_eq!(entries, baseline_entries, "round {round}");
        assert_eq!(view.coverage(), baseline.coverage(), "round {round}");
        assert_eq!(
            view.content_identity(),
            baseline.content_identity(),
            "round {round}"
        );
    }
    // A hidden dependency identity at the same snapshot changes nothing the
    // holder receives: not the entries, handles, view reference or cursors.
    let mut facts = corpus.facts();
    for dependency in &mut facts.dependencies {
        dependency.status = DependencyStatus::Present {
            identity: digest("shared-crate-v2"),
        };
    }
    let changed_trusted = corpus.trusted_with("project-reader", &definition, &facts);
    let scope = corpus.scope("project-reader");
    let view = ok(restrict(
        &changed_trusted,
        &scope,
        &ceiling,
        &key(),
        corpus.now_s,
    ));
    assert_ne!(view.binding().manifest(), baseline.binding().manifest());
    assert_eq!(
        ok(serde_json::to_string(&view.presentation())),
        ok(serde_json::to_string(&baseline.presentation()))
    );
    let page = |view: &AudienceView| {
        ok(view.page(&key(), &scope, corpus.now_s, None, 2))
            .next
            .map(|cursor| cursor.to_token())
    };
    assert_eq!(page(&view), page(&baseline));
}

// r[verify kiln.source.projection.disclosure.handles.scenario.keyed]
#[test]
fn handles_are_keyed_scoped_and_confer_nothing() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let reader = view_of(&corpus, "project-reader");
    let reader_scope = corpus.scope("project-reader");
    let enumeration = corpus.trusted("project-reader");
    let trusted = enumeration.view().clone();
    let maintainer = view_of(&corpus, "maintainer");

    // Equal bytes, an alias and its origin each get their own handle.
    let util = handle_of(&reader, "crates/app/src/util.rs");
    let alias = handle_of(&reader, "aliases/shared.rs");
    let origin = handle_of(&reader, "crates/shared/src/lib.rs");
    let handles: BTreeSet<OpaqueHandle> = reader.entries().iter().map(|e| e.handle).collect();
    assert_eq!(handles.len(), reader.entries().len());
    assert_ne!(alias, origin);
    // Knowing native digests forges nothing: no unkeyed hash is a handle.
    for guess in [
        "crates/app/src/util.rs",
        "crates/secret/src/key.rs",
        "equal-bytes",
        reader.binding().manifest().as_str(),
    ] {
        let unkeyed = OpaqueHandle(*blake3::hash(guess.as_bytes()).as_bytes());
        assert!(!handles.contains(&unkeyed), "{guess}");
        assert_eq!(
            unavailable(reader.resolve(&trusted, &reader_scope, now, &unkeyed, Op::ReadContent)),
            UnavailableReason::Missing
        );
    }
    // Another holder's handle for the same entry is another handle, and the
    // maintainer's handle for the equal-byte secret resolves nothing here.
    assert_ne!(handle_of(&maintainer, "crates/app/src/util.rs"), util);
    let secret = handle_of(&maintainer, "crates/secret/src/key.rs");
    assert_eq!(
        unavailable(reader.resolve(&trusted, &reader_scope, now, &secret, Op::ReadContent)),
        UnavailableReason::Missing
    );
    // The same binding under another key yields other handles.
    let rekeyed = ok(restrict(
        &enumeration,
        &reader_scope,
        &corpus.ceiling("project-reader"),
        &other_key(),
        now,
    ));
    assert_eq!(rekeyed.binding(), reader.binding());
    assert_ne!(handle_of(&rekeyed, "crates/app/src/util.rs"), util);

    // A visible handle resolves to its trusted origin for granted operations.
    let resolved = ok(reader.resolve(&trusted, &reader_scope, now, &alias, Op::ReadContent));
    assert_eq!(resolved.destination, "aliases/shared.rs");
    assert!(matches!(
        &resolved.origin,
        crate::projection::EntryOrigin::Native { path, .. } if path == "crates/shared/src/lib.rs"
    ));
    assert_eq!(
        unavailable(reader.resolve(&trusted, &reader_scope, now, &util, Op::ModifySource)),
        UnavailableReason::Forbidden
    );
    assert_eq!(
        unavailable(reader.resolve(&trusted, &reader_scope, now, &util, Op::Delegate)),
        UnavailableReason::Forbidden
    );

    // A handle presented at the port is not a resource.
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
    let request = SourceAuthorityRequest {
        operation: Op::ReadContent,
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        view_binding: reader.binding().identity().clone(),
        scope: OriginScope::Handle { handle: util },
        request_digest: [5; 32],
        attempt: None,
    };
    match port.admit(
        &request,
        &verified(&holder.audience),
        &corpus.current(),
        now,
    ) {
        Ok(decision) => unreachable!("a handle was admitted: {decision:?}"),
        Err(decision) => assert_eq!(decision.reason, SourceDenialReason::HandleConfersNothing),
    }
    // Handles round-trip as hex and nothing else.
    let text = ok(serde_json::to_string(&util));
    assert_eq!(ok(serde_json::from_str::<OpaqueHandle>(&text)), util);
    assert!(serde_json::from_str::<OpaqueHandle>("\"crates/app/src/util.rs\"").is_err());
}

// r[verify kiln.source.projection.disclosure.unavailable.scenario.probe]
#[test]
fn attribution_judges_authority_not_display() {
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let definition = corpus.definition("project-reader");
    let ceiling = corpus.ceiling("project-reader");
    let scope = corpus.scope("project-reader");
    let scoped = corpus.trusted("project-reader");
    let audit = corpus.audit(&definition, &corpus.facts());
    let reader = view_of(&corpus, "project-reader");
    let attribute = |view: &AudienceView, audit: &ScopedEnumeration, path: &str| {
        view.attribute(audit, &scope, now, path)
    };
    // A truly hidden existing file is forbidden and a nonexistent one
    // missing, judged against the whole-ceiling audit enumeration.
    assert_eq!(
        attribute(&reader, &audit, "crates/secret/src/key.rs"),
        Some(UnavailableReason::Forbidden)
    );
    assert_eq!(
        attribute(&reader, &audit, "crates/secret/src/absent.rs"),
        Some(UnavailableReason::Missing)
    );
    // The view's own scoped enumeration, or a truncated whole-ceiling one,
    // cannot attribute anything.
    assert_eq!(
        attribute(&reader, &scoped, "crates/secret/src/key.rs"),
        None
    );
    let whole = ScopedSelection::whole(&ceiling);
    let mut truncated = ok(whole.filter_facts(&corpus.facts()));
    truncated.coverage = crate::projection::Coverage::Truncated;
    let truncated = ok(ScopedEnumeration::new(
        &whole,
        corpus.resolved(&definition, &truncated),
    ));
    assert_eq!(
        attribute(&reader, &truncated, "crates/secret/src/key.rs"),
        None
    );
    // Another holder's narrower complete view gives no attribution, even
    // one holding every destination the probing view lists: it would call
    // the hidden file missing. The whole-ceiling audit calls it forbidden.
    let narrowed_scope = corpus.scope_with(
        vec![grant(
            "crates/app",
            &[
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    let narrowed = ok(restrict(
        &corpus.enumerate(&definition, &corpus.facts(), &narrowed_scope, &ceiling),
        &narrowed_scope,
        &ceiling,
        &key(),
        now,
    ));
    for (other, expected) in [
        (&scoped, None),
        (&audit, Some(UnavailableReason::Forbidden)),
    ] {
        assert_eq!(
            narrowed.attribute(other, &narrowed_scope, now, "crates/secret/src/key.rs"),
            expected
        );
    }
    // An audit lacking a destination the view lists is an owner-side audit mismatch.
    let mut lacking = corpus.facts();
    lacking
        .entries
        .retain(|entry| entry.path != "crates/app/src/main.rs");
    assert_eq!(
        attribute(
            &reader,
            &corpus.audit(&definition, &lacking),
            "crates/secret/src/key.rs"
        ),
        Some(UnavailableReason::AuditMismatch)
    );
    // A full-source holder's own enumeration is the whole-ceiling one, so it
    // is a valid audit; nothing in the ceiling is hidden from it.
    let maintainer_scope = corpus.scope("maintainer");
    let maintainer_ceiling = corpus.ceiling("maintainer");
    assert_eq!(
        ScopedSelection::for_holder(&maintainer_scope, &maintainer_ceiling),
        ScopedSelection::whole(&maintainer_ceiling)
    );
    assert_eq!(
        view_of(&corpus, "maintainer").attribute(
            &corpus.trusted("maintainer"),
            &maintainer_scope,
            now,
            "crates/secret/src/absent.rs"
        ),
        Some(UnavailableReason::Missing)
    );

    // An authorized destination the view's bound cut is truncated, not
    // forbidden, both for the owner and in the lookup's internal reason.
    let bound = 2;
    let cut = ok(restrict_bounded(
        &scoped,
        &scope,
        &ceiling,
        &key(),
        now,
        bound,
    ));
    assert!(!shown(&cut).contains(&"crates/app/src/main.rs"));
    assert_eq!(
        attribute(&cut, &audit, "crates/app/src/main.rs"),
        Some(UnavailableReason::Truncated)
    );
    assert_eq!(
        unavailable(cut.lookup(scoped.view(), &scope, now, "crates/app/src/main.rs")),
        UnavailableReason::Truncated
    );
    assert_eq!(
        attribute(&cut, &audit, "crates/secret/src/key.rs"),
        Some(UnavailableReason::Forbidden)
    );
    // One a truncated holder-scoped enumeration never listed is unknown.
    let selection = ScopedSelection::for_holder(&scope, &ceiling);
    let short = adapter_enumeration(&corpus.facts(), &selection, 4);
    let partial = ok(restrict(
        &ok(ScopedEnumeration::new(
            &selection,
            corpus.resolved(&definition, &short),
        )),
        &scope,
        &ceiling,
        &key(),
        now,
    ));
    assert!(!shown(&partial).contains(&"crates/shared/src/lib.rs"));
    assert_eq!(
        attribute(&partial, &audit, "crates/shared/src/lib.rs"),
        Some(UnavailableReason::Unknown)
    );
    // A scope that is no longer current attributes nothing but staleness.
    assert_eq!(
        reader.attribute(
            &audit,
            &corpus.scope("agent"),
            now,
            "crates/secret/src/key.rs"
        ),
        Some(UnavailableReason::Stale)
    );
}

// r[verify kiln.source.projection.disclosure.unavailable.scenario.probe]
#[test]
fn missing_and_forbidden_probes_share_one_public_shape() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let reader = view_of(&corpus, "project-reader");
    let trusted = corpus.trusted("project-reader").view().clone();
    let reader_scope = corpus.scope("project-reader");
    // A hidden destination is not in the holder-scoped enumeration at all, so
    // it is as missing as an absent one.
    let hidden = reader.lookup(&trusted, &reader_scope, now, "crates/secret/src/key.rs");
    let missing = reader.lookup(&trusted, &reader_scope, now, "crates/secret/src/absent.rs");
    // A destination the holder may read but not discover is enumerated and
    // forbidden by name.
    let reader_ceiling = corpus.ceiling("project-reader");
    let read_only_scope = corpus.scope_with(
        vec![
            grant("crates/app", &[Op::ReadContent]),
            grant("crates/shared", &[Op::Discover, Op::ReadContent]),
        ],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    let read_only_trusted = corpus.enumerate(
        &corpus.definition("project-reader"),
        &corpus.facts(),
        &read_only_scope,
        &reader_ceiling,
    );
    let read_only = ok(restrict(
        &read_only_trusted,
        &read_only_scope,
        &reader_ceiling,
        &key(),
        now,
    ));
    let forbidden = read_only.lookup(
        read_only_trusted.view(),
        &read_only_scope,
        now,
        "crates/app/src/main.rs",
    );
    let errors = [hidden, missing, forbidden]
        .map(|result| result.err().unwrap_or_else(|| unreachable!("unavailable")));
    assert_eq!(
        errors.map(|error| error.reason()),
        [
            UnavailableReason::Missing,
            UnavailableReason::Missing,
            UnavailableReason::Forbidden
        ]
    );
    for error in errors {
        assert_eq!(
            ok(serde_json::to_string(&error.public())),
            r#"{"error":"unavailable"}"#
        );
    }
    ok(reader.lookup(&trusted, &reader_scope, now, "crates/app/src/main.rs"));
    // The trusted owner attributes a refused probe against its whole-ceiling
    // audit enumeration: the existing hidden file is forbidden and the absent
    // one missing, while the holder received one public shape for both.
    let unscoped = corpus.audit(&corpus.definition("project-reader"), &corpus.facts());
    assert_eq!(
        [
            "crates/secret/src/key.rs",
            "crates/secret/src/absent.rs",
            "crates/app/src/main.rs",
        ]
        .map(|destination| reader.attribute(&unscoped, &reader_scope, now, destination)),
        [
            Some(UnavailableReason::Forbidden),
            Some(UnavailableReason::Missing),
            None
        ]
    );
    let [hidden_error, missing_error, _] = errors;
    assert_eq!(hidden_error.public(), missing_error.public());
    let moved = corpus.audit(
        &corpus.definition("project-reader"),
        &corpus.facts_at(&ok(ExternalRef::new("git", "commit", &"6".repeat(40)))),
    );
    assert_eq!(
        reader.attribute(&moved, &reader_scope, now, "crates/secret/src/key.rs"),
        Some(UnavailableReason::AuditMismatch)
    );

    // An alias of a hidden origin is as unavailable as a missing path.
    let agent = view_of(&corpus, "agent");
    let agent_trusted = corpus.trusted("agent").view().clone();
    let agent_scope = corpus.scope("agent");
    assert_eq!(
        unavailable(agent.lookup(&agent_trusted, &agent_scope, now, "aliases/shared.rs")),
        UnavailableReason::Missing
    );
    assert_eq!(
        unavailable(agent.lookup(&agent_trusted, &agent_scope, now, "aliases/absent.rs")),
        UnavailableReason::Missing
    );
    // Origin probes outside discovery are refused, never classified.
    let definition = corpus.definition("agent");
    let scope = corpus.scope("agent");
    for hidden in [
        "crates/shared/src/lib.rs",
        "crates/shared/src/absent.rs",
        "../etc",
    ] {
        assert_eq!(
            unavailable(agent.lookup_origin(&definition, &scope, now, hidden)),
            UnavailableReason::Forbidden,
            "{hidden}"
        );
    }
    // A stale trusted view is refused.
    assert_eq!(
        unavailable(reader.lookup(&agent_trusted, &reader_scope, now, "crates/app/src/main.rs")),
        UnavailableReason::Stale
    );
    // A cached view is not served after revocation, narrowing or expiry.
    let narrowed = corpus.scope_with(
        vec![grant(
            "crates/app",
            &[
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    let holder = &corpus.view("project-reader").holder;
    let mut facts = crate::projection::authority::EffectiveScopeFacts {
        audience: holder.audience.clone(),
        repository: corpus.repository().identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: corpus.revision.clone(),
        },
        revision: corpus.revision.clone(),
        view_binding: crate::projection::restricted_corpus::corpus_view_binding(),
        admitted: holder.grants.clone(),
        capabilities: Vec::new(),
        not_after_s: holder.not_after_s,
        depth: holder.depth,
        generation: crate::projection::restricted_corpus::GENERATION,
        revocation_generation: crate::projection::restricted_corpus::REVOCATION_GENERATION + 1,
        provenance: crate::implementation::authority::AuthorityProvenance::VerifiedUcan,
    };
    facts.capabilities =
        crate::projection::authority::SessionCapability::under(&facts.context(), &holder.grants);
    let revoked = ok(crate::projection::authority::EffectiveSourceScope::new(
        facts,
    ));
    for (scope, at) in [
        (&narrowed, now),
        (&revoked, now),
        (&reader_scope, holder.not_after_s),
    ] {
        assert_eq!(
            unavailable(reader.lookup(&trusted, scope, at, "crates/app/src/main.rs")),
            UnavailableReason::Stale
        );
    }
}

// r[verify kiln.source.projection.disclosure.unavailable.scenario.coverage]
#[test]
fn coverage_distinguishes_unrequested_absent_unknown_and_truncated() {
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let mut definition = corpus.view("maintainer").definition.clone();
    definition.id = "corpus-crates".into();
    definition.selection.include = vec!["crates/**".into()];
    definition.selection.exclude = vec!["crates/secret/**".into()];
    let admitted = ok(admit_definition(&definition));
    let ceiling = ViewCeiling::from_definition(&admitted);
    let scope = corpus.scope("maintainer");
    let trusted = corpus.trusted_with("maintainer", &admitted, &corpus.facts());
    let view = ok(restrict(&trusted, &scope, &ceiling, &key(), now));
    assert_eq!(view.coverage(), VisibleCoverage::Complete);
    fn lookup_in<'a>(
        view: &'a AudienceView,
        definition: &AdmittedDefinition,
        scope: &crate::projection::authority::EffectiveSourceScope,
        now: u64,
        path: &str,
    ) -> OriginLookup<'a> {
        ok(view.lookup_origin(definition, scope, now, path))
    }
    fn higher_ranked<F: for<'a> Fn(&'a AudienceView, &str) -> OriginLookup<'a>>(f: F) -> F {
        f
    }
    let lookup = higher_ranked(|view, path| lookup_in(view, &admitted, &scope, now, path));
    assert!(matches!(
        lookup(&view, "crates/app/src/main.rs"),
        OriginLookup::Entries(entries) if entries.len() == 1
    ));
    assert_eq!(
        lookup(&view, "crates/secret/src/key.rs"),
        OriginLookup::Unrequested
    );
    assert_eq!(
        lookup(&view, "crates/app/src/absent.rs"),
        OriginLookup::Absent
    );

    // The adapter stopped while enumerating the holder's own selection.
    let selection = ScopedSelection::for_holder(&scope, &ceiling);
    let mut truncated_facts = ok(selection.filter_facts(&corpus.facts()));
    truncated_facts.coverage = crate::projection::Coverage::Truncated;
    let partial = ok(ScopedEnumeration::new(
        &selection,
        corpus.resolved(&admitted, &truncated_facts),
    ));
    let unknown = ok(restrict(&partial, &scope, &ceiling, &key(), now));
    // A truncated holder-scoped enumeration is an incomplete listing, and a
    // probe of a discoverable path it did not list is unknown.
    assert_eq!(
        unknown.coverage(),
        VisibleCoverage::Truncated {
            bound: unknown.entries().len()
        }
    );
    assert_eq!(
        lookup(&unknown, "crates/app/src/absent.rs"),
        OriginLookup::Unknown
    );

    let bound = 2;
    let cut = ok(restrict_bounded(
        &trusted,
        &scope,
        &ceiling,
        &key(),
        now,
        bound,
    ));
    assert_eq!(cut.coverage(), VisibleCoverage::Truncated { bound });
    assert_eq!(cut.entries().len(), bound);
    assert_eq!(
        lookup(&cut, "crates/shared/src/lib.rs"),
        OriginLookup::Truncated { bound }
    );
    // The four states are distinct values, never interchangeable defaults.
    let states = [
        OriginLookup::Unrequested,
        OriginLookup::Absent,
        OriginLookup::Unknown,
        OriginLookup::Truncated { bound },
    ];
    for (index, state) in states.iter().enumerate() {
        for other in states.iter().skip(index + 1) {
            assert_ne!(state, other);
        }
    }
    // Coverage counts only authorized entries: the reader's view of the
    // same snapshot is complete although most of it is hidden.
    assert_eq!(
        view_of(&corpus, "project-reader").coverage(),
        VisibleCoverage::Complete
    );
}

/// A source adapter enumerating `facts` over `selection` that stops after
/// `bound` entries, as a real shell's adapter would.
fn adapter_enumeration(
    facts: &ProjectionFacts,
    selection: &ScopedSelection,
    bound: usize,
) -> ProjectionFacts {
    let mut listed = ok(selection.filter_facts(facts));
    if listed.entries.len() > bound {
        listed.entries.truncate(bound);
        listed.coverage = crate::projection::Coverage::Truncated;
    }
    listed
}

/// A generated entry, which has no native origin.
fn schema_output() -> crate::projection::GeneratedEntry {
    crate::projection::GeneratedEntry {
        destination: "generated/schema.rs".into(),
        kind: EntryKind::File,
        generator: crate::projection::ComponentVersion {
            name: "codegen".into(),
            version: "1".into(),
        },
        input_identity: digest("schema-input"),
    }
}

// r[verify kiln.source.projection.disclosure.unavailable.scenario.scoped]
#[test]
fn hidden_entries_never_truncate_a_holder_scoped_enumeration() {
    use crate::projection::Coverage;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let definition = corpus.definition("project-reader");
    let ceiling = corpus.ceiling("project-reader");
    let scope = corpus.scope("project-reader");
    let selection = ScopedSelection::for_holder(&scope, &ceiling);
    assert_eq!(
        selection
            .paths()
            .iter()
            .map(|path| path.components.join("/"))
            .collect::<Vec<_>>(),
        [".cairn/changes/app-feature", "crates/app", "crates/shared"]
    );
    // Many hidden files join the snapshot. An adapter bound the holder's own
    // entries fit in would stop early over the whole view selection...
    let mut facts = corpus.facts();
    for index in 0..64 {
        facts.entries.push(SourceEntry {
            path: alloc::format!("crates/secret/src/gen{index:02}.rs"),
            object: ok(ExternalRef::new(
                "git",
                "blob",
                &alloc::format!("hidden-{index}"),
            )),
            kind: EntryKind::File,
        });
    }
    facts.entries.sort_by(|a, b| a.path.cmp(&b.path));
    let bound = ok(selection.filter_facts(&corpus.facts())).entries.len();
    let whole_view = ScopedSelection::for_holder(&corpus.scope("maintainer"), &ceiling);
    let wide = adapter_enumeration(&facts, &whole_view, bound);
    assert_eq!(wide.coverage, Coverage::Truncated);
    // Filtering that truncated wider enumeration per holder is refused: its
    // truncation was decided by hidden entries. Complete input is filtered.
    assert_eq!(
        selection.filter_facts(&wide).err(),
        Some(ScopingError::TruncatedInput)
    );
    let filtered = ok(selection.filter_facts(&facts));
    assert_eq!(filtered.coverage, Coverage::Complete);
    assert_eq!(
        filtered
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        [
            ".cairn/changes/app-feature/tasks.md",
            "crates/app/Cargo.toml",
            "crates/app/src/main.rs",
            "crates/app/src/util.rs",
            "crates/shared/src/lib.rs",
        ]
    );
    // The holder-scoped enumeration lists every authorized entry, so the
    // reader's view is complete and equal to the one without them.
    let scoped = adapter_enumeration(&facts, &selection, bound);
    assert_eq!(scoped.coverage, Coverage::Complete);
    let trusted = ok(ScopedEnumeration::new(
        &selection,
        corpus.resolved(&definition, &scoped),
    ));
    let view = ok(restrict(&trusted, &scope, &ceiling, &key(), now));
    assert_eq!(view.coverage(), VisibleCoverage::Complete);
    assert_eq!(shown(&view), corpus.view("project-reader").expect.visible);
    assert_eq!(
        view.presentation(),
        view_of(&corpus, "project-reader").presentation()
    );
    assert_eq!(
        ok(view.lookup_origin(&definition, &scope, now, "crates/app/src/absent.rs")),
        OriginLookup::Absent
    );

    // When the holder's own entries exceed the adapter bound, the listing is
    // truncated after what it listed and an unlisted path is unknown.
    let short = adapter_enumeration(&facts, &selection, bound - 1);
    assert_eq!(short.coverage, Coverage::Truncated);
    let trusted = ok(ScopedEnumeration::new(
        &selection,
        corpus.resolved(&definition, &short),
    ));
    let view = ok(restrict(&trusted, &scope, &ceiling, &key(), now));
    assert_eq!(
        view.coverage(),
        VisibleCoverage::Truncated {
            bound: view.entries().len()
        }
    );
    assert!(matches!(
        ok(view.lookup_origin(&definition, &scope, now, "crates/app/src/main.rs")),
        OriginLookup::Entries(_)
    ));
    for unlisted in ["crates/app/src/absent.rs", "crates/shared/src/lib.rs"] {
        assert_eq!(
            ok(view.lookup_origin(&definition, &scope, now, unlisted)),
            OriginLookup::Unknown,
            "{unlisted}"
        );
    }
}

// r[verify kiln.source.projection.disclosure.unavailable.scenario.scoped]
#[test]
fn an_enumeration_over_another_selection_is_refused() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let definition = corpus.definition("project-reader");
    let ceiling = corpus.ceiling("project-reader");
    let scope = corpus.scope("project-reader");
    // A manifest enumerated over the unscoped view selection is stale for
    // the reader, as is the reader's own after its grant narrows.
    let unscoped = corpus.enumerate(
        &definition,
        &corpus.facts(),
        &corpus.scope("maintainer"),
        &ceiling,
    );
    assert_eq!(
        refused(restrict(&unscoped, &scope, &ceiling, &key(), now)),
        DisclosureRefusal::Stale
    );
    let narrowed = corpus.scope_with(
        vec![grant("crates/app", &[Op::Discover, Op::ReadContent])],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    assert_eq!(
        refused(restrict(
            &corpus.trusted("project-reader"),
            &narrowed,
            &ceiling,
            &key(),
            now
        )),
        DisclosureRefusal::Stale
    );

    // An enumeration holding entries outside its selection is refused.
    let selection = ScopedSelection::for_holder(&scope, &ceiling);
    assert_eq!(
        ScopedEnumeration::new(&selection, corpus.resolved(&definition, &corpus.facts())).err(),
        Some(DisclosureRefusal::OutsideSelection)
    );
    // Generated entries stay only in a selection covering the root.
    let mut facts = corpus.facts();
    facts.generated.push(schema_output());
    let scoped = ok(selection.filter_facts(&facts));
    assert!(scoped.generated.is_empty() && !selection.covers_root());
    ok(ScopedEnumeration::new(
        &selection,
        corpus.resolved(&definition, &scoped),
    ));
    let mut smuggled = scoped;
    smuggled.generated = facts.generated.clone();
    assert_eq!(
        ScopedEnumeration::new(&selection, corpus.resolved(&definition, &smuggled)).err(),
        Some(DisclosureRefusal::OutsideSelection)
    );
    let full =
        ScopedSelection::for_holder(&corpus.scope("maintainer"), &corpus.ceiling("maintainer"));
    assert!(full.covers_root());
    assert_eq!(ok(full.filter_facts(&facts)).generated, facts.generated);
}

// r[verify kiln.source.projection.disclosure.handles.scenario.cursor]
#[test]
fn cursors_and_caches_bind_scope_and_recheck_current_authority() {
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let reader = view_of(&corpus, "project-reader");
    let scope = corpus.scope("project-reader");
    let first = ok(reader.page(&key(), &scope, now, None, 4));
    assert_eq!(first.entries.len(), 4);
    let next = first.next.unwrap_or_else(|| unreachable!("more entries"));
    let token = next.to_token();
    let parsed = ViewCursor::from_token(&token).unwrap_or_else(|| unreachable!("token"));
    let second = ok(reader.page(&key(), &scope, now, Some(&parsed), 4));
    assert_eq!(second.entries.len(), 2);
    assert!(second.next.is_none());
    let json = ok(serde_json::to_string(&next));
    assert_eq!(ok(serde_json::from_str::<ViewCursor>(&json)), next);

    // A broader holder's cursor or cached view does not serve the reader.
    let maintainer = view_of(&corpus, "maintainer");
    let maintainer_scope = corpus.scope("maintainer");
    let broad = ok(maintainer.page(&key(), &maintainer_scope, now, None, 4))
        .next
        .unwrap_or_else(|| unreachable!("more entries"));
    assert_eq!(
        unavailable(reader.page(&key(), &scope, now, Some(&broad), 4)),
        UnavailableReason::Stale
    );
    assert_eq!(
        unavailable(maintainer.page(&key(), &scope, now, None, 4)),
        UnavailableReason::Stale
    );
    assert_eq!(
        unavailable(maintainer.check_current(&scope, now)),
        UnavailableReason::Stale
    );
    assert_ne!(maintainer.binding().identity(), reader.binding().identity());
    // A cursor under another key, or with a moved position, is refused.
    assert_eq!(
        unavailable(reader.page(&other_key(), &scope, now, Some(&next), 4)),
        UnavailableReason::Stale
    );
    let mut forged = token.clone();
    forged.replace_range(0..8, "00000001");
    let forged = ViewCursor::from_token(&forged).unwrap_or_else(|| unreachable!("token"));
    assert_eq!(
        unavailable(reader.page(&key(), &scope, now, Some(&forged), 4)),
        UnavailableReason::Stale
    );
    // Revocation, a new generation or expiry invalidate the cursor on use.
    let holder = &corpus.view("project-reader").holder;
    type Change = fn(&mut crate::projection::authority::EffectiveScopeFacts);
    let changes: [Change; 2] = [
        |facts| facts.revocation_generation += 1,
        |facts| facts.generation += 1,
    ];
    for change in changes {
        let mut facts = crate::projection::authority::EffectiveScopeFacts {
            audience: holder.audience.clone(),
            repository: corpus.repository().identity().clone(),
            snapshot: SnapshotRule::Exact {
                revision: corpus.revision.clone(),
            },
            revision: corpus.revision.clone(),
            view_binding: crate::projection::restricted_corpus::corpus_view_binding(),
            admitted: holder.grants.clone(),
            capabilities: Vec::new(),
            not_after_s: holder.not_after_s,
            depth: holder.depth,
            generation: crate::projection::restricted_corpus::GENERATION,
            revocation_generation: crate::projection::restricted_corpus::REVOCATION_GENERATION,
            provenance: crate::implementation::authority::AuthorityProvenance::VerifiedUcan,
        };
        change(&mut facts);
        facts.capabilities = crate::projection::authority::SessionCapability::under(
            &facts.context(),
            &holder.grants,
        );
        let current = ok(crate::projection::authority::EffectiveSourceScope::new(
            facts,
        ));
        assert_eq!(
            unavailable(reader.page(&key(), &current, now, Some(&next), 4)),
            UnavailableReason::Stale
        );
    }
    assert_eq!(
        unavailable(reader.page(&key(), &scope, holder.not_after_s, Some(&next), 4)),
        UnavailableReason::Stale
    );
    let prefix = token.get(..8).unwrap_or_default();
    for invalid in ["", "zz", prefix, &alloc::format!("{token}00")] {
        assert!(ViewCursor::from_token(invalid).is_none(), "{invalid}");
    }
}

// r[verify kiln.source.projection.disclosure.handles.scenario.narrowing]
#[test]
fn narrowing_keeps_canonical_hashes_and_invalidates_bindings() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let definition = corpus.definition("project-reader");
    let trusted = corpus.trusted("project-reader").view().clone();
    let ceiling = corpus.ceiling("project-reader");
    let reader = view_of(&corpus, "project-reader");
    let alias = handle_of(&reader, "aliases/shared.rs");
    let narrowed_scope = corpus.scope_with(
        vec![grant(
            "crates/app",
            &[Op::Discover, Op::ReadContent, Op::ReadHistory],
        )],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    let narrowed_trusted =
        corpus.enumerate(&definition, &corpus.facts(), &narrowed_scope, &ceiling);
    let narrowed = ok(restrict(
        &narrowed_trusted,
        &narrowed_scope,
        &ceiling,
        &key(),
        now,
    ));
    // Canonical content is untouched by access: each holder-scoped
    // enumeration holds canonical entries with their canonical origins.
    let canonical = corpus.resolved(&definition, &corpus.facts());
    for scoped in [&trusted, narrowed_trusted.view()] {
        assert!(
            scoped
                .manifest()
                .entries
                .iter()
                .all(|entry| canonical.manifest().entries.contains(entry))
        );
    }
    // The access binding, handles and cached view are not.
    assert_ne!(narrowed.binding().identity(), reader.binding().identity());
    assert_ne!(
        handle_of(&narrowed, "crates/app/src/main.rs"),
        handle_of(&reader, "crates/app/src/main.rs")
    );
    assert_eq!(
        unavailable(reader.check_current(&narrowed_scope, now)),
        UnavailableReason::Stale
    );
    assert_eq!(
        unavailable(reader.resolve(&trusted, &narrowed_scope, now, &alias, Op::ReadContent)),
        UnavailableReason::Stale
    );
    assert_eq!(
        unavailable(narrowed.resolve(
            narrowed_trusted.view(),
            &narrowed_scope,
            now,
            &alias,
            Op::ReadContent
        )),
        UnavailableReason::Missing
    );
    assert!(!shown(&narrowed).contains(&"aliases/shared.rs"));
    assert!(!shown(&narrowed).contains(&"crates/shared/src/lib.rs"));
}

// r[verify kiln.source.projection.sharing.ceiling.scenario.edits]
#[test]
fn view_edits_aliases_and_closure_stay_within_the_ceiling() {
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let original = corpus.ceiling("project-reader");
    let paths: Vec<(Vec<String>, bool)> = original
        .paths()
        .iter()
        .map(|path| (path.components.clone(), path.subtree))
        .collect();
    assert_eq!(
        paths,
        vec![
            (vec![".cairn".to_string(), "changes".to_string()], true),
            (vec!["crates".to_string()], true),
        ]
    );
    assert!(
        !original
            .operations()
            .contains(&SourceViewOperation::ModifySource)
    );
    assert!(
        !original
            .operations()
            .contains(&SourceViewOperation::PrepareWorkspace)
    );
    let agent_ceiling = corpus.ceiling("agent");
    assert!(
        agent_ceiling
            .operations()
            .contains(&SourceViewOperation::ModifySource)
    );
    let exported: ProjectionDefinition = ok(serde_json::from_str(include_str!(
        "../../../../../fixtures/source-projection/definition.json"
    )));
    let literal = ViewCeiling::from_definition(&ok(admit_definition(&exported)));
    assert_eq!(
        literal
            .paths()
            .iter()
            .map(|path| path.components.join("/"))
            .collect::<Vec<_>>(),
        vec!["crates/kiln-core".to_string()]
    );

    // The definition is edited: documentation joins the selection and an
    // alias places it inside the reader's granted crate.
    let mut edited = corpus.view("project-reader").definition.clone();
    edited.selection.include.push("docs/**".into());
    edited.mounts.push(Mount {
        source_prefix: "docs/guide.md".into(),
        destination_prefix: "crates/app/guide.md".into(),
    });
    let edited = ok(admit_definition(&edited));
    let facts = corpus.facts();
    assert!(
        corpus
            .resolved(&edited, &facts)
            .manifest()
            .entries
            .iter()
            .any(|entry| entry.destination == "crates/app/guide.md")
    );
    // The reader's grant decides by origin: the alias stays hidden.
    let edited_ceiling = ViewCeiling::from_definition(&edited);
    let reader_scope = corpus.scope("project-reader");
    let reader = ok(restrict(
        &corpus.enumerate(&edited, &facts, &reader_scope, &edited_ceiling),
        &reader_scope,
        &edited_ceiling,
        &key(),
        now,
    ));
    assert!(!shown(&reader).contains(&"crates/app/guide.md"));
    assert!(!shown(&reader).contains(&"docs/guide.md"));
    // Under the frozen ceiling, even full source gains nothing from the edit.
    let maintainer = corpus.scope("maintainer");
    let frozen = ok(restrict(
        &corpus.enumerate(&edited, &facts, &maintainer, &original),
        &maintainer,
        &original,
        &key(),
        now,
    ));
    assert!(!shown(&frozen).contains(&"crates/app/guide.md"));
    assert!(!shown(&frozen).contains(&"docs/guide.md"));

    // Owner-side audit over the expanded definition (or repository root)
    // covers the frozen ceiling and attributes both docs/guide.md and its
    // alias as Forbidden rather than Missing (#110).
    let expanded_whole = ScopedSelection::whole(&edited_ceiling);
    let expanded_audit = ok(ScopedEnumeration::new(
        &expanded_whole,
        corpus.resolved(&edited, &ok(expanded_whole.filter_facts(&facts))),
    ));
    let root_audit = ok(ScopedEnumeration::new(
        &ScopedSelection::root(),
        corpus.resolved(&edited, &facts),
    ));
    for audit in [&expanded_audit, &root_audit] {
        assert_eq!(
            frozen.attribute(audit, &maintainer, now, "docs/guide.md"),
            Some(UnavailableReason::Forbidden)
        );
        assert_eq!(
            frozen.attribute(audit, &maintainer, now, "crates/app/guide.md"),
            Some(UnavailableReason::Forbidden)
        );
        assert_eq!(
            frozen.attribute(audit, &maintainer, now, "docs/absent.md"),
            Some(UnavailableReason::Missing)
        );
    }

    // Backstop against mislabelling a holder's own narrowed view as a
    // whole-ceiling audit: never attribute hidden files as Missing (#110).
    let reader_definition = corpus.definition("project-reader");
    let reader_ceiling = corpus.ceiling("project-reader");
    let reader_scoped = corpus.trusted("project-reader");
    let reader_view = view_of(&corpus, "project-reader");
    let mislabelled = ok(ScopedEnumeration::new(
        &ScopedSelection::whole(&reader_ceiling),
        reader_scoped.view().clone(),
    ));
    assert_eq!(
        reader_view.attribute(&mislabelled, &reader_scope, now, "crates/secret/src/key.rs"),
        None
    );
    // Even for a full-source holder restricted under a narrower frozen ceiling,
    // mislabelling its frozen-ceiling view with the expanded definition's
    // selection is refused.
    let frozen_enumerated = corpus.enumerate(&edited, &facts, &maintainer, &original);
    let mislabelled_frozen = ok(ScopedEnumeration::new(
        &expanded_whole,
        frozen_enumerated.view().clone(),
    ));
    assert_eq!(
        frozen.attribute(&mislabelled_frozen, &maintainer, now, "docs/guide.md"),
        None
    );
    // An audit for a different definition is an owner-side audit mismatch,
    // distinct from holder staleness.
    let stale_definition_audit = corpus.audit(&reader_definition, &facts);
    assert_eq!(
        frozen.attribute(&stale_definition_audit, &maintainer, now, "docs/guide.md"),
        Some(UnavailableReason::AuditMismatch)
    );
}

// r[verify kiln.source.projection.disclosure.unavailable.scenario.dependencies]
#[test]
fn denied_and_missing_dependencies_are_the_same_unavailable() {
    let corpus = Corpus::load();
    let definition = corpus.definition("agent");
    let ceiling = corpus.ceiling("agent");
    let restricted = |status: DependencyStatus| {
        let mut facts = corpus.facts();
        for dependency in &mut facts.dependencies {
            dependency.status = status.clone();
        }
        let trusted = corpus.trusted_with("agent", &definition, &facts);
        let incomplete = trusted
            .view()
            .clone()
            .require_complete(crate::projection::ProjectionOperation::Write)
            .is_err();
        let view = ok(restrict(
            &trusted,
            &corpus.scope("agent"),
            &ceiling,
            &key(),
            corpus.now_s,
        ));
        (view, incomplete)
    };
    let (denied, denied_incomplete) = restricted(DependencyStatus::Denied);
    let (missing, missing_incomplete) = restricted(DependencyStatus::Missing);
    assert!(denied_incomplete && missing_incomplete);
    assert_eq!(denied.dependencies(), missing.dependencies());
    assert_eq!(denied.content_identity(), missing.content_identity());
    let dependency = denied
        .dependencies()
        .first()
        .unwrap_or_else(|| unreachable!("declared"));
    assert!(!dependency.available && dependency.identity.is_none());
}

// r[verify kiln.source.projection.disclosure.cairn.scenario.hidden_prerequisite]
#[test]
fn restricted_context_hides_prerequisites_and_binds_its_audience() {
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let observation = corpus.observation();
    let context_for = |name: &str| {
        let view = view_of(&corpus, name);
        ok(restrict_context(
            &view,
            &observation,
            &corpus.scope(name),
            &key(),
            now,
        ))
    };
    let reader = context_for("project-reader");
    let agent = context_for("agent");
    let maintainer = context_for("maintainer");

    // The hidden prerequisite is a handle; its status is withheld because the
    // reader may not discover its change.
    let task = reader
        .items()
        .iter()
        .find(|item| item.native_id == "app-feature.task.1")
        .unwrap_or_else(|| unreachable!("visible task"));
    let hidden = reader
        .hidden_prerequisites()
        .first()
        .unwrap_or_else(|| unreachable!("hidden prerequisite"));
    assert_eq!(hidden.status, PrerequisiteStatus::Unavailable);
    assert_eq!(task.prerequisites, vec![hidden.handle]);
    assert_eq!(reader.blocked(&task.handle), Some(true));
    assert_eq!(task.text.as_deref(), Some("Wire the app entry point"));
    let presented = ok(serde_json::to_string(&reader.presentation()));
    for secret in [
        "secret-rotation",
        "Rotate the signing key",
        "Audit key consumers",
    ] {
        assert!(!presented.contains(secret), "{secret}");
    }
    assert!(!presented.contains(observation.identity().as_str()));
    // The agent may discover its change but not read its text.
    assert!(agent.items().iter().all(|item| item.text.is_none()));
    // The maintainer sees every item; nothing is hidden from it.
    assert_eq!(maintainer.items().len(), 4);
    assert!(maintainer.hidden_prerequisites().is_empty());
    let maintainer_task = maintainer
        .items()
        .iter()
        .find(|item| item.native_id == "app-feature.task.1")
        .unwrap_or_else(|| unreachable!("visible task"));
    assert_eq!(maintainer.blocked(&maintainer_task.handle), Some(true));

    // Each delivered context is bound to its audience's binding; the
    // underlying observation stays the same server-side.
    assert_eq!(reader.observation(), observation.identity());
    assert_eq!(agent.observation(), observation.identity());
    assert_ne!(reader.identity(), agent.identity());
    assert_eq!(reader.binding().audience(), "did:key:z6MkReader");
    // Handles differ across audiences: an allowed task link is not portable.
    assert_ne!(
        task.handle, maintainer_task.handle,
        "a link from one view grants nothing in another"
    );
    assert_eq!(reader.blocked(&maintainer_task.handle), None);
    // A stale scope delivers nothing.
    let reader_view = view_of(&corpus, "project-reader");
    let reader_scope = corpus.scope("project-reader");
    assert_eq!(
        unavailable(restrict_context(
            &reader_view,
            &observation,
            &corpus.scope("agent"),
            &key(),
            now
        )),
        UnavailableReason::Stale
    );
    // An observation of another repository or revision is refused.
    let moved = ok(ExternalRef::new("git", "commit", &"5".repeat(40)));
    for (repository, revision) in [
        (digest("other-repository"), corpus.revision.clone()),
        (corpus.repository().identity().clone(), moved),
    ] {
        let foreign = ok(CairnObservation::new(
            repository,
            revision,
            corpus.cairn.clone(),
        ));
        assert_eq!(
            unavailable(restrict_context(
                &reader_view,
                &foreign,
                &reader_scope,
                &key(),
                now
            )),
            UnavailableReason::Stale
        );
    }

    // Status is shown where the holder may discover the prerequisite's
    // change, even when the view's ceiling hides the item itself.
    let mut narrow = corpus.view("project-reader").definition.clone();
    narrow.selection.include = vec!["crates/**".into(), ".cairn/changes/app-feature/**".into()];
    let narrow = ok(admit_definition(&narrow));
    let both = corpus.scope_with(
        vec![
            grant("crates/app", &[SourceViewOperation::Discover]),
            grant(".cairn/changes", &[SourceViewOperation::Discover]),
        ],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    let narrow_ceiling = ViewCeiling::from_definition(&narrow);
    let narrow_view = ok(restrict(
        &corpus.enumerate(&narrow, &corpus.facts(), &both, &narrow_ceiling),
        &both,
        &narrow_ceiling,
        &key(),
        now,
    ));
    let permitted = ok(restrict_context(
        &narrow_view,
        &observation,
        &both,
        &key(),
        now,
    ));
    assert_eq!(
        permitted
            .hidden_prerequisites()
            .iter()
            .map(|hidden| hidden.status)
            .collect::<Vec<_>>(),
        vec![PrerequisiteStatus::Open]
    );
}

// r[verify kiln.source.projection.disclosure.cairn.scenario.observation]
#[test]
fn cairn_observation_admits_only_change_scoped_items() {
    let corpus = Corpus::load();
    let item = |id: &str, path: &str, prerequisites: &[&str]| CairnItem {
        change: "app-feature".into(),
        kind: CairnItemKind::Task,
        native_id: id.into(),
        path: path.into(),
        text: "text".into(),
        status: CairnStatus::Open,
        prerequisites: prerequisites.iter().map(ToString::to_string).collect(),
    };
    let codes = |items: Vec<CairnItem>| -> Vec<String> {
        match CairnObservation::new(
            corpus.repository().identity().clone(),
            corpus.revision.clone(),
            items,
        ) {
            Ok(_) => unreachable!("invalid observation admitted"),
            Err(diagnostics) => diagnostics.into_iter().map(|d| d.code).collect(),
        }
    };
    assert_eq!(
        codes(vec![item("t", ".cairn/changes/other/tasks.md", &[])]),
        vec!["path_outside_change"]
    );
    assert_eq!(
        codes(vec![item("t", ".cairn/changes/app-feature/../x.md", &[])]),
        vec!["path_outside_change"]
    );
    assert_eq!(
        codes(vec![item(
            "t",
            ".cairn/changes/app-feature/tasks.md",
            &["missing"]
        )]),
        vec!["unknown_prerequisite"]
    );
    assert_eq!(
        codes(vec![
            item("t", ".cairn/changes/app-feature/tasks.md", &[]),
            item("t", ".cairn/changes/app-feature/tasks.md", &[]),
        ]),
        vec!["duplicate_native_id"]
    );
    // Input order never changes the observation.
    let mut reversed = corpus.cairn.clone();
    reversed.reverse();
    assert_eq!(
        ok(CairnObservation::new(
            corpus.repository().identity().clone(),
            corpus.revision.clone(),
            reversed
        ))
        .identity(),
        corpus.observation().identity()
    );
}

// r[verify kiln.source.projection.disclosure.restricted.scenario.presentation]
#[test]
fn presentations_carry_no_trusted_binding_or_native_identity() {
    let corpus = Corpus::load();
    let reader = view_of(&corpus, "project-reader");
    let agent = view_of(&corpus, "agent");
    for view in [&reader, &agent] {
        let presented = ok(serde_json::to_string(&view.presentation()));
        let binding = view.binding();
        for trusted in [
            binding.manifest().as_str(),
            binding.identity().as_str(),
            binding.effective_scope().as_str(),
            binding.definition().as_str(),
            binding.repository().as_str(),
            binding.snapshot().native_id.as_str(),
            "equal-bytes",
            "app-main",
            "crates/secret",
            "fake.compact.token",
        ] {
            assert!(!presented.contains(trusted), "{trusted}");
        }
        assert!(!alloc::format!("{view:?}").contains("crates/shared/src/lib.rs"));
    }
    let agent_presented = ok(serde_json::to_string(&agent.presentation()));
    for hidden in ["aliases/shared.rs", "crates/shared", "crates/apple"] {
        assert!(!agent_presented.contains(hidden), "{hidden}");
    }
    assert_eq!(alloc::format!("{:?}", key()), "HandleKey(<redacted>)");
}

// r[verify kiln.source.projection.authority.scope.scenario.admitted_only]
#[test]
fn an_unchecked_sibling_capability_discloses_nothing() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let holder = &corpus.view("project-reader").holder;
    let mut port = FakeVerifiedSourceAuthority::default();
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
    // The session's capability set also reports a sibling crate that the
    // delegation chain rule never checked for this holder.
    port.report_unchecked(
        &holder.audience,
        grant("crates/secret", &[Op::Discover, Op::ReadContent]),
    );
    let profile = verified(&holder.audience);
    let admit = |port: &mut FakeVerifiedSourceAuthority, operation: Op, path: &str| {
        let request = SourceAuthorityRequest {
            operation,
            repository: corpus.repository().identity().clone(),
            snapshot: SnapshotRule::Exact {
                revision: corpus.revision.clone(),
            },
            view_binding: corpus.ceiling("project-reader").identity().clone(),
            scope: OriginScope::Components {
                path: PathScope::from_path(path, true).unwrap_or_else(|| unreachable!("{path}")),
            },
            request_digest: [4; 32],
            attempt: None,
        };
        port.admit(&request, &profile, &corpus.current(), now)
            .map(|decision| decision.session.unwrap_or_else(|| unreachable!("session")))
            .map_err(|decision| decision.reason)
    };
    // The sibling itself is never admitted.
    match admit(&mut port, Op::Discover, "crates/secret") {
        Ok(session) => unreachable!("unchecked sibling admitted: {session:?}"),
        Err(reason) => assert_eq!(reason, SourceDenialReason::ResourceOutOfScope),
    }
    let mut scopes = Vec::new();
    for (operation, path) in [
        (Op::Discover, "crates/app"),
        (Op::ReadContent, "crates/app"),
        (Op::Discover, ".cairn/changes/app-feature"),
    ] {
        let session = ok(admit(&mut port, operation, path));
        let scope = port
            .effective_scope(session)
            .cloned()
            .unwrap_or_else(|| unreachable!("live session"));
        assert_eq!(scope.grants().len(), 1, "{operation:?} {path}");
        assert!(!scope.permits(Op::Discover, "crates/secret/src/key.rs"));
        scopes.push(scope);
    }
    let scope = ok(crate::projection::authority::EffectiveSourceScope::join(
        &scopes,
    ));
    let ceiling = corpus.ceiling("project-reader");
    let trusted = corpus.enumerate(
        &corpus.definition("project-reader"),
        &corpus.facts(),
        &scope,
        &ceiling,
    );
    let view = ok(restrict(&trusted, &scope, &ceiling, &key(), now));
    assert_eq!(
        shown(&view),
        [
            ".cairn/changes/app-feature/tasks.md",
            "crates/app/Cargo.toml",
            "crates/app/src/main.rs",
            "crates/app/src/util.rs",
        ]
    );
    assert_eq!(
        unavailable(view.lookup(trusted.view(), &scope, now, "crates/secret/src/key.rs")),
        UnavailableReason::Missing
    );
    assert_eq!(
        unavailable(view.lookup_origin(
            &corpus.definition("project-reader"),
            &scope,
            now,
            "crates/secret/src/key.rs"
        )),
        UnavailableReason::Forbidden
    );
    let presented = ok(serde_json::to_string(&view.presentation()));
    assert!(!presented.contains("crates/secret"));
}

// r[verify kiln.source.projection.disclosure.handles.scenario.keyed]
#[test]
fn generated_entries_need_full_source_and_resolve_rechecks_the_manifest() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let mut facts = corpus.facts();
    facts.generated.push(schema_output());
    let restricted = |name: &str| {
        let trusted = corpus.trusted_with(name, &corpus.definition(name), &facts);
        let view = ok(restrict(
            &trusted,
            &corpus.scope(name),
            &corpus.ceiling(name),
            &key(),
            now,
        ));
        (trusted, view)
    };
    let (maintainer_trusted, maintainer) = restricted("maintainer");
    let generated = maintainer
        .entries()
        .iter()
        .find(|entry| entry.display_path.as_deref() == Some("generated/schema.rs"))
        .unwrap_or_else(|| unreachable!("full source sees generated output"));
    assert_eq!(generated.status, VisibleStatus::Generated);
    let resolved = ok(maintainer.resolve(
        maintainer_trusted.view(),
        &corpus.scope("maintainer"),
        now,
        &generated.handle,
        Op::ReadContent,
    ));
    assert_eq!(resolved.destination, "generated/schema.rs");
    // A root grant of discovery and reading sees generated output, but is not
    // full source for any other operation on it.
    let maintainer_ceiling = corpus.ceiling("maintainer");
    let root_scope = corpus.scope_with(
        vec![grant("", &[Op::Discover, Op::ReadContent])],
        &corpus.view("maintainer").holder.audience,
        &corpus.revision,
    );
    let root_trusted = corpus.enumerate(
        &corpus.definition("maintainer"),
        &facts,
        &root_scope,
        &maintainer_ceiling,
    );
    let root_view = ok(restrict(
        &root_trusted,
        &root_scope,
        &maintainer_ceiling,
        &key(),
        now,
    ));
    let root_generated = handle_of(&root_view, "generated/schema.rs");
    ok(root_view.resolve(
        root_trusted.view(),
        &root_scope,
        now,
        &root_generated,
        Op::ReadContent,
    ));
    for operation in [Op::ReadHistory, Op::ModifySource] {
        assert_eq!(
            unavailable(root_view.resolve(
                root_trusted.view(),
                &root_scope,
                now,
                &root_generated,
                operation
            )),
            UnavailableReason::Forbidden,
            "{operation:?}"
        );
    }
    for name in ["project-reader", "agent"] {
        let (trusted, view) = restricted(name);
        let holds_generated = |view: &crate::projection::AdmittedPartialView| {
            view.manifest()
                .entries
                .iter()
                .any(|entry| entry.destination == "generated/schema.rs")
        };
        assert!(
            holds_generated(&corpus.resolved(&corpus.definition(name), &facts)),
            "{name} unscoped resolution holds the generated entry"
        );
        assert!(!holds_generated(trusted.view()), "{name}");
        assert!(!shown(&view).contains(&"generated/schema.rs"), "{name}");
        assert_eq!(
            unavailable(view.resolve(
                trusted.view(),
                &corpus.scope(name),
                now,
                &generated.handle,
                Op::ReadContent
            )),
            UnavailableReason::Missing,
            "{name}"
        );
    }
    // A handle resolved against another trusted view is stale.
    let reader = view_of(&corpus, "project-reader");
    let main = handle_of(&reader, "crates/app/src/main.rs");
    assert_eq!(
        unavailable(reader.resolve(
            corpus.trusted("agent").view(),
            &corpus.scope("project-reader"),
            now,
            &main,
            Op::ReadContent
        )),
        UnavailableReason::Stale
    );
}

// r[verify kiln.source.projection.disclosure.restricted.scenario.presentation]
#[test]
fn read_only_by_handle_entries_leak_no_order_count_or_kind() {
    use SourceViewOperation as Op;
    let corpus = Corpus::load();
    let now = corpus.now_s;
    let scope = corpus.scope_with(
        vec![
            grant("crates/app", &[Op::ReadContent]),
            grant("crates/shared", &[Op::Discover, Op::ReadContent]),
        ],
        "did:key:z6MkReader",
        &corpus.revision,
    );
    let ceiling = corpus.ceiling("project-reader");
    let view = ok(restrict(
        &corpus.enumerate(
            &corpus.definition("project-reader"),
            &corpus.facts(),
            &scope,
            &ceiling,
        ),
        &scope,
        &ceiling,
        &key(),
        now,
    ));
    let entries = view.entries();
    let first_undisplayed = entries
        .iter()
        .position(|entry| entry.display_path.is_none())
        .unwrap_or(entries.len());
    let (displayed, undisplayed) = entries.split_at(first_undisplayed);
    assert_eq!(
        displayed
            .iter()
            .filter_map(|entry| entry.display_path.as_deref())
            .collect::<Vec<_>>(),
        ["aliases/shared.rs", "crates/shared/src/lib.rs"]
    );
    assert_eq!(undisplayed.len(), 3);
    assert!(
        undisplayed.iter().all(|entry| {
            entry.display_path.is_none() && entry.kind.is_none() && entry.readable
        })
    );
    let handles: Vec<OpaqueHandle> = undisplayed.iter().map(|entry| entry.handle).collect();
    let mut sorted = handles.clone();
    sorted.sort();
    assert_eq!(handles, sorted, "ordered by handle, not by source path");
    assert!(displayed.iter().all(|entry| entry.kind.is_some()));
}

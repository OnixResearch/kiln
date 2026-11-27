use std::collections::{BTreeMap, BTreeSet};

use kiln_core::projection::authority::PathScope;
use kiln_core::projection::disclosure::ViewCeiling;
use kiln_core::projection::manifest_store::{
    BrokerDenial, BrokerRevocationState, BrokerTargetKind, BrokerTicketRequest, BrokerTicketScope,
    CasCondition, EvictionDecision, EvictionPolicy, MAX_MANIFEST_ENVELOPE_BYTES,
    ManifestLifecycleStage, RetentionRoot, RetentionRootKind, StorageTier, TierPresence,
};
use kiln_core::projection::{
    ComponentVersion, Coverage, DEFINITION_SCHEMA, DependencyDeclaration, EntryKind,
    GeneratedInputs, Mount, ProjectionDefinition, ProjectionFacts, ProjectionOperation,
    ProjectionProfile, ProviderBinding, RepositoryEnrollment, RepositoryLocator,
    ResolvedProjectionManifest, Selection, SourceCapability, SourceEntry, TargetProfile,
    admit_definition, admit_repositories, resolve,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};

use super::super::*;
use super::support::{TEST_MAXIMUM_BYTES, admitted, reference};

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|err| unreachable!("unexpected error: {err:?}"))
}

fn digest(label: &str) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(label.as_bytes()).to_hex().to_string(),
    ))
}

fn digest_bytes(bytes: &[u8]) -> ContentId {
    ok(ContentId::from_blake3_hex(
        blake3::hash(bytes).to_hex().to_string(),
    ))
}

fn projection_def(id: &str, root: &str, include: Vec<&str>) -> kiln_core::projection::AdmittedDefinition {
    let def = ProjectionDefinition {
        schema: DEFINITION_SCHEMA.to_string(),
        id: id.to_string(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: root.to_string(),
            include: include.into_iter().map(String::from).collect(),
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::<DependencyDeclaration>::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    };
    ok(admit_definition(&def))
}

struct ManifestFixture {
    manifest: ResolvedProjectionManifest,
    object_bytes: BTreeMap<ExternalRef, Vec<u8>>,
    shared_bytes: Vec<u8>,
    shared_digest: ContentId,
    obj_b_ref: ExternalRef,
}

fn build_manifest_fixture() -> ManifestFixture {
    let repo = ok(admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".to_string(),
            binding: ProviderBinding {
                source_provider: ok(ProviderProfileId::new("git-source-v1")),
                native_repository: reference("github", "repository", "OnixResearch/kiln"),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: "/srv/kiln".to_string(),
            }],
        }],
        &[],
    ))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repository"));

    let def = projection_def("workspace-all", "", vec!["crates/a/**", "crates/b/**"]);
    let obj_a_ref = reference("git", "blob", "blob-a");
    let obj_b_ref = reference("git", "blob", "blob-b");
    let facts = ProjectionFacts {
        repository: repo.identity().clone(),
        exact_revision: reference("git", "commit", "1111111111111111111111111111111111111111"),
        canonical_tree: digest("canonical-tree-1"),
        evaluator: ComponentVersion {
            name: "kiln-eval".to_string(),
            version: "1.0.0".to_string(),
        },
        adapter: ComponentVersion {
            name: "git-adapter".to_string(),
            version: "1.0.0".to_string(),
        },
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![
            SourceEntry {
                path: "crates/a/lib.rs".to_string(),
                object: obj_a_ref.clone(),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "crates/b/secret.rs".to_string(),
                object: obj_b_ref.clone(),
                kind: EntryKind::File,
            },
        ],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let manifest = ok(resolve(&def, &repo, &facts))
        .require_complete(ProjectionOperation::Build)
        .unwrap_or_else(|e| unreachable!("{:?}", e.diagnostics))
        .manifest()
        .clone();

    let shared_bytes = b"pub fn shared_source_bytes() -> u32 { 42 }\n".to_vec();
    let shared_digest = digest_bytes(&shared_bytes);
    let mut object_bytes = BTreeMap::new();
    object_bytes.insert(obj_a_ref.clone(), shared_bytes.clone());
    object_bytes.insert(obj_b_ref.clone(), shared_bytes.clone());

    ManifestFixture {
        manifest,
        object_bytes,
        shared_bytes,
        shared_digest,
        obj_b_ref,
    }
}

fn s3_enabled_config() -> S3TierConfig {
    S3TierConfig {
        enabled: true,
        contract_version: CONDITIONAL_OBJECT_STORE_PORT_VERSION,
        endpoint_ref: "s3-private-endpoint-v1".to_string(),
        bucket_ref: "kiln-projected-manifests-internal-bucket".to_string(),
        key_prefix: "tenant-1".to_string(),
        max_evict_batch: 64,
    }
}

fn permissive_local_eviction_policy() -> EvictionPolicy {
    EvictionPolicy {
        high_watermark_entries: 1,
        low_watermark_entries: 0,
        high_watermark_bytes: 1,
        low_watermark_bytes: 0,
        require_secondary_before_primary_eviction: false,
    }
}

fn strict_tiered_eviction_policy() -> EvictionPolicy {
    EvictionPolicy {
        high_watermark_entries: 1,
        low_watermark_entries: 0,
        high_watermark_bytes: 1,
        low_watermark_bytes: 0,
        require_secondary_before_primary_eviction: true,
    }
}

#[test]
fn local_molten_roundtrip_refuses_unconfigured_s3_and_partial_closure() {
    let fixture = build_manifest_fixture();
    let port = InMemoryAspenContent::default();
    let mut store = ok(ProjectedManifestStore::new_local_only(
        admitted(),
        port,
        permissive_local_eviction_policy(),
    ));
    assert!(!store.s3_enabled());

    // Off-by-default S3 fails closed when replication is requested.
    let s3_off = store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Published,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        true,
        None,
    );
    assert!(matches!(s3_off, Err(ManifestStoreFailure::Domain(d)) if d.code == "s3_tier_disabled"));

    // Incomplete object closure is refused before publishing any manifest root.
    let mut partial_objects = fixture.object_bytes.clone();
    partial_objects.remove(&fixture.obj_b_ref);
    let partial = store.persist_manifest(
        &fixture.manifest,
        &partial_objects,
        ManifestLifecycleStage::Published,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        false,
        None,
    );
    assert!(
        matches!(partial, Err(ManifestStoreFailure::Domain(d)) if d.code == "incomplete_object_closure")
    );
    assert!(store.placement(&fixture.manifest.identity).is_none());

    // Complete closure persists and reloads identically from MoltenPrimary.
    let outcome = ok(store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Published,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        false,
        None,
    ));
    let PersistManifestOutcome::Persisted {
        record_identity,
        placement,
    } = outcome
    else {
        unreachable!("expected persisted outcome");
    };
    assert_eq!(placement.molten_primary, TierPresence::Verified);
    assert_eq!(placement.s3_secondary, TierPresence::Absent);
    assert!(!placement.s3_enabled);

    let reloaded = ok(store.load_manifest(
        &fixture.manifest.identity,
        None,
        MAX_MANIFEST_ENVELOPE_BYTES,
    ));
    assert_eq!(reloaded.served_from, StorageTier::MoltenPrimary);
    assert_eq!(reloaded.manifest, fixture.manifest);
    assert_eq!(reloaded.envelope.record_identity, record_identity);
    assert_eq!(
        ok(store.load_blob_verified(&fixture.shared_digest, TEST_MAXIMUM_BYTES)),
        fixture.shared_bytes
    );

    // Evicting from MoltenPrimary without S3Secondary leaves manifest unavailable.
    let evict_decision = ok(store.evict_manifest(
        &fixture.manifest.identity,
        StorageTier::MoltenPrimary,
        1,
        1_024,
    ));
    assert!(matches!(evict_decision, EvictionDecision::Evicted { .. }));

    let after_evict = store.load_manifest(
        &fixture.manifest.identity,
        None,
        MAX_MANIFEST_ENVELOPE_BYTES,
    );
    assert!(matches!(
        after_evict,
        Err(ManifestStoreFailure::EvictedWithoutSecondary(_))
    ));
}

#[test]
fn s3_tier_roundtrip_cas_conflict_and_eviction_secondary_readback() {
    let fixture = build_manifest_fixture();
    let molten = InMemoryAspenContent::default();
    let s3 = InMemoryConditionalObjectStore::default();
    let mut store = ok(ProjectedManifestStore::with_optional_s3(
        admitted(),
        molten,
        Some(s3),
        s3_enabled_config(),
        strict_tiered_eviction_policy(),
    ));
    assert!(store.s3_enabled());

    let pending_roots = BTreeSet::from([RetentionRoot {
        kind: RetentionRootKind::PendingPublication,
        root_id: "pub-1".to_string(),
    }]);
    let PersistManifestOutcome::Persisted {
        record_identity,
        placement,
    } = ok(store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Verified,
        CasCondition::PutIfAbsent,
        pending_roots,
        true,
        None,
    ))
    else {
        unreachable!("expected persisted");
    };
    assert_eq!(placement.molten_primary, TierPresence::Verified);
    assert_eq!(placement.s3_secondary, TierPresence::Verified);

    // Duplicate PutIfAbsent fails CAS without overwriting.
    let dup = store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Adopted,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        true,
        None,
    );
    assert!(matches!(dup, Err(ManifestStoreFailure::CasConflict(_))));

    // ExpectedDigest with wrong digest fails CAS.
    let stale_cas = store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Adopted,
        CasCondition::ExpectedDigest {
            expected_record_identity: digest("wrong-record-identity"),
        },
        BTreeSet::new(),
        true,
        None,
    );
    assert!(matches!(stale_cas, Err(ManifestStoreFailure::CasConflict(_))));

    // ExpectedDigest with exact prior record_identity advances stage cleanly.
    let adopted = ok(store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Adopted,
        CasCondition::ExpectedDigest {
            expected_record_identity: record_identity,
        },
        BTreeSet::from([RetentionRoot {
            kind: RetentionRootKind::PendingPublication,
            root_id: "pub-1".to_string(),
        }]),
        true,
        None,
    ));
    assert!(matches!(adopted, PersistManifestOutcome::Persisted { .. }));

    // PendingPublication blocks hot-tier eviction.
    let blocked_evict = store.evict_manifest(
        &fixture.manifest.identity,
        StorageTier::MoltenPrimary,
        2,
        4_096,
    );
    assert!(
        matches!(blocked_evict, Err(ManifestStoreFailure::Domain(d)) if d.code == "retention_root_protected")
    );

    // Transition retention root to RetainedSnapshot: hot-tier eviction is now admitted,
    // while durable S3Secondary eviction remains forbidden.
    ok(store.update_retention_roots(
        &fixture.manifest.identity,
        BTreeSet::from([RetentionRoot {
            kind: RetentionRootKind::RetainedSnapshot,
            root_id: "snap-1".to_string(),
        }]),
    ));
    let s3_evict_refused = store.evict_manifest(
        &fixture.manifest.identity,
        StorageTier::S3Secondary,
        2,
        4_096,
    );
    assert!(
        matches!(s3_evict_refused, Err(ManifestStoreFailure::Domain(d)) if d.code == "durable_retention_protected")
    );

    let hot_evicted = ok(store.evict_manifest(
        &fixture.manifest.identity,
        StorageTier::MoltenPrimary,
        2,
        4_096,
    ));
    assert!(matches!(hot_evicted, EvictionDecision::Evicted { .. }));

    // Readback transparently recovers from S3Secondary with full verification.
    let reloaded = ok(store.load_manifest(
        &fixture.manifest.identity,
        None,
        MAX_MANIFEST_ENVELOPE_BYTES,
    ));
    assert_eq!(reloaded.served_from, StorageTier::S3Secondary);
    assert_eq!(reloaded.manifest, fixture.manifest);
    assert_eq!(
        ok(store.load_blob_verified(&fixture.shared_digest, TEST_MAXIMUM_BYTES)),
        fixture.shared_bytes
    );

    // Tampering bytes on S3Secondary triggers IntegrityMismatch on readback.
    let s3_port = store
        .s3_port()
        .unwrap_or_else(|| unreachable!("s3 port present"));
    ok(s3_port.set_next_fault(S3Fault::CorruptNextRead));
    let tampered = store.load_manifest(
        &fixture.manifest.identity,
        None,
        MAX_MANIFEST_ENVELOPE_BYTES,
    );
    assert!(matches!(
        tampered,
        Err(ManifestStoreFailure::IntegrityMismatch(_))
    ));
}

#[test]
fn tampered_molten_primary_bytes_are_refused() {
    let fixture = build_manifest_fixture();
    let molten = InMemoryAspenContent::default();
    let mut store = ok(ProjectedManifestStore::new_local_only(
        admitted(),
        molten.clone(),
        permissive_local_eviction_policy(),
    ));
    ok(store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Published,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        false,
        None,
    ));

    let blob_ref = ExternalRef::new(
        "molten",
        CONTENT_REFERENCE_KIND,
        fixture.shared_digest.as_str(),
    );
    ok(molten.replace_unchecked(&ok(blob_ref), b"tampered-bytes".to_vec()));
    let blob_res = store.load_blob_verified(&fixture.shared_digest, TEST_MAXIMUM_BYTES);
    assert!(matches!(
        blob_res,
        Err(ManifestStoreFailure::IntegrityMismatch(_))
    ));

    // Also tamper the stored manifest bytes directly in Molten.
    let manifest_json = ok(serde_json::to_vec(&fixture.manifest));
    let manifest_blob_digest = digest_bytes(&manifest_json);
    let manifest_ref = ok(ExternalRef::new(
        "molten",
        CONTENT_REFERENCE_KIND,
        manifest_blob_digest.as_str(),
    ));
    ok(molten.replace_unchecked(&manifest_ref, b"{}".to_vec()));
    let manifest_res = store.load_manifest(
        &fixture.manifest.identity,
        None,
        MAX_MANIFEST_ENVELOPE_BYTES,
    );
    assert!(matches!(
        manifest_res,
        Err(ManifestStoreFailure::IntegrityMismatch(_))
    ));
}

#[test]
fn ambiguous_s3_write_and_evict_preserve_unknown_explicitly() {
    let fixture = build_manifest_fixture();
    let molten = InMemoryAspenContent::default();
    let s3 = InMemoryConditionalObjectStore::default();
    let mut store = ok(ProjectedManifestStore::with_optional_s3(
        admitted(),
        molten,
        Some(s3.clone()),
        s3_enabled_config(),
        strict_tiered_eviction_policy(),
    ));

    ok(s3.set_next_fault(S3Fault::UnknownAfterPut));
    let outcome = ok(store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Published,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        true,
        None,
    ));
    let PersistManifestOutcome::Unknown {
        record_identity,
        placement,
        ..
    } = outcome
    else {
        unreachable!("expected PersistManifestOutcome::Unknown");
    };
    assert_eq!(placement.molten_primary, TierPresence::Verified);
    assert_eq!(placement.s3_secondary, TierPresence::Unknown);

    // Subsequent CAS write against S3Secondary refuses while state is Unknown.
    let retry = store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Adopted,
        CasCondition::ExpectedDigest {
            expected_record_identity: record_identity,
        },
        BTreeSet::new(),
        true,
        None,
    );
    assert!(
        matches!(retry, Err(ManifestStoreFailure::Domain(d)) if d.code == "cas_unknown_prior_state")
    );

    // Now test UnknownAfterEvict on a fresh manifest.
    let molten2 = InMemoryAspenContent::default();
    let s3_2 = InMemoryConditionalObjectStore::default();
    let mut store2 = ok(ProjectedManifestStore::with_optional_s3(
        admitted(),
        molten2,
        Some(s3_2.clone()),
        s3_enabled_config(),
        permissive_local_eviction_policy(),
    ));
    ok(store2.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Published,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        true,
        None,
    ));
    ok(s3_2.set_next_fault(S3Fault::UnknownAfterEvict));
    let evict_s3 = ok(store2.evict_manifest(
        &fixture.manifest.identity,
        StorageTier::S3Secondary,
        1,
        1_024,
    ));
    assert!(matches!(
        evict_s3,
        EvictionDecision::Unknown { placement, .. }
            if placement.s3_secondary == TierPresence::Unknown
    ));

    // Evict MoltenPrimary too; load_manifest must surface UnknownOutcome rather than NotFound.
    ok(store2.evict_manifest(
        &fixture.manifest.identity,
        StorageTier::MoltenPrimary,
        1,
        1_024,
    ));
    let load_unknown = store2.load_manifest(
        &fixture.manifest.identity,
        None,
        MAX_MANIFEST_ENVELOPE_BYTES,
    );
    assert!(matches!(
        load_unknown,
        Err(ManifestStoreFailure::UnknownOutcome(_))
    ));
}

#[test]
fn broker_enforces_view_ceiling_shared_blob_isolation_expiry_and_revocation() {
    let fixture = build_manifest_fixture();
    let molten = InMemoryAspenContent::default();
    let s3 = InMemoryConditionalObjectStore::default();
    let mut store = ok(ProjectedManifestStore::with_optional_s3(
        admitted(),
        molten,
        Some(s3),
        s3_enabled_config(),
        strict_tiered_eviction_policy(),
    ));
    ok(store.persist_manifest(
        &fixture.manifest,
        &fixture.object_bytes,
        ManifestLifecycleStage::Adopted,
        CasCondition::PutIfAbsent,
        BTreeSet::new(),
        true,
        None,
    ));

    let mut broker = ProjectedManifestBroker::new(
        store,
        BrokerRevocationState {
            current_generation: 1,
            revocation_generation: 1,
            ..BrokerRevocationState::default()
        },
    );

    let def_a = projection_def("view-a", "crates/a", vec!["**"]);
    let ceiling_a = ViewCeiling::from_definition(&def_a);
    let view_a_id = digest("view-binding-a");
    let scope_a = BrokerTicketScope {
        repository: fixture.manifest.repository.clone(),
        exact_revision: fixture.manifest.exact_revision.clone(),
        view_binding: view_a_id.clone(),
        audience: "did:key:worker-a".to_string(),
        ceiling: ceiling_a,
        generation: 1,
        revocation_generation: 1,
        allowed_paths: BTreeSet::from([PathScope {
            components: vec!["crates".to_string(), "a".to_string()],
            subtree: true,
        }]),
        max_bytes: TEST_MAXIMUM_BYTES,
    };

    // View A cannot mint a ticket for crates/b/secret.rs even though both blobs share the exact same digest!
    let cross_view_issue = broker.issue_ticket(
        &fixture.manifest.identity,
        BrokerTargetKind::Blob {
            destination: "crates/b/secret.rs".to_string(),
            object_digest: fixture.shared_digest.clone(),
            max_bytes: fixture.shared_bytes.len(),
        },
        scope_a.clone(),
        1_000,
        2_000,
    );
    assert!(
        matches!(cross_view_issue, Err(ManifestStoreFailure::Domain(d)) if d.code == "blob_outside_view_ceiling")
    );

    // View A cannot request FullManifest either.
    let full_issue = broker.issue_ticket(
        &fixture.manifest.identity,
        BrokerTargetKind::FullManifest,
        scope_a.clone(),
        1_000,
        2_000,
    );
    assert!(
        matches!(full_issue, Err(ManifestStoreFailure::Domain(d)) if d.code == "full_manifest_outside_ceiling")
    );

    // View A CAN mint ManifestView and receives ONLY crates/a/lib.rs.
    let view_ticket = ok(broker.issue_ticket(
        &fixture.manifest.identity,
        BrokerTargetKind::ManifestView,
        scope_a.clone(),
        1_000,
        2_000,
    ));
    let view_payload = ok(broker.download(
        &view_ticket,
        &BrokerTicketRequest {
            ticket_id: view_ticket.ticket_id.clone(),
            audience: "did:key:worker-a".to_string(),
            view_binding: view_a_id.clone(),
            expected_digest: fixture.manifest.identity.clone(),
            path: None,
            requested_bytes: TEST_MAXIMUM_BYTES,
        },
        1_100,
    ));
    let BrokeredPayload::ManifestView { entries, .. } = view_payload else {
        unreachable!("expected ManifestView");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries.first().map(|e| e.destination.as_str()),
        Some("crates/a/lib.rs")
    );

    // View A CAN download its authorized blob, and ticket JSON never exposes S3 bucket/key/credentials.
    let blob_ticket = ok(broker.issue_ticket(
        &fixture.manifest.identity,
        BrokerTargetKind::Blob {
            destination: "crates/a/lib.rs".to_string(),
            object_digest: fixture.shared_digest.clone(),
            max_bytes: fixture.shared_bytes.len(),
        },
        scope_a,
        1_000,
        2_000,
    ));
    let serialized_ticket = ok(serde_json::to_string(&blob_ticket));
    assert!(!serialized_ticket.contains("kiln-projected-manifests-internal-bucket"));
    assert!(!serialized_ticket.contains("s3-private-endpoint-v1"));
    assert!(!serialized_ticket.contains("tenant-1/blobs"));

    let blob_req = BrokerTicketRequest {
        ticket_id: blob_ticket.ticket_id.clone(),
        audience: "did:key:worker-a".to_string(),
        view_binding: view_a_id,
        expected_digest: fixture.shared_digest.clone(),
        path: Some("crates/a/lib.rs".to_string()),
        requested_bytes: fixture.shared_bytes.len(),
    };
    let downloaded = ok(broker.download(&blob_ticket, &blob_req, 1_100));
    assert!(matches!(
        downloaded,
        BrokeredPayload::Blob { bytes, .. } if bytes == fixture.shared_bytes
    ));

    // Another audience or path or wrong digest is refused before storage read.
    let mut wrong_audience = blob_req.clone();
    wrong_audience.audience = "did:key:worker-b".to_string();
    assert_eq!(
        broker.download(&blob_ticket, &wrong_audience, 1_100),
        Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::OutOfScope))
    );

    let mut wrong_path = blob_req.clone();
    wrong_path.path = Some("crates/b/secret.rs".to_string());
    assert_eq!(
        broker.download(&blob_ticket, &wrong_path, 1_100),
        Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::OutOfScope))
    );

    let mut wrong_digest = blob_req.clone();
    wrong_digest.expected_digest = digest("other-digest");
    assert_eq!(
        broker.download(&blob_ticket, &wrong_digest, 1_100),
        Err(ManifestStoreFailure::BrokerDenied(
            BrokerDenial::DigestMismatch
        ))
    );

    let mut over_bound = blob_req.clone();
    over_bound.requested_bytes = TEST_MAXIMUM_BYTES + 1;
    assert_eq!(
        broker.download(&blob_ticket, &over_bound, 1_100),
        Err(ManifestStoreFailure::BrokerDenied(
            BrokerDenial::ByteCeilingExceeded
        ))
    );

    // Expired ticket is refused.
    assert_eq!(
        broker.download(&blob_ticket, &blob_req, 2_000),
        Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::Expired))
    );

    // Revoked ticket is refused immediately even before expiry.
    broker.revoke_ticket(blob_ticket.ticket_id.clone());
    assert_eq!(
        broker.download(&blob_ticket, &blob_req, 1_100),
        Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::Revoked))
    );

    // Revocation generation bump invalidates existing view tickets too.
    broker.bump_revocation_generation();
    assert_eq!(
        broker.download(
            &view_ticket,
            &BrokerTicketRequest {
                ticket_id: view_ticket.ticket_id.clone(),
                audience: "did:key:worker-a".to_string(),
                view_binding: digest("view-binding-a"),
                expected_digest: fixture.manifest.identity.clone(),
                path: None,
                requested_bytes: TEST_MAXIMUM_BYTES,
            },
            1_100,
        ),
        Err(ManifestStoreFailure::BrokerDenied(BrokerDenial::Revoked))
    );
}

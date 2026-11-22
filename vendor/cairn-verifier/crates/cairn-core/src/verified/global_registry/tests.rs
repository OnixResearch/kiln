use super::*;

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const REGISTRY_NON_CLAIM: &str =
    "registry membership and graph linkage do not prove evidence truth or release eligibility";

fn policy() -> RegistryCorePolicy {
    RegistryCorePolicy {
        reserved_tag_namespaces: vec!["cairn.policy".to_string(), "cairn.system".to_string()],
        allowed_vocabulary_owners: vec!["cairn".to_string(), "valence".to_string()],
        required_non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
    }
}

fn candidates() -> RegistryCandidateRecords {
    RegistryCandidateRecords {
        sources: vec![RegistrySourceRecord {
            source_id: "source:cairn".to_string(),
            family: "change-graph".to_string(),
            schema: "cairn.change-graph.v1".to_string(),
            content_hash: HASH_A.to_string(),
            vocabulary_owner: "cairn".to_string(),
            source_ref: ".cairn/changes/graph.json".to_string(),
            non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
            record_hash: String::new(),
        }],
        nodes: vec![
            RegistryNode {
                stable_id: "change:a".to_string(),
                kind: "change".to_string(),
                schema: "cairn.change.v1".to_string(),
                content_hash: HASH_A.to_string(),
                payload_ref: ".cairn/changes/a".to_string(),
                source_id: "source:cairn".to_string(),
                vocabulary_owner: "cairn".to_string(),
                non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
                record_hash: String::new(),
            },
            RegistryNode {
                stable_id: "change:b".to_string(),
                kind: "change".to_string(),
                schema: "cairn.change.v1".to_string(),
                content_hash: HASH_B.to_string(),
                payload_ref: ".cairn/changes/b".to_string(),
                source_id: "source:cairn".to_string(),
                vocabulary_owner: "cairn".to_string(),
                non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
                record_hash: String::new(),
            },
        ],
        tags: vec![
            RegistryTagAssignment {
                assignment_id: "tag:a:channel".to_string(),
                subject_id: "change:a".to_string(),
                subject_kind: "node".to_string(),
                namespace: "operator".to_string(),
                key: "channel".to_string(),
                value: "dev".to_string(),
                source_id: "source:cairn".to_string(),
                non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
                record_hash: String::new(),
            },
            RegistryTagAssignment {
                assignment_id: "tag:a:owner".to_string(),
                subject_id: "change:a".to_string(),
                subject_kind: "node".to_string(),
                namespace: "operator".to_string(),
                key: "owner".to_string(),
                value: "onix".to_string(),
                source_id: "source:cairn".to_string(),
                non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
                record_hash: String::new(),
            },
        ],
        edges: vec![RegistryEdge {
            stable_id: "edge:a:b".to_string(),
            kind: "change_dependency".to_string(),
            from_id: "change:a".to_string(),
            to_id: "change:b".to_string(),
            vocabulary_owner: "cairn".to_string(),
            source_id: "source:cairn".to_string(),
            status: "declared".to_string(),
            non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
            record_hash: String::new(),
        }],
        adapters: vec![RegistryAdapterMetadata {
            adapter_id: "cairn.change-graph.v1".to_string(),
            adapter_kind: "ingest".to_string(),
            schema: "cairn.registry-adapter.v1".to_string(),
            capabilities: vec!["read".to_string()],
            record_families: vec!["change-graph".to_string()],
        }],
    }
}

fn snapshot() -> GlobalRegistrySnapshot {
    build_global_registry_snapshot("workspace:onix", candidates(), &policy())
        .snapshot
        .unwrap()
}

#[test]
// r[verify cairn.global_registry.snapshot.deterministic]
// r[verify cairn.global_registry.snapshot.rebuild]
// r[verify cairn.global_registry.validation.positive]
fn equivalent_input_order_rebuilds_same_snapshot() {
    let first = snapshot();
    let mut reordered = candidates();
    reordered.nodes.reverse();
    let second = build_global_registry_snapshot("workspace:onix", reordered, &policy())
        .snapshot
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.snapshot_hash, second.snapshot_hash);
}

#[test]
// r[verify cairn.global_registry.tags.evidence_label]
// r[verify cairn.global_registry.tags.authority]
// r[verify cairn.global_registry.query.tags]
fn provenance_tag_query_is_deterministic_and_non_authoritative() {
    let snapshot = snapshot();
    let first = query_registry_tags(
        &snapshot,
        &[
            (
                "operator".to_string(),
                "channel".to_string(),
                "dev".to_string(),
            ),
            (
                "operator".to_string(),
                "owner".to_string(),
                "onix".to_string(),
            ),
        ],
    );
    let second = query_registry_tags(
        &snapshot,
        &[
            (
                "operator".to_string(),
                "owner".to_string(),
                "onix".to_string(),
            ),
            (
                "operator".to_string(),
                "channel".to_string(),
                "dev".to_string(),
            ),
        ],
    );
    assert!(first.valid);
    assert_eq!(first, second);
    assert_eq!(first.selected_ids, vec!["change:a"]);
    assert!(first
        .non_claims
        .iter()
        .any(|claim| claim.contains("do not prove")));
}

#[test]
// r[verify cairn.global_registry.dag_profiles.acyclic]
// r[verify cairn.global_registry.query.graph]
fn dag_projection_has_stable_order_and_closure() {
    let report = evaluate_registry_graph(
        &snapshot(),
        &RegistryGraphProfile {
            id: "change-dependencies".to_string(),
            allowed_node_kinds: vec!["change".to_string()],
            allowed_edge_kinds: vec!["change_dependency".to_string()],
            require_dag: true,
        },
        None,
        RegistryGraphDirection::Descendants,
    );
    assert!(report.valid, "{:#?}", report.issues);
    assert_eq!(report.topological_order, vec!["change:b", "change:a"]);
}

#[test]
// r[verify cairn.global_registry.dag_profiles.cycle]
// r[verify cairn.global_registry.dag_profiles.non_dag]
fn cycles_fail_only_for_dag_profiles_with_canonical_witness() {
    let mut values = candidates();
    values.edges.push(RegistryEdge {
        stable_id: "edge:b:a".to_string(),
        kind: "change_dependency".to_string(),
        from_id: "change:b".to_string(),
        to_id: "change:a".to_string(),
        vocabulary_owner: "cairn".to_string(),
        source_id: "source:cairn".to_string(),
        status: "declared".to_string(),
        non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
        record_hash: String::new(),
    });
    let snapshot = build_global_registry_snapshot("workspace:onix", values, &policy())
        .snapshot
        .unwrap();
    let mut profile = RegistryGraphProfile {
        id: "change-dependencies".to_string(),
        allowed_node_kinds: vec!["change".to_string()],
        allowed_edge_kinds: vec!["change_dependency".to_string()],
        require_dag: true,
    };
    let dag = evaluate_registry_graph(&snapshot, &profile, None, RegistryGraphDirection::Both);
    assert!(!dag.valid);
    assert_eq!(dag.cycle_witness, vec!["change:a", "change:b", "change:a"]);
    profile.id = "related".to_string();
    profile.require_dag = false;
    let graph = evaluate_registry_graph(&snapshot, &profile, None, RegistryGraphDirection::Both);
    assert!(graph.valid);
    assert_eq!(
        graph.cycle_witness,
        vec!["change:a", "change:b", "change:a"]
    );
}

#[test]
// r[verify cairn.global_registry.snapshot.conflict]
// r[verify cairn.global_registry.tags.reserved]
// r[verify cairn.global_registry.validation]
// r[verify cairn.global_registry.validation.negative]
fn conflicts_dangling_edges_reserved_tags_and_claim_promotion_fail_closed() {
    let mut values = candidates();
    let mut conflict = values.nodes[0].clone();
    conflict.content_hash = HASH_B.to_string();
    values.nodes.push(conflict);
    values.edges[0].to_id = "change:missing".to_string();
    values.edges.push(RegistryEdge {
        stable_id: "edge:self".to_string(),
        kind: "change_dependency".to_string(),
        from_id: "change:a".to_string(),
        to_id: "change:a".to_string(),
        vocabulary_owner: "cairn".to_string(),
        source_id: "source:cairn".to_string(),
        status: "declared".to_string(),
        non_claims: vec![REGISTRY_NON_CLAIM.to_string()],
        record_hash: String::new(),
    });
    values.tags[0].namespace = "cairn.policy".to_string();
    values.sources[0].non_claims.clear();
    let report = build_global_registry_snapshot("workspace:onix", values, &policy());
    assert!(!report.valid);
    assert!(report.snapshot.is_none());
    let codes = report
        .issues
        .iter()
        .map(|row| row.code.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(codes.contains("registry.record_conflict"));
    assert!(codes.contains("registry.edge_unresolved_endpoint"));
    assert!(codes.contains("registry.edge_self_reference"));
    assert!(codes.contains("registry.tag_reserved_namespace"));
    assert!(codes.contains("registry.non_claims_missing"));
}

#[test]
// r[verify cairn.global_registry.portability.verify]
// r[verify cairn.global_registry.portability.tamper]
fn portable_snapshot_verification_rejects_tampering() {
    let original = snapshot();
    assert!(verify_global_registry_snapshot(&original, &policy()).valid);
    let mut tampered: serde_json::Value = serde_json::to_value(original).unwrap();
    tampered["nodes"][0]["content_hash"] = serde_json::Value::String(HASH_B.to_string());
    let tampered: GlobalRegistrySnapshot = serde_json::from_value(tampered).unwrap();
    let report = verify_global_registry_snapshot(&tampered, &policy());
    assert!(!report.valid);
    assert!(report
        .issues
        .iter()
        .any(|row| row.code == "registry.record_hash_mismatch"));

    let mut weakened = snapshot();
    weakened.non_claims.clear();
    let report = verify_global_registry_snapshot(&weakened, &policy());
    assert!(!report.valid);
    assert_eq!(
        report.issues[0].code,
        "registry.snapshot_non_claims_mismatch"
    );
}

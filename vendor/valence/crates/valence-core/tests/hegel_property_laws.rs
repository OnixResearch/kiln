use hegel::TestCase;
use hegel::generators as gs;
use valence_core::evidence_ir::EvidenceSpan;
use valence_core::evidence_ir::REQUIRED_NON_CLAIM;
use valence_core::evidence_ir::VerificationRole;
use valence_core::evidence_ir::build_evidence_artifact;
use valence_core::evidence_model::CacheDecisionKind;
use valence_core::evidence_model::CachedVerifierResult;
use valence_core::evidence_model::CapabilityPolicy;
use valence_core::evidence_model::EvidenceGraph;
use valence_core::evidence_model::EvidenceGraphEdge;
use valence_core::evidence_model::EvidenceGraphEdgeKind;
use valence_core::evidence_model::EvidenceGraphNode;
use valence_core::evidence_model::EvidenceGraphNodeKind;
use valence_core::evidence_model::EvidenceNameBinding;
use valence_core::evidence_model::NameBindingHistory;
use valence_core::evidence_model::NameBindingKind;
use valence_core::evidence_model::VerifierCacheKey;
use valence_core::evidence_model::VerifierCapabilityKind;
use valence_core::evidence_model::VerifierCapabilityLabel;
use valence_core::evidence_model::VerifierCapabilitySet;
use valence_core::evidence_model::decide_cache_reuse;
use valence_core::evidence_model::evidence_graph_identity;
use valence_core::evidence_model::validate_evidence_graph;
use valence_core::hegel_property::HEGEL_FAST_CASE_BUDGET;
use valence_core::hegel_property::render_b3_hash;

const HEGEL_FAST_PROPERTY_CASES: u64 = HEGEL_FAST_CASE_BUDGET;
const PROPERTY_MIN_SALT: u64 = 0;
const PROPERTY_MAX_SALT: u64 = 1_000;
const INITIAL_BINDING_SEQUENCE: u64 = 1;
const SPAN_START_BYTE: u64 = 0;
const SPAN_END_BYTE: u64 = 8;
const SPAN_START_LINE: u32 = 1;
const SPAN_START_COLUMN: u32 = 1;
const SPAN_END_LINE: u32 = 1;
const SPAN_END_COLUMN: u32 = 9;
const ARTIFACT_KIND: &str = "hegel-property-fixture";
const GRAPH_ALIAS: &str = "hegel-source";
const SOURCE_PATH_A: &str = "src/a.rs";
const SOURCE_PATH_B: &str = "src/relocated/a.rs";

#[hegel::test(test_cases = HEGEL_FAST_PROPERTY_CASES)]
fn hegel_artifact_identity_ignores_display_span_path(tc: TestCase) {
    let salt = draw_salt(&tc);
    let stable_id = format!("artifact:{salt}");
    let payload = serde_json::json!({"law": "display-path-churn", "salt": salt});
    let first = build_evidence_artifact(
        ARTIFACT_KIND,
        &stable_id,
        Some(span_for(SOURCE_PATH_A, salt)),
        VerificationRole::Property,
        payload.clone(),
        vec![REQUIRED_NON_CLAIM.to_owned()],
    )
    .expect("first artifact builds");
    let second = build_evidence_artifact(
        ARTIFACT_KIND,
        &stable_id,
        Some(span_for(SOURCE_PATH_B, salt)),
        VerificationRole::Property,
        payload,
        vec![REQUIRED_NON_CLAIM.to_owned()],
    )
    .expect("second artifact builds");

    assert_eq!(first.canonical_hash, second.canonical_hash);
}

#[hegel::test(test_cases = HEGEL_FAST_PROPERTY_CASES)]
fn hegel_graph_and_cache_identities_are_deterministic(tc: TestCase) {
    let salt = draw_salt(&tc);
    let graph = valid_graph(salt, false);
    let reordered_graph = valid_graph(salt, true);

    assert!(validate_evidence_graph(&graph).valid);
    assert!(validate_evidence_graph(&reordered_graph).valid);
    assert_eq!(
        evidence_graph_identity(&graph).expect("graph identity"),
        evidence_graph_identity(&reordered_graph).expect("reordered graph identity")
    );

    let cache_key = cache_key_for(salt, "policy-a");
    let cached = CachedVerifierResult {
        key: cache_key.clone(),
        result_hash: identity_for("result", salt),
    };
    let pure = VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Pure)]);
    let policy = CapabilityPolicy::new(Vec::new(), Vec::new());
    let decision = decide_cache_reuse(Some(&cached), &cache_key, &pure, &policy);
    assert_eq!(decision.decision, CacheDecisionKind::Hit);
    assert!(decision.issue_codes.is_empty());
}

#[hegel::test(test_cases = HEGEL_FAST_PROPERTY_CASES)]
fn hegel_generated_negative_graph_and_cache_inputs_fail_closed(tc: TestCase) {
    let salt = draw_salt(&tc);
    let duplicate_node_case = tc.draw(gs::booleans());
    let invalid_graph = if duplicate_node_case {
        duplicate_node_graph(salt)
    } else {
        dangling_edge_graph(salt)
    };

    let graph_report = validate_evidence_graph(&invalid_graph);
    assert!(!graph_report.valid);
    assert!(!graph_report.issue_codes.is_empty());

    let cache_key = cache_key_for(salt, "policy-a");
    let changed_key = cache_key_for(salt, "policy-b");
    let cached = CachedVerifierResult {
        key: cache_key,
        result_hash: identity_for("result", salt),
    };
    let pure = VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Pure)]);
    let policy = CapabilityPolicy::new(Vec::new(), Vec::new());
    let decision = decide_cache_reuse(Some(&cached), &changed_key, &pure, &policy);
    assert_eq!(decision.decision, CacheDecisionKind::Miss);
    assert!(!decision.issue_codes.is_empty());
}

fn draw_salt(tc: &TestCase) -> u64 {
    tc.draw(gs::integers::<u64>().min_value(PROPERTY_MIN_SALT).max_value(PROPERTY_MAX_SALT))
}

fn span_for(source_path: &str, salt: u64) -> EvidenceSpan {
    EvidenceSpan {
        source_id: format!("source:{salt}"),
        source_path: source_path.to_owned(),
        start_byte: SPAN_START_BYTE,
        end_byte: SPAN_END_BYTE,
        start_line: SPAN_START_LINE,
        start_column: SPAN_START_COLUMN,
        end_line: SPAN_END_LINE,
        end_column: SPAN_END_COLUMN,
        label: Some(format!("r[valence.hegel.{salt}]")),
    }
}

fn valid_graph(salt: u64, reordered: bool) -> EvidenceGraph {
    let source = identity_for("source", salt);
    let requirement = identity_for("requirement", salt);
    let receipt = identity_for("receipt", salt);
    let mut nodes = vec![
        EvidenceGraphNode::new(source.clone(), EvidenceGraphNodeKind::Definition, Some("source".to_owned())),
        EvidenceGraphNode::new(requirement.clone(), EvidenceGraphNodeKind::Requirement, Some("requirement".to_owned())),
        EvidenceGraphNode::new(receipt.clone(), EvidenceGraphNodeKind::Receipt, Some("receipt".to_owned())),
    ];
    let mut edges = vec![
        EvidenceGraphEdge::new(source.clone(), requirement, EvidenceGraphEdgeKind::CoversRequirement),
        EvidenceGraphEdge::new(source.clone(), receipt, EvidenceGraphEdgeKind::RecordedInReceipt),
    ];
    if reordered {
        nodes.reverse();
        edges.reverse();
    }
    EvidenceGraph::new(nodes, edges, binding_history(&source))
}

fn duplicate_node_graph(salt: u64) -> EvidenceGraph {
    let source = identity_for("source", salt);
    let nodes = vec![
        EvidenceGraphNode::new(source.clone(), EvidenceGraphNodeKind::Definition, Some("source-a".to_owned())),
        EvidenceGraphNode::new(source.clone(), EvidenceGraphNodeKind::Receipt, Some("source-b".to_owned())),
    ];
    EvidenceGraph::new(nodes, Vec::new(), binding_history(&source))
}

fn dangling_edge_graph(salt: u64) -> EvidenceGraph {
    let source = identity_for("source", salt);
    let missing = identity_for("missing", salt);
    let nodes = vec![EvidenceGraphNode::new(
        source.clone(),
        EvidenceGraphNodeKind::Definition,
        Some("source".to_owned()),
    )];
    let edges = vec![EvidenceGraphEdge::new(
        source.clone(),
        missing,
        EvidenceGraphEdgeKind::TracesTo,
    )];
    EvidenceGraph::new(nodes, edges, binding_history(&source))
}

fn binding_history(source: &str) -> NameBindingHistory {
    NameBindingHistory::new(vec![EvidenceNameBinding::new(
        NameBindingKind::Alias,
        GRAPH_ALIAS,
        source,
        INITIAL_BINDING_SEQUENCE,
        true,
    )])
}

fn cache_key_for(salt: u64, policy_label: &str) -> VerifierCacheKey {
    VerifierCacheKey::new(
        identity_for("input", salt),
        identity_for("verifier", salt),
        identity_for(policy_label, salt),
        identity_for("dependency", salt),
        identity_for("capability", salt),
    )
}

fn identity_for(prefix: &str, salt: u64) -> String {
    render_b3_hash(format!("{prefix}:{salt}").as_bytes())
}

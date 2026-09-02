// Test harness: closed fixtures use exact positions and fail immediately when construction or
// rejection invariants break.
#![allow(
    unknown_lints,
    clippy::expect_used,
    clippy::indexing_slicing,
    non_trait_imports,
    assertion_density,
    reason = "test-only fixture construction favors direct readable setup while production admission remains fully checked"
)]

use alloc::format;
use alloc::string::ToString;
use alloc::vec;

use crate::*;

const TEST_MAXIMUM_NODES: u32 = 8;
const TEST_MAXIMUM_EDGES: u32 = 16;
const TEST_MAXIMUM_LOG_REFERENCES: u32 = 8;
const TEST_MAXIMUM_LOG_READ_BYTES: u64 = 4_096;
const TEST_MAXIMUM_TOTAL_LOG_BYTES: u64 = 16_384;
const TEST_LOG_BYTES: u64 = 32;
const TEST_SEQUENCE: u64 = 7;
const TEST_NEXT_SEQUENCE: u64 = 8;
const TEST_EXCESS_CAPABILITY_ITEMS: usize = 257;

fn identity(digit: char) -> Identity {
    let hex = digit.to_string().repeat(BLAKE3_HEX_LENGTH);
    Identity::new(format!("{BLAKE3_PREFIX}{hex}")).expect("test identity is canonical")
}

fn label(value: &str) -> Label {
    Label::new(value.to_string()).expect("test label is canonical")
}

fn source_value(value: &str) -> SourceValue {
    SourceValue::new(value.to_string()).expect("test source value is bounded and control-free")
}

fn profile() -> CapabilityProfile {
    CapabilityProfile {
        schema: PROFILE_SCHEMA.to_string(),
        graph_schema: GRAPH_SCHEMA.to_string(),
        platforms: vec![label("linux-x86_64")],
        node_kinds: vec![label("build"), label("test")],
        effect_classes: vec![EffectClass::Build, EffectClass::Test],
        adapter_kinds: vec![label("local"), label("remote")],
        controls: ControlCapabilities {
            cancel: true,
            pause: true,
            resume: true,
        },
        limits: ContractLimits {
            maximum_nodes: TEST_MAXIMUM_NODES,
            maximum_edges: TEST_MAXIMUM_EDGES,
            maximum_log_references: TEST_MAXIMUM_LOG_REFERENCES,
            maximum_log_read_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
            maximum_total_log_bytes: TEST_MAXIMUM_TOTAL_LOG_BYTES,
        },
        non_claims: REQUIRED_NON_CLAIMS.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn request(profile: &CapabilityProfile) -> WorkflowRequest {
    let mut graph = WorkflowGraph {
        schema: GRAPH_SCHEMA.to_string(),
        identity: identity('0'),
        nodes: vec![
            WorkflowNode {
                id: label("build"),
                kind: label("build"),
                effect_class: EffectClass::Build,
                adapter_kind: label("local"),
                operation_identity: identity('1'),
            },
            WorkflowNode {
                id: label("test"),
                kind: label("test"),
                effect_class: EffectClass::Test,
                adapter_kind: label("remote"),
                operation_identity: identity('2'),
            },
        ],
        edges: vec![WorkflowEdge {
            source: label("build"),
            target: label("test"),
        }],
    };
    graph.identity = graph_identity(&graph);
    let mut request = WorkflowRequest {
        schema: REQUEST_SCHEMA.to_string(),
        identity: identity('0'),
        idempotency_key: identity('3'),
        profile_identity: profile_identity(profile),
        platform: label("linux-x86_64"),
        kiln: KilnLinkage {
            effect_identity: identity('4'),
            attempt_identity: identity('5'),
            candidate_identity: identity('6'),
            pipeline_revision_identity: identity('7'),
            workflow_policy_identity: identity('8'),
        },
        source: None,
        graph,
        controls: RequestedControls {
            cancel: true,
            pause: true,
            resume: true,
        },
        observation: ObservationPolicy {
            maximum_log_references: TEST_MAXIMUM_LOG_REFERENCES,
            maximum_log_read_bytes: TEST_MAXIMUM_LOG_READ_BYTES,
            maximum_total_log_bytes: TEST_MAXIMUM_TOTAL_LOG_BYTES,
        },
    };
    request.identity = request_identity(&request);
    request
}

fn accepted(request: &WorkflowRequest) -> WorkflowAccepted {
    WorkflowAccepted {
        schema: ACCEPTED_SCHEMA.to_string(),
        request_identity: request.identity.clone(),
        idempotency_key: request.idempotency_key.clone(),
        workflow_identity: identity('9'),
        execution_identity: identity('a'),
    }
}

fn completed_observation(request: &WorkflowRequest, accepted: &WorkflowAccepted) -> ExecutionObservation {
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('0'),
        request_identity: accepted.request_identity.clone(),
        idempotency_key: accepted.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence: TEST_SEQUENCE,
        state: ExecutionState::Completed,
        terminal: true,
        successful: true,
        nodes: vec![
            NodeObservation {
                node_id: request.graph.nodes[0].id.clone(),
                state: NodeState::Completed,
                terminal: true,
                successful: true,
                adapter_identity: identity('b'),
                receipt_identity: Some(identity('c')),
            },
            NodeObservation {
                node_id: request.graph.nodes[1].id.clone(),
                state: NodeState::Completed,
                terminal: true,
                successful: true,
                adapter_identity: identity('d'),
                receipt_identity: Some(identity('e')),
            },
        ],
        logs: vec![LogReference {
            node_id: request.graph.nodes[0].id.clone(),
            stream: LogStream::Stdout,
            reference_identity: identity('f'),
            cursor: 0,
            bytes: TEST_LOG_BYTES,
            truncated: false,
            gap_before: false,
            retained: true,
        }],
        receipt_identity: Some(identity('1')),
    };
    observation.identity = observation_identity(&observation);
    observation
}

// r[verify lattice.workflow_interchange.request]
// r[verify lattice.workflow_interchange.request.scenario.equal]
// r[verify lattice.workflow_interchange.graph.scenario.accepted]
#[test]
fn profile_and_request_are_admitted_deterministically() {
    let profile = profile();
    let request = request(&profile);

    assert_eq!(validate_profile(&profile), Ok(()));
    assert_eq!(validate_request(&profile, &request), Ok(()));
    assert_eq!(request.identity, request_identity(&request));
    assert_eq!(request.graph.identity, graph_identity(&request.graph));
}

// r[verify lattice.workflow_interchange.source_context]
// r[verify lattice.workflow_interchange.source_context.scenario.identity]
#[test]
fn source_context_changes_request_identity_and_stale_identity_fails() {
    let profile = profile();
    let mut first = request(&profile);
    first.source = Some(SourceContext {
        provider: label("radicle"),
        repository: source_value("rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE"),
        revision: source_value("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    });
    first.identity = request_identity(&first);
    let mut second = first.clone();
    second.source.as_mut().expect("source context exists").revision =
        source_value("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    assert_ne!(first.identity, request_identity(&second));
    assert_eq!(validate_request(&profile, &first), Ok(()));
    assert_eq!(
        validate_request(&profile, &second).map_err(|error| error.code),
        Err(ErrorCode::RequestIdentityMismatch)
    );
}

// r[verify lattice.workflow_interchange.source_context.scenario.opaque]
// r[verify lattice.workflow_interchange.source_context.scenario.control]
#[test]
fn source_values_preserve_mixed_case_and_reject_controls() {
    let value = source_value("rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE");
    assert_eq!(value.as_str(), "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE");
    assert_eq!(SourceValue::new("rad:zrepo\n".to_string()), Err(ValueError::InvalidSourceValue));
}

#[test]
fn oversized_capability_profile_is_rejected() {
    let mut profile = profile();
    profile.platforms = vec![label("linux-x86_64"); TEST_EXCESS_CAPABILITY_ITEMS];

    let error = validate_profile(&profile).expect_err("unbounded profile must fail");
    assert_eq!(error.code, ErrorCode::LimitInvalid);
}

// r[verify lattice.workflow_interchange.graph.scenario.adapter]
#[test]
fn unsupported_adapter_is_rejected_before_dispatch() {
    let profile = profile();
    let mut request = request(&profile);
    request.graph.nodes[0].adapter_kind = label("secret-shell");
    request.graph.identity = graph_identity(&request.graph);
    request.identity = request_identity(&request);

    let error = validate_request(&profile, &request).expect_err("undeclared adapter must fail");
    assert_eq!(error.code, ErrorCode::UnsupportedAdapterKind);
}

// r[verify lattice.workflow_interchange.graph]
// r[verify lattice.workflow_interchange.graph.scenario.cycle]
#[test]
fn cyclic_graph_is_rejected() {
    let profile = profile();
    let mut request = request(&profile);
    request.graph.edges.push(WorkflowEdge {
        source: label("test"),
        target: label("build"),
    });
    request.graph.identity = graph_identity(&request.graph);
    request.identity = request_identity(&request);

    let error = validate_request(&profile, &request).expect_err("cyclic graph must fail");
    assert_eq!(error.code, ErrorCode::GraphCycle);
}

// r[verify lattice.workflow_interchange.request.scenario.graph-change]
#[test]
fn changed_graph_identity_is_rejected() {
    let profile = profile();
    let mut request = request(&profile);
    request.graph.nodes[0].operation_identity = identity('f');

    let error = validate_request(&profile, &request).expect_err("stale graph identity must fail");
    assert_eq!(error.code, ErrorCode::GraphIdentityMismatch);
}

#[test]
fn unsupported_control_is_rejected() {
    let mut profile = profile();
    profile.controls.pause = false;
    let request = request(&profile);

    let error = validate_request(&profile, &request).expect_err("unsupported pause must fail");
    assert_eq!(error.code, ErrorCode::UnsupportedControl);
}

#[test]
fn accepted_execution_must_link_the_exact_request() {
    let profile = profile();
    let request = request(&profile);
    let mut accepted = accepted(&request);
    accepted.request_identity = identity('2');

    let error = validate_accepted(&request, &accepted).expect_err("stale acceptance must fail");
    assert_eq!(error.code, ErrorCode::AcceptedLinkageMismatch);
}

mod observations;

// Fixture tool: closed reviewed values abort generation when construction or serialization
// invariants break.
#![allow(
    unknown_lints,
    clippy::expect_used,
    non_trait_imports,
    no_unwrap,
    assertion_density,
    reason = "this test-only generator has no recovery path and validates its output through independent admission tests"
)]

use lattice_workflow_contract::*;
use serde::Serialize;

const FIXTURE_MAXIMUM_NODES: u32 = 8;
const FIXTURE_MAXIMUM_EDGES: u32 = 16;
const FIXTURE_MAXIMUM_LOG_REFERENCES: u32 = 8;
const FIXTURE_MAXIMUM_LOG_READ_BYTES: u64 = 4_096;
const FIXTURE_MAXIMUM_TOTAL_LOG_BYTES: u64 = 16_384;
const FIXTURE_LOG_BYTES: u64 = 32;
const FIXTURE_SEQUENCE: u64 = 7;

#[derive(Serialize)]
struct ProducerFixtures {
    schema: &'static str,
    profile: CapabilityProfile,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
    observation: ExecutionObservation,
    rejected: RejectedFixtures,
}

#[derive(Serialize)]
struct RejectedFixtures {
    cyclic_request: WorkflowRequest,
    false_paused_success: ExecutionObservation,
    stale_execution: ExecutionObservation,
}

fn main() {
    let profile = profile();
    let request = request(&profile);
    let accepted = accepted(&request);
    let observation = observation(&request, &accepted);
    let rejected = rejected(&request, &observation);
    let fixtures = ProducerFixtures {
        schema: "lattice.workflow-producer-fixtures.v1",
        profile,
        request,
        accepted,
        observation,
        rejected,
    };
    let output = serde_json::to_string_pretty(&fixtures).expect("producer fixtures serialize");
    println!("{output}");
}

fn identity(digit: char) -> Identity {
    let hex = digit.to_string().repeat(BLAKE3_HEX_LENGTH);
    Identity::new(format!("{BLAKE3_PREFIX}{hex}")).expect("fixture identity is canonical")
}

fn label(value: &str) -> Label {
    Label::new(value.to_string()).expect("fixture label is canonical")
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
            maximum_nodes: FIXTURE_MAXIMUM_NODES,
            maximum_edges: FIXTURE_MAXIMUM_EDGES,
            maximum_log_references: FIXTURE_MAXIMUM_LOG_REFERENCES,
            maximum_log_read_bytes: FIXTURE_MAXIMUM_LOG_READ_BYTES,
            maximum_total_log_bytes: FIXTURE_MAXIMUM_TOTAL_LOG_BYTES,
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
            maximum_log_references: FIXTURE_MAXIMUM_LOG_REFERENCES,
            maximum_log_read_bytes: FIXTURE_MAXIMUM_LOG_READ_BYTES,
            maximum_total_log_bytes: FIXTURE_MAXIMUM_TOTAL_LOG_BYTES,
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

fn rejected(request: &WorkflowRequest, observation: &ExecutionObservation) -> RejectedFixtures {
    let mut cyclic_request = request.clone();
    cyclic_request.graph.edges.push(WorkflowEdge {
        source: label("test"),
        target: label("build"),
    });
    cyclic_request.graph.identity = graph_identity(&cyclic_request.graph);
    cyclic_request.identity = request_identity(&cyclic_request);

    let mut false_paused_success = observation.clone();
    false_paused_success.state = ExecutionState::Paused;
    false_paused_success.identity = observation_identity(&false_paused_success);

    let mut stale_execution = observation.clone();
    stale_execution.execution_identity = identity('2');
    stale_execution.identity = observation_identity(&stale_execution);

    RejectedFixtures {
        cyclic_request,
        false_paused_success,
        stale_execution,
    }
}

fn observation(request: &WorkflowRequest, accepted: &WorkflowAccepted) -> ExecutionObservation {
    let mut node_ids = request.graph.nodes.iter().map(|node| node.id.clone());
    let build_node = node_ids.next().expect("fixture has a build node");
    let test_node = node_ids.next().expect("fixture has a test node");
    let mut observation = ExecutionObservation {
        schema: OBSERVATION_SCHEMA.to_string(),
        identity: identity('0'),
        request_identity: accepted.request_identity.clone(),
        idempotency_key: accepted.idempotency_key.clone(),
        workflow_identity: accepted.workflow_identity.clone(),
        execution_identity: accepted.execution_identity.clone(),
        sequence: FIXTURE_SEQUENCE,
        state: ExecutionState::Completed,
        terminal: true,
        successful: true,
        nodes: vec![
            NodeObservation {
                node_id: build_node.clone(),
                state: NodeState::Completed,
                terminal: true,
                successful: true,
                adapter_identity: identity('b'),
                receipt_identity: Some(identity('c')),
            },
            NodeObservation {
                node_id: test_node,
                state: NodeState::Completed,
                terminal: true,
                successful: true,
                adapter_identity: identity('d'),
                receipt_identity: Some(identity('e')),
            },
        ],
        logs: vec![LogReference {
            node_id: build_node,
            stream: LogStream::Stdout,
            reference_identity: identity('f'),
            cursor: 0,
            bytes: FIXTURE_LOG_BYTES,
            truncated: false,
            gap_before: false,
            retained: true,
        }],
        receipt_identity: Some(identity('1')),
    };
    observation.identity = observation_identity(&observation);
    observation
}

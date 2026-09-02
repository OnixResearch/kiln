use cairn_gate_contract::{
    build_result, request_identity, BoundedText, ChangeGateTarget, GateApplicability, GateFinding,
    GateRequest, GateStage, GateVerdict, GitRevision, Identity, Label, NativeGateObservation,
    ToolCohort, GATE_RECEIPT_SCHEMA, MACHINE_DIAGNOSTIC_SCHEMA, MACHINE_SCHEMA,
    NATIVE_OBSERVATION_SCHEMA, POLICY_SCHEMA, REQUEST_SCHEMA,
};
use serde_json::json;

const IDENTITY_HEX_LENGTH: usize = 64;
const SOURCE_REVISION: &str = "bfa88df5b2ac5f05e4a977bb8d4b8248d4511449";

fn identity(digit: char) -> Identity {
    Identity::new(format!(
        "b3:{}",
        digit.to_string().repeat(IDENTITY_HEX_LENGTH)
    ))
    .unwrap_or_else(|_| unreachable!())
}

fn label(value: &str) -> Label {
    Label::new(value.into()).unwrap_or_else(|_| unreachable!())
}

fn text(value: &str) -> BoundedText {
    BoundedText::new(value.into()).unwrap_or_else(|_| unreachable!())
}

fn request() -> GateRequest {
    let mut request = GateRequest {
        schema: REQUEST_SCHEMA.into(),
        identity: identity('0'),
        idempotency_key: identity('3'),
        applicability: GateApplicability {
            candidate_identity: identity('4'),
            candidate_root_identity: identity('5'),
            pipeline_revision_identity: identity('6'),
            policy_identity: identity('7'),
            evidence_identities: vec![identity('8'), identity('9')],
            provider_profile_identity: identity('a'),
            effect_identity: identity('b'),
        },
        target: ChangeGateTarget {
            change: label("demo-change"),
            stage: GateStage::Tasks,
        },
        tool: ToolCohort {
            source_revision: GitRevision::new(SOURCE_REVISION.into())
                .unwrap_or_else(|_| unreachable!()),
            executable_identity: identity('2'),
            machine_schema: MACHINE_SCHEMA.into(),
            diagnostic_schema: MACHINE_DIAGNOSTIC_SCHEMA.into(),
            policy_schema: POLICY_SCHEMA.into(),
            gate_receipt_schema: GATE_RECEIPT_SCHEMA.into(),
        },
    };
    request.identity = request_identity(&request);
    request
}

fn pass_observation() -> NativeGateObservation {
    NativeGateObservation {
        schema: NATIVE_OBSERVATION_SCHEMA.into(),
        change: label("demo-change"),
        stage: GateStage::Tasks,
        verdict: GateVerdict::Pass,
        valid: true,
        input_identity: identity('c'),
        policy_identity: identity('7'),
        receipt_identity: identity('d'),
        findings: Vec::new(),
    }
}

fn block_observation() -> NativeGateObservation {
    let mut observation = pass_observation();
    observation.verdict = GateVerdict::Block;
    observation.valid = false;
    observation.findings.push(GateFinding {
        code: label("cairn.gate.blocked"),
        message: text("required lifecycle evidence is missing"),
        scope: label("lifecycle"),
        promoted: label("gate"),
    });
    observation
}

fn main() {
    let request = request();
    let pass = build_result(&request, pass_observation()).unwrap_or_else(|_| unreachable!());
    let block = build_result(&request, block_observation()).unwrap_or_else(|_| unreachable!());
    let bundle = json!({
        "schema": "cairn.gate-producer-fixtures.v1",
        "request": request,
        "pass_result": pass,
        "block_result": block,
    });
    let rendered = serde_json::to_string_pretty(&bundle).unwrap_or_else(|_| unreachable!());
    println!("{rendered}");
}

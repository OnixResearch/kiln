const IDENTITY_HEX_LENGTH: usize = cairn_gate_contract::BLAKE3_HEX_LENGTH;
const SOURCE_REVISION: &str = "bfa88df5b2ac5f05e4a977bb8d4b8248d4511449";

fn identity(digit: char) -> cairn_gate_contract::Identity {
    cairn_gate_contract::Identity::new(format!(
        "b3:{}",
        digit.to_string().repeat(IDENTITY_HEX_LENGTH)
    ))
    .unwrap_or_else(|_| unreachable!())
}

fn label(value: &str) -> cairn_gate_contract::Label {
    cairn_gate_contract::Label::new(value.into()).unwrap_or_else(|_| unreachable!())
}

fn text(value: &str) -> cairn_gate_contract::BoundedText {
    cairn_gate_contract::BoundedText::new(value.into()).unwrap_or_else(|_| unreachable!())
}

fn request() -> cairn_gate_contract::GateRequest {
    let mut request = cairn_gate_contract::GateRequest {
        schema: cairn_gate_contract::REQUEST_SCHEMA.into(),
        identity: identity('0'),
        idempotency_key: identity('3'),
        applicability: cairn_gate_contract::GateApplicability {
            candidate_identity: identity('4'),
            candidate_root_identity: identity('5'),
            pipeline_revision_identity: identity('6'),
            policy_identity: identity('7'),
            evidence_identities: vec![identity('8'), identity('9')],
            provider_profile_identity: identity('a'),
            effect_identity: identity('b'),
        },
        target: cairn_gate_contract::ChangeGateTarget {
            change: label("demo-change"),
            stage: cairn_gate_contract::GateStage::Tasks,
        },
        tool: cairn_gate_contract::ToolCohort {
            source_revision: cairn_gate_contract::GitRevision::new(SOURCE_REVISION.into())
                .unwrap_or_else(|_| unreachable!()),
            executable_identity: identity('2'),
            machine_schema: cairn_gate_contract::MACHINE_SCHEMA.into(),
            diagnostic_schema: cairn_gate_contract::MACHINE_DIAGNOSTIC_SCHEMA.into(),
            policy_schema: cairn_gate_contract::POLICY_SCHEMA.into(),
            gate_receipt_schema: cairn_gate_contract::GATE_RECEIPT_SCHEMA.into(),
        },
    };
    request.identity = cairn_gate_contract::request_identity(&request);
    assert_eq!(cairn_gate_contract::validate_request(&request), Ok(()));
    assert_eq!(request.target.stage, cairn_gate_contract::GateStage::Tasks);
    request
}

fn pass_observation() -> cairn_gate_contract::NativeGateObservation {
    cairn_gate_contract::NativeGateObservation {
        schema: cairn_gate_contract::NATIVE_OBSERVATION_SCHEMA.into(),
        change: label("demo-change"),
        stage: cairn_gate_contract::GateStage::Tasks,
        verdict: cairn_gate_contract::GateVerdict::Pass,
        valid: true,
        input_identity: identity('c'),
        policy_identity: identity('7'),
        receipt_identity: identity('d'),
        findings: Vec::new(),
    }
}

fn block_observation() -> cairn_gate_contract::NativeGateObservation {
    let mut observation = pass_observation();
    observation.verdict = cairn_gate_contract::GateVerdict::Block;
    observation.valid = false;
    observation.findings.push(cairn_gate_contract::GateFinding {
        code: label("cairn.gate.blocked"),
        message: text("required lifecycle evidence is missing"),
        scope: label("lifecycle"),
        promoted: label("gate"),
    });
    observation
}

fn main() {
    let request = request();
    let pass = cairn_gate_contract::build_result(&request, pass_observation())
        .unwrap_or_else(|_| unreachable!());
    let block = cairn_gate_contract::build_result(&request, block_observation())
        .unwrap_or_else(|_| unreachable!());
    let bundle = serde_json::json!({
        "schema": "cairn.gate-producer-fixtures.v1",
        "request": request,
        "pass_result": pass,
        "block_result": block,
    });
    let rendered = serde_json::to_string_pretty(&bundle).unwrap_or_else(|_| unreachable!());
    println!("{rendered}");
}

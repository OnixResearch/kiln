use alloc::string::ToString;

mod scalars;

use crate::*;

const IDENTITY_HEX_LENGTH: usize = BLAKE3_HEX_LENGTH;
const REVISION_HEX_LENGTH: usize = GIT_REVISION_HEX_LENGTH;
const EXCESS_FINDING_COUNT: usize = MAXIMUM_FINDINGS.strict_add(1);

fn identity(digit: char) -> Identity {
    Identity::new(alloc::format!(
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

fn tool() -> ToolCohort {
    ToolCohort {
        source_revision: GitRevision::new("1".repeat(REVISION_HEX_LENGTH))
            .unwrap_or_else(|_| unreachable!()),
        executable_identity: identity('2'),
        machine_schema: MACHINE_SCHEMA.into(),
        diagnostic_schema: MACHINE_DIAGNOSTIC_SCHEMA.into(),
        policy_schema: POLICY_SCHEMA.into(),
        gate_receipt_schema: GATE_RECEIPT_SCHEMA.into(),
    }
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
            evidence_identities: alloc::vec![identity('8'), identity('9')],
            provider_profile_identity: identity('a'),
            effect_identity: identity('b'),
        },
        target: ChangeGateTarget {
            change: label("demo-change"),
            stage: GateStage::Tasks,
        },
        tool: tool(),
    };
    request.identity = request_identity(&request);
    assert_eq!(validate_request(&request), Ok(()));
    assert_eq!(request.target.stage, GateStage::Tasks);
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
        findings: alloc::vec::Vec::new(),
    }
}

fn finding() -> GateFinding {
    GateFinding {
        code: label("cairn.gate.blocked"),
        message: text("required lifecycle evidence is missing"),
        scope: label("lifecycle"),
        promoted: label("gate"),
    }
}

fn block_observation() -> NativeGateObservation {
    let mut observation = pass_observation();
    observation.verdict = GateVerdict::Block;
    observation.valid = false;
    observation.findings.push(finding());
    observation
}

#[test]
fn admits_exact_passing_and_blocked_results() {
    // r[verify cairn.gate_interchange.request.scenario.valid]
    // r[verify cairn.gate_interchange.result.scenario.pass]
    let request = request();
    assert_eq!(validate_request(&request), Ok(()));

    let pass = build_result(&request, pass_observation()).unwrap_or_else(|_| unreachable!());
    assert_eq!(validate_result(&request, &pass), Ok(()));
    assert_eq!(pass.observation.verdict, GateVerdict::Pass);

    let block = build_result(&request, block_observation()).unwrap_or_else(|_| unreachable!());
    assert_eq!(validate_result(&request, &block), Ok(()));
    assert_eq!(block.observation.verdict, GateVerdict::Block);
    assert_eq!(block.observation.findings.len(), 1);
}

#[test]
fn request_rejects_schema_identity_and_evidence_drift() {
    // r[verify cairn.gate_interchange.contract.scenario.schema]
    // r[verify cairn.gate_interchange.request.scenario.evidence]
    let mut wrong_schema = request();
    wrong_schema.schema = "cairn.gate-request.v2".into();
    wrong_schema.identity = request_identity(&wrong_schema);
    assert_eq!(
        validate_request(&wrong_schema),
        Err(ContractError::UnsupportedSchema)
    );

    let mut identity_drift = request();
    identity_drift.applicability.candidate_identity = identity('e');
    assert_eq!(
        validate_request(&identity_drift),
        Err(ContractError::RequestIdentityMismatch)
    );

    let mut reversed = request();
    reversed.applicability.evidence_identities.reverse();
    reversed.identity = request_identity(&reversed);
    assert_eq!(
        validate_request(&reversed),
        Err(ContractError::EvidenceNotCanonical)
    );

    let mut duplicate = request();
    duplicate.applicability.evidence_identities = alloc::vec![identity('8'), identity('8')];
    duplicate.identity = request_identity(&duplicate);
    assert_eq!(
        validate_request(&duplicate),
        Err(ContractError::EvidenceNotCanonical)
    );
}

#[test]
fn request_rejects_excess_evidence_and_tool_schema_drift() {
    let mut excess = request();
    excess.applicability.evidence_identities = (0..=MAXIMUM_EVIDENCE_IDENTITIES)
        .map(|index| {
            let digest = blake3::hash(&index.to_le_bytes());
            Identity::from_digest(digest)
        })
        .collect();
    excess.applicability.evidence_identities.sort();
    excess.identity = request_identity(&excess);
    assert_eq!(
        validate_request(&excess),
        Err(ContractError::EvidenceLimitExceeded)
    );

    let mut tool_drift = request();
    tool_drift.tool.machine_schema = "cairn.cli-machine.v2".into();
    tool_drift.identity = request_identity(&tool_drift);
    assert_eq!(
        validate_request(&tool_drift),
        Err(ContractError::InvalidToolCohort)
    );
}

#[test]
fn result_rejects_applicability_and_tool_substitution() {
    // r[verify cairn.gate_interchange.result.scenario.substitution]
    let request = request();
    let mut applicability =
        build_result(&request, pass_observation()).unwrap_or_else(|_| unreachable!());
    applicability.applicability.candidate_root_identity = identity('e');
    applicability.identity = result_identity(&applicability);
    assert_eq!(
        validate_result(&request, &applicability),
        Err(ContractError::ApplicabilityMismatch)
    );

    let mut cohort = build_result(&request, pass_observation()).unwrap_or_else(|_| unreachable!());
    cohort.tool.source_revision =
        GitRevision::new("2".repeat(REVISION_HEX_LENGTH)).unwrap_or_else(|_| unreachable!());
    cohort.identity = result_identity(&cohort);
    assert_eq!(
        validate_result(&request, &cohort),
        Err(ContractError::ToolCohortMismatch)
    );
}

#[test]
fn result_rejects_target_policy_and_identity_substitution() {
    let request = request();
    let mut target = pass_observation();
    target.change = label("other-change");
    assert_eq!(
        build_result(&request, target),
        Err(ContractError::TargetMismatch)
    );

    let mut policy = pass_observation();
    policy.policy_identity = identity('e');
    assert_eq!(
        build_result(&request, policy),
        Err(ContractError::PolicyMismatch)
    );

    let mut result = build_result(&request, pass_observation()).unwrap_or_else(|_| unreachable!());
    result.identity = identity('e');
    assert_eq!(
        validate_result(&request, &result),
        Err(ContractError::ResultIdentityMismatch)
    );
}

#[test]
fn result_rejects_contradictory_verdicts() {
    // r[verify cairn.gate_interchange.result.scenario.contradiction]
    let request = request();
    let mut pass_with_finding = pass_observation();
    pass_with_finding.findings.push(finding());
    assert_eq!(
        build_result(&request, pass_with_finding),
        Err(ContractError::ContradictoryVerdict)
    );

    let mut empty_block = block_observation();
    empty_block.findings.clear();
    assert_eq!(
        build_result(&request, empty_block),
        Err(ContractError::ContradictoryVerdict)
    );

    let mut invalid_pass = pass_observation();
    invalid_pass.valid = false;
    assert_eq!(
        build_result(&request, invalid_pass),
        Err(ContractError::ContradictoryVerdict)
    );
}

#[test]
fn result_rejects_finding_overflow_and_non_claim_drift() {
    let request = request();
    let mut overflow = block_observation();
    overflow.findings = alloc::vec![finding(); EXCESS_FINDING_COUNT];
    assert_eq!(
        build_result(&request, overflow),
        Err(ContractError::FindingLimitExceeded)
    );

    let mut result = build_result(&request, pass_observation()).unwrap_or_else(|_| unreachable!());
    result.non_claims.pop();
    result.identity = result_identity(&result);
    assert_eq!(
        validate_result(&request, &result),
        Err(ContractError::InvalidNonClaims)
    );
}

#[test]
fn validates_bounded_machine_diagnostic_schema() {
    let diagnostic = MachineDiagnostic {
        schema: MACHINE_DIAGNOSTIC_SCHEMA.into(),
        code: label("cairn.cli.blocked"),
        severity: DiagnosticSeverity::Error,
        message: text("gate command was blocked"),
        target: text("gate"),
        fix: Some(text("repair the lifecycle input")),
    };
    assert_eq!(validate_diagnostic(&diagnostic), Ok(()));

    let mut wrong_schema = diagnostic;
    wrong_schema.schema = "cairn.cli-machine-diagnostic.v2".into();
    assert_eq!(
        validate_diagnostic(&wrong_schema),
        Err(ContractError::InvalidDiagnostic)
    );
}

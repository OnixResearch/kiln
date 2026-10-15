use alloc::vec::Vec;

use crate::{
    request_identity, result_identity, BoundedText, ContractError, GateRequest, GateResult,
    GateVerdict, MachineDiagnostic, NativeGateObservation, GATE_RECEIPT_SCHEMA,
    MACHINE_DIAGNOSTIC_SCHEMA, MACHINE_SCHEMA, MAXIMUM_EVIDENCE_IDENTITIES, MAXIMUM_FINDINGS,
    NATIVE_OBSERVATION_SCHEMA, POLICY_SCHEMA, REQUEST_SCHEMA, REQUIRED_NON_CLAIMS, RESULT_SCHEMA,
};

pub fn validate_request(request: &GateRequest) -> Result<(), ContractError> {
    // r[impl cairn.gate_interchange.request]
    if request.schema != REQUEST_SCHEMA {
        return Err(ContractError::UnsupportedSchema);
    }
    validate_tool(request)?;
    if request.applicability.evidence_identities.len() > MAXIMUM_EVIDENCE_IDENTITIES {
        return Err(ContractError::EvidenceLimitExceeded);
    }
    if !evidence_is_strictly_sorted(&request.applicability.evidence_identities) {
        return Err(ContractError::EvidenceNotCanonical);
    }
    if request.identity != request_identity(request) {
        return Err(ContractError::RequestIdentityMismatch);
    }
    Ok(())
}

pub fn build_result(
    request: &GateRequest,
    observation: NativeGateObservation,
) -> Result<GateResult, ContractError> {
    // r[impl cairn.gate_interchange.result]
    // r[verify cairn.gate_interchange.reconciliation.scenario.unknown]
    validate_request(request)?;
    validate_observation(request, &observation)?;
    let non_claims = required_non_claims()?;
    let mut result = GateResult {
        schema: RESULT_SCHEMA.into(),
        identity: request.identity.clone(),
        request_identity: request.identity.clone(),
        applicability: request.applicability.clone(),
        tool: request.tool.clone(),
        observation,
        non_claims,
    };
    result.identity = result_identity(&result);
    validate_result(request, &result)?;
    Ok(result)
}

pub fn validate_result(request: &GateRequest, result: &GateResult) -> Result<(), ContractError> {
    // r[impl cairn.gate_interchange.result]
    // r[impl cairn.gate_interchange.reconciliation]
    validate_request(request)?;
    if result.schema != RESULT_SCHEMA {
        return Err(ContractError::UnsupportedSchema);
    }
    if result.request_identity != request.identity {
        return Err(ContractError::RequestIdentityMismatch);
    }
    if result.applicability != request.applicability {
        return Err(ContractError::ApplicabilityMismatch);
    }
    if result.tool != request.tool {
        return Err(ContractError::ToolCohortMismatch);
    }
    validate_observation(request, &result.observation)?;
    if result.non_claims != required_non_claims()? {
        return Err(ContractError::InvalidNonClaims);
    }
    if result.identity != result_identity(result) {
        return Err(ContractError::ResultIdentityMismatch);
    }
    Ok(())
}

pub fn validate_diagnostic(diagnostic: &MachineDiagnostic) -> Result<(), ContractError> {
    if diagnostic.schema != MACHINE_DIAGNOSTIC_SCHEMA {
        return Err(ContractError::InvalidDiagnostic);
    }
    Ok(())
}

fn validate_tool(request: &GateRequest) -> Result<(), ContractError> {
    let tool = &request.tool;
    if tool.machine_schema != MACHINE_SCHEMA
        || tool.diagnostic_schema != MACHINE_DIAGNOSTIC_SCHEMA
        || tool.policy_schema != POLICY_SCHEMA
        || tool.gate_receipt_schema != GATE_RECEIPT_SCHEMA
    {
        return Err(ContractError::InvalidToolCohort);
    }
    Ok(())
}

fn validate_observation(
    request: &GateRequest,
    observation: &NativeGateObservation,
) -> Result<(), ContractError> {
    if observation.schema != NATIVE_OBSERVATION_SCHEMA {
        return Err(ContractError::UnsupportedSchema);
    }
    if observation.findings.len() > MAXIMUM_FINDINGS {
        return Err(ContractError::FindingLimitExceeded);
    }
    if observation.change != request.target.change || observation.stage != request.target.stage {
        return Err(ContractError::TargetMismatch);
    }
    if observation.policy_identity != request.applicability.policy_identity {
        return Err(ContractError::PolicyMismatch);
    }
    match observation.verdict {
        GateVerdict::Pass if observation.valid && observation.findings.is_empty() => Ok(()),
        GateVerdict::Block if !observation.valid && !observation.findings.is_empty() => Ok(()),
        GateVerdict::Pass | GateVerdict::Block => Err(ContractError::ContradictoryVerdict),
    }
}

fn evidence_is_strictly_sorted(evidence: &[crate::Identity]) -> bool {
    let mut previous = None;
    for identity in evidence {
        if previous.is_some_and(|previous| previous >= identity) {
            return false;
        }
        previous = Some(identity);
    }
    true
}

fn required_non_claims() -> Result<Vec<BoundedText>, ContractError> {
    let mut non_claims = Vec::with_capacity(REQUIRED_NON_CLAIMS.len());
    for value in REQUIRED_NON_CLAIMS {
        let value = BoundedText::new(value.into()).map_err(|_| ContractError::InvalidNonClaims)?;
        non_claims.push(value);
    }
    Ok(non_claims)
}

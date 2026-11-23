pub fn validate_request(request: &crate::GateRequest) -> Result<(), crate::ContractError> {
    // r[impl cairn.gate_interchange.request]
    if request.schema != crate::REQUEST_SCHEMA {
        return Err(crate::ContractError::UnsupportedSchema);
    }
    validate_tool(request)?;
    if request.applicability.evidence_identities.len() > crate::MAXIMUM_EVIDENCE_IDENTITIES {
        return Err(crate::ContractError::EvidenceLimitExceeded);
    }
    if !evidence_is_strictly_sorted(&request.applicability.evidence_identities) {
        return Err(crate::ContractError::EvidenceNotCanonical);
    }
    if request.identity != crate::request_identity(request) {
        return Err(crate::ContractError::RequestIdentityMismatch);
    }
    Ok(())
}

pub fn build_result(
    request: &crate::GateRequest,
    observation: crate::NativeGateObservation,
) -> Result<crate::GateResult, crate::ContractError> {
    // r[impl cairn.gate_interchange.result]
    // r[verify cairn.gate_interchange.reconciliation.scenario.unknown]
    validate_request(request)?;
    validate_observation(request, &observation)?;
    let non_claims = required_non_claims()?;
    let mut result = crate::GateResult {
        schema: crate::RESULT_SCHEMA.into(),
        identity: request.identity.clone(),
        request_identity: request.identity.clone(),
        applicability: request.applicability.clone(),
        tool: request.tool.clone(),
        observation,
        non_claims,
    };
    result.identity = crate::result_identity(&result);
    validate_result(request, &result)?;
    Ok(result)
}

pub fn validate_result(
    request: &crate::GateRequest,
    result: &crate::GateResult,
) -> Result<(), crate::ContractError> {
    // r[impl cairn.gate_interchange.result]
    // r[impl cairn.gate_interchange.reconciliation]
    validate_request(request)?;
    if result.schema != crate::RESULT_SCHEMA {
        return Err(crate::ContractError::UnsupportedSchema);
    }
    if result.request_identity != request.identity {
        return Err(crate::ContractError::RequestIdentityMismatch);
    }
    if result.applicability != request.applicability {
        return Err(crate::ContractError::ApplicabilityMismatch);
    }
    if result.tool != request.tool {
        return Err(crate::ContractError::ToolCohortMismatch);
    }
    validate_observation(request, &result.observation)?;
    if result.non_claims != required_non_claims()? {
        return Err(crate::ContractError::InvalidNonClaims);
    }
    if result.identity != crate::result_identity(result) {
        return Err(crate::ContractError::ResultIdentityMismatch);
    }
    debug_assert_eq!(result.request_identity, request.identity);
    debug_assert_eq!(result.applicability, request.applicability);
    Ok(())
}

pub fn validate_diagnostic(
    diagnostic: &crate::MachineDiagnostic,
) -> Result<(), crate::ContractError> {
    if diagnostic.schema != crate::MACHINE_DIAGNOSTIC_SCHEMA {
        return Err(crate::ContractError::InvalidDiagnostic);
    }
    Ok(())
}

#[cfg_attr(
    dylint_lib = "octet",
    allow(
        missing_const_fn,
        reason = "Cairn gate-contract owner: String equality is not const on the pinned toolchain."
    )
)]
fn validate_tool(request: &crate::GateRequest) -> Result<(), crate::ContractError> {
    let tool = &request.tool;
    if tool.machine_schema != crate::MACHINE_SCHEMA {
        return Err(crate::ContractError::InvalidToolCohort);
    }
    if tool.diagnostic_schema != crate::MACHINE_DIAGNOSTIC_SCHEMA {
        return Err(crate::ContractError::InvalidToolCohort);
    }
    if tool.policy_schema != crate::POLICY_SCHEMA {
        return Err(crate::ContractError::InvalidToolCohort);
    }
    if tool.gate_receipt_schema != crate::GATE_RECEIPT_SCHEMA {
        return Err(crate::ContractError::InvalidToolCohort);
    }
    Ok(())
}

#[cfg_attr(
    dylint_lib = "octet",
    allow(
        missing_const_fn,
        reason = "Cairn gate-contract owner: runtime String and derived value comparisons are not const."
    )
)]
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        fragile_exhaustive_enum_match,
        reason = "Cairn gate-contract owner: a new closed-schema verdict requires an explicit admission decision."
    )
)]
fn validate_observation(
    request: &crate::GateRequest,
    observation: &crate::NativeGateObservation,
) -> Result<(), crate::ContractError> {
    if observation.schema != crate::NATIVE_OBSERVATION_SCHEMA {
        return Err(crate::ContractError::UnsupportedSchema);
    }
    if observation.findings.len() > crate::MAXIMUM_FINDINGS {
        return Err(crate::ContractError::FindingLimitExceeded);
    }
    if observation.change != request.target.change || observation.stage != request.target.stage {
        return Err(crate::ContractError::TargetMismatch);
    }
    if observation.policy_identity != request.applicability.policy_identity {
        return Err(crate::ContractError::PolicyMismatch);
    }
    debug_assert_eq!(observation.stage, request.target.stage);
    debug_assert_eq!(
        observation.policy_identity,
        request.applicability.policy_identity
    );
    match observation.verdict {
        crate::GateVerdict::Pass if observation.valid && observation.findings.is_empty() => Ok(()),
        crate::GateVerdict::Block if !observation.valid && !observation.findings.is_empty() => {
            Ok(())
        }
        crate::GateVerdict::Pass | crate::GateVerdict::Block => {
            Err(crate::ContractError::ContradictoryVerdict)
        }
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

fn required_non_claims() -> Result<alloc::vec::Vec<crate::BoundedText>, crate::ContractError> {
    let mut non_claims = alloc::vec::Vec::with_capacity(crate::REQUIRED_NON_CLAIMS.len());
    for value in crate::REQUIRED_NON_CLAIMS {
        let value = crate::BoundedText::new(value.into())
            .map_err(|_| crate::ContractError::InvalidNonClaims)?;
        non_claims.push(value);
    }
    Ok(non_claims)
}

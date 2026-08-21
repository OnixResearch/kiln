use std::string::ToString;

use cairn_gate_contract::{
    ChangeGateTarget, GateApplicability, GateRequest, GateResult as CairnGateResult,
    GateStage as CairnGateStage, GateVerdict, GitRevision, Identity, Label, REQUEST_SCHEMA,
    ToolCohort, request_identity, validate_request, validate_result,
};
use kiln_core::gating::{
    GateEffectPlan, GateEvaluationObservation, GateEvaluationOutcome, GateFinding,
    LifecycleGateStage, validate_gate_effect_plan,
};
use kiln_core::types::{BoundedText, ContentId, Diagnostic};
use kiln_provider_api::{PortFailure, PortResult};

use crate::profile::{CairnProfile, PINNED_CAIRN_REVISION};

const CANDIDATE_IDENTITY_DOMAIN: &str = "kiln.cairn-gate.candidate-identity.v1";
const PIPELINE_IDENTITY_DOMAIN: &str = "kiln.cairn-gate.pipeline-identity.v1";
const PROVIDER_IDENTITY_DOMAIN: &str = "kiln.cairn-gate.provider-identity.v1";
const EFFECT_IDENTITY_DOMAIN: &str = "kiln.cairn-gate.effect-identity.v1";

// r[impl kiln.cairn_gate.request]
pub(crate) fn lower_request(
    profile: &CairnProfile,
    plan: &GateEffectPlan,
) -> PortResult<GateRequest> {
    validate_gate_effect_plan(
        plan,
        profile.limits.max_changes,
        profile.limits.max_edges,
        profile.limits.max_evidence_identities,
    )?;
    if plan.provider_profile != profile.provider.profile_id
        || plan.policy_identity != profile.runtime.policy_identity
        || !profile.supported_stages.contains(&plan.target.stage)
    {
        return Err(domain(
            "cairn_request_profile",
            "cairn.request",
            "Cairn gate plan differs from the admitted provider, policy, or stage",
        ));
    }
    if plan.target.change.provider_namespace.as_str() != "cairn"
        || plan.target.change.native_kind.as_str() != "change"
    {
        return Err(domain(
            "cairn_request_target",
            "cairn.request.target",
            "Cairn gate target is not a Cairn change reference",
        ));
    }
    let mut request = GateRequest {
        schema: REQUEST_SCHEMA.to_string(),
        identity: identity_from_content(&plan.idempotency_key)?,
        idempotency_key: identity_from_content(&plan.idempotency_key)?,
        applicability: GateApplicability {
            candidate_identity: text_identity(
                CANDIDATE_IDENTITY_DOMAIN,
                plan.candidate.id.as_str(),
            )?,
            candidate_root_identity: identity_from_content(&plan.candidate_root)?,
            pipeline_revision_identity: text_identity(
                PIPELINE_IDENTITY_DOMAIN,
                plan.pipeline_revision.as_str(),
            )?,
            policy_identity: identity_from_content(&plan.policy_identity)?,
            evidence_identities: plan
                .evidence_identities
                .iter()
                .map(identity_from_content)
                .collect::<PortResult<Vec<_>>>()?,
            provider_profile_identity: text_identity(
                PROVIDER_IDENTITY_DOMAIN,
                plan.provider_profile.as_str(),
            )?,
            effect_identity: text_identity(EFFECT_IDENTITY_DOMAIN, plan.effect_id.as_str())?,
        },
        target: ChangeGateTarget {
            change: Label::new(plan.target.change.native_id.as_str().to_string()).map_err(
                |_| {
                    domain(
                        "cairn_request_change",
                        "cairn.request.target.change",
                        "Cairn change name does not fit the pinned contract",
                    )
                },
            )?,
            stage: lower_stage(plan.target.stage),
        },
        tool: ToolCohort {
            source_revision: GitRevision::new(PINNED_CAIRN_REVISION.to_string()).map_err(|_| {
                domain(
                    "cairn_request_revision",
                    "cairn.request.tool",
                    "Pinned Cairn revision does not fit the contract",
                )
            })?,
            executable_identity: identity_from_content(&profile.runtime.executable_identity)?,
            machine_schema: profile.machine_schema.clone(),
            diagnostic_schema: profile.diagnostic_schema.clone(),
            policy_schema: profile.policy_schema.clone(),
            gate_receipt_schema: profile.gate_receipt_schema.clone(),
        },
    };
    request.identity = request_identity(&request);
    validate_request(&request).map_err(contract_error)?;
    Ok(request)
}

// r[impl kiln.cairn_gate.result]
pub(crate) fn admitted_result(
    profile: &CairnProfile,
    plan: &GateEffectPlan,
    request: &GateRequest,
    result: &CairnGateResult,
) -> PortResult<GateEvaluationObservation> {
    validate_result(request, result).map_err(contract_error)?;
    if result.observation.findings.len() > profile.limits.max_findings {
        return Err(domain(
            "cairn_finding_bound",
            "cairn.result.findings",
            "Cairn result exceeds the selected finding bound",
        ));
    }
    let findings = result
        .observation
        .findings
        .iter()
        .map(|finding| {
            Ok(GateFinding {
                code: bounded(
                    finding.code.as_str(),
                    profile.limits.max_diagnostic_bytes,
                    "cairn.finding.code",
                )?,
                message: bounded(
                    finding.message.as_str(),
                    profile.limits.max_diagnostic_bytes,
                    "cairn.finding.message",
                )?,
                scope: bounded(
                    finding.scope.as_str(),
                    profile.limits.max_diagnostic_bytes,
                    "cairn.finding.scope",
                )?,
                promoted: bounded(
                    finding.promoted.as_str(),
                    profile.limits.max_diagnostic_bytes,
                    "cairn.finding.promoted",
                )?,
            })
        })
        .collect::<PortResult<Vec<_>>>()?;
    let outcome = match result.observation.verdict {
        GateVerdict::Pass => GateEvaluationOutcome::Passed,
        GateVerdict::Block => GateEvaluationOutcome::Blocked,
    };
    Ok(observation(
        plan,
        outcome,
        Some(content_from_identity(&result.identity)?),
        Some(content_from_identity(&result.observation.receipt_identity)?),
        findings,
    ))
}

pub(crate) fn outcome_observation(
    profile: &CairnProfile,
    plan: &GateEffectPlan,
    outcome: GateEvaluationOutcome,
    diagnostics: impl IntoIterator<Item = (String, String, String, String)>,
) -> PortResult<GateEvaluationObservation> {
    let findings = diagnostics
        .into_iter()
        .map(|(code, message, scope, promoted)| {
            Ok(GateFinding {
                code: bounded(
                    &code,
                    profile.limits.max_diagnostic_bytes,
                    "cairn.diagnostic.code",
                )?,
                message: bounded(
                    &message,
                    profile.limits.max_diagnostic_bytes,
                    "cairn.diagnostic.message",
                )?,
                scope: bounded(
                    &scope,
                    profile.limits.max_diagnostic_bytes,
                    "cairn.diagnostic.scope",
                )?,
                promoted: bounded(
                    &promoted,
                    profile.limits.max_diagnostic_bytes,
                    "cairn.diagnostic.promoted",
                )?,
            })
        })
        .collect::<PortResult<Vec<_>>>()?;
    Ok(observation(plan, outcome, None, None, findings))
}

fn observation(
    plan: &GateEffectPlan,
    outcome: GateEvaluationOutcome,
    result_identity: Option<ContentId>,
    receipt_identity: Option<ContentId>,
    findings: Vec<GateFinding>,
) -> GateEvaluationObservation {
    GateEvaluationObservation {
        effect_id: plan.effect_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate.id.clone(),
        candidate_root: plan.candidate_root.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        policy_identity: plan.policy_identity.clone(),
        evidence_identities: plan.evidence_identities.clone(),
        provider_profile: plan.provider_profile.clone(),
        target: plan.target.clone(),
        outcome,
        result_identity,
        receipt_identity,
        findings,
    }
}

fn lower_stage(stage: LifecycleGateStage) -> CairnGateStage {
    match stage {
        LifecycleGateStage::Proposal => CairnGateStage::Proposal,
        LifecycleGateStage::Design => CairnGateStage::Design,
        LifecycleGateStage::Tasks => CairnGateStage::Tasks,
    }
}

pub(crate) fn identity_from_content(content: &ContentId) -> PortResult<Identity> {
    Identity::new(format!("b3:{}", content.as_str())).map_err(|_| {
        domain(
            "cairn_identity",
            "cairn.identity",
            "Kiln content identity does not fit the Cairn contract",
        )
    })
}

fn content_from_identity(identity: &Identity) -> PortResult<ContentId> {
    let value = identity.as_str().strip_prefix("b3:").ok_or_else(|| {
        domain(
            "cairn_identity",
            "cairn.identity",
            "Cairn identity is missing the BLAKE3 prefix",
        )
    })?;
    ContentId::from_blake3_hex(value).map_err(PortFailure::Domain)
}

fn text_identity(domain_tag: &str, value: &str) -> PortResult<Identity> {
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, domain_tag.as_bytes())?;
    frame(&mut hasher, value.as_bytes())?;
    Identity::new(format!("b3:{}", hasher.finalize().to_hex())).map_err(|_| {
        domain(
            "cairn_identity",
            "cairn.identity",
            "Kiln label identity does not fit the Cairn contract",
        )
    })
}

fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) -> PortResult<()> {
    let length = u64::try_from(bytes.len()).map_err(|_| {
        domain(
            "cairn_identity_length",
            "cairn.identity",
            "Cairn identity input length is not representable",
        )
    })?;
    hasher.update(&length.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn bounded(value: &str, max_bytes: usize, field: &'static str) -> PortResult<BoundedText> {
    BoundedText::new(value.to_string(), max_bytes, field).map_err(PortFailure::Domain)
}

fn contract_error(_error: cairn_gate_contract::ContractError) -> PortFailure {
    domain(
        "cairn_contract",
        "cairn.contract",
        "Cairn request or result failed contract admission",
    )
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

use cairn_gate_contract::{
    BoundedText as CairnText, GateFinding as CairnFinding, GateResult as CairnGateResult, Identity,
    Label, NATIVE_OBSERVATION_SCHEMA, NativeGateObservation, build_result,
};
use kiln_core::gating::{GateEffectPlan, GateEvaluationObservation, GateEvaluationOutcome};
use kiln_core::types::Diagnostic;
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mapping::{admitted_result, outcome_observation};
use crate::profile::{CairnProfile, PINNED_MACHINE_SUCCESS_STATUS};
use crate::runtime::CairnProcessOutput;

pub(crate) fn interpret_output(
    profile: &CairnProfile,
    plan: &GateEffectPlan,
    request: &cairn_gate_contract::GateRequest,
    output: CairnProcessOutput,
) -> PortResult<GateEvaluationObservation> {
    match output {
        CairnProcessOutput::TimedOut => {
            outcome_observation(profile, plan, GateEvaluationOutcome::TimedOut, Vec::new())
        }
        CairnProcessOutput::Cancelled => {
            outcome_observation(profile, plan, GateEvaluationOutcome::Cancelled, Vec::new())
        }
        CairnProcessOutput::OutputLimitExceeded => outcome_observation(
            profile,
            plan,
            GateEvaluationOutcome::ProviderError,
            vec![diagnostic_tuple(
                "cairn.output.limit",
                "Cairn output exceeded the selected process bound",
            )],
        ),
        CairnProcessOutput::Unknown => {
            outcome_observation(profile, plan, GateEvaluationOutcome::Unknown, Vec::new())
        }
        CairnProcessOutput::Exited {
            process_status,
            stdout,
            stderr,
        } => interpret_exit(profile, plan, request, process_status, &stdout, &stderr),
    }
}

fn interpret_exit(
    profile: &CairnProfile,
    plan: &GateEffectPlan,
    request: &cairn_gate_contract::GateRequest,
    process_status: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> PortResult<GateEvaluationObservation> {
    if !stderr.is_empty() {
        return outcome_observation(
            profile,
            plan,
            GateEvaluationOutcome::ProviderError,
            vec![diagnostic_tuple(
                "cairn.machine.stderr",
                "Cairn machine mode emitted unexpected standard error",
            )],
        );
    }
    let envelope: MachineEnvelope = match serde_json::from_slice(stdout) {
        Ok(envelope) => envelope,
        Err(_error) => {
            return outcome_observation(
                profile,
                plan,
                GateEvaluationOutcome::ProviderError,
                vec![diagnostic_tuple(
                    "cairn.machine.encoding",
                    "Cairn machine output does not match the pinned JSON schema",
                )],
            );
        }
    };
    if envelope.schema_version != profile.machine_schema
        || envelope.command != profile.command
        || envelope.non_claims != profile.machine_non_claims
        || process_status != expected_status(profile, envelope.exit_class)
    {
        return outcome_observation(
            profile,
            plan,
            GateEvaluationOutcome::ProviderError,
            vec![diagnostic_tuple(
                "cairn.machine.cohort",
                "Cairn machine envelope differs from the pinned cohort",
            )],
        );
    }
    if !envelope.ok {
        if envelope.result.is_some() || envelope.diagnostics.is_empty() {
            return outcome_observation(
                profile,
                plan,
                GateEvaluationOutcome::ProviderError,
                vec![diagnostic_tuple(
                    "cairn.machine.failure",
                    "Cairn controlled failure has contradictory result facts",
                )],
            );
        }
        let outcome = match envelope.exit_class {
            MachineExitClass::PolicyError | MachineExitClass::Blocked => {
                GateEvaluationOutcome::PolicyError
            }
            MachineExitClass::Success
            | MachineExitClass::InvalidArguments
            | MachineExitClass::NotFound
            | MachineExitClass::InternalError => GateEvaluationOutcome::ProviderError,
        };
        return outcome_observation(
            profile,
            plan,
            outcome,
            envelope
                .diagnostics
                .into_iter()
                .map(MachineDiagnostic::into_finding_tuple),
        );
    }
    if envelope.exit_class != MachineExitClass::Success
        || process_status != Some(PINNED_MACHINE_SUCCESS_STATUS)
        || !envelope.diagnostics.is_empty()
    {
        return outcome_observation(
            profile,
            plan,
            GateEvaluationOutcome::ProviderError,
            vec![diagnostic_tuple(
                "cairn.machine.success",
                "Cairn success envelope has contradictory transport facts",
            )],
        );
    }
    let receipt = envelope.result.ok_or_else(|| {
        domain(
            "cairn.machine.result",
            "cairn.machine.result",
            "Cairn success envelope has no native gate receipt",
        )
    })?;
    let result = bind_native_receipt(profile, request, receipt)?;
    admitted_result(profile, plan, request, &result)
}

fn bind_native_receipt(
    profile: &CairnProfile,
    request: &cairn_gate_contract::GateRequest,
    receipt: NativeGateReceipt,
) -> PortResult<CairnGateResult> {
    verify_native_receipt_hash(profile, &receipt)?;
    if receipt.policy != profile.policy_name
        || receipt.policy_hash != profile.runtime.policy_identity.as_str()
        || receipt.non_claims != profile.native_gate_non_claims
    {
        return Err(domain(
            "cairn_native_policy",
            "cairn.result.policy",
            "Cairn native receipt differs from the admitted policy cohort",
        ));
    }
    if receipt.issues.len() > profile.limits.max_findings {
        return Err(domain(
            "cairn_native_findings",
            "cairn.result.issues",
            "Cairn native receipt exceeds the selected finding bound",
        ));
    }
    let verdict = match receipt.verdict.as_str() {
        "PASS" => cairn_gate_contract::GateVerdict::Pass,
        "FAIL" => cairn_gate_contract::GateVerdict::Block,
        _ => {
            return Err(domain(
                "cairn_native_verdict",
                "cairn.result.verdict",
                "Cairn native verdict is unsupported",
            ));
        }
    };
    let stage = parse_stage(&receipt.stage)?;
    let findings = receipt
        .issues
        .into_iter()
        .map(|issue| {
            Ok(CairnFinding {
                code: Label::new(issue.code).map_err(|_| native_value_error())?,
                message: CairnText::new(issue.message).map_err(|_| native_value_error())?,
                scope: Label::new(issue.scope).map_err(|_| native_value_error())?,
                promoted: Label::new(issue.promoted).map_err(|_| native_value_error())?,
            })
        })
        .collect::<PortResult<Vec<_>>>()?;
    let observation = NativeGateObservation {
        schema: NATIVE_OBSERVATION_SCHEMA.to_string(),
        change: Label::new(receipt.change).map_err(|_| native_value_error())?,
        stage,
        verdict,
        valid: receipt.valid,
        input_identity: native_identity(&receipt.input_hash)?,
        policy_identity: native_identity(&receipt.policy_hash)?,
        receipt_identity: native_identity(&receipt.receipt_hash)?,
        findings,
    };
    build_result(request, observation).map_err(|_| {
        domain(
            "cairn_native_contract",
            "cairn.result",
            "Cairn native receipt failed exact result binding",
        )
    })
}

fn verify_native_receipt_hash(
    profile: &CairnProfile,
    receipt: &NativeGateReceipt,
) -> PortResult<()> {
    let mut value = serde_json::to_value(receipt).map_err(|_| {
        infrastructure(
            "cairn_native_encoding",
            "Cairn native receipt could not be normalized for identity verification",
        )
    })?;
    let Value::Object(object) = &mut value else {
        return Err(native_value_error());
    };
    object.remove("receipt_hash");
    let root_text = profile.runtime.repository_root.display().to_string();
    let canonical_root = profile
        .runtime
        .repository_root
        .canonicalize()
        .ok()
        .map(|path| path.display().to_string());
    let portable = portable_json(&value, &root_text, canonical_root.as_deref());
    let canonical = canonical_json(&portable)?;
    let observed = blake3::hash(canonical.as_bytes()).to_hex().to_string();
    if observed != receipt.receipt_hash {
        return Err(domain(
            "cairn_native_receipt_identity",
            "cairn.result.receipt_hash",
            "Cairn native receipt identity does not match its canonical content",
        ));
    }
    Ok(())
}

fn portable_json(value: &Value, root: &str, canonical_root: Option<&str>) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| portable_json(item, root, canonical_root))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), portable_json(value, root, canonical_root)))
                .collect(),
        ),
        Value::String(text) => {
            let mut portable = text.replace(root, ".");
            if let Some(canonical_root) = canonical_root {
                portable = portable.replace(canonical_root, ".");
            }
            Value::String(portable)
        }
        other => other.clone(),
    }
}

fn canonical_json(value: &Value) -> PortResult<String> {
    let canonical = canonical_json_value(value);
    serde_json::to_string(&canonical).map_err(|_| {
        infrastructure(
            "cairn_native_canonical_json",
            "Cairn native receipt canonical JSON could not be encoded",
        )
    })
}

fn canonical_json_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                if let Some(child) = object.get(key) {
                    canonical.insert(key.clone(), canonical_json_value(child));
                }
            }
            Value::Object(canonical)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(canonical_json_value).collect::<Vec<_>>())
        }
        scalar => scalar.clone(),
    }
}

fn parse_stage(value: &str) -> PortResult<cairn_gate_contract::GateStage> {
    match value {
        "proposal" => Ok(cairn_gate_contract::GateStage::Proposal),
        "design" => Ok(cairn_gate_contract::GateStage::Design),
        "tasks" => Ok(cairn_gate_contract::GateStage::Tasks),
        _ => Err(native_value_error()),
    }
}

fn native_identity(value: &str) -> PortResult<Identity> {
    Identity::new(format!("b3:{value}")).map_err(|_| native_value_error())
}

fn native_value_error() -> PortFailure {
    domain(
        "cairn_native_value",
        "cairn.result",
        "Cairn native receipt contains a value outside the pinned contract",
    )
}

fn expected_status(profile: &CairnProfile, class: MachineExitClass) -> Option<i32> {
    Some(match class {
        MachineExitClass::Success => profile.exit_statuses.success,
        MachineExitClass::InvalidArguments => profile.exit_statuses.invalid_arguments,
        MachineExitClass::NotFound => profile.exit_statuses.not_found,
        MachineExitClass::PolicyError => profile.exit_statuses.policy_error,
        MachineExitClass::Blocked => profile.exit_statuses.blocked,
        MachineExitClass::InternalError => profile.exit_statuses.internal_error,
    })
}

fn diagnostic_tuple(code: &str, message: &str) -> (String, String, String, String) {
    (
        code.to_string(),
        message.to_string(),
        "machine".to_string(),
        "provider".to_string(),
    )
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum MachineExitClass {
    Success,
    InvalidArguments,
    NotFound,
    PolicyError,
    Blocked,
    InternalError,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MachineEnvelope {
    schema_version: String,
    ok: bool,
    command: String,
    exit_class: MachineExitClass,
    result: Option<NativeGateReceipt>,
    diagnostics: Vec<MachineDiagnostic>,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct MachineDiagnostic {
    code: String,
    severity: String,
    message: String,
    target: String,
    fix: Option<String>,
}

impl MachineDiagnostic {
    fn into_finding_tuple(self) -> (String, String, String, String) {
        let message = match self.fix {
            Some(fix) => format!("{} Fix: {fix}", self.message),
            None => self.message,
        };
        (self.code, message, self.target, self.severity)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeGateIssue {
    code: String,
    message: String,
    scope: String,
    promoted: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceSummary {
    mode: String,
    docs_only: usize,
    fixture: usize,
    probe: usize,
    property: usize,
    model: usize,
    formal: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeGateReceipt {
    acceptance_ids: Vec<String>,
    change: String,
    evidence_summary: EvidenceSummary,
    input_hash: String,
    issues: Vec<NativeGateIssue>,
    layout: String,
    non_claims: Vec<String>,
    policy: String,
    policy_hash: String,
    probe_evidence: Value,
    profile_identity_hash: String,
    receipt_hash: String,
    review_receipt_hashes: Vec<String>,
    spec_effect: String,
    stage: String,
    substance: Vec<Value>,
    task_ordering: Value,
    valid: bool,
    verdict: String,
    workflow_profile: String,
    workflow_profile_compatibility_fallback: bool,
    workflow_profile_source: String,
    workflow_profile_version: u64,
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

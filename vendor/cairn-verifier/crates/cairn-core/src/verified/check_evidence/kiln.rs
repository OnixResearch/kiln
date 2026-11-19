//! JSON translation through Kiln's original published admission function.
//! The CI owner retains workflow outcomes and skipped-node semantics. No
//! executed-test coverage or current Cairn applicability exists in this DTO.
use super::{ProducerIssue, MAX_CHECKS, MAX_TEXT_BYTES};
use crate::canonical_json_hash;
use kiln_core::workflow::{admit_workflow_observation, WorkflowEffectPlan, WorkflowObservation};
use serde::Serialize;
use serde_json::Value;

pub const KILN_REVISION: &str = "5d99f4e9bb954743443e340baece31256b4c53f1";
const MAX_NATIVE_DEPTH: usize = 16;
const MAX_NATIVE_VALUES: usize = 8192;
#[derive(Debug, Serialize)]
pub struct KilnAdmission {
    pub producer_revision: &'static str,
    pub native_status: kiln_core::workflow::WorkflowState,
    pub native_outcome: kiln_core::domain::Outcome,
    pub native_node_states: Vec<kiln_core::workflow::WorkflowNodeState>,
    pub native_plan_hash: String,
    pub native_observation_hash: String,
    pub completion_eligible: bool,
    pub non_claims: [&'static str; 3],
}
// r[impl cairn.check_evidence_producer.adapters]
pub fn admit_kiln_json(plan: &Value, observation: &Value) -> Result<KilnAdmission, ProducerIssue> {
    let mut remaining = MAX_NATIVE_VALUES;
    bounded(plan, MAX_NATIVE_DEPTH, &mut remaining)?;
    bounded(observation, MAX_NATIVE_DEPTH, &mut remaining)?;
    let p: WorkflowEffectPlan =
        serde_json::from_value(plan.clone()).map_err(|_| ProducerIssue::Observation)?;
    let o: WorkflowObservation =
        serde_json::from_value(observation.clone()).map_err(|_| ProducerIssue::Observation)?;
    nominal_values(&p, &o)?;
    let native_outcome =
        admit_workflow_observation(&p, &o).map_err(|_| ProducerIssue::Observation)?;
    Ok(KilnAdmission {
        producer_revision: KILN_REVISION,
        native_status: o.state,
        native_outcome,
        native_node_states: o.nodes.iter().map(|node| node.state).collect(),
        native_plan_hash: canonical_json_hash(plan),
        native_observation_hash: canonical_json_hash(observation),
        completion_eligible: false,
        non_claims: [
            "workflow observation admission does not prove source correctness or test completeness",
            "native skipped-node success is not executed-test coverage",
            "identity admission grants no execution, merge, publication or release authority",
        ],
    })
}
// Upstream DTO deserialization does not call its nominal constructors. Apply
// those original constructors at the JSON boundary before workflow admission.
fn nominal_values(p: &WorkflowEffectPlan, o: &WorkflowObservation) -> Result<(), ProducerIssue> {
    use kiln_core::types::{Alias, BoundedText, ContentId, ExternalRef, DEFAULT_MAX_ID_BYTES};
    let reject = |_| ProducerIssue::Observation;
    for value in [
        p.effect_id.as_str(),
        p.attempt_id.as_str(),
        p.candidate_id.as_str(),
        p.pipeline_revision_id.as_str(),
        p.workflow_policy_id.as_str(),
        p.provider_profile.as_str(),
    ] {
        BoundedText::new(value, DEFAULT_MAX_ID_BYTES, "identity").map_err(reject)?;
    }
    ContentId::from_blake3_hex(p.idempotency_key.as_str()).map_err(reject)?;
    Alias::new(p.platform.as_str()).map_err(reject)?;
    for node in &p.nodes {
        for value in [
            node.id.as_str(),
            node.kind.as_str(),
            node.adapter_kind.as_str(),
        ] {
            Alias::new(value).map_err(reject)?;
        }
        ContentId::from_blake3_hex(node.operation_identity.as_str()).map_err(reject)?;
    }
    let validate_ref = |value: &ExternalRef| -> Result<(), ProducerIssue> {
        value.validate().map_err(reject)?;
        ExternalRef::new(
            value.provider_namespace.as_str(),
            value.native_kind.as_str(),
            value.native_id.as_str(),
        )
        .map_err(reject)?;
        Ok(())
    };
    validate_ref(&o.workflow)?;
    validate_ref(&o.execution)?;
    if let Some(source) = &p.source {
        Alias::new(source.provider.as_str()).map_err(reject)?;
        validate_ref(&source.repository)?;
        validate_ref(&source.revision)?;
    }
    for node in &o.nodes {
        validate_ref(&node.adapter)?;
        if let Some(receipt) = &node.receipt {
            validate_ref(receipt)?;
        }
    }
    for log in &o.logs {
        validate_ref(&log.reference)?;
    }
    if let Some(receipt) = &o.receipt {
        validate_ref(receipt)?;
    }
    Ok(())
}
fn bounded(value: &Value, depth: usize, remaining: &mut usize) -> Result<(), ProducerIssue> {
    *remaining = remaining.checked_sub(1).ok_or(ProducerIssue::Bounds)?;
    if depth == 0 {
        return Err(ProducerIssue::Bounds);
    }
    match value {
        Value::String(s) if s.len() > MAX_TEXT_BYTES => Err(ProducerIssue::Bounds),
        Value::Array(values) => {
            if values.len() > MAX_CHECKS {
                return Err(ProducerIssue::Bounds);
            }
            for v in values {
                bounded(v, depth - 1, remaining)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > MAX_CHECKS || values.keys().any(|key| key.len() > MAX_TEXT_BYTES) {
                return Err(ProducerIssue::Bounds);
            }
            for v in values.values() {
                bounded(v, depth - 1, remaining)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

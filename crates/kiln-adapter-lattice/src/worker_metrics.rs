//! Non-authoritative metrics from the admitted native OMP observation cohort.
//! JSONL events are not model requests; measured model turns come only from
//! Lattice's `host_measured` budget-lineage draw and otherwise stay explicitly
//! absent (`NotInstrumented`).
use super::settlement::{ObservedDraw, extract_observed_draw};
use kiln_core::implementation::{
    AbsenceReason, AttemptUsage, AuxiliaryCallObservation, ModelTurnObservation, TokenObservation,
    WorkerObservation, WorkerTerminalState,
};
use kiln_core::types::ContentId;
use serde::Deserialize;
use serde_json::Value;

const NATIVE_SCHEMA: &str = "lattice.implementation-attempt-observation.v1";
const OMP_SCHEMA: &str = "lattice.omp-jsonl-observation.v1";

#[derive(Deserialize)]
struct OmpObservation {
    schema: String,
    terminal: WorkerTerminalState,
    raw_events_blake3: String,
}

// r[impl kiln.implementation.outcome_metrics.scenario.usage]
pub(super) fn normalize(
    request: &Value,
    response: &Value,
    response_bytes: &[u8],
    omp_bytes: &[u8],
) -> Result<WorkerObservation, String> {
    if response.get("schema").and_then(Value::as_str) != Some(NATIVE_SCHEMA)
        || request.get("envelope").is_none()
        || response.get("envelope") != request.get("envelope")
        || response.get("request_identity") != request.get("admitted_envelope")
    {
        return Err("worker metrics do not bind the admitted native request".into());
    }
    let observation: Result<OmpObservation, String> = serde_json::from_slice(omp_bytes)
        .map_err(|_| "native OMP metrics observation is malformed")?;
    let observation = observation.map_err(|_| "native OMP terminal observation is unavailable")?;
    if observation.schema != OMP_SCHEMA
        || observation.terminal == WorkerTerminalState::Deadline
        || ContentId::from_blake3_hex(observation.raw_events_blake3.clone()).is_err()
        || response
            .pointer("/result/stdout_blake3")
            .and_then(Value::as_str)
            != Some(observation.raw_events_blake3.as_str())
    {
        return Err("unsupported or crossed native OMP metrics observation".into());
    }
    let profile = request
        .pointer("/envelope/worker_profile")
        .and_then(Value::as_str)
        .ok_or("native worker metrics lack an admitted profile")?;
    let revision = request
        .pointer("/omp/revision")
        .and_then(Value::as_str)
        .ok_or("native worker metrics lack the OMP cohort")?;
    let model_turns = match extract_observed_draw(response_bytes) {
        ObservedDraw::HostMeasured { charged } => match charged.get("model_requests") {
            Some(&turns) => ModelTurnObservation::Observed { turns },
            None => ModelTurnObservation::Absent {
                reason: AbsenceReason::NotInstrumented,
            },
        },
        ObservedDraw::Unknown { .. } => ModelTurnObservation::Absent {
            reason: AbsenceReason::NotInstrumented,
        },
    };
    Ok(WorkerObservation {
        terminal: observation.terminal,
        worker_profile: Some(
            ContentId::from_blake3_hex(profile)
                .map_err(|_| "worker profile identity is malformed")?,
        ),
        harness: Some(format!("omp@{revision}")),
        // This is the selected profile's declared model label, not attestation
        // of the served weights or proof that a model request succeeded.
        model: request
            .pointer("/omp/model")
            .and_then(Value::as_str)
            .map(str::to_owned),
        usage: AttemptUsage {
            tokens: TokenObservation::Absent {
                reason: AbsenceReason::NotInstrumented,
            },
            auxiliary_calls: AuxiliaryCallObservation::Absent {
                reason: AbsenceReason::NotInstrumented,
            },
            model_turns,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(terminal: &str) -> (Value, Value, Vec<u8>) {
        let request = json!({"envelope":{"attempt_id":"attempt", "fence":7,
            "worker_profile":"a".repeat(64)}, "admitted_envelope":"b".repeat(64),
            "omp":{"revision":"pinned-omp", "model":"declared-model"}});
        let response = json!({"schema":NATIVE_SCHEMA,
            "envelope":request.get("envelope").unwrap_or_else(|| unreachable!()),
            "request_identity":request.get("admitted_envelope").unwrap_or_else(|| unreachable!()),
            "result":{"stdout_blake3":"c".repeat(64)}});
        let omp = serde_json::to_vec(&json!({"Ok":{"schema":OMP_SCHEMA,
            "terminal":terminal, "event_count":999, "usage_complete":false,
            "raw_events_blake3":"c".repeat(64)}}))
        .unwrap_or_else(|_| unreachable!());
        (request, response, omp)
    }

    #[test]
    // r[verify kiln.implementation.outcome_metrics.scenario.usage]
    fn native_terminals_are_retained_but_event_count_never_becomes_turns() {
        for (label, expected) in [
            ("completed", WorkerTerminalState::Completed),
            ("failed", WorkerTerminalState::Failed),
            ("aborted", WorkerTerminalState::Aborted),
        ] {
            let (request, response, bytes) = fixture(label);
            let response_bytes = serde_json::to_vec(&response).unwrap_or_else(|_| unreachable!());
            let observed = normalize(&request, &response, &response_bytes, &bytes)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(observed.terminal, expected);
            assert_eq!(observed.model.as_deref(), Some("declared-model"));
            assert_eq!(
                observed.usage.model_turns,
                ModelTurnObservation::Absent {
                    reason: AbsenceReason::NotInstrumented,
                }
            );
        }
    }

    #[test]
    // r[verify kiln.implementation.outcome_metrics.scenario.usage]
    fn host_measured_lineage_draw_supplies_model_turns_while_unknown_stays_absent() {
        let (request, mut response, bytes) = fixture("completed");
        if let Some(object) = response.as_object_mut() {
            object.insert(
                "budget_lineage".into(),
                json!({
                    "binding": "d".repeat(64),
                    "usage": [{
                        "phase": "edit",
                        "iteration": 1,
                        "model_requests": 3,
                        "tool_invocations": null,
                        "provenance": "host_measured",
                        "charged": {
                            "model_requests": 3,
                            "tool_invocations": 0
                        }
                    }]
                }),
            );
        }
        let response_bytes = serde_json::to_vec(&response).unwrap_or_else(|_| unreachable!());
        let observed = normalize(&request, &response, &response_bytes, &bytes)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            observed.usage.model_turns,
            ModelTurnObservation::Observed { turns: 3 }
        );

        // Unknown provenance never becomes turns (and never zero).
        let (request, mut unknown_response, bytes) = fixture("completed");
        if let Some(object) = unknown_response.as_object_mut() {
            object.insert(
                "budget_lineage".into(),
                json!({
                    "binding": "d".repeat(64),
                    "usage": [{
                        "phase": "edit",
                        "iteration": 1,
                        "model_requests": null,
                        "tool_invocations": null,
                        "provenance": "unknown",
                        "charged": {
                            "model_requests": 64,
                            "tool_invocations": 0
                        }
                    }]
                }),
            );
        }
        let unknown_bytes =
            serde_json::to_vec(&unknown_response).unwrap_or_else(|_| unreachable!());
        let observed = normalize(&request, &unknown_response, &unknown_bytes, &bytes)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            observed.usage.model_turns,
            ModelTurnObservation::Absent {
                reason: AbsenceReason::NotInstrumented,
            }
        );
    }

    #[test]
    // r[verify kiln.implementation.outcome_metrics.scenario.binding]
    fn crossed_and_unsupported_native_observations_cannot_supply_metrics() {
        let (request, response, bytes) = fixture("completed");
        let response_bytes = serde_json::to_vec(&response).unwrap_or_else(|_| unreachable!());
        let mut crossed = response.clone();
        *crossed
            .pointer_mut("/envelope/fence")
            .unwrap_or_else(|| unreachable!()) = json!(8);
        assert!(normalize(&request, &crossed, &response_bytes, &bytes).is_err());
        crossed = response.clone();
        *crossed
            .get_mut("request_identity")
            .unwrap_or_else(|| unreachable!()) = json!("different-request");
        assert!(normalize(&request, &crossed, &response_bytes, &bytes).is_err());
        crossed = response.clone();
        *crossed
            .pointer_mut("/result/stdout_blake3")
            .unwrap_or_else(|| unreachable!()) = json!("d".repeat(64));
        assert!(normalize(&request, &crossed, &response_bytes, &bytes).is_err());
        assert!(
            normalize(
                &request,
                &response,
                &response_bytes,
                br#"{"Err":"truncated"}"#
            )
            .is_err()
        );
        assert!(normalize(&request, &response, &response_bytes, b"not-json").is_err());
        let mut unsupported: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
        *unsupported
            .pointer_mut("/Ok/schema")
            .unwrap_or_else(|| unreachable!()) = json!("future-omp-schema");
        assert!(
            normalize(
                &request,
                &response,
                &response_bytes,
                &serde_json::to_vec(&unsupported).unwrap_or_else(|_| unreachable!())
            )
            .is_err()
        );
    }
}

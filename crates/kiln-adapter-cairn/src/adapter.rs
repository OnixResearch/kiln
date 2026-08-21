use std::collections::BTreeMap;
use std::ffi::OsString;
use std::sync::atomic::AtomicBool;

use bounded_exec::ExecutionLimits;
use kiln_core::gating::{
    GateEffectPlan, GateEvaluationObservation, GateEvaluationOutcome, LifecycleGateStage,
    admit_gate_evaluation,
};
use kiln_core::types::{Diagnostic, EffectId};
use kiln_provider_api::{GatePort, PortFailure, PortResult};

use crate::machine::interpret_output;
use crate::mapping::lower_request;
use crate::profile::{CairnProfile, CairnRerunPolicy, admit_profile};
use crate::runtime::{CairnRuntime, ProcessInvocation};

#[derive(Clone, Debug, Eq, PartialEq)]
struct EvaluationRecord {
    plan: GateEffectPlan,
    request: cairn_gate_contract::GateRequest,
    observation: GateEvaluationObservation,
}

#[derive(Debug)]
pub struct CairnAdapter<R> {
    profile: CairnProfile,
    runtime: R,
    evaluations: BTreeMap<EffectId, EvaluationRecord>,
}

impl<R: CairnRuntime> CairnAdapter<R> {
    pub fn new(profile: CairnProfile, runtime: R) -> PortResult<Self> {
        admit_profile(&profile)?;
        verify_executable(&profile, &runtime)?;
        Ok(Self {
            profile,
            runtime,
            evaluations: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn runtime(&self) -> &R {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    #[must_use]
    pub fn into_runtime(self) -> R {
        self.runtime
    }

    pub fn evaluate_with_cancellation(
        &mut self,
        plan: &GateEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<GateEvaluationObservation> {
        self.evaluate_inner(plan, Some(cancellation))
    }

    pub fn restore(
        &mut self,
        plan: &GateEffectPlan,
        observation: &GateEvaluationObservation,
    ) -> PortResult<()> {
        let request = lower_request(&self.profile, plan)?;
        admit_gate_evaluation(plan, observation)?;
        if let Some(record) = self.evaluations.get(&plan.effect_id) {
            if record.plan == *plan
                && record.request == request
                && record.observation == *observation
            {
                return Ok(());
            }
            return Err(domain(
                "cairn_restore_conflict",
                "cairn.evaluation",
                "Restored Cairn evaluation conflicts with the recorded request",
            ));
        }
        self.evaluations.insert(
            plan.effect_id.clone(),
            EvaluationRecord {
                plan: plan.clone(),
                request,
                observation: observation.clone(),
            },
        );
        Ok(())
    }

    fn evaluate_inner(
        &mut self,
        plan: &GateEffectPlan,
        cancellation: Option<&AtomicBool>,
    ) -> PortResult<GateEvaluationObservation> {
        // r[impl kiln.cairn_gate.reconciliation]
        let request = lower_request(&self.profile, plan)?;
        if let Some(record) = self.evaluations.get(&plan.effect_id) {
            if record.plan != *plan || record.request != request {
                return Err(domain(
                    "cairn_evaluation_conflict",
                    "cairn.evaluation",
                    "Repeated Cairn evaluation differs from the recorded request",
                ));
            }
            return Ok(record.observation.clone());
        }
        let unknown_count = self
            .evaluations
            .values()
            .filter(|record| record.observation.outcome == GateEvaluationOutcome::Unknown)
            .count();
        if unknown_count >= self.profile.limits.max_in_flight {
            return Err(PortFailure::Retryable(
                kiln_provider_api::InfrastructureFailure {
                    code: "cairn_in_flight_bound".to_string(),
                    message: "Cairn adapter reached its unknown evaluation bound".to_string(),
                },
            ));
        }
        verify_executable(&self.profile, &self.runtime)?;
        let encoded_request = serde_json::to_vec(&request).map_err(|_| {
            infrastructure(
                "cairn_request_encoding",
                "Cairn request could not be encoded for its byte-bound check",
            )
        })?;
        if encoded_request.len() > self.profile.limits.max_request_bytes {
            return Err(domain(
                "cairn_request_bound",
                "cairn.request",
                "Cairn request exceeds the selected profile bound",
            ));
        }
        let invocation = invocation(&self.profile, plan);
        let output = match cancellation {
            Some(cancellation) => self
                .runtime
                .run_with_cancellation(&invocation, cancellation)?,
            None => self.runtime.run(&invocation)?,
        };
        let observation = interpret_output(&self.profile, plan, &request, output)?;
        admit_gate_evaluation(plan, &observation)?;
        self.evaluations.insert(
            plan.effect_id.clone(),
            EvaluationRecord {
                plan: plan.clone(),
                request,
                observation: observation.clone(),
            },
        );
        Ok(observation)
    }
}

impl<R: CairnRuntime> GatePort for CairnAdapter<R> {
    fn profile(&self) -> &kiln_core::provider::CapabilityProfile {
        &self.profile.provider
    }

    fn evaluate(&mut self, plan: &GateEffectPlan) -> PortResult<GateEvaluationObservation> {
        self.evaluate_inner(plan, None)
    }

    fn reconcile(&mut self, plan: &GateEffectPlan) -> PortResult<GateEvaluationObservation> {
        let request = lower_request(&self.profile, plan)?;
        let record = self.evaluations.get(&plan.effect_id).ok_or_else(|| {
            domain(
                "cairn_evaluation_missing",
                "cairn.evaluation",
                "Cairn evaluation has no recorded outcome to reconcile",
            )
        })?;
        if record.plan != *plan || record.request != request {
            return Err(domain(
                "cairn_reconciliation_conflict",
                "cairn.evaluation",
                "Cairn reconciliation request differs from the recorded evaluation",
            ));
        }
        if record.observation.outcome == GateEvaluationOutcome::Unknown
            && self.profile.rerun_policy != CairnRerunPolicy::Never
        {
            return Err(domain(
                "cairn_rerun_policy",
                "cairn.evaluation",
                "Cairn rerun policy is not supported by this contract cohort",
            ));
        }
        Ok(record.observation.clone())
    }
}

fn verify_executable<R: CairnRuntime>(profile: &CairnProfile, runtime: &R) -> PortResult<()> {
    let observed = runtime.executable_identity(
        &profile.runtime.executable_path,
        profile.limits.max_executable_bytes,
    )?;
    if observed != profile.runtime.executable_identity {
        return Err(domain(
            "cairn_executable_identity",
            "cairn.executable",
            "Cairn executable identity differs from the admitted profile",
        ));
    }
    Ok(())
}

fn invocation(profile: &CairnProfile, plan: &GateEffectPlan) -> ProcessInvocation {
    ProcessInvocation {
        program: profile.runtime.executable_path.clone(),
        args: vec![
            OsString::from("--machine"),
            OsString::from(&profile.command),
            OsString::from(stage_name(plan.target.stage)),
            OsString::from(plan.target.change.native_id.as_str()),
            OsString::from("--root"),
            profile.runtime.repository_root.clone().into_os_string(),
            OsString::from("--policy"),
            profile.runtime.policy_path.clone().into_os_string(),
        ],
        current_dir: profile.runtime.repository_root.clone(),
        environment: Vec::new(),
        limits: ExecutionLimits {
            timeout_ms: profile.limits.timeout_milliseconds,
            stdin_max_bytes: profile.limits.stdin_max_bytes,
            stdout_max_bytes: profile.limits.max_result_bytes,
            stderr_max_bytes: profile.limits.max_stderr_bytes,
            poll_interval_ms: profile.limits.poll_interval_milliseconds,
            teardown_timeout_ms: profile.limits.teardown_timeout_milliseconds,
        },
    }
}

fn stage_name(stage: LifecycleGateStage) -> &'static str {
    match stage {
        LifecycleGateStage::Proposal => "proposal",
        LifecycleGateStage::Design => "design",
        LifecycleGateStage::Tasks => "tasks",
    }
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

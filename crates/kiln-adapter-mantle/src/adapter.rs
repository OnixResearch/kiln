use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use bounded_exec::ExecutionLimits;
use kiln_core::build::{
    BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome, BuildLogChunk,
    BuildLogReadRequest, admit_build_evaluation,
};
use kiln_core::types::{ContentId, Diagnostic, EffectId};
use kiln_provider_api::{BuildPort, PortFailure, PortResult};

use crate::mapping::{NormalizedBuild, lower_request, normalize_native_report, shell_outcome};
use crate::profile::{MantleProfile, admit_profile};
use crate::runtime::{MantleProcessOutput, MantleRuntime, ProcessInvocation};
use crate::wire::BuildRequest;

const SUCCESS_PROCESS_STATUS: i32 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildRecord {
    plan: BuildEffectPlan,
    request: BuildRequest,
    observation: BuildEvaluationObservation,
    logs: BTreeMap<ContentId, PathBuf>,
}

#[derive(Debug)]
pub struct MantleAdapter<R> {
    profile: MantleProfile,
    runtime: R,
    builds: BTreeMap<EffectId, BuildRecord>,
}

impl<R: MantleRuntime> MantleAdapter<R> {
    pub fn new(profile: MantleProfile, runtime: R) -> PortResult<Self> {
        admit_profile(&profile)?;
        verify_runtime(&profile, &runtime)?;
        Ok(Self {
            profile,
            runtime,
            builds: BTreeMap::new(),
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

    pub fn restore(
        &mut self,
        plan: &BuildEffectPlan,
        observation: &BuildEvaluationObservation,
    ) -> PortResult<()> {
        let request = lower_request(&self.profile, plan)?;
        admit_build_evaluation(plan, observation)?;
        if let Some(record) = self.builds.get(&plan.effect_id) {
            if record.plan == *plan
                && record.request == request
                && record.observation == *observation
            {
                return Ok(());
            }
            return Err(domain(
                "mantle_restore_conflict",
                "mantle.build",
                "Restored Mantle build conflicts with the recorded request",
            ));
        }
        self.builds.insert(
            plan.effect_id.clone(),
            BuildRecord {
                plan: plan.clone(),
                request,
                observation: observation.clone(),
                logs: BTreeMap::new(),
            },
        );
        Ok(())
    }

    fn dispatch_inner(
        &mut self,
        plan: &BuildEffectPlan,
        cancellation: Option<&AtomicBool>,
    ) -> PortResult<BuildEvaluationObservation> {
        // r[impl kiln.mantle.reconciliation]
        let request = lower_request(&self.profile, plan)?;
        if let Some(record) = self.builds.get(&plan.effect_id) {
            if record.plan != *plan || record.request != request {
                return Err(domain(
                    "mantle_build_conflict",
                    "mantle.build",
                    "Repeated Mantle build differs from the recorded request",
                ));
            }
            return Ok(record.observation.clone());
        }
        let unknown_count = self
            .builds
            .values()
            .filter(|record| record.observation.outcome == BuildEvaluationOutcome::Unknown)
            .count();
        if unknown_count >= self.profile.limits.max_in_flight {
            return Err(PortFailure::Retryable(
                kiln_provider_api::InfrastructureFailure {
                    code: "mantle_in_flight_bound".to_string(),
                    message: "Mantle adapter reached its unknown build bound".to_string(),
                },
            ));
        }
        verify_runtime(&self.profile, &self.runtime)?;
        let encoded_request = serde_json::to_vec(&request).map_err(|_| {
            infrastructure(
                "mantle_request_encoding",
                "Mantle request could not be encoded for its byte-bound check",
            )
        })?;
        if encoded_request.len() > self.profile.limits.max_request_bytes {
            return Err(domain(
                "mantle_request_bound",
                "mantle.request",
                "Mantle request exceeds the selected profile bound",
            ));
        }
        let invocation = invocation(&self.profile);
        let output = match cancellation {
            Some(cancellation) => self
                .runtime
                .run_with_cancellation(&invocation, cancellation)?,
            None => self.runtime.run(&invocation)?,
        };
        let normalized = interpret_output(&self.profile, plan, &request, output)?;
        admit_build_evaluation(plan, &normalized.observation)?;
        let observation = normalized.observation.clone();
        self.builds.insert(
            plan.effect_id.clone(),
            BuildRecord {
                plan: plan.clone(),
                request,
                observation: observation.clone(),
                logs: normalized.logs,
            },
        );
        Ok(observation)
    }
}

impl<R: MantleRuntime> BuildPort for MantleAdapter<R> {
    fn profile(&self) -> &kiln_core::provider::CapabilityProfile {
        &self.profile.provider
    }

    fn dispatch(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation> {
        self.dispatch_inner(plan, None)
    }

    fn dispatch_with_cancellation(
        &mut self,
        plan: &BuildEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<BuildEvaluationObservation> {
        self.dispatch_inner(plan, Some(cancellation))
    }

    fn observe(&mut self, effect_id: &EffectId) -> PortResult<BuildEvaluationObservation> {
        self.builds
            .get(effect_id)
            .map(|record| record.observation.clone())
            .ok_or_else(|| {
                domain(
                    "mantle_build_missing",
                    "mantle.build",
                    "Mantle build has no recorded observation",
                )
            })
    }

    fn read_log(&mut self, request: &BuildLogReadRequest) -> PortResult<BuildLogChunk> {
        if request.max_bytes == 0 || request.max_bytes > self.profile.limits.max_log_bytes {
            return Err(domain(
                "mantle_log_bound",
                "mantle.log.max_bytes",
                "Mantle log request exceeds the profile byte bound",
            ));
        }
        let record = self.builds.get(&request.effect_id).ok_or_else(|| {
            domain(
                "mantle_build_missing",
                "mantle.build",
                "Mantle log request names an unknown build",
            )
        })?;
        let path = record.logs.get(&request.log_identity).ok_or_else(|| {
            domain(
                "mantle_log_missing",
                "mantle.log.identity",
                "Mantle log identity is not bound to this build",
            )
        })?;
        let (bytes, end_of_log) = self.runtime.read_log(
            path,
            request.offset,
            request.max_bytes,
            self.profile.limits.max_log_bytes,
        )?;
        Ok(BuildLogChunk {
            effect_id: request.effect_id.clone(),
            log_identity: request.log_identity.clone(),
            offset: request.offset,
            bytes,
            end_of_log,
        })
    }

    fn reconcile(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation> {
        let request = lower_request(&self.profile, plan)?;
        let record = self.builds.get(&plan.effect_id).ok_or_else(|| {
            domain(
                "mantle_build_missing",
                "mantle.build",
                "Mantle build has no recorded outcome to reconcile",
            )
        })?;
        if record.plan != *plan || record.request != request {
            return Err(domain(
                "mantle_reconciliation_conflict",
                "mantle.build",
                "Mantle reconciliation differs from the recorded build request",
            ));
        }
        Ok(record.observation.clone())
    }
}

fn interpret_output(
    profile: &MantleProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    output: MantleProcessOutput,
) -> PortResult<NormalizedBuild> {
    match output {
        MantleProcessOutput::Exited {
            process_status,
            stdout,
            stderr: _,
        } => {
            let Some(process_status) = process_status else {
                return shell_outcome(profile, plan, request, BuildEvaluationOutcome::Unknown);
            };
            match normalize_native_report(profile, plan, request, &stdout) {
                Ok(normalized)
                    if process_status == SUCCESS_PROCESS_STATUS
                        || normalized.observation.outcome == BuildEvaluationOutcome::Failed =>
                {
                    Ok(normalized)
                }
                Ok(_) | Err(_) => shell_outcome(
                    profile,
                    plan,
                    request,
                    BuildEvaluationOutcome::ProviderError,
                ),
            }
        }
        MantleProcessOutput::TimedOut => {
            shell_outcome(profile, plan, request, BuildEvaluationOutcome::TimedOut)
        }
        MantleProcessOutput::Cancelled => {
            shell_outcome(profile, plan, request, BuildEvaluationOutcome::Cancelled)
        }
        MantleProcessOutput::OutputLimitExceeded => shell_outcome(
            profile,
            plan,
            request,
            BuildEvaluationOutcome::ProviderError,
        ),
        MantleProcessOutput::Unknown => {
            shell_outcome(profile, plan, request, BuildEvaluationOutcome::Unknown)
        }
    }
}

fn verify_runtime<R: MantleRuntime>(profile: &MantleProfile, runtime: &R) -> PortResult<()> {
    let executable = runtime.file_identity(
        &profile.runtime.executable_path,
        profile.limits.max_executable_bytes,
    )?;
    if executable != profile.runtime.executable_identity {
        return Err(domain(
            "mantle_executable_identity",
            "mantle.executable",
            "Mantle executable identity differs from the admitted profile",
        ));
    }
    let plan = runtime.file_identity(&profile.runtime.plan_path, profile.limits.max_plan_bytes)?;
    if plan != profile.runtime.plan_identity {
        return Err(domain(
            "mantle_plan_identity",
            "mantle.plan",
            "Mantle plan identity differs from the admitted profile",
        ));
    }
    let store_identity = runtime.file_identity(
        &profile.runtime.store_identity_path,
        profile.limits.max_store_identity_bytes,
    )?;
    if store_identity != profile.runtime.store_identity {
        return Err(domain(
            "mantle_store_identity",
            "mantle.store",
            "Mantle native store identity differs from the admitted profile",
        ));
    }
    Ok(())
}

fn invocation(profile: &MantleProfile) -> ProcessInvocation {
    let current_dir = profile
        .runtime
        .plan_path
        .parent()
        .map_or_else(|| PathBuf::from("/"), PathBuf::from);
    ProcessInvocation {
        program: profile.runtime.executable_path.clone(),
        args: vec![
            OsString::from("--store"),
            profile.runtime.store_path.clone().into_os_string(),
            OsString::from("--state-dir"),
            profile.runtime.state_path.clone().into_os_string(),
            OsString::from("--json"),
            OsString::from(&profile.command),
            OsString::from("--plan"),
            profile.runtime.plan_path.clone().into_os_string(),
        ],
        current_dir,
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

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

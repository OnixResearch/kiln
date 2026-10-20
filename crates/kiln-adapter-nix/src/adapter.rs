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
use kiln_provider_api::{BuildPort, InfrastructureFailure, PortFailure, PortResult};

use crate::mapping::{
    NormalizedBuild, gc_root_directory, log_identity, lower_request, normalize_realisation,
    shell_outcome,
};
use crate::profile::{NixBuildProfile, PINNED_NIX_CONFIG, admit_profile};
use crate::runtime::{NixBuildProcessOutput, NixBuildRuntime, ProcessInvocation};
use crate::wire::{BuildRequest, parse_realisation_roots};

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildRecord {
    plan: BuildEffectPlan,
    request: BuildRequest,
    observation: BuildEvaluationObservation,
    gc_root: PathBuf,
    log: Option<(ContentId, Vec<u8>)>,
}

#[derive(Debug)]
pub struct NixBuildAdapter<R> {
    profile: NixBuildProfile,
    runtime: R,
    builds: BTreeMap<EffectId, BuildRecord>,
}

impl<R: NixBuildRuntime> NixBuildAdapter<R> {
    pub fn new(profile: NixBuildProfile, runtime: R) -> PortResult<Self> {
        admit_profile(&profile)?;
        verify_runtime(&profile, &runtime)?;
        Ok(Self {
            profile,
            runtime,
            builds: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn profile_binding(&self) -> &NixBuildProfile {
        &self.profile
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
        let gc_root = gc_root_directory(&self.profile, &plan.effect_id)?;
        admit_build_evaluation(plan, observation)?;
        if let Some(record) = self.builds.get(&plan.effect_id) {
            if record.plan == *plan
                && record.request == request
                && record.observation == *observation
            {
                return Ok(());
            }
            return Err(domain(
                "nix_restore_conflict",
                "nix.build",
                "Restored Nix build conflicts with the recorded request",
            ));
        }
        self.builds.insert(
            plan.effect_id.clone(),
            BuildRecord {
                plan: plan.clone(),
                request,
                observation: observation.clone(),
                gc_root,
                log: None,
            },
        );
        Ok(())
    }

    /// Removes the run-scoped GC roots for an admitted build.
    ///
    /// Call only after the product set has been handed to durable downstream
    /// state. The operation is idempotent.
    // r[impl kiln.nix_build.retention]
    pub fn release_output_roots(&mut self, effect_id: &EffectId) -> PortResult<()> {
        let record = self.builds.get(effect_id).ok_or_else(|| {
            domain(
                "nix_build_missing",
                "nix.build",
                "Nix GC-root release names an unknown build",
            )
        })?;
        let gc_root = record.gc_root.clone();
        self.runtime
            .cleanup_gc_root_directory(&gc_root, self.profile.limits.max_gc_roots)
    }

    fn dispatch_inner(
        &mut self,
        plan: &BuildEffectPlan,
        cancellation: Option<&AtomicBool>,
    ) -> PortResult<BuildEvaluationObservation> {
        // r[impl kiln.nix_build.reconciliation]
        let request = lower_request(&self.profile, plan)?;
        if let Some(record) = self.builds.get(&plan.effect_id) {
            if record.plan != *plan || record.request != request {
                return Err(domain(
                    "nix_build_conflict",
                    "nix.build",
                    "Repeated Nix build differs from the recorded request",
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
            return Err(PortFailure::Retryable(InfrastructureFailure {
                code: "nix_in_flight_bound".to_string(),
                message: "Nix adapter reached its unknown build bound".to_string(),
            }));
        }
        verify_runtime(&self.profile, &self.runtime)?;
        let encoded_request = serde_json::to_vec(&request).map_err(|_| {
            infrastructure(
                "nix_request_encoding",
                "Nix request could not be encoded for its byte-bound check",
            )
        })?;
        if encoded_request.len() > self.profile.limits.max_request_bytes {
            return Err(domain(
                "nix_request_bound",
                "nix.request",
                "Nix request exceeds the selected profile bound",
            ));
        }
        let gc_root = gc_root_directory(&self.profile, &plan.effect_id)?;
        self.runtime
            .prepare_gc_root_directory(&self.profile.runtime.gc_root_base)?;
        self.runtime
            .cleanup_gc_root_directory(&gc_root, self.profile.limits.max_gc_roots)?;
        self.runtime.prepare_gc_root_directory(&gc_root)?;
        let invocation = invocation(&self.profile, &gc_root);
        let output = match cancellation {
            Some(cancellation) => self
                .runtime
                .run_with_cancellation(&invocation, cancellation)?,
            None => self.runtime.run(&invocation)?,
        };
        let normalized = interpret_output(
            &self.profile,
            plan,
            &request,
            output,
            &gc_root,
            &self.runtime,
        )?;
        admit_build_evaluation(plan, &normalized.observation)?;
        let observation = normalized.observation.clone();
        self.builds.insert(
            plan.effect_id.clone(),
            BuildRecord {
                plan: plan.clone(),
                request,
                observation: observation.clone(),
                gc_root,
                log: Some((normalized.log_identity, normalized.log)),
            },
        );
        Ok(observation)
    }
}

impl<R: NixBuildRuntime> BuildPort for NixBuildAdapter<R> {
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
                    "nix_build_missing",
                    "nix.build",
                    "Nix build has no recorded observation",
                )
            })
    }

    fn read_log(&mut self, request: &BuildLogReadRequest) -> PortResult<BuildLogChunk> {
        if request.max_bytes == 0 || request.max_bytes > self.profile.limits.max_log_bytes {
            return Err(domain(
                "nix_log_bound",
                "nix.log.max_bytes",
                "Nix log request exceeds the profile byte bound",
            ));
        }
        let record = self.builds.get(&request.effect_id).ok_or_else(|| {
            domain(
                "nix_build_missing",
                "nix.build",
                "Nix log request names an unknown build",
            )
        })?;
        let Some((identity, bytes)) = record.log.as_ref() else {
            return Err(domain(
                "nix_log_unavailable",
                "nix.log",
                "Nix log bytes are unavailable after an observation-only restore",
            ));
        };
        if *identity != request.log_identity {
            return Err(domain(
                "nix_log_identity",
                "nix.log.identity",
                "Nix log identity is not bound to this build",
            ));
        }
        let end = request
            .offset
            .saturating_add(request.max_bytes)
            .min(bytes.len());
        let chunk = bytes
            .get(request.offset..end)
            .map_or_else(Vec::new, <[u8]>::to_vec);
        Ok(BuildLogChunk {
            effect_id: request.effect_id.clone(),
            log_identity: request.log_identity.clone(),
            offset: request.offset,
            bytes: chunk,
            end_of_log: end >= bytes.len(),
        })
    }

    fn reconcile(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation> {
        let request = lower_request(&self.profile, plan)?;
        let record = self.builds.get(&plan.effect_id).ok_or_else(|| {
            domain(
                "nix_build_missing",
                "nix.build",
                "Nix build has no recorded outcome to reconcile",
            )
        })?;
        if record.plan != *plan || record.request != request {
            return Err(domain(
                "nix_reconciliation_conflict",
                "nix.build",
                "Nix reconciliation differs from the recorded build request",
            ));
        }
        Ok(record.observation.clone())
    }
}

// r[impl kiln.nix_build.execution]
fn interpret_output<R: NixBuildRuntime>(
    profile: &NixBuildProfile,
    plan: &BuildEffectPlan,
    request: &BuildRequest,
    output: NixBuildProcessOutput,
    gc_root: &std::path::Path,
    runtime: &R,
) -> PortResult<NormalizedBuild> {
    match output {
        NixBuildProcessOutput::Exited {
            process_status: Some(0),
            stdout,
            stderr,
        } => {
            let roots = match parse_realisation_roots(&stdout, profile.limits.max_gc_roots) {
                Ok(roots) => roots,
                Err(_) => {
                    let identity = log_identity(&stderr)?;
                    return shell_outcome(
                        profile,
                        plan,
                        request,
                        BuildEvaluationOutcome::ProviderError,
                        &stdout,
                        &stderr,
                        identity,
                    );
                }
            };
            let mut realisations = Vec::with_capacity(roots.len());
            for root in roots {
                match runtime.read_link(&root) {
                    Ok(target) => realisations.push((root, target)),
                    Err(_) => {
                        let identity = log_identity(&stderr)?;
                        return shell_outcome(
                            profile,
                            plan,
                            request,
                            BuildEvaluationOutcome::ProviderError,
                            &stdout,
                            &stderr,
                            identity,
                        );
                    }
                }
            }
            normalize_realisation(
                profile,
                plan,
                request,
                gc_root,
                &realisations,
                &stdout,
                &stderr,
            )
        }
        NixBuildProcessOutput::Exited {
            process_status: Some(_),
            stdout,
            stderr,
        } => {
            let identity = log_identity(&stderr)?;
            shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::Failed,
                &stdout,
                &stderr,
                identity,
            )
        }
        NixBuildProcessOutput::Exited {
            process_status: None,
            stdout,
            stderr,
        } => {
            let identity = log_identity(&stderr)?;
            shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::ProviderError,
                &stdout,
                &stderr,
                identity,
            )
        }
        NixBuildProcessOutput::TimedOut { stdout, stderr } => {
            let identity = log_identity(&stderr)?;
            shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::TimedOut,
                &stdout,
                &stderr,
                identity,
            )
        }
        NixBuildProcessOutput::Cancelled { stdout, stderr } => {
            let identity = log_identity(&stderr)?;
            shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::Cancelled,
                &stdout,
                &stderr,
                identity,
            )
        }
        NixBuildProcessOutput::OutputLimitExceeded { stdout, stderr } => {
            let identity = log_identity(&stderr)?;
            shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::ProviderError,
                &stdout,
                &stderr,
                identity,
            )
        }
        NixBuildProcessOutput::Unknown => {
            let identity = log_identity(&[])?;
            shell_outcome(
                profile,
                plan,
                request,
                BuildEvaluationOutcome::Unknown,
                &[],
                &[],
                identity,
            )
        }
    }
}

fn verify_runtime<R: NixBuildRuntime>(profile: &NixBuildProfile, runtime: &R) -> PortResult<()> {
    let executable = runtime.file_identity(
        &profile.runtime.executable_path,
        profile.limits.max_executable_bytes,
    )?;
    if executable != profile.runtime.executable_identity {
        return Err(domain(
            "nix_executable_identity",
            "nix.executable",
            "Nix executable identity differs from the admitted profile",
        ));
    }
    let nix_store = runtime.file_identity(
        &profile.runtime.nix_store_executable_path,
        profile.limits.max_executable_bytes,
    )?;
    if nix_store != profile.runtime.nix_store_executable_identity {
        return Err(domain(
            "nix_store_executable_identity",
            "nix.nix_store_executable",
            "Nix store executable identity differs from the admitted profile",
        ));
    }
    let derivation = runtime.file_identity(
        &profile.runtime.derivation_path,
        profile.limits.max_derivation_bytes,
    )?;
    if derivation != profile.runtime.derivation_identity {
        return Err(domain(
            "nix_derivation_identity",
            "nix.derivation",
            "Nix derivation identity differs from the admitted profile",
        ));
    }
    Ok(())
}

fn invocation(profile: &NixBuildProfile, gc_root: &std::path::Path) -> ProcessInvocation {
    ProcessInvocation {
        program: profile.runtime.nix_store_executable_path.clone(),
        args: vec![
            OsString::from("--add-root"),
            gc_root.join("root").into_os_string(),
            OsString::from("--indirect"),
            OsString::from("--realise"),
            profile.runtime.derivation_path.clone().into_os_string(),
        ],
        current_dir: profile.runtime.home_base.clone(),
        environment: vec![
            (
                OsString::from("HOME"),
                profile.runtime.home_base.clone().into_os_string(),
            ),
            (
                OsString::from("NIX_CONFIG"),
                OsString::from(PINNED_NIX_CONFIG),
            ),
        ],
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
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

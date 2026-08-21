use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::discovery::{
    DiscoveryCompletionFacts, DiscoveryEffectPlan, DiscoveryObservation, GcRootDecision,
    GcRootObservation, GcRootState, ProcessCompletionFact, TaskProposal,
    admit_discovery_observation, decide_gc_root_handoff,
};
use kiln_core::types::{Alias, Diagnostic, EffectId, ExternalRef, TaskId};
use kiln_provider_api::{DiscoveryPort, PortFailure, PortResult};

use crate::mapping::{
    admit_cohort_manifest, admit_plan, build_observation, gc_root_directory, invocation,
    replace_gc_root_observation,
};
use crate::profile::{NixEvalProfile, admit_profile};
use crate::runtime::{NixEvalProcessOutput, NixEvalRuntime};
use crate::wire::{IncrementalDecoder, StreamAdmission};

const STREAM_PARSE_CHUNK_BYTES: usize = 8_192;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiscoveryRecord {
    plan: DiscoveryEffectPlan,
    observation: DiscoveryObservation,
    gc_root_directory: PathBuf,
    handoff_complete: bool,
}

// r[impl kiln.nix_eval_jobs.conformance]
#[derive(Debug)]
pub struct NixEvalAdapter<R> {
    profile: NixEvalProfile,
    runtime: R,
    discoveries: BTreeMap<EffectId, DiscoveryRecord>,
}

impl<R: NixEvalRuntime> NixEvalAdapter<R> {
    pub fn new(profile: NixEvalProfile, runtime: R) -> PortResult<Self> {
        admit_profile(&profile)?;
        verify_runtime(&profile, &runtime)?;
        Ok(Self {
            profile,
            runtime,
            discoveries: BTreeMap::new(),
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
        plan: &DiscoveryEffectPlan,
        observation: &DiscoveryObservation,
    ) -> PortResult<()> {
        admit_plan(&self.profile, plan)?;
        admit_discovery_observation(plan, observation).map_err(PortFailure::Domain)?;
        if let Some(record) = self.discoveries.get(&plan.effect_id) {
            if record.plan == *plan && record.observation == *observation {
                return Ok(());
            }
            return Err(domain(
                "nix_eval_restore_conflict",
                "nix_eval.discovery",
                "restored discovery conflicts with the recorded request",
            ));
        }
        self.discoveries.insert(
            plan.effect_id.clone(),
            DiscoveryRecord {
                plan: plan.clone(),
                observation: observation.clone(),
                gc_root_directory: gc_root_directory(&self.profile, plan),
                handoff_complete: observation.gc_roots.state == GcRootState::Released,
            },
        );
        Ok(())
    }

    fn discover_inner(
        &mut self,
        plan: &DiscoveryEffectPlan,
        cancellation: Option<&AtomicBool>,
    ) -> PortResult<DiscoveryObservation> {
        // r[impl kiln.nix_eval_jobs.core_shell_boundary]
        // r[impl kiln.nix_eval_jobs.terminal_completeness]
        admit_plan(&self.profile, plan)?;
        if let Some(record) = self.discoveries.get(&plan.effect_id) {
            if record.plan != *plan {
                return Err(domain(
                    "nix_eval_discovery_conflict",
                    "nix_eval.discovery",
                    "repeated discovery differs from the recorded request",
                ));
            }
            return Ok(record.observation.clone());
        }
        let unknown_count = self
            .discoveries
            .values()
            .filter(|record| !record.observation.terminal.is_terminal())
            .count();
        if unknown_count >= self.profile.limits.max_in_flight {
            return Err(PortFailure::Retryable(
                kiln_provider_api::InfrastructureFailure {
                    code: "nix_eval_in_flight_bound".to_string(),
                    message: "Nix evaluation adapter reached its unknown discovery bound"
                        .to_string(),
                },
            ));
        }
        verify_runtime(&self.profile, &self.runtime)?;
        let root_directory = gc_root_directory(&self.profile, plan);
        self.runtime.prepare_gc_root_directory(&root_directory)?;
        let process_result = match cancellation {
            Some(cancellation) => self.runtime.run_with_cancellation(
                &invocation(&self.profile, plan, &root_directory),
                cancellation,
            ),
            None => self
                .runtime
                .run(&invocation(&self.profile, plan, &root_directory)),
        };
        let process = match process_result {
            Ok(process) => process,
            Err(process_error) => {
                self.runtime
                    .cleanup_gc_root_directory(&root_directory, self.profile.limits.max_gc_roots)?;
                return Err(process_error);
            }
        };
        let interpreted = interpret_process(&self.profile, process);
        let (gc_roots, gc_root_observation_ready) =
            observe_gc_roots(&self.profile, &self.runtime, &root_directory);
        let expected_roots: BTreeSet<_> = interpreted
            .stream
            .jobs
            .iter()
            .map(|job| job.derivation.clone())
            .collect();
        let observed_roots: BTreeSet<_> = gc_roots.derivations.iter().cloned().collect();
        let gc_roots_ready = gc_root_observation_ready && expected_roots == observed_roots;
        let fatal_job_error = interpreted.stream.errors.iter().any(|error| error.fatal);
        let completion = DiscoveryCompletionFacts {
            stream_complete: interpreted.stream_complete,
            stream_valid: interpreted.stream_valid,
            stderr_within_bound: interpreted.stderr_within_bound,
            gc_roots_ready,
            fatal_job_error,
            process: interpreted.process,
        };
        let observation = build_observation(plan, interpreted.stream, completion, gc_roots)?;
        admit_discovery_observation(plan, &observation).map_err(PortFailure::Domain)?;
        self.discoveries.insert(
            plan.effect_id.clone(),
            DiscoveryRecord {
                plan: plan.clone(),
                observation: observation.clone(),
                gc_root_directory: root_directory,
                handoff_complete: false,
            },
        );
        Ok(observation)
    }
}

impl<R: NixEvalRuntime> DiscoveryPort for NixEvalAdapter<R> {
    fn profile(&self) -> &kiln_core::provider::CapabilityProfile {
        &self.profile.provider
    }

    fn discover(&mut self, plan: &DiscoveryEffectPlan) -> PortResult<DiscoveryObservation> {
        self.discover_inner(plan, None)
    }

    fn discover_with_cancellation(
        &mut self,
        plan: &DiscoveryEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<DiscoveryObservation> {
        self.discover_inner(plan, Some(cancellation))
    }

    fn observe(&mut self, effect_id: &EffectId) -> PortResult<DiscoveryObservation> {
        self.discoveries
            .get(effect_id)
            .map(|record| record.observation.clone())
            .ok_or_else(|| {
                domain(
                    "nix_eval_discovery_missing",
                    "nix_eval.discovery",
                    "discovery has no recorded observation",
                )
            })
    }

    fn reconcile(&mut self, plan: &DiscoveryEffectPlan) -> PortResult<DiscoveryObservation> {
        admit_plan(&self.profile, plan)?;
        let record = self.discoveries.get(&plan.effect_id).ok_or_else(|| {
            domain(
                "nix_eval_discovery_missing",
                "nix_eval.discovery",
                "discovery has no recorded outcome to reconcile",
            )
        })?;
        if record.plan != *plan {
            return Err(domain(
                "nix_eval_reconciliation_conflict",
                "nix_eval.discovery",
                "reconciliation differs from the recorded discovery request",
            ));
        }
        Ok(record.observation.clone())
    }

    fn handoff_gc_roots(
        &mut self,
        plan: &DiscoveryEffectPlan,
        durable_tasks: &BTreeSet<TaskId>,
    ) -> PortResult<Vec<TaskProposal>> {
        admit_plan(&self.profile, plan)?;
        let record = self.discoveries.get_mut(&plan.effect_id).ok_or_else(|| {
            domain(
                "nix_eval_discovery_missing",
                "nix_eval.discovery",
                "GC-root handoff names an unknown discovery",
            )
        })?;
        if record.plan != *plan {
            return Err(domain(
                "nix_eval_handoff_conflict",
                "nix_eval.discovery",
                "GC-root handoff differs from the recorded discovery request",
            ));
        }
        match decide_gc_root_handoff(
            &record.observation,
            durable_tasks,
            self.profile.limits.max_tasks,
        )
        .map_err(PortFailure::Domain)?
        {
            GcRootDecision::Keep => Ok(Vec::new()),
            GcRootDecision::Release => {
                record.handoff_complete = true;
                Ok(record.observation.tasks.clone())
            }
            GcRootDecision::AlreadyReleased => Ok(record.observation.tasks.clone()),
        }
    }

    fn cleanup_gc_roots(&mut self, plan: &DiscoveryEffectPlan) -> PortResult<DiscoveryObservation> {
        admit_plan(&self.profile, plan)?;
        let record = self.discoveries.get_mut(&plan.effect_id).ok_or_else(|| {
            domain(
                "nix_eval_discovery_missing",
                "nix_eval.discovery",
                "GC-root cleanup names an unknown discovery",
            )
        })?;
        if record.plan != *plan {
            return Err(domain(
                "nix_eval_cleanup_conflict",
                "nix_eval.discovery",
                "GC-root cleanup differs from the recorded discovery request",
            ));
        }
        if record.observation.gc_roots.state == GcRootState::Released {
            return Ok(record.observation.clone());
        }
        if !record.handoff_complete {
            return Err(domain(
                "nix_eval_cleanup_before_handoff",
                "nix_eval.gc_roots",
                "GC roots cannot be removed before durable downstream handoff",
            ));
        }
        let state = if self
            .runtime
            .cleanup_gc_root_directory(&record.gc_root_directory, self.profile.limits.max_gc_roots)
            .is_ok()
        {
            GcRootState::Released
        } else {
            GcRootState::CleanupFailed
        };
        let updated_roots = GcRootObservation {
            directory: record.observation.gc_roots.directory.clone(),
            derivations: record.observation.gc_roots.derivations.clone(),
            state,
        };
        let updated = replace_gc_root_observation(plan, &record.observation, updated_roots)?;
        admit_discovery_observation(plan, &updated).map_err(PortFailure::Domain)?;
        record.observation = updated.clone();
        Ok(updated)
    }
}

struct InterpretedProcess {
    stream: StreamAdmission,
    stream_complete: bool,
    stream_valid: bool,
    stderr_within_bound: bool,
    process: ProcessCompletionFact,
}

fn interpret_process(profile: &NixEvalProfile, output: NixEvalProcessOutput) -> InterpretedProcess {
    match output {
        NixEvalProcessOutput::Exited {
            process_status,
            signal,
            stdout,
            stderr: _,
        } => {
            let process = match (process_status, signal) {
                (Some(status), _) => ProcessCompletionFact::Exited { status },
                (None, Some(_)) => ProcessCompletionFact::Signalled,
                (None, None) => ProcessCompletionFact::Unknown,
            };
            let (stream, valid) = decode_stream(profile, &stdout);
            InterpretedProcess {
                stream,
                stream_complete: true,
                stream_valid: valid,
                stderr_within_bound: true,
                process,
            }
        }
        NixEvalProcessOutput::TimedOut { stdout, stderr: _ } => {
            let (stream, valid) = decode_stream(profile, &stdout);
            InterpretedProcess {
                stream,
                stream_complete: false,
                stream_valid: valid,
                stderr_within_bound: true,
                process: ProcessCompletionFact::TimedOut,
            }
        }
        NixEvalProcessOutput::Cancelled { stdout, stderr: _ } => {
            let (stream, valid) = decode_stream(profile, &stdout);
            InterpretedProcess {
                stream,
                stream_complete: false,
                stream_valid: valid,
                stderr_within_bound: true,
                process: ProcessCompletionFact::Cancelled,
            }
        }
        NixEvalProcessOutput::OutputLimitExceeded { stdout, stderr: _ } => {
            let (stream, _) = decode_stream(profile, &stdout);
            InterpretedProcess {
                stream,
                stream_complete: false,
                stream_valid: false,
                stderr_within_bound: false,
                process: ProcessCompletionFact::OutputLimitExceeded,
            }
        }
        NixEvalProcessOutput::Unknown => InterpretedProcess {
            stream: empty_stream(),
            stream_complete: false,
            stream_valid: true,
            stderr_within_bound: true,
            process: ProcessCompletionFact::Unknown,
        },
    }
}

fn decode_stream(profile: &NixEvalProfile, bytes: &[u8]) -> (StreamAdmission, bool) {
    let mut decoder = IncrementalDecoder::new(profile.limits);
    for chunk in bytes.chunks(STREAM_PARSE_CHUNK_BYTES) {
        if let Err(error) = decoder.push(chunk) {
            return (invalid_stream(error), false);
        }
    }
    match decoder.finish() {
        Ok(stream) => (stream, true),
        Err(error) => (invalid_stream(error), false),
    }
}

fn invalid_stream(error: PortFailure) -> StreamAdmission {
    let diagnostic = match error {
        PortFailure::Domain(diagnostic) => diagnostic,
        _ => Diagnostic::new(
            "nix_eval_stream_failure",
            "nix_eval.stream",
            "stream admission failed outside its domain error boundary",
        ),
    };
    StreamAdmission {
        jobs: Vec::new(),
        errors: vec![kiln_core::discovery::DiscoveredJobError {
            attribute: Alias::new("stream").unwrap_or_else(|_| unreachable!()),
            diagnostic,
            fatal: true,
        }],
        record_count: 1,
        total_bytes: 0,
    }
}

fn empty_stream() -> StreamAdmission {
    StreamAdmission {
        jobs: Vec::new(),
        errors: Vec::new(),
        record_count: 0,
        total_bytes: 0,
    }
}

fn observe_gc_roots<R: NixEvalRuntime>(
    profile: &NixEvalProfile,
    runtime: &R,
    root_directory: &Path,
) -> (GcRootObservation, bool) {
    let directory = root_directory
        .to_str()
        .and_then(|path| ExternalRef::new("nix", "gc-root-directory", path).ok());
    match runtime.list_gc_roots(root_directory, profile.limits.max_gc_roots) {
        Ok(derivations) => (
            GcRootObservation {
                directory,
                derivations,
                state: GcRootState::Retained,
            },
            true,
        ),
        Err(_) => (
            GcRootObservation {
                directory,
                derivations: Vec::new(),
                state: GcRootState::Unknown,
            },
            false,
        ),
    }
}

// r[impl kiln.nix_eval_jobs.cohort_admission]
fn verify_runtime<R: NixEvalRuntime>(profile: &NixEvalProfile, runtime: &R) -> PortResult<()> {
    let executable = runtime.file_identity(
        &profile.runtime.executable_path,
        profile.limits.max_executable_bytes,
    )?;
    if executable != profile.runtime.executable_identity {
        return Err(domain(
            "nix_eval_executable_identity",
            "nix_eval.executable",
            "Nix evaluation executable identity differs from the admitted profile",
        ));
    }
    let manifest_identity = runtime.file_identity(
        &profile.runtime.cohort_manifest_path,
        profile.limits.max_manifest_bytes,
    )?;
    if manifest_identity != profile.runtime.cohort_manifest_identity {
        return Err(domain(
            "nix_eval_manifest_identity",
            "nix_eval.cohort_manifest",
            "Nix evaluation cohort manifest identity differs from the admitted profile",
        ));
    }
    let manifest = runtime.read_file(
        &profile.runtime.cohort_manifest_path,
        profile.limits.max_manifest_bytes,
    )?;
    let _admitted = admit_cohort_manifest(profile, &manifest)?;
    Ok(())
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

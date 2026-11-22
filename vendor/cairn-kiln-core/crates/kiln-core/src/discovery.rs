use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::types::{
    Alias, ContentId, CoreResult, Diagnostic, EffectId, EvaluationId, ExternalRef,
    PipelineRevisionId, PolicyId, ProjectId, ProviderProfileId, TaskId,
};

pub const DISCOVERY_TASK_SCHEMA: &str = "kiln.discovery-task.v1";
pub const DISCOVERY_OBSERVATION_SCHEMA: &str = "kiln.discovery-observation.v1";
const TASK_ID_PREFIX: &str = "discovered:";
const ADJACENT_PAIR_ITEMS: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DiscoverySourceMode {
    Flake,
    Expression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryOptions {
    pub workers: usize,
    pub max_memory_mebibytes: u64,
    pub include_metadata: bool,
    pub include_input_derivations: bool,
    pub check_cache_status: bool,
    pub partial_admission: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryBounds {
    pub max_records: usize,
    pub max_jobs: usize,
    pub max_job_errors: usize,
    pub max_diagnostic_bytes: usize,
    pub max_outputs_per_job: usize,
    pub max_dependencies_per_job: usize,
    pub max_tasks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryEffectPlan {
    pub effect_id: EffectId,
    pub evaluation_id: EvaluationId,
    pub project_id: ProjectId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub discovery_policy_id: PolicyId,
    pub provider_profile: ProviderProfileId,
    pub build_provider_profile: ProviderProfileId,
    pub idempotency_key: ContentId,
    pub source_identity: ContentId,
    pub source: ExternalRef,
    pub source_mode: DiscoverySourceMode,
    pub root_selector: Alias,
    pub system: Alias,
    pub options: DiscoveryOptions,
    pub bounds: DiscoveryBounds,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CacheAdvisory {
    NotChecked,
    Local,
    Cached,
    NotBuilt,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DiscoveredOutput {
    pub name: Alias,
    pub path: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredJob {
    pub attribute: Alias,
    pub attribute_path: Vec<Alias>,
    pub name: Alias,
    pub system: Alias,
    pub derivation: ExternalRef,
    pub outputs: Vec<DiscoveredOutput>,
    pub input_derivations: Vec<ExternalRef>,
    pub metadata_identity: Option<ContentId>,
    pub cache: CacheAdvisory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredJobError {
    pub attribute: Alias,
    pub diagnostic: Diagnostic,
    pub fatal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskProposal {
    pub task_id: TaskId,
    pub mapping_identity: ContentId,
    pub build_provider: ProviderProfileId,
    pub native_attribute: ExternalRef,
    pub derivation: ExternalRef,
    pub outputs: Vec<DiscoveredOutput>,
    pub dependencies: Vec<TaskId>,
    pub cache: CacheAdvisory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DiscoveryTerminal {
    Complete,
    Failed,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

impl DiscoveryTerminal {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    #[must_use]
    pub const fn is_successful(self) -> bool {
        matches!(self, Self::Complete)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProcessCompletionFact {
    Exited { status: i32 },
    Signalled,
    TimedOut,
    Cancelled,
    OutputLimitExceeded,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryCompletionFacts {
    pub stream_complete: bool,
    pub stream_valid: bool,
    pub stderr_within_bound: bool,
    pub gc_roots_ready: bool,
    pub fatal_job_error: bool,
    pub process: ProcessCompletionFact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GcRootState {
    NotRequested,
    Retained,
    Released,
    CleanupFailed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GcRootObservation {
    pub directory: Option<ExternalRef>,
    pub derivations: Vec<ExternalRef>,
    pub state: GcRootState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GcRootDecision {
    Keep,
    Release,
    AlreadyReleased,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryObservation {
    pub effect_id: EffectId,
    pub evaluation_id: EvaluationId,
    pub project_id: ProjectId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub discovery_policy_id: PolicyId,
    pub provider_profile: ProviderProfileId,
    pub idempotency_key: ContentId,
    pub source_identity: ContentId,
    pub observation_identity: ContentId,
    pub terminal: DiscoveryTerminal,
    pub jobs: Vec<DiscoveredJob>,
    pub job_errors: Vec<DiscoveredJobError>,
    pub tasks: Vec<TaskProposal>,
    pub completion: DiscoveryCompletionFacts,
    pub gc_roots: GcRootObservation,
}

// r[impl kiln.nix_eval_jobs.input_admission]
pub fn validate_discovery_plan(plan: &DiscoveryEffectPlan) -> CoreResult<()> {
    plan.source.validate()?;
    if plan.options.workers == 0 || plan.options.max_memory_mebibytes == 0 {
        return Err(Diagnostic::new(
            "discovery_option_bound",
            "discovery.options",
            "discovery worker and memory bounds must be positive",
        ));
    }
    if plan.options.partial_admission {
        return Err(Diagnostic::new(
            "partial_admission_unsupported",
            "discovery.options.partial_admission",
            "the selected discovery policy does not admit partial task sets",
        ));
    }
    let bounds = plan.bounds;
    if bounds.max_records == 0
        || bounds.max_jobs == 0
        || bounds.max_job_errors == 0
        || bounds.max_diagnostic_bytes == 0
        || bounds.max_outputs_per_job == 0
        || bounds.max_dependencies_per_job == 0
        || bounds.max_tasks == 0
        || bounds.max_jobs > bounds.max_records
        || bounds.max_job_errors > bounds.max_records
        || bounds.max_tasks > bounds.max_jobs
    {
        return Err(Diagnostic::new(
            "discovery_bound",
            "discovery.bounds",
            "discovery bounds must be positive and internally consistent",
        ));
    }
    Ok(())
}

// r[impl kiln.nix_eval_jobs.terminal_completeness]
// r[impl kiln.nix_eval_jobs.failure_scope]
#[must_use]
pub const fn classify_discovery_terminal(facts: DiscoveryCompletionFacts) -> DiscoveryTerminal {
    match facts.process {
        ProcessCompletionFact::TimedOut => DiscoveryTerminal::TimedOut,
        ProcessCompletionFact::Cancelled => DiscoveryTerminal::Cancelled,
        ProcessCompletionFact::Unknown | ProcessCompletionFact::Signalled => {
            DiscoveryTerminal::Unknown
        }
        ProcessCompletionFact::OutputLimitExceeded => DiscoveryTerminal::ProviderError,
        ProcessCompletionFact::Exited { status } => {
            if status != 0
                || !facts.stream_complete
                || !facts.stream_valid
                || !facts.stderr_within_bound
                || !facts.gc_roots_ready
            {
                DiscoveryTerminal::ProviderError
            } else if facts.fatal_job_error {
                DiscoveryTerminal::Failed
            } else {
                DiscoveryTerminal::Complete
            }
        }
    }
}

// r[impl kiln.nix_eval_jobs.task_mapping]
// r[impl kiln.nix_eval_jobs.cache_advisory]
pub fn map_task_proposals(
    plan: &DiscoveryEffectPlan,
    jobs: &[DiscoveredJob],
) -> CoreResult<Vec<TaskProposal>> {
    validate_discovery_plan(plan)?;
    if jobs.len() > plan.bounds.max_jobs || jobs.len() > plan.bounds.max_tasks {
        return Err(Diagnostic::new(
            "discovery_job_bound",
            "discovery.jobs",
            "discovered jobs exceed the selected task bound",
        ));
    }

    let mut ordered = jobs.to_vec();
    ordered.sort_by(|left, right| left.attribute.cmp(&right.attribute));
    let mut attributes = BTreeSet::new();
    let mut derivations = BTreeMap::new();
    for job in &ordered {
        validate_job(plan, job)?;
        if !attributes.insert(job.attribute.clone()) {
            return Err(Diagnostic::new(
                "duplicate_discovery_attribute",
                "discovery.jobs.attribute",
                "two discovered jobs use the same attribute identity",
            ));
        }
        if derivations
            .insert(job.derivation.clone(), job.attribute.clone())
            .is_some()
        {
            return Err(Diagnostic::new(
                "conflicting_discovery_derivation",
                "discovery.jobs.derivation",
                "two discovered jobs use the same derivation identity",
            ));
        }
    }

    let mut task_by_derivation = BTreeMap::new();
    let mut proposal_by_attribute = BTreeMap::new();
    for job in &ordered {
        let mapping_identity = task_mapping_identity(plan, job);
        let task_id = TaskId::new(format!("{TASK_ID_PREFIX}{}", mapping_identity.as_str()))?;
        task_by_derivation.insert(job.derivation.clone(), task_id.clone());
        proposal_by_attribute.insert(
            job.attribute.clone(),
            TaskProposal {
                task_id,
                mapping_identity,
                build_provider: plan.build_provider_profile.clone(),
                native_attribute: ExternalRef::new(
                    "nix-eval-jobs",
                    "attribute",
                    job.attribute.as_str(),
                )?,
                derivation: job.derivation.clone(),
                outputs: job.outputs.clone(),
                dependencies: Vec::new(),
                cache: job.cache,
            },
        );
    }

    for job in &ordered {
        let proposal = proposal_by_attribute
            .get_mut(&job.attribute)
            .ok_or(Diagnostic::new(
                "missing_discovery_proposal",
                "discovery.tasks",
                "task proposal disappeared during deterministic mapping",
            ))?;
        let mut dependencies = BTreeSet::new();
        for derivation in &job.input_derivations {
            if let Some(task_id) = task_by_derivation.get(derivation) {
                dependencies.insert(task_id.clone());
            }
        }
        proposal.dependencies = dependencies.into_iter().collect();
    }

    let mut proposals: Vec<TaskProposal> = proposal_by_attribute.into_values().collect();
    proposals.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    reject_dependency_cycles(&proposals)?;
    Ok(proposals)
}

#[must_use]
pub fn compute_discovery_observation_identity(
    plan: &DiscoveryEffectPlan,
    terminal: DiscoveryTerminal,
    jobs: &[DiscoveredJob],
    errors: &[DiscoveredJobError],
    tasks: &[TaskProposal],
    completion: DiscoveryCompletionFacts,
    gc_roots: &GcRootObservation,
) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(
        &mut hasher,
        "schema",
        DISCOVERY_OBSERVATION_SCHEMA.as_bytes(),
    );
    update_field(&mut hasher, "effect", plan.effect_id.as_str().as_bytes());
    update_field(
        &mut hasher,
        "evaluation",
        plan.evaluation_id.as_str().as_bytes(),
    );
    update_field(&mut hasher, "project", plan.project_id.as_str().as_bytes());
    update_field(
        &mut hasher,
        "pipeline",
        plan.pipeline_revision_id.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "policy",
        plan.discovery_policy_id.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "provider",
        plan.provider_profile.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "build_provider",
        plan.build_provider_profile.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "idempotency",
        plan.idempotency_key.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "source_identity",
        plan.source_identity.as_str().as_bytes(),
    );
    update_external_ref(&mut hasher, "source", &plan.source);
    update_field(
        &mut hasher,
        "source_mode",
        source_mode_label(plan.source_mode).as_bytes(),
    );
    update_field(
        &mut hasher,
        "selector",
        plan.root_selector.as_str().as_bytes(),
    );
    update_field(&mut hasher, "system", plan.system.as_str().as_bytes());
    update_usize(&mut hasher, "workers", plan.options.workers);
    update_u64(
        &mut hasher,
        "memory_mebibytes",
        plan.options.max_memory_mebibytes,
    );
    update_bool(
        &mut hasher,
        "include_metadata",
        plan.options.include_metadata,
    );
    update_bool(
        &mut hasher,
        "include_input_derivations",
        plan.options.include_input_derivations,
    );
    update_bool(
        &mut hasher,
        "check_cache_status",
        plan.options.check_cache_status,
    );
    update_bool(
        &mut hasher,
        "partial_admission",
        plan.options.partial_admission,
    );
    update_usize(&mut hasher, "max_records", plan.bounds.max_records);
    update_usize(&mut hasher, "max_jobs", plan.bounds.max_jobs);
    update_usize(&mut hasher, "max_job_errors", plan.bounds.max_job_errors);
    update_usize(
        &mut hasher,
        "max_diagnostic_bytes",
        plan.bounds.max_diagnostic_bytes,
    );
    update_usize(
        &mut hasher,
        "max_outputs_per_job",
        plan.bounds.max_outputs_per_job,
    );
    update_usize(
        &mut hasher,
        "max_dependencies_per_job",
        plan.bounds.max_dependencies_per_job,
    );
    update_usize(&mut hasher, "max_tasks", plan.bounds.max_tasks);
    update_field(&mut hasher, "terminal", terminal_label(terminal).as_bytes());
    for job in jobs {
        update_field(
            &mut hasher,
            "job_attribute",
            job.attribute.as_str().as_bytes(),
        );
        for segment in &job.attribute_path {
            update_field(
                &mut hasher,
                "job_attribute_path",
                segment.as_str().as_bytes(),
            );
        }
        update_field(&mut hasher, "job_name", job.name.as_str().as_bytes());
        update_field(&mut hasher, "job_system", job.system.as_str().as_bytes());
        update_external_ref(&mut hasher, "job_derivation", &job.derivation);
        for output in &job.outputs {
            update_field(
                &mut hasher,
                "job_output_name",
                output.name.as_str().as_bytes(),
            );
            update_external_ref(&mut hasher, "job_output_path", &output.path);
        }
        for dependency in &job.input_derivations {
            update_external_ref(&mut hasher, "job_input_derivation", dependency);
        }
        if let Some(metadata) = &job.metadata_identity {
            update_field(&mut hasher, "job_metadata", metadata.as_str().as_bytes());
        }
        update_field(&mut hasher, "job_cache", cache_label(job.cache).as_bytes());
    }
    for error in errors {
        update_field(
            &mut hasher,
            "error_attribute",
            error.attribute.as_str().as_bytes(),
        );
        update_field(&mut hasher, "error_code", error.diagnostic.code.as_bytes());
        update_field(
            &mut hasher,
            "error_field",
            error.diagnostic.field.as_bytes(),
        );
        update_field(
            &mut hasher,
            "error_message",
            error.diagnostic.message.as_bytes(),
        );
        update_bool(&mut hasher, "error_fatal", error.fatal);
    }
    for task in tasks {
        update_field(&mut hasher, "task_id", task.task_id.as_str().as_bytes());
        update_field(
            &mut hasher,
            "task_mapping",
            task.mapping_identity.as_str().as_bytes(),
        );
        update_field(
            &mut hasher,
            "task_build_provider",
            task.build_provider.as_str().as_bytes(),
        );
        update_external_ref(&mut hasher, "task_native_attribute", &task.native_attribute);
        update_external_ref(&mut hasher, "task_derivation", &task.derivation);
        for output in &task.outputs {
            update_field(
                &mut hasher,
                "task_output_name",
                output.name.as_str().as_bytes(),
            );
            update_external_ref(&mut hasher, "task_output_path", &output.path);
        }
        for dependency in &task.dependencies {
            update_field(
                &mut hasher,
                "task_dependency",
                dependency.as_str().as_bytes(),
            );
        }
        update_field(
            &mut hasher,
            "task_cache",
            cache_label(task.cache).as_bytes(),
        );
    }
    update_bool(&mut hasher, "stream_complete", completion.stream_complete);
    update_bool(&mut hasher, "stream_valid", completion.stream_valid);
    update_bool(
        &mut hasher,
        "stderr_within_bound",
        completion.stderr_within_bound,
    );
    update_bool(&mut hasher, "gc_roots_ready", completion.gc_roots_ready);
    update_bool(&mut hasher, "fatal_job_error", completion.fatal_job_error);
    update_field(
        &mut hasher,
        "process",
        process_label(completion.process).as_bytes(),
    );
    if let ProcessCompletionFact::Exited { status } = completion.process {
        update_i32(&mut hasher, "process_status", status);
    }
    if let Some(directory) = &gc_roots.directory {
        update_external_ref(&mut hasher, "gc_root_directory", directory);
    }
    for derivation in &gc_roots.derivations {
        update_external_ref(&mut hasher, "gc_root_derivation", derivation);
    }
    update_field(
        &mut hasher,
        "gc_root_state",
        gc_root_state_label(gc_roots.state).as_bytes(),
    );
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}

// r[impl kiln.nix_eval_jobs.task_mapping]
// r[impl kiln.nix_eval_jobs.core_shell_boundary]
pub fn admit_discovery_observation(
    plan: &DiscoveryEffectPlan,
    observation: &DiscoveryObservation,
) -> CoreResult<Vec<TaskProposal>> {
    validate_discovery_plan(plan)?;
    if observation.effect_id != plan.effect_id
        || observation.evaluation_id != plan.evaluation_id
        || observation.project_id != plan.project_id
        || observation.pipeline_revision_id != plan.pipeline_revision_id
        || observation.discovery_policy_id != plan.discovery_policy_id
        || observation.provider_profile != plan.provider_profile
        || observation.idempotency_key != plan.idempotency_key
        || observation.source_identity != plan.source_identity
    {
        return Err(Diagnostic::new(
            "discovery_observation_substitution",
            "discovery.observation",
            "discovery observation does not match the admitted effect plan",
        ));
    }
    if observation.jobs.len() > plan.bounds.max_jobs
        || observation.job_errors.len() > plan.bounds.max_job_errors
        || observation
            .jobs
            .len()
            .checked_add(observation.job_errors.len())
            .is_none_or(|count| count > plan.bounds.max_records)
    {
        return Err(Diagnostic::new(
            "discovery_observation_bound",
            "discovery.observation.records",
            "discovery observation exceeds its named record bounds",
        ));
    }
    validate_observation_order_and_diagnostics(plan, observation)?;
    let expected_identity = compute_discovery_observation_identity(
        plan,
        observation.terminal,
        &observation.jobs,
        &observation.job_errors,
        &observation.tasks,
        observation.completion,
        &observation.gc_roots,
    );
    if observation.observation_identity != expected_identity {
        return Err(Diagnostic::new(
            "discovery_observation_identity",
            "discovery.observation.identity",
            "discovery observation identity differs from its canonical facts",
        ));
    }
    if classify_discovery_terminal(observation.completion) != observation.terminal {
        return Err(Diagnostic::new(
            "discovery_terminal_mismatch",
            "discovery.observation.terminal",
            "discovery terminal class does not match the supplied process facts",
        ));
    }
    validate_gc_root_observation(plan, observation)?;
    if observation.terminal != DiscoveryTerminal::Complete {
        if !observation.tasks.is_empty() {
            return Err(Diagnostic::new(
                "partial_tasks_rejected",
                "discovery.observation.tasks",
                "an incomplete discovery observation cannot admit tasks",
            ));
        }
        return Ok(Vec::new());
    }
    let expected = map_task_proposals(plan, &observation.jobs)?;
    if observation.tasks != expected {
        return Err(Diagnostic::new(
            "discovery_task_substitution",
            "discovery.observation.tasks",
            "discovery task proposals differ from pure canonical mapping",
        ));
    }
    Ok(expected)
}

// r[impl kiln.nix_eval_jobs.gc_root_handoff]
pub fn decide_gc_root_handoff(
    observation: &DiscoveryObservation,
    durable_tasks: &BTreeSet<TaskId>,
    max_tasks: usize,
) -> CoreResult<GcRootDecision> {
    if max_tasks == 0 || observation.tasks.len() > max_tasks || durable_tasks.len() > max_tasks {
        return Err(Diagnostic::new(
            "gc_root_handoff_bound",
            "discovery.gc_roots",
            "GC-root handoff exceeds its named task bound",
        ));
    }
    if observation.terminal != DiscoveryTerminal::Complete || observation.tasks.is_empty() {
        return Ok(GcRootDecision::Keep);
    }
    let known_tasks: BTreeSet<TaskId> = observation
        .tasks
        .iter()
        .map(|proposal| proposal.task_id.clone())
        .collect();
    if !durable_tasks.is_subset(&known_tasks) {
        return Err(Diagnostic::new(
            "gc_root_handoff_task",
            "discovery.gc_roots",
            "GC-root handoff names a task outside the admitted discovery observation",
        ));
    }
    match observation.gc_roots.state {
        GcRootState::NotRequested => Ok(GcRootDecision::AlreadyReleased),
        GcRootState::Released => Ok(GcRootDecision::AlreadyReleased),
        GcRootState::CleanupFailed | GcRootState::Unknown => Ok(GcRootDecision::Keep),
        GcRootState::Retained => {
            if observation
                .tasks
                .iter()
                .all(|proposal| durable_tasks.contains(&proposal.task_id))
            {
                Ok(GcRootDecision::Release)
            } else {
                Ok(GcRootDecision::Keep)
            }
        }
    }
}

fn validate_observation_order_and_diagnostics(
    plan: &DiscoveryEffectPlan,
    observation: &DiscoveryObservation,
) -> CoreResult<()> {
    if observation.jobs.windows(ADJACENT_PAIR_ITEMS).any(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(left, right)| left.attribute >= right.attribute)
    }) || observation
        .job_errors
        .windows(ADJACENT_PAIR_ITEMS)
        .any(|pair| {
            pair.first()
                .zip(pair.get(1))
                .is_some_and(|(left, right)| left.attribute >= right.attribute)
        })
    {
        return Err(Diagnostic::new(
            "discovery_observation_order",
            "discovery.observation.records",
            "discovery jobs and errors must use strict canonical attribute order",
        ));
    }
    let job_attributes: BTreeSet<Alias> = observation
        .jobs
        .iter()
        .map(|job| job.attribute.clone())
        .collect();
    for error in &observation.job_errors {
        if job_attributes.contains(&error.attribute)
            || error.diagnostic.code.is_empty()
            || error.diagnostic.field.is_empty()
            || error.diagnostic.message.is_empty()
            || error.diagnostic.code.len() > plan.bounds.max_diagnostic_bytes
            || error.diagnostic.field.len() > plan.bounds.max_diagnostic_bytes
            || error.diagnostic.message.len() > plan.bounds.max_diagnostic_bytes
        {
            return Err(Diagnostic::new(
                "discovery_error_admission",
                "discovery.observation.errors",
                "job error conflicts with a job or exceeds its diagnostic bounds",
            ));
        }
    }
    Ok(())
}

fn validate_gc_root_observation(
    plan: &DiscoveryEffectPlan,
    observation: &DiscoveryObservation,
) -> CoreResult<()> {
    if observation.gc_roots.derivations.len() > plan.bounds.max_jobs {
        return Err(Diagnostic::new(
            "gc_root_observation_bound",
            "discovery.gc_roots",
            "GC-root observation exceeds its named derivation bound",
        ));
    }
    if let Some(directory) = &observation.gc_roots.directory {
        directory.validate()?;
    }
    let mut roots = BTreeSet::new();
    for derivation in &observation.gc_roots.derivations {
        derivation.validate()?;
        if !roots.insert(derivation.clone()) {
            return Err(Diagnostic::new(
                "gc_root_observation_duplicate",
                "discovery.gc_roots",
                "GC-root observation repeats a derivation identity",
            ));
        }
    }
    if observation.gc_roots.state == GcRootState::NotRequested
        && (observation.gc_roots.directory.is_some() || !roots.is_empty())
    {
        return Err(Diagnostic::new(
            "gc_root_observation_contradiction",
            "discovery.gc_roots",
            "GC-root observation contains roots when retention was not requested",
        ));
    }
    if matches!(
        observation.gc_roots.state,
        GcRootState::Retained | GcRootState::Released | GcRootState::CleanupFailed
    ) && observation.gc_roots.directory.is_none()
    {
        return Err(Diagnostic::new(
            "gc_root_directory_missing",
            "discovery.gc_roots",
            "GC-root state requires its exact owned directory identity",
        ));
    }
    if observation.terminal == DiscoveryTerminal::Complete {
        let expected: BTreeSet<ExternalRef> = observation
            .jobs
            .iter()
            .map(|job| job.derivation.clone())
            .collect();
        if expected != roots {
            return Err(Diagnostic::new(
                "gc_root_derivation_mismatch",
                "discovery.gc_roots",
                "retained derivations differ from the complete discovered job set",
            ));
        }
    }
    Ok(())
}

fn validate_job(plan: &DiscoveryEffectPlan, job: &DiscoveredJob) -> CoreResult<()> {
    job.derivation.validate()?;
    if job.system != plan.system {
        return Err(Diagnostic::new(
            "discovery_system_mismatch",
            "discovery.jobs.system",
            "discovered job system differs from the admitted target system",
        ));
    }
    if job.attribute_path.is_empty()
        || job.outputs.is_empty()
        || job.outputs.len() > plan.bounds.max_outputs_per_job
        || job.input_derivations.len() > plan.bounds.max_dependencies_per_job
    {
        return Err(Diagnostic::new(
            "discovery_job_shape",
            "discovery.jobs",
            "discovered job collections are empty or exceed their named bounds",
        ));
    }
    if job.outputs.windows(ADJACENT_PAIR_ITEMS).any(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(left, right)| left >= right)
    }) || job
        .input_derivations
        .windows(ADJACENT_PAIR_ITEMS)
        .any(|pair| {
            pair.first()
                .zip(pair.get(1))
                .is_some_and(|(left, right)| left >= right)
        })
    {
        return Err(Diagnostic::new(
            "discovery_job_order",
            "discovery.jobs",
            "discovered outputs and input derivations must use strict canonical order",
        ));
    }
    let mut output_names = BTreeSet::new();
    for output in &job.outputs {
        output.path.validate()?;
        if !output_names.insert(output.name.clone()) {
            return Err(Diagnostic::new(
                "duplicate_discovery_output",
                "discovery.jobs.outputs",
                "discovered job contains a duplicate output name",
            ));
        }
    }
    let mut dependencies = BTreeSet::new();
    for dependency in &job.input_derivations {
        dependency.validate()?;
        if !dependencies.insert(dependency.clone()) {
            return Err(Diagnostic::new(
                "duplicate_discovery_dependency",
                "discovery.jobs.input_derivations",
                "discovered job contains a duplicate input derivation",
            ));
        }
    }
    Ok(())
}

fn task_mapping_identity(plan: &DiscoveryEffectPlan, job: &DiscoveredJob) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    update_field(&mut hasher, "schema", DISCOVERY_TASK_SCHEMA.as_bytes());
    update_field(
        &mut hasher,
        "source",
        plan.source_identity.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "selector",
        plan.root_selector.as_str().as_bytes(),
    );
    update_field(&mut hasher, "system", plan.system.as_str().as_bytes());
    update_field(
        &mut hasher,
        "policy",
        plan.discovery_policy_id.as_str().as_bytes(),
    );
    update_field(
        &mut hasher,
        "pipeline",
        plan.pipeline_revision_id.as_str().as_bytes(),
    );
    update_field(&mut hasher, "attribute", job.attribute.as_str().as_bytes());
    update_external_ref(&mut hasher, "derivation", &job.derivation);
    update_field(
        &mut hasher,
        "build_provider",
        plan.build_provider_profile.as_str().as_bytes(),
    );
    for output in &job.outputs {
        update_field(&mut hasher, "output_name", output.name.as_str().as_bytes());
        update_external_ref(&mut hasher, "output_path", &output.path);
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}

fn update_external_ref(hasher: &mut blake3::Hasher, label: &str, reference: &ExternalRef) {
    update_u64(
        hasher,
        "external_contract",
        u64::from(reference.contract_version),
    );
    update_field(
        hasher,
        label,
        reference.provider_namespace.as_str().as_bytes(),
    );
    update_field(hasher, "kind", reference.native_kind.as_str().as_bytes());
    update_field(hasher, "id", reference.native_id.as_str().as_bytes());
}

fn update_bool(hasher: &mut blake3::Hasher, label: &str, value: bool) {
    update_field(hasher, label, if value { b"true" } else { b"false" });
}

fn update_i32(hasher: &mut blake3::Hasher, label: &str, value: i32) {
    update_field(hasher, label, &value.to_le_bytes());
}

fn update_u64(hasher: &mut blake3::Hasher, label: &str, value: u64) {
    update_field(hasher, label, &value.to_le_bytes());
}

fn update_usize(hasher: &mut blake3::Hasher, label: &str, value: usize) {
    update_u64(
        hasher,
        label,
        u64::try_from(value).unwrap_or_else(|_| unreachable!()),
    );
}

const fn source_mode_label(value: DiscoverySourceMode) -> &'static str {
    match value {
        DiscoverySourceMode::Flake => "flake",
        DiscoverySourceMode::Expression => "expression",
    }
}

const fn terminal_label(value: DiscoveryTerminal) -> &'static str {
    match value {
        DiscoveryTerminal::Complete => "complete",
        DiscoveryTerminal::Failed => "failed",
        DiscoveryTerminal::Cancelled => "cancelled",
        DiscoveryTerminal::TimedOut => "timed_out",
        DiscoveryTerminal::ProviderError => "provider_error",
        DiscoveryTerminal::Unknown => "unknown",
    }
}

const fn process_label(value: ProcessCompletionFact) -> &'static str {
    match value {
        ProcessCompletionFact::Exited { .. } => "exited",
        ProcessCompletionFact::Signalled => "signalled",
        ProcessCompletionFact::TimedOut => "timed_out",
        ProcessCompletionFact::Cancelled => "cancelled",
        ProcessCompletionFact::OutputLimitExceeded => "output_limit_exceeded",
        ProcessCompletionFact::Unknown => "unknown",
    }
}

const fn cache_label(value: CacheAdvisory) -> &'static str {
    match value {
        CacheAdvisory::NotChecked => "not_checked",
        CacheAdvisory::Local => "local",
        CacheAdvisory::Cached => "cached",
        CacheAdvisory::NotBuilt => "not_built",
    }
}

const fn gc_root_state_label(value: GcRootState) -> &'static str {
    match value {
        GcRootState::NotRequested => "not_requested",
        GcRootState::Retained => "retained",
        GcRootState::Released => "released",
        GcRootState::CleanupFailed => "cleanup_failed",
        GcRootState::Unknown => "unknown",
    }
}

fn update_field(hasher: &mut blake3::Hasher, label: &str, value: &[u8]) {
    let label_length = u64::try_from(label.len()).unwrap_or_else(|_| unreachable!());
    let value_length = u64::try_from(value.len()).unwrap_or_else(|_| unreachable!());
    hasher.update(&label_length.to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(&value_length.to_le_bytes());
    hasher.update(value);
}

fn reject_dependency_cycles(proposals: &[TaskProposal]) -> CoreResult<()> {
    let mut indegree: BTreeMap<TaskId, usize> = proposals
        .iter()
        .map(|proposal| (proposal.task_id.clone(), 0_usize))
        .collect();
    let mut outgoing: BTreeMap<TaskId, Vec<TaskId>> = BTreeMap::new();
    for proposal in proposals {
        for dependency in &proposal.dependencies {
            let Some(value) = indegree.get_mut(&proposal.task_id) else {
                return Err(Diagnostic::new(
                    "missing_discovery_task",
                    "discovery.tasks",
                    "task disappeared during dependency admission",
                ));
            };
            *value = value.checked_add(1).ok_or(Diagnostic::new(
                "discovery_dependency_overflow",
                "discovery.tasks.dependencies",
                "task dependency count overflowed",
            ))?;
            outgoing
                .entry(dependency.clone())
                .or_default()
                .push(proposal.task_id.clone());
        }
    }
    let mut ready: BTreeSet<TaskId> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(task, _)| task.clone())
        .collect();
    let mut visited = 0_usize;
    while let Some(task) = ready.pop_first() {
        visited = visited.checked_add(1).ok_or(Diagnostic::new(
            "discovery_task_overflow",
            "discovery.tasks",
            "visited task count overflowed",
        ))?;
        if let Some(dependents) = outgoing.get(&task) {
            for dependent in dependents {
                let Some(count) = indegree.get_mut(dependent) else {
                    return Err(Diagnostic::new(
                        "missing_discovery_dependency",
                        "discovery.tasks.dependencies",
                        "task dependency names an unknown task",
                    ));
                };
                *count = count.checked_sub(1).ok_or(Diagnostic::new(
                    "discovery_dependency_underflow",
                    "discovery.tasks.dependencies",
                    "task dependency count underflowed",
                ))?;
                if *count == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
    }
    if visited != proposals.len() {
        return Err(Diagnostic::new(
            "discovery_dependency_cycle",
            "discovery.tasks.dependencies",
            "discovered task dependencies contain a cycle",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use crate::types::BLAKE3_HEX_BYTES;

    use super::*;

    const RECORD_BOUND: usize = 8;
    const JOB_BOUND: usize = 4;
    const ERROR_BOUND: usize = 4;
    const DIAGNOSTIC_BOUND: usize = 4_096;
    const OUTPUT_BOUND: usize = 4;
    const DEPENDENCY_BOUND: usize = 4;
    const TASK_BOUND: usize = 4;
    const WORKER_COUNT: usize = 2;
    const MEMORY_MEBIBYTES: u64 = 512;
    const EXPECTED_TASK_COUNT: usize = 2;
    const SUCCESS_STATUS: i32 = 0;

    fn id(value: &str) -> ContentId {
        ContentId::from_blake3_hex(value.repeat(BLAKE3_HEX_BYTES))
            .unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> DiscoveryEffectPlan {
        DiscoveryEffectPlan {
            effect_id: EffectId::new("discover-one").unwrap_or_else(|_| unreachable!()),
            evaluation_id: EvaluationId::new("evaluation-one").unwrap_or_else(|_| unreachable!()),
            project_id: ProjectId::new("project-one").unwrap_or_else(|_| unreachable!()),
            pipeline_revision_id: PipelineRevisionId::new("pipeline-one")
                .unwrap_or_else(|_| unreachable!()),
            discovery_policy_id: PolicyId::new("discovery-policy-one")
                .unwrap_or_else(|_| unreachable!()),
            provider_profile: ProviderProfileId::new("nix-discovery-one")
                .unwrap_or_else(|_| unreachable!()),
            build_provider_profile: ProviderProfileId::new("mantle-build-v1")
                .unwrap_or_else(|_| unreachable!()),
            idempotency_key: id("1"),
            source_identity: id("2"),
            source: ExternalRef::new(
                "kiln-source",
                "nix-store-path",
                "/nix/store/00000000000000000000000000000000-source",
            )
            .unwrap_or_else(|_| unreachable!()),
            source_mode: DiscoverySourceMode::Flake,
            root_selector: Alias::new("jobs").unwrap_or_else(|_| unreachable!()),
            system: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
            options: DiscoveryOptions {
                workers: WORKER_COUNT,
                max_memory_mebibytes: MEMORY_MEBIBYTES,
                include_metadata: true,
                include_input_derivations: true,
                check_cache_status: true,
                partial_admission: false,
            },
            bounds: DiscoveryBounds {
                max_records: RECORD_BOUND,
                max_jobs: JOB_BOUND,
                max_job_errors: ERROR_BOUND,
                max_diagnostic_bytes: DIAGNOSTIC_BOUND,
                max_outputs_per_job: OUTPUT_BOUND,
                max_dependencies_per_job: DEPENDENCY_BOUND,
                max_tasks: TASK_BOUND,
            },
        }
    }

    fn job(attribute: &str, derivation: &str, dependencies: Vec<ExternalRef>) -> DiscoveredJob {
        DiscoveredJob {
            attribute: Alias::new(attribute).unwrap_or_else(|_| unreachable!()),
            attribute_path: vec![Alias::new(attribute).unwrap_or_else(|_| unreachable!())],
            name: Alias::new(attribute).unwrap_or_else(|_| unreachable!()),
            system: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
            derivation: ExternalRef::new("nix", "derivation", derivation)
                .unwrap_or_else(|_| unreachable!()),
            outputs: vec![DiscoveredOutput {
                name: Alias::new("out").unwrap_or_else(|_| unreachable!()),
                path: ExternalRef::new("nix", "store-path", &format!("{derivation}-out"))
                    .unwrap_or_else(|_| unreachable!()),
            }],
            input_derivations: dependencies,
            metadata_identity: None,
            cache: CacheAdvisory::NotBuilt,
        }
    }

    #[test]
    fn exact_jobs_map_deterministically_with_dependencies() {
        let first = job("first", "/nix/store/first.drv", Vec::new());
        let second = job(
            "second",
            "/nix/store/second.drv",
            vec![first.derivation.clone()],
        );
        let mapped = map_task_proposals(&plan(), &[second.clone(), first.clone()])
            .unwrap_or_else(|_| unreachable!());
        let replay =
            map_task_proposals(&plan(), &[first, second]).unwrap_or_else(|_| unreachable!());
        assert_eq!(mapped, replay);
        assert_eq!(mapped.len(), EXPECTED_TASK_COUNT);
        assert_eq!(
            mapped
                .iter()
                .map(|task| task.dependencies.len())
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn duplicate_derivations_and_cycles_fail_closed() {
        let first = job("first", "/nix/store/shared.drv", Vec::new());
        let second = job("second", "/nix/store/shared.drv", Vec::new());
        assert!(map_task_proposals(&plan(), &[first, second]).is_err());

        let first_derivation = ExternalRef::new("nix", "derivation", "/nix/store/first.drv")
            .unwrap_or_else(|_| unreachable!());
        let second_derivation = ExternalRef::new("nix", "derivation", "/nix/store/second.drv")
            .unwrap_or_else(|_| unreachable!());
        let cyclic_first = job("first", "/nix/store/first.drv", vec![second_derivation]);
        let cyclic_second = job("second", "/nix/store/second.drv", vec![first_derivation]);
        assert!(map_task_proposals(&plan(), &[cyclic_first, cyclic_second]).is_err());
    }

    #[test]
    fn terminal_classification_preserves_uncertainty() {
        let complete = DiscoveryCompletionFacts {
            stream_complete: true,
            stream_valid: true,
            stderr_within_bound: true,
            gc_roots_ready: true,
            fatal_job_error: false,
            process: ProcessCompletionFact::Exited {
                status: SUCCESS_STATUS,
            },
        };
        assert_eq!(
            classify_discovery_terminal(complete),
            DiscoveryTerminal::Complete
        );
        assert_eq!(
            classify_discovery_terminal(DiscoveryCompletionFacts {
                process: ProcessCompletionFact::TimedOut,
                ..complete
            }),
            DiscoveryTerminal::TimedOut
        );
        assert_eq!(
            classify_discovery_terminal(DiscoveryCompletionFacts {
                process: ProcessCompletionFact::Signalled,
                ..complete
            }),
            DiscoveryTerminal::Unknown
        );
    }

    #[test]
    fn observation_identity_and_gc_handoff_bind_exact_facts() {
        let effect_plan = plan();
        let jobs = vec![job("first", "/nix/store/first.drv", Vec::new())];
        let tasks = map_task_proposals(&effect_plan, &jobs).unwrap_or_else(|_| unreachable!());
        let completion = DiscoveryCompletionFacts {
            stream_complete: true,
            stream_valid: true,
            stderr_within_bound: true,
            gc_roots_ready: true,
            fatal_job_error: false,
            process: ProcessCompletionFact::Exited {
                status: SUCCESS_STATUS,
            },
        };
        let gc_roots = GcRootObservation {
            directory: Some(
                ExternalRef::new("nix", "gc-root-directory", "/var/lib/kiln/roots/one")
                    .unwrap_or_else(|_| unreachable!()),
            ),
            derivations: vec![
                ExternalRef::new("nix", "derivation", "/nix/store/first.drv")
                    .unwrap_or_else(|_| unreachable!()),
            ],
            state: GcRootState::Retained,
        };
        let observation_identity = compute_discovery_observation_identity(
            &effect_plan,
            DiscoveryTerminal::Complete,
            &jobs,
            &[],
            &tasks,
            completion,
            &gc_roots,
        );
        let observation = DiscoveryObservation {
            effect_id: effect_plan.effect_id.clone(),
            evaluation_id: effect_plan.evaluation_id.clone(),
            project_id: effect_plan.project_id.clone(),
            pipeline_revision_id: effect_plan.pipeline_revision_id.clone(),
            discovery_policy_id: effect_plan.discovery_policy_id.clone(),
            provider_profile: effect_plan.provider_profile.clone(),
            idempotency_key: effect_plan.idempotency_key.clone(),
            source_identity: effect_plan.source_identity.clone(),
            observation_identity,
            terminal: DiscoveryTerminal::Complete,
            jobs,
            job_errors: Vec::new(),
            tasks: tasks.clone(),
            completion,
            gc_roots,
        };
        assert_eq!(
            admit_discovery_observation(&effect_plan, &observation),
            Ok(tasks.clone())
        );
        let mut tampered = observation.clone();
        tampered.observation_identity = id("3");
        assert!(admit_discovery_observation(&effect_plan, &tampered).is_err());
        assert_eq!(
            decide_gc_root_handoff(&observation, &BTreeSet::new(), TASK_BOUND),
            Ok(GcRootDecision::Keep)
        );
        let task_id = tasks
            .first()
            .map(|task| task.task_id.clone())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            decide_gc_root_handoff(&observation, &BTreeSet::from([task_id]), TASK_BOUND,),
            Ok(GcRootDecision::Release)
        );
    }
}

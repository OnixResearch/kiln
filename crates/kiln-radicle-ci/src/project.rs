use std::path::{Component, Path, PathBuf};

use kiln_adapter_aspen::{canonical_trigger_identity, hosted_run_id};
use kiln_adapter_nix::NixBuildProfileConfig;
use kiln_adapter_nix_eval::{NixEvalProfile, OnixSourceBinding, admit_plan};
use kiln_adapter_radicle::source::{
    RadicleSourceProfile, RadicleSourceResult, RadicleSourceSelection, admit_source_profile,
};
use kiln_core::build::{BuildEvaluationOutcome, admit_build_evaluation};
use kiln_core::discovery::{DiscoveryEffectPlan, DiscoveryTerminal, admit_discovery_observation};
use kiln_core::domain::{Outcome, Product};
use kiln_core::source::validate_resolution;
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, EvaluationId, ExternalRef, RunId,
};
use kiln_nix_build::{NixBuildIntent, plan_for_task};
use kiln_onixos_pilot::WorkerResult;
use kiln_provider_api::{TriggerEventKind, TriggerRequest};
use serde::{Deserialize, Serialize};

pub type Result<T, E = String> = std::result::Result<T, E>;
pub const PROJECT_PROFILE_SCHEMA: &str = "kiln.radicle-ci.project-worker.v1";
pub const SOURCE_INTENT_SCHEMA: &str = "kiln.radicle-ci.source-intent.v1";
pub const PROJECT_RESULT_SCHEMA: &str = "kiln.radicle-ci.project-result.v1";
pub const SOURCE_INTENT_FILE: &str = "source-intent.json";
pub const PROJECT_RESULT_FILE: &str = "project-result.json";
pub const RESOLVED_JOB_FILE: &str = "resolved-job.json";
pub const WORKER_JOB_SCHEMA: &str = "kiln.onixos-pilot.worker-job.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceContext {
    pub provider: String,
    pub repository: String,
    pub revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectWorkerProfile {
    pub schema: String,
    pub source: RadicleSourceSelection,
    pub state_root: PathBuf,
    pub discovery_profile: NixEvalProfile,
    pub discovery_plan: DiscoveryEffectPlan,
    pub selected_task: Alias,
    pub build_profile: NixBuildProfileConfig,
    pub build_intent: NixBuildIntent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectBindings {
    pub key: ContentId,
    pub run_id: RunId,
    pub attempt_id: AttemptId,
    pub root: PathBuf,
    pub build_intent: NixBuildIntent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSourceIntent {
    pub schema: String,
    pub profile_identity: ContentId,
    pub source: SourceContext,
    pub pinned_profile: RadicleSourceProfile,
    pub resolution: RadicleSourceResult,
}

/// Actual bounded import and source measurements, retained before discovery.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceImportObservation {
    pub executable_identity: ContentId,
    pub process_status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub canonical_tree: ContentId,
    pub flake_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectWorkerResult {
    pub schema: String,
    pub profile_identity: ContentId,
    pub source: SourceContext,
    pub source_intent: ProjectSourceIntent,
    pub source_store_path: PathBuf,
    pub source_import: SourceImportObservation,
    pub worker: WorkerResult,
}

/// The existing worker contract, resolved from a fixed profile and acquired source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedWorkerJob {
    pub schema: String,
    pub source_uri: String,
    pub expected_source: SourceContext,
    pub discovery_profile: NixEvalProfile,
    pub discovery_plan: DiscoveryEffectPlan,
    pub selected_task: Alias,
    pub build_profile: NixBuildProfileConfig,
    pub build_intent: NixBuildIntent,
    pub state_root: PathBuf,
}

pub fn identity(value: &impl Serialize) -> Result<ContentId> {
    let bytes = serde_json::to_vec(value).map_err(text)?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(text)
}

impl ProjectWorkerProfile {
    // r[impl kiln.radicle_ci.runtime]
    pub fn admit(&self) -> Result<()> {
        self.source.admit().map_err(text)?;
        if self.schema != PROJECT_PROFILE_SCHEMA
            || !absolute_normal(&self.state_root)
            || !self.source.signed_reference.starts_with("refs/heads/")
        {
            return Err("invalid project profile schema, private state root or push branch".into());
        }
        Alias::new(
            self.source
                .signed_reference
                .trim_start_matches("refs/heads/"),
        )
        .map_err(text)?;
        admit_plan(&self.discovery_profile, &self.discovery_plan).map_err(text)?;
        let build = self.build_profile.clone().into_profile().map_err(text)?;
        if self.discovery_plan.build_provider_profile != build.provider.profile_id
            || self.discovery_plan.system != self.build_intent.platform
            || self.build_intent.platform.as_str() != build.platform
            || self.discovery_plan.evaluator_cohort != self.discovery_profile.evaluator_cohort
            || self.build_intent.plan_identity != self.build_profile.plan_identity
            || self.build_intent.policy_identity != self.build_profile.policy_identity
        {
            return Err("project discovery/build template identities differ".into());
        }
        Ok(())
    }

    pub fn trigger(&self, source: &SourceContext) -> Result<TriggerRequest> {
        if source.provider != "radicle"
            || source.repository != self.source.repository_id
            || source.revision.len() != 40
            || !source
                .revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(
                "project requires the configured repository and exact lowercase Radicle revision"
                    .into(),
            );
        }
        let branch = self
            .source
            .signed_reference
            .strip_prefix("refs/heads/")
            .ok_or("project supports only signed push branches")?;
        Ok(TriggerRequest {
            repository: ExternalRef::new("radicle", "repository", &source.repository)
                .map_err(text)?,
            actor: ExternalRef::new("radicle", "actor", "broker").map_err(text)?,
            before: None,
            after: ExternalRef::new("radicle", "revision", &source.revision).map_err(text)?,
            branch: Some(Alias::new(branch).map_err(text)?),
            patch: None,
            event: TriggerEventKind::Push,
        })
    }

    pub fn bindings(
        &self,
        profile_identity: &ContentId,
        source: &SourceContext,
    ) -> Result<ProjectBindings> {
        self.admit()?;
        ContentId::from_blake3_hex(profile_identity.as_str()).map_err(text)?;
        let trigger = self.trigger(source)?;
        let key = identity(&("kiln.radicle-ci.candidate.v1", profile_identity, source))?;
        let native = canonical_trigger_identity(&trigger, 4 * 1024 * 1024).map_err(text)?;
        let run_id = hosted_run_id(&native).map_err(text)?;
        let attempt_id =
            AttemptId::new(format!("radicle-ci-attempt-{}", key.as_str())).map_err(text)?;
        let mut build_intent = self.build_intent.clone();
        build_intent.effect_id =
            EffectId::new(format!("radicle-ci-build-{}", key.as_str())).map_err(text)?;
        build_intent.attempt_id = attempt_id.clone();
        build_intent.idempotency_key = identity(&("kiln.radicle-ci.build.v1", &key))?;
        build_intent.candidate_id =
            CandidateId::new(format!("radicle-ci-candidate-{}", key.as_str())).map_err(text)?;
        Ok(ProjectBindings {
            root: self.state_root.join(key.as_str()),
            key,
            run_id,
            attempt_id,
            build_intent,
        })
    }
}

// r[impl kiln.radicle_ci.source]
pub fn admit_source_intent(
    profile: &ProjectWorkerProfile,
    identity: &ContentId,
    intent: &ProjectSourceIntent,
) -> Result<()> {
    profile.bindings(identity, &intent.source)?;
    let pinned = &intent.pinned_profile;
    let request = &pinned.request;
    admit_source_profile(pinned).map_err(text)?;
    kiln_radicle_source_contract::admit_receipt(&intent.resolution.receipt).map_err(text)?;
    validate_resolution(&intent.resolution.resolution).map_err(text)?;
    let resolved = &intent.resolution.resolution;
    let kiln_core::source::SourceNativeFacts::Git(facts) = &resolved.native_facts else {
        return Err("source intent must retain native Git facts".into());
    };
    if facts.native_tree
        != ExternalRef::new(
            "radicle",
            "git-tree",
            &intent.resolution.receipt.snapshot.native_tree,
        )
        .map_err(text)?
        || !facts.signature_observed
    {
        return Err("source native tree/signature differs from signed receipt".into());
    }
    if intent.schema != SOURCE_INTENT_SCHEMA
        || intent.profile_identity != *identity
        || pinned.repository_path != profile.source.repository_path
        || request.repository_id != profile.source.repository_id
        || request.signer_node_id != profile.source.signer_node_id
        || request.signed_reference != profile.source.signed_reference
        || request.profile_id != profile.source.profile_id
        || request.limits != profile.source.limits
        || request.expected_revision != intent.source.revision
        || intent.resolution.receipt.request != *request
        || resolved.source_provider.as_str() != request.profile_id
        || resolved.repository != profile.trigger(&intent.source)?.repository
        || resolved.exact_revision != profile.trigger(&intent.source)?.after
        || resolved.symbolic_reference
            != ExternalRef::new("radicle", "reference", &request.signed_reference).map_err(text)?
        || resolved.canonical_tree.as_str()
            != intent.resolution.receipt.snapshot.canonical_tree_blake3
    {
        return Err("source intent differs from exact profile, source or receipt".into());
    }
    Ok(())
}

pub fn native_store_path(store: &Path, path: &Path) -> bool {
    if !absolute_normal(path) || path.parent() != Some(store) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some((hash, suffix)) = name.split_once('-') else {
        return false;
    };
    hash.len() == 32
        && hash
            .bytes()
            .all(|b| b"0123456789abcdfghijklmnpqrsvwxyz".contains(&b))
        && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+._?=-".contains(&b))
}

pub fn resolved_job(
    profile: &ProjectWorkerProfile,
    profile_identity: &ContentId,
    intent: &ProjectSourceIntent,
    source_store_path: &Path,
    imported: &SourceImportObservation,
) -> Result<ResolvedWorkerJob> {
    admit_source_intent(profile, profile_identity, intent)?;
    let bindings = profile.bindings(profile_identity, &intent.source)?;
    ContentId::from_blake3_hex(imported.flake_identity.as_str()).map_err(text)?;
    if !native_store_path(&profile.build_profile.store_path, source_store_path)
        || imported.executable_identity != profile.build_profile.executable_identity
        || imported.process_status != 0
        || imported.canonical_tree != intent.resolution.resolution.canonical_tree
        || imported.stdout != format!("{}\n", source_store_path.display()).into_bytes()
        || imported.stdout.len() > profile.build_profile.limits.max_result_bytes
        || imported.stderr.len() > profile.build_profile.limits.max_stderr_bytes
    {
        return Err(
            "source import does not bind exact measured executable, tree or native store path"
                .into(),
        );
    }
    let source = ExternalRef::new(
        "kiln-source",
        "nix-store-path",
        &source_store_path.to_string_lossy(),
    )
    .map_err(text)?;
    let source_uri = format!("path:{}", source_store_path.display());
    let mut discovery_plan = profile.discovery_plan.clone();
    discovery_plan.effect_id =
        EffectId::new(format!("radicle-ci-discover-{}", bindings.key.as_str())).map_err(text)?;
    discovery_plan.evaluation_id =
        EvaluationId::new(format!("radicle-ci-evaluation-{}", bindings.key.as_str()))
            .map_err(text)?;
    discovery_plan.idempotency_key = identity(&("kiln.radicle-ci.discovery.v1", &bindings.key))?;
    discovery_plan.source = source.clone();
    let mut discovery_profile = profile.discovery_profile.clone();
    discovery_profile.runtime.home_base = bindings.root.join("eval-home");
    discovery_plan.source_identity = imported.canonical_tree.clone();
    if discovery_profile.evaluator_cohort.is_some() {
        discovery_plan.source_identity = imported.flake_identity.clone();
        discovery_profile.onix_source_binding = Some(OnixSourceBinding {
            source,
            source_identity: imported.flake_identity.clone(),
            root_selector: discovery_plan.root_selector.clone(),
            system: discovery_plan.system.clone(),
        });
    }
    admit_plan(&discovery_profile, &discovery_plan).map_err(text)?;
    let mut build_profile = profile.build_profile.clone();
    build_profile.home_base = bindings.root.join("build-home");
    Ok(ResolvedWorkerJob {
        schema: WORKER_JOB_SCHEMA.into(),
        source_uri,
        expected_source: intent.source.clone(),
        discovery_profile,
        discovery_plan,
        selected_task: profile.selected_task.clone(),
        build_profile,
        build_intent: bindings.build_intent,
        state_root: bindings.root,
    })
}

// r[impl kiln.radicle_ci.runtime] r[impl kiln.radicle_ci.journal]
pub fn admit_result(
    profile: &ProjectWorkerProfile,
    identity: &ContentId,
    trigger: &TriggerRequest,
    result: &ProjectWorkerResult,
) -> Result<(Outcome, Vec<Product>)> {
    if result.schema != PROJECT_RESULT_SCHEMA
        || result.profile_identity != *identity
        || result.source != result.source_intent.source
        || profile.trigger(&result.source)? != *trigger
    {
        return Err("project result profile/source/trigger substitution".into());
    }
    let job = resolved_job(
        profile,
        identity,
        &result.source_intent,
        &result.source_store_path,
        &result.source_import,
    )?;
    let job_bytes = serde_json::to_vec_pretty(&job).map_err(text)?;
    let job_identity =
        ContentId::from_blake3_hex(blake3::hash(&job_bytes).to_hex().to_string()).map_err(text)?;
    let worker = &result.worker;
    if worker.schema != kiln_onixos_pilot::WORKER_RESULT_SCHEMA
        || worker.job_identity != job_identity
        || worker.source_uri != job.source_uri
        || worker.discovery_plan != job.discovery_plan
    {
        return Err("worker source/job/discovery substitution".into());
    }
    let tasks = admit_discovery_observation(&job.discovery_plan, &worker.discovery_observation)
        .map_err(text)?;
    if worker.discovery_observation.terminal != DiscoveryTerminal::Complete {
        if worker.build_plan.is_some() || worker.build_observation.is_some() {
            return Err("failed discovery cannot invent build observations".into());
        }
        let outcome = match worker.discovery_observation.terminal {
            DiscoveryTerminal::Failed => Outcome::DomainFailure,
            DiscoveryTerminal::Cancelled => Outcome::Cancelled,
            DiscoveryTerminal::TimedOut => Outcome::TimedOut,
            DiscoveryTerminal::ProviderError => Outcome::ProviderError,
            DiscoveryTerminal::Unknown => Outcome::Unknown,
            DiscoveryTerminal::Complete => return Err("invalid discovery terminal".into()),
        };
        return Ok((outcome, Vec::new()));
    }
    let task = tasks
        .iter()
        .find(|t| t.native_attribute.native_id.as_str() == profile.selected_task.as_str())
        .ok_or("selected actual discovery task is absent")?;
    let base = job.build_profile.into_profile().map_err(text)?;
    let expected = plan_for_task(&base, task, &job.build_intent).map_err(text)?;
    let plan = worker
        .build_plan
        .as_ref()
        .ok_or("actual build plan is absent")?;
    let observation = worker
        .build_observation
        .as_ref()
        .ok_or("actual build observation is absent")?;
    if *plan != expected {
        return Err("actual build plan differs from discovered task and candidate intent".into());
    }
    admit_build_evaluation(plan, observation).map_err(text)?;
    if observation.builder_identity != base.runtime.executable_identity
        || observation.worker_identity != base.runtime.worker_identity
        || observation.store_identity != base.runtime.store_identity
    {
        return Err(
            "actual build observation differs from reviewed executable/worker/store cohort".into(),
        );
    }
    let outcome = match observation.outcome {
        BuildEvaluationOutcome::Success => Outcome::Success,
        BuildEvaluationOutcome::Failed => Outcome::DomainFailure,
        BuildEvaluationOutcome::Cancelled => Outcome::Cancelled,
        BuildEvaluationOutcome::TimedOut => Outcome::TimedOut,
        BuildEvaluationOutcome::ProviderError => Outcome::ProviderError,
        BuildEvaluationOutcome::Unknown => Outcome::Unknown,
    };
    Ok((
        outcome,
        observation
            .products
            .iter()
            .map(|p| Product {
                identity: p.artifact_identity.clone(),
                producing_attempt: observation.attempt_id.clone(),
                provider_reference: None,
            })
            .collect(),
    ))
}

fn absolute_normal(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}
fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
#[path = "../../kiln-nix-build/tests/support/mod.rs"]
mod build_support;

#[cfg(test)]
pub(crate) mod tests {
    use super::build_support::{alias, config, content, intent};
    use super::*;
    use kiln_adapter_nix_eval::{NixEvalLimits, NixEvalRuntimeBinding};
    use kiln_core::build::{BuildCacheKind, BuildEvaluationObservation, BuildProductObservation};
    use kiln_core::discovery::*;
    use kiln_core::source::{
        GitObjectFormat, GitResolutionFacts, SourceNativeFacts, SourceResolution,
    };
    use kiln_core::types::{PipelineRevisionId, PolicyId, ProjectId, ProviderProfileId};

    fn observation(
        plan: &DiscoveryEffectPlan,
        terminal: DiscoveryTerminal,
        jobs: Vec<DiscoveredJob>,
        completion: DiscoveryCompletionFacts,
        gc_roots: GcRootObservation,
    ) -> DiscoveryObservation {
        let tasks = map_task_proposals(plan, &jobs).unwrap_or_else(|_| unreachable!());
        let observation_identity = compute_discovery_observation_identity(
            plan,
            terminal,
            &jobs,
            &[],
            &tasks,
            completion,
            &gc_roots,
        );
        DiscoveryObservation {
            effect_id: plan.effect_id.clone(),
            evaluation_id: plan.evaluation_id.clone(),
            project_id: plan.project_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            discovery_policy_id: plan.discovery_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            source_identity: plan.source_identity.clone(),
            observation_identity,
            terminal,
            jobs,
            job_errors: Vec::new(),
            tasks,
            completion,
            gc_roots,
        }
    }

    fn fixture() -> (ProjectWorkerProfile, ContentId, ProjectSourceIntent) {
        let limits = NixEvalLimits {
            max_line_bytes: 65_536,
            max_total_stdout_bytes: 1_048_576,
            max_stderr_bytes: 65_536,
            max_records: 64,
            max_jobs: 32,
            max_job_errors: 32,
            max_metadata_bytes: 65_536,
            max_diagnostic_bytes: 4096,
            max_outputs_per_job: 16,
            max_dependencies_per_job: 32,
            max_tasks: 32,
            max_gc_roots: 64,
            max_executable_bytes: 268_435_456,
            max_manifest_bytes: 1_048_576,
            max_in_flight: 4,
            max_workers: 2,
            max_memory_mebibytes: 512,
            stdin_max_bytes: 1,
            timeout_milliseconds: 30_000,
            poll_interval_milliseconds: 5,
            teardown_timeout_milliseconds: 1000,
        };
        let eval = NixEvalProfile::pinned(
            ProviderProfileId::new("nix-eval-jobs-v1").unwrap_or_else(|_| unreachable!()),
            NixEvalRuntimeBinding {
                executable_path: kiln_adapter_nix_eval::PINNED_EXECUTABLE_PATH.into(),
                executable_identity: ContentId::from_blake3_hex(
                    kiln_adapter_nix_eval::PINNED_EXECUTABLE_BLAKE3,
                )
                .unwrap_or_else(|_| unreachable!()),
                cohort_manifest_path: kiln_adapter_nix_eval::PINNED_COHORT_MANIFEST_PATH.into(),
                cohort_manifest_identity: ContentId::from_blake3_hex(
                    kiln_adapter_nix_eval::PINNED_COHORT_MANIFEST_BLAKE3,
                )
                .unwrap_or_else(|_| unreachable!()),
                gc_root_base: "/tmp/krc-e".into(),
                home_base: "/tmp/krc-h".into(),
            },
            limits,
        )
        .unwrap_or_else(|_| unreachable!());
        let mut receipt = kiln_radicle_source_contract::fixture_receipt();
        receipt.request.repository_id = "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE".into();
        receipt.request.signer_node_id = "z6MksnXbFoE8zkCkGWhHc8zuxpnEUhrJHv2KECRV4GSv9gkx".into();
        receipt.observation.repository_id = receipt.request.repository_id.clone();
        receipt.observation.signer_node_id = receipt.request.signer_node_id.clone();
        receipt.observation.namespaced_sigrefs_reference = format!(
            "refs/namespaces/{}/refs/rad/sigrefs",
            receipt.request.signer_node_id
        );
        let source = SourceContext {
            provider: "radicle".into(),
            repository: receipt.request.repository_id.clone(),
            revision: receipt.request.expected_revision.clone(),
        };
        let source_policy = RadicleSourceSelection {
            repository_path: "/tmp/krc-repository".into(),
            repository_id: source.repository.clone(),
            signer_node_id: receipt.request.signer_node_id.clone(),
            signed_reference: receipt.request.signed_reference.clone(),
            profile_id: receipt.request.profile_id.clone(),
            limits: receipt.request.limits.clone(),
        };
        let plan = DiscoveryEffectPlan {
            effect_id: EffectId::new("discovery-template").unwrap_or_else(|_| unreachable!()),
            evaluation_id: EvaluationId::new("evaluation-template")
                .unwrap_or_else(|_| unreachable!()),
            project_id: ProjectId::new("project").unwrap_or_else(|_| unreachable!()),
            pipeline_revision_id: PipelineRevisionId::new("pipeline-a")
                .unwrap_or_else(|_| unreachable!()),
            discovery_policy_id: PolicyId::new("discovery-policy")
                .unwrap_or_else(|_| unreachable!()),
            provider_profile: eval.provider.profile_id.clone(),
            evaluator_cohort: None,
            build_provider_profile: ProviderProfileId::new("nix-build-v1")
                .unwrap_or_else(|_| unreachable!()),
            idempotency_key: content('1'),
            source_identity: content('6'),
            source: ExternalRef::new(
                "kiln-source",
                "nix-store-path",
                "/nix/store/00000000000000000000000000000000-template",
            )
            .unwrap_or_else(|_| unreachable!()),
            source_mode: DiscoverySourceMode::Flake,
            root_selector: alias("jobs"),
            system: alias("x86_64-linux"),
            options: DiscoveryOptions {
                workers: 1,
                max_memory_mebibytes: 128,
                include_metadata: true,
                include_input_derivations: true,
                check_cache_status: true,
                partial_admission: false,
            },
            bounds: DiscoveryBounds {
                max_records: 64,
                max_jobs: 32,
                max_job_errors: 32,
                max_diagnostic_bytes: 4096,
                max_outputs_per_job: 16,
                max_dependencies_per_job: 32,
                max_tasks: 32,
            },
        };
        let profile = ProjectWorkerProfile {
            schema: PROJECT_PROFILE_SCHEMA.into(),
            source: source_policy,
            state_root: "/tmp/krc".into(),
            discovery_profile: eval,
            discovery_plan: plan,
            selected_task: alias("jobs.app"),
            build_profile: config(),
            build_intent: intent(),
        };
        let pinned = RadicleSourceProfile::pinned(
            profile.source.repository_path.clone(),
            receipt.request.clone(),
        );
        let repository = ExternalRef::new("radicle", "repository", &source.repository)
            .unwrap_or_else(|_| unreachable!());
        let reference = ExternalRef::new("radicle", "reference", &profile.source.signed_reference)
            .unwrap_or_else(|_| unreachable!());
        let revision = ExternalRef::new("radicle", "revision", &source.revision)
            .unwrap_or_else(|_| unreachable!());
        let resolution = SourceResolution {
            source_provider: ProviderProfileId::new(&profile.source.profile_id)
                .unwrap_or_else(|_| unreachable!()),
            repository: repository.clone(),
            symbolic_reference: reference.clone(),
            exact_revision: revision.clone(),
            canonical_tree: ContentId::from_blake3_hex(&receipt.snapshot.canonical_tree_blake3)
                .unwrap_or_else(|_| unreachable!()),
            native_facts: SourceNativeFacts::Git(GitResolutionFacts {
                repository,
                symbolic_reference: reference,
                exact_commit: revision.clone(),
                native_tree: ExternalRef::new("radicle", "git-tree", &receipt.snapshot.native_tree)
                    .unwrap_or_else(|_| unreachable!()),
                observed_ref_before: revision.clone(),
                observed_ref_after: revision,
                object_format: GitObjectFormat::Sha1,
                partial_repository: false,
                replacement_objects: false,
                alternate_object_database: false,
                signature_observed: true,
            }),
        };
        let profile_identity = identity(&profile).unwrap_or_else(|_| unreachable!());
        let source_intent = ProjectSourceIntent {
            schema: SOURCE_INTENT_SCHEMA.into(),
            profile_identity: profile_identity.clone(),
            source,
            pinned_profile: pinned,
            resolution: RadicleSourceResult {
                resolution,
                receipt,
            },
        };
        (profile, profile_identity, source_intent)
    }

    pub(crate) fn result() -> (
        ProjectWorkerProfile,
        ContentId,
        TriggerRequest,
        ProjectWorkerResult,
    ) {
        let (profile, identity, intent) = fixture();
        let path = PathBuf::from("/nix/store/00000000000000000000000000000000-radicle-source");
        let imported = SourceImportObservation {
            executable_identity: profile.build_profile.executable_identity.clone(),
            process_status: 0,
            stdout: format!("{}\n", path.display()).into_bytes(),
            stderr: Vec::new(),
            canonical_tree: intent.resolution.resolution.canonical_tree.clone(),
            flake_identity: content('7'),
        };
        let job = resolved_job(&profile, &identity, &intent, &path, &imported)
            .unwrap_or_else(|_| unreachable!());
        let job_record = DiscoveredJob {
            attribute: alias("jobs.app"),
            attribute_path: vec![alias("jobs"), alias("app")],
            name: alias("app"),
            system: alias("x86_64-linux"),
            derivation: ExternalRef::new(
                "nix",
                "derivation",
                "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app.drv",
            )
            .unwrap_or_else(|_| unreachable!()),
            outputs: vec![DiscoveredOutput {
                name: alias("out"),
                path: ExternalRef::new(
                    "nix",
                    "store-path",
                    "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-app",
                )
                .unwrap_or_else(|_| unreachable!()),
            }],
            input_derivations: Vec::new(),
            metadata_identity: None,
            cache: CacheAdvisory::NotChecked,
        };
        let rooted_derivation = job_record.derivation.clone();
        let discovery = observation(
            &job.discovery_plan,
            DiscoveryTerminal::Complete,
            vec![job_record],
            DiscoveryCompletionFacts {
                stream_complete: true,
                stream_valid: true,
                stderr_within_bound: true,
                gc_roots_ready: true,
                fatal_job_error: false,
                process: ProcessCompletionFact::Exited { status: 0 },
            },
            GcRootObservation {
                directory: Some(
                    ExternalRef::new("nix-eval-jobs", "gc-root-directory", "/tmp/krc-e/retained")
                        .unwrap_or_else(|_| unreachable!()),
                ),
                derivations: vec![rooted_derivation],
                state: GcRootState::Retained,
            },
        );
        let base = job
            .build_profile
            .clone()
            .into_profile()
            .unwrap_or_else(|_| unreachable!());
        let plan = plan_for_task(
            &base,
            discovery.tasks.first().unwrap_or_else(|| unreachable!()),
            &job.build_intent,
        )
        .unwrap_or_else(|_| unreachable!());
        let built = BuildEvaluationObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision: plan.pipeline_revision.clone(),
            plan_identity: plan.plan_identity.clone(),
            policy_identity: plan.policy_identity.clone(),
            provider_profile: plan.provider_profile.clone(),
            platform: plan.platform.clone(),
            outcome: BuildEvaluationOutcome::Success,
            products: vec![BuildProductObservation {
                name: alias("out"),
                artifact_identity: content('8'),
                store_identity: content('9'),
            }],
            builder_identity: base.runtime.executable_identity,
            worker_identity: base.runtime.worker_identity,
            store_identity: base.runtime.store_identity,
            cache_kind: BuildCacheKind::None,
            cache_source_identity: None,
            log_identities: Vec::new(),
            metrics: Vec::new(),
            receipt_identity: Some(content('a')),
        };
        let worker = WorkerResult {
            schema: kiln_onixos_pilot::WORKER_RESULT_SCHEMA.into(),
            job_identity: ContentId::from_blake3_hex(
                blake3::hash(&serde_json::to_vec_pretty(&job).unwrap_or_else(|_| unreachable!()))
                    .to_hex()
                    .to_string(),
            )
            .unwrap_or_else(|_| unreachable!()),
            source_uri: job.source_uri,
            discovery_plan: job.discovery_plan,
            discovery_observation: discovery,
            build_plan: Some(plan),
            build_observation: Some(built),
        };
        let trigger = profile
            .trigger(&intent.source)
            .unwrap_or_else(|_| unreachable!());
        let result = ProjectWorkerResult {
            schema: PROJECT_RESULT_SCHEMA.into(),
            profile_identity: identity.clone(),
            source: intent.source.clone(),
            source_intent: intent,
            source_store_path: path,
            source_import: imported,
            worker,
        };
        (profile, identity, trigger, result)
    }

    // r[verify kiln.radicle_ci.runtime] r[verify kiln.radicle_ci.source]
    #[test]
    fn fixed_profile_has_distinct_candidate_bindings_and_unchanged_selector() {
        let (profile, id, intent) = fixture();
        let first = profile
            .bindings(&id, &intent.source)
            .unwrap_or_else(|_| unreachable!());
        let mut second_source = intent.source.clone();
        second_source.revision = "a".repeat(40);
        let second = profile
            .bindings(&id, &second_source)
            .unwrap_or_else(|_| unreachable!());
        assert_ne!(first.key, second.key);
        assert_ne!(first.run_id, second.run_id);
        assert_ne!(first.build_intent.effect_id, second.build_intent.effect_id);
        assert_ne!(first.root, second.root);
        assert!(admit_source_intent(&profile, &id, &intent).is_ok());
        let mut changed = intent.clone();
        changed.pinned_profile.request.expected_sigrefs_commit = "a".repeat(40);
        assert!(admit_source_intent(&profile, &id, &changed).is_err());
        let (profile, id, trigger, result) = result();
        let resolved = resolved_job(
            &profile,
            &id,
            &result.source_intent,
            &result.source_store_path,
            &result.source_import,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            resolved.discovery_plan.root_selector,
            profile.discovery_plan.root_selector
        );
        assert_eq!(
            resolved.discovery_profile.runtime.gc_root_base,
            profile.discovery_profile.runtime.gc_root_base
        );
        assert_eq!(
            resolved.build_profile.gc_root_base,
            profile.build_profile.gc_root_base
        );
        let (outcome, products) =
            admit_result(&profile, &id, &trigger, &result).unwrap_or_else(|_| unreachable!());
        assert_eq!(outcome, Outcome::Success);
        assert_eq!(
            products,
            vec![Product {
                identity: content('8'),
                producing_attempt: first.attempt_id,
                provider_reference: None
            }]
        );
    }

    // r[verify kiln.radicle_ci.runtime] r[verify kiln.radicle_ci.journal]
    #[test]
    fn substituted_source_discovery_task_build_and_cohort_fail_closed() {
        let (profile, id, trigger, valid) = result();
        for mutate in [
            (|r: &mut ProjectWorkerResult| r.source.revision = "a".repeat(40))
                as fn(&mut ProjectWorkerResult),
            |r| r.source_import.canonical_tree = content('b'),
            |r| r.source_import.executable_identity = content('b'),
            |r| r.source_store_path = "/tmp/not-native-store".into(),
            |r| r.worker.discovery_plan.root_selector = alias("another.selector"),
            |r| r.worker.discovery_observation.source_identity = content('b'),
            |r| {
                r.worker
                    .discovery_observation
                    .tasks
                    .first_mut()
                    .unwrap_or_else(|| unreachable!())
                    .native_attribute =
                    ExternalRef::new("nix-eval-jobs", "attribute", "another.task")
                        .unwrap_or_else(|_| unreachable!())
            },
            |r| {
                r.worker
                    .build_plan
                    .as_mut()
                    .unwrap_or_else(|| unreachable!())
                    .attempt_id =
                    AttemptId::new("another-attempt").unwrap_or_else(|_| unreachable!())
            },
            |r| {
                r.worker
                    .build_observation
                    .as_mut()
                    .unwrap_or_else(|| unreachable!())
                    .worker_identity = content('b')
            },
        ] {
            let mut changed = valid.clone();
            mutate(&mut changed);
            assert!(admit_result(&profile, &id, &trigger, &changed).is_err());
        }
        let mut other_trigger = trigger.clone();
        other_trigger.actor =
            ExternalRef::new("radicle", "actor", "operator").unwrap_or_else(|_| unreachable!());
        assert!(admit_result(&profile, &id, &other_trigger, &valid).is_err());
        let mut failed = valid;
        failed
            .worker
            .build_observation
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .outcome = BuildEvaluationOutcome::Failed;
        failed
            .worker
            .build_observation
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .products
            .clear();
        assert_eq!(
            admit_result(&profile, &id, &trigger, &failed).unwrap_or_else(|_| unreachable!()),
            (Outcome::DomainFailure, Vec::new())
        );
    }

    // r[verify kiln.radicle_ci.runtime] r[verify kiln.radicle_ci.recovery]
    #[test]
    fn unknown_discovery_never_invents_build_evidence() {
        let (profile, id, trigger, mut result) = result();
        result.worker.discovery_observation = observation(
            &result.worker.discovery_plan,
            DiscoveryTerminal::Unknown,
            Vec::new(),
            DiscoveryCompletionFacts {
                stream_complete: false,
                stream_valid: true,
                stderr_within_bound: true,
                gc_roots_ready: false,
                fatal_job_error: false,
                process: ProcessCompletionFact::Unknown,
            },
            GcRootObservation {
                directory: None,
                derivations: Vec::new(),
                state: GcRootState::Unknown,
            },
        );
        assert!(admit_result(&profile, &id, &trigger, &result).is_err());
        result.worker.build_plan = None;
        result.worker.build_observation = None;
        assert_eq!(
            admit_result(&profile, &id, &trigger, &result).unwrap_or_else(|_| unreachable!()),
            (Outcome::Unknown, Vec::new())
        );
    }

    // r[verify kiln.radicle_ci.runtime] r[verify kiln.radicle_ci.source]
    #[test]
    fn onix_keeps_reviewed_cohort_and_selector_but_binds_measured_source() {
        use kiln_adapter_nix_eval::{
            ONIX_EXECUTABLE_BLAKE3, ONIX_EXECUTABLE_PATH, ONIX_MANIFEST_BLAKE3, ONIX_MANIFEST_PATH,
        };
        let (mut profile, _, mut source_intent) = fixture();
        let mut runtime = profile.discovery_profile.runtime.clone();
        runtime.executable_path = ONIX_EXECUTABLE_PATH.into();
        runtime.executable_identity =
            ContentId::from_blake3_hex(ONIX_EXECUTABLE_BLAKE3).unwrap_or_else(|_| unreachable!());
        runtime.cohort_manifest_path = ONIX_MANIFEST_PATH.into();
        runtime.cohort_manifest_identity =
            ContentId::from_blake3_hex(ONIX_MANIFEST_BLAKE3).unwrap_or_else(|_| unreachable!());
        profile.discovery_profile = NixEvalProfile::onix(
            runtime,
            profile.discovery_profile.limits,
            OnixSourceBinding {
                source: profile.discovery_plan.source.clone(),
                source_identity: profile.discovery_plan.source_identity.clone(),
                root_selector: profile.discovery_plan.root_selector.clone(),
                system: profile.discovery_plan.system.clone(),
            },
        )
        .unwrap_or_else(|_| unreachable!());
        profile.discovery_plan.provider_profile =
            profile.discovery_profile.provider.profile_id.clone();
        profile.discovery_plan.evaluator_cohort =
            profile.discovery_profile.evaluator_cohort.clone();
        let id = identity(&profile).unwrap_or_else(|_| unreachable!());
        source_intent.profile_identity = id.clone();
        let path = Path::new("/nix/store/00000000000000000000000000000000-acquired");
        let imported = SourceImportObservation {
            executable_identity: profile.build_profile.executable_identity.clone(),
            process_status: 0,
            stdout: format!("{}\n", path.display()).into_bytes(),
            stderr: Vec::new(),
            canonical_tree: source_intent.resolution.resolution.canonical_tree.clone(),
            flake_identity: content('d'),
        };
        let job = resolved_job(&profile, &id, &source_intent, path, &imported)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(job.discovery_plan.source_identity, imported.flake_identity);
        assert_eq!(
            job.discovery_plan.root_selector,
            profile.discovery_plan.root_selector
        );
        assert_eq!(
            job.discovery_plan.evaluator_cohort,
            profile.discovery_plan.evaluator_cohort
        );
        assert_eq!(
            job.discovery_profile.runtime.executable_identity,
            profile.discovery_profile.runtime.executable_identity
        );
        let binding = job
            .discovery_profile
            .onix_source_binding
            .as_ref()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(binding.source, job.discovery_plan.source);
        assert_eq!(binding.source_identity, imported.flake_identity);
        let mut mixed = profile;
        mixed.discovery_plan.evaluator_cohort = None;
        assert!(mixed.admit().is_err());
    }
}

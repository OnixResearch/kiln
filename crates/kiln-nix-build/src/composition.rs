use std::path::{Path, PathBuf};

use kiln_adapter_nix::{
    NixBuildAdapter, NixBuildProfile, NixBuildRuntime, NixOutputBinding, admit_profile,
};
use kiln_core::build::{BuildEffectPlan, BuildEvaluationObservation, validate_build_effect_plan};
use kiln_core::discovery::TaskProposal;
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, Diagnostic, EffectId, PipelineRevisionId,
};
use kiln_provider_api::{BuildPort, PortFailure, PortResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NixBuildIntent {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub idempotency_key: ContentId,
    pub candidate_id: CandidateId,
    pub pipeline_revision: PipelineRevisionId,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub platform: Alias,
}

// r[impl kiln.nix_build.composition]
pub fn profile_for_task(
    base: &NixBuildProfile,
    task: &TaskProposal,
    intent: &NixBuildIntent,
    derivation_identity: ContentId,
) -> PortResult<NixBuildProfile> {
    let derivation_path = task_derivation_path(base, task)?;
    let mut expected_outputs = task_outputs(base, task)?;
    expected_outputs.sort_by(|left, right| left.name.cmp(&right.name));
    let mut profile = base.clone();
    profile.runtime.derivation_path = derivation_path;
    profile.runtime.derivation_identity = derivation_identity;
    profile.runtime.plan_identity = intent.plan_identity.clone();
    profile.runtime.policy_identity = intent.policy_identity.clone();
    profile.runtime.expected_outputs = expected_outputs;
    admit_profile(&profile)?;
    Ok(profile)
}

// r[impl kiln.nix_build.composition]
pub fn plan_for_task(
    base: &NixBuildProfile,
    task: &TaskProposal,
    intent: &NixBuildIntent,
) -> PortResult<BuildEffectPlan> {
    if task.build_provider != base.provider.profile_id {
        return Err(domain(
            "nix_task_provider",
            "nix.task.build_provider",
            "Task build provider differs from the admitted Nix provider",
        ));
    }
    if intent.platform.as_str() != base.platform {
        return Err(domain(
            "nix_task_platform",
            "nix.task.intent.platform",
            "Task intent platform differs from the admitted Nix platform",
        ));
    }
    let mut required_products = task
        .outputs
        .iter()
        .map(|output| output.name.clone())
        .collect::<Vec<_>>();
    required_products.sort();
    let plan = BuildEffectPlan {
        effect_id: intent.effect_id.clone(),
        attempt_id: intent.attempt_id.clone(),
        idempotency_key: intent.idempotency_key.clone(),
        candidate_id: intent.candidate_id.clone(),
        pipeline_revision: intent.pipeline_revision.clone(),
        plan_identity: intent.plan_identity.clone(),
        policy_identity: intent.policy_identity.clone(),
        provider_profile: base.provider.profile_id.clone(),
        platform: intent.platform.clone(),
        required_products,
    };
    validate_build_effect_plan(&plan, base.limits.max_required_products)?;
    Ok(plan)
}

// r[impl kiln.nix_build.composition]
pub fn build_task<R: NixBuildRuntime>(
    base: &NixBuildProfile,
    task: &TaskProposal,
    intent: &NixBuildIntent,
    runtime: R,
) -> PortResult<(
    NixBuildAdapter<R>,
    BuildEffectPlan,
    BuildEvaluationObservation,
)> {
    let derivation_path = task_derivation_path(base, task)?;
    let derivation_identity =
        runtime.file_identity(&derivation_path, base.limits.max_derivation_bytes)?;
    let profile = profile_for_task(base, task, intent, derivation_identity)?;
    let plan = plan_for_task(base, task, intent)?;
    let mut adapter = NixBuildAdapter::new(profile, runtime)?;
    let observation = adapter.dispatch(&plan)?;
    Ok((adapter, plan, observation))
}

fn task_derivation_path(base: &NixBuildProfile, task: &TaskProposal) -> PortResult<PathBuf> {
    if task.build_provider != base.provider.profile_id {
        return Err(domain(
            "nix_task_provider",
            "nix.task.build_provider",
            "Task build provider differs from the admitted Nix provider",
        ));
    }
    if task.derivation.provider_namespace.as_str() != "nix"
        || task.derivation.native_kind.as_str() != "derivation"
    {
        return Err(domain(
            "nix_task_derivation",
            "nix.task.derivation",
            "Task derivation is not a native Nix derivation reference",
        ));
    }
    let path = PathBuf::from(task.derivation.native_id.as_str());
    if path.parent() != Some(base.runtime.store_path.as_path())
        || path.extension() != Some(std::ffi::OsStr::new("drv"))
    {
        return Err(domain(
            "nix_task_derivation",
            "nix.task.derivation",
            "Task derivation must be a direct derivation child of the admitted store",
        ));
    }
    Ok(path)
}

fn task_outputs(base: &NixBuildProfile, task: &TaskProposal) -> PortResult<Vec<NixOutputBinding>> {
    let mut outputs = Vec::with_capacity(task.outputs.len());
    for output in &task.outputs {
        if output.path.provider_namespace.as_str() != "nix"
            || output.path.native_kind.as_str() != "store-path"
        {
            return Err(domain(
                "nix_task_output",
                "nix.task.outputs",
                "Task output is not a native Nix store-path reference",
            ));
        }
        let path = PathBuf::from(output.path.native_id.as_str());
        if path.parent() != Some(base.runtime.store_path.as_path())
            || !Path::new(&path).is_absolute()
        {
            return Err(domain(
                "nix_task_output",
                "nix.task.outputs",
                "Task output must be a direct child of the admitted store",
            ));
        }
        outputs.push(NixOutputBinding {
            name: output.name.clone(),
            path,
        });
    }
    if outputs.is_empty() {
        return Err(domain(
            "nix_task_outputs",
            "nix.task.outputs",
            "Task contains no required Nix outputs",
        ));
    }
    Ok(outputs)
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

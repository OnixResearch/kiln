use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use kiln_adapter_nix::{
    BoundedNixBuildRuntime, NixBuildProfile, NixBuildProfileConfig, NixBuildRuntime,
};
use kiln_core::build::{BuildEffectPlan, BuildEvaluationObservation};
use kiln_core::discovery::TaskProposal;
use kiln_core::types::Diagnostic;
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};

use crate::composition::{NixBuildIntent, plan_for_task, profile_for_task};

pub const STATE_SCHEMA: &str = "kiln.nix-build-state.v1";
const MAX_STATE_BYTES: u64 = 4_194_304;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NixBuildProfileSource {
    Bound {
        config: Box<NixBuildProfileConfig>,
    },
    Task {
        base: Box<NixBuildProfileConfig>,
        task: Box<TaskProposal>,
        intent: Box<NixBuildIntent>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NixBuildState {
    pub schema: String,
    pub profile: NixBuildProfileSource,
    pub plan: BuildEffectPlan,
    pub observation: Option<BuildEvaluationObservation>,
    pub roots_released: bool,
}

// r[impl kiln.nix_build.state]
impl NixBuildState {
    #[must_use]
    pub fn new_bound(config: NixBuildProfileConfig, plan: BuildEffectPlan) -> Self {
        Self {
            schema: STATE_SCHEMA.to_string(),
            profile: NixBuildProfileSource::Bound {
                config: Box::new(config),
            },
            plan,
            observation: None,
            roots_released: false,
        }
    }

    #[must_use]
    pub fn new_task(
        base: NixBuildProfileConfig,
        task: TaskProposal,
        intent: NixBuildIntent,
        plan: BuildEffectPlan,
    ) -> Self {
        Self {
            schema: STATE_SCHEMA.to_string(),
            profile: NixBuildProfileSource::Task {
                base: Box::new(base),
                task: Box::new(task),
                intent: Box::new(intent),
            },
            plan,
            observation: None,
            roots_released: false,
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if !metadata.is_file() || metadata.len() > MAX_STATE_BYTES {
            return Err(format!(
                "{} is not a bounded regular state document",
                path.display()
            ));
        }
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|mut file| file.read_to_end(&mut bytes))
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(format!("{} exceeds its state bound", path.display()));
        }
        let state: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
        if state.schema != STATE_SCHEMA {
            return Err(format!(
                "{} has an unsupported state schema",
                path.display()
            ));
        }
        Ok(state)
    }

    pub fn store(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("cannot encode state: {error}"))?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err("state document exceeds its bound".to_string());
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        let temporary = path.with_extension("tmp");
        {
            let mut file = File::create(&temporary)
                .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
            file.write_all(&bytes)
                .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
            file.sync_all()
                .map_err(|error| format!("cannot sync {}: {error}", temporary.display()))?;
        }
        fs::rename(&temporary, path)
            .map_err(|error| format!("cannot publish {}: {error}", path.display()))?;
        if let Some(directory) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .and_then(|parent| File::open(parent).ok())
        {
            let _ = directory.sync_all();
        }
        Ok(())
    }

    pub fn rehydrate(&self) -> PortResult<(NixBuildProfile, BuildEffectPlan)> {
        match &self.profile {
            NixBuildProfileSource::Bound { config } => {
                Ok((config.as_ref().clone().into_profile()?, self.plan.clone()))
            }
            NixBuildProfileSource::Task { base, task, intent } => {
                let base_profile = base.clone().into_profile()?;
                let runtime = BoundedNixBuildRuntime;
                let derivation_path = PathBuf::from(task.derivation.native_id.as_str());
                let derivation_identity = runtime
                    .file_identity(&derivation_path, base_profile.limits.max_derivation_bytes)?;
                let profile = profile_for_task(&base_profile, task, intent, derivation_identity)?;
                let plan = plan_for_task(&base_profile, task, intent)?;
                if plan != self.plan {
                    return Err(domain(
                        "nix_state_plan",
                        "nix.state.plan",
                        "Durable state plan differs from the recomputed task plan",
                    ));
                }
                Ok((profile, plan))
            }
        }
    }
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

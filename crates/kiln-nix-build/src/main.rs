use std::path::Path;

use kiln_adapter_nix::{
    BoundedNixBuildRuntime, NixBuildAdapter, NixBuildProfile, NixBuildProfileConfig,
    NixBuildRuntime,
};
use kiln_core::build::{BuildEffectPlan, BuildEvaluationObservation, BuildLogReadRequest};
use kiln_core::discovery::TaskProposal;
use kiln_nix_build::{
    NixBuildIntent, NixBuildProfileSource, NixBuildState, build_task, plan_for_task,
};
use kiln_provider_api::{BuildPort, PortFailure};
use serde::de::DeserializeOwned;

const USAGE: &str = "usage: kiln-nix-build doctor --profile PROFILE.json
       kiln-nix-build build --profile PROFILE.json --plan PLAN.json [--state STATE.json] [--log LOG] [--release]
       kiln-nix-build from-task --profile BASE.json --task TASK.json --intent INTENT.json [--state STATE.json] [--log LOG] [--release]
       kiln-nix-build observe --state STATE.json
       kiln-nix-build reconcile --state STATE.json
       kiln-nix-build release --state STATE.json";
const MAX_DOCUMENT_BYTES: u64 = 4_194_304;
const MAX_LOG_CHUNK_BYTES: usize = 65_536;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

// r[impl kiln.nix_build.composition]
fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        fail(USAGE, EXIT_USAGE);
    };
    let rest = arguments.collect::<Vec<_>>();
    let result = match command.as_str() {
        "doctor" => doctor(&rest),
        "build" => build(&rest),
        "from-task" => from_task(&rest),
        "observe" => observe(&rest, false),
        "reconcile" => observe(&rest, true),
        "release" => release(&rest),
        _ => Err(format!("unsupported command: {command}")),
    };
    match result {
        Ok(output) => println!("{output}"),
        Err(message) => fail(&message, EXIT_FAILURE),
    }
}

fn doctor(arguments: &[String]) -> Result<String, String> {
    let options = Options::parse(arguments, &["--profile"], &[])?;
    let profile_path = options.required("--profile")?;
    let profile = load_profile(profile_path)?;
    let profile_id = profile.provider.profile_id.as_str().to_string();
    let mut adapter =
        NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
    adapter.preflight().map_err(failure_text)?;
    Ok(format!(
        "{{\"profile\":\"{profile_id}\",\"status\":\"admitted\"}}"
    ))
}

fn build(arguments: &[String]) -> Result<String, String> {
    let options = Options::parse(
        arguments,
        &["--profile", "--plan", "--state", "--log"],
        &["--release"],
    )?;
    let profile_path = options.required("--profile")?;
    let plan_path = options.required("--plan")?;
    let state_path = options.optional("--state").map(Path::new);
    let log_path = options.optional("--log");
    let release = options.flag("--release");
    let config = load_config(profile_path)?;
    let plan: BuildEffectPlan = load_document(plan_path)?;
    if let Some(path) = state_path {
        if path.exists() {
            let state = NixBuildState::load(path)?;
            if state.plan != plan
                || state.profile
                    != (NixBuildProfileSource::Bound {
                        config: Box::new(config.clone()),
                    })
            {
                return Err("state does not match the requested profile and plan".to_string());
            }
            let Some(observation) = state.observation.as_ref() else {
                return Err(
                    "state has no terminal observation; reconcile or remove the state file"
                        .to_string(),
                );
            };
            if log_path.is_some() {
                return Err("log bytes are unavailable after a state restore".to_string());
            }
            let (profile, plan) = state.rehydrate().map_err(failure_text)?;
            let mut adapter =
                NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
            adapter.restore(&plan, observation).map_err(failure_text)?;
            let observed = adapter.observe(&plan.effect_id).map_err(failure_text)?;
            let rendered = render_observation(&observed)?;
            if release && !state.roots_released {
                adapter
                    .release_output_roots(&plan.effect_id)
                    .map_err(failure_text)?;
                let mut state = state;
                state.roots_released = true;
                state.store(path)?;
            }
            return Ok(rendered);
        }
        let profile = config.clone().into_profile().map_err(failure_text)?;
        let mut adapter =
            NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
        adapter.preflight().map_err(failure_text)?;
        let mut state = NixBuildState::new_bound(config, plan.clone());
        state.store(path)?;
        let observation = adapter.dispatch(&plan).map_err(failure_text)?;
        state.observation = Some(observation.clone());
        if release {
            adapter
                .release_output_roots(&plan.effect_id)
                .map_err(failure_text)?;
            state.roots_released = true;
        }
        state.store(path)?;
        if let Some(log) = log_path {
            write_log(&mut adapter, &observation, log)?;
        }
        return render_observation(&observation);
    }
    let profile = config.into_profile().map_err(failure_text)?;
    let mut adapter =
        NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
    adapter.preflight().map_err(failure_text)?;
    let observation = adapter.dispatch(&plan).map_err(failure_text)?;
    if let Some(log) = log_path {
        write_log(&mut adapter, &observation, log)?;
    }
    let rendered = render_observation(&observation)?;
    if release {
        adapter
            .release_output_roots(&plan.effect_id)
            .map_err(failure_text)?;
    }
    Ok(rendered)
}

fn from_task(arguments: &[String]) -> Result<String, String> {
    let options = Options::parse(
        arguments,
        &["--profile", "--task", "--intent", "--state", "--log"],
        &["--release"],
    )?;
    let profile_path = options.required("--profile")?;
    let task_path = options.required("--task")?;
    let intent_path = options.required("--intent")?;
    let state_path = options.optional("--state").map(Path::new);
    let log_path = options.optional("--log");
    let release = options.flag("--release");
    let base_config = load_config(profile_path)?;
    let task: TaskProposal = load_document(task_path)?;
    let intent: NixBuildIntent = load_document(intent_path)?;
    let base = base_config.clone().into_profile().map_err(failure_text)?;
    let plan = plan_for_task(&base, &task, &intent).map_err(failure_text)?;
    if let Some(path) = state_path {
        if path.exists() {
            let state = NixBuildState::load(path)?;
            if state.plan != plan
                || state.profile
                    != (NixBuildProfileSource::Task {
                        base: Box::new(base_config.clone()),
                        task: Box::new(task.clone()),
                        intent: Box::new(intent.clone()),
                    })
            {
                return Err("state does not match the requested task and intent".to_string());
            }
            let Some(observation) = state.observation.as_ref() else {
                return Err(
                    "state has no terminal observation; reconcile or remove the state file"
                        .to_string(),
                );
            };
            if log_path.is_some() {
                return Err("log bytes are unavailable after a state restore".to_string());
            }
            let (profile, plan) = state.rehydrate().map_err(failure_text)?;
            let mut adapter =
                NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
            adapter.restore(&plan, observation).map_err(failure_text)?;
            let observed = adapter.observe(&plan.effect_id).map_err(failure_text)?;
            let rendered = render_observation(&observed)?;
            if release && !state.roots_released {
                adapter
                    .release_output_roots(&plan.effect_id)
                    .map_err(failure_text)?;
                let mut state = state;
                state.roots_released = true;
                state.store(path)?;
            }
            return Ok(rendered);
        }
        let mut state =
            NixBuildState::new_task(base_config, task.clone(), intent.clone(), plan.clone());
        state.store(path)?;
        let (mut adapter, built_plan, observation) =
            build_task(&base, &task, &intent, BoundedNixBuildRuntime).map_err(failure_text)?;
        if built_plan != plan {
            return Err("composed plan changed during dispatch".to_string());
        }
        state.observation = Some(observation.clone());
        if release {
            adapter
                .release_output_roots(&plan.effect_id)
                .map_err(failure_text)?;
            state.roots_released = true;
        }
        state.store(path)?;
        if let Some(log) = log_path {
            write_log(&mut adapter, &observation, log)?;
        }
        return render_observation(&observation);
    }
    let (mut adapter, plan, observation) =
        build_task(&base, &task, &intent, BoundedNixBuildRuntime).map_err(failure_text)?;
    if let Some(log) = log_path {
        write_log(&mut adapter, &observation, log)?;
    }
    let rendered = render_observation(&observation)?;
    if release {
        adapter
            .release_output_roots(&plan.effect_id)
            .map_err(failure_text)?;
    }
    Ok(rendered)
}

fn observe(arguments: &[String], reconcile: bool) -> Result<String, String> {
    let options = Options::parse(arguments, &["--state"], &[])?;
    let path = options.required("--state")?;
    let state = NixBuildState::load(Path::new(path))?;
    let Some(observation) = state.observation.as_ref() else {
        return Err("state has no terminal observation".to_string());
    };
    let (profile, plan) = state.rehydrate().map_err(failure_text)?;
    let mut adapter =
        NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
    adapter.restore(&plan, observation).map_err(failure_text)?;
    let observed = if reconcile {
        adapter.reconcile(&plan).map_err(failure_text)?
    } else {
        adapter.observe(&plan.effect_id).map_err(failure_text)?
    };
    render_observation(&observed)
}

fn release(arguments: &[String]) -> Result<String, String> {
    let options = Options::parse(arguments, &["--state"], &[])?;
    let path = options.required("--state")?;
    let mut state = NixBuildState::load(Path::new(path))?;
    if state.roots_released {
        return Ok("{\"released\":true}".to_string());
    }
    let Some(observation) = state.observation.as_ref() else {
        return Err("state has no terminal observation".to_string());
    };
    let (profile, plan) = state.rehydrate().map_err(failure_text)?;
    let mut adapter =
        NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
    adapter.restore(&plan, observation).map_err(failure_text)?;
    adapter
        .release_output_roots(&plan.effect_id)
        .map_err(failure_text)?;
    state.roots_released = true;
    state.store(Path::new(path))?;
    Ok("{\"released\":true}".to_string())
}

fn load_config(path: &str) -> Result<NixBuildProfileConfig, String> {
    let value: serde_json::Value = load_document(path)?;
    let value = match value {
        serde_json::Value::Object(mut object) if object.contains_key("profile") => object
            .remove("profile")
            .ok_or_else(|| format!("{path} has no profile value"))?,
        other => other,
    };
    serde_json::from_value(value).map_err(|error| format!("cannot parse {path}: {error}"))
}

fn load_profile(path: &str) -> Result<NixBuildProfile, String> {
    load_config(path)?.into_profile().map_err(failure_text)
}

fn load_document<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(format!("{path} is not a bounded regular JSON document"));
    }
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("cannot parse {path}: {error}"))
}

fn write_log<R: NixBuildRuntime>(
    adapter: &mut NixBuildAdapter<R>,
    observation: &BuildEvaluationObservation,
    path: &str,
) -> Result<(), String> {
    let Some(log_identity) = observation.log_identities.first() else {
        return Err("observation has no log identity".to_string());
    };
    let max_total = adapter.profile_binding().limits.max_log_bytes;
    let chunk_size = MAX_LOG_CHUNK_BYTES.min(max_total);
    let mut offset = 0_usize;
    let mut bytes = Vec::new();
    loop {
        let chunk = adapter
            .read_log(&BuildLogReadRequest {
                effect_id: observation.effect_id.clone(),
                log_identity: log_identity.clone(),
                offset,
                max_bytes: chunk_size,
            })
            .map_err(failure_text)?;
        let count = chunk.bytes.len();
        bytes.extend_from_slice(&chunk.bytes);
        if bytes.len() > max_total {
            return Err("log exceeds the admitted profile bound".to_string());
        }
        if chunk.end_of_log {
            break;
        }
        offset = offset
            .checked_add(count)
            .ok_or_else(|| "log offset overflowed".to_string())?;
        if count == 0 {
            return Err("log read made no progress".to_string());
        }
    }
    std::fs::write(path, bytes).map_err(|error| format!("cannot write {path}: {error}"))
}

fn render_observation(observation: &BuildEvaluationObservation) -> Result<String, String> {
    serde_json::to_string_pretty(observation)
        .map_err(|error| format!("cannot encode observation: {error}"))
}

fn failure_text(failure: PortFailure) -> String {
    match failure {
        PortFailure::Domain(diagnostic) => {
            format!(
                "{}: {}: {}",
                diagnostic.code, diagnostic.field, diagnostic.message
            )
        }
        PortFailure::Infrastructure(error)
        | PortFailure::Retryable(error)
        | PortFailure::TimedOut(error)
        | PortFailure::Cancelled(error)
        | PortFailure::Unknown(error) => format!("{}: {}", error.code, error.message),
    }
}

fn fail(message: &str, code: i32) -> ! {
    eprintln!("{message}");
    std::process::exit(code);
}

#[derive(Debug, Default)]
struct Options {
    values: Vec<(String, String)>,
}

impl Options {
    fn parse(arguments: &[String], allowed: &[&str], booleans: &[&str]) -> Result<Self, String> {
        let mut values = Vec::new();
        let mut index = 0;
        while index < arguments.len() {
            let flag = arguments
                .get(index)
                .ok_or_else(|| "missing option".to_string())?;
            if !allowed.contains(&flag.as_str()) && !booleans.contains(&flag.as_str()) {
                return Err(format!("unsupported option: {flag}"));
            }
            if booleans.contains(&flag.as_str()) {
                values.push((flag.clone(), String::new()));
                index += 1;
                continue;
            }
            let value = arguments
                .get(index + 1)
                .ok_or_else(|| format!("missing value for {flag}"))?;
            values.push((flag.clone(), value.clone()));
            index += 2;
        }
        Ok(Self { values })
    }

    fn required(&self, flag: &str) -> Result<&str, String> {
        self.optional(flag)
            .ok_or_else(|| format!("missing required option {flag}"))
    }

    fn optional(&self, flag: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(name, _value)| name == flag)
            .map(|(_name, value)| value.as_str())
    }

    fn flag(&self, flag: &str) -> bool {
        self.values.iter().any(|(name, _value)| name == flag)
    }
}

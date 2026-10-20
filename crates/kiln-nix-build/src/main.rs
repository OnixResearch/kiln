use kiln_adapter_nix::{
    BoundedNixBuildRuntime, NixBuildAdapter, NixBuildProfileConfig, NixBuildRuntime,
};
use kiln_core::build::{BuildEffectPlan, BuildEvaluationObservation, BuildLogReadRequest};
use kiln_core::discovery::TaskProposal;
use kiln_nix_build::{NixBuildIntent, build_task};
use kiln_provider_api::{BuildPort, PortFailure};
use serde::de::DeserializeOwned;

const USAGE: &str = "usage: kiln-nix-build doctor --profile PROFILE.json\n       kiln-nix-build build --profile PROFILE.json --plan PLAN.json [--log LOG] [--release]\n       kiln-nix-build from-task --profile BASE.json --task TASK.json --intent INTENT.json [--log LOG] [--release]";
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
    NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
    Ok(format!(
        "{{\"profile\":\"{profile_id}\",\"status\":\"admitted\"}}"
    ))
}

fn build(arguments: &[String]) -> Result<String, String> {
    let options = Options::parse(arguments, &["--profile", "--plan", "--log"], &["--release"])?;
    let profile_path = options.required("--profile")?;
    let plan_path = options.required("--plan")?;
    let log_path = options.optional("--log");
    let release = options.flag("--release");
    let profile = load_profile(profile_path)?;
    let plan: BuildEffectPlan = load_document(plan_path)?;
    let mut adapter =
        NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(failure_text)?;
    let observation = adapter.dispatch(&plan).map_err(failure_text)?;
    if let Some(path) = log_path {
        write_log(&mut adapter, &observation, path)?;
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
        &["--profile", "--task", "--intent", "--log"],
        &["--release"],
    )?;
    let profile_path = options.required("--profile")?;
    let task_path = options.required("--task")?;
    let intent_path = options.required("--intent")?;
    let log_path = options.optional("--log");
    let release = options.flag("--release");
    let profile = load_profile(profile_path)?;
    let task: TaskProposal = load_document(task_path)?;
    let intent: NixBuildIntent = load_document(intent_path)?;
    let (mut adapter, plan, observation) =
        build_task(&profile, &task, &intent, BoundedNixBuildRuntime).map_err(failure_text)?;
    if let Some(path) = log_path {
        write_log(&mut adapter, &observation, path)?;
    }
    let rendered = render_observation(&observation)?;
    if release {
        adapter
            .release_output_roots(&plan.effect_id)
            .map_err(failure_text)?;
    }
    Ok(rendered)
}

fn load_profile(path: &str) -> Result<kiln_adapter_nix::NixBuildProfile, String> {
    let value: serde_json::Value = load_document(path)?;
    let value = match value {
        serde_json::Value::Object(mut object) if object.contains_key("profile") => object
            .remove("profile")
            .ok_or_else(|| format!("{path} has no profile value"))?,
        other => other,
    };
    let config: NixBuildProfileConfig =
        serde_json::from_value(value).map_err(|error| format!("cannot parse {path}: {error}"))?;
    config.into_profile().map_err(failure_text)
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

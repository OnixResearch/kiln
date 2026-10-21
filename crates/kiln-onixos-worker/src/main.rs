mod project;
mod runtime;

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use fs2::FileExt;
use kiln_adapter_nix::BoundedNixBuildRuntime;
use kiln_adapter_nix_eval::{NixEvalAdapter, NixEvalProfile, admit_plan};
use kiln_core::build::{BuildEvaluationOutcome, BuildLogReadRequest};
use kiln_core::discovery::{DiscoveryTerminal, admit_discovery_observation};
use kiln_core::types::{ContentId, ProviderProfileId};
use kiln_nix_build::{NixBuildState, build_task, plan_for_task};
use kiln_onixos_pilot::WorkerResult;
use kiln_provider_api::{BuildPort, DiscoveryPort};
use serde::{Deserialize, Serialize};

const JOB_SCHEMA: &str = "kiln.onixos-pilot.worker-job.v1";
const RESULT_SCHEMA: &str = "kiln.onixos-pilot.worker-result.v1";
const MAX_DOCUMENT_BYTES: u64 = 16_777_216;
const RESULT_FILE: &str = "worker-result.json";
const INTENT_FILE: &str = "worker-intent.json";
const BUILD_FILE: &str = "build-state.json";

use kiln_radicle_ci::project::{ResolvedWorkerJob as WorkerJob, SourceContext};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EvaluatorKind {
    #[default]
    Onix,
    Stock,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvalSelection {
    #[serde(default)]
    kind: EvaluatorKind,
    runtime: kiln_adapter_nix_eval::NixEvalRuntimeBinding,
    limits: kiln_adapter_nix_eval::NixEvalLimits,
    source_binding: kiln_adapter_nix_eval::OnixSourceBinding,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerIntent {
    schema: String,
    job_identity: ContentId,
    source_uri: String,
}

fn main() {
    match execute(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(success) => {
            if !success {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("kiln-onixos-worker: {error}");
            std::process::exit(2);
        }
    }
}

// r[impl kiln.onixos_pilot.runtime] r[impl kiln.onixos_pilot.failure]
fn execute(arguments: &[String]) -> Result<bool, String> {
    if arguments
        .first()
        .is_some_and(|command| command == "project-run" || command == "project-release")
    {
        return project::execute(arguments);
    }
    if let [command, path] = arguments {
        if command == "eval-profile" {
            let selected: EvalSelection =
                serde_json::from_slice(&read_bounded(Path::new(path))?).map_err(text)?;
            let profile = match selected.kind {
                EvaluatorKind::Onix => {
                    NixEvalProfile::onix(selected.runtime, selected.limits, selected.source_binding)
                }
                EvaluatorKind::Stock => NixEvalProfile::pinned(
                    ProviderProfileId::new("nix-eval-jobs-v1").map_err(text)?,
                    selected.runtime,
                    selected.limits,
                ),
            }
            .map_err(text)?;
            println!("{}", serde_json::to_string(&profile).map_err(text)?);
            return Ok(true);
        }
    }
    let [command, path, identity] = arguments else {
        return Err("usage: kiln-onixos-worker eval-profile SELECTION_JSON | run|release JOB_JSON JOB_BLAKE3".to_string());
    };
    if command != "run" && command != "release" {
        return Err("unknown worker command".to_string());
    }
    let bytes = read_bounded(Path::new(path))?;
    let job_identity = ContentId::from_blake3_hex(identity).map_err(text)?;
    if blake3::hash(&bytes).to_hex().as_str() != identity {
        return Err("worker job bytes do not match their reviewed identity".to_string());
    }
    let job: WorkerJob = serde_json::from_slice(&bytes).map_err(text)?;
    admit_job(&job)?;
    if command == "run" {
        let actual = SourceContext {
            provider: std::env::var("LATTICE_SOURCE_PROVIDER").map_err(text)?,
            repository: std::env::var("LATTICE_SOURCE_REPOSITORY").map_err(text)?,
            revision: std::env::var("LATTICE_SOURCE_REVISION").map_err(text)?,
        };
        admit_source_context(&job.expected_source, &actual)?;
    }
    private_directory(&job.state_root)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(job.state_root.join("worker.lock"))
        .map_err(text)?;
    lock.try_lock_exclusive()
        .map_err(|_| "worker state is already in use".to_string())?;
    if command == "release" {
        release(&job, &job_identity)?;
        println!("{{\"roots_released\":true}}");
        return Ok(true);
    }
    run(&job, job_identity)
}

fn admit_job(job: &WorkerJob) -> Result<(), String> {
    if job.schema != JOB_SCHEMA || job.source_uri.is_empty() || !job.state_root.is_absolute() {
        return Err("worker job schema, source or state root is invalid".to_string());
    }
    if job.expected_source.provider != "radicle"
        || job.expected_source.repository.is_empty()
        || job.expected_source.revision.len() != 40
        || !job
            .expected_source
            .revision
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err("worker requires an exact declared Radicle source context".to_string());
    }
    admit_plan(&job.discovery_profile, &job.discovery_plan).map_err(text)?;
    job.build_profile.clone().into_profile().map_err(text)?;
    Ok(())
}

fn admit_source_context(expected: &SourceContext, actual: &SourceContext) -> Result<(), String> {
    if expected.provider != actual.provider
        || expected.repository != actual.repository
        || expected.revision != actual.revision
    {
        return Err("native workflow source context differs from the reviewed job".to_string());
    }
    Ok(())
}

// r[impl kiln.onixos_pilot.recovery]
fn run(job: &WorkerJob, job_identity: ContentId) -> Result<bool, String> {
    let result_path = job.state_root.join(RESULT_FILE);
    let intent_path = job.state_root.join(INTENT_FILE);
    if intent_path.exists() {
        let intent: WorkerIntent =
            serde_json::from_slice(&read_bounded(&intent_path)?).map_err(text)?;
        if intent.schema != JOB_SCHEMA
            || intent.job_identity != job_identity
            || intent.source_uri != job.source_uri
        {
            return Err("existing worker intent has different job or source bindings".to_string());
        }
        let result: WorkerResult =
            serde_json::from_slice(&read_bounded(&result_path).map_err(|_| {
                "worker intent has no complete result; uncertainty does not authorize redispatch"
                    .to_string()
            })?)
            .map_err(text)?;
        if result.schema != RESULT_SCHEMA
            || result.job_identity != job_identity
            || result.source_uri != job.source_uri
            || result.discovery_plan != job.discovery_plan
        {
            return Err("existing worker result does not match its reserved job".to_string());
        }
        admit_discovery_observation(&job.discovery_plan, &result.discovery_observation)
            .map_err(text)?;
        println!("{}", serde_json::to_string(&result).map_err(text)?);
        return Ok(successful(&result));
    }
    if result_path.exists() {
        return Err("worker result exists without its reserved intent".to_string());
    }
    private_directory(&job.build_profile.home_base)?;
    store(
        &intent_path,
        &WorkerIntent {
            schema: JOB_SCHEMA.to_string(),
            job_identity: job_identity.clone(),
            source_uri: job.source_uri.clone(),
        },
    )?;
    let runtime = runtime::RetainingEvalRuntime {
        root: job.state_root.clone(),
    };
    let mut discovery =
        NixEvalAdapter::new(job.discovery_profile.clone(), runtime).map_err(text)?;
    let observation = discovery.discover(&job.discovery_plan).map_err(text)?;
    admit_discovery_observation(&job.discovery_plan, &observation).map_err(text)?;
    store(&job.state_root.join("discovery.json"), &observation)?;
    let mut result = WorkerResult {
        schema: RESULT_SCHEMA.to_string(),
        job_identity,
        source_uri: job.source_uri.clone(),
        discovery_plan: job.discovery_plan.clone(),
        discovery_observation: observation,
        build_plan: None,
        build_observation: None,
    };
    if result.discovery_observation.terminal == DiscoveryTerminal::Complete {
        let task = result
            .discovery_observation
            .tasks
            .iter()
            .find(|task| task.native_attribute.native_id.as_str() == job.selected_task.as_str())
            .ok_or_else(|| {
                "admitted discovery omitted the selected application task".to_string()
            })?;
        let base = job.build_profile.clone().into_profile().map_err(text)?;
        let plan = plan_for_task(&base, task, &job.build_intent).map_err(text)?;
        let mut build_state = NixBuildState::new_task(
            job.build_profile.clone(),
            task.clone(),
            job.build_intent.clone(),
            plan.clone(),
        );
        build_state.store(&job.state_root.join(BUILD_FILE))?;
        let (mut adapter, built_plan, built) =
            build_task(&base, task, &job.build_intent, BoundedNixBuildRuntime).map_err(text)?;
        if plan != built_plan {
            return Err("build plan changed during real dispatch".to_string());
        }
        for log_identity in &built.log_identities {
            let log = adapter
                .read_log(&BuildLogReadRequest {
                    effect_id: plan.effect_id.clone(),
                    log_identity: log_identity.clone(),
                    offset: 0,
                    max_bytes: base.limits.max_log_bytes,
                })
                .map_err(text)?;
            if !log.end_of_log {
                return Err("build log exceeds the admitted retained window".to_string());
            }
            store_bytes(
                &job.state_root
                    .join(format!("{}.log", log_identity.as_str())),
                &log.bytes,
            )?;
        }
        build_state.observation = Some(built.clone());
        build_state.store(&job.state_root.join(BUILD_FILE))?;
        result.build_plan = Some(built_plan);
        result.build_observation = Some(built);
    }
    // Persist the observation before asking the core to authorize any root release.
    store(&result_path, &result)?;
    let durable: BTreeSet<_> = result
        .discovery_observation
        .tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect();
    let handed_off = discovery
        .handoff_gc_roots(&job.discovery_plan, &durable)
        .map_err(text)?;
    if !handed_off.is_empty() {
        let released = discovery
            .cleanup_gc_roots(&job.discovery_plan)
            .map_err(text)?;
        store(
            &job.state_root.join("discovery-root-release.json"),
            &released,
        )?;
    }
    println!("{}", serde_json::to_string(&result).map_err(text)?);
    Ok(successful(&result))
}

fn successful(result: &WorkerResult) -> bool {
    result.discovery_observation.terminal == DiscoveryTerminal::Complete
        && result
            .build_observation
            .as_ref()
            .is_some_and(|build| build.outcome == BuildEvaluationOutcome::Success)
}

// Explicit operator handoff only: never invoked automatically by the workflow.
fn release(job: &WorkerJob, job_identity: &ContentId) -> Result<(), String> {
    let result: WorkerResult =
        serde_json::from_slice(&read_bounded(&job.state_root.join(RESULT_FILE))?).map_err(text)?;
    if result.job_identity != *job_identity
        || result.source_uri != job.source_uri
        || result.discovery_plan != job.discovery_plan
    {
        return Err("root release names a different worker result".to_string());
    }
    let mut state = NixBuildState::load(&job.state_root.join(BUILD_FILE))?;
    let observation = state
        .observation
        .as_ref()
        .ok_or_else(|| "build has no durable observation".to_string())?;
    if result.build_plan.as_ref() != Some(&state.plan)
        || result.build_observation.as_ref() != Some(observation)
    {
        return Err("build state differs from the durable worker result".to_string());
    }
    if state.roots_released {
        return Ok(());
    }
    let (profile, plan) = state.rehydrate().map_err(text)?;
    let mut adapter =
        kiln_adapter_nix::NixBuildAdapter::new(profile, BoundedNixBuildRuntime).map_err(text)?;
    adapter.restore(&plan, observation).map_err(text)?;
    adapter
        .release_output_roots(&plan.effect_id)
        .map_err(text)?;
    state.roots_released = true;
    state.store(&job.state_root.join(BUILD_FILE))
}

fn private_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err("worker state root is not a real directory".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(text)
        }
        Err(error) => Err(text(error)),
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(text)?;
    let metadata = file.metadata().map_err(text)?;
    if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES {
        return Err("worker document is not a bounded regular file".to_string());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("worker document grew beyond its bound".to_string());
    }
    Ok(bytes)
}

fn store(path: &Path, value: &impl Serialize) -> Result<(), String> {
    store_bytes(path, &serde_json::to_vec_pretty(value).map_err(text)?)
}

fn store_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("worker output exceeds its document bound".to_string());
    }
    let temporary = path.with_extension("stage");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(text)?;
    file.write_all(bytes).map_err(text)?;
    file.sync_all().map_err(text)?;
    fs::rename(&temporary, path).map_err(text)?;
    File::open(
        path.parent()
            .ok_or_else(|| "worker output lacks parent directory".to_string())?,
    )
    .and_then(|directory| directory.sync_all())
    .map_err(text)
}

fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify kiln.onixos_pilot.trigger] r[verify kiln.onixos_pilot.failure]
    #[test]
    fn native_source_context_must_match_every_admitted_fact() {
        let expected = SourceContext {
            provider: "radicle".to_string(),
            repository: "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE".to_string(),
            revision: "6c6e4f220fe0234ce607085fa622b580fd1cd016".to_string(),
        };
        assert!(admit_source_context(&expected, &expected).is_ok());
        for actual in [
            SourceContext {
                provider: "github".to_string(),
                repository: expected.repository.clone(),
                revision: expected.revision.clone(),
            },
            SourceContext {
                provider: expected.provider.clone(),
                repository: "rad:another-repository".to_string(),
                revision: expected.revision.clone(),
            },
            SourceContext {
                provider: expected.provider.clone(),
                repository: expected.repository.clone(),
                revision: "69afc2697f4330005797256135ae81b2722a125c".to_string(),
            },
        ] {
            assert!(admit_source_context(&expected, &actual).is_err());
        }
    }
}

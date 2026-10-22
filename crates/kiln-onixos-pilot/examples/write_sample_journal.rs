//! Write a sample canonical journal with the OnixOS pilot's own writer.
//!
//! The pilot stores its durable `StartRun` intent before it reaches the
//! Aspen host, so this example produces a real pilot-written journal without a
//! host: the recorded document carries the pilot's own schema, its own event
//! identity convention, and its own seal.
//!
//! usage: cargo run -p kiln-onixos-pilot --example write_sample_journal -- <path>

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::SigningKey;
use kiln_core::build::BuildEffectPlan;
use kiln_core::discovery::{
    DiscoveryBounds, DiscoveryEffectPlan, DiscoveryOptions, DiscoverySourceMode,
};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, CoreResult, EffectId, EvaluationId, ExternalRef,
    PipelineRevisionId, PolicyId, ProjectId, ProviderProfileId,
};
use kiln_onixos_pilot::{PROFILE_SCHEMA, PilotProfile, prepare_request, sign};
use kiln_provider_api::{TriggerEventKind, TriggerRequest};

const SECRET_SEED: [u8; 32] = [7; 32];
const SOCKET_PATH: &str = "/tmp/kiln-pilot-sample.sock";
const REQUEST_WINDOW_SECONDS: u64 = 60;

fn failure(message: String) -> ExitCode {
    eprintln!("write_sample_journal: {message}");
    ExitCode::FAILURE
}

fn content(text: &str) -> Result<ContentId, String> {
    kiln_onixos_pilot::identity(&text).map_err(|error| format!("{error:?}"))
}

fn external(kind: &str, value: &str) -> Result<ExternalRef, String> {
    ExternalRef::new("test", kind, value).map_err(|error| format!("{error:?}"))
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next().map(PathBuf::from) else {
        eprintln!("usage: write_sample_journal <path>");
        return ExitCode::FAILURE;
    };
    match write(&path) {
        Ok(()) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(message) => failure(message),
    }
}

fn write(path: &Path) -> Result<(), String> {
    let key = SigningKey::from_bytes(&SECRET_SEED);
    let public_key = URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes());
    // The pilot refuses a journal whose directory is group or world writable,
    // so the example owns a private directory next to the requested journal.
    let directory = path
        .parent()
        .ok_or_else(|| "journal path lacks a parent directory".to_string())?;
    std::fs::create_dir_all(directory).map_err(|error| format!("directory: {error}"))?;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("directory permissions: {error}"))?;
    let secret_path = directory.join("operator.key");
    // The pilot reads its operator secret as a JSON string document.
    let secret = format!("\"{}\"", URL_SAFE_NO_PAD.encode(SECRET_SEED));
    std::fs::write(&secret_path, secret).map_err(|error| format!("secret: {error}"))?;
    let profile = pilot_profile(&public_key)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock: {error}"))?
        .as_secs();
    let request = prepare_request(&profile, trigger()?, now, now + REQUEST_WINDOW_SECONDS)?;
    let signed = sign(&secret_path, request)?;
    // The pilot stores its durable intent before the host effect, so the host
    // failure below is expected and the journal is the recorded artifact.
    let outcome = kiln_onixos_pilot::start(&profile, signed, path);
    match outcome {
        Ok(_) => Ok(()),
        Err(error) => {
            if path.exists() {
                Ok(())
            } else {
                Err(format!("pilot did not write a journal: {error}"))
            }
        }
    }
}

fn trigger() -> Result<TriggerRequest, String> {
    Ok(TriggerRequest {
        repository: external("repository", "rad:test")?,
        actor: external("actor", "operator")?,
        before: None,
        after: external("revision", "revision")?,
        branch: None,
        patch: None,
        event: TriggerEventKind::Push,
    })
}

fn pilot_profile(public_key: &str) -> Result<PilotProfile, String> {
    let discovery_plan = DiscoveryEffectPlan {
        effect_id: effect_id("discovery")?,
        evaluation_id: evaluation_id("evaluation")?,
        project_id: project_id("project")?,
        pipeline_revision_id: pipeline_revision_id("pipeline")?,
        discovery_policy_id: policy_id("policy")?,
        provider_profile: provider_profile_id("discovery")?,
        evaluator_cohort: None,
        build_provider_profile: provider_profile_id("build")?,
        idempotency_key: content("discovery-key")?,
        source_identity: content("source")?,
        source: external("source", "/nix/store/test-source")?,
        source_mode: DiscoverySourceMode::Flake,
        root_selector: alias("jobs")?,
        system: alias("x86_64-linux")?,
        options: DiscoveryOptions {
            workers: 1,
            max_memory_mebibytes: 128,
            include_metadata: false,
            include_input_derivations: true,
            check_cache_status: false,
            partial_admission: false,
        },
        bounds: DiscoveryBounds {
            max_records: 16,
            max_jobs: 16,
            max_job_errors: 16,
            max_diagnostic_bytes: 512,
            max_outputs_per_job: 16,
            max_dependencies_per_job: 16,
            max_tasks: 16,
        },
    };
    let build_plan = BuildEffectPlan {
        effect_id: effect_id("build")?,
        attempt_id: attempt_id("attempt")?,
        idempotency_key: content("build-key")?,
        candidate_id: candidate_id("candidate")?,
        pipeline_revision: discovery_plan.pipeline_revision_id.clone(),
        plan_identity: content("plan")?,
        policy_identity: content("policy")?,
        provider_profile: discovery_plan.build_provider_profile.clone(),
        platform: discovery_plan.system.clone(),
        required_products: vec![alias("out")?],
    };
    Ok(PilotProfile {
        schema: PROFILE_SCHEMA.into(),
        allowed_operator_keys: vec![public_key.to_string()],
        cohort_identity: content("cohort")?,
        source_uri: "rad:test@revision".into(),
        source_identity: discovery_plan.source_identity.clone(),
        job_identity: content("job")?,
        selected_task: alias("app")?,
        max_request_age_seconds: REQUEST_WINDOW_SECONDS,
        max_request_bytes: 1024 * 1024,
        max_journal_bytes: 1024 * 1024,
        max_journal_records: 16,
        socket_path: SOCKET_PATH.into(),
        transport_timeout_seconds: 1,
        maximum_frame_bytes: 1024 * 1024,
        runtime_profile: kiln_adapter_aspen::profile_fixture(),
        discovery_plan,
        build_plan,
    })
}

fn core<T>(value: CoreResult<T>) -> Result<T, String> {
    value.map_err(|error| format!("{error:?}"))
}

fn effect_id(value: &str) -> Result<EffectId, String> {
    core(EffectId::new(value))
}

fn evaluation_id(value: &str) -> Result<EvaluationId, String> {
    core(EvaluationId::new(value))
}

fn project_id(value: &str) -> Result<ProjectId, String> {
    core(ProjectId::new(value))
}

fn pipeline_revision_id(value: &str) -> Result<PipelineRevisionId, String> {
    core(PipelineRevisionId::new(value))
}

fn policy_id(value: &str) -> Result<PolicyId, String> {
    core(PolicyId::new(value))
}

fn provider_profile_id(value: &str) -> Result<ProviderProfileId, String> {
    core(ProviderProfileId::new(value))
}

fn attempt_id(value: &str) -> Result<AttemptId, String> {
    core(AttemptId::new(value))
}

fn candidate_id(value: &str) -> Result<CandidateId, String> {
    core(CandidateId::new(value))
}

fn alias(value: &str) -> Result<Alias, String> {
    core(Alias::new(value))
}

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kiln_adapter_patchgate::{
    PatchgateAdapter, PatchgateProfile, candidate_root, native_binding_identity, policy_identity,
};
use kiln_core::gating::{CandidateChange, CandidateSet, DependencyEdge};
use kiln_core::review::{ReviewEffectPlan, ReviewEvaluationObservation};
use kiln_core::types::{
    CandidateId, ContentId, EffectId, ExternalRef, PipelineRevisionId, PolicyId, ProviderProfileId,
};
use kiln_provider_api::PortFailure;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const USAGE: &str = "usage: kiln-patchgate-review plan --profile PROFILE.json --intent INTENT.json
       kiln-patchgate-review review --profile PROFILE.json --plan PLAN.json
       kiln-patchgate-review reconcile --profile PROFILE.json --plan PLAN.json --receipt RECEIPT.json

Only isolated review evidence is supported. No command authorizes source acceptance.
A successful exit means the evidence was admitted, not that every review was accepted.";
const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
// One native byte can require six JSON bytes; reserve one document for metadata.
const MAX_JSON_ESCAPE_EXPANSION: usize = 6;
const MAX_RECEIPT_BYTES: usize = (MAX_JSON_ESCAPE_EXPANSION + 1) * MAX_DOCUMENT_BYTES;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;
const EXIT_CANCELLED: i32 = 130;
const RECEIPT_SCHEMA: &str = "kiln.patchgate.review.v1";
const IDEMPOTENCY_SCHEMA: &str = "kiln.review.idempotency.v1";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenSource {
    change: ExternalRef,
    snapshot: ContentId,
}

// JSON cannot represent a map keyed by a structured ExternalRef. Keep that
// interchange detail outside the provider-neutral CandidateSet.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateInput {
    id: CandidateId,
    ordered_changes: Vec<CandidateChange>,
    dependencies: Vec<DependencyEdge>,
    frozen_sources: Vec<FrozenSource>,
    policy_id: PolicyId,
}

impl CandidateInput {
    fn into_candidate(self) -> Result<CandidateSet, String> {
        let mut frozen_sources = BTreeMap::new();
        for source in self.frozen_sources {
            match frozen_sources.entry(source.change) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(source.snapshot);
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err("duplicate frozen source identity".to_string());
                }
            }
        }
        Ok(CandidateSet {
            id: self.id,
            ordered_changes: self.ordered_changes,
            dependencies: self.dependencies,
            frozen_sources,
            policy_id: self.policy_id,
        })
    }
}

impl From<CandidateSet> for CandidateInput {
    fn from(candidate: CandidateSet) -> Self {
        Self {
            id: candidate.id,
            ordered_changes: candidate.ordered_changes,
            dependencies: candidate.dependencies,
            frozen_sources: candidate
                .frozen_sources
                .into_iter()
                .map(|(change, snapshot)| FrozenSource { change, snapshot })
                .collect(),
            policy_id: candidate.policy_id,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewIntent {
    effect_id: EffectId,
    candidate: CandidateInput,
    pipeline_revision: PipelineRevisionId,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanDocument {
    effect_id: EffectId,
    idempotency_key: ContentId,
    candidate: CandidateInput,
    candidate_root: ContentId,
    pipeline_revision: PipelineRevisionId,
    policy_identity: ContentId,
    provider_profile: ProviderProfileId,
    native_binding_identity: ContentId,
}

impl PlanDocument {
    fn into_plan(self) -> Result<ReviewEffectPlan, String> {
        Ok(ReviewEffectPlan {
            effect_id: self.effect_id,
            idempotency_key: self.idempotency_key,
            candidate: self.candidate.into_candidate()?,
            candidate_root: self.candidate_root,
            pipeline_revision: self.pipeline_revision,
            policy_identity: self.policy_identity,
            provider_profile: self.provider_profile,
            native_binding_identity: self.native_binding_identity,
        })
    }
}

impl From<ReviewEffectPlan> for PlanDocument {
    fn from(plan: ReviewEffectPlan) -> Self {
        Self {
            effect_id: plan.effect_id,
            idempotency_key: plan.idempotency_key,
            candidate: plan.candidate.into(),
            candidate_root: plan.candidate_root,
            pipeline_revision: plan.pipeline_revision,
            policy_identity: plan.policy_identity,
            provider_profile: plan.provider_profile,
            native_binding_identity: plan.native_binding_identity,
        }
    }
}

#[derive(Debug, Serialize)]
struct ReviewReceipt<'a> {
    schema: &'static str,
    request_identity: ContentId,
    observation: ReviewEvaluationObservation,
    // A string, not a parsed/re-serialized Value: every original report byte,
    // including whitespace and provenance fields, survives receipt retention.
    native_report: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedReceipt {
    schema: String,
    request_identity: ContentId,
    observation: ReviewEvaluationObservation,
    native_report: String,
}

#[derive(Debug)]
struct CliError {
    message: String,
    exit_code: i32,
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self {
            message,
            exit_code: EXIT_FAILURE,
        }
    }
}

fn main() {
    match execute(std::env::args().skip(1).collect()) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::exit(error.exit_code);
        }
    }
}

// r[impl kiln.patchgate.request] r[impl kiln.patchgate.nonclaims]
fn execute(arguments: Vec<String>) -> Result<String, CliError> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage());
    };
    if matches!(command, "--help" | "-h") && arguments.len() == 1 {
        return Ok(USAGE.to_string());
    }
    let allowed: &[&str] = match command {
        "plan" => &["--profile", "--intent"],
        "review" => &["--profile", "--plan"],
        "reconcile" => &["--profile", "--plan", "--receipt"],
        _ => return Err(usage()),
    };
    let rest = arguments.get(1..).ok_or_else(usage)?;
    let options = parse_options(rest, allowed)?;
    let profile: PatchgateProfile = load_document(required(&options, "--profile")?)?;
    let mut adapter = PatchgateAdapter::new(profile).map_err(port_error)?;
    if command == "plan" {
        let intent: ReviewIntent = load_document(required(&options, "--intent")?)?;
        let profile = adapter.profile_config();
        let candidate = intent.candidate.into_candidate()?;
        let root =
            candidate_root(&candidate, profile.limits.max_request_bytes).map_err(port_error)?;
        let binding = native_binding_identity(profile).map_err(port_error)?;
        let policy = policy_identity(profile).map_err(port_error)?;
        let (_, idempotency_key) = kiln_codec::encode_and_identify(
            &(
                IDEMPOTENCY_SCHEMA,
                &intent.effect_id,
                &root,
                &intent.pipeline_revision,
                &policy,
                &profile.provider.profile_id,
                &binding,
            ),
            profile.limits.max_request_bytes,
        )
        .map_err(|_| "review intent exceeds canonical request bounds".to_string())?;
        let plan = ReviewEffectPlan {
            effect_id: intent.effect_id,
            idempotency_key,
            candidate,
            candidate_root: root,
            pipeline_revision: intent.pipeline_revision,
            policy_identity: policy,
            provider_profile: profile.provider.profile_id.clone(),
            native_binding_identity: binding,
        };
        adapter.validate_plan(&plan).map_err(port_error)?;
        return render(&PlanDocument::from(plan)).map_err(Into::into);
    }
    let document: PlanDocument = load_document(required(&options, "--plan")?)?;
    let plan = document.into_plan()?;
    let request_bound = adapter.profile_config().limits.max_request_bytes;
    let (_, request_identity) = kiln_codec::encode_and_identify(&plan, request_bound)
        .map_err(|_| "review plan exceeds canonical request bounds".to_string())?;
    let (observation, retained) = if command == "reconcile" {
        let bytes = load_bytes(required(&options, "--receipt")?, MAX_RECEIPT_BYTES)?;
        let receipt: RetainedReceipt = serde_json::from_slice(&bytes)
            .map_err(|_| "retained receipt has invalid fields or syntax".to_string())?;
        if receipt.schema != RECEIPT_SCHEMA || receipt.request_identity != request_identity {
            return Err("retained receipt belongs to a different request or schema"
                .to_string()
                .into());
        }
        let report = receipt.native_report.into_bytes();
        let observation = adapter
            .reconcile(&plan, &receipt.observation, &report)
            .map_err(port_error)?;
        (observation, Some(report))
    } else {
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut registrations = Vec::new();
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            match signal_hook::flag::register(signal, Arc::clone(&cancellation)) {
                Ok(id) => registrations.push(id),
                Err(_) => {
                    for id in registrations {
                        signal_hook::low_level::unregister(id);
                    }
                    return Err("could not register review cancellation".to_string().into());
                }
            }
        }
        let result = adapter.review_with_cancellation(&plan, &cancellation);
        for id in registrations {
            signal_hook::low_level::unregister(id);
        }
        (result.map_err(port_error)?, None)
    };
    let bytes = retained
        .as_deref()
        .or_else(|| adapter.raw_report())
        .ok_or_else(|| "completed review has no retained native report".to_string())?;
    let native_report =
        std::str::from_utf8(bytes).map_err(|_| "native review report is not UTF-8".to_string())?;
    render(&ReviewReceipt {
        schema: RECEIPT_SCHEMA,
        request_identity,
        observation,
        native_report,
    })
    .map_err(Into::into)
}

fn parse_options<'a>(
    arguments: &'a [String],
    allowed: &[&str],
) -> Result<BTreeMap<&'a str, &'a str>, CliError> {
    let mut result = BTreeMap::new();
    let mut pairs = arguments.chunks_exact(2);
    for pair in &mut pairs {
        let name = pair.first().ok_or_else(usage)?.as_str();
        let value = pair.get(1).ok_or_else(usage)?.as_str();
        if !allowed.contains(&name) || value.is_empty() || result.insert(name, value).is_some() {
            return Err(usage());
        }
    }
    if !pairs.remainder().is_empty() || result.len() != allowed.len() {
        return Err(usage());
    }
    Ok(result)
}

fn required<'a>(options: &BTreeMap<&str, &'a str>, name: &str) -> Result<&'a str, CliError> {
    options.get(name).copied().ok_or_else(usage)
}

fn load_bytes(path: &str, requested_bound: usize) -> Result<Vec<u8>, String> {
    let bound = requested_bound.min(MAX_RECEIPT_BYTES);
    if bound == 0 {
        return Err("document bound must be positive".to_string());
    }
    let file =
        File::open(Path::new(path)).map_err(|_| "input document is unavailable".to_string())?;
    let metadata = file
        .metadata()
        .map_err(|_| "input metadata is unavailable".to_string())?;
    let limit = u64::try_from(bound).map_err(|_| "document bound is invalid".to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err("input document is not a bounded regular file".to_string());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input document could not be read".to_string())?;
    if bytes.len() > bound {
        return Err("input document exceeds its byte bound".to_string());
    }
    Ok(bytes)
}

fn load_document<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let bytes = load_bytes(path, MAX_DOCUMENT_BYTES)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "input JSON has invalid fields or syntax".to_string())
}

fn render(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string_pretty(value)
        .map_err(|_| "could not serialize review evidence".to_string())
}

fn usage() -> CliError {
    CliError {
        message: USAGE.to_string(),
        exit_code: EXIT_USAGE,
    }
}

fn port_error(error: PortFailure) -> CliError {
    let exit_code = if matches!(error, PortFailure::Cancelled(_)) {
        EXIT_CANCELLED
    } else {
        EXIT_FAILURE
    };
    let message = match error {
        PortFailure::Domain(error) => format!("{}: {}", error.code, error.message),
        PortFailure::Infrastructure(error)
        | PortFailure::Retryable(error)
        | PortFailure::TimedOut(error)
        | PortFailure::Cancelled(error)
        | PortFailure::Unknown(error) => format!("{}: {}", error.code, error.message),
    };
    CliError { message, exit_code }
}

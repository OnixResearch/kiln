//! Exact read-only native Lattice evidence for one canonical implementation attempt.
//! This does not infer liveness, host health, or served-model/replica lineage from
//! a Fleet placement, OMP usage, a declared model route, or a dispatch intent.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use kiln_adapter_lattice::worker::WorkerTransportProfile;
use kiln_adapter_lattice::worker::inspect::{self, NativeInstance, NativeProfile};
use kiln_core::implementation::admit_dispatch_plan;
use kiln_core::types::ContentId;
use kiln_implementer::observation::ImplementationObservation;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROFILE_SCHEMA: &str = "kiln.native-runtime-evidence-profile.v1";
pub const EVIDENCE_SCHEMA: &str = "kiln.native-runtime-evidence.v1";
const MAX_PROFILE_BYTES: u64 = 4_096;
const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TRANSPORT_PROFILE_BYTES: u64 = 4_096;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEvidenceProfile {
    pub schema: String,
    pub implementation_database: PathBuf,
    /// The campaign producer's retained bound inputs, not a caller-provided artifact path.
    pub bound_input_directory: PathBuf,
    /// The existing v2 worker transport profile owns the qualified Lattice binary and state.
    pub transport_profile: PathBuf,
}

fn bounded_file(path: &Path, bound: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|_| "native runtime input unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "native runtime input unavailable")?;
    if !metadata.is_file()
        || metadata.len() > bound
        || std::fs::canonicalize(path).map_err(|_| "native runtime input unavailable")? != path
    {
        return Err("native runtime input is not a bounded canonical ordinary file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(bound + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native runtime input unreadable")?;
    if bytes.len() as u64 > bound {
        return Err("native runtime input exceeds byte bound".into());
    }
    Ok(bytes)
}

impl RuntimeEvidenceProfile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = bounded_file(path, MAX_PROFILE_BYTES)?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "native runtime profile malformed")?;
        if profile.schema != PROFILE_SCHEMA
            || [
                &profile.implementation_database,
                &profile.bound_input_directory,
                &profile.transport_profile,
            ]
            .iter()
            .any(|path| !path.is_absolute())
        {
            return Err("native runtime profile identity or paths invalid".into());
        }
        let directory = std::fs::symlink_metadata(&profile.bound_input_directory)
            .map_err(|_| "campaign bound inputs unavailable")?;
        if !directory.is_dir()
            || std::fs::canonicalize(&profile.bound_input_directory)
                .map_err(|_| "campaign bound inputs unavailable")?
                != profile.bound_input_directory
        {
            return Err("campaign bound inputs must be a canonical directory".into());
        }
        let transport: WorkerTransportProfile = serde_json::from_slice(&bounded_file(
            &profile.transport_profile,
            MAX_TRANSPORT_PROFILE_BYTES,
        )?)
        .map_err(|_| "native transport profile malformed")?;
        if transport.schema != "kiln.lattice-implementation-cli.v2"
            || !transport.state_directory.is_absolute()
            || !transport.output_directory.is_absolute()
            || !transport.lattice_program.is_absolute()
        {
            return Err("native transport profile is not the approved v2 producer".into());
        }
        Ok(profile)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEvidence {
    pub schema: &'static str,
    pub work_identity: ContentId,
    pub attempt_identity: ContentId,
    pub fence: u64,
    pub request_identity: ContentId,
    pub profile: NativeProfile,
    pub worker: Option<NativeInstance>,
    pub model_requests: inspect::ModelRequests,
    pub agent_lineage: inspect::AgentLineage,
}

fn exact_instance(
    instance: &NativeInstance,
    work: &ContentId,
    attempt: &ContentId,
    fence: u64,
    request: &ContentId,
) -> bool {
    instance.work_id == work.as_str()
        && instance.attempt_id == attempt.as_str()
        && instance.fence == fence
        && instance.incarnation == request.as_str()
}

/// Read one exact canonical owner seal, frozen native request, and native inspect
/// projection. A missing seal or frozen request is an unavailable read, not a
/// substitute receipt. The native producer does not claim live host/replica state.
pub fn read_lattice(
    profile: &RuntimeEvidenceProfile,
    work_identity: &ContentId,
    attempt_identity: &ContentId,
    fence: u64,
) -> Result<RuntimeEvidence, String> {
    if fence == 0 || profile.schema != PROFILE_SCHEMA {
        return Err("native runtime attempt fence or profile invalid".into());
    }
    let (works, attempts) = ImplementationObservation::open(&profile.implementation_database)
        .and_then(|owner| owner.dispatch_recovery_ledgers())
        .map_err(|_| "canonical attempt read unavailable")?;
    let works = works.ok_or("canonical work ledger unavailable")?;
    let attempts = attempts.ok_or("canonical attempt ledger unavailable")?;
    let attempt = attempts
        .get(attempt_identity)
        .ok_or("canonical attempt absent")?;
    if &attempt.work_identity != work_identity || attempt.fence != fence {
        return Err("canonical work/attempt/fence differ".into());
    }
    let work = works
        .current()
        .chain(works.superseded().iter())
        .find(|candidate| candidate.identity().is_ok_and(|id| &id == work_identity))
        .ok_or("canonical attempt work unavailable")?;
    let plan = attempts
        .dispatch_plan(attempt_identity)
        .ok_or("canonical native dispatch plan unavailable")?;
    if plan.execution.is_none() {
        return Err("attempt is not a native worker dispatch".into());
    }
    let plan_id =
        admit_dispatch_plan(work, attempt, plan).map_err(|_| "canonical dispatch plan crossed")?;
    if attempt.dispatch.as_ref().is_none_or(|reservation| {
        reservation.plan != plan_id || reservation.provider_profile != plan.provider_profile
    }) {
        return Err("canonical native dispatch reservation crossed".into());
    }
    let seal = attempt
        .dispatch_request_binding
        .as_ref()
        .ok_or("native request seal unavailable")?;
    let request_path = profile.bound_input_directory.join(format!(
        "campaign-{}.request.json",
        attempt_identity.as_str()
    ));
    let request_bytes = bounded_file(&request_path, MAX_REQUEST_BYTES)?;
    let digest = ContentId::from_blake3_hex(blake3::hash(&request_bytes).to_hex().to_string())
        .map_err(|_| "native request digest invalid")?;
    seal.validate_for(attempt_identity, fence, &plan_id, &digest)
        .map_err(|_| "native request differs from canonical dispatch seal")?;
    let request: Value =
        serde_json::from_slice(&request_bytes).map_err(|_| "native request malformed")?;
    if request.get("schema").and_then(Value::as_str)
        != Some("lattice.implementation-attempt-run.v3")
        || request.pointer("/envelope/work_id").and_then(Value::as_str)
            != Some(work_identity.as_str())
        || request
            .pointer("/envelope/attempt_id")
            .and_then(Value::as_str)
            != Some(attempt_identity.as_str())
        || request.pointer("/envelope/fence").and_then(Value::as_u64) != Some(fence)
        || request
            .pointer("/envelope/worker_profile")
            .and_then(Value::as_str)
            != plan
                .execution
                .as_ref()
                .and_then(|execution| execution.native_request_profile.as_ref())
                .map(ContentId::as_str)
    {
        return Err("native request differs from exact owner attempt generation".into());
    }
    // The guarded worker created this immutable name before native dispatch.
    // It is not a generic observation directory: both owner seal and bytes agree.
    let transport: WorkerTransportProfile = serde_json::from_slice(&bounded_file(
        &profile.transport_profile,
        MAX_TRANSPORT_PROFILE_BYTES,
    )?)
    .map_err(|_| "native transport profile malformed")?;
    let frozen = transport
        .output_directory
        .join(format!("request-{}.json", digest.as_str()));
    if bounded_file(&frozen, MAX_REQUEST_BYTES)? != request_bytes {
        return Err("frozen native request differs from canonical sealed bytes".into());
    }
    let mut inventory = inspect::inspect(&profile.transport_profile, &frozen)?;
    if inventory.profile.revision
        != request
            .pointer("/envelope/worker_profile")
            .and_then(Value::as_str)
            .ok_or("native request profile unavailable")?
    {
        return Err("native profile generation crossed".into());
    }
    if inventory.instance.as_ref().is_some_and(|instance| {
        !exact_instance(instance, work_identity, attempt_identity, fence, &digest)
    }) {
        return Err("native instance crossed the exact owner attempt".into());
    }
    let model_requests = inventory.take_model_requests()?;
    let agent_lineage = inventory.take_agent_lineage();
    let selected_animus = request
        .get("animus_implementation")
        .is_some_and(Value::is_object);
    if selected_animus == matches!(agent_lineage, inspect::AgentLineage::Unsupported) {
        return Err("native harness differs from the sealed request".into());
    }
    Ok(RuntimeEvidence {
        schema: EVIDENCE_SCHEMA,
        work_identity: work_identity.clone(),
        attempt_identity: attempt_identity.clone(),
        fence,
        request_identity: digest,
        profile: inventory.profile,
        worker: inventory.instance,
        model_requests,
        agent_lineage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_refuses_relative_paths_and_unknown_inputs() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime.json");
        std::fs::write(&path, br#"{"schema":"kiln.native-runtime-evidence-profile.v1","implementation_database":"relative","bound_input_directory":"/tmp","transport_profile":"/tmp/t.json"}"#).unwrap();
        assert!(RuntimeEvidenceProfile::load(&path).is_err());
        std::fs::write(&path, br#"{"schema":"kiln.native-runtime-evidence-profile.v1","implementation_database":"/tmp/db","bound_input_directory":"/tmp","transport_profile":"/tmp/t.json","producer":"unbound"}"#).unwrap();
        assert!(RuntimeEvidenceProfile::load(&path).is_err());
    }

    #[test]
    fn profile_loads_only_existing_native_transport_v2() {
        let root = tempfile::tempdir().unwrap();
        let bound = root.path().join("bound");
        std::fs::create_dir(&bound).unwrap();
        let transport = root.path().join("transport.json");
        let profile_path = root.path().join("runtime.json");
        let transport_json = |schema: &str| {
            serde_json::json!({
                "schema": schema,
                "lattice_program":"/nix/store/native-lattice/bin/lattice",
                "executable_blake3":"ab".repeat(32),
                "state_directory":root.path().join("state"),
                "output_directory":root.path().join("output"),
            })
        };
        let write = |schema: &str| {
            std::fs::write(
                &transport,
                serde_json::to_vec(&transport_json(schema)).unwrap(),
            )
            .unwrap();
            std::fs::write(
                &profile_path,
                serde_json::to_vec(&serde_json::json!({
                    "schema": PROFILE_SCHEMA,
                    "implementation_database":root.path().join("owner.sqlite"),
                    "bound_input_directory":bound,
                    "transport_profile":transport,
                }))
                .unwrap(),
            )
            .unwrap();
        };
        write("kiln.lattice-implementation-cli.v2");
        assert!(RuntimeEvidenceProfile::load(&profile_path).is_ok());
        write("kiln.lattice-implementation-cli.v1");
        assert!(RuntimeEvidenceProfile::load(&profile_path).is_err());
    }

    #[test]
    fn worker_instance_requires_exact_work_attempt_fence_and_request_generation() {
        let work = ContentId::from_blake3_hex("ab".repeat(32)).unwrap();
        let attempt = ContentId::from_blake3_hex("cd".repeat(32)).unwrap();
        let request = ContentId::from_blake3_hex("ef".repeat(32)).unwrap();
        let mut instance: NativeInstance = serde_json::from_value(serde_json::json!({
            "schema":"lattice.worker-instance-observation.v3",
            "source":"lattice.implementation-attempt-state.v1",
            "profile_revision":"12".repeat(32),
            "work_id":work, "attempt_id":attempt, "incarnation":request, "fence":7,
            "binding_identity":"34".repeat(32), "definition_identity":format!("blake3:{}", "78".repeat(32)),
            "lifecycle":{"state":"terminal_observed","disposition":"failed"},
            "cancellation":"not_requested", "host_reachability":{"support":"unknown","owner":"kiln#49"},
            "allocation_liveness":{"support":"unknown","owner":"kiln#49"},
            "observed_at_epoch_ms":123, "state_digest":"56".repeat(32)
        })).unwrap();
        assert!(exact_instance(&instance, &work, &attempt, 7, &request));
        assert!(!exact_instance(&instance, &work, &attempt, 8, &request));
        instance.work_id = "00".repeat(32);
        assert!(!exact_instance(&instance, &work, &attempt, 7, &request));
        instance.work_id = work.as_str().to_owned();
        instance.attempt_id = "00".repeat(32);
        assert!(!exact_instance(&instance, &work, &attempt, 7, &request));
        instance.attempt_id = attempt.as_str().to_owned();
        instance.incarnation = "00".repeat(32);
        assert!(!exact_instance(&instance, &work, &attempt, 7, &request));
    }

    #[test]
    fn zero_fence_never_reaches_canonical_owner() {
        let profile = RuntimeEvidenceProfile {
            schema: PROFILE_SCHEMA.into(),
            implementation_database: "/unavailable".into(),
            bound_input_directory: "/unavailable".into(),
            transport_profile: "/unavailable".into(),
        };
        let id = ContentId::from_blake3_hex("ab".repeat(32)).unwrap();
        assert!(read_lattice(&profile, &id, &id, 0).is_err());
    }
}

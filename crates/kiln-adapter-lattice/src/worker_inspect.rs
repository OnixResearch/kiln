//! Read-only Lattice implementation-attempt inspect transport.
//! The native producer owns lifecycle; no request or state path comes from HTTP.
use super::{WorkerTransportProfile, program_identity, read_json};
use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, path::Path};

#[path = "worker_inspect_lineage.rs"]
mod lineage;
pub use lineage::AgentLineage;

const ANIMUS_ADAPTER: &str = "animus-implementation";
const INVENTORY_SCHEMA: &str = "lattice.worker-inventory.v3";
const PROFILE_SCHEMA: &str = "lattice.worker-profile-facts.v3";
const INSTANCE_SCHEMA: &str = "lattice.worker-instance-observation.v3";
const STATE_SOURCE: &str = "lattice.implementation-attempt-state.v1";
const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_INSPECT_OUTPUT_BYTES: usize = 512 * 1024;
const INSPECT_TIMEOUT_MS: u64 = 10_000;
const MAX_NATIVE_PROGRAM_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeInventory {
    pub schema: String,
    pub profile: NativeProfile,
    pub instance: Option<NativeInstance>,
    model_requests: Option<Coverage>,
    agent_lineage: Option<lineage::LineageReceipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NativeProfile {
    pub schema: String,
    pub owner: String,
    pub revision: String,
    #[serde(skip_serializing)]
    adapter: Option<AdapterFact>,
}

#[derive(Clone, Debug, Deserialize)]
struct AdapterFact {
    support: String,
    value: Option<String>,
}

impl NativeInventory {
    /// Release the admitted producer receipt after the caller checks the
    /// canonical Kiln work, attempt, fence and profile generation.
    #[must_use]
    pub fn take_agent_lineage(&mut self) -> AgentLineage {
        match self.agent_lineage.take() {
            Some(receipt) => AgentLineage::Recorded { receipt },
            None if self.is_animus() => AgentLineage::Unobserved,
            None => AgentLineage::Unsupported,
        }
    }

    fn is_animus(&self) -> bool {
        self.profile.adapter.as_ref().is_some_and(|adapter| {
            matches!(adapter.support.as_str(), "declared" | "qualified")
                && adapter.value.as_deref() == Some(ANIMUS_ADAPTER)
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
// Project only retained identity and lifecycle fields; do not forward opaque native metadata.
pub struct NativeInstance {
    pub schema: String,
    pub source: String,
    pub profile_revision: String,
    pub work_id: String,
    pub attempt_id: String,
    pub incarnation: String,
    pub fence: u64,
    pub binding_identity: String,
    pub definition_identity: String,
    pub lifecycle: Value,
    pub cancellation: String,
    pub host_reachability: OwnedUnknown,
    pub allocation_liveness: OwnedUnknown,
    pub observed_at_epoch_ms: i64,
    pub state_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RequestObservation {
    request_id: String,
    class: RequestClass,
    deployment_id: Option<String>,
    deployment_generation: Option<u64>,
    replica: Option<String>,
    evidence: Option<Evidence>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RequestClass {
    NotDispatched,
    Completed,
    ResponseRejected,
    Uncertain,
    DispatchUnconfirmed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Evidence {
    Fixture,
    LiveEndpoint,
    NoUpstreamContact,
}

#[derive(Clone, Debug, Deserialize)]
struct ServiceUsage {
    route_name: String,
    service_id: String,
    model_id: String,
    requests: Vec<RequestObservation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Coverage {
    Unknown,
    NoModel,
    BrokeredUnattributed {
        schema: String,
        is_count_complete: bool,
        dispatched_requests: u64,
    },
    Served {
        schema: String,
        is_complete: bool,
        services: Vec<ServiceUsage>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ModelRequests {
    Unobserved,
    NoModel,
    BrokeredUnattributed {
        count_complete: bool,
        dispatched_requests: u64,
    },
    ServingOwner {
        usage_complete: bool,
        services: Vec<ServingService>,
        omitted: usize,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct ServingService {
    route: String,
    service: String,
    expected_model: String,
    requests: Vec<RequestObservation>,
}

impl NativeInventory {
    /// Project the admitted inspect command's validated owner ledger, not the requested route.
    /// The caller must still join the worker to the exact canonical attempt.
    pub fn take_model_requests(&mut self) -> Result<ModelRequests, String> {
        match self.model_requests.take() {
            None | Some(Coverage::Unknown) => Ok(ModelRequests::Unobserved),
            Some(Coverage::NoModel) => Ok(ModelRequests::NoModel),
            Some(Coverage::BrokeredUnattributed {
                schema,
                is_count_complete,
                dispatched_requests,
            }) if schema == "lattice.brokered-model-request-usage.v1" => {
                Ok(ModelRequests::BrokeredUnattributed {
                    count_complete: is_count_complete,
                    dispatched_requests,
                })
            }
            Some(Coverage::Served {
                schema,
                is_complete,
                services,
            }) if schema == "lattice.model-request-usage.v1" => {
                project_served(is_complete, services)
            }
            _ => Err("unsupported native model request coverage schema".into()),
        }
    }
}

fn opaque(value: &str) -> bool {
    (1..=256).contains(&value.len())
        && value
            .bytes()
            .all(|c| c.is_ascii_graphic() && !b"/\\\"'`<>{}".contains(&c))
}

fn project_served(
    usage_complete: bool,
    services: Vec<ServiceUsage>,
) -> Result<ModelRequests, String> {
    const MAX_DISPLAY_REQUESTS: usize = 32;
    let mut remaining = MAX_DISPLAY_REQUESTS;
    let mut omitted = 0;
    let mut seen = std::collections::BTreeSet::new();
    for service in &services {
        if ![&service.route_name, &service.service_id, &service.model_id]
            .into_iter()
            .all(|v| opaque(v))
        {
            return Err("invalid serving route identity".into());
        }
        for observation in &service.requests {
            if !reference(&observation.request_id)
                || !seen.insert(observation.request_id.as_str())
                || observation
                    .deployment_id
                    .as_deref()
                    .is_some_and(|v| !opaque(v))
                || observation.replica.as_deref().is_some_and(|v| !opaque(v))
                || (observation.class == RequestClass::Completed
                    && (!matches!(
                        observation.evidence,
                        Some(Evidence::Fixture | Evidence::LiveEndpoint)
                    ) || observation.deployment_id.is_none()
                        || observation.deployment_generation.is_none()))
            {
                return Err("invalid serving request observation".into());
            }
        }
    }
    let services = services
        .into_iter()
        .filter_map(|mut service| {
            let kept = remaining.min(service.requests.len());
            omitted += service.requests.len() - kept;
            remaining -= kept;
            service.requests.truncate(kept);
            (kept > 0).then_some(ServingService {
                route: service.route_name,
                service: service.service_id,
                expected_model: service.model_id,
                requests: service.requests,
            })
        })
        .collect();
    Ok(ModelRequests::ServingOwner {
        usage_complete,
        services,
        omitted,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedUnknown {
    support: String,
    owner: String,
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn reference(value: &str) -> bool {
    value.strip_prefix("blake3:").is_some_and(hash)
}

fn admit(inventory: NativeInventory, digest: &str) -> Result<NativeInventory, String> {
    if inventory.schema != INVENTORY_SCHEMA
        || inventory.profile.schema != PROFILE_SCHEMA
        || inventory.profile.owner != "implementation_attempt"
        || !hash(&inventory.profile.revision)
    {
        return Err("unsupported native Lattice inventory profile".into());
    }
    if let Some(instance) = &inventory.instance {
        if instance.schema != INSTANCE_SCHEMA
            || instance.source != STATE_SOURCE
            || instance.profile_revision != inventory.profile.revision
            || instance.incarnation != digest
            || !reference(&instance.binding_identity)
            || !reference(&instance.definition_identity)
            || !reference(&instance.state_digest)
            || instance.fence == 0
            || instance.observed_at_epoch_ms < 0
            || !matches!(
                instance.lifecycle.get("state").and_then(Value::as_str),
                Some(
                    "admitted_not_dispatched"
                        | "binding_rejected_before_dispatch"
                        | "revision_blocked_before_dispatch"
                        | "dispatch_recorded_outcome_unknown"
                        | "terminal_observed"
                )
            )
            || ![
                "not_requested",
                "requested_unobserved",
                "observed_terminated",
            ]
            .contains(&instance.cancellation.as_str())
            || ![&instance.host_reachability, &instance.allocation_liveness]
                .iter()
                .all(|fact| fact.support == "unknown" && fact.owner == "kiln#49")
        {
            return Err("native Lattice instance is crossed or unsupported".into());
        }
    }
    if let Some(receipt) = &inventory.agent_lineage
        && (!inventory.is_animus()
            || !inventory
                .instance
                .as_ref()
                .is_some_and(|instance| receipt.matches(instance, digest)))
    {
        return Err("native agent lineage is crossed or unsupported".into());
    }
    // A closed, never-dispatched attempt can have a binding but no instance.
    // Without the exact instance join, do not disclose any usage attribution.
    let mut inventory = inventory;
    if inventory.instance.is_none() {
        inventory.model_requests = None;
    }
    Ok(inventory)
}

fn frozen_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|_| "frozen Lattice request unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "frozen Lattice request unavailable")?;
    if !metadata.is_file() || metadata.len() > MAX_REQUEST_BYTES {
        return Err("frozen Lattice request exceeds its file bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "frozen Lattice request unreadable")?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err("frozen Lattice request exceeds its byte bound".into());
    }
    Ok(bytes)
}

/// Inspect a native frozen request produced by the guarded implementation worker.
/// The request must already reside at Lattice's content-addressed frozen path;
/// this command never prepares, executes, recovers or modifies the producer.
pub fn inspect(profile_path: &Path, request_path: &Path) -> Result<NativeInventory, String> {
    let profile: WorkerTransportProfile = read_json(profile_path)?;
    let executable = std::fs::metadata(&profile.lattice_program)
        .map_err(|_| "native Lattice executable unavailable")?;
    if !executable.is_file() || executable.len() > MAX_NATIVE_PROGRAM_BYTES {
        return Err("native Lattice executable exceeds its bound".into());
    }
    if profile.schema != "kiln.lattice-implementation-cli.v2"
        || !profile.state_directory.is_absolute()
        || !profile.output_directory.is_absolute()
        || program_identity(&profile.lattice_program)? != profile.executable_blake3
    {
        return Err("Lattice inspect profile or executable changed".into());
    }
    let request = frozen_bytes(request_path)?;
    let digest = blake3::hash(&request).to_hex().to_string();
    let expected = profile
        .output_directory
        .join(format!("request-{digest}.json"));
    if request_path != expected
        || std::fs::canonicalize(request_path).map_err(|_| "frozen Lattice request unavailable")?
            != expected
    {
        return Err("Lattice request is not its exact frozen producer artifact".into());
    }
    for directory in [&profile.state_directory, &profile.output_directory] {
        let meta = std::fs::symlink_metadata(directory)
            .map_err(|_| "native Lattice directory unavailable")?;
        if !meta.is_dir()
            || std::fs::canonicalize(directory)
                .map_err(|_| "native Lattice directory unavailable")?
                != *directory
        {
            return Err("native Lattice directory is not canonical".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("native Lattice directory is not private".into());
            }
        }
    }
    let output = bounded_exec::run(RunRequest {
        command: CommandSpec {
            program: profile.lattice_program,
            args: vec![
                "implementation-attempt".into(),
                "inspect".into(),
                "--request".into(),
                request_path.as_os_str().into(),
                "--state".into(),
                profile.state_directory.as_os_str().into(),
            ],
            current_dir: profile.output_directory,
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: INSPECT_TIMEOUT_MS,
            stdin_max_bytes: 1,
            stdout_max_bytes: MAX_INSPECT_OUTPUT_BYTES,
            stderr_max_bytes: 16 * 1024,
            poll_interval_ms: 10,
            teardown_timeout_ms: 1_000,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true)
            .map_err(|_| "Lattice inspect policy unavailable")?,
    })
    .map_err(|_| "Lattice inspect unavailable")?;
    if output.disposition != Disposition::Succeeded {
        return Err("Lattice inspect did not produce verified native facts".into());
    }
    let after = frozen_bytes(request_path).map_err(|_| "frozen Lattice request changed")?;
    if after != request {
        return Err("frozen Lattice request changed during inspect".into());
    }
    let decoded = serde_json::from_slice(&output.stdout.bytes)
        .map_err(|_| "Lattice inspect output malformed")?;
    admit(decoded, &digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_inventory_requires_own_schema_generation_and_incarnation() {
        let digest = "ab".repeat(32);
        let profile = json!({"schema": PROFILE_SCHEMA, "owner":"implementation_attempt", "revision": "cd".repeat(32)});
        let inventory: NativeInventory = serde_json::from_value(json!({"schema": INVENTORY_SCHEMA, "profile": profile, "instance": null, "model_requests": null})).unwrap();
        assert!(admit(inventory.clone(), &digest).is_ok());
        let mut wrong = inventory.clone();
        wrong.profile.revision = "stale".into();
        assert!(admit(wrong, &digest).is_err());
        let mut wrong = inventory;
        wrong.schema = "other".into();
        assert!(admit(wrong, &digest).is_err());
    }
    #[test]
    fn instance_rejects_crossed_incarnation_profile_and_false_readiness() {
        let digest = "ab".repeat(32);
        let profile = "cd".repeat(32);
        let sample = serde_json::json!({
            "schema": INVENTORY_SCHEMA,
            "model_requests": null,
            "profile": {"schema": PROFILE_SCHEMA, "owner":"implementation_attempt", "revision": profile},
            "instance": {"schema": INSTANCE_SCHEMA, "source": STATE_SOURCE,
                "profile_revision": profile, "work_id":"ef".repeat(32), "attempt_id":"12".repeat(32),
                "incarnation": digest, "fence": 7, "binding_identity":format!("blake3:{}", "34".repeat(32)),
                "definition_identity":format!("blake3:{}", "78".repeat(32)),
                "compute":{}, "model_routes":{}, "lifecycle":{"state":"terminal_observed","disposition":"failed"},
                "cancellation":"not_requested", "host_reachability":{"support":"unknown","owner":"kiln#49"},
                "model_readiness":{"support":"unknown","owner":"animus#17"},
                "allocation_liveness":{"support":"unknown","owner":"kiln#49"},
                "observed_at_epoch_ms": 123, "state_digest":format!("blake3:{}", "56".repeat(32))}
        });
        let parse = |v| serde_json::from_value::<NativeInventory>(v).unwrap();
        assert!(admit(parse(sample.clone()), &digest).is_ok());
        let mut crossed = sample.clone();
        crossed["instance"]["incarnation"] = serde_json::json!("00".repeat(32));
        assert!(admit(parse(crossed), &digest).is_err());
        let mut crossed = sample.clone();
        crossed["instance"]["profile_revision"] = serde_json::json!("00".repeat(32));
        assert!(admit(parse(crossed), &digest).is_err());
        let mut animus = sample.clone();
        animus["profile"]["adapter"] = json!({"support":"declared", "value":ANIMUS_ADAPTER});
        let mut unobserved = admit(parse(animus.clone()), &digest).unwrap();
        assert!(matches!(
            unobserved.take_agent_lineage(),
            AgentLineage::Unobserved
        ));
        let root = "9a".repeat(32);
        animus["agent_lineage"] = json!({
            "schema":"animus.implementation-child-lineage.v1", "work_identity":"ef".repeat(32),
            "attempt_identity":"12".repeat(32), "fence":7, "request_identity":digest,
            "worker_incarnation":digest, "profile_revision":profile, "root_id":root, "coverage":"complete",
            "edges":[{"parent_id":root, "child_id":"bc".repeat(32), "state":"completed"}],
        });
        let mut admitted = admit(parse(animus.clone()), &digest).unwrap();
        assert!(matches!(
            admitted.take_agent_lineage(),
            AgentLineage::Recorded { .. }
        ));
        for (path, value) in [
            ("/agent_lineage/fence", json!(8)),
            ("/agent_lineage/request_identity", json!("01".repeat(32))),
            ("/agent_lineage/edges/0/parent_id", json!("01".repeat(32))),
            ("/profile/adapter/value", json!("omp-jsonl")),
        ] {
            let mut crossed = animus.clone();
            *crossed.pointer_mut(path).unwrap() = value;
            assert!(admit(parse(crossed), &digest).is_err(), "accepted {path}");
        }
        let mut false_readiness = sample;
        false_readiness["instance"]["host_reachability"]["support"] =
            serde_json::json!("qualified");
        assert!(admit(parse(false_readiness), &digest).is_err());
    }

    #[test]
    fn model_evidence_requires_completed_response_and_preserves_admission_attribution() {
        let id = |part: &str| format!("blake3:{}", part.repeat(32));
        let mut rows = json!([{
            "route_name":"primary", "service_id":"svc", "model_id":"model-a", "requests":[
                {"request_id":id("ab"), "class":"completed", "deployment_id":"dep",
                 "deployment_generation":17, "replica":"rep-a", "evidence":"fixture", "private":"not exported"},
                {"request_id":id("cd"), "class":"dispatch_unconfirmed", "deployment_id":"dep",
                 "deployment_generation":17, "replica":null, "evidence":null}
            ]
        }]);
        let project = |rows| project_served(false, serde_json::from_value(rows).unwrap());
        let view = serde_json::to_value(project(rows.clone()).unwrap()).unwrap();
        let service = &view["services"][0];
        assert_eq!(service["expected_model"], "model-a");
        assert_eq!(service["requests"][0]["class"], "completed");
        assert_eq!(service["requests"][0]["evidence"], "fixture");
        assert_eq!(service["requests"][0]["replica"], "rep-a");
        assert!(service["requests"][0].get("private").is_none());
        assert_eq!(service["requests"][1]["class"], "dispatch_unconfirmed");
        rows[0]["requests"][0]["evidence"] = json!("no_upstream_contact");
        assert!(project(rows).is_err());
    }
}

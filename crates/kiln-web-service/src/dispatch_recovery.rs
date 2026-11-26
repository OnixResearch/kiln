//! Operator-pinned recovery of an original Lattice dispatch, never a replacement.
//!
//! The browser supplies only the attempt/fence and the reviewed request digest.
//! Request bytes, transport, owner authority and database are loaded once from
//! the protected startup profile; no URL or POST field selects a route.
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use kiln_adapter_lattice::{
    UnixWorkflowExchange, admit_implementation_request, recover_implementation_request,
};
use kiln_core::implementation::{
    AttemptLedger, AttemptPhase, Enrollment, ImplementationAttempt, WorkLedger, admit_dispatch_plan,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::observation::ImplementationObservation;
use kiln_implementer::{Implementer, current_unix_ms};
use kiln_provider_api::inventory::InventoryReadFailure;
use lattice_workflow_contract::{CapabilityProfile, WorkflowRequest};
use lattice_workflow_transport::TransportProfile;
use serde::{Deserialize, Serialize};

use crate::engine::{ApiResponse, Role};

pub const ROUTE: &str = "/api/implementation/dispatch-recovery";
const SCHEMA: &str = "kiln.dispatch-recovery-observation.v1";
const RESULT_SCHEMA: &str = "kiln.dispatch-recovery-result.v1";
const PROFILE_SCHEMA: &str = "kiln.dispatch-recovery-profile.v1";
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_PROFILE_BYTES: u64 = 1024 * 1024;
const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_POST_BYTES: usize = 256;
const NON_CLAIMS: [&str; 3] = [
    "recovery only queries the original Lattice request and never dispatches a replacement",
    "a persisted transport intent is not proof of execution, termination or eligibility to retry",
    "a transport acknowledgement is not proof of workflow termination or candidate completion",
];

/// A closed operator profile. Paths and credentials never come from an HTTP request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryProfile {
    schema: String,
    principal: String,
    local_operator: String,
    database: PathBuf,
    enrollment: Enrollment,
    request: PathBuf,
    profile: CapabilityProfile,
    transport_profile: TransportProfile,
    socket_path: PathBuf,
    maximum_records: usize,
}

/// Startup-validated, operator-retained exact request and route.
pub struct RecoveryConfig {
    profile: RecoveryProfile,
    request: WorkflowRequest,
    request_bytes: Vec<u8>,
    request_digest: ContentId,
}

impl RecoveryConfig {
    /// Load the protected profile and snapshot the exact request bytes at startup.
    /// The digest must match the durable seal at the point of each recovery.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !absolute(path) {
            return Err("recovery profile path must be absolute".into());
        }
        let bytes = read_bounded(path, MAX_PROFILE_BYTES)?;
        let profile: RecoveryProfile =
            serde_json::from_slice(&bytes).map_err(|_| "recovery profile is malformed")?;
        if profile.schema != PROFILE_SCHEMA
            || profile.principal.is_empty()
            || profile.principal.len() > 128
            || profile.local_operator.is_empty()
            || !absolute(&profile.database)
            || !absolute(&profile.request)
            || !absolute(&profile.socket_path)
        {
            return Err("recovery profile scope is invalid".into());
        }
        // The original implementation adapter fixes a 4 MiB response bound;
        // reject profiles that cannot perform the operation GET advertises.
        if profile.transport_profile.limits.maximum_frame_bytes < 4 * 1024 * 1024 {
            return Err("recovery transport cannot carry the adapter response bound".into());
        }
        kiln_core::implementation::authority::LocalOperatorAuthority::new(
            profile.local_operator.clone(),
            false,
        )
        .map_err(|_| "recovery local operator is invalid")?;
        UnixWorkflowExchange::new(
            profile.socket_path.clone(),
            profile.transport_profile.clone(),
            profile.profile.clone(),
            profile.maximum_records,
        )
        .map_err(|_| "recovery transport profile is invalid")?;
        let request_bytes = read_bounded(&profile.request, MAX_REQUEST_BYTES)?;
        if request_bytes.is_empty() {
            return Err("recovery request is empty".into());
        }
        let request =
            serde_json::from_slice(&request_bytes).map_err(|_| "recovery request is malformed")?;
        // The owner seals the file bytes; the adapter sends the parsed request's
        // canonical encoding. Both are retained from this protected input.
        let request_digest =
            ContentId::from_blake3_hex(blake3::hash(&request_bytes).to_hex().to_string())
                .map_err(|_| "recovery request digest is invalid")?;
        Ok(Self {
            profile,
            request,
            request_bytes,
            request_digest,
        })
    }
}

fn absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|_| "recovery input unavailable")?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "recovery input unavailable")?;
    if bytes.len() as u64 > limit {
        return Err("recovery input exceeds byte bound".into());
    }
    Ok(bytes)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryCommand {
    request_digest: ContentId,
}
fn pinned_request_admitted(
    config: &RecoveryConfig,
    database: &Path,
    work: Option<&WorkLedger>,
    ledger: &AttemptLedger,
    attempt: &ImplementationAttempt,
) -> bool {
    if config.request_bytes.is_empty() || database != config.profile.database.as_path() {
        return false;
    }
    let Some(binding) = attempt.dispatch_request_binding.as_ref() else {
        return false;
    };
    let Some(reservation) = attempt.dispatch.as_ref() else {
        return false;
    };
    let Some(plan) = ledger
        .dispatch_plan(&attempt.identity)
        .filter(|plan| plan.execution.is_none())
    else {
        return false;
    };
    let Some(work) = work.and_then(|work| {
        work.current().chain(work.superseded().iter()).find(|work| {
            work.identity()
                .is_ok_and(|identity| identity == attempt.work_identity)
        })
    }) else {
        return false;
    };
    binding.request_digest == config.request_digest
        && binding
            .validate_for(
                &attempt.identity,
                attempt.fence,
                &reservation.plan,
                &config.request_digest,
            )
            .is_ok()
        && admit_dispatch_plan(work, attempt, plan)
            .is_ok_and(|identity| identity == reservation.plan)
        && plan.provider_profile == reservation.provider_profile
        && admit_implementation_request(plan, reservation, attempt, &config.request).is_ok()
        && lattice_workflow_contract::validate_request(&config.profile.profile, &config.request)
            .is_ok()
}

fn recovery_admitted(
    config: &RecoveryConfig,
    database: &Path,
    work: Option<&WorkLedger>,
    ledger: &AttemptLedger,
    attempt: &ImplementationAttempt,
    now_ms: u64,
) -> bool {
    pinned_request_admitted(config, database, work, ledger, attempt)
        && ledger.is_current(&attempt.identity, attempt.fence, now_ms)
        && matches!(
            attempt.phase,
            AttemptPhase::Claimed | AttemptPhase::Executing
        )
        && attempt.workflow.is_none()
        && ledger.dispatch_requires_recovery(&attempt.identity)
}

fn exact_query(target: &str) -> Option<(ContentId, u64)> {
    let query = target.strip_prefix(ROUTE)?.strip_prefix('?')?;
    let (attempt, fence) = query.strip_prefix("attempt=")?.split_once("&fence=")?;
    if attempt.len() != blake3::OUT_LEN * 2
        || !attempt
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || fence.is_empty()
        || fence.starts_with('0')
        || !fence.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((
        ContentId::from_blake3_hex(attempt.to_owned()).ok()?,
        fence.parse::<u64>().ok().filter(|fence| *fence > 0)?,
    ))
}

fn error(status: u16, code: &'static str) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: serde_json::json!({"code":code,"message":"Request could not be admitted"})
            .to_string()
            .into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

fn observation(
    work: Option<&WorkLedger>,
    ledger: &AttemptLedger,
    attempt: &ImplementationAttempt,
) -> Result<serde_json::Value, &'static str> {
    let started = ledger.dispatch_requires_recovery(&attempt.identity);
    let dispatch = attempt.dispatch.as_ref();
    let binding = attempt.dispatch_request_binding.as_ref();
    let plan = ledger.dispatch_plan(&attempt.identity);
    if let Some(binding) = binding {
        let reserved = dispatch.ok_or("dispatch_recovery_malformed")?;
        binding
            .validate_for(
                &attempt.identity,
                attempt.fence,
                &reserved.plan,
                &binding.request_digest,
            )
            .map_err(|_| "dispatch_recovery_malformed")?;
    }
    if let Some(plan) = plan {
        let work = work
            .into_iter()
            .flat_map(|ledger| ledger.current().chain(ledger.superseded().iter()))
            .find(|work| {
                work.identity()
                    .is_ok_and(|identity| identity == attempt.work_identity)
            })
            .ok_or("dispatch_recovery_malformed")?;
        let digest =
            admit_dispatch_plan(work, attempt, plan).map_err(|_| "dispatch_recovery_malformed")?;
        if dispatch.is_none_or(|reserved| {
            reserved.plan != digest || reserved.provider_profile != plan.provider_profile
        }) {
            return Err("dispatch_recovery_malformed");
        }
    }
    let transport_route = match plan {
        Some(plan) if plan.execution.is_none() => "original_lattice",
        Some(_) => "native_worker",
        None => "unknown",
    };
    let exact_seal = binding.is_some() && transport_route == "original_lattice";
    let recovery_status = if transport_route == "native_worker" {
        "different_transport_route"
    } else if attempt.phase.is_terminal() {
        "terminal_attempt"
    } else if attempt.workflow.is_some() {
        "workflow_observed"
    } else if !started {
        "no_transport_intent"
    } else if !exact_seal {
        "missing_exact_request_seal"
    } else {
        "retained_transport_intent"
    };
    let transport_state = if started {
        "started"
    } else if dispatch.is_some() {
        "reserved"
    } else {
        "not_reserved"
    };
    Ok(serde_json::json!({
        "schema": SCHEMA,
        "attempt": attempt.identity,
        "fence": attempt.fence,
        "work_identity": attempt.work_identity,
        "phase": attempt.phase,
        "lease_until_ms": attempt.lease_until_ms,
        "transport_state": transport_state,
        "transport_route": transport_route,
        "request_binding": binding.as_ref().map(|binding| serde_json::json!({
            "plan": binding.plan,
            "request_digest": binding.request_digest,
            "seal": binding.seal,
        })),
        "workflow": attempt.workflow,
        "recovery_status": recovery_status,
        "non_claims": NON_CLAIMS,
    }))
}

/// HTTPS shell must authenticate and same-origin-admit the request first. The
/// configured principal is a second, exact grant; operator role alone is not
/// authority to use another operator's pinned Lattice request.
pub fn response(
    database: Option<&Path>,
    config: Option<&RecoveryConfig>,
    method: &str,
    target: &str,
    role: Role,
    principal: &str,
    post_body: &[u8],
) -> ApiResponse {
    if role != Role::Operator {
        return error(403, "dispatch_recovery_forbidden");
    }
    if method != "GET" && method != "POST" {
        return error(405, "dispatch_recovery_method_invalid");
    }
    if config.is_some_and(|config| config.profile.principal != principal) {
        return error(403, "dispatch_recovery_forbidden");
    }
    let Some((identity, fence)) = exact_query(target) else {
        return error(400, "dispatch_recovery_query_invalid");
    };
    let Some(database) = database else {
        return error(503, "dispatch_recovery_input_not_configured");
    };
    if config.is_some_and(|config| config.profile.database.as_path() != database) {
        return error(503, "dispatch_recovery_database_mismatch");
    }
    let command = if method == "POST" {
        if config.is_none() {
            return error(503, "dispatch_recovery_input_not_configured");
        }
        if post_body.is_empty() || post_body.len() > MAX_POST_BYTES {
            return error(400, "dispatch_recovery_command_invalid");
        }
        match serde_json::from_slice::<RecoveryCommand>(post_body) {
            Ok(command) => Some(command),
            Err(_) => return error(400, "dispatch_recovery_command_invalid"),
        }
    } else {
        None
    };
    let (work, ledger) = match ImplementationObservation::open(database)
        .and_then(|observation| observation.dispatch_recovery_ledgers())
    {
        Ok((work, Some(ledger))) => (work, ledger),
        Ok((_, None)) => return error(404, "dispatch_recovery_attempt_not_found"),
        Err(InventoryReadFailure::MalformedObservation) => {
            return error(503, "dispatch_recovery_malformed");
        }
        Err(_) => return error(503, "dispatch_recovery_unavailable"),
    };
    let Some(attempt) = ledger
        .get(&identity)
        .filter(|attempt| attempt.fence == fence)
    else {
        return error(404, "dispatch_recovery_attempt_not_found");
    };
    let admitted = config.is_some_and(|config| {
        recovery_admitted(
            config,
            database,
            work.as_ref(),
            &ledger,
            attempt,
            current_unix_ms(),
        ) && Implementer::open_read_only(database, config.profile.enrollment.clone())
            .and_then(|mut owner| owner.dispatch_record(&identity))
            .is_ok_and(|record| {
                record.is_some_and(|record| {
                    record.transport_started
                        && record.launch.is_none()
                        && record.request.is_none()
                        && record.attempt == *attempt
                })
            })
    });
    if let Some(command) = command {
        let Some(config) = config else {
            return error(503, "dispatch_recovery_input_not_configured");
        };
        if !admitted || command.request_digest != config.request_digest {
            return error(409, "dispatch_recovery_not_admitted");
        }
        let Some(reservation) = attempt.dispatch.as_ref() else {
            return error(409, "dispatch_recovery_not_admitted");
        };
        return recover_original(
            config,
            database,
            &identity,
            fence,
            &attempt.work_identity,
            &reservation.plan,
        );
    }
    let mut value = match observation(work.as_ref(), &ledger, attempt) {
        Ok(value) => value,
        Err(code) => return error(503, code),
    };
    value["can_recover"] = serde_json::json!(admitted);
    if admitted {
        value["request_digest"] =
            serde_json::json!(config.expect("admitted configuration").request_digest);
    }
    // A sealed original plan remains visible after provider acknowledgment or
    // lease expiry; this does not imply recovery is still actionable.
    if config.is_some_and(|config| {
        pinned_request_admitted(config, database, work.as_ref(), &ledger, attempt)
    }) {
        // This is the pinned requested plan, not observed execution. Never
        // expose adapter operation identities, arguments, or request payloads.
        if let Some(config) = config {
            let graph = serde_json::json!({
                "nodes": config.request.graph.nodes.iter().map(|node| serde_json::json!({
                    "id": node.id, "kind": node.kind,
                })).collect::<Vec<_>>(),
                "edges": config.request.graph.edges.iter().map(|edge| serde_json::json!({
                    "source": edge.source, "target": edge.target,
                })).collect::<Vec<_>>(),
            });
            if serde_json::to_vec(&graph).is_ok_and(|bytes| bytes.len() <= MAX_RESPONSE_BYTES / 2) {
                value["planned_graph"] = graph;
                value["planned_graph_source"] = serde_json::json!("operator_pinned_request");
            } else {
                value["planned_graph_status"] = serde_json::json!("exceeds_response_bound");
            }
        }
    }
    let body = match serde_json::to_vec(&value) {
        Ok(body) if body.len() <= MAX_RESPONSE_BYTES => body,
        Ok(_) => return error(503, "dispatch_recovery_byte_bound"),
        Err(_) => return error(503, "dispatch_recovery_malformed"),
    };
    ApiResponse {
        status: 200,
        content_type: "application/json",
        body,
        download_name: None,
        headers: Vec::new(),
    }
}

fn unknown(identity: &ContentId, fence: u64, work: &ContentId) -> ApiResponse {
    json_response(
        503,
        serde_json::json!({
            "schema": RESULT_SCHEMA, "attempt": identity, "fence": fence,
            "work_identity": work, "outcome": "unknown",
            "code": "dispatch_recovery_unknown",
        }),
    )
}

fn json_response(status: u16, value: serde_json::Value) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: value.to_string().into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

fn recover_original(
    config: &RecoveryConfig,
    database: &Path,
    identity: &ContentId,
    fence: u64,
    work: &ContentId,
    plan: &ContentId,
) -> ApiResponse {
    let mut owner = match Implementer::open(database, config.profile.enrollment.clone()) {
        Ok(owner) => owner,
        Err(_) => return unknown(identity, fence, work),
    };
    if owner
        .select_local_operator(config.profile.local_operator.clone())
        .is_err()
    {
        return unknown(identity, fence, work);
    }
    // This profile recovers legacy socket dispatches only. Other launch
    // cohorts require their own recorded launch and frozen-request authority.
    if owner
        .authorize_current(identity, fence, current_unix_ms())
        .is_err()
        || !owner.dispatch_record(identity).is_ok_and(|record| {
            record.is_some_and(|record| {
                record.transport_started
                    && record.launch.is_none()
                    && record.request.is_none()
                    && record.attempt.fence == fence
                    && &record.attempt.work_identity == work
                    && record
                        .attempt
                        .dispatch
                        .as_ref()
                        .is_some_and(|reservation| &reservation.plan == plan)
                    && record
                        .attempt
                        .dispatch_request_binding
                        .as_ref()
                        .is_some_and(|seal| {
                            seal.validate_for(identity, fence, plan, &config.request_digest)
                                .is_ok()
                        })
                    && record.attempt.workflow.is_none()
            })
        })
    {
        return error(409, "dispatch_recovery_not_admitted");
    }
    let mut transport = match UnixWorkflowExchange::new(
        config.profile.socket_path.clone(),
        config.profile.transport_profile.clone(),
        config.profile.profile.clone(),
        config.profile.maximum_records,
    ) {
        Ok(transport) => transport,
        Err(_) => return unknown(identity, fence, work),
    };
    let accepted = match recover_implementation_request(
        &mut transport,
        &config.profile.profile,
        &config.request,
    ) {
        Ok(accepted) => accepted,
        Err(_) => return unknown(identity, fence, work),
    };
    let workflow =
        match ExternalRef::new("lattice", "workflow", accepted.execution_identity.as_str()) {
            Ok(workflow) => workflow,
            Err(_) => return unknown(identity, fence, work),
        };
    if owner
        .start_execution(identity, fence, current_unix_ms(), &workflow)
        .is_err()
        || owner
            .admission_execution_observed(identity, fence, current_unix_ms())
            .is_err()
    {
        return unknown(identity, fence, work);
    }
    json_response(
        200,
        serde_json::json!({
            "schema": RESULT_SCHEMA, "attempt": identity, "fence": fence,
            "work_identity": work, "outcome": "accepted", "workflow": workflow,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_operator_only_read_admission() {
        let id = "ab".repeat(32);
        let target = format!("{ROUTE}?attempt={id}&fence=2");
        assert_eq!(exact_query(&target).map(|(_, fence)| fence), Some(2));
        for suffix in [
            format!("attempt={id}&fence=0"),
            format!("attempt={id}&fence=02"),
            format!("attempt={id}&fence=2&extra=x"),
            format!("fence=2&attempt={id}"),
            format!("attempt={}&fence=2", id.to_uppercase()),
        ] {
            assert!(exact_query(&format!("{ROUTE}?{suffix}")).is_none());
        }
        assert_eq!(
            response(None, None, "GET", &target, Role::Worker, "owner", &[]).status,
            403
        );
        assert_eq!(
            response(None, None, "GET", &target, Role::Viewer, "owner", &[]).status,
            403
        );
        assert_eq!(
            response(None, None, "PUT", &target, Role::Operator, "owner", &[]).status,
            405
        );
        assert_eq!(
            response(None, None, "POST", &target, Role::Operator, "owner", &[]).status,
            503
        );
        assert_eq!(
            response(None, None, "GET", &target, Role::Operator, "owner", &[]).status,
            503
        );
        assert_eq!(
            response(
                None,
                None,
                "GET",
                "/api/implementation/dispatch-recovery?attempt=bad",
                Role::Operator,
                "owner",
                &[]
            )
            .status,
            400
        );
    }

    #[test]
    fn started_transport_without_seal_is_never_reported_recoverable() {
        let identity = ContentId::from_blake3_hex("ab".repeat(32)).unwrap();
        let work = ContentId::from_blake3_hex("cd".repeat(32)).unwrap();
        let attempt: ImplementationAttempt = serde_json::from_value(serde_json::json!({
            "schema": "kiln.implementation-attempt.v2",
            "identity": identity,
            "canonical_key": work,
            "work_identity": work,
            "work_revision": 1,
            "ordinal": 1,
            "fence": 2,
            "lease_until_ms": 1000,
            "phase": "claimed",
            "workflow": null,
            "candidate": null,
            "failure": null,
            "cause": null,
            "superseded_by": null
        }))
        .unwrap();
        let mut stored = serde_json::to_value(AttemptLedger::default()).unwrap();
        stored["attempts"][identity.as_str()] = serde_json::to_value(&attempt).unwrap();
        stored["dispatch_started"] = serde_json::json!([identity]);
        let ledger: AttemptLedger = serde_json::from_value(stored).unwrap();
        let result = observation(None, &ledger, &attempt).unwrap();
        assert_eq!(result["transport_state"], "started");
        assert_eq!(result["recovery_status"], "missing_exact_request_seal");
        assert!(result["workflow"].is_null());
    }

    #[test]
    fn unknown_never_claims_a_missing_execution_or_termination() {
        let attempt = ContentId::from_blake3_hex("ab".repeat(32)).unwrap();
        let work = ContentId::from_blake3_hex("cd".repeat(32)).unwrap();
        let reply = unknown(&attempt, 2, &work);
        assert_eq!(reply.status, 503);
        let result: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
        assert_eq!(result["outcome"], "unknown");
        assert_eq!(result["attempt"], attempt.as_str());
        assert_eq!(result["work_identity"], work.as_str());
        assert!(result.get("workflow").is_none());
    }

    #[test]
    fn recovery_command_requires_one_digest_and_no_route_inputs() {
        let digest = "ab".repeat(32);
        let command = format!("{{\"request_digest\":\"{digest}\"}}");
        assert_eq!(
            serde_json::from_str::<RecoveryCommand>(&command)
                .unwrap()
                .request_digest
                .as_str(),
            digest.as_str()
        );
        for extra in ["socket_path", "request", "executable", "attempt", "fence"] {
            let command = format!("{{\"request_digest\":\"{digest}\",\"{extra}\":\"/tmp/other\"}}");
            assert!(serde_json::from_str::<RecoveryCommand>(&command).is_err());
        }
        assert!(serde_json::from_str::<RecoveryCommand>("{}").is_err());
    }

    #[test]
    fn startup_inputs_are_bounded_and_do_not_accept_relative_routes() {
        assert!(!absolute(Path::new("relative.json")));
        assert!(!absolute(Path::new("/trusted/../socket")));
        assert!(absolute(Path::new("/trusted/socket")));
        let input = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(input.path(), b"12345").unwrap();
        assert!(read_bounded(input.path(), 4).is_err());
        assert_eq!(read_bounded(input.path(), 5).unwrap(), b"12345");
    }
}

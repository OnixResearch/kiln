//! Fleet API v1: routing and handlers.
//!
//! Every request is first bound to an authenticated machine (see
//! [`crate::identity`]); handlers then act as that machine. Bodies are
//! bounded JSON with unknown fields refused. Store refusals map to statuses
//! through [`crate::store::classify`]. Returned text written by agents is
//! data, never instructions.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use kiln_adapter_herdr::client::{MAX_PROMPT_BYTES, codes};
use kiln_adapter_herdr::{FailureClass, HerdrClient, HerdrFailure, HerdrInventory, MachineLoad};
use kiln_core::fleet::{
    BoardPostKind, ClaimCommand, ClaimOutcome, DEFAULT_STALE_AFTER_MS, FleetAgent, FleetSession,
    ForeignClaim, MentionTarget, Observation, QuotaFailoverRequest, SessionState,
    SpawnMemberRequest, SwarmEnvelope, SwarmMemberState, SwarmRole, SwarmRunConfig,
    VerdictDecision, parse_scope_text,
};
use kiln_core::implementation::Eligibility;
use kiln_core::types::ContentId;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::config::{FleetConfig, fleet_console_target, machine_label, validate_console_url};
use crate::controls::{
    ControlStore, FleetControls, MAX_EXPIRED_CLAIMS, MAX_PANE_TEXT_BYTES, RequestIdConflict,
    TickSummary, parse_claims_query, parse_pane_lines_query, payload_hash, project_foreign_claim,
    project_inventory_machine, project_machine_slots, sanitize_pane_output, select_expired_claims,
    validate_reason, validate_request_id,
};
use crate::http::{Request, Response};
use crate::identity::PeerIdentity;
use crate::log;
use crate::store::{Store, classify, error_code};
use crate::swarm::SwarmLink;

pub const API_VERSION: &str = "v1";
pub const MAX_LISTED_WORK: usize = 100;
pub const MAX_LISTED_ATTEMPTS: usize = 256;
pub const MAX_LISTED_DECISIONS: usize = 256;
pub const MS_PER_MINUTE: u64 = 60_000;
/// Handoffs name a full SHA-1 commit, the only form the herdr receipt admits.
pub const HANDOFF_COMMIT_HEX: usize = 40;
/// A probe expires after three dispatch intervals without a new observation.
const MACHINE_STALE_TICKS: u64 = 3;
// Core sends heartbeats every fifteen seconds; missing two expires the view.
const CORE_STALE_MS: u64 = 30_000;

#[derive(Debug, Default)]
struct CoreObservations {
    latest: BTreeMap<String, crate::core_observer::CoreHost>,
    observed_at_ms: Option<u64>,
    last_seen: BTreeMap<String, u64>,
    invalidated: bool,
}

impl CoreObservations {
    fn update(&mut self, incoming: Option<BTreeMap<String, crate::core_observer::CoreHost>>, now: u64) {
        let Some(incoming) = incoming else {
            self.invalidated = true;
            return;
        };
        for (machine, observation) in &incoming {
            if observation.connected {
                self.last_seen.insert(machine.clone(), now);
            }
        }
        self.latest = incoming;
        self.observed_at_ms = Some(now);
        self.invalidated = false;
    }

    fn project(&self, machine: &str, now: u64) -> Value {
        let last_seen = self.last_seen.get(machine).copied();
        let row = self.latest.get(machine);
        let fresh = !self.invalidated
            && self.observed_at_ms.and_then(|at| now.checked_sub(at))
                .is_some_and(|age| age < CORE_STALE_MS);
        let state = match (fresh, row, last_seen) {
            (true, Some(row), _) if row.connected => "reporting",
            (_, _, Some(_)) => "stale",
            (true, None, _) => "not_reporting",
            _ => "unreachable",
        };
        let reporting = row.filter(|_| state == "reporting");
        json!({
            "state": state,
            "observed_at_ms": row.and(self.observed_at_ms).or(last_seen),
            "last_seen_at_ms": last_seen,
            "terminals": reporting.and_then(|row| row.terminals),
            "statuses": reporting.and_then(|row| row.statuses.as_ref()),
        })
    }
}

#[cfg(test)]
#[test]
fn core_counts_expire_and_missing_hosts_are_not_idle() {
    use crate::core_observer::CoreHost;
    let mut view = CoreObservations::default();
    assert_eq!(view.project("local", 1)["state"], "unreachable");
    view.update(Some(BTreeMap::from([("local".to_owned(), CoreHost {
        connected: true, terminals: Some(0), statuses: Some(BTreeMap::new()),
    })])), 2);
    assert_eq!(view.project("local", 2)["terminals"], 0);
    assert_eq!(view.project("absent", 2)["state"], "not_reporting");
    assert_eq!(view.project("local", 2 + CORE_STALE_MS)["state"], "stale");
    view.update(None, 3);
    let stale = view.project("local", 3);
    assert_eq!(stale["state"], "stale");
    assert_eq!(stale["last_seen_at_ms"], 2);
    assert!(stale["terminals"].is_null() && stale["statuses"].is_null());
}

#[derive(Debug)]
struct MachineObservation {
    has_route: bool,
    observed_at_ms: Option<u64>,
    last_seen_at_ms: Option<u64>,
    sessions: Option<usize>,
    load: Option<MachineLoad>,
}

/// Shared service state for the API and the dispatch thread.
#[derive(Debug)]
pub struct Service {
    pub config: Arc<FleetConfig>,
    pub store: Store,
    pub identity: PeerIdentity,
    pub swarm: Option<Arc<SwarmLink>>,
    pub clock: fn() -> u64,
    machine_observations: Mutex<BTreeMap<String, MachineObservation>>,
    core_observations: Mutex<CoreObservations>,
    controls: ControlStore,
    herdr: Mutex<Option<HerdrClient>>,
    last_tick: Mutex<Option<TickSummary>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimBody {
    scope: String,
    tool: String,
    session: String,
    purpose: String,
    #[serde(default)]
    lease_minutes: Option<u64>,
    /// Accepted for clients that declare it, and always replaced by the
    /// authenticated machine.
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenewBody {
    fence: u64,
    #[serde(default)]
    lease_minutes: Option<u64>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseBody {
    fence: u64,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffBody {
    fence: u64,
    commit: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelBody {
    reason: String,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnswerBody {
    text: String,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineDrainBody {
    draining: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptPromptBody {
    text: String,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptInterruptBody {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchControlBody {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

const fn default_swarm_concurrency() -> u32 {
    4
}

const fn default_swarm_repair_rounds() -> u32 {
    3
}

const fn default_swarm_budget() -> u64 {
    10_000
}

const fn default_swarm_stale_after_ms() -> u64 {
    DEFAULT_STALE_AFTER_MS
}

const fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwarmCreateBody {
    title: String,
    allowed_scopes: Vec<String>,
    #[serde(default = "default_swarm_concurrency")]
    max_concurrency: u32,
    #[serde(default = "default_swarm_repair_rounds")]
    max_repair_rounds: u32,
    #[serde(default = "default_swarm_budget")]
    budget_ceiling_units: u64,
    #[serde(default = "default_swarm_stale_after_ms")]
    stale_after_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwarmSpawnBody {
    name: String,
    role: SwarmRole,
    #[serde(default)]
    host: Option<String>,
    harness: String,
    model: String,
    #[serde(default)]
    scope: Option<String>,
    brief: String,
    #[serde(default)]
    requester: Option<String>,
    #[serde(default)]
    attempt: Option<ContentId>,
    #[serde(default)]
    claim_identity: Option<ContentId>,
    #[serde(default)]
    claim_fence: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwarmMemberStateUpdateBody {
    #[serde(default)]
    action: Option<String>,
    member: String,
    state: SwarmMemberState,
    #[serde(default)]
    budget_draw_delta: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SwarmMemberBody {
    UpdateState(SwarmMemberStateUpdateBody),
    Spawn(SwarmSpawnBody),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwarmPostBody {
    sender: String,
    kind: BoardPostKind,
    #[serde(default)]
    mentions: Vec<MentionTarget>,
    body: String,
    #[serde(default)]
    in_reply_to: Option<u64>,
    #[serde(default)]
    candidate_commit: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwarmVerdictBody {
    verifier: String,
    author: String,
    commit: String,
    verdict: String,
    #[serde(default)]
    findings: Vec<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwarmWatchdogBody {
    #[serde(default = "default_true")]
    apply: bool,
}

type Handled = Result<Response, Response>;

impl Service {
    /// Build the service from a validated configuration.
    ///
    /// # Errors
    /// Returns the enrollment or swarm diagnostic.
    pub fn new(config: FleetConfig) -> Result<Self, String> {
        if let Some(console_url) = &config.console_url {
            validate_console_url(console_url)?;
        }
        let store = Store::load(
            &config.implementer.database,
            &config.implementer.enrollment,
            config.authority_operator.as_deref(),
            config.authority_config.as_deref(),
        )?;
        let swarm = match config.active_swarm() {
            Some(swarm) => Some(Arc::new(SwarmLink::new(swarm)?)),
            None => None,
        };
        let controls = ControlStore::load(&config.implementer.database)?;
        Ok(Self {
            identity: PeerIdentity::new(&config),
            config: Arc::new(config),
            store,
            swarm,
            clock: log::now_ms,
            machine_observations: Mutex::default(),
            core_observations: Mutex::default(),
            controls,
            herdr: Mutex::default(),
            last_tick: Mutex::default(),
        })
    }

    // r[impl kiln.fleet.api]
    /// Authenticate the peer, then route.
    #[must_use]
    pub fn handle(&self, peer: IpAddr, request: &Request) -> Response {
        let now_ms = (self.clock)();
        match self.identity.identify(peer, now_ms) {
            Ok(machine) => self.route(&machine, request, now_ms),
            Err(error) => {
                log::warn(
                    "api.refused_peer",
                    json!({ "peer": peer.to_string(), "reason": error }),
                );
                Response::error(403, "fleet_peer", error)
            }
        }
    }

    // r[impl kiln.fleet.unified_console_surface]
    /// Route one request from an authenticated `machine`.
    #[must_use]
    pub fn route(&self, machine: &str, request: &Request, now_ms: u64) -> Response {
        if matches!(request.path.as_str(), "/" | "/ui") {
            if request.method != "GET" {
                return Response::error(405, "fleet_method", "method not allowed on this route");
            }
            return console_pointer_response(self.config.console_url.as_deref());
        }
        let Some(rest) = request.path.strip_prefix("/v1/") else {
            return Response::error(404, "fleet_route", "no such route");
        };
        let segments: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
        let method = request.method.as_str();
        let outcome = match (method, segments.as_slice()) {
            ("GET", ["health"]) => Ok(Response::new(
                200,
                json!({ "machine": self.config.machine, "version": env!("CARGO_PKG_VERSION"), "api": API_VERSION, "peer": machine }),
            )),
            ("GET", ["machines"]) => self.machines(now_ms),
            ("POST", ["machines", name, "drain"]) => {
                self.drain_machine(machine, name, &request.body, now_ms)
            }
            ("GET", ["version"]) => self.state_version(machine, now_ms),
            ("GET", ["claims"]) => self.list_claims(request.query.as_deref(), now_ms),
            ("POST", ["claims"]) => self.acquire(machine, &request.body, now_ms),
            ("POST", ["claims", id, "renew"]) => self.renew(machine, id, &request.body, now_ms),
            ("POST", ["claims", id, "release"]) => self.release(machine, id, &request.body, now_ms),
            ("GET", ["work"]) => self.work(now_ms),
            ("GET", ["attempts"]) => self.attempts(now_ms),
            ("GET", ["attempts", id, "pane"]) => {
                self.attempt_pane(id, request.query.as_deref())
            }
            ("POST", ["attempts", id, "prompt"]) => {
                self.attempt_prompt(machine, id, &request.body, now_ms)
            }
            ("POST", ["attempts", id, "interrupt"]) => {
                self.attempt_interrupt(machine, id, &request.body, now_ms)
            }
            ("POST", ["attempts", id, "handoff"]) => {
                self.handoff(machine, id, &request.body, now_ms)
            }
            ("POST", ["attempts", id, "cancel"]) => self.cancel(machine, id, &request.body, now_ms),
            ("GET", ["decisions"]) => self.decisions(),
            ("GET", ["metrics"]) => self.metrics(now_ms),
            ("POST", ["decisions", id, "answer"]) => {
                self.answer(machine, id, &request.body, now_ms)
            }
            ("GET", ["dispatch", "status"]) => Ok(self.dispatch_status()),
            ("POST", ["dispatch", "pause"]) => {
                self.dispatch_pause(machine, &request.body, now_ms)
            }
            ("POST", ["dispatch", "resume"]) => {
                self.dispatch_resume(machine, &request.body, now_ms)
            }
            ("GET", ["swarms"]) => self.list_swarms(),
            ("POST", ["swarms"]) => self.create_swarm(machine, &request.body, now_ms),
            ("GET", ["swarms", id]) => self.get_swarm(id),
            ("POST", ["swarms", id, "members"]) => {
                self.swarm_member(machine, id, &request.body, now_ms)
            }
            ("GET", ["swarms", id, "board"]) => self.read_swarm_board(id, request.query.as_deref()),
            ("POST", ["swarms", id, "board"]) => {
                self.post_swarm_board(machine, id, &request.body, now_ms)
            }
            ("POST", ["swarms", id, "verdicts"]) => {
                self.swarm_verdict(machine, id, &request.body, now_ms)
            }
            ("POST", ["swarms", id, "watchdog"]) => {
                self.swarm_watchdog(machine, id, &request.body, now_ms)
            }
            ("POST", ["swarms", id, "failover"]) => {
                self.swarm_failover(machine, id, &request.body, now_ms)
            }
            ("GET", ["swarms", id, "graph"]) => self.swarm_graph(id, request.query.as_deref()),
            (
                _,
                [
                    "health" | "machines" | "version" | "claims" | "work" | "attempts"
                    | "decisions" | "metrics" | "swarms",
                ]
                | ["machines", _, "drain"]
                | ["dispatch", "status" | "pause" | "resume"]
                | ["swarms", _]
                | [
                    "swarms",
                    _,
                    "members" | "board" | "verdicts" | "watchdog" | "failover" | "graph",
                ]
                | ["claims", _, "renew" | "release"]
                | ["attempts", _, "pane" | "prompt" | "interrupt" | "handoff" | "cancel"]
                | ["decisions", _, "answer"],
            ) => Err(Response::error(
                405,
                "fleet_method",
                "method not allowed on this route",
            )),
            _ => Err(Response::error(404, "fleet_route", "no such route")),
        };
        outcome.unwrap_or_else(|response| response)
    }

    /// Attach the live `HerdrClient` and initialize machine observation slots.
    pub fn set_herdr(&self, client: HerdrClient) {
        self.configure_machine_observations(client.inventory());
        if let Ok(mut guard) = self.herdr.lock() {
            *guard = Some(client);
        }
    }

    pub(crate) fn herdr_client(&self) -> Option<HerdrClient> {
        self.herdr.lock().ok().and_then(|guard| guard.clone())
    }

    #[must_use]
    pub fn controls_snapshot(&self) -> FleetControls {
        self.controls.snapshot()
    }

    #[must_use]
    pub fn is_machine_draining(&self, machine: &str) -> bool {
        self.controls.snapshot().is_draining(machine)
    }

    pub(crate) fn record_tick_summary(&self, summary: TickSummary) {
        if let Ok(mut guard) = self.last_tick.lock() {
            *guard = Some(summary);
        }
    }

    pub(crate) fn configure_machine_observations(&self, inventory: &HerdrInventory) {
        if let Ok(mut observations) = self.machine_observations.lock() {
            *observations = inventory
                .machines
                .iter()
                .map(|machine| {
                    (
                        machine.label.clone(),
                        MachineObservation {
                            // Only the coordinator can be reached via the local Herdr session.
                            has_route: machine.label == self.config.machine
                                || machine.herdr_machine.is_some(),
                            observed_at_ms: None,
                            last_seen_at_ms: None,
                            sessions: None,
                            load: None,
                        },
                    )
                })
                .collect();
        }
    }

    pub(crate) fn record_machine_load(
        &self,
        machine: &str,
        sessions: Option<usize>,
        load: MachineLoad,
    ) {
        if let Ok(mut observations) = self.machine_observations.lock()
            && let Some(observation) = observations.get_mut(machine)
            && observation.has_route
        {
            let now_ms = (self.clock)();
            observation.observed_at_ms = Some(now_ms);
            if sessions.is_some() {
                observation.last_seen_at_ms = Some(now_ms);
            }
            observation.sessions = sessions;
            observation.load = Some(load);
        }
    }

    pub(crate) fn observe_core(&self) {
        const RECONNECT_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
        let Some(config) = &self.config.herdr_core else { return; };
        let mut last_error = None;
        loop {
            let result = crate::core_observer::observe(&config.socket, &config.version, &config.hosts, |value| {
                if value.is_some() && last_error.take().is_some() {
                    log::info("core.observation_restored", json!({}));
                }
                if let Ok(mut observations) = self.core_observations.lock() {
                    observations.update(value, (self.clock)());
                }
            });
            if let Ok(mut observations) = self.core_observations.lock() {
                observations.update(None, (self.clock)());
            }
            if let Err(reason) = result && last_error != Some(reason) {
                log::warn("core.observation_unavailable", json!({ "reason": reason }));
                last_error = Some(reason);
            }
            std::thread::sleep(RECONNECT_DELAY);
        }
    }

    // r[impl kiln.fleet.aggregate_workspace]
    // r[impl kiln.fleet.herdr_core_observation]
    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn machines(&self, now_ms: u64) -> Handled {
        let mut implementer = self.open("GET /v1/machines")?;
        let views = implementer
            .fleet_attempt_views(now_ms)
            .map_err(|error| store_error(&error))?;
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let mut active_by_machine: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for view in &views {
            let session = state
                .sessions
                .sessions
                .values()
                .filter(|session| {
                    session.attempt == view.attempt
                        && session.fence == view.fence
                        && !session.state.is_ended()
                })
                .max_by_key(|session| session.created_ms);
            if let Some(session) = session {
                active_by_machine
                    .entry(session.machine.clone())
                    .or_default()
                    .push(json!({
                        "attempt": view.attempt,
                        "fence": view.fence,
                        "state": view.state,
                        "kind": session.kind,
                        "session": session.identity,
                        "session_state": session_state_label(&session.state),
                        "agent_name": session.agent_name,
                    }));
            }
        }
        let herdr = self.herdr_client();
        let controls = self.controls.snapshot();
        let observations = self.machine_observations.lock().ok();
        let core = self.core_observations.lock().ok();
        let max_age = self.config.policy.tick_ms.saturating_mul(MACHINE_STALE_TICKS);
        let machines: Vec<Value> = self
            .config
            .machines()
            .into_iter()
            .map(|machine| {
                let observation = observations.as_ref().and_then(|rows| rows.get(machine));
                let observed_at_ms = observation.and_then(|row| row.observed_at_ms);
                let last_seen_at_ms = observation.and_then(|row| row.last_seen_at_ms);
                let sessions = observation.and_then(|row| row.sessions);
                let load = observation.and_then(|row| row.load.as_ref());
                let state = match observed_at_ms {
                    None => "not_reporting",
                    Some(at) if !now_ms.checked_sub(at).is_some_and(|age| age < max_age) => {
                        "stale"
                    }
                    Some(_) if sessions.is_none() => "unreachable",
                    Some(_) => "reporting",
                };
                let inventory_machine = herdr
                    .as_ref()
                    .and_then(|client| client.inventory().machine(machine));
                let (available, slots, error) =
                    project_machine_slots(state, inventory_machine, load, herdr.is_some());
                let drain = controls.drain_record(machine);
                let active_attempts = active_by_machine.get(machine).cloned().unwrap_or_default();
                let mut entry = json!({
                    "machine": machine,
                    "placement_configured": observation.is_some(),
                    "observed_at_ms": observed_at_ms,
                    "last_seen_at_ms": last_seen_at_ms,
                    "state": state,
                    "sessions": if state == "reporting" { sessions } else { None },
                    "health": if available { "available" } else { "unavailable" },
                    "error": error,
                    "slots": slots,
                    "active_attempts": active_attempts,
                    "draining": drain.is_some(),
                    "drain_reason": drain.and_then(|record| record.reason.clone()),
                    "drained_at_ms": drain.map(|record| record.updated_at_ms),
                    "inventory": inventory_machine.map(project_inventory_machine),
                });
                if self.config.herdr_core.as_ref().is_some_and(|config| config.hosts.contains_key(machine)) {
                    entry["core"] = core.as_ref().map_or_else(|| CoreObservations::default().project(machine, now_ms), |view| view.project(machine, now_ms));
                }
                entry
            })
            .collect();
        Ok(Response::new(200, json!({ "machines": machines })))
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn drain_machine(&self, machine: &str, name: &str, body: &[u8], now_ms: u64) -> Handled {
        self.operator(machine)?;
        if !machine_label(name) {
            return Err(Response::error(
                400,
                "fleet_machine_invalid",
                "machine name must be a lowercase label",
            ));
        }
        if !self.config.machines().contains(name) {
            return Err(Response::error(
                404,
                "fleet_machine_unknown",
                format!("unknown fleet machine {name}"),
            ));
        }
        let body: MachineDrainBody = parse_body(body)?;
        let reason = validate_reason(body.reason.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let request_id = validate_request_id(body.request_id.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let hash = payload_hash(&json!({ "draining": body.draining, "reason": &reason }));
        let req_key = request_id
            .as_ref()
            .map(|id| format!("drain:{name}:{id}"));
        let response_json = self
            .controls
            .mutate(|controls| {
                if let Some(key) = &req_key {
                    match controls.lookup_request(key, &hash) {
                        Ok(Some(cached)) => return Ok(cached.clone()),
                        Ok(None) => {}
                        Err(RequestIdConflict) => return Err("request_id_conflict".to_owned()),
                    }
                }
                controls.set_machine_drain(name, body.draining, reason.clone(), machine, now_ms);
                let drain = controls.drain_record(name);
                let reply = json!({
                    "machine": name,
                    "draining": drain.is_some(),
                    "drain_reason": drain.and_then(|record| record.reason.clone()),
                    "drained_at_ms": drain.map(|record| record.updated_at_ms),
                    "request_id": request_id,
                });
                if let Some(key) = req_key {
                    controls.record_request(key, hash, reply.clone());
                }
                Ok(reply)
            })
            .map_err(|error| {
                if error == "request_id_conflict" {
                    Response::error(
                        409,
                        "fleet_request_id_conflict",
                        "request_id already used with a different drain payload",
                    )
                } else {
                    Response::error(500, "fleet_controls_persist", error)
                }
            })?;
        log::info(
            "api.machine_drain",
            json!({ "target": name, "draining": body.draining, "actor": machine }),
        );
        Ok(Response::new(200, response_json))
    }

    /// Open only the holder seeds admitted for this route.
    fn open(&self, route: &str) -> Result<kiln_implementer::Implementer, Response> {
        self.store
            .open_route(route)
            .map_err(|error| store_error(&error))
    }

    fn foreign(&self, now_ms: u64) -> Result<Vec<ForeignClaim>, Response> {
        match &self.swarm {
            Some(swarm) => swarm
                .foreign(now_ms)
                .map_err(|error| Response::error(503, "fleet_swarm_unavailable", error)),
            None => Ok(Vec::new()),
        }
    }

    fn lease_ms(&self, minutes: Option<u64>) -> Result<u64, Response> {
        let policy = &self.config.policy;
        let Some(minutes) = minutes else {
            return Ok(policy.claim_lease_ms);
        };
        minutes
            .checked_mul(MS_PER_MINUTE)
            .filter(|lease| *lease <= policy.max_claim_lease_ms)
            .ok_or_else(|| {
                Response::error(
                    400,
                    "fleet_claim_lease",
                    format!(
                        "lease_minutes exceeds {} minutes",
                        policy.max_claim_lease_ms / MS_PER_MINUTE
                    ),
                )
            })
    }

    fn state_version(&self, machine: &str, now_ms: u64) -> Handled {
        let version = self
            .open("GET /v1/version")?
            .fleet_observation_version()
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(
            200,
            json!({
                "machine": self.config.machine,
                "peer": machine,
                "version": version,
                "observed_ms": now_ms,
            }),
        ))
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn list_claims(&self, query: Option<&str>, now_ms: u64) -> Handled {
        let include =
            parse_claims_query(query).map_err(|msg| Response::error(400, "fleet_query", msg))?;
        let state = self
            .open("GET /v1/claims")?
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let claims: Vec<_> = state.claims.live(now_ms).cloned().collect();
        let mut payload = json!({ "claims": claims });
        if include.foreign {
            let (foreign_claims, foreign_mirror) = match &self.swarm {
                Some(swarm) => match swarm.foreign(now_ms) {
                    Ok(rows) => (
                        rows.iter()
                            .map(|claim| project_foreign_claim(claim, now_ms))
                            .collect::<Vec<_>>(),
                        json!({
                            "configured": true,
                            "status": "ok",
                            "error": null,
                        }),
                    ),
                    Err(error) => (
                        Vec::new(),
                        json!({
                            "configured": true,
                            "status": "unavailable",
                            "error": error,
                        }),
                    ),
                },
                None => (
                    Vec::new(),
                    json!({
                        "configured": false,
                        "status": "unconfigured",
                        "error": null,
                    }),
                ),
            };
            payload["foreign_claims"] = Value::Array(foreign_claims);
            payload["foreign_mirror"] = foreign_mirror;
        }
        if include.expired {
            let (expired_claims, expired_truncated) =
                select_expired_claims(&state.claims.claims, now_ms, MAX_EXPIRED_CLAIMS);
            payload["expired_claims"] = json!(expired_claims);
            payload["expired_truncated"] = json!(expired_truncated);
        }
        Ok(Response::new(200, payload))
    }

    // r[impl kiln.fleet.api.scenario.identity]
    fn acquire(&self, machine: &str, body: &[u8], now_ms: u64) -> Handled {
        let body: ClaimBody = parse_body(body)?;
        let _request_id = validate_request_id(body.request_id.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        if body.host.as_deref().is_some_and(|host| host != machine) {
            log::info(
                "api.claim_host_replaced",
                json!({ "declared": body.host, "machine": machine }),
            );
        }
        let scope = parse_scope_text(&body.scope)
            .map_err(|error| Response::error(400, &error.code, error.message))?;
        let holder = FleetAgent {
            tool: body.tool,
            host: machine.to_owned(),
            session: body.session,
        };
        let lease_ms = self.lease_ms(body.lease_minutes)?;
        let foreign = self.foreign(now_ms)?;
        let (outcome, _) = self
            .open("POST /v1/claims")?
            .fleet_claim(
                ClaimCommand::Acquire {
                    scope,
                    holder,
                    purpose: body.purpose,
                    lease_ms,
                    attempt: None,
                },
                now_ms,
                &self.config.machine,
                &foreign,
            )
            .map_err(|error| store_error(&error))?;
        Ok(match outcome {
            ClaimOutcome::Granted(claim) => {
                Response::new(201, json!({ "outcome": "granted", "claim": claim }))
            }
            ClaimOutcome::Existing(claim) => {
                Response::new(200, json!({ "outcome": "existing", "claim": claim }))
            }
            ClaimOutcome::Conflict(conflict) => Response::new(
                409,
                json!({ "outcome": "conflict", "code": "fleet_claim_conflict", "error": "the scope overlaps an existing claim", "conflict": conflict }),
            ),
            other => Response::new(200, json!({ "outcome": other })),
        })
    }

    fn holder_check(
        &self,
        implementer: &mut kiln_implementer::Implementer,
        machine: &str,
        identity: &ContentId,
    ) -> Result<(), Response> {
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let claim =
            state.claims.claims.get(identity).ok_or_else(|| {
                Response::error(404, "fleet_claim_unknown", "no such fleet claim")
            })?;
        if claim.holder.host != machine && !self.config.is_operator(machine) {
            return Err(Response::error(
                403,
                "fleet_claim_machine",
                "only the holder's machine or an operator may change this claim",
            ));
        }
        Ok(())
    }

    fn check_idempotent(
        &self,
        namespace: &str,
        target: &str,
        request_id: Option<&str>,
        payload: &Value,
    ) -> Result<Option<(String, String, Option<Response>)>, Response> {
        let Some(request_id) = validate_request_id(request_id)
            .map_err(|error| Response::error(400, "fleet_body", error))?
        else {
            return Ok(None);
        };
        let key = format!("{namespace}:{target}:{request_id}");
        let hash = payload_hash(payload);
        match self.controls.snapshot().lookup_request(&key, &hash) {
            Ok(Some(cached)) => Ok(Some((key, hash, Some(Response::new(200, cached.clone()))))),
            Ok(None) => Ok(Some((key, hash, None))),
            Err(RequestIdConflict) => Err(Response::error(
                409,
                "fleet_request_id_conflict",
                "request_id already used with a different payload",
            )),
        }
    }

    fn record_idempotent(
        &self,
        token: Option<(String, String, Option<Response>)>,
        reply: &Value,
    ) -> Result<(), Response> {
        if let Some((key, hash, None)) = token {
            self.controls
                .mutate(|controls| {
                    controls.record_request(key, hash, reply.clone());
                    Ok(())
                })
                .map_err(|error| Response::error(500, "fleet_controls_persist", error))?;
        }
        Ok(())
    }

    fn renew(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let body: RenewBody = parse_body(body)?;
        let idemp = self.check_idempotent(
            "renew",
            identity.as_str(),
            body.request_id.as_deref(),
            &json!({ "fence": body.fence, "lease_minutes": body.lease_minutes }),
        )?;
        if let Some((_, _, Some(cached))) = idemp {
            return Ok(cached);
        }
        let lease_ms = self.lease_ms(body.lease_minutes)?;
        let mut implementer = self.open("POST /v1/claims/{id}/renew")?;
        self.holder_check(&mut implementer, machine, &identity)?;
        let (outcome, _) = implementer
            .fleet_claim(
                ClaimCommand::Renew {
                    identity,
                    fence: body.fence,
                    lease_ms,
                },
                now_ms,
                &self.config.machine,
                &[],
            )
            .map_err(|error| store_error(&error))?;
        let response = claim_response(outcome);
        self.record_idempotent(idemp, &response.body)?;
        Ok(response)
    }

    fn release(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let body: ReleaseBody = parse_body(body)?;
        let idemp = self.check_idempotent(
            "release",
            identity.as_str(),
            body.request_id.as_deref(),
            &json!({ "fence": body.fence, "note": &body.note }),
        )?;
        if let Some((_, _, Some(cached))) = idemp {
            return Ok(cached);
        }
        let mut implementer = self.open("POST /v1/claims/{id}/release")?;
        self.holder_check(&mut implementer, machine, &identity)?;
        let (outcome, _) = implementer
            .fleet_claim(
                ClaimCommand::Release {
                    identity,
                    fence: body.fence,
                    note: body.note.unwrap_or_default(),
                },
                now_ms,
                &self.config.machine,
                &[],
            )
            .map_err(|error| store_error(&error))?;
        let response = claim_response(outcome);
        self.record_idempotent(idemp, &response.body)?;
        Ok(response)
    }

    fn work(&self, now_ms: u64) -> Handled {
        let mut implementer = self.open("GET /v1/work")?;
        let ledger = implementer.work().map_err(|error| store_error(&error))?;
        let ready_unclaimed = implementer
            .fleet_ready_work(now_ms)
            .map_err(|error| store_error(&error))?;
        let ready = ledger.ranked();
        let ranked: Vec<Value> = ready
            .iter()
            .take(MAX_LISTED_WORK)
            .filter_map(|row| {
                let work = ledger.get(&row.canonical_key)?;
                Some(json!({
                    "identity": row.identity,
                    "priority": row.priority,
                    "repository": work.repository,
                    "title": work.title,
                    "permitted_paths": work.permitted_paths,
                    "sources": work.aliases,
                    "interactive": work.permitted_effects.contains(kiln_core::implementation::INTERACTIVE_AGENT_EFFECT),
                }))
            })
            .collect();
        let mut blocked = ledger
            .current()
            .filter_map(|work| {
                let key = work.canonical_key().ok()?;
                match ledger.eligibility_of(&key)? {
                    Eligibility::Blocked { reasons } => Some((work.identity().ok()?, work, reasons)),
                    Eligibility::Ready => None,
                }
            });
        let blocked_rows: Vec<Value> = blocked
            .by_ref()
            .take(MAX_LISTED_WORK)
            .map(|(identity, work, reasons)| {
                json!({
                    "identity": identity,
                    "repository": work.repository,
                    "title": work.title,
                    "sources": work.aliases,
                    "reasons": reasons,
                })
            })
            .collect();
        let truncated = ready.len() > MAX_LISTED_WORK || blocked.next().is_some();
        Ok(Response::new(
            200,
            json!({ "ready_unclaimed": ready_unclaimed, "ranked": ranked, "blocked": blocked_rows, "truncated": truncated }),
        ))
    }

    fn attempts(&self, now_ms: u64) -> Handled {
        let mut implementer = self.open("GET /v1/attempts")?;
        let views = implementer
            .fleet_attempt_views(now_ms)
            .map_err(|error| store_error(&error))?;
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let attempts = implementer
            .attempts()
            .map_err(|error| store_error(&error))?;
        let ledger = implementer.work().map_err(|error| store_error(&error))?;
        let policy = self.config.policy.fleet();
        let listed: Vec<Value> = views
            .iter()
            .take(MAX_LISTED_ATTEMPTS)
            .map(|view| {
                let attempt = attempts.get(&view.attempt);
                let work = attempt.and_then(|attempt| {
                    ledger
                        .current()
                        .chain(ledger.superseded())
                        .find(|work| work.identity().is_ok_and(|id| id == attempt.work_identity))
                });
                let session = state
                    .sessions
                    .sessions
                    .values()
                    .filter(|session| {
                        session.attempt == view.attempt && session.fence == view.fence
                    })
                    .max_by_key(|session| session.created_ms);
                let observation_age_ms = session.map(|session| session.observation_age_ms(now_ms));
                let observation_stale =
                    session.is_some_and(|session| session.is_observation_stale(now_ms, &policy));
                json!({
                    "attempt": view.attempt,
                    "fence": view.fence,
                    "state": view.state,
                    "lease_expires_ms": view.lease_expires_ms,
                    "held_by_fleet": view.held_by_fleet,
                    "phase": attempt.map(|attempt| attempt.phase),
                    "work": attempt.map(|attempt| &attempt.work_identity),
                    "repository": work.map(|work| &work.repository),
                    "title": work.map(|work| &work.title),
                    "observation_age_ms": observation_age_ms,
                    "observation_stale": observation_stale,
                    "sources": work.map(|work| &work.aliases),
                    "session": session,
                })
            })
            .collect();
        Ok(Response::new(200, json!({ "attempts": listed, "truncated": views.len() > MAX_LISTED_ATTEMPTS })))
    }

    // r[impl kiln.fleet.capture]
    fn handoff(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let attempt = path_identity(id)?;
        let body: HandoffBody = parse_body(body)?;
        if body.commit.len() != HANDOFF_COMMIT_HEX
            || !body
                .commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Response::error(
                400,
                "fleet_handoff_commit",
                "commit must be a full 40-character lowercase hexadecimal id",
            ));
        }
        let mut implementer = self.open("POST /v1/attempts/{id}/handoff")?;
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let session = state
            .sessions
            .sessions
            .values()
            .find(|session| {
                session.attempt == attempt
                    && session.fence == body.fence
                    && !session.state.is_ended()
            })
            .ok_or_else(|| {
                Response::error(
                    404,
                    "fleet_session_unknown",
                    "no live session follows this attempt and fence",
                )
            })?;
        if session.machine != machine {
            return Err(Response::error(
                403,
                "fleet_handoff_machine",
                "a handoff is accepted only from the machine hosting the session",
            ));
        }
        implementer
            .authorize_current(&attempt, body.fence, now_ms)
            .map_err(|error| store_error(&error))?;
        let observed = implementer
            .fleet_observe(
                &session.identity,
                Observation::Handoff {
                    commit: body.commit,
                },
                now_ms,
            )
            .map_err(|error| store_error(&error))?;
        log::info(
            "api.handoff",
            json!({ "attempt": attempt, "fence": body.fence, "session": observed.identity, "machine": machine }),
        );
        Ok(Response::new(202, json!({ "session": observed })))
    }

    fn cancel(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        self.operator(machine)?;
        let attempt = path_identity(id)?;
        let body: CancelBody = parse_body(body)?;
        let idemp = self.check_idempotent(
            "cancel",
            attempt.as_str(),
            body.request_id.as_deref(),
            &json!({ "reason": &body.reason }),
        )?;
        if let Some((_, _, Some(cached))) = idemp {
            return Ok(cached);
        }
        let mut implementer = self.open("POST /v1/attempts/{id}/cancel")?;
        let current = implementer
            .attempt(&attempt)
            .map_err(|error| store_error(&error))?
            .ok_or_else(|| {
                Response::error(404, "implementation_attempt_missing", "no such attempt")
            })?;
        implementer
            .cancel(&attempt, current.fence, now_ms, &body.reason)
            .map_err(|error| store_error(&error))?;
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let mut sessions = Vec::new();
        for session in state.sessions.sessions.values().filter(|session| {
            session.attempt == attempt
                && session.fence == current.fence
                && !session.state.is_ended()
        }) {
            match implementer.fleet_observe(
                &session.identity,
                Observation::Cancelled {
                    cause: body.reason.clone(),
                },
                now_ms,
            ) {
                Ok(observed) => sessions.push(observed),
                Err(error) => log::warn(
                    "api.cancel_session",
                    json!({ "session": session.identity, "error": error }),
                ),
            }
        }
        log::info(
            "api.cancel",
            json!({ "attempt": attempt, "fence": current.fence, "machine": machine }),
        );
        let reply = json!({ "attempt": attempt, "fence": current.fence, "cancelled": true, "sessions": sessions });
        self.record_idempotent(idemp, &reply)?;
        Ok(Response::new(200, reply))
    }

    fn metrics(&self, now_ms: u64) -> Handled {
        let mut implementer = self.open("GET /v1/metrics")?;
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let version = implementer
            .fleet_version()
            .map_err(|error| store_error(&error))?;
        let ledger = implementer.work().map_err(|error| store_error(&error))?;
        let ready = implementer
            .fleet_ready_work(now_ms)
            .map_err(|error| store_error(&error))?;
        let attempts = implementer
            .fleet_attempt_views(now_ms)
            .map_err(|error| store_error(&error))?
            .len();
        let claims = state.claims.live(now_ms).count();
        let horizon = now_ms.saturating_add(15 * MS_PER_MINUTE);
        let expiring = state
            .claims
            .live(now_ms)
            .filter(|claim| claim.expires_ms <= horizon)
            .count();
        let blocked = ledger
            .current()
            .filter_map(|work| {
                let key = work.canonical_key().ok()?;
                matches!(ledger.eligibility_of(&key)?, Eligibility::Blocked { .. }).then_some(())
            })
            .count();
        let decisions = state.decisions.decisions.values().count();
        let open_decisions = state
            .decisions
            .decisions
            .values()
            .filter(|decision| decision.is_open())
            .count();
        let policy = self.config.policy.fleet();
        let max_observation_age_ms = state
            .sessions
            .sessions
            .values()
            .filter(|session| session.state.requires_observation())
            .map(|session| session.observation_age_ms(now_ms))
            .max()
            .unwrap_or(0);
        let stale_sessions = state
            .sessions
            .sessions
            .values()
            .filter(|session| session.is_observation_stale(now_ms, &policy))
            .count();
        let mut text = String::new();
        use std::fmt::Write as _;
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_state_version Monotonic persisted fleet state version.\n# TYPE kiln_fleet_state_version gauge\nkiln_fleet_state_version {version}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_claims Live claims.\n# TYPE kiln_fleet_claims gauge\nkiln_fleet_claims {claims}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_claims_expiring_15m Live claims expiring within 15 minutes.\n# TYPE kiln_fleet_claims_expiring_15m gauge\nkiln_fleet_claims_expiring_15m {expiring}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_ready_work Canonical work currently eligible for a new attempt.\n# TYPE kiln_fleet_ready_work gauge\nkiln_fleet_ready_work {ready}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_blocked_work Canonical work with a recorded eligibility reason.\n# TYPE kiln_fleet_blocked_work gauge\nkiln_fleet_blocked_work {blocked}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_attempts Current attempt views.\n# TYPE kiln_fleet_attempts gauge\nkiln_fleet_attempts {attempts}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_decisions Decisions retained by the coordinator.\n# TYPE kiln_fleet_decisions gauge\nkiln_fleet_decisions {decisions}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_decisions_open Decisions neither answered nor closed.\n# TYPE kiln_fleet_decisions_open gauge\nkiln_fleet_decisions_open {open_decisions}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_session_max_observation_age_ms Maximum observation age across live observed sessions in milliseconds.\n# TYPE kiln_fleet_session_max_observation_age_ms gauge\nkiln_fleet_session_max_observation_age_ms {max_observation_age_ms}"
        );
        let _ = writeln!(
            text,
            "# HELP kiln_fleet_sessions_stale Live sessions whose observation age reached the stale bound.\n# TYPE kiln_fleet_sessions_stale gauge\nkiln_fleet_sessions_stale {stale_sessions}"
        );
        Ok(Response::text(200, text))
    }

    fn decisions(&self) -> Handled {
        let state = self
            .open("GET /v1/decisions")?
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let mut decisions: Vec<_> = state.decisions.decisions.values().collect();
        decisions.sort_by_key(|decision| std::cmp::Reverse(decision.raised_ms));
        decisions.truncate(MAX_LISTED_DECISIONS);
        Ok(Response::new(200, json!({ "decisions": decisions })))
    }

    // r[impl kiln.fleet.decisions]
    fn answer(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        self.operator(machine)?;
        let decision = path_identity(id)?;
        let body: AnswerBody = parse_body(body)?;
        let idemp = self.check_idempotent(
            "answer",
            decision.as_str(),
            body.request_id.as_deref(),
            &json!({ "text": &body.text }),
        )?;
        if let Some((_, _, Some(cached))) = idemp {
            return Ok(cached);
        }
        let prompt = self
            .open("POST /v1/decisions/{id}/answer")?
            .fleet_answer_decision(&decision, body.text, machine, true, now_ms)
            .map_err(|error| store_error(&error))?;
        log::info(
            "api.answer",
            json!({ "decision": decision, "machine": machine }),
        );
        let reply = json!({ "decision": decision, "answered": true, "prompt": prompt });
        self.record_idempotent(idemp, &reply)?;
        Ok(Response::new(200, reply))
    }

    fn live_attempt_session(
        implementer: &mut kiln_implementer::Implementer,
        attempt_id: &ContentId,
    ) -> Result<(u64, FleetSession, String), Response> {
        let attempt = implementer
            .attempt(attempt_id)
            .map_err(|error| store_error(&error))?
            .ok_or_else(|| {
                Response::error(404, "implementation_attempt_missing", "no such attempt")
            })?;
        let state = implementer
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let session = state
            .sessions
            .sessions
            .values()
            .filter(|session| session.attempt == *attempt_id && session.fence == attempt.fence)
            .max_by_key(|session| session.created_ms)
            .cloned()
            .ok_or_else(|| {
                Response::error(
                    404,
                    "fleet_session_unknown",
                    "no herdr session exists for this attempt and fence",
                )
            })?;
        if session.state.is_ended() {
            return Err(Response::error(
                409,
                "fleet_session_ended",
                "the attempt session has already ended",
            ));
        }
        let agent = session.agent_name.clone().ok_or_else(|| {
            Response::error(
                409,
                "fleet_session_not_started",
                "the attempt session has not started an agent yet",
            )
        })?;
        Ok((attempt.fence, session, agent))
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn attempt_pane(&self, id: &str, query: Option<&str>) -> Handled {
        let attempt_id = path_identity(id)?;
        let requested_lines =
            parse_pane_lines_query(query).map_err(|msg| Response::error(400, "fleet_query", msg))?;
        let mut implementer = self.open("GET /v1/attempts/{id}/pane")?;
        let (fence, session, agent) = Self::live_attempt_session(&mut implementer, &attempt_id)?;
        let herdr = self.herdr_client().ok_or_else(|| {
            Response::error(
                503,
                "fleet_herdr_unavailable",
                "herdr inventory is not attached or dispatch is disabled",
            )
        })?;
        let raw = herdr
            .read(&session.machine, &agent, requested_lines)
            .map_err(|failure| herdr_error_response(&failure))?;
        let pane = sanitize_pane_output(&raw, requested_lines, MAX_PANE_TEXT_BYTES);
        let pane_id = session
            .workspace
            .as_ref()
            .map_or("", |workspace| workspace.pane.as_str());
        Ok(Response::new(
            200,
            json!({
                "attempt": attempt_id,
                "fence": fence,
                "session": session.identity,
                "machine": session.machine,
                "agent": agent,
                "pane_id": pane_id,
                "session_state": session_state_label(&session.state),
                "requested_lines": requested_lines,
                "line_count": pane.line_count,
                "truncated": pane.truncated,
                "text": pane.text,
                "lines": pane.lines,
            }),
        ))
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn attempt_prompt(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        self.operator(machine)?;
        let attempt_id = path_identity(id)?;
        let body: AttemptPromptBody = parse_body(body)?;
        if body.text.is_empty()
            || body.text.len() > MAX_PROMPT_BYTES
            || body
                .text
                .chars()
                .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
        {
            return Err(Response::error(
                400,
                "fleet_prompt_invalid",
                "prompt text is empty, oversized, or contains control characters",
            ));
        }
        let request_id = validate_request_id(body.request_id.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let hash = payload_hash(&json!({ "text": &body.text }));
        let req_key = request_id
            .as_ref()
            .map(|req| format!("prompt:{}:{req}", attempt_id.as_str()));
        if let Some(key) = &req_key {
            match self.controls.snapshot().lookup_request(key, &hash) {
                Ok(Some(cached)) => return Ok(Response::new(200, cached.clone())),
                Ok(None) => {}
                Err(RequestIdConflict) => {
                    return Err(Response::error(
                        409,
                        "fleet_request_id_conflict",
                        "request_id already used with a different prompt payload",
                    ));
                }
            }
        }
        let mut implementer = self.open("POST /v1/attempts/{id}/prompt")?;
        let (fence, session, agent) = Self::live_attempt_session(&mut implementer, &attempt_id)?;
        if matches!(session.state, SessionState::HandoffReceived { .. }) {
            return Err(Response::error(
                409,
                "fleet_session_state",
                "cannot prompt a session after candidate handoff",
            ));
        }
        let herdr = self.herdr_client().ok_or_else(|| {
            Response::error(
                503,
                "fleet_herdr_unavailable",
                "herdr inventory is not attached or dispatch is disabled",
            )
        })?;
        herdr
            .prompt(&session.machine, &agent, &body.text)
            .map_err(|failure| herdr_error_response(&failure))?;
        if let Err(error) =
            implementer.fleet_observe(&session.identity, Observation::Prompted, now_ms)
        {
            log::warn(
                "api.prompt_observe_failed",
                json!({ "session": session.identity, "error": error }),
            );
        }
        let reply = json!({
            "attempt": attempt_id,
            "fence": fence,
            "session": session.identity,
            "machine": session.machine,
            "agent": agent,
            "prompted": true,
            "request_id": request_id,
        });
        if let Some(key) = req_key {
            self.controls
                .mutate(|controls| {
                    controls.record_request(key, hash, reply.clone());
                    Ok(())
                })
                .map_err(|error| Response::error(500, "fleet_controls_persist", error))?;
        }
        log::info(
            "api.attempt_prompt",
            json!({ "attempt": attempt_id, "fence": fence, "session": session.identity, "actor": machine }),
        );
        Ok(Response::new(200, reply))
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn attempt_interrupt(&self, machine: &str, id: &str, body: &[u8], _now_ms: u64) -> Handled {
        self.operator(machine)?;
        let attempt_id = path_identity(id)?;
        let body: AttemptInterruptBody = if body.is_empty() {
            AttemptInterruptBody::default()
        } else {
            parse_body(body)?
        };
        let reason = validate_reason(body.reason.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let request_id = validate_request_id(body.request_id.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let hash = payload_hash(&json!({ "reason": &reason }));
        let req_key = request_id
            .as_ref()
            .map(|req| format!("interrupt:{}:{req}", attempt_id.as_str()));
        if let Some(key) = &req_key {
            match self.controls.snapshot().lookup_request(key, &hash) {
                Ok(Some(cached)) => return Ok(Response::new(200, cached.clone())),
                Ok(None) => {}
                Err(RequestIdConflict) => {
                    return Err(Response::error(
                        409,
                        "fleet_request_id_conflict",
                        "request_id already used with a different interrupt payload",
                    ));
                }
            }
        }
        let mut implementer = self.open("POST /v1/attempts/{id}/interrupt")?;
        let (fence, session, agent) = Self::live_attempt_session(&mut implementer, &attempt_id)?;
        let herdr = self.herdr_client().ok_or_else(|| {
            Response::error(
                503,
                "fleet_herdr_unavailable",
                "herdr inventory is not attached or dispatch is disabled",
            )
        })?;
        herdr
            .interrupt(&session.machine, &agent)
            .map_err(|failure| herdr_error_response(&failure))?;
        let reply = json!({
            "attempt": attempt_id,
            "fence": fence,
            "session": session.identity,
            "machine": session.machine,
            "agent": agent,
            "interrupted": true,
            "reason": reason,
            "request_id": request_id,
        });
        if let Some(key) = req_key {
            self.controls
                .mutate(|controls| {
                    controls.record_request(key, hash, reply.clone());
                    Ok(())
                })
                .map_err(|error| Response::error(500, "fleet_controls_persist", error))?;
        }
        log::info(
            "api.attempt_interrupt",
            json!({ "attempt": attempt_id, "fence": fence, "session": session.identity, "actor": machine }),
        );
        Ok(Response::new(200, reply))
    }

    fn dispatch_status_payload(&self, controls: &FleetControls) -> Value {
        let last_tick = self.last_tick.lock().ok().and_then(|guard| guard.clone());
        json!({
            "enabled": self.config.dispatch.enabled,
            "state": if controls.dispatch_pause.paused { "paused" } else { "running" },
            "paused_reason": controls.dispatch_pause.reason,
            "paused_at_ms": controls.dispatch_pause.paused_at_ms,
            "paused_by": controls.dispatch_pause.paused_by,
            "draining_machines": controls.draining_machines(),
            "last_tick": last_tick,
        })
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn dispatch_status(&self) -> Response {
        let controls = self.controls.snapshot();
        Response::new(200, self.dispatch_status_payload(&controls))
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn dispatch_pause(&self, machine: &str, body: &[u8], now_ms: u64) -> Handled {
        self.toggle_dispatch_pause(machine, true, body, now_ms)
    }

    // r[impl kiln.fleet.live_pane_and_machine_census]
    fn dispatch_resume(&self, machine: &str, body: &[u8], now_ms: u64) -> Handled {
        self.toggle_dispatch_pause(machine, false, body, now_ms)
    }

    fn toggle_dispatch_pause(
        &self,
        machine: &str,
        paused: bool,
        body: &[u8],
        now_ms: u64,
    ) -> Handled {
        self.operator(machine)?;
        let body: DispatchControlBody = if body.is_empty() {
            DispatchControlBody::default()
        } else {
            parse_body(body)?
        };
        let reason = validate_reason(body.reason.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let request_id = validate_request_id(body.request_id.as_deref())
            .map_err(|error| Response::error(400, "fleet_body", error))?;
        let action = if paused { "pause" } else { "resume" };
        let hash = payload_hash(&json!({ "paused": paused, "reason": &reason }));
        let req_key = request_id
            .as_ref()
            .map(|req| format!("dispatch:{action}:{req}"));
        let reply = self
            .controls
            .mutate(|controls| {
                if let Some(key) = &req_key {
                    match controls.lookup_request(key, &hash) {
                        Ok(Some(cached)) => return Ok(cached.clone()),
                        Ok(None) => {}
                        Err(RequestIdConflict) => return Err("request_id_conflict".to_owned()),
                    }
                }
                controls.set_dispatch_pause(paused, reason.clone(), machine, now_ms);
                let mut payload = self.dispatch_status_payload(controls);
                payload["request_id"] = json!(request_id);
                if let Some(key) = req_key {
                    controls.record_request(key, hash, payload.clone());
                }
                Ok(payload)
            })
            .map_err(|error| {
                if error == "request_id_conflict" {
                    Response::error(
                        409,
                        "fleet_request_id_conflict",
                        "request_id already used with a different dispatch control payload",
                    )
                } else {
                    Response::error(500, "fleet_controls_persist", error)
                }
            })?;
        log::info(
            if paused {
                "api.dispatch_paused"
            } else {
                "api.dispatch_resumed"
            },
            json!({ "actor": machine, "reason": reason }),
        );
        Ok(Response::new(200, reply))
    }

    fn operator(&self, machine: &str) -> Result<(), Response> {
        if self.config.is_operator(machine) {
            Ok(())
        } else {
            Err(Response::error(
                403,
                "fleet_operator",
                "only an operator machine may use this route",
            ))
        }
    }

    fn swarm_member_host_check(
        &self,
        implementer: &mut kiln_implementer::Implementer,
        machine: &str,
        swarm_id: &ContentId,
        member_name: &str,
    ) -> Result<(), Response> {
        if self.config.is_operator(machine) {
            return Ok(());
        }
        let swarm = implementer
            .fleet_swarm(swarm_id)
            .map_err(|error| store_error(&error))?
            .ok_or_else(|| Response::error(404, "fleet_swarm_unknown", "no such swarm run"))?;
        let member = swarm.members.get(member_name).ok_or_else(|| {
            Response::error(
                404,
                "fleet_swarm_member_unknown",
                "no such member in this swarm run",
            )
        })?;
        if member.host != machine {
            return Err(Response::error(
                403,
                "fleet_swarm_host_mismatch",
                "only the member's authenticated host or an operator may act as this member",
            ));
        }
        Ok(())
    }

    // r[impl kiln.fleet.swarm.surface]
    fn list_swarms(&self) -> Handled {
        let swarms = self
            .open("GET /v1/swarms")?
            .fleet_swarms()
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(200, json!({ "swarms": swarms })))
    }

    // r[impl kiln.fleet.swarm.surface]
    fn create_swarm(&self, machine: &str, body: &[u8], now_ms: u64) -> Handled {
        self.operator(machine)?;
        let body: SwarmCreateBody = parse_body(body)?;
        let mut allowed_scopes = Vec::with_capacity(body.allowed_scopes.len());
        for raw in &body.allowed_scopes {
            let scope = parse_scope_text(raw)
                .map_err(|error| Response::error(400, &error.code, error.message))?;
            allowed_scopes.push(scope);
        }
        let config = SwarmRunConfig {
            title: body.title,
            envelope: SwarmEnvelope { allowed_scopes },
            max_concurrency: body.max_concurrency,
            max_repair_rounds: body.max_repair_rounds,
            budget_ceiling_units: body.budget_ceiling_units,
            stale_after_ms: body.stale_after_ms,
        };
        let swarm = self
            .open("POST /v1/swarms")?
            .fleet_swarm_create(config, now_ms)
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(201, json!({ "swarm": swarm })))
    }

    // r[impl kiln.fleet.swarm.surface]
    fn get_swarm(&self, id: &str) -> Handled {
        let identity = path_identity(id)?;
        let swarm = self
            .open("GET /v1/swarms/{id}")?
            .fleet_swarm(&identity)
            .map_err(|error| store_error(&error))?
            .ok_or_else(|| Response::error(404, "fleet_swarm_unknown", "no such swarm run"))?;
        Ok(Response::new(200, json!({ "swarm": swarm })))
    }

    // r[impl kiln.fleet.swarm.surface] r[impl kiln.fleet.swarm.delegation]
    fn swarm_member(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let payload: SwarmMemberBody = parse_body(body)?;
        let mut implementer = self.open("POST /v1/swarms/{id}/members")?;
        match payload {
            SwarmMemberBody::UpdateState(update) => {
                if let Some(action) = update.action.as_deref()
                    && action != "update_state"
                {
                    return Err(Response::error(
                        400,
                        "fleet_body",
                        "unknown member action; expected update_state",
                    ));
                }
                self.swarm_member_host_check(&mut implementer, machine, &identity, &update.member)?;
                let member = implementer
                    .fleet_swarm_update_member(
                        &identity,
                        &update.member,
                        update.state,
                        update.budget_draw_delta,
                        now_ms,
                    )
                    .map_err(|error| store_error(&error))?;
                Ok(Response::new(200, json!({ "member": member })))
            }
            SwarmMemberBody::Spawn(spawn) => {
                match spawn.requester.as_deref() {
                    None => self.operator(machine)?,
                    Some(requester) => {
                        self.swarm_member_host_check(
                            &mut implementer,
                            machine,
                            &identity,
                            requester,
                        )?;
                    }
                }
                if spawn.host.as_deref().is_some_and(|host| host != machine) {
                    self.operator(machine)?;
                }
                let scope = match spawn.scope.as_deref() {
                    Some(text) => Some(
                        parse_scope_text(text)
                            .map_err(|error| Response::error(400, &error.code, error.message))?,
                    ),
                    None => None,
                };
                let request = SpawnMemberRequest {
                    name: spawn.name,
                    role: spawn.role,
                    host: spawn.host.unwrap_or_else(|| machine.to_owned()),
                    harness: spawn.harness,
                    model: spawn.model,
                    scope,
                    brief: spawn.brief,
                    attempt: spawn.attempt,
                    claim_identity: spawn.claim_identity,
                    claim_fence: spawn.claim_fence,
                };
                let (member, post) = implementer
                    .fleet_swarm_spawn(&identity, spawn.requester.as_deref(), request, now_ms)
                    .map_err(|error| store_error(&error))?;
                Ok(Response::new(
                    201,
                    json!({ "member": member, "post": post }),
                ))
            }
        }
    }

    // r[impl kiln.fleet.swarm.surface] r[impl kiln.fleet.swarm.model]
    fn read_swarm_board(&self, id: &str, query: Option<&str>) -> Handled {
        let identity = path_identity(id)?;
        let mut since: Option<u64> = None;
        let mut limit: Option<usize> = None;
        let mut seqs: Option<Vec<u64>> = None;
        let mut member: Option<String> = None;
        if let Some(raw) = query
            && !raw.is_empty()
        {
            for pair in raw.split('&') {
                let (key, value) = pair.split_once('=').ok_or_else(|| {
                    Response::error(400, "fleet_query", "malformed query parameter")
                })?;
                match key {
                    "since" => {
                        since = Some(value.parse::<u64>().map_err(|_| {
                            Response::error(
                                400,
                                "fleet_query",
                                "since must be a non-negative integer",
                            )
                        })?);
                    }
                    "limit" => {
                        limit = Some(value.parse::<usize>().map_err(|_| {
                            Response::error(400, "fleet_query", "limit must be a positive integer")
                        })?);
                    }
                    "seqs" => {
                        let mut parsed = Vec::new();
                        for token in value.split(',').filter(|s| !s.is_empty()) {
                            parsed.push(token.parse::<u64>().map_err(|_| {
                                Response::error(
                                    400,
                                    "fleet_query",
                                    "seqs must be comma-separated integers",
                                )
                            })?);
                        }
                        seqs = Some(parsed);
                    }
                    "member" => {
                        if value.is_empty() {
                            return Err(Response::error(
                                400,
                                "fleet_query",
                                "member must not be empty",
                            ));
                        }
                        member = Some(value.to_owned());
                    }
                    _ => {
                        return Err(Response::error(
                            400,
                            "fleet_query",
                            "unknown query parameter on board route",
                        ));
                    }
                }
            }
        }
        let result = self
            .open("GET /v1/swarms/{id}/board")?
            .fleet_swarm_read_board(&identity, member.as_deref(), since, seqs.as_deref(), limit)
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(200, json!(result)))
    }

    // r[impl kiln.fleet.swarm.surface] r[impl kiln.fleet.swarm.model]
    fn post_swarm_board(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let body: SwarmPostBody = parse_body(body)?;
        let mut implementer = self.open("POST /v1/swarms/{id}/board")?;
        self.swarm_member_host_check(&mut implementer, machine, &identity, &body.sender)?;
        let post = implementer
            .fleet_swarm_post(
                &identity,
                &body.sender,
                body.kind,
                body.mentions,
                body.body,
                body.in_reply_to,
                body.candidate_commit,
                now_ms,
            )
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(201, json!({ "post": post })))
    }

    // r[impl kiln.fleet.swarm.surface] r[impl kiln.fleet.swarm.verdict]
    fn swarm_verdict(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let body: SwarmVerdictBody = parse_body(body)?;
        let decision = match body.verdict.as_str() {
            "approve" => VerdictDecision::Approve,
            "request_changes" => VerdictDecision::RequestChanges {
                findings: body.findings,
            },
            "blocked" => VerdictDecision::Blocked {
                reason: body.reason.unwrap_or_default(),
            },
            _ => {
                return Err(Response::error(
                    400,
                    "fleet_swarm_verdict",
                    "verdict must be approve, request_changes, or blocked",
                ));
            }
        };
        let mut implementer = self.open("POST /v1/swarms/{id}/verdicts")?;
        self.swarm_member_host_check(&mut implementer, machine, &identity, &body.verifier)?;
        let (verdict, outcome, post) = implementer
            .fleet_swarm_verdict(
                &identity,
                &body.verifier,
                &body.author,
                body.commit,
                decision,
                now_ms,
            )
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(
            201,
            json!({ "verdict": verdict, "outcome": outcome, "post": post }),
        ))
    }

    // r[impl kiln.fleet.swarm.surface] r[impl kiln.fleet.swarm.watchdog]
    fn swarm_watchdog(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let apply = if body.is_empty() {
            true
        } else {
            let parsed: SwarmWatchdogBody = parse_body(body)?;
            parsed.apply
        };
        if apply {
            self.operator(machine)?;
        }
        let (actions, post) = self
            .open("POST /v1/swarms/{id}/watchdog")?
            .fleet_swarm_watchdog(&identity, apply, now_ms)
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(
            200,
            json!({ "actions": actions, "post": post }),
        ))
    }

    // r[impl kiln.fleet.swarm.surface] r[impl kiln.fleet.swarm.watchdog]
    fn swarm_failover(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        self.operator(machine)?;
        let identity = path_identity(id)?;
        let request: QuotaFailoverRequest = parse_body(body)?;
        let (member, post) = self
            .open("POST /v1/swarms/{id}/failover")?
            .fleet_swarm_failover(&identity, request, now_ms)
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(
            200,
            json!({ "member": member, "post": post }),
        ))
    }

    // r[impl kiln.fleet.swarm.surface]
    fn swarm_graph(&self, id: &str, query: Option<&str>) -> Handled {
        let identity = path_identity(id)?;
        let mut up_to_seq: Option<u64> = None;
        if let Some(raw) = query
            && !raw.is_empty()
        {
            let (key, value) = raw
                .split_once('=')
                .ok_or_else(|| Response::error(400, "fleet_query", "malformed query parameter"))?;
            if key != "up_to_seq" || value.contains('&') {
                return Err(Response::error(
                    400,
                    "fleet_query",
                    "only optional up_to_seq=<u64> is accepted on graph route",
                ));
            }
            up_to_seq = Some(value.parse::<u64>().map_err(|_| {
                Response::error(
                    400,
                    "fleet_query",
                    "up_to_seq must be a non-negative integer",
                )
            })?);
        }
        let graph = self
            .open("GET /v1/swarms/{id}/graph")?
            .fleet_swarm_graph(&identity, up_to_seq)
            .map_err(|error| store_error(&error))?;
        Ok(Response::new(200, json!({ "graph": graph })))
    }
}

fn claim_response(outcome: ClaimOutcome) -> Response {
    match outcome {
        ClaimOutcome::Renewed(claim) => {
            Response::new(200, json!({ "outcome": "renewed", "claim": claim }))
        }
        ClaimOutcome::Released(claim) => {
            Response::new(200, json!({ "outcome": "released", "claim": claim }))
        }
        other => Response::new(200, json!({ "outcome": other })),
    }
}

fn parse_body<T: DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    serde_json::from_slice(body).map_err(|error| {
        Response::error(
            400,
            "fleet_body",
            format!("request body is invalid: {error}"),
        )
    })
}

fn path_identity(id: &str) -> Result<ContentId, Response> {
    ContentId::from_blake3_hex(id).map_err(|_| {
        Response::error(
            400,
            "fleet_identity",
            "path identity is not a BLAKE3 hex id",
        )
    })
}

/// One store refusal as a response.
#[must_use]
pub fn store_error(error: &str) -> Response {
    let class = classify(error);
    let code = error_code(error).unwrap_or("fleet_store");
    Response::error(class.status(), code, error)
}

const fn session_state_label(state: &SessionState) -> &'static str {
    match state {
        SessionState::Placing => "placing",
        SessionState::Starting => "starting",
        SessionState::Prompted => "prompted",
        SessionState::Working => "working",
        SessionState::Blocked { .. } => "blocked",
        SessionState::Ready { .. } => "ready",
        SessionState::HandoffReceived { .. } => "handoff_received",
        SessionState::Captured => "captured",
        SessionState::Failed { .. } => "failed",
        SessionState::Cancelled { .. } => "cancelled",
    }
}

fn herdr_error_response(failure: &HerdrFailure) -> Response {
    let (status, code) = match failure.class {
        FailureClass::Unavailable => (503, "fleet_herdr_unavailable"),
        FailureClass::Rejected if failure.is_code(codes::AGENT_NOT_FOUND) => {
            (404, "fleet_agent_gone")
        }
        FailureClass::Rejected => (409, "fleet_herdr_rejected"),
        FailureClass::Unknown => (502, "fleet_herdr_unknown"),
    };
    Response::new(
        status,
        json!({
            "error": failure.to_string(),
            "code": code,
            "failure": {
                "class": failure.class.as_str(),
                "code": failure.code,
                "detail": failure.detail,
            },
        }),
    )
}

fn console_pointer_response(console_url: Option<&str>) -> Response {
    match console_url {
        Some(raw) => match fleet_console_target(raw) {
            Ok(target) => Response::redirect(target.clone(), redirect_html(&target)),
            Err(error) => Response::error(500, "fleet_console_url", error),
        },
        None => Response::html(200, unconfigured_pointer_html()),
    }
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

fn redirect_html(target: &str) -> String {
    let escaped = escape_html(target);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Kiln Fleet Console</title></head>\n<body>\n<h1>Kiln Fleet</h1>\n<p>The Fleet UI has moved to the unified Kiln Factory Control Plane.</p>\n<p><a href=\"{escaped}\">Open Factory Control Plane (Fleet)</a></p>\n</body>\n</html>\n"
    )
}

fn unconfigured_pointer_html() -> String {
    "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Kiln Fleet Console</title></head>\n<body>\n<h1>Kiln Fleet</h1>\n<p>The standalone Fleet UI has been combined into the unified <code>kiln-web-service</code> Factory Control Plane (<code>/?page=fleet</code>).</p>\n<p>Configure <code>console_url</code> (or <code>KILN_CONSOLE_URL</code> / <code>--console-url</code>) on <code>kiln-fleet</code> to redirect <code>/</code> and <code>/ui</code> to your Factory Control Plane, or use the <code>kiln-fleet</code> CLI (<code>kiln-fleet status</code>, <code>kiln-fleet watch</code>, <code>kiln-fleet claims</code>) and <code>/v1/*</code> API endpoints.</p>\n</body>\n</html>\n".to_owned()
}

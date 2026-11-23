//! Fleet API v1: routing and handlers.
//!
//! Every request is first bound to an authenticated machine (see
//! [`crate::identity`]); handlers then act as that machine. Bodies are
//! bounded JSON with unknown fields refused. Store refusals map to statuses
//! through [`crate::store::classify`]. Returned text written by agents is
//! data, never instructions.

use std::net::IpAddr;
use std::sync::Arc;

use kiln_core::fleet::{
    ClaimCommand, ClaimOutcome, FleetAgent, ForeignClaim, Observation, parse_scope_text,
};
use kiln_core::implementation::Eligibility;
use kiln_core::types::ContentId;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::config::FleetConfig;
use crate::http::{Request, Response};
use crate::identity::PeerIdentity;
use crate::log;
use crate::store::{Store, classify, error_code};
use crate::swarm::SwarmLink;

pub const API_VERSION: &str = "v1";
/// Embedded operator view. Polls the v1 routes from the same origin.
const UI_PAGE: &str = include_str!("ui.html");
pub const MAX_LISTED_WORK: usize = 100;
pub const MAX_LISTED_ATTEMPTS: usize = 256;
pub const MAX_LISTED_DECISIONS: usize = 256;
pub const MS_PER_MINUTE: u64 = 60_000;
/// Handoffs name a full SHA-1 commit, the only form the herdr receipt admits.
pub const HANDOFF_COMMIT_HEX: usize = 40;

/// Shared service state for the API and the dispatch thread.
#[derive(Debug)]
pub struct Service {
    pub config: Arc<FleetConfig>,
    pub store: Store,
    pub identity: PeerIdentity,
    pub swarm: Option<Arc<SwarmLink>>,
    pub clock: fn() -> u64,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenewBody {
    fence: u64,
    #[serde(default)]
    lease_minutes: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseBody {
    fence: u64,
    #[serde(default)]
    note: Option<String>,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnswerBody {
    text: String,
}

type Handled = Result<Response, Response>;

impl Service {
    /// Build the service from a validated configuration.
    ///
    /// # Errors
    /// Returns the enrollment or swarm diagnostic.
    pub fn new(config: FleetConfig) -> Result<Self, String> {
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
        Ok(Self {
            identity: PeerIdentity::new(&config),
            config: Arc::new(config),
            store,
            swarm,
            clock: log::now_ms,
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

    /// Route one request from an authenticated `machine`.
    #[must_use]
    pub fn route(&self, machine: &str, request: &Request, now_ms: u64) -> Response {
        if request.method == "GET" && matches!(request.path.as_str(), "/" | "/ui") {
            return Response::html(200, UI_PAGE.to_owned());
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
            ("GET", ["version"]) => self.state_version(machine, now_ms),
            ("GET", ["claims"]) => self.list_claims(now_ms),
            ("POST", ["claims"]) => self.acquire(machine, &request.body, now_ms),
            ("POST", ["claims", id, "renew"]) => self.renew(machine, id, &request.body, now_ms),
            ("POST", ["claims", id, "release"]) => self.release(machine, id, &request.body, now_ms),
            ("GET", ["work"]) => self.work(now_ms),
            ("GET", ["attempts"]) => self.attempts(now_ms),
            ("POST", ["attempts", id, "handoff"]) => {
                self.handoff(machine, id, &request.body, now_ms)
            }
            ("POST", ["attempts", id, "cancel"]) => self.cancel(machine, id, &request.body, now_ms),
            ("GET", ["decisions"]) => self.decisions(),
            ("GET", ["metrics"]) => self.metrics(now_ms),
            ("POST", ["decisions", id, "answer"]) => {
                self.answer(machine, id, &request.body, now_ms)
            }
            (
                _,
                ["health" | "version" | "claims" | "work" | "attempts" | "decisions" | "metrics"]
                | ["claims", _, "renew" | "release"]
                | ["attempts", _, "handoff" | "cancel"]
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

    /// Open the store for `route`, the handler's `METHOD /v1/...` row in
    /// `ENTRY_POINTS`: only the holder seeds of the operations that row
    /// checks are loaded, so an observation route loads none.
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

    fn list_claims(&self, now_ms: u64) -> Handled {
        let state = self
            .open("GET /v1/claims")?
            .fleet_state()
            .map_err(|error| store_error(&error))?;
        let claims: Vec<_> = state.claims.live(now_ms).cloned().collect();
        Ok(Response::new(200, json!({ "claims": claims })))
    }

    // r[impl kiln.fleet.api.scenario.identity]
    fn acquire(&self, machine: &str, body: &[u8], now_ms: u64) -> Handled {
        let body: ClaimBody = parse_body(body)?;
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

    fn renew(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let body: RenewBody = parse_body(body)?;
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
        Ok(claim_response(outcome))
    }

    fn release(&self, machine: &str, id: &str, body: &[u8], now_ms: u64) -> Handled {
        let identity = path_identity(id)?;
        let body: ReleaseBody = parse_body(body)?;
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
        Ok(claim_response(outcome))
    }

    fn work(&self, now_ms: u64) -> Handled {
        let mut implementer = self.open("GET /v1/work")?;
        let ledger = implementer.work().map_err(|error| store_error(&error))?;
        let ready_unclaimed = implementer
            .fleet_ready_work(now_ms)
            .map_err(|error| store_error(&error))?;
        let find = |identity: &ContentId| {
            ledger
                .current()
                .find(|work| work.identity().is_ok_and(|id| &id == identity))
        };
        let ranked: Vec<Value> = ledger
            .ranked()
            .iter()
            .take(MAX_LISTED_WORK)
            .filter_map(|row| {
                let work = find(&row.identity)?;
                Some(json!({
                    "identity": row.identity,
                    "priority": row.priority,
                    "repository": work.repository,
                    "title": work.title,
                    "permitted_paths": work.permitted_paths,
                    "interactive": work.permitted_effects.contains(kiln_core::implementation::INTERACTIVE_AGENT_EFFECT),
                }))
            })
            .collect();
        let blocked: Vec<Value> = ledger
            .current()
            .filter_map(|work| {
                let key = work.canonical_key().ok()?;
                match ledger.eligibility_of(&key)? {
                    Eligibility::Blocked { reasons } => Some(json!({
                        "identity": work.identity().ok()?,
                        "repository": work.repository,
                        "title": work.title,
                        "reasons": reasons,
                    })),
                    Eligibility::Ready => None,
                }
            })
            .take(MAX_LISTED_WORK)
            .collect();
        Ok(Response::new(
            200,
            json!({ "ready_unclaimed": ready_unclaimed, "ranked": ranked, "blocked": blocked }),
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
                    "session": session,
                })
            })
            .collect();
        Ok(Response::new(200, json!({ "attempts": listed })))
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
        Ok(Response::new(
            200,
            json!({ "attempt": attempt, "fence": current.fence, "cancelled": true, "sessions": sessions }),
        ))
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
        let prompt = self
            .open("POST /v1/decisions/{id}/answer")?
            .fleet_answer_decision(&decision, body.text, machine, true, now_ms)
            .map_err(|error| store_error(&error))?;
        log::info(
            "api.answer",
            json!({ "decision": decision, "machine": machine }),
        );
        Ok(Response::new(
            200,
            json!({ "decision": decision, "answered": true, "prompt": prompt }),
        ))
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

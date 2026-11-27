//! Placement shell on the existing implementation and fleet state owner.

use kiln_core::fleet::placement::{
    MovePlan, MoveRequest, SplitPlan, SplitRequest, SplitUnit, plan_move, queued_move_source,
    work_scopes,
};
use kiln_core::fleet::{AttemptScope, ClaimCommand, ClaimContext, ClaimOutcome, FleetAgent};
use kiln_core::implementation::{
    AttemptLedger, ImplementationAttempt, ImplementationDispatchPlan, ImplementationWork,
    TransportLaunches, WorkLedger,
};
use kiln_core::types::ContentId;
use rusqlite::TransactionBehavior;
use serde::{Deserialize, Serialize};

use crate::authority_bridge::DispatchGate;
use crate::fleet::{FLEET_STATE_KIND, FLEET_STATE_SCHEMA, FleetState};
use crate::{
    ATTEMPT_KIND, Implementer, TRANSPORT_LAUNCH_KIND, WORK_KIND, authorize_reservation, diagnostic,
    load, save, text,
};

const SPLIT_KIND_PREFIX: &str = "fleet-placement-split-v1:";
const SPLIT_UNIT_PREFIX: &str = "fleet-placement-unit-v1:";
const MOVE_KIND_PREFIX: &str = "fleet-placement-move-v1:";
const MOVED_ATTEMPT_PREFIX: &str = "fleet-placement-moved-attempt-v1:";

#[derive(Debug, Serialize)]
pub struct MoveStatus {
    pub plan: MovePlan,
    pub state: MoveState,
    pub session: Option<ContentId>,
    pub observed_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveState {
    Requested,
    Placing,
    Observed,
    Ended,
    ReconcileRequired,
    Superseded,
}

fn move_row(identity: &ContentId) -> String {
    format!("{MOVE_KIND_PREFIX}{}", identity.as_str())
}

pub(crate) fn moved_attempt_row(identity: &ContentId) -> String {
    format!("{MOVED_ATTEMPT_PREFIX}{}", identity.as_str())
}

fn move_status(
    plan: MovePlan,
    attempts: &AttemptLedger,
    fleet: &FleetState,
    now_ms: u64,
) -> MoveStatus {
    let id = &plan.request.attempt;
    let session = fleet.sessions.for_attempt(id);
    let exact = session.filter(|session| {
        session.machine == plan.request.destination
            && session.fence == plan.request.fence
            && session.work == plan.work
            && session.work_revision == plan.work_revision
    });
    let observed =
        exact.filter(|session| session.agent_name.is_some() && session.workspace.is_some());
    let current = attempts.is_current(id, plan.request.fence, now_ms);
    let issued = attempts.dispatch_requires_recovery(id) && !attempts.termination_observed(id);
    let state = if !attempts
        .get(id)
        .and_then(|attempt| attempt.dispatch.as_ref())
        .is_some_and(|reservation| reservation.effect_id == plan.new_reservation)
    {
        MoveState::Superseded
    } else if attempts.unresolved().any(|attempt| &attempt.identity == id)
        || (session.is_some() && exact.is_none())
        || (observed.is_none() && issued)
    {
        MoveState::ReconcileRequired
    } else if exact.is_some_and(|session| session.state.is_ended()) || (!current && !issued) {
        MoveState::Ended
    } else if !current {
        MoveState::ReconcileRequired
    } else if observed.is_some() {
        MoveState::Observed
    } else if exact.is_some() {
        MoveState::Placing
    } else {
        MoveState::Requested
    };
    let followed = state != MoveState::Superseded;
    MoveStatus {
        session: exact
            .filter(|_| followed)
            .map(|session| session.identity.clone()),
        observed_ms: observed
            .filter(|_| followed)
            .map(|session| session.observed_ms),
        plan,
        state,
    }
}

#[derive(Debug, Serialize)]
pub struct SplitStatus {
    pub plan: SplitPlan,
    pub units: Vec<UnitStatus>,
}

#[derive(Debug, Serialize)]
pub struct UnitStatus {
    pub attempt: ContentId,
    pub state: UnitState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<ContentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_ms: Option<u64>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitState {
    Requested,
    Placing,
    Observed,
    Ended,
    ReconcileRequired,
}

fn row(identity: &ContentId) -> String {
    format!("{SPLIT_KIND_PREFIX}{}", identity.as_str())
}

fn unit_row(identity: &ContentId) -> String {
    format!("{SPLIT_UNIT_PREFIX}{}", identity.as_str())
}

fn status(
    plan: SplitPlan,
    attempts: &AttemptLedger,
    fleet: &FleetState,
    now_ms: u64,
) -> SplitStatus {
    let units = plan
        .units
        .iter()
        .map(|unit| {
            let session = fleet.sessions.for_attempt(&unit.attempt).filter(|session| {
                session.machine == unit.destination
                    && session.fence == unit.fence
                    && session.work == unit.work
                    && session.work_revision == unit.work_revision
            });
            let observed = session
                .filter(|session| session.agent_name.is_some() && session.workspace.is_some());
            let current = attempts.is_current(&unit.attempt, unit.fence, now_ms);
            let unknown = attempts
                .unresolved()
                .any(|attempt| attempt.identity == unit.attempt);
            let state = if unknown
                || (observed.is_none() && attempts.dispatch_requires_recovery(&unit.attempt))
                || fleet.sessions.for_attempt(&unit.attempt).is_some() && session.is_none()
            {
                UnitState::ReconcileRequired
            } else if session.is_some_and(|session| session.state.is_ended()) {
                UnitState::Ended
            } else if !current {
                if session.is_some() {
                    UnitState::ReconcileRequired
                } else {
                    UnitState::Ended
                }
            } else if observed.is_some() {
                UnitState::Observed
            } else if session.is_some() {
                UnitState::Placing
            } else {
                UnitState::Requested
            };
            UnitStatus {
                attempt: unit.attempt.clone(),
                state,
                session: session.map(|s| s.identity.clone()),
                observed_ms: observed.map(|s| s.observed_ms),
            }
        })
        .collect();
    SplitStatus { plan, units }
}

impl Implementer {
    /// Whether `attempt_id` at `fence` satisfies every owner-side queued-move
    /// admission check enforced by [`Self::fleet_prepare_move`] and
    /// [`queued_move_source`], including re-moving an already-moved unstarted attempt.
    pub fn fleet_queued_move_eligible(
        &mut self,
        attempt_id: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<bool, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(text)?;
        if require_native_admission(&tx).is_err() {
            return Ok(false);
        }
        let (split, _) = load::<Option<ContentId>>(&tx, &unit_row(attempt_id))?;
        if split.is_some() {
            return Ok(false);
        }
        let (launches, _) = load::<TransportLaunches>(&tx, TRANSPORT_LAUNCH_KIND)?;
        if launches.recall(attempt_id).is_some() {
            return Ok(false);
        }
        let (fleet, _) = load::<FleetState>(&tx, FLEET_STATE_KIND)?;
        if fleet.schema != FLEET_STATE_SCHEMA {
            return Err("fleet_state_schema: fleet state schema is unsupported".into());
        }
        if fleet.sessions.for_attempt(attempt_id).is_some() {
            return Ok(false);
        }
        let (attempts, _) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        let Some(attempt) = attempts.get(attempt_id) else {
            return Ok(false);
        };
        let Some(reservation) = attempt.dispatch.as_ref() else {
            return Ok(false);
        };
        let (work, _) = load::<WorkLedger>(&tx, WORK_KIND)?;
        let Some(work) = work.get(&attempt.canonical_key) else {
            return Ok(false);
        };
        let probe = MoveRequest {
            attempt: attempt_id.clone(),
            fence,
            reservation: reservation.effect_id.clone(),
            destination: "__probe__".into(),
        };
        let eligible = queued_move_source(&attempts, work, &probe, now_ms).is_ok();
        tx.commit().map_err(text)?;
        Ok(eligible)
    }

    /// Read one immutable rebind receipt and its current provider observation.
    pub fn fleet_move_status(
        &mut self,
        identity: &ContentId,
        now_ms: u64,
    ) -> Result<Option<MoveStatus>, String> {
        identity.validate().map_err(diagnostic)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(text)?;
        let (plan, _) = load::<Option<MovePlan>>(&tx, &move_row(identity))?;
        let result = if let Some(plan) = plan {
            let (attempts, _) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
            let (fleet, _) = load::<FleetState>(&tx, FLEET_STATE_KIND)?;
            Some(move_status(plan, &attempts, &fleet, now_ms))
        } else {
            None
        };
        tx.commit().map_err(text)?;
        Ok(result)
    }

    /// Compare the exact queued source, admit its replacement and persist the receipt
    /// in one transaction with the session/transport guards. The native planner
    /// uses collected observations only; no provider effects run inside the write.
    pub fn fleet_prepare_move(
        &mut self,
        request: &MoveRequest,
        expected: Option<&ContentId>,
        now_ms: u64,
        native: impl FnOnce(
            &ImplementationWork,
            &ImplementationAttempt,
            &ImplementationDispatchPlan,
        ) -> Result<(String, ContentId), String>,
    ) -> Result<MoveStatus, String> {
        request.validate().map_err(diagnostic)?;
        let tx = self
            .connection
            .transaction_with_behavior(if expected.is_some() {
                TransactionBehavior::Immediate
            } else {
                TransactionBehavior::Deferred
            })
            .map_err(text)?;
        let (mut attempts, version) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        let (fleet, _) = load::<FleetState>(&tx, FLEET_STATE_KIND)?;
        if fleet.schema != FLEET_STATE_SCHEMA {
            return Err("fleet_state_schema: fleet state schema is unsupported".into());
        }
        let receipt = expected.map(move_row);
        if let Some(key) = &receipt {
            let (plan, _) = load::<Option<MovePlan>>(&tx, key)?;
            if let Some(plan) = plan {
                if &plan.request != request {
                    return Err(
                        "fleet_placement_changed: request does not match the retained move".into(),
                    );
                }
                let result = move_status(plan, &attempts, &fleet, now_ms);
                tx.commit().map_err(text)?;
                return Ok(result);
            }
        }
        require_native_admission(&tx)?;
        let (split, _) = load::<Option<ContentId>>(&tx, &unit_row(&request.attempt))?;
        let (launches, _) = load::<TransportLaunches>(&tx, TRANSPORT_LAUNCH_KIND)?;
        if split.is_some()
            || launches.recall(&request.attempt).is_some()
            || fleet.sessions.for_attempt(&request.attempt).is_some()
        {
            return Err("fleet_placement_source: split lineage or reserved session/launch evidence prevents queued rebind".into());
        }
        let (work, _) = load::<WorkLedger>(&tx, WORK_KIND)?;
        let stale = "fleet_placement_stale: attempt or current work is missing";
        let work = attempts
            .get(&request.attempt)
            .and_then(|attempt| work.get(&attempt.canonical_key))
            .ok_or(stale)?;
        let (attempt, old) =
            queued_move_source(&attempts, work, request, now_ms).map_err(diagnostic)?;
        let (source, workspace) = native(work, attempt, old)?;
        if source == request.destination {
            return Err("fleet_placement_source: destination is already reserved".into());
        }
        let replacement = attempts
            .rebind_queued_dispatch(work, request, now_ms, workspace)
            .map_err(diagnostic)?;
        let attempt = attempts.get(&request.attempt).ok_or(stale)?;
        let plan = plan_move(request, source, attempt, &replacement).map_err(diagnostic)?;
        if expected.is_some_and(|id| *id != plan.identity) {
            return Err(
                "fleet_placement_changed: re-preview the move; the exact plan changed".into(),
            );
        }
        if let Some(key) = receipt {
            let mut gate = DispatchGate {
                authority: &mut self.authority,
                profile: &self.authority_profile,
                last: &mut self.authority_last,
                launches: &launches,
                now_ms,
            };
            authorize_reservation(&mut gate, work, attempt, &replacement)?;
            attempts
                .bind_transport_start(&request.attempt)
                .map_err(diagnostic)?;
            let pointer = moved_attempt_row(&request.attempt);
            let (_, pointer_version) = load::<Option<MovePlan>>(&tx, &pointer)?;
            save(&tx, ATTEMPT_KIND, version, &attempts)?;
            save(&tx, &pointer, pointer_version, &plan)?;
            save(&tx, &key, 0, &plan)?;
        }
        let result = move_status(plan, &attempts, &fleet, now_ms);
        tx.commit().map_err(text)?;
        Ok(result)
    }

    /// Return durable intent and exact native observations from the same snapshot.
    pub fn fleet_split_status(
        &mut self,
        identity: &ContentId,
        now_ms: u64,
    ) -> Result<Option<SplitStatus>, String> {
        identity.validate().map_err(diagnostic)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(text)?;
        let (plan, _) = load::<Option<SplitPlan>>(&transaction, &row(identity))?;
        let result = if let Some(plan) = plan {
            let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
            let (fleet, _) = load::<FleetState>(&transaction, FLEET_STATE_KIND)?;
            Some(status(plan, &attempts, &fleet, now_ms))
        } else {
            None
        };
        transaction.commit().map_err(text)?;
        Ok(result)
    }

    /// Revalidate explicit placement. `None` means standard dispatch;
    /// `Some(true)` means we already issued the single launch intent.
    pub fn fleet_check_explicit_launch(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<Option<bool>, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(text)?;
        let (split, _) = load::<Option<ContentId>>(&transaction, &unit_row(identity))?;
        let (moved, _) = load::<Option<MovePlan>>(&transaction, &moved_attempt_row(identity))?;
        let issued = if split.is_some() || moved.is_some() {
            require_native_admission(&transaction)?;
            let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
            let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
            let (_, record) =
                explicit_work(&work, &attempts, identity, fence, now_ms, moved.as_ref())?;
            if split.is_some() {
                let attempt = attempts
                    .get(identity)
                    .ok_or("fleet_placement_stale: attempt missing")?;
                kiln_core::fleet::placement::check_split_dependencies(
                    &work, &attempts, record, attempt,
                )
                .map_err(diagnostic)?;
            }
            Some(attempts.dispatch_requires_recovery(identity))
        } else {
            None
        };
        transaction.commit().map_err(text)?;
        Ok(issued)
    }

    /// New effects are single-shot. An already-recorded native launch must
    /// be reconciled by observation only, even after restart or lease loss.
    pub fn fleet_begin_explicit_launch(
        &mut self,
        session: &kiln_core::fleet::FleetSession,
        now_ms: u64,
    ) -> Result<bool, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        require_native_admission(&transaction)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (mut attempts, version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (split, _) = load::<Option<ContentId>>(&transaction, &unit_row(&session.attempt))?;
        let (moved, _) =
            load::<Option<MovePlan>>(&transaction, &moved_attempt_row(&session.attempt))?;
        let (fleet, _) = load::<FleetState>(&transaction, FLEET_STATE_KIND)?;
        if session.state != kiln_core::fleet::SessionState::Placing
            || fleet.sessions.sessions.get(&session.identity) != Some(session)
        {
            return Err("fleet_placement_stale: the native session changed before launch".into());
        }
        let (attempt, record) = explicit_work(
            &work,
            &attempts,
            &session.attempt,
            session.fence,
            now_ms,
            moved.as_ref(),
        )?;
        let (schema, lineage) = if let Some(moved) = &moved {
            if moved.request.destination != session.machine {
                return Err(
                    "fleet_placement_stale: bound session differs from moved destination".into(),
                );
            }
            ("kiln.fleet-move-herdr-launch.v1", &moved.identity)
        } else if let Some(split) = &split {
            kiln_core::fleet::placement::check_split_dependencies(
                &work, &attempts, record, attempt,
            )
            .map_err(diagnostic)?;
            ("kiln.fleet-split-herdr-launch.v1", split)
        } else {
            return Err("fleet_placement_source: explicit placement lineage missing".into());
        };
        let reservation = attempt
            .dispatch
            .as_ref()
            .ok_or("fleet_placement_stale: native reservation missing")?
            .plan
            .clone();
        let digest = blake3::hash(
            format!(
                "{schema}\n{}\n{}\n{}\n{}\n{}",
                lineage.as_str(),
                reservation.as_str(),
                session.identity.as_str(),
                session.machine,
                session.kind
            )
            .as_bytes(),
        );
        let request =
            ContentId::from_blake3_hex(digest.to_hex().to_string()).map_err(diagnostic)?;
        attempts
            .bind_dispatch_request(record, &session.attempt, session.fence, now_ms, &request)
            .map_err(diagnostic)?;
        let fresh = !attempts
            .begin_dispatch_transport(&session.attempt, session.fence, now_ms, &reservation)
            .map_err(diagnostic)?;
        if fresh {
            save(&transaction, ATTEMPT_KIND, version, &attempts)?;
        }
        transaction.commit().map_err(text)?;
        Ok(fresh)
    }

    /// Preview or reserve the entire split. The provider plan closure must be
    /// pure (over a pre-collected inventory and occupancy snapshot).
    /// Commit exact native reservations, lease claims and lineage together,
    /// before the existing dispatcher can see any unit. No transport here.
    pub fn fleet_prepare_split(
        &mut self,
        request: &SplitRequest,
        expected: Option<&ContentId>,
        context: ClaimContext<'_>,
        claim_lease_ms: u64,
        native: impl FnMut(
            &SplitUnit,
            &ImplementationWork,
            &ImplementationAttempt,
        ) -> Result<ImplementationDispatchPlan, String>,
    ) -> Result<SplitStatus, String> {
        let ordered = request.ordered().map_err(diagnostic)?;
        let transaction = self
            .connection
            .transaction_with_behavior(if expected.is_some() {
                TransactionBehavior::Immediate
            } else {
                TransactionBehavior::Deferred
            })
            .map_err(text)?;
        let (mut attempts, version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (mut fleet, fleet_version) = load::<FleetState>(&transaction, FLEET_STATE_KIND)?;
        if fleet.schema != FLEET_STATE_SCHEMA {
            return Err("fleet_state_schema: fleet state schema is unsupported".into());
        }
        let receipt_row = expected.map(row);
        if let Some(key) = &receipt_row {
            let (plan, _) = load::<Option<SplitPlan>>(&transaction, key)?;
            if let Some(plan) = plan {
                if !plan.matches_units(&ordered) {
                    return Err(
                        "fleet_placement_changed: submitted units do not match the reserved plan"
                            .into(),
                    );
                }
                let result = status(plan, &attempts, &fleet, context.now_ms);
                transaction.commit().map_err(text)?;
                return Ok(result);
            }
        }
        require_native_admission(&transaction)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (launches, _) = load::<TransportLaunches>(&transaction, TRANSPORT_LAUNCH_KIND)?;
        let mut native = native;
        let mut inputs = Vec::with_capacity(ordered.len());
        for unit in ordered {
            let (work, attempt) =
                kiln_core::fleet::placement::split_work(&work, &attempts, unit, context.now_ms)
                    .map_err(diagnostic)?;
            if launches.recall(&unit.attempt).is_some()
                || fleet.sessions.for_attempt(&unit.attempt).is_some()
            {
                return Err(
                    "fleet_placement_source: attempt has retained launch or session evidence"
                        .into(),
                );
            }
            inputs.push((unit, work, attempt, native(unit, work, attempt)?));
        }
        let plan = kiln_core::fleet::placement::plan_split(
            inputs
                .iter()
                .map(|(unit, work, attempt, native)| (*unit, *work, *attempt, native)),
        )
        .map_err(diagnostic)?;
        if expected.is_some_and(|identity| *identity != plan.identity) {
            return Err(
                "fleet_placement_changed: re-preview the split; the exact plan changed".into(),
            );
        }
        let native_plans: Vec<_> = inputs
            .into_iter()
            .map(|(_, work, _, native)| (work, native))
            .collect();
        let scopes = claim_scopes(&work, &attempts)?;
        let context = ClaimContext {
            attempts: &scopes,
            ..context
        };
        let mut gate = DispatchGate {
            authority: &mut self.authority,
            profile: &self.authority_profile,
            last: &mut self.authority_last,
            launches: &launches,
            now_ms: context.now_ms,
        };
        for (unit, (work, native)) in plan.units.iter().zip(native_plans) {
            for scope in &unit.scopes {
                let command = ClaimCommand::Acquire {
                    scope: scope.clone(),
                    holder: FleetAgent {
                        tool: "kiln".into(),
                        host: context.coordinator.into(),
                        session: unit.attempt.as_str()[..12].into(),
                    },
                    purpose: "Kiln split placement".into(),
                    lease_ms: claim_lease_ms,
                    attempt: Some(unit.attempt.clone()),
                };
                let (claims, outcome, _) =
                    kiln_core::fleet::decide(&fleet.claims, command, &context)
                        .map_err(diagnostic)?;
                if !matches!(outcome, ClaimOutcome::Granted(_)) {
                    return Err("fleet_placement_overlap: split unit overlaps a live or unresolved execution or fleet claim".into());
                }
                fleet.claims = claims;
            }
            let reservation = attempts
                .reserve_dispatch_plan(work, &unit.attempt, unit.fence, context.now_ms, &native)
                .map_err(diagnostic)?;
            if expected.is_some() {
                attempts
                    .bind_transport_start(&unit.attempt)
                    .map_err(diagnostic)?;
                let attempt = attempts
                    .get(&unit.attempt)
                    .ok_or("fleet_placement_stale: attempt disappeared")?;
                authorize_reservation(&mut gate, work, attempt, &reservation)?;
            }
        }
        if let Some(key) = receipt_row {
            save(&transaction, ATTEMPT_KIND, version, &attempts)?;
            save(&transaction, FLEET_STATE_KIND, fleet_version, &fleet)?;
            save(&transaction, &key, 0, &plan)?;
            for unit in &plan.units {
                save(&transaction, &unit_row(&unit.attempt), 0, &plan.identity)?;
            }
        }
        let result = status(plan, &attempts, &fleet, context.now_ms);
        transaction.commit().map_err(text)?;
        Ok(result)
    }
}

fn explicit_work<'work, 'attempt>(
    work: &'work WorkLedger,
    attempts: &'attempt AttemptLedger,
    identity: &ContentId,
    fence: u64,
    now_ms: u64,
    moved: Option<&MovePlan>,
) -> Result<(&'attempt ImplementationAttempt, &'work ImplementationWork), String> {
    let stale =
        "fleet_placement_stale: explicit attempt, work, fence or reservation changed before launch";
    let attempt = attempts
        .get(identity)
        .filter(|_| attempts.is_current(identity, fence, now_ms))
        .ok_or(stale)?;
    let record = work.get(&attempt.canonical_key).ok_or(stale)?;
    if let Some(moved) = moved
        && (attempt.superseded_by.is_some()
            || record.revision != attempt.work_revision
            || record.identity().map_err(diagnostic)? != attempt.work_identity
            || attempt
                .dispatch
                .as_ref()
                .is_none_or(|reservation| reservation.effect_id != moved.new_reservation))
    {
        return Err(stale.into());
    }
    Ok((attempt, record))
}

fn require_native_admission(transaction: &rusqlite::Transaction<'_>) -> Result<(), String> {
    let (admission, _) =
        load::<kiln_core::fair_admission::AdmissionLedger>(transaction, super::ADMISSION_KIND)?;
    if admission.policy.is_some() {
        return Err("fleet_placement_unavailable: the configured admission owner does not expose explicit reservation".into());
    }
    Ok(())
}

fn claim_scopes(work: &WorkLedger, attempts: &AttemptLedger) -> Result<Vec<AttemptScope>, String> {
    let mut scopes = Vec::new();
    for attempt in attempts
        .attempts()
        .filter(|attempt| attempt.phase.is_active())
        .chain(
            attempts
                .unresolved()
                .filter(|attempt| !attempt.phase.is_active()),
        )
    {
        let frozen = work.get(&attempt.canonical_key).into_iter().chain(work.superseded()).find(|candidate| {
            candidate.revision == attempt.work_revision && candidate.identity().is_ok_and(|id| id == attempt.work_identity)
        }).ok_or("fleet_placement_source: cannot resolve scopes of another active or unresolved attempt")?;
        let work = work_scopes(frozen);
        if work.is_empty() {
            return Err("fleet_placement_source: cannot resolve scopes of another active or unresolved attempt".into());
        }
        scopes.extend(work.into_iter().map(|scope| AttemptScope {
            attempt: attempt.identity.clone(),
            scope,
        }));
    }
    Ok(scopes)
}

//! Hosted retained-mobility shell for moving a running guest workload.
//!
//! Defines versioned ports for Aspen exclusive placement fencing
//! (`fence-guest-placement-ownership`) and Lattice epoch-scoped guest reconnect
//! (`reconnect-restored-guest-sessions`), plus durable step-by-step journaling.
//! By default, missing Aspen or Lattice prerequisites fail closed before any
//! durable write or external effect.

use kiln_core::fleet::guest_move::{
    DestinationAdmission, DestinationPlacementGrant, GuestCompatibilityProfile, GuestMoveEffect,
    GuestMoveOutcome, GuestMovePlan, GuestMoveReconcileObservation, GuestMoveRequest,
    GuestMoveSource, GuestMoveState, GuestMoveStepKind, GuestMoveStepObservation,
    GuestMoveStepStatus, ReconnectConfirmation, plan_guest_move,
};
use kiln_core::types::ContentId;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::microvm::retained::SELECTED_MOLTEN_REVISION;

pub const ASPEN_PLACEMENT_FENCE_PORT: &str = "aspen.fence-guest-placement-ownership.v1";
pub const LATTICE_GUEST_RECONNECT_PORT: &str = "lattice.reconnect-restored-guest-sessions.v1";
pub const GUEST_RUNTIME_PORT: &str = "kiln.retained-guest-mobility.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StepReply<T> {
    Confirmed(T),
    LostReply { cause: String },
    Failed { cause: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativePlacementObservation {
    DestinationOwned(DestinationPlacementGrant),
    SourceOwned,
    StillUnknown { cause: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeReconnectObservation {
    Reconnected(ReconnectConfirmation),
    NotConnected { cause: String },
    StillUnknown { cause: String },
}

/// Versioned port for Aspen exclusive guest placement and resource fencing
/// (`fence-guest-placement-ownership`).
pub trait AspenPlacementFencePort {
    fn port_version(&self) -> &'static str {
        ASPEN_PLACEMENT_FENCE_PORT
    }

    fn check_available(&self) -> Result<(), String>;

    fn admit_destination(
        &self,
        destination: &str,
        profile: &GuestCompatibilityProfile,
    ) -> Result<DestinationAdmission, String>;

    fn grant_destination_placement(
        &mut self,
        plan: &GuestMovePlan,
    ) -> Result<StepReply<DestinationPlacementGrant>, String>;

    fn fence_source_resources(
        &mut self,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
    ) -> Result<StepReply<ContentId>, String>;

    fn read_native_placement(
        &self,
        plan: &GuestMovePlan,
    ) -> Result<NativePlacementObservation, String>;
}

/// Versioned port for Lattice epoch-scoped restored guest reconnect
/// (`reconnect-restored-guest-sessions`).
pub trait LatticeGuestReconnectPort {
    fn port_version(&self) -> &'static str {
        LATTICE_GUEST_RECONNECT_PORT
    }

    fn check_available(&self) -> Result<(), String>;

    fn reconnect_restored_guest(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
    ) -> Result<StepReply<ReconnectConfirmation>, String>;

    fn read_native_reconnect(
        &self,
        plan: &GuestMovePlan,
    ) -> Result<NativeReconnectObservation, String>;
}

/// Versioned port for retained Cloud Hypervisor + Iroh guest mobility effects.
pub trait GuestRuntimePort {
    fn port_version(&self) -> &'static str {
        GUEST_RUNTIME_PORT
    }

    fn pause_source(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
    ) -> Result<StepReply<ContentId>, String>;

    fn capture_checkpoint(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
    ) -> Result<StepReply<ContentId>, String>;

    fn transfer_checkpoint(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        checkpoint: &ContentId,
    ) -> Result<StepReply<ContentId>, String>;

    fn restore_destination(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
    ) -> Result<StepReply<ContentId>, String>;

    fn retire_source(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
        reconnect: &ReconnectConfirmation,
    ) -> Result<StepReply<ContentId>, String>;

    fn read_native_step(
        &self,
        plan: &GuestMovePlan,
        step: GuestMoveStepKind,
    ) -> Result<GuestMoveReconcileObservation, String>;
}

/// Default Aspen port until `fence-guest-placement-ownership` (Aspen #15) lands.
/// Always fails closed before any journal write or host/network effect.
#[derive(Clone, Debug, Default)]
pub struct UnavailableAspenPlacementFence;

impl AspenPlacementFencePort for UnavailableAspenPlacementFence {
    fn check_available(&self) -> Result<(), String> {
        Err(format!(
            "guest_move_unsupported: Aspen {SELECTED_MOLTEN_REVISION} lacks fence-guest-placement-ownership (Aspen #15); refusing running guest move before any effect"
        ))
    }

    fn admit_destination(
        &self,
        _destination: &str,
        _profile: &GuestCompatibilityProfile,
    ) -> Result<DestinationAdmission, String> {
        self.check_available()?;
        unreachable!("check_available always fails closed")
    }

    fn grant_destination_placement(
        &mut self,
        _plan: &GuestMovePlan,
    ) -> Result<StepReply<DestinationPlacementGrant>, String> {
        self.check_available()?;
        unreachable!("check_available always fails closed")
    }

    fn fence_source_resources(
        &mut self,
        _plan: &GuestMovePlan,
        _grant: &DestinationPlacementGrant,
    ) -> Result<StepReply<ContentId>, String> {
        self.check_available()?;
        unreachable!("check_available always fails closed")
    }

    fn read_native_placement(
        &self,
        _plan: &GuestMovePlan,
    ) -> Result<NativePlacementObservation, String> {
        self.check_available()?;
        unreachable!("check_available always fails closed")
    }
}

/// Default Lattice port until `reconnect-restored-guest-sessions` lands.
/// Always fails closed before any journal write or host/network effect.
#[derive(Clone, Debug, Default)]
pub struct UnavailableLatticeGuestReconnect;

impl LatticeGuestReconnectPort for UnavailableLatticeGuestReconnect {
    fn check_available(&self) -> Result<(), String> {
        Err(
            "guest_move_unsupported: Lattice lacks reconnect-restored-guest-sessions; refusing running guest move before any effect"
                .into(),
        )
    }

    fn reconnect_restored_guest(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
        _grant: &DestinationPlacementGrant,
    ) -> Result<StepReply<ReconnectConfirmation>, String> {
        self.check_available()?;
        unreachable!("check_available always fails closed")
    }

    fn read_native_reconnect(
        &self,
        _plan: &GuestMovePlan,
    ) -> Result<NativeReconnectObservation, String> {
        self.check_available()?;
        unreachable!("check_available always fails closed")
    }
}

fn durable_database(db: &Connection) -> Result<(), String> {
    let synchronous: u32 = db
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let journal: String = db
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if synchronous < 2 || matches!(journal.as_str(), "off" | "memory") {
        return Err(
            "guest move journal requires a file-backed SQLite journal with synchronous FULL or EXTRA"
                .into(),
        );
    }
    Ok(())
}

pub fn init(db: &Connection) -> Result<(), String> {
    durable_database(db)?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS hosted_guest_move(
            plan TEXT PRIMARY KEY,
            attempt TEXT NOT NULL,
            session TEXT NOT NULL,
            source_host TEXT NOT NULL,
            destination_host TEXT NOT NULL,
            outcome TEXT NOT NULL,
            state_json TEXT NOT NULL,
            updated_ms INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS hosted_guest_move_active_attempt
            ON hosted_guest_move(attempt) WHERE outcome IN ('planned','in_progress','uncertain');
        CREATE UNIQUE INDEX IF NOT EXISTS hosted_guest_move_active_session
            ON hosted_guest_move(session) WHERE outcome IN ('planned','in_progress','uncertain');
        CREATE TABLE IF NOT EXISTS hosted_guest_move_step(
            plan TEXT NOT NULL,
            step_order INTEGER NOT NULL,
            step_kind TEXT NOT NULL,
            step_identity TEXT NOT NULL,
            status TEXT NOT NULL,
            evidence TEXT,
            diagnostic TEXT,
            PRIMARY KEY(plan, step_kind),
            FOREIGN KEY(plan) REFERENCES hosted_guest_move(plan)
        );",
    )
    .map_err(|e| e.to_string())
}

fn outcome_label(outcome: GuestMoveOutcome) -> &'static str {
    match outcome {
        GuestMoveOutcome::Planned => "planned",
        GuestMoveOutcome::InProgress => "in_progress",
        GuestMoveOutcome::Completed => "completed",
        GuestMoveOutcome::Uncertain => "uncertain",
        GuestMoveOutcome::RestoreFailedPaused => "restore_failed_paused",
        GuestMoveOutcome::Aborted => "aborted",
    }
}

fn status_label(status: GuestMoveStepStatus) -> &'static str {
    match status {
        GuestMoveStepStatus::Pending => "pending",
        GuestMoveStepStatus::Requested => "requested",
        GuestMoveStepStatus::Observed => "observed",
        GuestMoveStepStatus::Uncertain => "uncertain",
        GuestMoveStepStatus::Failed => "failed",
    }
}

fn persist_state(db: &mut Connection, state: &GuestMoveState, now_ms: u64) -> Result<(), String> {
    let encoded = serde_json::to_string(state).map_err(|e| e.to_string())?;
    let updated = crate::sqlite_integer(now_ms)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO hosted_guest_move(plan,attempt,session,source_host,destination_host,outcome,state_json,updated_ms)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(plan) DO UPDATE SET outcome=excluded.outcome,state_json=excluded.state_json,updated_ms=excluded.updated_ms",
        params![
            state.plan.identity.as_str(),
            state.plan.request.attempt.as_str(),
            state.plan.request.session.as_str(),
            state.plan.source_host,
            state.plan.request.destination,
            outcome_label(state.outcome),
            encoded,
            updated,
        ],
    )
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("UNIQUE constraint failed") {
            "guest_move_duplicate_source: source attempt or session already has an active or unresolved guest move".to_string()
        } else {
            msg
        }
    })?;
    for (order, step) in state.plan.steps.iter().enumerate() {
        tx.execute(
            "INSERT INTO hosted_guest_move_step(plan,step_order,step_kind,step_identity,status,evidence,diagnostic)
             VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(plan,step_kind) DO UPDATE SET status=excluded.status,evidence=excluded.evidence,diagnostic=excluded.diagnostic",
            params![
                state.plan.identity.as_str(),
                order as i64,
                step.kind.as_str(),
                step.identity.as_str(),
                status_label(step.status),
                step.evidence.as_ref().map(ContentId::as_str),
                step.diagnostic.as_deref(),
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn load_move(db: &Connection, plan: &ContentId) -> Result<Option<GuestMoveState>, String> {
    plan.validate().map_err(|e| format!("{e:?}"))?;
    let row: Option<String> = db
        .query_row(
            "SELECT state_json FROM hosted_guest_move WHERE plan=?1",
            [plan.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    row.map(|json| serde_json::from_str(&json).map_err(|e| e.to_string()))
        .transpose()
}

fn active_source_moves(db: &Connection) -> Result<Vec<ContentId>, String> {
    let mut stmt = db
        .prepare("SELECT attempt,session FROM hosted_guest_move WHERE outcome IN ('planned','in_progress','uncertain')")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut active = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let attempt: String = row.get(0).map_err(|e| e.to_string())?;
        let session: String = row.get(1).map_err(|e| e.to_string())?;
        active.push(ContentId::from_blake3_hex(attempt).map_err(|e| format!("{e:?}"))?);
        active.push(ContentId::from_blake3_hex(session).map_err(|e| format!("{e:?}"))?);
    }
    Ok(active)
}

/// Plan and execute a running guest move through ordered durable steps.
/// Fails closed before any durable write or external effect if Aspen placement
/// fencing, Lattice reconnect, or destination admission is unavailable.
// r[impl kiln.sdlc.guest_move]
// r[impl kiln.sdlc.guest_move.scenario.restored]
// r[impl kiln.sdlc.guest_move.scenario.uncertain]
pub fn execute_move<A, L, R>(
    db: &mut Connection,
    aspen: &mut A,
    lattice: &mut L,
    runtime: &mut R,
    source: &GuestMoveSource,
    request: &GuestMoveRequest,
    now_ms: u64,
) -> Result<GuestMoveState, String>
where
    A: AspenPlacementFencePort,
    L: LatticeGuestReconnectPort,
    R: GuestRuntimePort,
{
    durable_database(db)?;
    aspen.check_available()?;
    lattice.check_available()?;
    let admission = aspen.admit_destination(&request.destination, &source.profile)?;
    let active = active_source_moves(db)?;
    let plan = plan_guest_move(source, request, &admission, &active)
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let mut state = GuestMoveState::new(plan).map_err(|e| format!("{}: {}", e.code, e.message))?;
    persist_state(db, &state, now_ms)?;

    let mut checkpoint_evidence: Option<ContentId> = None;

    for kind in GuestMoveStepKind::ORDERED {
        if kind == GuestMoveStepKind::Restore {
            match aspen.grant_destination_placement(&state.plan)? {
                StepReply::Confirmed(grant) => {
                    state
                        .record_aspen_grant(grant)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                }
                StepReply::LostReply { cause } => {
                    state
                        .mark_step_uncertain(GuestMoveStepKind::Restore, &cause)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                    return Ok(state);
                }
                StepReply::Failed { cause } => {
                    state
                        .begin_step(
                            GuestMoveStepKind::Restore,
                            request.fence,
                            request.placement_version,
                        )
                        .ok();
                    let _ = state.mark_step_uncertain(GuestMoveStepKind::Restore, &cause);
                    // Placement grant rejected before restore: abort cleanly without retiring source.
                    if let Some(step) = state
                        .plan
                        .steps
                        .get_mut(GuestMoveStepKind::Restore.index())
                    {
                        step.status = GuestMoveStepStatus::Failed;
                        step.diagnostic = Some(cause);
                    }
                    state.outcome = GuestMoveOutcome::Aborted;
                    persist_state(db, &state, now_ms)?;
                    return Ok(state);
                }
            }
        }

        if kind == GuestMoveStepKind::RetireSource {
            let grant = state
                .aspen_grant
                .clone()
                .ok_or("guest_move_missing_grant: destination placement grant missing")?;
            match aspen.fence_source_resources(&state.plan, &grant)? {
                StepReply::Confirmed(fence_receipt) => {
                    state
                        .record_source_fence(fence_receipt)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                }
                StepReply::LostReply { cause } => {
                    state
                        .mark_step_uncertain(GuestMoveStepKind::RetireSource, &cause)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                    return Ok(state);
                }
                StepReply::Failed { cause } => {
                    state
                        .mark_step_uncertain(GuestMoveStepKind::RetireSource, &cause)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                    return Ok(state);
                }
            }
        }

        // Persist Requested status BEFORE executing any external effect.
        let effect = state
            .begin_step(kind, request.fence, request.placement_version)
            .map_err(|e| format!("{}: {}", e.code, e.message))?;
        persist_state(db, &state, now_ms)?;

        match kind {
            GuestMoveStepKind::Pause => match runtime.pause_source(&effect, &state.plan)? {
                StepReply::Confirmed(evidence) => {
                    state
                        .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                }
                StepReply::LostReply { cause } => {
                    state
                        .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                    return Ok(state);
                }
                StepReply::Failed { cause } => {
                    state
                        .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    persist_state(db, &state, now_ms)?;
                    return Ok(state);
                }
            },
            GuestMoveStepKind::Checkpoint => {
                match runtime.capture_checkpoint(&effect, &state.plan)? {
                    StepReply::Confirmed(evidence) => {
                        checkpoint_evidence = Some(evidence.clone());
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    }
                    StepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                    StepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                }
            }
            GuestMoveStepKind::Transfer => {
                let ckpt = checkpoint_evidence
                    .as_ref()
                    .ok_or("guest_move_missing_checkpoint: checkpoint evidence missing")?;
                match runtime.transfer_checkpoint(&effect, &state.plan, ckpt)? {
                    StepReply::Confirmed(evidence) => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    }
                    StepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                    StepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                }
            }
            GuestMoveStepKind::Restore => {
                let grant = state
                    .aspen_grant
                    .as_ref()
                    .ok_or("guest_move_missing_grant: Aspen placement grant missing")?;
                match runtime.restore_destination(&effect, &state.plan, grant)? {
                    StepReply::Confirmed(evidence) => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    }
                    StepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                    StepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                }
            }
            GuestMoveStepKind::Reconnect => {
                let grant = state
                    .aspen_grant
                    .as_ref()
                    .ok_or("guest_move_missing_grant: Aspen placement grant missing")?;
                match lattice.reconnect_restored_guest(&effect, &state.plan, grant)? {
                    StepReply::Confirmed(confirmation) => {
                        state
                            .record_reconnect(confirmation)
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    }
                    StepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                    StepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                }
            }
            GuestMoveStepKind::RetireSource => {
                let grant = state
                    .aspen_grant
                    .as_ref()
                    .ok_or("guest_move_missing_grant: Aspen placement grant missing")?;
                let reconnect = state
                    .reconnect
                    .as_ref()
                    .ok_or("guest_move_missing_reconnect: Lattice reconnect missing")?;
                match runtime.retire_source(&effect, &state.plan, grant, reconnect)? {
                    StepReply::Confirmed(evidence) => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    }
                    StepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                    StepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| format!("{}: {}", e.code, e.message))?;
                        persist_state(db, &state, now_ms)?;
                        return Ok(state);
                    }
                }
            }
        }
        persist_state(db, &state, now_ms)?;
    }
    Ok(state)
}

/// Reconcile an uncertain guest move by reading native Aspen, Lattice, and
/// guest state without repeating any effect.
// r[impl kiln.sdlc.guest_move.scenario.uncertain]
pub fn reconcile_move<A, L, R>(
    db: &mut Connection,
    aspen: &A,
    lattice: &L,
    runtime: &R,
    plan: &ContentId,
    now_ms: u64,
) -> Result<GuestMoveState, String>
where
    A: AspenPlacementFencePort,
    L: LatticeGuestReconnectPort,
    R: GuestRuntimePort,
{
    durable_database(db)?;
    aspen.check_available()?;
    lattice.check_available()?;
    let mut state = load_move(db, plan)?
        .ok_or_else(|| "guest_move_unknown: no durable guest move for this plan".to_string())?;
    let Some(uncertain_kind) = state
        .plan
        .steps
        .iter()
        .find(|step| {
            matches!(
                step.status,
                GuestMoveStepStatus::Uncertain | GuestMoveStepStatus::Requested
            )
        })
        .map(|step| step.kind)
    else {
        return Ok(state);
    };

    match uncertain_kind {
        GuestMoveStepKind::Reconnect => match lattice.read_native_reconnect(&state.plan)? {
            NativeReconnectObservation::Reconnected(conf) => {
                state
                    .record_reconnect(conf)
                    .map_err(|e| format!("{}: {}", e.code, e.message))?;
            }
            NativeReconnectObservation::NotConnected { cause } => {
                state
                    .reconcile_step(
                        uncertain_kind,
                        GuestMoveReconcileObservation::Failed { cause },
                    )
                    .map_err(|e| format!("{}: {}", e.code, e.message))?;
            }
            NativeReconnectObservation::StillUnknown { cause } => {
                state
                    .reconcile_step(
                        uncertain_kind,
                        GuestMoveReconcileObservation::StillUnknown { cause },
                    )
                    .map_err(|e| format!("{}: {}", e.code, e.message))?;
            }
        },
        GuestMoveStepKind::Restore if state.aspen_grant.is_none() => {
            match aspen.read_native_placement(&state.plan)? {
                NativePlacementObservation::DestinationOwned(grant) => {
                    state
                        .record_aspen_grant(grant)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                    let obs = runtime.read_native_step(&state.plan, GuestMoveStepKind::Restore)?;
                    state
                        .reconcile_step(GuestMoveStepKind::Restore, obs)
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                }
                NativePlacementObservation::SourceOwned => {
                    state
                        .reconcile_step(
                            GuestMoveStepKind::Restore,
                            GuestMoveReconcileObservation::Failed {
                                cause: "Aspen still reports source ownership".into(),
                            },
                        )
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                }
                NativePlacementObservation::StillUnknown { cause } => {
                    state
                        .reconcile_step(
                            GuestMoveStepKind::Restore,
                            GuestMoveReconcileObservation::StillUnknown { cause },
                        )
                        .map_err(|e| format!("{}: {}", e.code, e.message))?;
                }
            }
        }
        other => {
            let obs = runtime.read_native_step(&state.plan, other)?;
            state
                .reconcile_step(other, obs)
                .map_err(|e| format!("{}: {}", e.code, e.message))?;
        }
    }
    persist_state(db, &state, now_ms)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::fleet::guest_move::{GuestRunState, GuestSourceKind};
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    fn id(ch: char) -> ContentId {
        ContentId::from_blake3_hex(ch.to_string().repeat(64))
            .unwrap_or_else(|e| unreachable!("{e:?}"))
    }

    fn durable_db() -> Result<(tempfile::TempDir, Connection), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let db = Connection::open(dir.path().join("guest_move.sqlite")).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
            .map_err(|e| e.to_string())?;
        init(&db)?;
        Ok((dir, db))
    }

    fn profile() -> GuestCompatibilityProfile {
        GuestCompatibilityProfile {
            vmm_version: "v54.0".into(),
            vmm_binary: id('1'),
            kernel: id('2'),
            initrd: id('3'),
            cpu_profile: id('4'),
            memory_backing: id('5'),
            devices: id('6'),
            vcpus: 2,
            memory_mib: 1024,
        }
    }

    fn source() -> GuestMoveSource {
        GuestMoveSource {
            attempt: id('a'),
            session: id('b'),
            vm: id('c'),
            work: id('d'),
            work_revision: 1,
            fence: 5,
            placement_version: 1,
            source_host: "host-a".into(),
            kind: GuestSourceKind::Guest,
            state: GuestRunState::Running,
            unresolved_prior_move: false,
            profile: profile(),
        }
    }

    fn request(src: &GuestMoveSource) -> GuestMoveRequest {
        GuestMoveRequest {
            attempt: src.attempt.clone(),
            session: src.session.clone(),
            fence: src.fence,
            placement_version: src.placement_version,
            destination: "host-b".into(),
        }
    }

    #[derive(Clone, Default)]
    struct MockPorts {
        calls: Rc<RefCell<Vec<String>>>,
        lost_on_step: Option<GuestMoveStepKind>,
        fail_restore: bool,
        incompatible_dst: bool,
        native_reconcile: BTreeMap<GuestMoveStepKind, GuestMoveReconcileObservation>,
    }

    impl AspenPlacementFencePort for MockPorts {
        fn check_available(&self) -> Result<(), String> {
            Ok(())
        }

        fn admit_destination(
            &self,
            destination: &str,
            profile: &GuestCompatibilityProfile,
        ) -> Result<DestinationAdmission, String> {
            let mut prof = profile.clone();
            if self.incompatible_dst {
                prof.vmm_version = "v53.0".into();
            }
            Ok(DestinationAdmission {
                host: destination.into(),
                profile: prof,
                disk_ready: true,
                has_free_capacity: true,
                draining: false,
            })
        }

        fn grant_destination_placement(
            &mut self,
            plan: &GuestMovePlan,
        ) -> Result<StepReply<DestinationPlacementGrant>, String> {
            self.calls.borrow_mut().push("aspen_grant".into());
            Ok(StepReply::Confirmed(DestinationPlacementGrant {
                plan: plan.identity.clone(),
                attempt: plan.request.attempt.clone(),
                session: plan.request.session.clone(),
                fence: plan.request.fence,
                source_host: plan.source_host.clone(),
                source_placement_version: plan.request.placement_version,
                destination_host: plan.request.destination.clone(),
                destination_placement_version: plan.target_placement_version,
                grant_id: id('8'),
                source_resources_fenced: id('9'),
            }))
        }

        fn fence_source_resources(
            &mut self,
            _plan: &GuestMovePlan,
            grant: &DestinationPlacementGrant,
        ) -> Result<StepReply<ContentId>, String> {
            self.calls.borrow_mut().push("aspen_fence".into());
            Ok(StepReply::Confirmed(grant.source_resources_fenced.clone()))
        }

        fn read_native_placement(
            &self,
            plan: &GuestMovePlan,
        ) -> Result<NativePlacementObservation, String> {
            Ok(NativePlacementObservation::DestinationOwned(
                DestinationPlacementGrant {
                    plan: plan.identity.clone(),
                    attempt: plan.request.attempt.clone(),
                    session: plan.request.session.clone(),
                    fence: plan.request.fence,
                    source_host: plan.source_host.clone(),
                    source_placement_version: plan.request.placement_version,
                    destination_host: plan.request.destination.clone(),
                    destination_placement_version: plan.target_placement_version,
                    grant_id: id('8'),
                    source_resources_fenced: id('9'),
                },
            ))
        }
    }

    impl LatticeGuestReconnectPort for MockPorts {
        fn check_available(&self) -> Result<(), String> {
            Ok(())
        }

        fn reconnect_restored_guest(
            &mut self,
            _effect: &GuestMoveEffect,
            plan: &GuestMovePlan,
            grant: &DestinationPlacementGrant,
        ) -> Result<StepReply<ReconnectConfirmation>, String> {
            self.calls.borrow_mut().push("reconnect".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Reconnect) {
                return Ok(StepReply::LostReply {
                    cause: "lattice reconnect reply lost".into(),
                });
            }
            Ok(StepReply::Confirmed(ReconnectConfirmation {
                plan: plan.identity.clone(),
                attempt: plan.request.attempt.clone(),
                session: plan.request.session.clone(),
                epoch: grant.destination_placement_version,
                reconnect_id: id('7'),
            }))
        }

        fn read_native_reconnect(
            &self,
            plan: &GuestMovePlan,
        ) -> Result<NativeReconnectObservation, String> {
            Ok(NativeReconnectObservation::Reconnected(
                ReconnectConfirmation {
                    plan: plan.identity.clone(),
                    attempt: plan.request.attempt.clone(),
                    session: plan.request.session.clone(),
                    epoch: plan.target_placement_version,
                    reconnect_id: id('7'),
                },
            ))
        }
    }

    impl GuestRuntimePort for MockPorts {
        fn pause_source(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
        ) -> Result<StepReply<ContentId>, String> {
            self.calls.borrow_mut().push("pause".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Pause) {
                return Ok(StepReply::LostReply {
                    cause: "pause reply lost".into(),
                });
            }
            Ok(StepReply::Confirmed(id('1')))
        }

        fn capture_checkpoint(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
        ) -> Result<StepReply<ContentId>, String> {
            self.calls.borrow_mut().push("checkpoint".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Checkpoint) {
                return Ok(StepReply::LostReply {
                    cause: "checkpoint reply lost".into(),
                });
            }
            Ok(StepReply::Confirmed(id('2')))
        }

        fn transfer_checkpoint(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
            _checkpoint: &ContentId,
        ) -> Result<StepReply<ContentId>, String> {
            self.calls.borrow_mut().push("transfer".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Transfer) {
                return Ok(StepReply::LostReply {
                    cause: "transfer reply lost".into(),
                });
            }
            Ok(StepReply::Confirmed(id('3')))
        }

        fn restore_destination(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
            _grant: &DestinationPlacementGrant,
        ) -> Result<StepReply<ContentId>, String> {
            self.calls.borrow_mut().push("restore".into());
            if self.fail_restore {
                return Ok(StepReply::Failed {
                    cause: "destination restore failed".into(),
                });
            }
            if self.lost_on_step == Some(GuestMoveStepKind::Restore) {
                return Ok(StepReply::LostReply {
                    cause: "restore reply lost".into(),
                });
            }
            Ok(StepReply::Confirmed(id('4')))
        }

        fn retire_source(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
            _grant: &DestinationPlacementGrant,
            _reconnect: &ReconnectConfirmation,
        ) -> Result<StepReply<ContentId>, String> {
            self.calls.borrow_mut().push("retire_source".into());
            if self.lost_on_step == Some(GuestMoveStepKind::RetireSource) {
                return Ok(StepReply::LostReply {
                    cause: "retire reply lost".into(),
                });
            }
            Ok(StepReply::Confirmed(id('5')))
        }

        fn read_native_step(
            &self,
            _plan: &GuestMovePlan,
            step: GuestMoveStepKind,
        ) -> Result<GuestMoveReconcileObservation, String> {
            Ok(self
                .native_reconcile
                .get(&step)
                .cloned()
                .unwrap_or(GuestMoveReconcileObservation::Observed { evidence: id('f') }))
        }
    }

    // r[verify kiln.sdlc.guest_move]
    #[test]
    fn fails_closed_by_default_before_journal_write_or_external_effect() -> Result<(), String> {
        let (_dir, mut db) = durable_db()?;
        let mut aspen = UnavailableAspenPlacementFence;
        let mut lattice = UnavailableLatticeGuestReconnect;
        let mut runtime = MockPorts::default();
        let src = source();
        let req = request(&src);

        let err = execute_move(&mut db, &mut aspen, &mut lattice, &mut runtime, &src, &req, 100)
            .err()
            .ok_or("default ports must fail closed")?;
        assert!(err.starts_with("guest_move_unsupported:"));
        assert!(runtime.calls.borrow().is_empty());
        let rows: i64 = db
            .query_row("SELECT COUNT(*) FROM hosted_guest_move", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        assert_eq!(rows, 0);
        Ok(())
    }

    // r[verify kiln.sdlc.guest_move.scenario.restored]
    #[test]
    fn executes_full_ordered_move_and_rejects_duplicate_source() -> Result<(), String> {
        let (_dir, mut db) = durable_db()?;
        let mut ports = MockPorts::default();
        let mut aspen = ports.clone();
        let mut lattice = ports.clone();
        let src = source();
        let req = request(&src);

        let result = execute_move(
            &mut db,
            &mut aspen,
            &mut lattice,
            &mut ports,
            &src,
            &req,
            1_000,
        )?;
        assert_eq!(result.outcome, GuestMoveOutcome::Completed);
        assert!(result.source_retired());
        assert_eq!(
            *ports.calls.borrow(),
            vec![
                "pause",
                "checkpoint",
                "transfer",
                "aspen_grant",
                "restore",
                "reconnect",
                "aspen_fence",
                "retire_source"
            ]
        );
        Ok(())
    }

    // r[verify kiln.sdlc.guest_move.scenario.uncertain]
    #[test]
    fn lost_reply_at_each_step_marks_uncertain_never_retries_or_retires_source() -> Result<(), String>
    {
        for step in GuestMoveStepKind::ORDERED {
            let (_dir, mut db) = durable_db()?;
            let mut ports = MockPorts {
                lost_on_step: Some(step),
                ..Default::default()
            };
            let mut aspen = ports.clone();
            let mut lattice = ports.clone();
            let src = source();
            let req = request(&src);

            let state = execute_move(
                &mut db,
                &mut aspen,
                &mut lattice,
                &mut ports,
                &src,
                &req,
                1_000,
            )?;
            assert_eq!(state.outcome, GuestMoveOutcome::Uncertain, "step={step:?}");
            assert_eq!(
                state.step(step).map(|s| s.status),
                Some(GuestMoveStepStatus::Uncertain),
                "step={step:?}"
            );
            assert!(!state.source_retired(), "step={step:?}");
            // Duplicate source attempt is refused while Uncertain!
            let dup = execute_move(
                &mut db,
                &mut aspen,
                &mut lattice,
                &mut ports,
                &src,
                &req,
                1_001,
            );
            assert!(
                dup.err()
                    .is_some_and(|e| e.starts_with("guest_move_duplicate_source:")),
                "step={step:?}"
            );

            // Reconcile reads native state without repeating the effect.
            let calls_before = ports.calls.borrow().len();
            let reconciled = reconcile_move(
                &mut db,
                &aspen,
                &lattice,
                &ports,
                &state.plan.identity,
                1_002,
            )?;
            assert_eq!(ports.calls.borrow().len(), calls_before);
            assert_eq!(
                reconciled.step(step).map(|s| s.status),
                Some(GuestMoveStepStatus::Observed),
                "step={step:?}"
            );
        }
        Ok(())
    }

    // r[verify kiln.sdlc.guest_move.scenario.uncertain]
    #[test]
    fn failed_restore_and_incompatible_destination_never_retire_source() -> Result<(), String> {
        let (_dir, mut db) = durable_db()?;
        let mut ports = MockPorts {
            incompatible_dst: true,
            ..Default::default()
        };
        let mut aspen = ports.clone();
        let mut lattice = ports.clone();
        let src = source();
        let req = request(&src);
        let incompatible = execute_move(&mut db, &mut aspen, &mut lattice, &mut ports, &src, &req, 10);
        assert!(
            incompatible
                .err()
                .is_some_and(|e| e.starts_with("guest_move_incompatible_destination:"))
        );
        assert!(ports.calls.borrow().is_empty());

        let mut ports = MockPorts {
            fail_restore: true,
            ..Default::default()
        };
        let mut aspen = ports.clone();
        let mut lattice = ports.clone();
        let state = execute_move(&mut db, &mut aspen, &mut lattice, &mut ports, &src, &req, 20)?;
        assert_eq!(state.outcome, GuestMoveOutcome::RestoreFailedPaused);
        assert!(!state.source_retired());
        assert!(
            !ports
                .calls
                .borrow()
                .iter()
                .any(|call| call == "retire_source")
        );
        Ok(())
    }
}

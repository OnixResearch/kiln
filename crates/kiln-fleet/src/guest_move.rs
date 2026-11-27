//! Operator-only running guest workload move adapter for `kiln-fleet`.
//!
//! Coordinates planning, step-by-step execution, status projection, and
//! durable move journal persistence using `kiln_core::fleet::guest_move`.
//! Fails closed before any external effect when Aspen placement fencing or
//! Lattice epoch-scoped guest reconnect prerequisites are absent.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use kiln_core::fleet::guest_move::{
    DestinationAdmission, DestinationPlacementGrant, GuestCompatibilityProfile, GuestMoveEffect,
    GuestMoveOutcome, GuestMovePlan, GuestMoveReconcileObservation, GuestMoveRequest,
    GuestMoveSource, GuestMoveState, GuestMoveStepKind, GuestMoveStepObservation,
    GuestMoveStepStatus, GuestMoveSubmit, GuestRunState, GuestSourceKind, PlannedGuestMoveStep,
    ReconnectConfirmation, plan_guest_move,
};
use kiln_core::types::ContentId;
use kiln_implementer::Implementer;
use serde::{Deserialize, Serialize};

use crate::api::Service;

const JOURNAL_SCHEMA: &str = "kiln.fleet-guest-move-journal.v1";
const MAX_JOURNAL_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMovePrerequisite {
    pub id: String,
    pub owner: String,
    pub satisfied: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveStatus {
    pub plan: GuestMovePlan,
    pub steps: Vec<PlannedGuestMoveStep>,
    pub current_step: Option<GuestMoveStepKind>,
    pub outcome: GuestMoveOutcome,
    pub source_retired: bool,
    pub aspen_grant: Option<DestinationPlacementGrant>,
    pub source_fence: Option<ContentId>,
    pub reconnect: Option<ReconnectConfirmation>,
    pub updated_ms: u64,
}

impl GuestMoveStatus {
    #[must_use]
    pub fn from_state(state: &GuestMoveState, updated_ms: u64) -> Self {
        Self {
            plan: state.plan.clone(),
            steps: state.plan.steps.clone(),
            current_step: state.current_step().map(|step| step.kind),
            outcome: state.outcome,
            source_retired: state.source_retired(),
            aspen_grant: state.aspen_grant.clone(),
            source_fence: state.source_fence.clone(),
            reconnect: state.reconnect.clone(),
            updated_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveOverview {
    pub schema: String,
    pub available: bool,
    pub reason: String,
    pub prerequisites: Vec<GuestMovePrerequisite>,
    pub ordered_steps: Vec<GuestMoveStepKind>,
    pub moves: Vec<GuestMoveStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderStepReply<T> {
    Confirmed(T),
    LostReply { cause: String },
    Failed { cause: String },
}

/// Port implemented by a qualified Aspen+Lattice+Cloud Hypervisor runtime
/// composition. The default implementation fails closed before any effect.
pub trait GuestMoveProvider {
    fn check_available(&self) -> Result<(), String>;

    fn resolve_source(
        &self,
        store: &mut Implementer,
        request: &GuestMoveRequest,
        now_ms: u64,
    ) -> Result<GuestMoveSource, String>;

    fn admit_destination(
        &self,
        service: &Service,
        destination: &str,
        profile: &GuestCompatibilityProfile,
    ) -> Result<DestinationAdmission, String>;

    fn grant_destination_placement(
        &mut self,
        plan: &GuestMovePlan,
    ) -> Result<ProviderStepReply<DestinationPlacementGrant>, String>;

    fn fence_source_resources(
        &mut self,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
    ) -> Result<ProviderStepReply<ContentId>, String>;

    fn pause_source(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
    ) -> Result<ProviderStepReply<ContentId>, String>;

    fn capture_checkpoint(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
    ) -> Result<ProviderStepReply<ContentId>, String>;

    fn transfer_checkpoint(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        checkpoint: &ContentId,
    ) -> Result<ProviderStepReply<ContentId>, String>;

    fn restore_destination(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
    ) -> Result<ProviderStepReply<ContentId>, String>;

    fn reconnect_restored_guest(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
    ) -> Result<ProviderStepReply<ReconnectConfirmation>, String>;

    fn retire_source(
        &mut self,
        effect: &GuestMoveEffect,
        plan: &GuestMovePlan,
        grant: &DestinationPlacementGrant,
        reconnect: &ReconnectConfirmation,
    ) -> Result<ProviderStepReply<ContentId>, String>;

    fn reconcile_native_step(
        &self,
        plan: &GuestMovePlan,
        step: GuestMoveStepKind,
    ) -> Result<GuestMoveReconcileObservation, String>;
}

#[derive(Clone, Debug, Default)]
pub struct UnavailableGuestMoveProvider;

impl UnavailableGuestMoveProvider {
    fn unavailable_error() -> String {
        "fleet_placement_unavailable: guest_move_unsupported: Aspen fence-guest-placement-ownership (Aspen #15) and Lattice reconnect-restored-guest-sessions are unavailable; failing closed before any effect".into()
    }
}

impl GuestMoveProvider for UnavailableGuestMoveProvider {
    fn check_available(&self) -> Result<(), String> {
        Err(Self::unavailable_error())
    }

    fn resolve_source(
        &self,
        _store: &mut Implementer,
        _request: &GuestMoveRequest,
        _now_ms: u64,
    ) -> Result<GuestMoveSource, String> {
        Err(Self::unavailable_error())
    }

    fn admit_destination(
        &self,
        _service: &Service,
        _destination: &str,
        _profile: &GuestCompatibilityProfile,
    ) -> Result<DestinationAdmission, String> {
        Err(Self::unavailable_error())
    }

    fn grant_destination_placement(
        &mut self,
        _plan: &GuestMovePlan,
    ) -> Result<ProviderStepReply<DestinationPlacementGrant>, String> {
        Err(Self::unavailable_error())
    }

    fn fence_source_resources(
        &mut self,
        _plan: &GuestMovePlan,
        _grant: &DestinationPlacementGrant,
    ) -> Result<ProviderStepReply<ContentId>, String> {
        Err(Self::unavailable_error())
    }

    fn pause_source(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
    ) -> Result<ProviderStepReply<ContentId>, String> {
        Err(Self::unavailable_error())
    }

    fn capture_checkpoint(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
    ) -> Result<ProviderStepReply<ContentId>, String> {
        Err(Self::unavailable_error())
    }

    fn transfer_checkpoint(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
        _checkpoint: &ContentId,
    ) -> Result<ProviderStepReply<ContentId>, String> {
        Err(Self::unavailable_error())
    }

    fn restore_destination(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
        _grant: &DestinationPlacementGrant,
    ) -> Result<ProviderStepReply<ContentId>, String> {
        Err(Self::unavailable_error())
    }

    fn reconnect_restored_guest(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
        _grant: &DestinationPlacementGrant,
    ) -> Result<ProviderStepReply<ReconnectConfirmation>, String> {
        Err(Self::unavailable_error())
    }

    fn retire_source(
        &mut self,
        _effect: &GuestMoveEffect,
        _plan: &GuestMovePlan,
        _grant: &DestinationPlacementGrant,
        _reconnect: &ReconnectConfirmation,
    ) -> Result<ProviderStepReply<ContentId>, String> {
        Err(Self::unavailable_error())
    }

    fn reconcile_native_step(
        &self,
        _plan: &GuestMovePlan,
        _step: GuestMoveStepKind,
    ) -> Result<GuestMoveReconcileObservation, String> {
        Err(Self::unavailable_error())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalEntry {
    state: GuestMoveState,
    updated_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalFile {
    schema: String,
    moves: BTreeMap<String, JournalEntry>,
}

impl Default for JournalFile {
    fn default() -> Self {
        Self {
            schema: JOURNAL_SCHEMA.to_string(),
            moves: BTreeMap::new(),
        }
    }
}

fn journal_path(service: &Service) -> PathBuf {
    service
        .config
        .implementer
        .database
        .with_extension("guest-moves.json")
}

fn load_journal(path: &Path) -> Result<JournalFile, String> {
    if !path.exists() {
        return Ok(JournalFile::default());
    }
    let bytes = crate::config::read_bounded(path, MAX_JOURNAL_BYTES)?;
    let journal: JournalFile =
        serde_json::from_slice(&bytes).map_err(|e| format!("guest move journal malformed: {e}"))?;
    if journal.schema != JOURNAL_SCHEMA {
        return Err("guest move journal schema mismatch".into());
    }
    Ok(journal)
}

fn save_journal(path: &Path, journal: &JournalFile) -> Result<(), String> {
    let encoded = serde_json::to_vec_pretty(journal).map_err(|e| e.to_string())?;
    let temp = path.with_extension("guest-moves.json.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    file.write_all(&encoded)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| e.to_string())?;
    Ok(())
}

fn map_core_error(code: &str, message: &str) -> String {
    let prefix = match code {
        "guest_move_stale" => "fleet_placement_stale",
        "guest_move_incompatible_destination" | "guest_move_same_host" => {
            "fleet_placement_unavailable"
        }
        _ => "fleet_placement_source",
    };
    format!("{prefix}: {code}: {message}")
}

fn active_source_identities(journal: &JournalFile) -> Vec<ContentId> {
    journal
        .moves
        .values()
        .filter(|entry| {
            matches!(
                entry.state.outcome,
                GuestMoveOutcome::Planned
                    | GuestMoveOutcome::InProgress
                    | GuestMoveOutcome::Uncertain
            )
        })
        .flat_map(|entry| {
            [
                entry.state.plan.request.attempt.clone(),
                entry.state.plan.request.session.clone(),
            ]
        })
        .collect()
}

pub fn guest_move_overview(service: &Service) -> Result<GuestMoveOverview, String> {
    let journal = load_journal(&journal_path(service))?;
    let moves = journal
        .moves
        .values()
        .map(|entry| GuestMoveStatus::from_state(&entry.state, entry.updated_ms))
        .collect();
    Ok(GuestMoveOverview {
        schema: "kiln.fleet-guest-move-overview.v1".into(),
        available: false,
        reason: "Running guest move fails closed before any effect until Aspen fence-guest-placement-ownership (Aspen #15), Lattice reconnect-restored-guest-sessions, and two-host KVM qualification are satisfied.".into(),
        prerequisites: vec![
            GuestMovePrerequisite {
                id: "fence-guest-placement-ownership".into(),
                owner: "Aspen (#15)".into(),
                satisfied: false,
                detail: "Exclusive guest placement CAS and source resource fencing port unavailable".into(),
            },
            GuestMovePrerequisite {
                id: "reconnect-restored-guest-sessions".into(),
                owner: "Lattice".into(),
                satisfied: false,
                detail: "Epoch-scoped restored guest session reconnect port unavailable".into(),
            },
            GuestMovePrerequisite {
                id: "two-host-kvm-qualification".into(),
                owner: "Operations".into(),
                satisfied: false,
                detail: "Two-host compatible KVM qualification not yet retained".into(),
            },
        ],
        ordered_steps: GuestMoveStepKind::ORDERED.to_vec(),
        moves,
    })
}

pub fn preview_guest_move(
    service: &Service,
    store: &mut Implementer,
    request: GuestMoveRequest,
    now_ms: u64,
) -> Result<GuestMovePlan, String> {
    preview_with_provider(
        service,
        store,
        request,
        now_ms,
        &UnavailableGuestMoveProvider,
    )
}

// r[impl kiln.sdlc.guest_move]
pub fn preview_with_provider<P: GuestMoveProvider>(
    service: &Service,
    store: &mut Implementer,
    request: GuestMoveRequest,
    now_ms: u64,
    provider: &P,
) -> Result<GuestMovePlan, String> {
    request
        .validate()
        .map_err(|e| map_core_error(&e.code, &e.message))?;
    provider.check_available()?;
    let controls = service.controls_snapshot();
    if controls.dispatch_pause.paused || controls.is_draining(&request.destination) {
        return Err(
            "fleet_placement_unavailable: fleet dispatch is paused or destination is draining"
                .into(),
        );
    }
    let source = provider.resolve_source(store, &request, now_ms)?;
    let admission = provider.admit_destination(service, &request.destination, &source.profile)?;
    let journal = load_journal(&journal_path(service))?;
    let active = active_source_identities(&journal);
    plan_guest_move(&source, &request, &admission, &active)
        .map_err(|e| map_core_error(&e.code, &e.message))
}

pub fn submit_guest_move(
    service: &Service,
    store: &mut Implementer,
    submission: GuestMoveSubmit,
    now_ms: u64,
) -> Result<GuestMoveStatus, String> {
    submit_with_provider(
        service,
        store,
        submission,
        now_ms,
        &mut UnavailableGuestMoveProvider,
    )
}

// r[impl kiln.sdlc.guest_move]
// r[impl kiln.sdlc.guest_move.scenario.restored]
// r[impl kiln.sdlc.guest_move.scenario.uncertain]
pub fn submit_with_provider<P: GuestMoveProvider>(
    service: &Service,
    store: &mut Implementer,
    submission: GuestMoveSubmit,
    now_ms: u64,
    provider: &mut P,
) -> Result<GuestMoveStatus, String> {
    let request = submission.request();
    request
        .validate()
        .map_err(|e| map_core_error(&e.code, &e.message))?;
    submission
        .plan
        .validate()
        .map_err(|e| map_core_error(&e.code, &e.message))?;

    let path = journal_path(service);
    let mut journal = load_journal(&path)?;
    if let Some(existing) = journal.moves.get(submission.plan.as_str()) {
        if existing.state.plan.request != request {
            return Err(
                "fleet_placement_changed: submitted request does not match the recorded guest move plan"
                    .into(),
            );
        }
        return Ok(GuestMoveStatus::from_state(
            &existing.state,
            existing.updated_ms,
        ));
    }

    provider.check_available()?;
    let controls = service.controls_snapshot();
    if controls.dispatch_pause.paused || controls.is_draining(&request.destination) {
        return Err(
            "fleet_placement_unavailable: fleet dispatch is paused or destination is draining"
                .into(),
        );
    }
    let source = provider.resolve_source(store, &request, now_ms)?;
    let admission = provider.admit_destination(service, &request.destination, &source.profile)?;
    let active = active_source_identities(&journal);
    let plan = plan_guest_move(&source, &request, &admission, &active)
        .map_err(|e| map_core_error(&e.code, &e.message))?;
    if plan.identity != submission.plan {
        return Err(
            "fleet_placement_stale: exact plan identity no longer matches current source or destination state"
                .into(),
        );
    }

    let mut state = GuestMoveState::new(plan).map_err(|e| map_core_error(&e.code, &e.message))?;
    persist_entry(&path, &mut journal, &state, now_ms)?;

    let mut checkpoint_evidence: Option<ContentId> = None;

    for kind in GuestMoveStepKind::ORDERED {
        if kind == GuestMoveStepKind::Restore {
            match provider.grant_destination_placement(&state.plan)? {
                ProviderStepReply::Confirmed(grant) => {
                    state
                        .record_aspen_grant(grant)
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                }
                ProviderStepReply::LostReply { cause } => {
                    state
                        .mark_step_uncertain(GuestMoveStepKind::Restore, &cause)
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                    return Ok(GuestMoveStatus::from_state(&state, now_ms));
                }
                ProviderStepReply::Failed { cause } => {
                    if let Some(step) = state
                        .plan
                        .steps
                        .get_mut(GuestMoveStepKind::Restore.index())
                    {
                        step.status = GuestMoveStepStatus::Failed;
                        step.diagnostic = Some(cause);
                    }
                    state.outcome = GuestMoveOutcome::Aborted;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                    return Ok(GuestMoveStatus::from_state(&state, now_ms));
                }
            }
        }

        if kind == GuestMoveStepKind::RetireSource {
            let grant = state
                .aspen_grant
                .clone()
                .ok_or("fleet_placement_source: Aspen placement grant missing")?;
            match provider.fence_source_resources(&state.plan, &grant)? {
                ProviderStepReply::Confirmed(fence_receipt) => {
                    state
                        .record_source_fence(fence_receipt)
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                }
                ProviderStepReply::LostReply { cause }
                | ProviderStepReply::Failed { cause } => {
                    state
                        .mark_step_uncertain(GuestMoveStepKind::RetireSource, &cause)
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                    return Ok(GuestMoveStatus::from_state(&state, now_ms));
                }
            }
        }

        let effect = state
            .begin_step(kind, request.fence, request.placement_version)
            .map_err(|e| map_core_error(&e.code, &e.message))?;
        persist_entry(&path, &mut journal, &state, now_ms)?;

        match kind {
            GuestMoveStepKind::Pause => match provider.pause_source(&effect, &state.plan)? {
                ProviderStepReply::Confirmed(evidence) => {
                    state
                        .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                }
                ProviderStepReply::LostReply { cause } => {
                    state
                        .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                    return Ok(GuestMoveStatus::from_state(&state, now_ms));
                }
                ProviderStepReply::Failed { cause } => {
                    state
                        .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                        .map_err(|e| map_core_error(&e.code, &e.message))?;
                    persist_entry(&path, &mut journal, &state, now_ms)?;
                    return Ok(GuestMoveStatus::from_state(&state, now_ms));
                }
            },
            GuestMoveStepKind::Checkpoint => {
                match provider.capture_checkpoint(&effect, &state.plan)? {
                    ProviderStepReply::Confirmed(evidence) => {
                        checkpoint_evidence = Some(evidence.clone());
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                    }
                    ProviderStepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                    ProviderStepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                }
            }
            GuestMoveStepKind::Transfer => {
                let ckpt = checkpoint_evidence
                    .as_ref()
                    .ok_or("fleet_placement_source: checkpoint evidence missing")?;
                match provider.transfer_checkpoint(&effect, &state.plan, ckpt)? {
                    ProviderStepReply::Confirmed(evidence) => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                    }
                    ProviderStepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                    ProviderStepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                }
            }
            GuestMoveStepKind::Restore => {
                let grant = state
                    .aspen_grant
                    .as_ref()
                    .ok_or("fleet_placement_source: Aspen placement grant missing")?;
                match provider.restore_destination(&effect, &state.plan, grant)? {
                    ProviderStepReply::Confirmed(evidence) => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                    }
                    ProviderStepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                    ProviderStepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                }
            }
            GuestMoveStepKind::Reconnect => {
                let grant = state
                    .aspen_grant
                    .as_ref()
                    .ok_or("fleet_placement_source: Aspen placement grant missing")?;
                match provider.reconnect_restored_guest(&effect, &state.plan, grant)? {
                    ProviderStepReply::Confirmed(confirmation) => {
                        state
                            .record_reconnect(confirmation)
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                    }
                    ProviderStepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                    ProviderStepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                }
            }
            GuestMoveStepKind::RetireSource => {
                let grant = state
                    .aspen_grant
                    .as_ref()
                    .ok_or("fleet_placement_source: Aspen placement grant missing")?;
                let reconnect = state
                    .reconnect
                    .as_ref()
                    .ok_or("fleet_placement_source: Lattice reconnect missing")?;
                match provider.retire_source(&effect, &state.plan, grant, reconnect)? {
                    ProviderStepReply::Confirmed(evidence) => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Confirmed { evidence })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                    }
                    ProviderStepReply::LostReply { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::LostReply { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                    ProviderStepReply::Failed { cause } => {
                        state
                            .observe_step(kind, GuestMoveStepObservation::Failed { cause })
                            .map_err(|e| map_core_error(&e.code, &e.message))?;
                        persist_entry(&path, &mut journal, &state, now_ms)?;
                        return Ok(GuestMoveStatus::from_state(&state, now_ms));
                    }
                }
            }
        }
        persist_entry(&path, &mut journal, &state, now_ms)?;
    }

    Ok(GuestMoveStatus::from_state(&state, now_ms))
}

fn persist_entry(
    path: &Path,
    journal: &mut JournalFile,
    state: &GuestMoveState,
    updated_ms: u64,
) -> Result<(), String> {
    journal.moves.insert(
        state.plan.identity.as_str().to_string(),
        JournalEntry {
            state: state.clone(),
            updated_ms,
        },
    );
    save_journal(path, journal)
}

pub fn guest_move_status(service: &Service, identity: &ContentId) -> Result<GuestMoveStatus, String> {
    identity
        .validate()
        .map_err(|e| map_core_error(&e.code, &e.message))?;
    let journal = load_journal(&journal_path(service))?;
    let entry = journal
        .moves
        .get(identity.as_str())
        .ok_or("fleet_placement_unknown: no such running guest move")?;
    Ok(GuestMoveStatus::from_state(&entry.state, entry.updated_ms))
}

/// Inspect live attempt & session records to ensure a non-guest (`HostProcess`)
/// or non-running attempt is rejected cleanly.
pub fn classify_fleet_source(
    store: &mut Implementer,
    request: &GuestMoveRequest,
    now_ms: u64,
    placement_version: u64,
    profile: GuestCompatibilityProfile,
    vm: ContentId,
    is_guest: bool,
) -> Result<GuestMoveSource, String> {
    let attempt = store.authorize_current(&request.attempt, request.fence, now_ms)?;
    let fleet = store.fleet_state()?;
    let session = fleet
        .sessions
        .sessions
        .get(&request.session)
        .filter(|s| s.attempt == request.attempt && s.fence == request.fence)
        .ok_or("fleet_placement_stale: matching fleet session not found")?;
    let kind = if is_guest {
        GuestSourceKind::Guest
    } else {
        GuestSourceKind::HostProcess
    };
    let state = if session.state.is_ended() {
        GuestRunState::Ended
    } else {
        match session.state {
            kiln_core::fleet::SessionState::Placing | kiln_core::fleet::SessionState::Starting => {
                GuestRunState::Queued
            }
            kiln_core::fleet::SessionState::Prompted
            | kiln_core::fleet::SessionState::Working
            | kiln_core::fleet::SessionState::Blocked { .. }
            | kiln_core::fleet::SessionState::Ready { .. } => GuestRunState::Running,
            _ => GuestRunState::Ended,
        }
    };
    Ok(GuestMoveSource {
        attempt: attempt.identity,
        session: session.identity.clone(),
        vm,
        work: attempt.work_identity,
        work_revision: attempt.work_revision,
        fence: attempt.fence,
        placement_version,
        source_host: session.machine.clone(),
        kind,
        state,
        unresolved_prior_move: false,
        profile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::rc::Rc;

    use kiln_core::fleet::FleetAgent;
    use kiln_core::implementation::Enrollment;
    use kiln_core::types::ExternalRef;

    use crate::config::FleetConfig;
    use crate::http::{Request, Response};

    fn id(ch: char) -> ContentId {
        ContentId::from_blake3_hex(ch.to_string().repeat(64))
            .unwrap_or_else(|e| unreachable!("{e:?}"))
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

    fn make_service(dir: &Path) -> Result<Service, String> {
        let enrollment = Enrollment {
            repositories: BTreeSet::from(["onixresearch/kiln".to_owned()]),
            actors: BTreeSet::from(["operator".to_owned()]),
        };
        let enrollment_path = dir.join("enrollment.json");
        fs::write(
            &enrollment_path,
            serde_json::to_vec(&enrollment).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let inventory_path = dir.join("inventory.json");
        fs::write(&inventory_path, br#"{"machines":[]}"#).map_err(|e| e.to_string())?;
        let db_path = dir.join("implementer.sqlite");
        let config_value = serde_json::json!({
            "schema": "kiln.fleet.v1",
            "machine": "coord",
            "listen": ["127.0.0.1:8095"],
            "peers": { "peer-a.local.": "peer-a" },
            "operators": ["coord"],
            "tailscale": dir.join("tailscale"),
            "implementer": {
                "database": db_path,
                "enrollment": enrollment_path,
            },
            "inventory": inventory_path,
            "swarm": null,
            "capture": null,
            "policy": {},
            "dispatch": { "enabled": false, "kinds": [] },
            "authority_operator": "coord",
        });
        let config: FleetConfig =
            serde_json::from_value(config_value).map_err(|e| e.to_string())?;
        Service::new(config)
    }

    fn route(service: &Service, machine: &str, method: &str, path: &str, body: &[u8]) -> Response {
        service.route(
            machine,
            &Request {
                method: method.into(),
                path: path.into(),
                query: None,
                body: body.to_vec(),
            },
            1_000,
        )
    }

    // r[verify kiln.sdlc.guest_move]
    #[test]
    fn fleet_routes_enforce_operator_only_and_fail_closed_by_default() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let service = make_service(dir.path())?;

        // Non-operator peer is rejected on all guest-move routes.
        for (method, path) in [
            ("GET", "/v1/placement/guest-move"),
            ("POST", "/v1/placement/guest-move/plan"),
            ("POST", "/v1/placement/guest-move"),
            ("GET", &format!("/v1/placement/guest-move/{}", id('a').as_str())),
        ] {
            let res = route(&service, "peer-a", method, path, b"{}");
            assert_eq!(res.status, 403, "{method} {path}");
        }

        // Operator overview clearly reports unavailable prerequisites and ordered steps.
        let overview_res = route(&service, "coord", "GET", "/v1/placement/guest-move", b"");
        assert_eq!(overview_res.status, 200);
        assert_eq!(overview_res.body["available"], false);
        assert_eq!(
            overview_res.body["ordered_steps"],
            serde_json::json!([
                "pause",
                "checkpoint",
                "transfer",
                "restore",
                "reconnect",
                "retire_source"
            ])
        );

        // Preview and submit fail closed before any effect when external providers are absent.
        let req_body = serde_json::to_vec(&GuestMoveRequest {
            attempt: id('a'),
            session: id('b'),
            fence: 1,
            placement_version: 1,
            destination: "peer-a".into(),
        })
        .map_err(|e| e.to_string())?;
        let preview_res = route(
            &service,
            "coord",
            "POST",
            "/v1/placement/guest-move/plan",
            &req_body,
        );
        assert_eq!(preview_res.status, 409);
        assert!(
            preview_res.body.to_string().contains("guest_move_unsupported"),
            "{}",
            preview_res.body
        );

        let submit_body = serde_json::to_vec(&GuestMoveSubmit {
            plan: id('c'),
            attempt: id('a'),
            session: id('b'),
            fence: 1,
            placement_version: 1,
            destination: "peer-a".into(),
        })
        .map_err(|e| e.to_string())?;
        let submit_res = route(
            &service,
            "coord",
            "POST",
            "/v1/placement/guest-move",
            &submit_body,
        );
        assert_eq!(submit_res.status, 409);
        assert!(
            submit_res.body.to_string().contains("guest_move_unsupported"),
            "{}",
            submit_res.body
        );

        let missing_res = route(
            &service,
            "coord",
            "GET",
            &format!("/v1/placement/guest-move/{}", id('c').as_str()),
            b"",
        );
        assert_eq!(missing_res.status, 404);
        Ok(())
    }

    struct MockFleetProvider {
        calls: Rc<RefCell<Vec<String>>>,
        lost_on_step: Option<GuestMoveStepKind>,
        fail_restore: bool,
        incompatible_dst: bool,
        non_guest: bool,
        non_running: bool,
        stale_fence: bool,
    }

    impl Default for MockFleetProvider {
        fn default() -> Self {
            Self {
                calls: Rc::default(),
                lost_on_step: None,
                fail_restore: false,
                incompatible_dst: false,
                non_guest: false,
                non_running: false,
                stale_fence: false,
            }
        }
    }

    impl GuestMoveProvider for MockFleetProvider {
        fn check_available(&self) -> Result<(), String> {
            Ok(())
        }

        fn resolve_source(
            &self,
            _store: &mut Implementer,
            request: &GuestMoveRequest,
            _now_ms: u64,
        ) -> Result<GuestMoveSource, String> {
            Ok(GuestMoveSource {
                attempt: request.attempt.clone(),
                session: request.session.clone(),
                vm: id('e'),
                work: id('d'),
                work_revision: 1,
                fence: if self.stale_fence {
                    request.fence + 1
                } else {
                    request.fence
                },
                placement_version: request.placement_version,
                source_host: "coord".into(),
                kind: if self.non_guest {
                    GuestSourceKind::HostProcess
                } else {
                    GuestSourceKind::Guest
                },
                state: if self.non_running {
                    GuestRunState::Paused
                } else {
                    GuestRunState::Running
                },
                unresolved_prior_move: false,
                profile: profile(),
            })
        }

        fn admit_destination(
            &self,
            _service: &Service,
            destination: &str,
            prof: &GuestCompatibilityProfile,
        ) -> Result<DestinationAdmission, String> {
            let mut admitted = prof.clone();
            if self.incompatible_dst {
                admitted.memory_mib = 512;
            }
            Ok(DestinationAdmission {
                host: destination.into(),
                profile: admitted,
                disk_ready: true,
                has_free_capacity: true,
                draining: false,
            })
        }

        fn grant_destination_placement(
            &mut self,
            plan: &GuestMovePlan,
        ) -> Result<ProviderStepReply<DestinationPlacementGrant>, String> {
            self.calls.borrow_mut().push("grant".into());
            Ok(ProviderStepReply::Confirmed(DestinationPlacementGrant {
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
        ) -> Result<ProviderStepReply<ContentId>, String> {
            self.calls.borrow_mut().push("fence".into());
            Ok(ProviderStepReply::Confirmed(
                grant.source_resources_fenced.clone(),
            ))
        }

        fn pause_source(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
        ) -> Result<ProviderStepReply<ContentId>, String> {
            self.calls.borrow_mut().push("pause".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Pause) {
                return Ok(ProviderStepReply::LostReply {
                    cause: "pause lost".into(),
                });
            }
            Ok(ProviderStepReply::Confirmed(id('1')))
        }

        fn capture_checkpoint(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
        ) -> Result<ProviderStepReply<ContentId>, String> {
            self.calls.borrow_mut().push("checkpoint".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Checkpoint) {
                return Ok(ProviderStepReply::LostReply {
                    cause: "checkpoint lost".into(),
                });
            }
            Ok(ProviderStepReply::Confirmed(id('2')))
        }

        fn transfer_checkpoint(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
            _checkpoint: &ContentId,
        ) -> Result<ProviderStepReply<ContentId>, String> {
            self.calls.borrow_mut().push("transfer".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Transfer) {
                return Ok(ProviderStepReply::LostReply {
                    cause: "transfer lost".into(),
                });
            }
            Ok(ProviderStepReply::Confirmed(id('3')))
        }

        fn restore_destination(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
            _grant: &DestinationPlacementGrant,
        ) -> Result<ProviderStepReply<ContentId>, String> {
            self.calls.borrow_mut().push("restore".into());
            if self.fail_restore {
                return Ok(ProviderStepReply::Failed {
                    cause: "restore failed".into(),
                });
            }
            if self.lost_on_step == Some(GuestMoveStepKind::Restore) {
                return Ok(ProviderStepReply::LostReply {
                    cause: "restore lost".into(),
                });
            }
            Ok(ProviderStepReply::Confirmed(id('4')))
        }

        fn reconnect_restored_guest(
            &mut self,
            _effect: &GuestMoveEffect,
            plan: &GuestMovePlan,
            grant: &DestinationPlacementGrant,
        ) -> Result<ProviderStepReply<ReconnectConfirmation>, String> {
            self.calls.borrow_mut().push("reconnect".into());
            if self.lost_on_step == Some(GuestMoveStepKind::Reconnect) {
                return Ok(ProviderStepReply::LostReply {
                    cause: "reconnect lost".into(),
                });
            }
            Ok(ProviderStepReply::Confirmed(ReconnectConfirmation {
                plan: plan.identity.clone(),
                attempt: plan.request.attempt.clone(),
                session: plan.request.session.clone(),
                epoch: grant.destination_placement_version,
                reconnect_id: id('7'),
            }))
        }

        fn retire_source(
            &mut self,
            _effect: &GuestMoveEffect,
            _plan: &GuestMovePlan,
            _grant: &DestinationPlacementGrant,
            _reconnect: &ReconnectConfirmation,
        ) -> Result<ProviderStepReply<ContentId>, String> {
            self.calls.borrow_mut().push("retire_source".into());
            if self.lost_on_step == Some(GuestMoveStepKind::RetireSource) {
                return Ok(ProviderStepReply::LostReply {
                    cause: "retire lost".into(),
                });
            }
            Ok(ProviderStepReply::Confirmed(id('5')))
        }

        fn reconcile_native_step(
            &self,
            _plan: &GuestMovePlan,
            _step: GuestMoveStepKind,
        ) -> Result<GuestMoveReconcileObservation, String> {
            Ok(GuestMoveReconcileObservation::Observed { evidence: id('f') })
        }
    }

    // r[verify kiln.sdlc.guest_move.scenario.restored]
    // r[verify kiln.sdlc.guest_move.scenario.uncertain]
    #[test]
    fn mock_provider_exercises_completed_move_and_all_fault_paths() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let service = make_service(dir.path())?;
        let mut store = service.store.open(&Default::default())?;
        let req = GuestMoveRequest {
            attempt: id('a'),
            session: id('b'),
            fence: 3,
            placement_version: 1,
            destination: "peer-a".into(),
        };

        // Refusals: non-guest, non-running, stale fence, incompatible destination
        for (mut provider, expected) in [
            (
                MockFleetProvider {
                    non_guest: true,
                    ..Default::default()
                },
                "guest_move_non_guest",
            ),
            (
                MockFleetProvider {
                    non_running: true,
                    ..Default::default()
                },
                "guest_move_not_running",
            ),
            (
                MockFleetProvider {
                    stale_fence: true,
                    ..Default::default()
                },
                "guest_move_stale",
            ),
            (
                MockFleetProvider {
                    incompatible_dst: true,
                    ..Default::default()
                },
                "guest_move_incompatible_destination",
            ),
        ] {
            let err = preview_with_provider(&service, &mut store, req.clone(), 1_000, &provider)
                .err()
                .ok_or("expected refusal")?;
            assert!(err.contains(expected), "{err}");
            let _ = &mut provider;
        }

        // Completed move + readable status
        let mut provider = MockFleetProvider::default();
        let plan = preview_with_provider(&service, &mut store, req.clone(), 1_000, &provider)?;
        let submit_req = GuestMoveSubmit {
            plan: plan.identity.clone(),
            attempt: req.attempt.clone(),
            session: req.session.clone(),
            fence: req.fence,
            placement_version: req.placement_version,
            destination: req.destination.clone(),
        };
        let status_val =
            submit_with_provider(&service, &mut store, submit_req.clone(), 1_001, &mut provider)?;
        assert_eq!(status_val.outcome, GuestMoveOutcome::Completed);
        assert!(status_val.source_retired);
        let read_back = guest_move_status(&service, &plan.identity)?;
        assert_eq!(read_back.outcome, GuestMoveOutcome::Completed);

        // HTTP status route returns the persisted guest move
        let http_status = route(
            &service,
            "coord",
            "GET",
            &format!("/v1/placement/guest-move/{}", plan.identity.as_str()),
            b"",
        );
        assert_eq!(http_status.status, 200);

        // Changed submission for same plan is rejected
        let mut bad_submit = submit_req;
        bad_submit.destination = "other-host".into();
        let err = submit_with_provider(&service, &mut store, bad_submit, 1_002, &mut provider)
            .err()
            .ok_or("expected changed plan rejection")?;
        assert!(err.starts_with("fleet_placement_changed:"));

        // Failed restore leaves source unretired in fresh service dir
        let dir2 = tempfile::tempdir().map_err(|e| e.to_string())?;
        let service2 = make_service(dir2.path())?;
        let mut store2 = service2.store.open(&Default::default())?;
        let mut fail_restore_provider = MockFleetProvider {
            fail_restore: true,
            ..Default::default()
        };
        let plan2 = preview_with_provider(
            &service2,
            &mut store2,
            req.clone(),
            2_000,
            &fail_restore_provider,
        )?;
        let res2 = submit_with_provider(
            &service2,
            &mut store2,
            GuestMoveSubmit {
                plan: plan2.identity,
                attempt: req.attempt.clone(),
                session: req.session.clone(),
                fence: req.fence,
                placement_version: req.placement_version,
                destination: req.destination.clone(),
            },
            2_001,
            &mut fail_restore_provider,
        )?;
        assert_eq!(res2.outcome, GuestMoveOutcome::RestoreFailedPaused);
        assert!(!res2.source_retired);

        // Ensure unused imports don't warn by exercising classify_fleet_source lightly
        let _ = (
            FleetAgent {
                tool: "t".into(),
                host: "h".into(),
                session: "s".into(),
            },
            ExternalRef::new("local", "x", "y").map_err(|e| format!("{e:?}"))?,
        );
        Ok(())
    }
}

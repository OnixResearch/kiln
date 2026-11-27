//! Pure planning and ordered step transitions for moving one running guest attempt.
//!
//! No I/O, clock, or provider effect runs here. Every step is planned with
//! deterministic BLAKE3 identities, transitions through `Requested -> Observed`
//! or `Uncertain`, and refuses stale fences, incompatible destinations,
//! duplicate sources, non-guest/non-running attempts, and premature source
//! retirement.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{bounded_line, bounded_token, content_identity, push_field, refusal};
use crate::types::{ContentId, CoreResult};

pub const GUEST_MOVE_PLAN_SCHEMA: &str = "kiln.fleet-guest-move-plan.v1";
pub const GUEST_MOVE_STEP_SCHEMA: &str = "kiln.fleet-guest-move-step.v1";
pub const MAX_GUEST_MOVE_HOST_BYTES: usize = 128;
pub const MAX_GUEST_MOVE_VERSION_BYTES: usize = 64;
pub const MAX_GUEST_MOVE_DIAGNOSTIC_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestCompatibilityProfile {
    pub vmm_version: String,
    pub vmm_binary: ContentId,
    pub kernel: ContentId,
    pub initrd: ContentId,
    pub cpu_profile: ContentId,
    pub memory_backing: ContentId,
    pub devices: ContentId,
    pub vcpus: u16,
    pub memory_mib: u32,
}

impl GuestCompatibilityProfile {
    pub fn validate(&self, field: &str) -> CoreResult<()> {
        if !bounded_line(&self.vmm_version, MAX_GUEST_MOVE_VERSION_BYTES)
            || self.vcpus == 0
            || self.memory_mib == 0
        {
            return Err(refusal(
                "guest_move_profile",
                field,
                "guest compatibility profile requires bounded VMM version and non-zero resources",
            ));
        }
        for id in [
            &self.vmm_binary,
            &self.kernel,
            &self.initrd,
            &self.cpu_profile,
            &self.memory_backing,
            &self.devices,
        ] {
            id.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationAdmission {
    pub host: String,
    pub profile: GuestCompatibilityProfile,
    pub disk_ready: bool,
    pub has_free_capacity: bool,
    pub draining: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestSourceKind {
    Guest,
    HostProcess,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestRunState {
    Queued,
    Running,
    Paused,
    Ended,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveSource {
    pub attempt: ContentId,
    pub session: ContentId,
    pub vm: ContentId,
    pub work: ContentId,
    pub work_revision: u64,
    pub fence: u64,
    pub placement_version: u64,
    pub source_host: String,
    pub kind: GuestSourceKind,
    pub state: GuestRunState,
    pub unresolved_prior_move: bool,
    pub profile: GuestCompatibilityProfile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveRequest {
    pub attempt: ContentId,
    pub session: ContentId,
    pub fence: u64,
    pub placement_version: u64,
    pub destination: String,
}

impl GuestMoveRequest {
    pub fn validate(&self) -> CoreResult<()> {
        self.attempt.validate()?;
        self.session.validate()?;
        if self.fence == 0
            || self.placement_version == 0
            || !bounded_token(&self.destination, MAX_GUEST_MOVE_HOST_BYTES)
        {
            return Err(refusal(
                "guest_move_request",
                "guest_move",
                "guest move requires non-zero fence, placement version, and bounded destination host",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveSubmit {
    pub plan: ContentId,
    pub attempt: ContentId,
    pub session: ContentId,
    pub fence: u64,
    pub placement_version: u64,
    pub destination: String,
}

impl GuestMoveSubmit {
    #[must_use]
    pub fn request(&self) -> GuestMoveRequest {
        GuestMoveRequest {
            attempt: self.attempt.clone(),
            session: self.session.clone(),
            fence: self.fence,
            placement_version: self.placement_version,
            destination: self.destination.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestMoveStepKind {
    Pause,
    Checkpoint,
    Transfer,
    Restore,
    Reconnect,
    RetireSource,
}

impl GuestMoveStepKind {
    pub const ORDERED: [Self; 6] = [
        Self::Pause,
        Self::Checkpoint,
        Self::Transfer,
        Self::Restore,
        Self::Reconnect,
        Self::RetireSource,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Pause => 0,
            Self::Checkpoint => 1,
            Self::Transfer => 2,
            Self::Restore => 3,
            Self::Reconnect => 4,
            Self::RetireSource => 5,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Checkpoint => "checkpoint",
            Self::Transfer => "transfer",
            Self::Restore => "restore",
            Self::Reconnect => "reconnect",
            Self::RetireSource => "retire_source",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestMoveStepStatus {
    Pending,
    Requested,
    Observed,
    Uncertain,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedGuestMoveStep {
    pub kind: GuestMoveStepKind,
    pub identity: ContentId,
    pub status: GuestMoveStepStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ContentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMovePlan {
    pub schema: String,
    pub identity: ContentId,
    pub request: GuestMoveRequest,
    pub source_host: String,
    pub target_placement_version: u64,
    pub vm: ContentId,
    pub work: ContentId,
    pub work_revision: u64,
    pub profile: GuestCompatibilityProfile,
    pub steps: Vec<PlannedGuestMoveStep>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationPlacementGrant {
    pub plan: ContentId,
    pub attempt: ContentId,
    pub session: ContentId,
    pub fence: u64,
    pub source_host: String,
    pub source_placement_version: u64,
    pub destination_host: String,
    pub destination_placement_version: u64,
    pub grant_id: ContentId,
    pub source_resources_fenced: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconnectConfirmation {
    pub plan: ContentId,
    pub attempt: ContentId,
    pub session: ContentId,
    pub epoch: u64,
    pub reconnect_id: ContentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestMoveOutcome {
    Planned,
    InProgress,
    Completed,
    Uncertain,
    RestoreFailedPaused,
    Aborted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveEffect {
    pub plan: ContentId,
    pub step_identity: ContentId,
    pub kind: GuestMoveStepKind,
    pub attempt: ContentId,
    pub session: ContentId,
    pub fence: u64,
    pub source_host: String,
    pub destination_host: String,
    pub target_placement_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestMoveStepObservation {
    Confirmed { evidence: ContentId },
    LostReply { cause: String },
    Failed { cause: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestMoveReconcileObservation {
    Observed { evidence: ContentId },
    Failed { cause: String },
    StillUnknown { cause: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMoveState {
    pub plan: GuestMovePlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspen_grant: Option<DestinationPlacementGrant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fence: Option<ContentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnect: Option<ReconnectConfirmation>,
    pub outcome: GuestMoveOutcome,
}

// r[impl kiln.sdlc.guest_move]
pub fn plan_guest_move(
    source: &GuestMoveSource,
    request: &GuestMoveRequest,
    destination: &DestinationAdmission,
    active_source_moves: &[ContentId],
) -> CoreResult<GuestMovePlan> {
    request.validate()?;
    source.attempt.validate()?;
    source.session.validate()?;
    source.vm.validate()?;
    source.work.validate()?;
    source.profile.validate("guest_move.source.profile")?;
    destination
        .profile
        .validate("guest_move.destination.profile")?;
    if !bounded_token(&source.source_host, MAX_GUEST_MOVE_HOST_BYTES)
        || !bounded_token(&destination.host, MAX_GUEST_MOVE_HOST_BYTES)
    {
        return Err(refusal(
            "guest_move_host",
            "guest_move.host",
            "source and destination host labels must be bounded tokens",
        ));
    }
    if source.kind != GuestSourceKind::Guest {
        return Err(refusal(
            "guest_move_non_guest",
            "guest_move.source",
            "only Cloud Hypervisor guest attempts can be moved; host processes are refused",
        ));
    }
    if source.state != GuestRunState::Running {
        return Err(refusal(
            "guest_move_not_running",
            "guest_move.source",
            "running guest move requires an actively running guest attempt",
        ));
    }
    if source.unresolved_prior_move
        || active_source_moves
            .iter()
            .any(|id| id == &source.attempt || id == &source.session)
    {
        return Err(refusal(
            "guest_move_duplicate_source",
            "guest_move.source",
            "source attempt or session already has an active or unresolved guest move",
        ));
    }
    if source.fence == 0
        || source.placement_version == 0
        || request.attempt != source.attempt
        || request.session != source.session
        || request.fence != source.fence
        || request.placement_version != source.placement_version
    {
        return Err(refusal(
            "guest_move_stale",
            "guest_move.fence",
            "attempt, session, fence, or placement version does not match current owned state",
        ));
    }
    if request.destination == source.source_host {
        return Err(refusal(
            "guest_move_same_host",
            "guest_move.destination",
            "destination host must differ from the source host",
        ));
    }
    if destination.host != request.destination
        || destination.draining
        || !destination.has_free_capacity
        || !destination.disk_ready
        || destination.profile != source.profile
    {
        return Err(refusal(
            "guest_move_incompatible_destination",
            "guest_move.destination",
            "destination host is draining, lacks capacity/disk, or fails guest compatibility profile admission",
        ));
    }
    let target_placement_version = source.placement_version.checked_add(1).ok_or_else(|| {
        refusal(
            "guest_move_placement_overflow",
            "guest_move.placement_version",
            "placement version overflow",
        )
    })?;

    let mut plan_bytes = Vec::new();
    for field in [
        GUEST_MOVE_PLAN_SCHEMA,
        source.attempt.as_str(),
        source.session.as_str(),
        source.vm.as_str(),
        source.work.as_str(),
        &source.source_host,
        &request.destination,
        &source.profile.vmm_version,
        source.profile.vmm_binary.as_str(),
        source.profile.kernel.as_str(),
        source.profile.initrd.as_str(),
        source.profile.cpu_profile.as_str(),
        source.profile.memory_backing.as_str(),
        source.profile.devices.as_str(),
    ] {
        push_field(&mut plan_bytes, field, "guest_move.plan")?;
    }
    plan_bytes.extend_from_slice(&source.work_revision.to_be_bytes());
    plan_bytes.extend_from_slice(&source.fence.to_be_bytes());
    plan_bytes.extend_from_slice(&source.placement_version.to_be_bytes());
    plan_bytes.extend_from_slice(&target_placement_version.to_be_bytes());
    plan_bytes.extend_from_slice(&source.profile.vcpus.to_be_bytes());
    plan_bytes.extend_from_slice(&source.profile.memory_mib.to_be_bytes());
    let plan_identity = content_identity(&plan_bytes)?;

    let mut steps = Vec::with_capacity(GuestMoveStepKind::ORDERED.len());
    for (index, kind) in GuestMoveStepKind::ORDERED.into_iter().enumerate() {
        let mut step_bytes = Vec::new();
        push_field(&mut step_bytes, GUEST_MOVE_STEP_SCHEMA, "guest_move.step")?;
        push_field(&mut step_bytes, plan_identity.as_str(), "guest_move.step")?;
        step_bytes.extend_from_slice(&(index as u32).to_be_bytes());
        push_field(&mut step_bytes, kind.as_str(), "guest_move.step")?;
        steps.push(PlannedGuestMoveStep {
            kind,
            identity: content_identity(&step_bytes)?,
            status: GuestMoveStepStatus::Pending,
            evidence: None,
            diagnostic: None,
        });
    }

    Ok(GuestMovePlan {
        schema: GUEST_MOVE_PLAN_SCHEMA.to_string(),
        identity: plan_identity,
        request: request.clone(),
        source_host: source.source_host.clone(),
        target_placement_version,
        vm: source.vm.clone(),
        work: source.work.clone(),
        work_revision: source.work_revision,
        profile: source.profile.clone(),
        steps,
    })
}

impl GuestMoveState {
    pub fn new(plan: GuestMovePlan) -> CoreResult<Self> {
        if plan.schema != GUEST_MOVE_PLAN_SCHEMA
            || plan.steps.len() != GuestMoveStepKind::ORDERED.len()
            || !plan
                .steps
                .iter()
                .zip(GuestMoveStepKind::ORDERED)
                .all(|(step, expected)| step.kind == expected)
        {
            return Err(refusal(
                "guest_move_plan_invalid",
                "guest_move.plan",
                "guest move plan schema or ordered steps are invalid",
            ));
        }
        plan.identity.validate()?;
        plan.request.validate()?;
        Ok(Self {
            plan,
            aspen_grant: None,
            source_fence: None,
            reconnect: None,
            outcome: GuestMoveOutcome::Planned,
        })
    }

    #[must_use]
    pub fn step(&self, kind: GuestMoveStepKind) -> Option<&PlannedGuestMoveStep> {
        self.plan.steps.get(kind.index())
    }

    fn step_mut(&mut self, kind: GuestMoveStepKind) -> CoreResult<&mut PlannedGuestMoveStep> {
        self.plan.steps.get_mut(kind.index()).ok_or_else(|| {
            refusal(
                "guest_move_step_missing",
                "guest_move.step",
                "guest move step is missing from plan",
            )
        })
    }

    #[must_use]
    pub fn has_uncertain_step(&self) -> bool {
        self.plan
            .steps
            .iter()
            .any(|step| step.status == GuestMoveStepStatus::Uncertain)
    }

    #[must_use]
    pub fn current_step(&self) -> Option<&PlannedGuestMoveStep> {
        self.plan
            .steps
            .iter()
            .find(|step| step.status != GuestMoveStepStatus::Observed)
    }

    #[must_use]
    pub fn source_retired(&self) -> bool {
        self.step(GuestMoveStepKind::RetireSource)
            .is_some_and(|step| step.status == GuestMoveStepStatus::Observed)
    }

    /// Next step eligible to begin right now, respecting strict ordering and the
    /// mandatory Aspen grant + Lattice reconnect + source fence preconditions.
    #[must_use]
    pub fn next_step(&self) -> Option<GuestMoveStepKind> {
        if self.has_uncertain_step()
            || matches!(
                self.outcome,
                GuestMoveOutcome::Completed
                    | GuestMoveOutcome::Uncertain
                    | GuestMoveOutcome::RestoreFailedPaused
                    | GuestMoveOutcome::Aborted
            )
            || self
                .plan
                .steps
                .iter()
                .any(|step| matches!(step.status, GuestMoveStepStatus::Requested | GuestMoveStepStatus::Failed))
        {
            return None;
        }
        let next = self
            .plan
            .steps
            .iter()
            .find(|step| step.status == GuestMoveStepStatus::Pending)?
            .kind;
        match next {
            GuestMoveStepKind::Restore if self.aspen_grant.is_none() => None,
            GuestMoveStepKind::RetireSource
                if self.aspen_grant.is_none()
                    || self.source_fence.is_none()
                    || self.reconnect.is_none() =>
            {
                None
            }
            _ => Some(next),
        }
    }

    // r[impl kiln.sdlc.guest_move.scenario.restored]
    pub fn record_aspen_grant(&mut self, grant: DestinationPlacementGrant) -> CoreResult<()> {
        grant.plan.validate()?;
        grant.attempt.validate()?;
        grant.session.validate()?;
        grant.grant_id.validate()?;
        grant.source_resources_fenced.validate()?;
        if self.has_uncertain_step() {
            return Err(refusal(
                "guest_move_uncertain",
                "guest_move.grant",
                "cannot record placement grant while an earlier step is uncertain",
            ));
        }
        if !self
            .step(GuestMoveStepKind::Transfer)
            .is_some_and(|step| step.status == GuestMoveStepStatus::Observed)
        {
            return Err(refusal(
                "guest_move_order",
                "guest_move.grant",
                "placement grant requires observed pause, checkpoint, and transfer",
            ));
        }
        if grant.plan != self.plan.identity
            || grant.attempt != self.plan.request.attempt
            || grant.session != self.plan.request.session
            || grant.fence != self.plan.request.fence
            || grant.source_host != self.plan.source_host
            || grant.source_placement_version != self.plan.request.placement_version
            || grant.destination_host != self.plan.request.destination
            || grant.destination_placement_version != self.plan.target_placement_version
        {
            return Err(refusal(
                "guest_move_stale",
                "guest_move.grant",
                "Aspen placement grant does not match exact plan, fence, hosts, or placement version",
            ));
        }
        self.source_fence = Some(grant.source_resources_fenced.clone());
        self.aspen_grant = Some(grant);
        Ok(())
    }

    pub fn record_source_fence(&mut self, fence_receipt: ContentId) -> CoreResult<()> {
        fence_receipt.validate()?;
        if self.aspen_grant.is_none() {
            return Err(refusal(
                "guest_move_missing_grant",
                "guest_move.source_fence",
                "source fence requires an observed destination placement grant",
            ));
        }
        if self.has_uncertain_step() {
            return Err(refusal(
                "guest_move_uncertain",
                "guest_move.source_fence",
                "cannot record source fence while a step is uncertain",
            ));
        }
        self.source_fence = Some(fence_receipt);
        Ok(())
    }

    // r[impl kiln.sdlc.guest_move.scenario.restored]
    pub fn record_reconnect(&mut self, reconnect: ReconnectConfirmation) -> CoreResult<()> {
        reconnect.plan.validate()?;
        reconnect.attempt.validate()?;
        reconnect.session.validate()?;
        reconnect.reconnect_id.validate()?;
        if reconnect.plan != self.plan.identity
            || reconnect.attempt != self.plan.request.attempt
            || reconnect.session != self.plan.request.session
            || reconnect.epoch != self.plan.target_placement_version
        {
            return Err(refusal(
                "guest_move_stale",
                "guest_move.reconnect",
                "Lattice reconnect confirmation does not match plan, session, or target epoch",
            ));
        }
        if !self
            .step(GuestMoveStepKind::Restore)
            .is_some_and(|step| step.status == GuestMoveStepStatus::Observed)
        {
            return Err(refusal(
                "guest_move_order",
                "guest_move.reconnect",
                "reconnect requires an observed destination restore",
            ));
        }
        let step = self.step_mut(GuestMoveStepKind::Reconnect)?;
        if !matches!(
            step.status,
            GuestMoveStepStatus::Requested | GuestMoveStepStatus::Uncertain
        ) {
            return Err(refusal(
                "guest_move_order",
                "guest_move.reconnect",
                "reconnect confirmation requires an active or uncertain reconnect step",
            ));
        }
        step.status = GuestMoveStepStatus::Observed;
        step.evidence = Some(reconnect.reconnect_id.clone());
        step.diagnostic = None;
        self.reconnect = Some(reconnect);
        self.outcome = GuestMoveOutcome::InProgress;
        Ok(())
    }

    // r[impl kiln.sdlc.guest_move]
    pub fn begin_step(
        &mut self,
        kind: GuestMoveStepKind,
        fence: u64,
        placement_version: u64,
    ) -> CoreResult<GuestMoveEffect> {
        if fence != self.plan.request.fence
            || placement_version != self.plan.request.placement_version
        {
            return Err(refusal(
                "guest_move_stale",
                "guest_move.fence",
                "fence or placement version changed before step execution",
            ));
        }
        if self.has_uncertain_step() || self.outcome == GuestMoveOutcome::Uncertain {
            return Err(refusal(
                "guest_move_uncertain",
                "guest_move.step",
                "uncertain step blocks subsequent steps until native reconciliation completes",
            ));
        }
        if matches!(
            self.outcome,
            GuestMoveOutcome::Completed
                | GuestMoveOutcome::RestoreFailedPaused
                | GuestMoveOutcome::Aborted
        ) {
            return Err(refusal(
                "guest_move_halted",
                "guest_move.outcome",
                "guest move has already halted or completed",
            ));
        }
        if self
            .plan
            .steps
            .iter()
            .any(|step| step.status == GuestMoveStepStatus::Requested)
        {
            return Err(refusal(
                "guest_move_step_in_flight",
                "guest_move.step",
                "another step is already requested and awaiting observation",
            ));
        }
        for prior in GuestMoveStepKind::ORDERED
            .into_iter()
            .take(kind.index())
        {
            if !self
                .step(prior)
                .is_some_and(|step| step.status == GuestMoveStepStatus::Observed)
            {
                return Err(refusal(
                    "guest_move_order",
                    "guest_move.step",
                    "guest move steps must run strictly in order after observed completion of prior steps",
                ));
            }
        }
        if kind == GuestMoveStepKind::Restore && self.aspen_grant.is_none() {
            return Err(refusal(
                "guest_move_missing_grant",
                "guest_move.restore",
                "destination restore requires an observed Aspen placement grant",
            ));
        }
        if kind == GuestMoveStepKind::RetireSource
            && (self.aspen_grant.is_none()
                || self.source_fence.is_none()
                || self.reconnect.is_none())
        {
            return Err(refusal(
                "guest_move_retire_precondition",
                "guest_move.retire_source",
                "source retirement requires observed Aspen destination grant, source fence, and Lattice epoch reconnect",
            ));
        }
        let step = self.step_mut(kind)?;
        if step.status != GuestMoveStepStatus::Pending {
            return Err(refusal(
                "guest_move_replay",
                "guest_move.step",
                "step was already requested or completed and cannot be re-executed",
            ));
        }
        step.status = GuestMoveStepStatus::Requested;
        let step_identity = step.identity.clone();
        self.outcome = GuestMoveOutcome::InProgress;
        Ok(GuestMoveEffect {
            plan: self.plan.identity.clone(),
            step_identity,
            kind,
            attempt: self.plan.request.attempt.clone(),
            session: self.plan.request.session.clone(),
            fence: self.plan.request.fence,
            source_host: self.plan.source_host.clone(),
            destination_host: self.plan.request.destination.clone(),
            target_placement_version: self.plan.target_placement_version,
        })
    }

    // r[impl kiln.sdlc.guest_move.scenario.uncertain]
    pub fn observe_step(
        &mut self,
        kind: GuestMoveStepKind,
        observation: GuestMoveStepObservation,
    ) -> CoreResult<()> {
        if kind == GuestMoveStepKind::Reconnect
            && matches!(observation, GuestMoveStepObservation::Confirmed { .. })
            && self.reconnect.is_none()
        {
            return Err(refusal(
                "guest_move_missing_reconnect",
                "guest_move.reconnect",
                "reconnect step cannot be confirmed without an epoch-scoped ReconnectConfirmation",
            ));
        }
        let step = self.step_mut(kind)?;
        if step.status != GuestMoveStepStatus::Requested {
            return Err(refusal(
                "guest_move_step_phase",
                "guest_move.step",
                "step observation requires a step in Requested status",
            ));
        }
        match observation {
            GuestMoveStepObservation::Confirmed { evidence } => {
                evidence.validate()?;
                step.status = GuestMoveStepStatus::Observed;
                step.evidence = Some(evidence);
                step.diagnostic = None;
                if kind == GuestMoveStepKind::RetireSource {
                    self.outcome = GuestMoveOutcome::Completed;
                }
            }
            GuestMoveStepObservation::LostReply { cause } => {
                let cause = sanitize_diagnostic(&cause)?;
                step.status = GuestMoveStepStatus::Uncertain;
                step.diagnostic = Some(cause);
                self.outcome = GuestMoveOutcome::Uncertain;
            }
            GuestMoveStepObservation::Failed { cause } => {
                let cause = sanitize_diagnostic(&cause)?;
                step.status = GuestMoveStepStatus::Failed;
                step.diagnostic = Some(cause);
                self.outcome = if kind == GuestMoveStepKind::Restore {
                    GuestMoveOutcome::RestoreFailedPaused
                } else {
                    GuestMoveOutcome::Aborted
                };
            }
        }
        Ok(())
    }

    /// Explicitly mark a step uncertain before or during an effect (e.g. Aspen grant
    /// or pre-step lost reply) so no later step can proceed without reconciliation.
    pub fn mark_step_uncertain(
        &mut self,
        kind: GuestMoveStepKind,
        cause: &str,
    ) -> CoreResult<()> {
        let cause = sanitize_diagnostic(cause)?;
        let step = self.step_mut(kind)?;
        if !matches!(
            step.status,
            GuestMoveStepStatus::Pending
                | GuestMoveStepStatus::Requested
                | GuestMoveStepStatus::Uncertain
        ) {
            return Err(refusal(
                "guest_move_step_phase",
                "guest_move.step",
                "cannot mark completed or failed step as uncertain",
            ));
        }
        step.status = GuestMoveStepStatus::Uncertain;
        step.diagnostic = Some(cause);
        self.outcome = GuestMoveOutcome::Uncertain;
        Ok(())
    }

    // r[impl kiln.sdlc.guest_move.scenario.uncertain]
    pub fn reconcile_step(
        &mut self,
        kind: GuestMoveStepKind,
        observation: GuestMoveReconcileObservation,
    ) -> CoreResult<()> {
        if kind == GuestMoveStepKind::Reconnect
            && matches!(observation, GuestMoveReconcileObservation::Observed { .. })
            && self.reconnect.is_none()
        {
            return Err(refusal(
                "guest_move_missing_reconnect",
                "guest_move.reconnect",
                "reconciling reconnect as observed requires an epoch-scoped ReconnectConfirmation",
            ));
        }
        let step = self.step_mut(kind)?;
        if !matches!(
            step.status,
            GuestMoveStepStatus::Uncertain | GuestMoveStepStatus::Requested
        ) {
            return Err(refusal(
                "guest_move_reconcile_phase",
                "guest_move.reconcile",
                "reconciliation applies only to uncertain or in-flight steps",
            ));
        }
        match observation {
            GuestMoveReconcileObservation::Observed { evidence } => {
                evidence.validate()?;
                step.status = GuestMoveStepStatus::Observed;
                step.evidence = Some(evidence);
                step.diagnostic = None;
                self.outcome = if kind == GuestMoveStepKind::RetireSource {
                    GuestMoveOutcome::Completed
                } else {
                    GuestMoveOutcome::InProgress
                };
            }
            GuestMoveReconcileObservation::Failed { cause } => {
                let cause = sanitize_diagnostic(&cause)?;
                step.status = GuestMoveStepStatus::Failed;
                step.diagnostic = Some(cause);
                self.outcome = if kind == GuestMoveStepKind::Restore {
                    GuestMoveOutcome::RestoreFailedPaused
                } else {
                    GuestMoveOutcome::Aborted
                };
            }
            GuestMoveReconcileObservation::StillUnknown { cause } => {
                let cause = sanitize_diagnostic(&cause)?;
                step.status = GuestMoveStepStatus::Uncertain;
                step.diagnostic = Some(cause);
                self.outcome = GuestMoveOutcome::Uncertain;
            }
        }
        Ok(())
    }
}

fn sanitize_diagnostic(cause: &str) -> CoreResult<String> {
    let trimmed = cause.trim();
    if trimmed.is_empty() {
        return Err(refusal(
            "guest_move_diagnostic",
            "guest_move.diagnostic",
            "diagnostic cause must not be empty",
        ));
    }
    let clean: String = trimmed
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(MAX_GUEST_MOVE_DIAGNOSTIC_BYTES)
        .collect();
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn source() -> GuestMoveSource {
        GuestMoveSource {
            attempt: id('a'),
            session: id('b'),
            vm: id('c'),
            work: id('d'),
            work_revision: 3,
            fence: 7,
            placement_version: 1,
            source_host: "host-a".into(),
            kind: GuestSourceKind::Guest,
            state: GuestRunState::Running,
            unresolved_prior_move: false,
            profile: profile(),
        }
    }

    fn destination(host: &str) -> DestinationAdmission {
        DestinationAdmission {
            host: host.into(),
            profile: profile(),
            disk_ready: true,
            has_free_capacity: true,
            draining: false,
        }
    }

    fn request(src: &GuestMoveSource, dst: &str) -> GuestMoveRequest {
        GuestMoveRequest {
            attempt: src.attempt.clone(),
            session: src.session.clone(),
            fence: src.fence,
            placement_version: src.placement_version,
            destination: dst.into(),
        }
    }

    // r[verify kiln.sdlc.guest_move.scenario.restored]
    #[test]
    fn ordered_guest_move_completes_only_after_grant_fence_and_reconnect() -> CoreResult<()> {
        let src = source();
        let req = request(&src, "host-b");
        let dst = destination("host-b");
        let plan = plan_guest_move(&src, &req, &dst, &[])?;
        assert_eq!(plan.target_placement_version, 2);
        assert_eq!(plan.steps.len(), 6);
        assert_eq!(
            plan.steps.iter().map(|s| s.kind).collect::<Vec<_>>(),
            GuestMoveStepKind::ORDERED
        );

        let mut state = GuestMoveState::new(plan)?;
        assert_eq!(state.next_step(), Some(GuestMoveStepKind::Pause));

        for step in [
            GuestMoveStepKind::Pause,
            GuestMoveStepKind::Checkpoint,
            GuestMoveStepKind::Transfer,
        ] {
            state.begin_step(step, src.fence, src.placement_version)?;
            state.observe_step(step, GuestMoveStepObservation::Confirmed { evidence: id('e') })?;
        }

        // Restore is blocked until Aspen placement grant is recorded.
        assert_eq!(state.next_step(), None);
        assert!(
            state
                .begin_step(GuestMoveStepKind::Restore, src.fence, src.placement_version)
                .is_err()
        );

        let grant = DestinationPlacementGrant {
            plan: state.plan.identity.clone(),
            attempt: src.attempt.clone(),
            session: src.session.clone(),
            fence: src.fence,
            source_host: "host-a".into(),
            source_placement_version: 1,
            destination_host: "host-b".into(),
            destination_placement_version: 2,
            grant_id: id('8'),
            source_resources_fenced: id('9'),
        };
        state.record_aspen_grant(grant)?;
        assert_eq!(state.next_step(), Some(GuestMoveStepKind::Restore));
        state.begin_step(GuestMoveStepKind::Restore, src.fence, src.placement_version)?;
        state.observe_step(
            GuestMoveStepKind::Restore,
            GuestMoveStepObservation::Confirmed { evidence: id('f') },
        )?;

        assert_eq!(state.next_step(), Some(GuestMoveStepKind::Reconnect));
        state.begin_step(GuestMoveStepKind::Reconnect, src.fence, src.placement_version)?;
        // RetireSource is blocked until reconnect is confirmed.
        assert!(
            state
                .begin_step(
                    GuestMoveStepKind::RetireSource,
                    src.fence,
                    src.placement_version
                )
                .is_err()
        );
        state.record_reconnect(ReconnectConfirmation {
            plan: state.plan.identity.clone(),
            attempt: src.attempt.clone(),
            session: src.session.clone(),
            epoch: 2,
            reconnect_id: id('7'),
        })?;

        assert_eq!(state.next_step(), Some(GuestMoveStepKind::RetireSource));
        state.begin_step(
            GuestMoveStepKind::RetireSource,
            src.fence,
            src.placement_version,
        )?;
        state.observe_step(
            GuestMoveStepKind::RetireSource,
            GuestMoveStepObservation::Confirmed { evidence: id('6') },
        )?;
        assert_eq!(state.outcome, GuestMoveOutcome::Completed);
        assert!(state.source_retired());
        Ok(())
    }

    // r[verify kiln.sdlc.guest_move]
    #[test]
    fn refuses_non_guest_non_running_duplicate_stale_and_incompatible_destination() -> CoreResult<()> {
        let src = source();
        let req = request(&src, "host-b");
        let dst = destination("host-b");

        let mut host_proc = src.clone();
        host_proc.kind = GuestSourceKind::HostProcess;
        assert_eq!(
            plan_guest_move(&host_proc, &req, &dst, &[])
                .err()
                .map(|d| d.code),
            Some("guest_move_non_guest".into())
        );

        let mut paused = src.clone();
        paused.state = GuestRunState::Paused;
        assert_eq!(
            plan_guest_move(&paused, &req, &dst, &[])
                .err()
                .map(|d| d.code),
            Some("guest_move_not_running".into())
        );

        assert_eq!(
            plan_guest_move(&src, &req, &dst, &[src.attempt.clone()])
                .err()
                .map(|d| d.code),
            Some("guest_move_duplicate_source".into())
        );

        let mut stale_req = req.clone();
        stale_req.fence += 1;
        assert_eq!(
            plan_guest_move(&src, &stale_req, &dst, &[])
                .err()
                .map(|d| d.code),
            Some("guest_move_stale".into())
        );

        let mut stale_ver = req.clone();
        stale_ver.placement_version += 1;
        assert_eq!(
            plan_guest_move(&src, &stale_ver, &dst, &[])
                .err()
                .map(|d| d.code),
            Some("guest_move_stale".into())
        );

        let mut bad_cpu = dst.clone();
        bad_cpu.profile.cpu_profile = id('9');
        assert_eq!(
            plan_guest_move(&src, &req, &bad_cpu, &[])
                .err()
                .map(|d| d.code),
            Some("guest_move_incompatible_destination".into())
        );

        let mut no_disk = dst;
        no_disk.disk_ready = false;
        assert_eq!(
            plan_guest_move(&src, &req, &no_disk, &[])
                .err()
                .map(|d| d.code),
            Some("guest_move_incompatible_destination".into())
        );
        Ok(())
    }

    // r[verify kiln.sdlc.guest_move.scenario.uncertain]
    #[test]
    fn lost_reply_enters_uncertain_blocks_progress_and_failed_restore_keeps_source() -> CoreResult<()> {
        let src = source();
        let req = request(&src, "host-b");
        let dst = destination("host-b");
        let plan = plan_guest_move(&src, &req, &dst, &[])?;
        let mut state = GuestMoveState::new(plan)?;

        state.begin_step(GuestMoveStepKind::Pause, src.fence, src.placement_version)?;
        state.observe_step(
            GuestMoveStepKind::Pause,
            GuestMoveStepObservation::LostReply {
                cause: "vmm socket timed out".into(),
            },
        )?;
        assert_eq!(state.outcome, GuestMoveOutcome::Uncertain);
        assert!(state.has_uncertain_step());
        assert_eq!(state.next_step(), None);
        assert!(!state.source_retired());
        assert!(
            state
                .begin_step(
                    GuestMoveStepKind::Checkpoint,
                    src.fence,
                    src.placement_version
                )
                .is_err()
        );
        // Re-requesting the uncertain Pause step is also refused (never repeat!).
        assert!(
            state
                .begin_step(GuestMoveStepKind::Pause, src.fence, src.placement_version)
                .is_err()
        );

        // Native reconciliation resolves Pause without re-executing it.
        state.reconcile_step(
            GuestMoveStepKind::Pause,
            GuestMoveReconcileObservation::Observed { evidence: id('e') },
        )?;
        assert_eq!(state.next_step(), Some(GuestMoveStepKind::Checkpoint));

        for step in [GuestMoveStepKind::Checkpoint, GuestMoveStepKind::Transfer] {
            state.begin_step(step, src.fence, src.placement_version)?;
            state.observe_step(step, GuestMoveStepObservation::Confirmed { evidence: id('e') })?;
        }
        state.record_aspen_grant(DestinationPlacementGrant {
            plan: state.plan.identity.clone(),
            attempt: src.attempt.clone(),
            session: src.session.clone(),
            fence: src.fence,
            source_host: "host-a".into(),
            source_placement_version: 1,
            destination_host: "host-b".into(),
            destination_placement_version: 2,
            grant_id: id('8'),
            source_resources_fenced: id('9'),
        })?;

        state.begin_step(GuestMoveStepKind::Restore, src.fence, src.placement_version)?;
        state.observe_step(
            GuestMoveStepKind::Restore,
            GuestMoveStepObservation::Failed {
                cause: "cloud-hypervisor restore rejected".into(),
            },
        )?;
        assert_eq!(state.outcome, GuestMoveOutcome::RestoreFailedPaused);
        assert!(!state.source_retired());
        assert_eq!(state.next_step(), None);
        assert!(
            state
                .begin_step(
                    GuestMoveStepKind::RetireSource,
                    src.fence,
                    src.placement_version
                )
                .is_err()
        );
        Ok(())
    }
}

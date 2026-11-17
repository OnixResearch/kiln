//! Pure ownership and lifecycle for an opt-in retained Lattice session.
//!
//! A retained session is not a second VMM supervisor. This module owns only the
//! placement generation, fence checks and legal lifecycle commands that the
//! existing hosted runner and a future migration transport must consult. It
//! performs no I/O and grants no work, retry or landing authority.

use serde::{Deserialize, Serialize};

use kiln_core::types::{ContentId, Diagnostic};

use crate::backend::{Capabilities, NetworkCapability};

pub mod drain;

pub const RETAINED_SESSION_SCHEMA: &str = "kiln.retained-session.v3";
pub const MAX_HOST_BYTES: usize = 256;
pub const MAX_CAUSE_BYTES: usize = 512;
pub const MAX_VCPUS: u16 = 256;
pub const MAX_MEMORY_MIB: u32 = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionIdentity {
    pub session: ContentId,
    pub execution: ContentId,
    pub vm: ContentId,
    pub work: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub host: String,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedResources {
    pub vcpus: u16,
    pub memory_mib: u32,
    pub slot: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetainedState {
    Receiving,
    Restoring,
    Paused,
    Running,
    Migrating,
    MigratedAway,
    /// Ownership, confinement or cleanup is unknown; capacity stays occupied.
    ReconcileRequired,
    /// Cleanup requested but not yet independently observed; capacity is held.
    Retiring,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetainedEffect {
    Restore,
    Resume,
    Pause,
    Migrate,
    /// Retire the source after migration or pause, releasing capacity only when
    /// the shell observes cleanup.
    Retire,
    Reconcile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Pause,
    Resume,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlStatus {
    Requested,
    InProgress,
    ObservedComplete,
    Rejected,
    Uncertain,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingControl {
    pub operation: ContentId,
    pub kind: ControlKind,
    pub status: ControlStatus,
    pub session: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub placement: Placement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationIntent {
    pub target_host: String,
    pub target_generation: u64,
    pub intent: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerToken {
    pub session: ContentId,
    pub fence: u64,
    pub host: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedSession {
    pub schema: String,
    pub identity: SessionIdentity,
    pub placement: Placement,
    pub resources: RetainedResources,
    pub state: RetainedState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<PendingControl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<MigrationIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// Admit a backend for the retained-session planning surface.
///
/// This checks declared planning capabilities only: snapshot/restore support,
/// no guest network, seccomp and no-new-privs confinement, and x86_64. It does
/// not qualify a live VMM, migration transport or host.
pub fn admit_retained_backend(capabilities: &Capabilities) -> Result<(), Diagnostic> {
    if !capabilities.snapshot
        || capabilities.network != NetworkCapability::NoDevice
        || !capabilities.confinement.seccomp
        || !capabilities.confinement.no_new_privs
        || !capabilities.admits_arch("x86_64")
    {
        return Err(refusal(
            "retained_backend_capabilities",
            "retained.backend",
            "backend does not declare the retained-session planning capabilities",
        ));
    }
    Ok(())
}

impl RetainedSession {
    // r[impl kiln.hosted.retained.lifecycle]
    pub fn admit(
        identity: SessionIdentity,
        placement: Placement,
        resources: RetainedResources,
    ) -> Result<Self, Diagnostic> {
        identity.session.validate()?;
        identity.execution.validate()?;
        identity.vm.validate()?;
        identity.work.validate()?;
        identity.attempt.validate()?;
        validate_host(&placement.host, "retained.host")?;
        if placement.generation == 0
            || identity.fence == 0
            || resources.vcpus == 0
            || resources.vcpus > MAX_VCPUS
            || resources.memory_mib == 0
            || resources.memory_mib > MAX_MEMORY_MIB
            || resources.slot == 0
        {
            return Err(refusal(
                "retained_session_admission",
                "retained.session",
                "generation, fence or resources are zero or over bound",
            ));
        }
        Ok(Self {
            schema: RETAINED_SESSION_SCHEMA.to_string(),
            identity,
            placement,
            resources,
            state: RetainedState::Receiving,
            control: None,
            migration: None,
            cause: None,
        })
    }

    /// Whether this session still needs its occupied capacity retained.
    #[must_use]
    pub fn occupies_capacity(&self) -> bool {
        self.state != RetainedState::Retired
    }

    pub(crate) fn authorize_identity(&self, owner: &OwnerToken) -> Result<(), Diagnostic> {
        owner.session.validate()?;
        validate_host(&owner.host, "retained.owner.host")?;
        if self.schema != RETAINED_SESSION_SCHEMA
            || self.identity.session != owner.session
            || self.identity.fence != owner.fence
            || self.placement.host != owner.host
            || self.placement.generation != owner.generation
        {
            return Err(refusal(
                "retained_owner_stale",
                "retained.owner",
                "owner token does not hold the current placement and fence",
            ));
        }
        Ok(())
    }

    pub(crate) fn authorize(&self, owner: &OwnerToken) -> Result<(), Diagnostic> {
        self.authorize_identity(owner)?;
        if self.state == RetainedState::Retired {
            return Err(refusal(
                "retained_session_retired",
                "retained.state",
                "retired session admits no further commands",
            ));
        }
        Ok(())
    }

    /// Begin restore only from receiving.
    pub fn begin_restore(&mut self, owner: &OwnerToken) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Receiving)?;
        self.state = RetainedState::Restoring;
        Ok(RetainedEffect::Restore)
    }

    /// Observation after restore. Failed confinement never reaches workload
    /// effects and requires reconciliation.
    pub fn observe_restored(
        &mut self,
        owner: &OwnerToken,
        confinement_ok: bool,
    ) -> Result<Option<RetainedEffect>, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Restoring)?;
        if confinement_ok {
            self.state = RetainedState::Paused;
            Ok(None)
        } else {
            self.state = RetainedState::ReconcileRequired;
            self.cause = Some("confinement_unobserved".to_string());
            Ok(Some(RetainedEffect::Reconcile))
        }
    }

    pub fn request_control(
        &mut self,
        owner: &OwnerToken,
        operation: ContentId,
        kind: ControlKind,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        operation.validate()?;
        if self
            .control
            .as_ref()
            .is_some_and(|c| c.operation == operation)
        {
            return Err(refusal(
                "retained_control_replay",
                "retained.control",
                "control operation ID cannot be reused",
            ));
        }
        if self.control.as_ref().is_some_and(|c| {
            matches!(
                c.status,
                ControlStatus::Requested | ControlStatus::InProgress | ControlStatus::Uncertain
            )
        }) {
            return Err(refusal(
                "retained_control_pending",
                "retained.control",
                "unfinished control must be reconciled first",
            ));
        }
        self.require(match kind {
            ControlKind::Pause => RetainedState::Running,
            ControlKind::Resume => RetainedState::Paused,
        })?;
        self.control = Some(PendingControl {
            operation,
            kind,
            status: ControlStatus::Requested,
            session: self.identity.session.clone(),
            attempt: self.identity.attempt.clone(),
            fence: self.identity.fence,
            placement: self.placement.clone(),
        });
        Ok(match kind {
            ControlKind::Pause => RetainedEffect::Pause,
            ControlKind::Resume => RetainedEffect::Resume,
        })
    }

    pub fn control_in_progress(
        &mut self,
        owner: &OwnerToken,
        operation: &ContentId,
    ) -> Result<(), Diagnostic> {
        self.authorize_identity(owner)?;
        let control = self.match_control(operation)?;
        if control.status != ControlStatus::Requested {
            return Err(refusal(
                "retained_control_phase",
                "retained.control",
                "control is not newly requested",
            ));
        }
        control.status = ControlStatus::InProgress;
        Ok(())
    }

    pub fn observe_control(
        &mut self,
        owner: &OwnerToken,
        operation: &ContentId,
        observed: bool,
    ) -> Result<(), Diagnostic> {
        self.authorize_identity(owner)?;
        let control = self.match_control(operation)?;
        let kind = control.kind;
        if !matches!(
            control.status,
            ControlStatus::Requested | ControlStatus::InProgress | ControlStatus::Uncertain
        ) {
            return Err(refusal(
                "retained_control_phase",
                "retained.control",
                "control has no unfinished observation",
            ));
        }
        if observed {
            control.status = ControlStatus::ObservedComplete;
            self.state = match kind {
                ControlKind::Pause => RetainedState::Paused,
                ControlKind::Resume => RetainedState::Running,
            };
            self.cause = None;
        } else {
            control.status = ControlStatus::Uncertain;
            self.state = RetainedState::ReconcileRequired;
            self.cause = Some("control_observation_unknown".into());
        }
        Ok(())
    }

    pub fn reject_control(
        &mut self,
        owner: &OwnerToken,
        operation: &ContentId,
    ) -> Result<(), Diagnostic> {
        self.authorize_identity(owner)?;
        let control = self.match_control(operation)?;
        if !matches!(
            control.status,
            ControlStatus::Requested | ControlStatus::InProgress | ControlStatus::Uncertain
        ) {
            return Err(refusal(
                "retained_control_phase",
                "retained.control",
                "control cannot be rejected in its current phase",
            ));
        }
        control.status = ControlStatus::Rejected;
        Ok(())
    }

    fn match_control(&mut self, operation: &ContentId) -> Result<&mut PendingControl, Diagnostic> {
        let c = self.control.as_mut().ok_or_else(|| {
            refusal(
                "retained_control_missing",
                "retained.control",
                "no control is pending",
            )
        })?;
        if &c.operation != operation
            || c.session != self.identity.session
            || c.attempt != self.identity.attempt
            || c.fence != self.identity.fence
            || c.placement != self.placement
        {
            return Err(refusal(
                "retained_control_stale",
                "retained.control",
                "control reply does not match operation and current owner",
            ));
        }
        Ok(c)
    }

    /// Begin one outbound migration from the paused source.
    pub fn begin_migration(
        &mut self,
        owner: &OwnerToken,
        target_host: String,
        target_generation: u64,
        intent: ContentId,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        if self.control.as_ref().is_some_and(|c| {
            matches!(
                c.status,
                ControlStatus::Requested | ControlStatus::InProgress | ControlStatus::Uncertain
            )
        }) {
            return Err(refusal(
                "retained_control_pending",
                "retained.control",
                "unfinished control blocks migration",
            ));
        }
        self.require(RetainedState::Paused)?;
        validate_host(&target_host, "retained.migration.target")?;
        intent.validate()?;
        if target_generation <= self.placement.generation || target_host == self.placement.host {
            return Err(refusal(
                "retained_migration_target",
                "retained.migration",
                "migration target must advance the placement generation on another host",
            ));
        }
        self.migration = Some(MigrationIntent {
            target_host,
            target_generation,
            intent,
        });
        self.state = RetainedState::Migrating;
        Ok(RetainedEffect::Migrate)
    }

    /// Observe the source as migrated away. The source still needs retirement;
    /// migration success is not placement commit or cleanup.
    pub fn observe_migrated_away(
        &mut self,
        owner: &OwnerToken,
        evidence: &drain::CutoverEvidence,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        self.require(RetainedState::Migrating)?;
        let intent = self.migration.as_ref().ok_or_else(|| {
            refusal(
                "retained_migration_missing",
                "retained.migration",
                "migrated-away observation has no migration intent",
            )
        })?;
        let target = Placement {
            host: intent.target_host.clone(),
            generation: intent.target_generation,
        };
        let unknown = drain::DispatchObservation {
            sender_exit: None,
            source: drain::VmObservation::Unknown,
            destination: drain::VmObservation::Unknown,
            destination_ever_wrote: false,
        };
        if drain::after_dispatch(self, &intent.intent, &target, Some(evidence), &unknown)
            != drain::AfterDispatch::TargetOwned
        {
            return Err(refusal(
                "retained_cutover_unconfirmed",
                "retained.migration",
                "migrated-away requires exact durable target acknowledgement, placement commit and source resource fencing",
            ));
        }
        self.state = RetainedState::MigratedAway;
        Ok(RetainedEffect::Retire)
    }

    /// Request cleanup without releasing the source reservation. The shell must
    /// journal this transition before requesting an owned host effect.
    pub fn begin_retire(&mut self, owner: &OwnerToken) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        if self.control.as_ref().is_some_and(|c| {
            matches!(
                c.status,
                ControlStatus::Requested | ControlStatus::InProgress | ControlStatus::Uncertain
            )
        }) {
            return Err(refusal(
                "retained_control_pending",
                "retained.control",
                "unfinished control blocks retirement",
            ));
        }
        if !matches!(
            self.state,
            RetainedState::Paused | RetainedState::MigratedAway
        ) {
            return Err(refusal(
                "retained_retire_phase",
                "retained.state",
                "only a paused or migrated-away session may request retirement",
            ));
        }
        self.state = RetainedState::Retiring;
        Ok(RetainedEffect::Retire)
    }

    /// Only observed absence of the exact owned process/cgroup and released
    /// resource attachments releases capacity. A send result or lost lease is
    /// never sufficient. Reconciliation may supply this observation later.
    pub fn observe_retired(
        &mut self,
        owner: &OwnerToken,
        cleanup_observed: bool,
    ) -> Result<(), Diagnostic> {
        self.authorize(owner)?;
        if !matches!(
            self.state,
            RetainedState::Retiring | RetainedState::ReconcileRequired
        ) {
            return Err(refusal(
                "retained_retire_observation_phase",
                "retained.state",
                "retirement observation requires cleanup intent or reconciliation",
            ));
        }
        if cleanup_observed {
            self.state = RetainedState::Retired;
            self.cause = None;
        } else {
            self.state = RetainedState::ReconcileRequired;
            self.cause = Some("cleanup_unobserved".to_string());
        }
        Ok(())
    }

    /// Mark an uncertain ownership/confinement/cleanup observation. Capacity
    /// stays occupied until a later observed retirement.
    pub fn require_reconciliation(
        &mut self,
        owner: &OwnerToken,
        cause: &str,
    ) -> Result<RetainedEffect, Diagnostic> {
        self.authorize(owner)?;
        if cause.is_empty() || cause.len() > MAX_CAUSE_BYTES || cause.chars().any(char::is_control)
        {
            return Err(refusal(
                "retained_cause",
                "retained.cause",
                "retained cause is empty, oversized or malformed",
            ));
        }
        self.state = RetainedState::ReconcileRequired;
        self.cause = Some(cause.to_string());
        Ok(RetainedEffect::Reconcile)
    }

    fn require(&self, expected: RetainedState) -> Result<(), Diagnostic> {
        if self.state != expected {
            return Err(refusal(
                "retained_session_phase",
                "retained.state",
                "retained session phase does not admit this command",
            ));
        }
        Ok(())
    }
}

fn validate_host(host: &str, field: &str) -> Result<(), Diagnostic> {
    if host.is_empty()
        || host.len() > MAX_HOST_BYTES
        || host.chars().any(char::is_control)
        || host.chars().any(char::is_whitespace)
    {
        return Err(refusal(
            "retained_host",
            field,
            "retained host is empty, oversized or malformed",
        ));
    }
    Ok(())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn identity() -> SessionIdentity {
        SessionIdentity {
            session: content('1'),
            execution: content('4'),
            vm: content('5'),
            work: content('2'),
            work_revision: 4,
            attempt: content('3'),
            fence: 9,
        }
    }

    fn owner(session: &RetainedSession) -> OwnerToken {
        OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: session.placement.host.clone(),
            generation: session.placement.generation,
        }
    }

    fn session() -> RetainedSession {
        RetainedSession::admit(
            identity(),
            Placement {
                host: "host-a".to_string(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 2,
                memory_mib: 1024,
                slot: 3,
            },
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn paused(session: &mut RetainedSession) -> OwnerToken {
        let token = owner(session);
        session
            .begin_restore(&token)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .observe_restored(&token, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        token
    }

    // r[verify kiln.hosted.retained.lifecycle] r[verify kiln.hosted.retained.lifecycle.scenario.restore]
    #[test]
    fn requested_resume_is_not_observed_running_until_exact_confirmation() {
        let mut session = session();
        let token = paused(&mut session);
        let operation = content('9');
        assert!(matches!(
            session.request_control(&token, operation.clone(), ControlKind::Resume),
            Ok(RetainedEffect::Resume)
        ));
        assert_eq!(session.state, RetainedState::Paused);
        assert_eq!(
            session.control.as_ref().map(|control| control.status),
            Some(ControlStatus::Requested)
        );
        let encoded = serde_json::to_string(&session).unwrap_or_else(|_| unreachable!());
        let restarted: RetainedSession =
            serde_json::from_str(&encoded).unwrap_or_else(|_| unreachable!());
        assert_eq!(restarted.control, session.control);
        assert!(session.begin_retire(&token).is_err());
        assert!(
            session
                .begin_migration(&token, "host-b".into(), 2, content('a'))
                .is_err()
        );
        session
            .control_in_progress(&token, &operation)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(
            session
                .observe_control(&token, &content('8'), true)
                .is_err()
        );
        session
            .observe_control(&token, &operation, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Running);
        assert_eq!(
            session.control.as_ref().map(|control| control.status),
            Some(ControlStatus::ObservedComplete)
        );
        let rejected = content('6');
        session
            .request_control(&token, rejected.clone(), ControlKind::Pause)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .reject_control(&token, &rejected)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Running);
        assert_eq!(
            session.control.as_ref().map(|control| control.status),
            Some(ControlStatus::Rejected)
        );
        assert!(
            session
                .request_control(&token, rejected, ControlKind::Pause)
                .is_err()
        );
    }

    // r[verify kiln.hosted.retained.lifecycle.scenario.confinement]
    #[test]
    fn failed_confinement_requires_reconciliation_and_never_runs() {
        let mut session = session();
        let token = owner(&session);
        session
            .begin_restore(&token)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            session
                .observe_restored(&token, false)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            Some(RetainedEffect::Reconcile)
        );
        assert_eq!(session.state, RetainedState::ReconcileRequired);
        assert!(session.occupies_capacity());
        assert!(
            session
                .request_control(&token, content('7'), ControlKind::Resume)
                .is_err()
        );
    }

    // r[verify kiln.hosted.retained.lifecycle.scenario.stale]
    #[test]
    fn stale_owner_or_generation_cannot_resume_or_retire() {
        let mut session = session();
        let token = paused(&mut session);
        let mut stale = token.clone();
        stale.generation += 1;
        assert!(
            session
                .request_control(&stale, content('7'), ControlKind::Resume)
                .is_err()
        );
        stale = token.clone();
        stale.fence += 1;
        assert!(
            session
                .request_control(&stale, content('7'), ControlKind::Resume)
                .is_err()
        );
        assert_eq!(session.state, RetainedState::Paused);
    }

    // r[verify kiln.hosted.retained.lifecycle.scenario.migration]
    #[test]
    fn migrated_away_source_cannot_resume_and_retirement_is_separate() {
        let mut session = session();
        let token = paused(&mut session);
        session
            .begin_migration(&token, "host-b".to_string(), 2, content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Migrating);
        // Retirement and a second migration wait for this migration's outcome.
        assert!(session.begin_retire(&token).is_err());
        assert!(
            session
                .begin_migration(&token, "host-c".to_string(), 3, content('5'))
                .is_err()
        );
        let mut evidence = drain::CutoverEvidence {
            operation: content('4'),
            session: session.identity.session.clone(),
            execution: session.identity.execution.clone(),
            vm: session.identity.vm.clone(),
            attempt: session.identity.attempt.clone(),
            fence: session.identity.fence,
            source: session.placement.clone(),
            checkpoint: None,
            target: Placement {
                host: "host-b".to_string(),
                generation: 2,
            },
            durable_target_commit: Some(content('5')),
            source_volume_fenced: Some(content('6')),
            source_effects_fenced: Some(content('7')),
            destination_durable_ack: None,
        };
        assert!(session.observe_migrated_away(&token, &evidence).is_err());
        assert!(session.occupies_capacity());
        evidence.destination_durable_ack = Some(content('8'));
        assert_eq!(
            session
                .observe_migrated_away(&token, &evidence)
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            RetainedEffect::Retire
        );
        assert_eq!(session.state, RetainedState::MigratedAway);
        assert!(
            session
                .request_control(&token, content('7'), ControlKind::Resume)
                .is_err()
        );
        assert!(session.occupies_capacity());
        session
            .begin_retire(&token)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Retiring);
        assert!(session.occupies_capacity());
        session
            .observe_retired(&token, false)
            .unwrap_or_else(|_| unreachable!());
        assert!(session.occupies_capacity());
        assert!(
            session
                .request_control(&token, content('7'), ControlKind::Resume)
                .is_err()
        );
        session
            .observe_retired(&token, true)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(session.state, RetainedState::Retired);
        assert!(!session.occupies_capacity());
        assert!(session.begin_retire(&token).is_err());
    }

    #[test]
    // r[verify kiln.hosted.retained.lifecycle]
    fn retained_backend_requires_snapshot_network_and_confinement_facts() {
        use crate::backend::{CloudHypervisor, Firecracker, VmmBackend};
        let mut capabilities = Firecracker.capabilities();
        assert!(admit_retained_backend(&capabilities).is_ok());
        capabilities = CloudHypervisor.capabilities();
        assert!(admit_retained_backend(&capabilities).is_ok());
        capabilities.confinement.seccomp = false;
        assert!(admit_retained_backend(&capabilities).is_err());
        capabilities = Firecracker.capabilities();
        capabilities.snapshot = false;
        assert!(admit_retained_backend(&capabilities).is_err());
    }

    #[test]
    fn retained_session_round_trips_and_rejects_bad_admission() {
        let session = session();
        let json = serde_json::to_string(&session).unwrap_or_else(|_| unreachable!());
        let decoded: RetainedSession =
            serde_json::from_str(&json).unwrap_or_else(|_| unreachable!());
        assert_eq!(decoded, session);
        assert!(
            RetainedSession::admit(
                identity(),
                Placement {
                    host: "host a".to_string(),
                    generation: 1
                },
                RetainedResources {
                    vcpus: 2,
                    memory_mib: 1024,
                    slot: 1
                }
            )
            .is_err()
        );
    }
}

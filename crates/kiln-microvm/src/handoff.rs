//! Pure paused-handoff manifest admission and ordering.
//!
//! A handoff binds one retained session, execution, source/target placement,
//! exact disk generation, container profile and checkpoint objects. This module
//! performs no copy, transfer, VMM operation or ownership commit; it refuses
//! ordering and identity mistakes before a shell requests those effects.

use serde::{Deserialize, Serialize};

use crate::retained::{ControlStatus, OwnerToken, Placement, RetainedSession, RetainedState};
use kiln_core::types::{ContentId, Diagnostic, ExternalRef};

pub const CHECKPOINT_SCHEMA: &str = "kiln.checkpoint-manifest.v2";
pub const HANDOFF_SCHEMA: &str = "kiln.paused-handoff.v3";
pub const MAX_CAUSE_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointProfile {
    pub image: ContentId,
    pub kernel: ContentId,
    pub hypervisor: ContentId,
    pub cpu: ContentId,
    pub initrd: ContentId,
    pub devices: ContentId,
    pub memory_backing: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointObjects {
    pub ram: ContentId,
    pub device_state: ContentId,
    pub disk: ContentId,
    pub workspace: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointManifest {
    pub schema: String,
    pub session: ContentId,
    pub vm: ContentId,
    pub work: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub execution: ContentId,
    pub source: Placement,
    pub target: Placement,
    pub operation: ContentId,
    pub profile: CheckpointProfile,
    pub disk_generation: u64,
    pub objects: CheckpointObjects,
}

/// Facts the shell resolved independently before accepting a checkpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointExpectation {
    pub session: ContentId,
    pub vm: ContentId,
    pub work: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub execution: ContentId,
    pub source: Placement,
    pub target: Placement,
    pub operation: ContentId,
    pub disk: ContentId,
    pub disk_generation: u64,
    pub profile: CheckpointProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffPhase {
    Intent,
    Quiesced,
    SourcePaused,
    ObjectsVerified,
    TargetAdmitted,
    Resumed,
    SourceRetired,
    ReconcileRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetAdmissionReceipt {
    pub reference: ExternalRef,
    pub operation: ContentId,
    pub checkpoint: ContentId,
    pub source: Placement,
    pub target: Placement,
    pub target_session: ContentId,
    pub target_fence: u64,
    pub cutover: crate::retained::drain::CutoverEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PausedHandoff {
    pub schema: String,
    pub operation: ContentId,
    pub origin: crate::retained::SessionIdentity,
    pub source: Placement,
    pub target: Placement,
    pub phase: HandoffPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<CheckpointManifest>,
    /// Exact independently observed target-admission receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_receipt: Option<TargetAdmissionReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_identity: Option<ContentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

// r[impl kiln.hosted.handoff.order]
impl PausedHandoff {
    /// Record migration intent under the current paused/running owner.
    ///
    /// The target must advance the placement generation on another host.
    pub fn begin(
        session: &RetainedSession,
        owner: &OwnerToken,
        target: Placement,
        operation: ContentId,
    ) -> Result<Self, Diagnostic> {
        session.authorize(owner)?;
        if session.control.as_ref().is_some_and(|control| {
            matches!(
                control.status,
                ControlStatus::Requested | ControlStatus::InProgress | ControlStatus::Uncertain
            )
        }) {
            return Err(refusal(
                "handoff_control_pending",
                "handoff.session",
                "unfinished control must be observed before handoff",
            ));
        }
        if !matches!(
            session.state,
            RetainedState::Paused | RetainedState::Running
        ) {
            return Err(refusal(
                "handoff_session_phase",
                "handoff.session",
                "handoff requires a paused or running retained session",
            ));
        }
        validate_target(&session.placement, &target)?;
        operation.validate()?;
        Ok(Self {
            schema: HANDOFF_SCHEMA.to_string(),
            operation,
            origin: session.identity.clone(),
            source: session.placement.clone(),
            target,
            phase: HandoffPhase::Intent,
            manifest: None,
            target_receipt: None,
            manifest_identity: None,
            cause: None,
        })
    }
    /// Record that new external effects are quiesced. The source may still run.
    pub fn record_quiesced(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
    ) -> Result<(), Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::Intent])?;
        self.phase = HandoffPhase::Quiesced;
        Ok(())
    }

    /// Record that the source VM is paused before any snapshot/transfer.
    pub fn record_source_paused(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
    ) -> Result<(), Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::Quiesced])?;
        if session.state != RetainedState::Paused {
            return Err(refusal(
                "handoff_source_running",
                "handoff.source",
                "source must be paused before checkpoint capture",
            ));
        }
        self.phase = HandoffPhase::SourcePaused;
        Ok(())
    }

    /// Admit an exact checkpoint manifest after all objects were captured.
    ///
    /// Admission does not prove object durability or transfer; it binds the
    /// exact identities and rejects an old-memory/new-disk mismatch.
    pub fn record_objects_verified(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
        manifest: CheckpointManifest,
        expected: CheckpointExpectation,
    ) -> Result<ContentId, Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::SourcePaused])?;
        if session.state != RetainedState::Paused {
            return Err(refusal(
                "handoff_source_running",
                "handoff.source",
                "source must remain paused while checkpoint objects are verified",
            ));
        }
        if expected.session != self.origin.session
            || expected.vm != self.origin.vm
            || expected.work != self.origin.work
            || expected.work_revision != self.origin.work_revision
            || expected.attempt != self.origin.attempt
            || expected.fence != self.origin.fence
            || expected.execution != self.origin.execution
            || expected.source != self.source
            || expected.target != self.target
            || expected.operation != self.operation
        {
            return Err(refusal(
                "handoff_expectation_binding",
                "handoff.manifest",
                "independent checkpoint expectations do not match the originating handoff",
            ));
        }
        let identity = admit_checkpoint(&manifest, &expected)?;
        self.manifest = Some(manifest);
        self.manifest_identity = Some(identity.clone());
        self.phase = HandoffPhase::ObjectsVerified;
        Ok(identity)
    }

    /// Record a destination receipt explicitly scoped to this operation and checkpoint.
    pub fn record_target_admitted(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
        target_owner: &OwnerToken,
        receipt: &TargetAdmissionReceipt,
    ) -> Result<(), Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::ObjectsVerified])?;
        receipt.reference.validate()?;
        use crate::retained::drain::{
            AfterDispatch, DispatchObservation, VmObservation, after_dispatch,
        };
        let observed = DispatchObservation {
            sender_exit: None,
            source: VmObservation::Paused,
            destination: VmObservation::Paused,
            destination_ever_wrote: false,
        };
        let manifest = self.manifest.as_ref().ok_or_else(|| {
            refusal(
                "handoff_manifest_missing",
                "handoff.manifest",
                "target admission requires a verified checkpoint manifest",
            )
        })?;
        let identity = self.manifest_identity.as_ref().ok_or_else(|| {
            refusal(
                "handoff_manifest_missing",
                "handoff.manifest",
                "target admission requires the exact admitted manifest identity",
            )
        })?;
        let expected = CheckpointExpectation {
            session: self.origin.session.clone(),
            vm: self.origin.vm.clone(),
            work: self.origin.work.clone(),
            work_revision: self.origin.work_revision,
            attempt: self.origin.attempt.clone(),
            fence: self.origin.fence,
            execution: self.origin.execution.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            operation: self.operation.clone(),
            disk: manifest.objects.disk.clone(),
            disk_generation: manifest.disk_generation,
            profile: manifest.profile.clone(),
        };
        if receipt.operation != self.operation
            || receipt.checkpoint != *identity
            || receipt.source != self.source
            || receipt.target != self.target
            || receipt.target_session != self.origin.session
            || receipt.target_fence != self.origin.fence
            || target_owner.session != self.origin.session
            || target_owner.fence != self.origin.fence
            || target_owner.host != self.target.host
            || target_owner.generation != self.target.generation
            || receipt.target_session != target_owner.session
            || receipt.target_fence != target_owner.fence
            || admit_checkpoint(manifest, &expected).as_ref() != Ok(identity)
            || receipt.cutover.checkpoint.as_ref() != Some(identity)
            || after_dispatch(
                session,
                &self.operation,
                &self.target,
                Some(&receipt.cutover),
                &observed,
            ) != AfterDispatch::TargetOwned
        {
            return Err(refusal(
                "handoff_target_evidence",
                "handoff.target",
                "target receipt must bind the exact manifest, operation, placement, owner and cutover/fencing facts",
            ));
        }
        self.target_receipt = Some(receipt.clone());
        self.phase = HandoffPhase::TargetAdmitted;
        Ok(())
    }

    /// Record the explicit destination resume after target admission.
    pub fn record_resumed(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
        target_owner: &OwnerToken,
    ) -> Result<(), Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::TargetAdmitted])?;
        if !self.target_admission_valid(session) {
            return Err(refusal(
                "handoff_resume_evidence",
                "handoff.resume",
                "resume requires the persisted exact scoped receipt and durable placement/fencing/acknowledgement evidence",
            ));
        }
        if target_owner.session != self.origin.session
            || target_owner.fence != self.origin.fence
            || target_owner.host != self.target.host
            || target_owner.generation != self.target.generation
        {
            return Err(refusal(
                "handoff_resume_owner",
                "handoff.resume",
                "resume requires the admitted target placement",
            ));
        }
        self.phase = HandoffPhase::Resumed;
        Ok(())
    }

    /// Record observed source retirement after destination ownership.
    pub fn record_source_retired(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
    ) -> Result<(), Diagnostic> {
        self.authorize_identity(session, owner)?;
        if self.phase != HandoffPhase::Resumed
            || session.state != RetainedState::Retired
            || !self.target_admission_valid(session)
        {
            return Err(refusal(
                "handoff_source_not_retired",
                "handoff.source",
                "source retirement requires observed cleanup of the exact source placement",
            ));
        }
        self.phase = HandoffPhase::SourceRetired;
        Ok(())
    }

    /// Enter reconciliation without claiming a target or releasing capacity.
    pub fn require_reconciliation(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
        cause: &str,
    ) -> Result<(), Diagnostic> {
        self.authorize_identity(session, owner)?;
        if cause.is_empty() || cause.len() > MAX_CAUSE_BYTES || cause.chars().any(char::is_control)
        {
            return Err(refusal(
                "handoff_cause",
                "handoff.cause",
                "handoff cause is empty, oversized or malformed",
            ));
        }
        self.phase = HandoffPhase::ReconcileRequired;
        self.cause = Some(cause.to_string());
        Ok(())
    }

    fn target_admission_valid(&self, session: &RetainedSession) -> bool {
        use crate::retained::drain::{
            AfterDispatch, DispatchObservation, VmObservation, after_dispatch,
        };
        let (Some(manifest), Some(identity), Some(receipt)) = (
            self.manifest.as_ref(),
            self.manifest_identity.as_ref(),
            self.target_receipt.as_ref(),
        ) else {
            return false;
        };
        let expected = CheckpointExpectation {
            session: self.origin.session.clone(),
            vm: self.origin.vm.clone(),
            work: self.origin.work.clone(),
            work_revision: self.origin.work_revision,
            attempt: self.origin.attempt.clone(),
            fence: self.origin.fence,
            execution: self.origin.execution.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            operation: self.operation.clone(),
            disk: manifest.objects.disk.clone(),
            disk_generation: manifest.disk_generation,
            profile: manifest.profile.clone(),
        };
        receipt.reference.validate().is_ok()
            && receipt.operation == self.operation
            && receipt.checkpoint == *identity
            && receipt.source == self.source
            && receipt.target == self.target
            && receipt.target_session == self.origin.session
            && receipt.target_fence == self.origin.fence
            && admit_checkpoint(manifest, &expected).as_ref() == Ok(identity)
            && receipt.cutover.checkpoint.as_ref() == Some(identity)
            && after_dispatch(
                session,
                &self.operation,
                &self.target,
                Some(&receipt.cutover),
                &DispatchObservation {
                    sender_exit: None,
                    source: VmObservation::Unknown,
                    destination: VmObservation::Unknown,
                    destination_ever_wrote: false,
                },
            ) == AfterDispatch::TargetOwned
    }

    fn authorize_identity(
        &self,
        session: &RetainedSession,
        owner: &OwnerToken,
    ) -> Result<(), Diagnostic> {
        session.authorize_identity(owner)?;
        self.require_origin(session)
    }

    fn require_origin(&self, session: &RetainedSession) -> Result<(), Diagnostic> {
        if self.schema != HANDOFF_SCHEMA {
            return Err(refusal(
                "handoff_schema",
                "handoff.schema",
                "handoff schema is unsupported",
            ));
        }
        if session.identity != self.origin || session.placement != self.source {
            return Err(refusal(
                "handoff_origin_mismatch",
                "handoff.session",
                "supplied session does not match the immutable originating identity and placement",
            ));
        }
        Ok(())
    }

    fn authorize(
        &self,
        session: &RetainedSession,
        owner: &OwnerToken,
        allowed: &[HandoffPhase],
    ) -> Result<(), Diagnostic> {
        session.authorize(owner)?;
        self.require_origin(session)?;
        if !allowed.contains(&self.phase) {
            return Err(refusal(
                "handoff_phase",
                "handoff.phase",
                "handoff phase does not admit this command",
            ));
        }
        Ok(())
    }
}

/// Admit one checkpoint manifest against independently resolved facts.
// r[impl kiln.hosted.handoff.manifest]
pub fn admit_checkpoint(
    manifest: &CheckpointManifest,
    expected: &CheckpointExpectation,
) -> Result<ContentId, Diagnostic> {
    if manifest.schema != CHECKPOINT_SCHEMA
        || manifest.session != expected.session
        || manifest.vm != expected.vm
        || manifest.work != expected.work
        || manifest.work_revision != expected.work_revision
        || manifest.attempt != expected.attempt
        || manifest.fence != expected.fence
        || manifest.execution != expected.execution
        || manifest.source != expected.source
        || manifest.target != expected.target
        || manifest.operation != expected.operation
        || manifest.disk_generation == 0
        || manifest.disk_generation != expected.disk_generation
        || manifest.objects.disk != expected.disk
        || manifest.profile != expected.profile
    {
        return Err(refusal(
            "handoff_manifest_binding",
            "handoff.manifest",
            "checkpoint manifest does not bind the exact session, placement, disk or operation",
        ));
    }
    validate_target(&manifest.source, &manifest.target)?;
    let identities = [
        &manifest.session,
        &manifest.vm,
        &manifest.work,
        &manifest.attempt,
        &manifest.execution,
        &manifest.operation,
        &manifest.profile.image,
        &manifest.profile.kernel,
        &manifest.profile.hypervisor,
        &manifest.profile.cpu,
        &manifest.profile.initrd,
        &manifest.profile.devices,
        &manifest.profile.memory_backing,
        &manifest.objects.ram,
        &manifest.objects.device_state,
        &manifest.objects.disk,
        &manifest.objects.workspace,
    ];
    for identity in identities {
        identity.validate()?;
    }
    let mut objects = std::collections::BTreeSet::new();
    for object in [
        &manifest.objects.ram,
        &manifest.objects.device_state,
        &manifest.objects.disk,
        &manifest.objects.workspace,
    ] {
        if !objects.insert(object) {
            return Err(refusal(
                "handoff_objects",
                "handoff.objects",
                "checkpoint objects must be distinct identities",
            ));
        }
    }
    let mut bytes = Vec::new();
    for value in [
        CHECKPOINT_SCHEMA,
        manifest.session.as_str(),
        manifest.vm.as_str(),
        manifest.work.as_str(),
    ] {
        push(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&manifest.work_revision.to_be_bytes());
    push(&mut bytes, manifest.attempt.as_str())?;
    bytes.extend_from_slice(&manifest.fence.to_be_bytes());
    push(&mut bytes, manifest.execution.as_str())?;
    for placement in [&manifest.source, &manifest.target] {
        push(&mut bytes, &placement.host)?;
        bytes.extend_from_slice(&placement.generation.to_be_bytes());
    }
    push(&mut bytes, manifest.operation.as_str())?;
    for identity in [
        &manifest.profile.image,
        &manifest.profile.kernel,
        &manifest.profile.hypervisor,
        &manifest.profile.cpu,
        &manifest.profile.initrd,
        &manifest.profile.devices,
        &manifest.profile.memory_backing,
    ] {
        push(&mut bytes, identity.as_str())?;
    }
    bytes.extend_from_slice(&manifest.disk_generation.to_be_bytes());
    for object in [
        &manifest.objects.ram,
        &manifest.objects.device_state,
        &manifest.objects.disk,
        &manifest.objects.workspace,
    ] {
        push(&mut bytes, object.as_str())?;
    }
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

fn validate_target(source: &Placement, target: &Placement) -> Result<(), Diagnostic> {
    if source.host.is_empty()
        || target.host.is_empty()
        || source.host == target.host
        || target.generation <= source.generation
        || target.generation == 0
        || source.generation == 0
        || [&source.host, &target.host].into_iter().any(|host| {
            host.len() > crate::retained::MAX_HOST_BYTES
                || host
                    .chars()
                    .any(|character| character.is_control() || character.is_whitespace())
        })
    {
        return Err(refusal(
            "handoff_target",
            "handoff.target",
            "target must be another host with a higher placement generation",
        ));
    }
    Ok(())
}

fn push(bytes: &mut Vec<u8>, value: &str) -> Result<(), Diagnostic> {
    let length = u32::try_from(value.len()).map_err(|_| {
        refusal(
            "handoff_encoding",
            "handoff.manifest",
            "handoff field exceeds its encoding bound",
        )
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained::{ControlKind, RetainedResources, SessionIdentity};

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn cutover(
        session: &RetainedSession,
        checkpoint: ContentId,
    ) -> crate::retained::drain::CutoverEvidence {
        crate::retained::drain::CutoverEvidence {
            operation: content('4'),
            session: session.identity.session.clone(),
            execution: session.identity.execution.clone(),
            vm: session.identity.vm.clone(),
            attempt: session.identity.attempt.clone(),
            fence: session.identity.fence,
            source: session.placement.clone(),
            target: target(),
            checkpoint: Some(checkpoint),
            durable_target_commit: Some(content('e')),
            source_volume_fenced: Some(content('f')),
            source_effects_fenced: Some(content('5')),
            destination_durable_ack: Some(content('6')),
        }
    }

    fn target_receipt(
        session: &RetainedSession,
        checkpoint: ContentId,
        cutover: crate::retained::drain::CutoverEvidence,
    ) -> TargetAdmissionReceipt {
        TargetAdmissionReceipt {
            reference: ExternalRef::new("lattice", "candidate", "target")
                .unwrap_or_else(|_| unreachable!()),
            operation: content('4'),
            checkpoint,
            source: session.placement.clone(),
            target: target(),
            target_session: session.identity.session.clone(),
            target_fence: session.identity.fence,
            cutover,
        }
    }

    fn session() -> RetainedSession {
        RetainedSession::admit(
            SessionIdentity {
                session: content('1'),
                execution: content('e'),
                vm: content('f'),
                work: content('2'),
                work_revision: 3,
                attempt: content('3'),
                fence: 9,
            },
            Placement {
                host: "host-a".to_string(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 2,
                memory_mib: 1024,
                slot: 1,
            },
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn owner(session: &RetainedSession) -> OwnerToken {
        OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: session.placement.host.clone(),
            generation: session.placement.generation,
        }
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

    fn target() -> Placement {
        Placement {
            host: "host-b".to_string(),
            generation: 2,
        }
    }

    fn manifest(session: &RetainedSession, target: Placement) -> CheckpointManifest {
        CheckpointManifest {
            schema: CHECKPOINT_SCHEMA.to_string(),
            session: session.identity.session.clone(),
            vm: session.identity.vm.clone(),
            work: session.identity.work.clone(),
            work_revision: session.identity.work_revision,
            attempt: session.identity.attempt.clone(),
            fence: session.identity.fence,
            execution: session.identity.execution.clone(),
            source: session.placement.clone(),
            target,
            operation: content('4'),
            profile: CheckpointProfile {
                image: content('5'),
                kernel: content('6'),
                hypervisor: content('7'),
                cpu: content('8'),
                initrd: content('9'),
                devices: content('e'),
                memory_backing: content('f'),
            },
            disk_generation: 11,
            objects: CheckpointObjects {
                ram: content('a'),
                device_state: content('b'),
                disk: content('c'),
                workspace: content('d'),
            },
        }
    }

    fn expectation(session: &RetainedSession) -> CheckpointExpectation {
        CheckpointExpectation {
            session: session.identity.session.clone(),
            vm: session.identity.vm.clone(),
            work: session.identity.work.clone(),
            work_revision: session.identity.work_revision,
            attempt: session.identity.attempt.clone(),
            fence: session.identity.fence,
            execution: session.identity.execution.clone(),
            source: session.placement.clone(),
            target: target(),
            operation: content('4'),
            disk: content('c'),
            disk_generation: 11,
            profile: CheckpointProfile {
                image: content('5'),
                kernel: content('6'),
                hypervisor: content('7'),
                cpu: content('8'),
                initrd: content('9'),
                devices: content('e'),
                memory_backing: content('f'),
            },
        }
    }

    #[test]
    fn pending_pause_cannot_start_handoff_but_observed_pause_can() {
        let mut session = session();
        let owner = paused(&mut session);
        let resume = content('8');
        session
            .request_control(&owner, resume.clone(), ControlKind::Resume)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .control_in_progress(&owner, &resume)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .observe_control(&owner, &resume, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let pause = content('9');
        session
            .request_control(&owner, pause.clone(), ControlKind::Pause)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(session.state, RetainedState::Running);
        assert!(PausedHandoff::begin(&session, &owner, target(), content('a')).is_err());
        session
            .control_in_progress(&owner, &pause)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .observe_control(&owner, &pause, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(PausedHandoff::begin(&session, &owner, target(), content('a')).is_ok());
    }

    // r[verify kiln.hosted.handoff.manifest] r[verify kiln.hosted.handoff.order] r[verify kiln.hosted.handoff.order.scenario.complete]
    #[test]
    fn paused_handoff_orders_quiesce_capture_target_and_retirement() {
        let mut session = session();
        let owner = paused(&mut session);
        let target = target();
        let mut handoff = PausedHandoff::begin(&session, &owner, target.clone(), content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut pending_intent = handoff.clone();
        handoff
            .record_quiesced(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff
            .record_source_paused(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let manifest = manifest(&session, target.clone());
        let identity = handoff
            .record_objects_verified(&session, &owner, manifest, expectation(&session))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            handoff
                .manifest
                .as_ref()
                .map(|manifest| admit_checkpoint(manifest, &expectation(&session))),
            Some(Ok(identity))
        );
        let target_owner = OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: target.host.clone(),
            generation: target.generation,
        };
        let checkpoint = handoff
            .manifest_identity
            .clone()
            .unwrap_or_else(|| unreachable!());
        let receipt = target_receipt(
            &session,
            checkpoint.clone(),
            cutover(&session, checkpoint.clone()),
        );
        let mut unfenced = receipt.clone();
        unfenced.cutover.source_volume_fenced = None;
        assert!(
            handoff
                .record_target_admitted(&session, &owner, &target_owner, &unfenced)
                .is_err()
        );
        assert_eq!(handoff.phase, HandoffPhase::ObjectsVerified);
        for mutate in 0..6 {
            let mut forged = receipt.clone();
            match mutate {
                0 => forged.operation = content('8'),
                1 => forged.checkpoint = content('8'),
                2 => forged.target.generation += 1,
                3 => forged.target_session = content('8'),
                4 => forged.target_fence += 1,
                _ => forged.cutover.destination_durable_ack = None,
            }
            let before = handoff.clone();
            assert!(
                handoff
                    .record_target_admitted(&session, &owner, &target_owner, &forged)
                    .is_err()
            );
            assert_eq!(handoff, before);
        }
        handoff
            .record_target_admitted(&session, &owner, &target_owner, &receipt)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(handoff.phase, HandoffPhase::TargetAdmitted);
        assert_eq!(handoff.target_receipt.as_ref(), Some(&receipt));
        handoff
            .record_resumed(&session, &owner, &target_owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(handoff.record_source_retired(&session, &owner).is_err());
        session
            .begin_retire(&owner)
            .unwrap_or_else(|_| unreachable!());
        assert!(handoff.record_source_retired(&session, &owner).is_err());
        session
            .observe_retired(&owner, true)
            .unwrap_or_else(|_| unreachable!());
        assert!(pending_intent.record_quiesced(&session, &owner).is_err());
        assert_eq!(pending_intent.phase, HandoffPhase::Intent);
        handoff
            .record_source_retired(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(handoff.phase, HandoffPhase::SourceRetired);
    }

    // r[verify kiln.hosted.handoff.order] r[verify kiln.hosted.handoff.order.scenario.order]
    #[test]
    fn handoff_refuses_out_of_order_and_stale_target_commands() {
        let mut session = session();
        let owner = paused(&mut session);
        let target = target();
        let mut handoff = PausedHandoff::begin(&session, &owner, target.clone(), content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(
            handoff
                .record_objects_verified(
                    &session,
                    &owner,
                    manifest(&session, target.clone()),
                    expectation(&session)
                )
                .is_err()
        );
        handoff
            .record_quiesced(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(
            handoff
                .record_objects_verified(
                    &session,
                    &owner,
                    manifest(&session, target.clone()),
                    expectation(&session)
                )
                .is_err()
        );
        handoff
            .record_source_paused(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff
            .record_objects_verified(
                &session,
                &owner,
                manifest(&session, target.clone()),
                expectation(&session),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let checkpoint = handoff
            .manifest_identity
            .clone()
            .unwrap_or_else(|| unreachable!());
        let exact = target_receipt(&session, checkpoint.clone(), cutover(&session, checkpoint));
        let target_owner = OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: target.host.clone(),
            generation: target.generation,
        };
        // Each scope fact is resolved; a valid native reference alone is insufficient.
        for change in 0..9 {
            let mut receipt = exact.clone();
            match change {
                0 => receipt.operation = content('8'),
                1 => receipt.checkpoint = content('9'),
                2 => receipt.source.generation += 1,
                3 => receipt.target.generation += 1,
                4 => receipt.target_session = content('a'),
                5 => receipt.target_fence += 1,
                6 => receipt.cutover.checkpoint = None,
                7 => receipt.cutover.source_volume_fenced = None,
                _ => receipt.cutover.destination_durable_ack = None,
            }
            let before = handoff.clone();
            assert!(
                handoff
                    .record_target_admitted(&session, &owner, &target_owner, &receipt)
                    .is_err()
            );
            assert_eq!(handoff, before);
        }
        handoff
            .record_target_admitted(&session, &owner, &target_owner, &exact)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(handoff.phase, HandoffPhase::TargetAdmitted);
    }

    // r[verify kiln.hosted.handoff.order.scenario.order]
    #[test]
    fn originating_identity_is_enforced_through_reopened_phases() {
        let mut session = session();
        let owner_a = paused(&mut session);
        let target = target();
        let mut handoff = PausedHandoff::begin(&session, &owner_a, target.clone(), content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut other = session.clone();
        other.identity.session = content('a');
        let other_owner = owner(&other);
        for change in 0..8 {
            let mut substituted = session.clone();
            match change {
                0 => substituted.identity.work = content('b'),
                1 => substituted.identity.work_revision += 1,
                2 => substituted.identity.attempt = content('c'),
                3 => substituted.identity.fence += 1,
                4 => substituted.identity.execution = content('d'),
                5 => substituted.identity.vm = content('e'),
                6 => substituted.placement.host = "host-other".to_string(),
                _ => substituted.placement.generation += 1,
            }
            let substituted_owner = owner(&substituted);
            let before = handoff.clone();
            assert!(
                handoff
                    .record_quiesced(&substituted, &substituted_owner)
                    .is_err()
            );
            assert_eq!(handoff, before);
        }

        let before = handoff.clone();
        assert!(handoff.record_quiesced(&other, &other_owner).is_err());
        assert_eq!(handoff, before);
        handoff
            .record_quiesced(&session, &owner_a)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff = serde_json::from_slice(&serde_json::to_vec(&handoff).unwrap_or_default())
            .unwrap_or_else(|error| unreachable!("{error:?}"));

        let before = handoff.clone();
        assert!(handoff.record_source_paused(&other, &other_owner).is_err());
        assert_eq!(handoff, before);
        handoff
            .record_source_paused(&session, &owner_a)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff = serde_json::from_slice(&serde_json::to_vec(&handoff).unwrap_or_default())
            .unwrap_or_else(|error| unreachable!("{error:?}"));

        let before = handoff.clone();
        assert!(
            handoff
                .record_objects_verified(
                    &other,
                    &other_owner,
                    manifest(&other, target.clone()),
                    expectation(&other)
                )
                .is_err()
        );
        assert_eq!(handoff, before);
        handoff
            .record_objects_verified(
                &session,
                &owner_a,
                manifest(&session, target.clone()),
                expectation(&session),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff = serde_json::from_slice(&serde_json::to_vec(&handoff).unwrap_or_default())
            .unwrap_or_else(|error| unreachable!("{error:?}"));

        let identity = handoff
            .manifest_identity
            .clone()
            .unwrap_or_else(|| unreachable!());
        let target_owner = OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: target.host.clone(),
            generation: target.generation,
        };
        let receipt = target_receipt(&session, identity.clone(), cutover(&session, identity));
        let before = handoff.clone();
        assert!(
            handoff
                .record_target_admitted(&other, &other_owner, &target_owner, &receipt)
                .is_err()
        );
        assert_eq!(handoff, before);
        handoff
            .record_target_admitted(&session, &owner_a, &target_owner, &receipt)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff = serde_json::from_slice(&serde_json::to_vec(&handoff).unwrap_or_default())
            .unwrap_or_else(|error| unreachable!("{error:?}"));

        let before = handoff.clone();
        assert!(
            handoff
                .record_resumed(&other, &other_owner, &target_owner)
                .is_err()
        );
        assert_eq!(handoff, before);
        handoff
            .record_resumed(&session, &owner_a, &target_owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff = serde_json::from_slice(&serde_json::to_vec(&handoff).unwrap_or_default())
            .unwrap_or_else(|error| unreachable!("{error:?}"));

        let mut retired_other = other.clone();
        retired_other.state = RetainedState::Retired;
        let retired_owner = owner(&retired_other);
        let before = handoff.clone();
        assert!(
            handoff
                .record_source_retired(&retired_other, &retired_owner)
                .is_err()
        );
        assert_eq!(handoff, before);
        session
            .begin_retire(&owner_a)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        session
            .observe_retired(&owner_a, true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff
            .record_source_retired(&session, &owner_a)
            .unwrap_or_else(|error| unreachable!("{error:?}"));

        let mut reconcile = handoff.clone();
        let before = reconcile.clone();
        assert!(
            reconcile
                .require_reconciliation(&retired_other, &retired_owner, "uncertain")
                .is_err()
        );
        assert_eq!(reconcile, before);
    }

    // r[verify kiln.hosted.handoff.manifest.scenario.disk]
    #[test]
    fn independent_checkpoint_expectation_mismatches_refuse_without_advancement() {
        let mut session = session();
        let owner = paused(&mut session);
        let target = target();
        let manifest = manifest(&session, target.clone());
        for change in 0..10 {
            let mut expected = expectation(&session);
            match change {
                0 => expected.session = content('a'),
                1 => expected.vm = content('b'),
                2 => expected.work = content('c'),
                3 => expected.work_revision += 1,
                4 => expected.attempt = content('d'),
                5 => expected.fence += 1,
                6 => expected.execution = expected.session.clone(),
                7 => expected.source.generation += 1,
                8 => expected.target.generation += 1,
                _ => expected.operation = content('8'),
            }
            assert!(admit_checkpoint(&manifest, &expected).is_err());
            let mut handoff = PausedHandoff::begin(&session, &owner, target.clone(), content('4'))
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            handoff
                .record_quiesced(&session, &owner)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            handoff
                .record_source_paused(&session, &owner)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            let before = handoff.clone();
            assert!(
                handoff
                    .record_objects_verified(&session, &owner, manifest.clone(), expected)
                    .is_err()
            );
            assert_eq!(handoff, before);
        }
    }

    // r[verify kiln.hosted.handoff.manifest.scenario.disk]
    #[test]
    fn old_memory_with_a_new_disk_generation_is_refused() {
        let mut session = session();
        let _owner = paused(&mut session);
        let mut manifest = manifest(&session, target());
        manifest.disk_generation += 1;
        assert!(admit_checkpoint(&manifest, &expectation(&session)).is_err());
        manifest.disk_generation = 11;
        let profile = manifest.profile.clone();
        manifest.profile.memory_backing = content('1');
        assert!(admit_checkpoint(&manifest, &expectation(&session)).is_err());
        manifest.profile = profile;
        let execution = manifest.execution.clone();
        manifest.execution = manifest.session.clone();
        assert!(admit_checkpoint(&manifest, &expectation(&session)).is_err());
        manifest.execution = execution;
        assert!(admit_checkpoint(&manifest, &expectation(&session)).is_ok());
        manifest.schema = "kiln.checkpoint-manifest.v1".to_string();
        assert!(admit_checkpoint(&manifest, &expectation(&session)).is_err());
        manifest.schema = CHECKPOINT_SCHEMA.to_string();
        manifest.objects.workspace = manifest.objects.ram.clone();
        assert!(admit_checkpoint(&manifest, &expectation(&session)).is_err());
    }

    // r[verify kiln.hosted.handoff.manifest.scenario.reconcile]
    #[test]
    fn uncertain_handoff_requires_reconciliation_without_claiming_retirement() {
        let mut session = session();
        let owner = paused(&mut session);
        let mut handoff = PausedHandoff::begin(&session, &owner, target(), content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff
            .record_quiesced(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff
            .record_source_paused(&session, &owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        handoff
            .require_reconciliation(&session, &owner, "lost final acknowledgement")
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(handoff.phase, HandoffPhase::ReconcileRequired);
        assert!(handoff.record_source_retired(&session, &owner).is_err());
        assert!(session.occupies_capacity());
    }
}

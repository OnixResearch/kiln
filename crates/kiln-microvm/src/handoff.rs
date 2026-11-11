//! Pure paused-handoff manifest admission and ordering.
//!
//! A handoff binds one retained session, execution, source/target placement,
//! exact disk generation, container profile and checkpoint objects. This module
//! performs no copy, transfer, VMM operation or ownership commit; it refuses
//! ordering and identity mistakes before a shell requests those effects.

use serde::{Deserialize, Serialize};

use crate::retained::{OwnerToken, Placement, RetainedSession, RetainedState};
use kiln_core::types::{ContentId, Diagnostic, ExternalRef};

pub const CHECKPOINT_SCHEMA: &str = "kiln.checkpoint-manifest.v1";
pub const HANDOFF_SCHEMA: &str = "kiln.paused-handoff.v1";
pub const MAX_CAUSE_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointProfile {
    pub image: ContentId,
    pub kernel: ContentId,
    pub hypervisor: ContentId,
    pub cpu: ContentId,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointExpectation {
    pub session: ContentId,
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
pub struct PausedHandoff {
    pub schema: String,
    pub operation: ContentId,
    pub source: Placement,
    pub target: Placement,
    pub phase: HandoffPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<CheckpointManifest>,
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
            source: session.placement.clone(),
            target,
            phase: HandoffPhase::Intent,
            manifest: None,
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
        mut expected: CheckpointExpectation,
    ) -> Result<ContentId, Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::SourcePaused])?;
        if session.state != RetainedState::Paused {
            return Err(refusal(
                "handoff_source_running",
                "handoff.source",
                "source must remain paused while checkpoint objects are verified",
            ));
        }
        expected.source = self.source.clone();
        expected.target = self.target.clone();
        expected.operation = self.operation.clone();
        expected.session = session.identity.session.clone();
        expected.work = session.identity.work.clone();
        expected.work_revision = session.identity.work_revision;
        expected.attempt = session.identity.attempt.clone();
        expected.fence = session.identity.fence;
        expected.execution = session.identity.session.clone();
        let identity = admit_checkpoint(&manifest, &expected)?;
        self.manifest = Some(manifest);
        self.phase = HandoffPhase::ObjectsVerified;
        Ok(identity)
    }

    /// Record that the destination admitted the exact manifest and placement.
    pub fn record_target_admitted(
        &mut self,
        session: &RetainedSession,
        owner: &OwnerToken,
        target_owner: &OwnerToken,
        receipt: &ExternalRef,
    ) -> Result<(), Diagnostic> {
        self.authorize(session, owner, &[HandoffPhase::ObjectsVerified])?;
        receipt.validate()?;
        let manifest = self.manifest.as_ref().ok_or_else(|| {
            refusal(
                "handoff_manifest_missing",
                "handoff.manifest",
                "target admission requires a verified checkpoint manifest",
            )
        })?;
        target_owner.session.validate()?;
        if target_owner.session != session.identity.session
            || target_owner.fence != session.identity.fence
            || target_owner.host != manifest.target.host
            || target_owner.generation != manifest.target.generation
            || self.target != manifest.target
        {
            return Err(refusal(
                "handoff_target_owner",
                "handoff.target",
                "target owner does not hold the manifest target placement and fence",
            ));
        }
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
        if target_owner.session != session.identity.session
            || target_owner.fence != session.identity.fence
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
        self.authorize(session, owner, &[HandoffPhase::Resumed])?;
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
        session.authorize(owner)?;
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

    fn authorize(
        &self,
        session: &RetainedSession,
        owner: &OwnerToken,
        allowed: &[HandoffPhase],
    ) -> Result<(), Diagnostic> {
        if self.schema != HANDOFF_SCHEMA {
            return Err(refusal(
                "handoff_schema",
                "handoff.schema",
                "handoff schema is unsupported",
            ));
        }
        session.authorize(owner)?;
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
        &manifest.work,
        &manifest.attempt,
        &manifest.execution,
        &manifest.operation,
        &manifest.profile.image,
        &manifest.profile.kernel,
        &manifest.profile.hypervisor,
        &manifest.profile.cpu,
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
    use crate::retained::{RetainedResources, SessionIdentity};

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn session() -> RetainedSession {
        RetainedSession::admit(
            SessionIdentity {
                session: content('1'),
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
            work: session.identity.work.clone(),
            work_revision: session.identity.work_revision,
            attempt: session.identity.attempt.clone(),
            fence: session.identity.fence,
            execution: session.identity.session.clone(),
            source: session.placement.clone(),
            target,
            operation: content('4'),
            profile: CheckpointProfile {
                image: content('5'),
                kernel: content('6'),
                hypervisor: content('7'),
                cpu: content('8'),
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
            work: session.identity.work.clone(),
            work_revision: session.identity.work_revision,
            attempt: session.identity.attempt.clone(),
            fence: session.identity.fence,
            execution: session.identity.session.clone(),
            source: session.placement.clone(),
            target: target(),
            operation: content('4'),
            disk: content('c'),
            disk_generation: 11,
        }
    }

    // r[verify kiln.hosted.handoff.manifest] r[verify kiln.hosted.handoff.order] r[verify kiln.hosted.handoff.order.scenario.complete]
    #[test]
    fn paused_handoff_orders_quiesce_capture_target_and_retirement() {
        let mut session = session();
        let owner = paused(&mut session);
        let target = target();
        let mut handoff = PausedHandoff::begin(&session, &owner, target.clone(), content('4'))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
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
        let receipt =
            ExternalRef::new("lattice", "candidate", "target").unwrap_or_else(|_| unreachable!());
        handoff
            .record_target_admitted(&session, &owner, &target_owner, &receipt)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(handoff.phase, HandoffPhase::TargetAdmitted);
        handoff
            .record_resumed(&session, &owner, &target_owner)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
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
        let stale = OwnerToken {
            session: session.identity.session.clone(),
            fence: session.identity.fence,
            host: target.host.clone(),
            generation: target.generation + 1,
        };
        let receipt =
            ExternalRef::new("lattice", "candidate", "target").unwrap_or_else(|_| unreachable!());
        assert!(
            handoff
                .record_target_admitted(&session, &owner, &stale, &receipt)
                .is_err()
        );
        assert_eq!(handoff.phase, HandoffPhase::ObjectsVerified);
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

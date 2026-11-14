//! Evidence-based drain projection and post-dispatch safety. No scheduler,
//! placement allocator, or live-migration qualification is created here.
use super::{Placement, RetainedSession, RetainedState};
use kiln_core::types::{ContentId, Diagnostic};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmObservation {
    Running,
    Paused,
    Stopped,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchObservation {
    pub sender_exit: Option<i32>,
    pub source: VmObservation,
    pub destination: VmObservation,
    pub destination_ever_wrote: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AfterDispatch {
    /// No automatic stop, resume, cleanup or byte-stream replay. This includes
    /// ordinary nonzero exits and a currently stopped target that wrote earlier.
    ReconcileBoth,
    /// Only the exact durable target owner and enforced source exclusion permit
    /// continuation. Source retirement is still separately observed.
    TargetOwned,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CutoverEvidence {
    pub operation: ContentId,
    pub session: ContentId,
    pub execution: ContentId,
    pub vm: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub source: Placement,
    pub target: Placement,
    pub durable_target_commit: Option<ContentId>,
    pub source_volume_fenced: Option<ContentId>,
    pub source_effects_fenced: Option<ContentId>,
    pub destination_durable_ack: Option<ContentId>,
}

/// Sender status (including zero), source Running and receiver currently stopped
/// are observations, never authorization to kill the receiver or resume source.
/// Independently resolved durable receipts must bind this exact handoff scope.
// r[impl kiln.hosted.mobility.runtime.scenario.replay]
pub fn after_dispatch(
    session: &RetainedSession,
    operation: &ContentId,
    target: &Placement,
    evidence: Option<&CutoverEvidence>,
    _observed: &DispatchObservation,
) -> AfterDispatch {
    let Some(evidence) = evidence else {
        return AfterDispatch::ReconcileBoth;
    };
    if evidence.operation != *operation
        || evidence.session != session.identity.session
        || evidence.execution != session.identity.execution
        || evidence.vm != session.identity.vm
        || evidence.attempt != session.identity.attempt
        || evidence.fence != session.identity.fence
        || evidence.source != session.placement
        || evidence.target != *target
        || evidence.target.host == evidence.source.host
        || evidence.target.generation <= evidence.source.generation
    {
        return AfterDispatch::ReconcileBoth;
    }
    if [
        &evidence.durable_target_commit,
        &evidence.source_volume_fenced,
        &evidence.source_effects_fenced,
        &evidence.destination_durable_ack,
    ]
    .into_iter()
    .any(|receipt| {
        receipt
            .as_ref()
            .is_none_or(|receipt| receipt.validate().is_err())
    }) {
        return AfterDispatch::ReconcileBoth;
    }
    AfterDispatch::TargetOwned
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrainDisposition {
    Completed,
    Migrated,
    Checkpointed,
    Finishing,
    Pinned,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Continuation {
    TerminalObserved,
    TargetDurablyAcknowledged,
    CheckpointDurablyAcknowledged,
    None,
}

/// Per-host projection from owned session observations. `Retired` is obtained
/// only through observe_retired; an empty placement map is not a drain receipt.
pub fn drain_disposition(
    session: &RetainedSession,
    continuation: Continuation,
    remaining_ms: Option<u64>,
    finish_grace_ms: u64,
) -> DrainDisposition {
    if session.state == RetainedState::Retired {
        return match continuation {
            Continuation::TerminalObserved => DrainDisposition::Completed,
            Continuation::TargetDurablyAcknowledged => DrainDisposition::Migrated,
            Continuation::CheckpointDurablyAcknowledged => DrainDisposition::Checkpointed,
            Continuation::None => DrainDisposition::Unknown,
        };
    }
    match session.state {
        RetainedState::ReconcileRequired
        | RetainedState::Migrating
        | RetainedState::MigratedAway
        | RetainedState::Retiring
        | RetainedState::Receiving
        | RetainedState::Restoring => DrainDisposition::Unknown,
        RetainedState::Running if remaining_ms.is_some_and(|left| left <= finish_grace_ms) => {
            DrainDisposition::Finishing
        }
        // The selected provider has no qualified running cutover capability.
        // Do not report an unqualified long session as migrated or checkpointed.
        _ => DrainDisposition::Pinned,
    }
}

/// Cost/cooldown guard, not move authority. A true result still requires exact
/// compatibility, target capacity and provider-owned resource fencing.
pub fn move_economical(
    now_ms: u64,
    last_move_ms: Option<u64>,
    cooldown_ms: u64,
    estimated_move_ms: u64,
    remaining_ms: Option<u64>,
) -> bool {
    if estimated_move_ms == 0 || remaining_ms.is_none_or(|left| estimated_move_ms >= left) {
        return false;
    }
    last_move_ms.is_none_or(|last| {
        now_ms
            .checked_sub(last)
            .is_some_and(|elapsed| elapsed >= cooldown_ms)
    })
}

/// Current selected fabric cannot enable live migration. A runtime version
/// string, matching ISA, stream receipt, or old happy-path receipt cannot turn
/// this into authority. The actual provider needs a separately versioned API.
pub fn require_live_capability() -> Result<(), Diagnostic> {
    Err(Diagnostic::new(
        "retained_live_unqualified",
        "retained.mobility",
        "selected Aspen profile lacks qualified cross-host streaming, current placement/resource fencing and final-ACK-safe live cutover",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained::{RetainedResources, SessionIdentity};
    fn id(c: char) -> ContentId {
        ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }
    fn session() -> RetainedSession {
        RetainedSession::admit(
            SessionIdentity {
                session: id('a'),
                execution: id('e'),
                vm: id('f'),
                work: id('b'),
                work_revision: 1,
                attempt: id('c'),
                fence: 2,
            },
            Placement {
                host: "a".into(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 1,
                memory_mib: 512,
                slot: 1,
            },
        )
        .unwrap_or_else(|_| unreachable!())
    }
    // r[verify kiln.hosted.mobility.runtime.scenario.durable]
    #[test]
    fn ordinary_sender_error_and_previously_writing_stopped_target_never_authorize_cleanup() {
        let session = session();
        let target = Placement {
            host: "b".into(),
            generation: 2,
        };
        for exit in [None, Some(0), Some(1), Some(124), Some(255)] {
            for destination in [
                VmObservation::Running,
                VmObservation::Stopped,
                VmObservation::Unknown,
            ] {
                let observed = DispatchObservation {
                    sender_exit: exit,
                    source: VmObservation::Running,
                    destination,
                    destination_ever_wrote: true,
                };
                assert_eq!(
                    after_dispatch(&session, &id('d'), &target, None, &observed),
                    AfterDispatch::ReconcileBoth
                );
            }
        }
        let mut evidence = CutoverEvidence {
            operation: id('d'),
            session: id('a'),
            execution: id('e'),
            vm: id('f'),
            attempt: id('c'),
            fence: 2,
            source: session.placement.clone(),
            target: target.clone(),
            durable_target_commit: Some(id('e')),
            source_volume_fenced: Some(id('f')),
            source_effects_fenced: Some(id('1')),
            destination_durable_ack: Some(id('2')),
        };
        let observed = DispatchObservation {
            sender_exit: Some(1),
            source: VmObservation::Unknown,
            destination: VmObservation::Running,
            destination_ever_wrote: true,
        };
        assert_eq!(
            after_dispatch(&session, &id('d'), &target, Some(&evidence), &observed),
            AfterDispatch::TargetOwned
        );
        evidence.fence += 1;
        assert_eq!(
            after_dispatch(&session, &id('d'), &target, Some(&evidence), &observed),
            AfterDispatch::ReconcileBoth
        );
    }
    #[test]
    fn unknown_cleanup_blocks_drain_and_cooldown_blocks_churn() {
        let mut session = session();
        session.state = RetainedState::MigratedAway;
        assert_eq!(
            drain_disposition(
                &session,
                Continuation::TargetDurablyAcknowledged,
                None,
                1000
            ),
            DrainDisposition::Unknown
        );
        session.state = RetainedState::Retired;
        assert_eq!(
            drain_disposition(&session, Continuation::None, None, 1000),
            DrainDisposition::Unknown
        );
        assert_eq!(
            drain_disposition(
                &session,
                Continuation::TargetDurablyAcknowledged,
                None,
                1000
            ),
            DrainDisposition::Migrated
        );
        assert!(!move_economical(100, Some(90), 20, 10, Some(100)));
        assert!(move_economical(110, Some(90), 20, 10, Some(100)));
        assert!(!move_economical(80, Some(90), 20, 10, Some(100)));
        assert!(!move_economical(110, None, 20, 100, Some(100)));
        assert!(require_live_capability().is_err());
    }
}

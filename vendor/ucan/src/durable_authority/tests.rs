use std::cell::Cell;
use std::cell::RefCell;

use replay_ledger_core::CommitObservation as ReplayCommitObservation;
use replay_ledger_core::CompareAndSwapCommand as ReplayCommand;
use replay_ledger_core::DurabilityObservation as ReplayDurabilityObservation;
use replay_ledger_core::RecordVersion;
use replay_ledger_core::ReplayLedgerPort;
use replay_ledger_core::ReplayRecord;
use replay_ledger_core::ReplaySlotIdentity;
use replay_ledger_core::SlotObservation;
use revocation_view_core::AuthenticityObservation;
use revocation_view_core::CommitObservation as RevocationCommitObservation;
use revocation_view_core::CompareAndSwapCommand as RevocationCommand;
use revocation_view_core::CompletenessObservation;
use revocation_view_core::DurabilityObservation as RevocationDurabilityObservation;
use revocation_view_core::EntryLimit;
use revocation_view_core::RevocationEntryDigest;
use revocation_view_core::RevocationSnapshot;
use revocation_view_core::RevocationViewPort;
use revocation_view_core::SnapshotInput;
use revocation_view_core::SnapshotObservation;
use revocation_view_core::SnapshotVersion;
use revocation_view_core::SourceDigest;
use revocation_view_core::SourceGeneration;
use revocation_view_core::StoredSnapshot;
use revocation_view_core::UnixTime;
use revocation_view_core::UpdateDecision;
use revocation_view_core::plan_update;
use transactional_reconciliation_core as reconciliation;

use super::DurableAuthorityConfigError;
use super::DurableAuthorityField;
use super::DurableHolderReplayAdmission;
use super::DurableReplayContext;
use super::DurableReplayContextInput;
use super::DurableReplayDurability;
use super::DurableRevocationChecker;
use super::DurableRevocationContext;
use super::DurableRevocationContextInput;
use super::durable_revocation_entry_digest;
use crate::AudienceDid;
use crate::HolderInvocationReplayError;
use crate::ProofReference;
use crate::RevocationChecker;
use crate::RevocationError;

const TENANT_BYTE: u8 = 2;
const OPERATION_BYTE: u8 = 3;
const NAMESPACE_BYTE: u8 = 4;
const ATTEMPT_BYTE: u8 = 5;
const NONCE_BYTE: u8 = 6;
const REQUEST_BYTE: u8 = 7;
const OTHER_REQUEST_BYTE: u8 = 8;
const SOURCE_BYTE: u8 = 9;
const REFERENCE_BYTE: u8 = 10;
const OTHER_REFERENCE_BYTE: u8 = 11;
const PILOT_SUCCESSOR_BYTE: u8 = 12;
const CURRENT_TIME_SECONDS: u64 = 100;
const EFFECTIVE_TIME_SECONDS: u64 = 90;
const EXPIRY_TIME_SECONDS: u64 = 110;
const EXPIRED_TIME_SECONDS: u64 = 120;
const ENTRY_LIMIT: u32 = 8;
const AUDIENCE: &str = "did:key:z6MkDurableAuthorityPilot";

struct PilotBlake3;

impl reconciliation::Blake3IdentityDeriver for PilotBlake3 {
    fn derive_identity(&self, framed_bytes: &[u8]) -> Result<reconciliation::Identity, reconciliation::CoreError> {
        reconciliation::Identity::new(*blake3::hash(framed_bytes).as_bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryError {
    Injected,
}

struct MemoryReplayPort {
    record: RefCell<Option<ReplayRecord>>,
    durability: ReplayDurabilityObservation,
    fail_load: Cell<bool>,
    forced_commit: Cell<Option<ReplayCommitObservation>>,
    writes: Cell<usize>,
}

impl MemoryReplayPort {
    fn new(durability: ReplayDurabilityObservation) -> Self {
        Self {
            record: RefCell::new(None),
            durability,
            fail_load: Cell::new(false),
            forced_commit: Cell::new(None),
            writes: Cell::new(0),
        }
    }
}

impl ReplayLedgerPort for MemoryReplayPort {
    type Error = MemoryError;

    fn load(&self, slot: ReplaySlotIdentity) -> Result<SlotObservation, Self::Error> {
        if self.fail_load.get() {
            return Err(MemoryError::Injected);
        }
        Ok(match *self.record.borrow() {
            Some(record) if record.slot() == slot => SlotObservation::Present(record),
            Some(_) | None => SlotObservation::Empty,
        })
    }

    fn compare_and_swap(&self, command: ReplayCommand) -> Result<ReplayCommitObservation, Self::Error> {
        if let Some(observation) = self.forced_commit.take() {
            return Ok(observation);
        }
        let current = *self.record.borrow();
        let current_version = current.map(|record| record.version());
        if current_version != command.expected_version() {
            return Ok(ReplayCommitObservation::Stale {
                observed_version: current_version,
            });
        }
        let replacement = command.replacement();
        *self.record.borrow_mut() = Some(replacement);
        self.writes.set(self.writes.get().saturating_add(1));
        Ok(ReplayCommitObservation::Applied {
            version: replacement.version(),
            durability: self.durability,
        })
    }
}

struct MemoryRevocationPort {
    snapshot: RefCell<Option<StoredSnapshot>>,
    fail_load: Cell<bool>,
}

impl MemoryRevocationPort {
    fn new() -> Self {
        Self {
            snapshot: RefCell::new(None),
            fail_load: Cell::new(false),
        }
    }

    fn install(&self, snapshot: RevocationSnapshot) {
        let UpdateDecision::Install(command) = plan_update(snapshot, SnapshotObservation::Empty) else {
            panic!("empty revocation view must produce an install command");
        };
        let observation = self.compare_and_swap(command).expect("in-memory snapshot installation must succeed");
        assert!(matches!(observation, RevocationCommitObservation::Applied {
            durability: RevocationDurabilityObservation::Synchronized,
            ..
        }));
    }
}

impl RevocationViewPort for MemoryRevocationPort {
    type Error = MemoryError;

    fn load(&self, source: SourceDigest) -> Result<SnapshotObservation, Self::Error> {
        if self.fail_load.get() {
            return Err(MemoryError::Injected);
        }
        Ok(match self.snapshot.borrow().as_ref() {
            Some(stored) if stored.snapshot().source() == source => SnapshotObservation::Present(stored.clone()),
            Some(_) | None => SnapshotObservation::Empty,
        })
    }

    fn compare_and_swap(&self, command: RevocationCommand) -> Result<RevocationCommitObservation, Self::Error> {
        let current_version = self.snapshot.borrow().as_ref().map(StoredSnapshot::version);
        if current_version != command.expected_version() {
            return Ok(RevocationCommitObservation::Stale {
                observed_version: current_version,
            });
        }
        let replacement = command.replacement().clone();
        let version = replacement.version();
        *self.snapshot.borrow_mut() = Some(replacement);
        Ok(RevocationCommitObservation::Applied {
            version,
            durability: RevocationDurabilityObservation::Synchronized,
        })
    }
}

fn replay_context(durability: DurableReplayDurability) -> DurableReplayContext {
    DurableReplayContext::new(DurableReplayContextInput {
        tenant: [TENANT_BYTE; replay_ledger_core::DIGEST_BYTES],
        operation_profile: [OPERATION_BYTE; replay_ledger_core::DIGEST_BYTES],
        authority_generation: 1,
        ledger_namespace: [NAMESPACE_BYTE; replay_ledger_core::DIGEST_BYTES],
        attempt: [ATTEMPT_BYTE; replay_ledger_core::DIGEST_BYTES],
        durability,
    })
    .expect("complete replay context must be admitted")
}

fn audience() -> AudienceDid {
    AudienceDid::new(AUDIENCE).expect("test audience must be valid")
}

fn reconciliation_identity(byte: u8) -> Result<reconciliation::Identity, reconciliation::CoreError> {
    reconciliation::Identity::new([byte; blake3::OUT_LEN])
}

fn reconciliation_pilot()
-> Result<(reconciliation::PersistenceBinding, reconciliation::AttemptReservation), reconciliation::CoreError> {
    let revision = reconciliation::Revision::observed(1);
    let desired = reconciliation_identity(TENANT_BYTE)?;
    let policy = reconciliation_identity(OPERATION_BYTE)?;
    let plan = reconciliation::build_plan(
        &PilotBlake3,
        reconciliation::Limits::new(reconciliation::Bound::new(1)?, reconciliation::Bound::new(1)?),
        reconciliation::PlanningInput::new(revision, desired, policy, Vec::new(), vec![
            reconciliation::OperationDraft::new(
                reconciliation_identity(NAMESPACE_BYTE)?,
                reconciliation_identity(REQUEST_BYTE)?,
                reconciliation::Generation::observed(1),
                reconciliation_identity(SOURCE_BYTE)?,
            ),
        ]),
    )?;
    let current = reconciliation::CurrentFacts::new(revision, desired, policy, Vec::new());
    let publication =
        reconciliation::plan_publication(&plan, &current, reconciliation_identity(PILOT_SUCCESSOR_BYTE)?)?;
    let reservation = reconciliation::reserve_attempt(
        &plan,
        plan.operations()[0].idempotency_identity(),
        reconciliation_identity(ATTEMPT_BYTE)?,
    )?;
    let binding = reconciliation::PersistenceBinding::new(
        publication,
        reservation,
        reconciliation_identity(OTHER_REQUEST_BYTE)?,
    )?;
    Ok((binding, reservation))
}

fn reference(byte: u8) -> ProofReference {
    ProofReference::new(vec![byte; revocation_view_core::DIGEST_BYTES])
}

fn revocation_context(current_time_seconds: u64) -> DurableRevocationContext {
    DurableRevocationContext::new(DurableRevocationContextInput {
        source: [SOURCE_BYTE; revocation_view_core::DIGEST_BYTES],
        minimum_generation: 1,
        current_time_seconds,
    })
    .expect("complete revocation context must be admitted")
}

fn snapshot(completeness: CompletenessObservation, authenticity: AuthenticityObservation) -> RevocationSnapshot {
    let revoked = reference(REFERENCE_BYTE);
    RevocationSnapshot::admit(SnapshotInput {
        source: Some(
            SourceDigest::new([SOURCE_BYTE; revocation_view_core::DIGEST_BYTES]).expect("source digest must be valid"),
        ),
        generation: Some(SourceGeneration::new(1).expect("generation must be valid")),
        observed_at: UnixTime::from_seconds(CURRENT_TIME_SECONDS),
        effective_from: UnixTime::from_seconds(EFFECTIVE_TIME_SECONDS),
        expires_at: UnixTime::from_seconds(EXPIRY_TIME_SECONDS),
        completeness,
        authenticity,
        entries: vec![
            RevocationEntryDigest::new(durable_revocation_entry_digest(&revoked)).expect("entry digest must be valid"),
        ],
        entry_limit: EntryLimit::new(ENTRY_LIMIT).expect("entry limit must be valid"),
    })
    .expect("snapshot fixture must be admitted")
}

#[test]
fn replay_context_rejects_missing_roles() {
    let error = DurableReplayContext::new(DurableReplayContextInput {
        tenant: [0; replay_ledger_core::DIGEST_BYTES],
        operation_profile: [OPERATION_BYTE; replay_ledger_core::DIGEST_BYTES],
        authority_generation: 1,
        ledger_namespace: [NAMESPACE_BYTE; replay_ledger_core::DIGEST_BYTES],
        attempt: [ATTEMPT_BYTE; replay_ledger_core::DIGEST_BYTES],
        durability: DurableReplayDurability::CommitObserved,
    })
    .expect_err("missing tenant must fail");
    assert_eq!(error, DurableAuthorityConfigError::MissingDigest {
        field: DurableAuthorityField::Tenant,
    });
}

#[test]
fn first_replay_reserves_once_and_duplicate_fails_closed() {
    let port = MemoryReplayPort::new(ReplayDurabilityObservation::Synchronized);
    let adapter = DurableHolderReplayAdmission::new(&port, replay_context(DurableReplayDurability::Synchronized));
    let nonce = [NONCE_BYTE; replay_ledger_core::DIGEST_BYTES];
    let request = [REQUEST_BYTE; replay_ledger_core::DIGEST_BYTES];

    assert_eq!(adapter.admit_parts(&audience(), &nonce, &request), Ok(()));
    assert_eq!(adapter.admit_parts(&audience(), &nonce, &request), Err(HolderInvocationReplayError::Duplicate));
    assert_eq!(port.writes.get(), 1);
}

#[test]
fn conflicting_request_for_one_nonce_is_denied() {
    let port = MemoryReplayPort::new(ReplayDurabilityObservation::Synchronized);
    let adapter = DurableHolderReplayAdmission::new(&port, replay_context(DurableReplayDurability::CommitObserved));
    let nonce = [NONCE_BYTE; replay_ledger_core::DIGEST_BYTES];
    let request = [REQUEST_BYTE; replay_ledger_core::DIGEST_BYTES];
    let other_request = [OTHER_REQUEST_BYTE; replay_ledger_core::DIGEST_BYTES];

    assert_eq!(adapter.admit_parts(&audience(), &nonce, &request), Ok(()));
    assert_eq!(adapter.admit_parts(&audience(), &nonce, &other_request), Err(HolderInvocationReplayError::Denied));
}

#[test]
fn unknown_or_insufficient_commit_observation_fails_closed() {
    let unknown_port = MemoryReplayPort::new(ReplayDurabilityObservation::Synchronized);
    unknown_port.forced_commit.set(Some(ReplayCommitObservation::OutcomeUnknown));
    let unknown =
        DurableHolderReplayAdmission::new(&unknown_port, replay_context(DurableReplayDurability::CommitObserved));
    assert_eq!(
        unknown.admit_parts(
            &audience(),
            &[NONCE_BYTE; replay_ledger_core::DIGEST_BYTES],
            &[REQUEST_BYTE; replay_ledger_core::DIGEST_BYTES],
        ),
        Err(HolderInvocationReplayError::Backend)
    );

    let commit_only_port = MemoryReplayPort::new(ReplayDurabilityObservation::CommitOnly);
    let synchronized =
        DurableHolderReplayAdmission::new(&commit_only_port, replay_context(DurableReplayDurability::Synchronized));
    assert_eq!(
        synchronized.admit_parts(
            &audience(),
            &[NONCE_BYTE; replay_ledger_core::DIGEST_BYTES],
            &[REQUEST_BYTE; replay_ledger_core::DIGEST_BYTES],
        ),
        Err(HolderInvocationReplayError::Backend)
    );
}

#[test]
fn current_complete_authenticated_snapshot_answers_lookup() {
    let port = MemoryRevocationPort::new();
    port.install(snapshot(CompletenessObservation::Complete, AuthenticityObservation::Verified));
    let adapter = DurableRevocationChecker::new(&port, revocation_context(CURRENT_TIME_SECONDS));

    assert_eq!(adapter.is_revoked(&reference(REFERENCE_BYTE)), Ok(true));
    assert_eq!(adapter.is_revoked(&reference(OTHER_REFERENCE_BYTE)), Ok(false));
}

#[test]
fn stale_incomplete_and_unavailable_views_fail_closed() {
    let expired_port = MemoryRevocationPort::new();
    expired_port.install(snapshot(CompletenessObservation::Complete, AuthenticityObservation::Verified));
    let expired = DurableRevocationChecker::new(&expired_port, revocation_context(EXPIRED_TIME_SECONDS));
    assert!(matches!(expired.is_revoked(&reference(REFERENCE_BYTE)), Err(RevocationError::Stale { .. })));

    let partial_port = MemoryRevocationPort::new();
    partial_port.install(snapshot(CompletenessObservation::Partial, AuthenticityObservation::Verified));
    let partial = DurableRevocationChecker::new(&partial_port, revocation_context(CURRENT_TIME_SECONDS));
    assert!(matches!(partial.is_revoked(&reference(REFERENCE_BYTE)), Err(RevocationError::Malformed { .. })));

    let unavailable_port = MemoryRevocationPort::new();
    unavailable_port.fail_load.set(true);
    let unavailable = DurableRevocationChecker::new(&unavailable_port, revocation_context(CURRENT_TIME_SECONDS));
    assert!(matches!(unavailable.is_revoked(&reference(REFERENCE_BYTE)), Err(RevocationError::Backend { .. })));
}

#[test]
fn exact_durable_authority_publication_admits_dispatch() -> Result<(), reconciliation::CoreError> {
    let (binding, reservation) = reconciliation_pilot()?;
    let decision = reconciliation::classify_commit(binding, reconciliation::CommitObservation::Applied(binding));
    let published = reconciliation::admit_published(decision, reservation)?;

    assert_eq!(decision.state(), reconciliation::PersistenceState::Published);
    assert_eq!(published.binding().attempt_identity(), reservation.attempt_identity());
    Ok(())
}

#[test]
fn uncertain_durable_authority_publication_denies_dispatch() -> Result<(), reconciliation::CoreError> {
    let (binding, reservation) = reconciliation_pilot()?;
    let decision = reconciliation::classify_commit(binding, reconciliation::CommitObservation::OutcomeUnknown);

    assert_eq!(decision.state(), reconciliation::PersistenceState::PublicationUnknown);
    assert!(matches!(
        decision.quarantine(),
        reconciliation::QuarantineStatus::Quarantined(reconciliation::QuarantineReason::CommitOutcomeUnknown)
    ));
    assert_eq!(
        reconciliation::admit_published(decision, reservation),
        Err(reconciliation::CoreError::PersistenceQuarantined)
    );
    Ok(())
}

#[test]
fn memory_ports_report_expected_initial_versions() {
    assert_eq!(RecordVersion::initial().get(), 1);
    assert_eq!(SnapshotVersion::initial().get(), 1);
}

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_codec::identity_from_bytes;
use kiln_core::domain::{Event, State};
use kiln_core::operations::{
    JournalRecord, LeaseState, OutboxRecord, RecoveryAction, classify_recovery, replay,
};
use kiln_core::types::{
    CONTRACT_VERSION_V1, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef,
    ProviderProfileId,
};
use kiln_provider_api::{
    EventAppendRequest, EventStorePort, EventStoreSnapshot, InfrastructureFailure, PortFailure,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

pub const RUSQLITE_VERSION: &str = "0.40.2";
pub const LIBSQLITE3_SYS_VERSION: &str = "0.38.2";
pub const SQLITE_RUNTIME_VERSION: &str = "3.53.2";
pub const SQLITE_LICENSE: &str = "MIT";
pub const STORE_SCHEMA_VERSION: u16 = 1;
pub const INITIAL_SCHEMA_VERSION: u16 = 0;

const METADATA_SCHEMA_KEY: &str = "schema_version";
const INTEGRITY_OK: &str = "ok";
const LEASE_STATE_UNCLAIMED: &str = "unclaimed";
const LEASE_STATE_LEASED: &str = "leased";
const LEASE_STATE_EXPIRED: &str = "expired";
const LEASE_STATE_COMPLETED: &str = "completed";
const LEASE_STATE_AMBIGUOUS: &str = "ambiguous";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteTransactionKind {
    Immediate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteJournalMode {
    Wal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteSynchronous {
    Full,
}

// r[impl kiln.sqlite.profile]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SqliteProfile {
    pub rusqlite_version: String,
    pub libsqlite3_sys_version: String,
    pub sqlite_runtime_version: String,
    pub license: String,
    pub schema_version: u16,
    pub transaction_kind: SqliteTransactionKind,
    pub journal_mode: SqliteJournalMode,
    pub synchronous: SqliteSynchronous,
    pub busy_timeout_millis: u64,
    pub max_events_per_append: usize,
    pub max_replay_records: usize,
    pub max_outbox_records: usize,
    pub max_event_bytes: usize,
    pub max_snapshot_bytes: usize,
    pub max_backup_bytes: usize,
    pub max_migration_steps: usize,
    pub max_retention_deletes: usize,
    pub backup_required: bool,
    pub destructive_repair: bool,
}

pub fn admit_profile(profile: &SqliteProfile) -> CoreResult<()> {
    if profile.rusqlite_version != RUSQLITE_VERSION
        || profile.libsqlite3_sys_version != LIBSQLITE3_SYS_VERSION
        || profile.sqlite_runtime_version != SQLITE_RUNTIME_VERSION
        || profile.license != SQLITE_LICENSE
        || profile.schema_version != STORE_SCHEMA_VERSION
        || profile.transaction_kind != SqliteTransactionKind::Immediate
        || profile.journal_mode != SqliteJournalMode::Wal
        || profile.synchronous != SqliteSynchronous::Full
    {
        return Err(Diagnostic::new(
            "sqlite_profile_drift",
            "sqlite.profile",
            "SQLite library, runtime, schema, transaction, journal, or synchronization cohort differs from the pinned profile",
        ));
    }
    if profile.busy_timeout_millis == 0
        || profile.max_events_per_append == 0
        || profile.max_replay_records == 0
        || profile.max_outbox_records == 0
        || profile.max_event_bytes == 0
        || profile.max_snapshot_bytes == 0
        || profile.max_backup_bytes == 0
        || profile.max_migration_steps == 0
        || profile.max_retention_deletes == 0
    {
        return Err(Diagnostic::new(
            "sqlite_profile_bound",
            "sqlite.bounds",
            "SQLite profile bounds must be positive",
        ));
    }
    if !profile.backup_required || profile.destructive_repair {
        return Err(Diagnostic::new(
            "sqlite_unsafe_policy",
            "sqlite.profile",
            "SQLite profile requires backups and forbids destructive repair",
        ));
    }
    Ok(())
}

pub fn build_journal_record(
    stream: ExternalRef,
    expected_revision: u64,
    event: Event,
) -> CoreResult<JournalRecord> {
    let bytes = serde_json::to_vec(&event).map_err(|_| {
        Diagnostic::new(
            "sqlite_event_encoding",
            "sqlite.event",
            "event canonical JSON encoding failed",
        )
    })?;
    let identity = identity_from_bytes(&bytes).map_err(|_| {
        Diagnostic::new(
            "sqlite_event_identity",
            "sqlite.event",
            "event BLAKE3 identity failed",
        )
    })?;
    Ok(JournalRecord {
        schema_version: CONTRACT_VERSION_V1,
        stream,
        expected_revision,
        event_identity: ExternalRef::new("kiln", "event-blake3", identity.as_str())?,
        event,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendPlan {
    pub next_revision: u64,
    pub event_count: usize,
    pub outbox_count: usize,
}

// r[impl kiln.sqlite.atomic_append]
pub fn admit_append_batch(
    profile: &SqliteProfile,
    current_revision: u64,
    existing_identities: &BTreeSet<ExternalRef>,
    request: &EventAppendRequest,
) -> CoreResult<AppendPlan> {
    admit_profile(profile)?;
    request.stream.validate()?;
    if request.expected_revision != current_revision {
        return Err(Diagnostic::new(
            "sqlite_stale_revision",
            "sqlite.expected_revision",
            "event append expected revision differs from the committed stream revision",
        ));
    }
    if request.records.is_empty() || request.records.len() > profile.max_events_per_append {
        return Err(Diagnostic::new(
            "sqlite_append_bound",
            "sqlite.records",
            "event append is empty or exceeds its named bound",
        ));
    }
    if request.outbox.len() > profile.max_outbox_records {
        return Err(Diagnostic::new(
            "sqlite_outbox_bound",
            "sqlite.outbox",
            "outbox batch exceeds its named bound",
        ));
    }
    let mut identities = existing_identities.clone();
    for (offset, record) in request.records.iter().enumerate() {
        let offset = u64::try_from(offset).map_err(|_| {
            Diagnostic::new(
                "sqlite_revision_overflow",
                "sqlite.records",
                "event offset cannot be represented",
            )
        })?;
        let expected = current_revision.checked_add(offset).ok_or(Diagnostic::new(
            "sqlite_revision_overflow",
            "sqlite.records",
            "event revision overflowed",
        ))?;
        if record.stream != request.stream || record.expected_revision != expected {
            return Err(Diagnostic::new(
                "sqlite_event_sequence",
                "sqlite.records",
                "event stream or expected revision is not contiguous",
            ));
        }
        admit_record_identity(profile, record)?;
        if !identities.insert(record.event_identity.clone()) {
            return Err(Diagnostic::new(
                "sqlite_duplicate_event",
                "sqlite.event_identity",
                "event identity is already committed or duplicated in the append batch",
            ));
        }
    }
    let mut effects = BTreeSet::new();
    let mut idempotency_keys = BTreeSet::new();
    for record in &request.outbox {
        if !effects.insert(record.effect_id.clone())
            || !idempotency_keys.insert(record.idempotency_key.clone())
        {
            return Err(Diagnostic::new(
                "sqlite_duplicate_outbox",
                "sqlite.outbox",
                "outbox effect or idempotency identity is duplicated",
            ));
        }
        record.idempotency_key.validate()?;
    }
    let event_count = u64::try_from(request.records.len()).map_err(|_| {
        Diagnostic::new(
            "sqlite_revision_overflow",
            "sqlite.records",
            "event count cannot be represented",
        )
    })?;
    let next_revision = current_revision
        .checked_add(event_count)
        .ok_or(Diagnostic::new(
            "sqlite_revision_overflow",
            "sqlite.records",
            "event revision overflowed",
        ))?;
    Ok(AppendPlan {
        next_revision,
        event_count: request.records.len(),
        outbox_count: request.outbox.len(),
    })
}

fn admit_record_identity(profile: &SqliteProfile, record: &JournalRecord) -> CoreResult<()> {
    if record.schema_version != CONTRACT_VERSION_V1 {
        return Err(Diagnostic::new(
            "sqlite_event_version",
            "sqlite.event.schema_version",
            "event schema version is unsupported",
        ));
    }
    let bytes = serde_json::to_vec(&record.event).map_err(|_| {
        Diagnostic::new(
            "sqlite_event_encoding",
            "sqlite.event",
            "event canonical JSON encoding failed",
        )
    })?;
    if bytes.len() > profile.max_event_bytes {
        return Err(Diagnostic::new(
            "sqlite_event_bound",
            "sqlite.event",
            "event bytes exceed their named bound",
        ));
    }
    let identity = identity_from_bytes(&bytes).map_err(|_| {
        Diagnostic::new(
            "sqlite_event_identity",
            "sqlite.event",
            "event BLAKE3 identity failed",
        )
    })?;
    if record.event_identity.provider_namespace.as_str() != "kiln"
        || record.event_identity.native_kind.as_str() != "event-blake3"
        || record.event_identity.native_id.as_str() != identity.as_str()
    {
        return Err(Diagnostic::new(
            "sqlite_event_identity_mismatch",
            "sqlite.event_identity",
            "event bytes do not match their BLAKE3 identity",
        ));
    }
    Ok(())
}

// r[impl kiln.sqlite.replay]
pub fn replay_checked(
    profile: &SqliteProfile,
    initial: &State,
    records: &[JournalRecord],
) -> CoreResult<State> {
    if records.len() > profile.max_replay_records {
        return Err(Diagnostic::new(
            "sqlite_replay_bound",
            "sqlite.records",
            "replay history exceeds its named bound",
        ));
    }
    for record in records {
        admit_record_identity(profile, record)?;
    }
    replay(initial, records)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableLease {
    pub effect_id: EffectId,
    pub provider_profile: ProviderProfileId,
    pub state: LeaseState,
    pub owner: Option<ExternalRef>,
    pub lease_until: Option<u64>,
    pub attempts: u32,
}

// r[impl kiln.sqlite.leases]
pub fn plan_lease_claim(
    current: &DurableLease,
    owner: ExternalRef,
    now: u64,
    lease_ticks: u64,
) -> CoreResult<DurableLease> {
    owner.validate()?;
    if current.state != LeaseState::Unclaimed || lease_ticks == 0 {
        return Err(Diagnostic::new(
            "sqlite_lease_claim",
            "sqlite.lease",
            "only an unclaimed effect can receive a positive lease",
        ));
    }
    let lease_until = now.checked_add(lease_ticks).ok_or(Diagnostic::new(
        "sqlite_lease_overflow",
        "sqlite.lease_until",
        "lease bound overflowed",
    ))?;
    let attempts = current.attempts.checked_add(1).ok_or(Diagnostic::new(
        "sqlite_attempt_overflow",
        "sqlite.lease.attempts",
        "lease attempt count overflowed",
    ))?;
    Ok(DurableLease {
        effect_id: current.effect_id.clone(),
        provider_profile: current.provider_profile.clone(),
        state: LeaseState::Leased,
        owner: Some(owner),
        lease_until: Some(lease_until),
        attempts,
    })
}

#[must_use]
pub fn lease_recovery_action(lease: &DurableLease, now: u64) -> RecoveryAction {
    if lease.state == LeaseState::Leased
        && lease
            .lease_until
            .is_some_and(|lease_until| lease_until <= now)
    {
        return RecoveryAction::Reconcile;
    }
    classify_recovery(lease.state)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationPlan {
    pub steps: Vec<(u16, u16)>,
}

// r[impl kiln.sqlite.migrations]
pub fn plan_migration(
    current: u16,
    target: u16,
    admitted_edges: &[(u16, u16)],
    max_steps: usize,
) -> CoreResult<MigrationPlan> {
    if current == target {
        return Ok(MigrationPlan { steps: Vec::new() });
    }
    let mut version = current;
    let mut steps = Vec::new();
    let mut visited = BTreeSet::from([current]);
    while version != target {
        if steps.len() >= max_steps {
            return Err(Diagnostic::new(
                "sqlite_migration_bound",
                "sqlite.migrations",
                "migration plan exceeds its named step bound",
            ));
        }
        let next = admitted_edges
            .iter()
            .find_map(|(from, to)| (*from == version).then_some(*to))
            .ok_or(Diagnostic::new(
                "sqlite_migration_unknown",
                "sqlite.schema_version",
                "database schema version has no admitted migration edge",
            ))?;
        if !visited.insert(next) {
            return Err(Diagnostic::new(
                "sqlite_migration_cycle",
                "sqlite.migrations",
                "database migration plan contains a cycle",
            ));
        }
        steps.push((version, next));
        version = next;
    }
    Ok(MigrationPlan { steps })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionPlan {
    pub delete_count: usize,
    pub keep_count: usize,
}

pub fn plan_retention(
    total_records: usize,
    keep_records: usize,
    max_deletes: usize,
) -> CoreResult<RetentionPlan> {
    let delete_count = total_records.saturating_sub(keep_records);
    if delete_count > max_deletes {
        return Err(Diagnostic::new(
            "sqlite_retention_bound",
            "sqlite.retention",
            "retention delete count exceeds its named bound",
        ));
    }
    Ok(RetentionPlan {
        delete_count,
        keep_count: total_records.saturating_sub(delete_count),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppendCommitMode {
    Commit,
    InterruptAfterEvents,
}

pub struct SqliteEventStore {
    connection: Connection,
    profile: SqliteProfile,
    path: PathBuf,
}

impl fmt::Debug for SqliteEventStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteEventStore")
            .field("profile", &self.profile)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl SqliteEventStore {
    pub fn open(path: &Path, profile: SqliteProfile) -> Result<Self, PortFailure> {
        admit_profile(&profile).map_err(PortFailure::Domain)?;
        if rusqlite::version() != profile.sqlite_runtime_version {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_runtime_drift",
                "sqlite.runtime_version",
                "loaded SQLite runtime differs from the admitted profile",
            )));
        }
        let connection = Connection::open(path).map_err(infrastructure)?;
        connection
            .busy_timeout(Duration::from_millis(profile.busy_timeout_millis))
            .map_err(infrastructure)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(infrastructure)?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(infrastructure)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(infrastructure)?;
        initialize_or_admit_schema(&connection, &profile)?;
        quick_check(&connection)?;
        Ok(Self {
            connection,
            profile,
            path: path.to_path_buf(),
        })
    }

    pub fn append_with_mode(
        &mut self,
        request: &EventAppendRequest,
        mode: AppendCommitMode,
    ) -> Result<u64, PortFailure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(infrastructure)?;
        let stream_key = encode_external_ref(&request.stream)?;
        let current_revision = current_revision(&transaction, &stream_key)?;
        let existing = existing_event_identities(&transaction, &stream_key)?;
        let plan = admit_append_batch(&self.profile, current_revision, &existing, request)
            .map_err(PortFailure::Domain)?;
        for record in &request.records {
            insert_event(&transaction, &self.profile, &stream_key, record)?;
        }
        if mode == AppendCommitMode::InterruptAfterEvents {
            return Err(PortFailure::Infrastructure(InfrastructureFailure {
                code: "sqlite_interrupted".to_string(),
                message: "transaction stopped before outbox writes and commit".to_string(),
            }));
        }
        for record in &request.outbox {
            insert_outbox(&transaction, record)?;
        }
        update_stream_revision(&transaction, &stream_key, plan.next_revision)?;
        transaction.commit().map_err(infrastructure)?;
        Ok(plan.next_revision)
    }

    pub fn replay_stream(
        &self,
        stream: &ExternalRef,
        initial: &State,
    ) -> Result<State, PortFailure> {
        let snapshot = self.load(stream, self.profile.max_replay_records)?;
        let replay_initial = match snapshot.records.first() {
            Some(first) if first.expected_revision != initial.revision => {
                let stored = self.load_snapshot(stream)?.ok_or_else(|| {
                    PortFailure::Domain(Diagnostic::new(
                        "sqlite_replay_gap",
                        "sqlite.snapshot",
                        "retained history requires an exact committed snapshot",
                    ))
                })?;
                if stored.0 != first.expected_revision {
                    return Err(PortFailure::Domain(Diagnostic::new(
                        "sqlite_replay_gap",
                        "sqlite.snapshot.revision",
                        "snapshot revision does not match retained history",
                    )));
                }
                stored.1
            }
            _ => initial.clone(),
        };
        replay_checked(&self.profile, &replay_initial, &snapshot.records)
            .map_err(PortFailure::Domain)
    }

    pub fn save_snapshot(
        &mut self,
        stream: &ExternalRef,
        revision: u64,
        state: &State,
    ) -> Result<ContentId, PortFailure> {
        let bytes = serde_json::to_vec(state).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_snapshot_encoding",
                "sqlite.snapshot",
                "snapshot encoding failed",
            ))
        })?;
        if bytes.len() > self.profile.max_snapshot_bytes {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_snapshot_bound",
                "sqlite.snapshot",
                "snapshot bytes exceed their named bound",
            )));
        }
        let identity = identity_from_bytes(&bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_snapshot_identity",
                "sqlite.snapshot",
                "snapshot BLAKE3 identity failed",
            ))
        })?;
        let stream_key = encode_external_ref(stream)?;
        let revision = database_i64(revision, "sqlite.snapshot.revision")?;
        self.connection
            .execute(
                "INSERT OR REPLACE INTO snapshots (stream_key, revision, state_identity, state_bytes) VALUES (?1, ?2, ?3, ?4)",
                params![stream_key, revision, identity.as_str(), bytes],
            )
            .map_err(infrastructure)?;
        Ok(identity)
    }

    pub fn load_snapshot(
        &self,
        stream: &ExternalRef,
    ) -> Result<Option<(u64, State, ContentId)>, PortFailure> {
        let stream_key = encode_external_ref(stream)?;
        let row = self
            .connection
            .query_row(
                "SELECT revision, state_identity, state_bytes FROM snapshots WHERE stream_key = ?1 ORDER BY revision DESC LIMIT 1",
                [stream_key],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(infrastructure)?;
        let Some((revision, stored_identity, bytes)) = row else {
            return Ok(None);
        };
        let revision = database_u64(revision, "sqlite.snapshot.revision")?;
        let identity = identity_from_bytes(&bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_snapshot_identity",
                "sqlite.snapshot",
                "snapshot BLAKE3 identity failed",
            ))
        })?;
        if identity.as_str() != stored_identity {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_snapshot_damage",
                "sqlite.snapshot",
                "snapshot bytes do not match their stored identity",
            )));
        }
        let state = serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_snapshot_decode",
                "sqlite.snapshot",
                "snapshot bytes are malformed",
            ))
        })?;
        Ok(Some((revision, state, identity)))
    }

    pub fn claim_lease(
        &mut self,
        effect_id: &EffectId,
        owner: ExternalRef,
        now: u64,
        lease_ticks: u64,
    ) -> Result<DurableLease, PortFailure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(infrastructure)?;
        let current = load_lease(&transaction, effect_id)?.ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_effect_missing",
                "sqlite.outbox.effect_id",
                "outbox effect does not exist",
            ))
        })?;
        let next =
            plan_lease_claim(&current, owner, now, lease_ticks).map_err(PortFailure::Domain)?;
        let owner_json = next.owner.as_ref().map(encode_external_ref).transpose()?;
        let lease_until = next
            .lease_until
            .map(|value| database_i64(value, "sqlite.lease_until"))
            .transpose()?;
        transaction
            .execute(
                "UPDATE outbox SET lease_state = ?1, owner_json = ?2, lease_until = ?3, attempts = ?4 WHERE effect_id = ?5",
                params![
                    lease_state_text(next.state),
                    owner_json,
                    lease_until,
                    next.attempts,
                    next.effect_id.as_str(),
                ],
            )
            .map_err(infrastructure)?;
        transaction.commit().map_err(infrastructure)?;
        Ok(next)
    }

    pub fn recovery_actions(
        &self,
        now: u64,
    ) -> Result<Vec<(EffectId, RecoveryAction)>, PortFailure> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT effect_id, provider_profile, lease_state, owner_json, lease_until, attempts FROM outbox ORDER BY effect_id LIMIT ?1",
            )
            .map_err(infrastructure)?;
        let max_records = i64::try_from(self.profile.max_outbox_records).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_outbox_bound",
                "sqlite.outbox",
                "outbox bound cannot be represented",
            ))
        })?;
        let rows = statement
            .query_map([max_records], decode_lease_row)
            .map_err(infrastructure)?;
        let mut actions = Vec::new();
        for row in rows {
            let lease = row.map_err(infrastructure)?;
            actions.push((lease.effect_id.clone(), lease_recovery_action(&lease, now)));
        }
        Ok(actions)
    }

    pub fn complete_lease(&mut self, effect_id: &EffectId) -> Result<(), PortFailure> {
        let changed = self
            .connection
            .execute(
                "UPDATE outbox SET lease_state = ?1 WHERE effect_id = ?2 AND lease_state = ?3",
                params![
                    LEASE_STATE_COMPLETED,
                    effect_id.as_str(),
                    LEASE_STATE_LEASED,
                ],
            )
            .map_err(infrastructure)?;
        if changed != 1 {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_lease_complete",
                "sqlite.outbox",
                "only one leased effect can complete",
            )));
        }
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<(), PortFailure> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(PASSIVE);")
            .map_err(infrastructure)
    }

    pub fn backup_to(&self, destination: &Path) -> Result<BackupObservation, PortFailure> {
        if destination.exists() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_backup_exists",
                "sqlite.backup",
                "backup destination already exists",
            )));
        }
        let destination_text = destination.to_string_lossy().into_owned();
        self.connection
            .execute("VACUUM INTO ?1", [destination_text])
            .map_err(infrastructure)?;
        let backup = Connection::open(destination).map_err(infrastructure)?;
        quick_check(&backup)?;
        let bytes = std::fs::read(destination).map_err(io_infrastructure)?;
        if bytes.len() > self.profile.max_backup_bytes {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_backup_bound",
                "sqlite.backup",
                "backup bytes exceed their named bound",
            )));
        }
        let identity = identity_from_bytes(&bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_backup_identity",
                "sqlite.backup",
                "backup BLAKE3 identity failed",
            ))
        })?;
        Ok(BackupObservation {
            identity,
            byte_count: bytes.len(),
            integrity_ok: true,
        })
    }

    pub fn prune_before(
        &mut self,
        stream: &ExternalRef,
        keep_from_revision: u64,
    ) -> Result<RetentionPlan, PortFailure> {
        let stream_key = encode_external_ref(stream)?;
        let snapshot = self.load_snapshot(stream)?.ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_retention_snapshot",
                "sqlite.snapshot",
                "retention requires a committed snapshot",
            ))
        })?;
        if snapshot.0 != keep_from_revision {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_retention_snapshot",
                "sqlite.snapshot.revision",
                "retention boundary must equal the committed snapshot revision",
            )));
        }
        let total = retained_record_count(&self.connection, &stream_key)?;
        let delete_count = records_before(&self.connection, &stream_key, keep_from_revision)?;
        let delete_count = usize::try_from(delete_count).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_retention_bound",
                "sqlite.retention",
                "retention revision cannot be represented",
            ))
        })?;
        let total = usize::try_from(total).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_retention_bound",
                "sqlite.retention",
                "stream revision cannot be represented",
            ))
        })?;
        let plan = plan_retention(
            total,
            total.saturating_sub(delete_count),
            self.profile.max_retention_deletes,
        )
        .map_err(PortFailure::Domain)?;
        let keep_from_revision =
            database_i64(keep_from_revision, "sqlite.retention.keep_from_revision")?;
        self.connection
            .execute(
                "DELETE FROM events WHERE stream_key = ?1 AND sequence < ?2",
                params![stream_key, keep_from_revision],
            )
            .map_err(infrastructure)?;
        Ok(plan)
    }

    pub fn migrate_path(
        path: &Path,
        profile: &SqliteProfile,
        target: u16,
    ) -> Result<MigrationPlan, PortFailure> {
        admit_profile(profile).map_err(PortFailure::Domain)?;
        let mut connection = Connection::open(path).map_err(infrastructure)?;
        let current = read_schema_version(&connection)?.ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_migration_metadata",
                "sqlite.schema_version",
                "migration requires an existing metadata table",
            ))
        })?;
        let plan = plan_migration(
            current,
            target,
            &[(INITIAL_SCHEMA_VERSION, STORE_SCHEMA_VERSION)],
            profile.max_migration_steps,
        )
        .map_err(PortFailure::Domain)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(infrastructure)?;
        for (from, to) in &plan.steps {
            if (*from, *to) != (INITIAL_SCHEMA_VERSION, STORE_SCHEMA_VERSION) {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "sqlite_migration_unknown",
                    "sqlite.migrations",
                    "migration step has no shell implementation",
                )));
            }
            create_schema_tables(&transaction)?;
            transaction
                .execute(
                    "UPDATE metadata SET value = ?1 WHERE key = ?2",
                    params![to.to_string(), METADATA_SCHEMA_KEY],
                )
                .map_err(infrastructure)?;
            transaction
                .execute(
                    "INSERT INTO migrations (from_version, to_version) VALUES (?1, ?2)",
                    params![from, to],
                )
                .map_err(infrastructure)?;
        }
        transaction.commit().map_err(infrastructure)?;
        quick_check(&connection)?;
        Ok(plan)
    }
}

impl EventStorePort for SqliteEventStore {
    fn append(&mut self, request: &EventAppendRequest) -> Result<u64, PortFailure> {
        self.append_with_mode(request, AppendCommitMode::Commit)
    }

    fn load(
        &self,
        stream: &ExternalRef,
        max_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        if max_records == 0 || max_records > self.profile.max_replay_records {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_load_bound",
                "sqlite.max_records",
                "load bound is zero or exceeds the admitted replay bound",
            )));
        }
        let stream_key = encode_external_ref(stream)?;
        let retained = retained_record_count(&self.connection, &stream_key)?;
        let max_records_u64 = u64::try_from(max_records).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_load_bound",
                "sqlite.max_records",
                "load bound cannot be represented",
            ))
        })?;
        if retained > max_records_u64 {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_load_bound",
                "sqlite.max_records",
                "retained history exceeds the requested load bound",
            )));
        }
        let records = load_records(&self.connection, &self.profile, &stream_key, max_records)?;
        let outbox = load_outbox(&self.connection, self.profile.max_outbox_records)?;
        Ok(EventStoreSnapshot { records, outbox })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupObservation {
    pub identity: ContentId,
    pub byte_count: usize,
    pub integrity_ok: bool,
}

fn initialize_or_admit_schema(
    connection: &Connection,
    profile: &SqliteProfile,
) -> Result<(), PortFailure> {
    match read_schema_version(connection)? {
        None => {
            let transaction = connection.unchecked_transaction().map_err(infrastructure)?;
            create_schema_tables(&transaction)?;
            transaction
                .execute(
                    "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
                    params![METADATA_SCHEMA_KEY, profile.schema_version.to_string()],
                )
                .map_err(infrastructure)?;
            transaction.commit().map_err(infrastructure)?;
            Ok(())
        }
        Some(version) if version == profile.schema_version => Ok(()),
        Some(_) => Err(PortFailure::Domain(Diagnostic::new(
            "sqlite_schema_unknown",
            "sqlite.schema_version",
            "database schema version is unsupported and requires explicit migration",
        ))),
    }
}

fn read_schema_version(connection: &Connection) -> Result<Option<u16>, PortFailure> {
    let metadata_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'metadata')",
            [],
            |row| row.get(0),
        )
        .map_err(infrastructure)?;
    if !metadata_exists {
        return Ok(None);
    }
    let value = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            [METADATA_SCHEMA_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(infrastructure)?;
    value
        .map(|value| {
            value.parse::<u16>().map_err(|_| {
                PortFailure::Domain(Diagnostic::new(
                    "sqlite_schema_metadata",
                    "sqlite.schema_version",
                    "database schema version is malformed",
                ))
            })
        })
        .transpose()
}

fn create_schema_tables(connection: &Connection) -> Result<(), PortFailure> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS streams (
                stream_key TEXT PRIMARY KEY,
                revision INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                stream_key TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                schema_version INTEGER NOT NULL,
                event_identity_json TEXT NOT NULL UNIQUE,
                event_bytes BLOB NOT NULL,
                PRIMARY KEY (stream_key, sequence)
            );
            CREATE TABLE IF NOT EXISTS outbox (
                effect_id TEXT PRIMARY KEY,
                provider_profile TEXT NOT NULL,
                idempotency_key_json TEXT NOT NULL UNIQUE,
                lease_state TEXT NOT NULL,
                owner_json TEXT,
                lease_until INTEGER,
                attempts INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS snapshots (
                stream_key TEXT NOT NULL,
                revision INTEGER NOT NULL,
                state_identity TEXT NOT NULL,
                state_bytes BLOB NOT NULL,
                PRIMARY KEY (stream_key, revision)
            );
            CREATE TABLE IF NOT EXISTS migrations (
                ordinal INTEGER PRIMARY KEY AUTOINCREMENT,
                from_version INTEGER NOT NULL,
                to_version INTEGER NOT NULL
            );",
        )
        .map_err(infrastructure)
}

fn current_revision(connection: &Connection, stream_key: &str) -> Result<u64, PortFailure> {
    let value = connection
        .query_row(
            "SELECT revision FROM streams WHERE stream_key = ?1",
            [stream_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(infrastructure)?;
    value
        .map(|value| database_u64(value, "sqlite.stream_revision"))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn update_stream_revision(
    transaction: &Transaction<'_>,
    stream_key: &str,
    revision: u64,
) -> Result<(), PortFailure> {
    let revision = database_i64(revision, "sqlite.stream_revision")?;
    transaction
        .execute(
            "INSERT INTO streams (stream_key, revision) VALUES (?1, ?2) ON CONFLICT(stream_key) DO UPDATE SET revision = excluded.revision",
            params![stream_key, revision],
        )
        .map_err(infrastructure)?;
    Ok(())
}

fn retained_record_count(connection: &Connection, stream_key: &str) -> Result<u64, PortFailure> {
    let value = connection
        .query_row(
            "SELECT COUNT(*) FROM events WHERE stream_key = ?1",
            [stream_key],
            |row| row.get::<_, i64>(0),
        )
        .map_err(infrastructure)?;
    database_u64(value, "sqlite.retained_record_count")
}

fn records_before(
    connection: &Connection,
    stream_key: &str,
    revision: u64,
) -> Result<u64, PortFailure> {
    let revision = database_i64(revision, "sqlite.retention.revision")?;
    let value = connection
        .query_row(
            "SELECT COUNT(*) FROM events WHERE stream_key = ?1 AND sequence < ?2",
            params![stream_key, revision],
            |row| row.get::<_, i64>(0),
        )
        .map_err(infrastructure)?;
    database_u64(value, "sqlite.retention.delete_count")
}

fn existing_event_identities(
    transaction: &Transaction<'_>,
    stream_key: &str,
) -> Result<BTreeSet<ExternalRef>, PortFailure> {
    let mut statement = transaction
        .prepare("SELECT event_identity_json FROM events WHERE stream_key = ?1")
        .map_err(infrastructure)?;
    let rows = statement
        .query_map([stream_key], |row| row.get::<_, String>(0))
        .map_err(infrastructure)?;
    let mut identities = BTreeSet::new();
    for row in rows {
        identities.insert(decode_external_ref(&row.map_err(infrastructure)?)?);
    }
    Ok(identities)
}

fn insert_event(
    transaction: &Transaction<'_>,
    profile: &SqliteProfile,
    stream_key: &str,
    record: &JournalRecord,
) -> Result<(), PortFailure> {
    admit_record_identity(profile, record).map_err(PortFailure::Domain)?;
    let identity = encode_external_ref(&record.event_identity)?;
    let event_bytes = serde_json::to_vec(&record.event).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_event_encoding",
            "sqlite.event",
            "event canonical JSON encoding failed",
        ))
    })?;
    let expected_revision =
        database_i64(record.expected_revision, "sqlite.event.expected_revision")?;
    transaction
        .execute(
            "INSERT INTO events (stream_key, sequence, schema_version, event_identity_json, event_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                stream_key,
                expected_revision,
                record.schema_version,
                identity,
                event_bytes,
            ],
        )
        .map_err(infrastructure)?;
    Ok(())
}

fn insert_outbox(transaction: &Transaction<'_>, record: &OutboxRecord) -> Result<(), PortFailure> {
    let idempotency = encode_external_ref(&record.idempotency_key)?;
    transaction
        .execute(
            "INSERT INTO outbox (effect_id, provider_profile, idempotency_key_json, lease_state, owner_json, lease_until, attempts) VALUES (?1, ?2, ?3, ?4, NULL, NULL, 0)",
            params![
                record.effect_id.as_str(),
                record.provider_profile.as_str(),
                idempotency,
                lease_state_text(record.lease),
            ],
        )
        .map_err(infrastructure)?;
    Ok(())
}

fn load_records(
    connection: &Connection,
    profile: &SqliteProfile,
    stream_key: &str,
    max_records: usize,
) -> Result<Vec<JournalRecord>, PortFailure> {
    let max_records = i64::try_from(max_records).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_load_bound",
            "sqlite.max_records",
            "load bound cannot be represented",
        ))
    })?;
    let mut statement = connection
        .prepare(
            "SELECT sequence, schema_version, event_identity_json, event_bytes FROM events WHERE stream_key = ?1 ORDER BY sequence LIMIT ?2",
        )
        .map_err(infrastructure)?;
    let rows = statement
        .query_map(params![stream_key, max_records], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, u16>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(infrastructure)?;
    let stream = decode_external_ref(stream_key)?;
    let mut records = Vec::new();
    for row in rows {
        let (sequence, schema_version, identity_json, event_bytes) = row.map_err(infrastructure)?;
        let sequence = database_u64(sequence, "sqlite.event.sequence")?;
        if event_bytes.len() > profile.max_event_bytes {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_event_bound",
                "sqlite.event",
                "stored event bytes exceed their named bound",
            )));
        }
        let event_identity = decode_external_ref(&identity_json)?;
        let identity = identity_from_bytes(&event_bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_event_identity",
                "sqlite.event",
                "stored event BLAKE3 identity failed",
            ))
        })?;
        if event_identity.native_id.as_str() != identity.as_str() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "sqlite_store_damage",
                "sqlite.event",
                "stored event bytes do not match their identity",
            )));
        }
        let event = serde_json::from_slice(&event_bytes).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "sqlite_event_decode",
                "sqlite.event",
                "stored event bytes are malformed",
            ))
        })?;
        records.push(JournalRecord {
            schema_version,
            stream: stream.clone(),
            expected_revision: sequence,
            event_identity,
            event,
        });
    }
    Ok(records)
}

fn load_outbox(
    connection: &Connection,
    max_records: usize,
) -> Result<Vec<OutboxRecord>, PortFailure> {
    let count = connection
        .query_row("SELECT COUNT(*) FROM outbox", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(infrastructure)?;
    let count = database_u64(count, "sqlite.outbox.count")?;
    let max_records_u64 = u64::try_from(max_records).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_outbox_bound",
            "sqlite.outbox",
            "outbox bound cannot be represented",
        ))
    })?;
    if count > max_records_u64 {
        return Err(PortFailure::Domain(Diagnostic::new(
            "sqlite_outbox_bound",
            "sqlite.outbox",
            "outbox record count exceeds its named bound",
        )));
    }
    let max_records = i64::try_from(max_records).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_outbox_bound",
            "sqlite.outbox",
            "outbox bound cannot be represented",
        ))
    })?;
    let mut statement = connection
        .prepare(
            "SELECT effect_id, provider_profile, idempotency_key_json, lease_state FROM outbox ORDER BY effect_id LIMIT ?1",
        )
        .map_err(infrastructure)?;
    let rows = statement
        .query_map([max_records], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(infrastructure)?;
    let mut records = Vec::new();
    for row in rows {
        let (effect_id, provider_profile, idempotency, lease) = row.map_err(infrastructure)?;
        records.push(OutboxRecord {
            effect_id: EffectId::new(effect_id).map_err(PortFailure::Domain)?,
            provider_profile: ProviderProfileId::new(provider_profile)
                .map_err(PortFailure::Domain)?,
            idempotency_key: decode_external_ref(&idempotency)?,
            lease: parse_lease_state(&lease)?,
        });
    }
    Ok(records)
}

fn load_lease(
    transaction: &Transaction<'_>,
    effect_id: &EffectId,
) -> Result<Option<DurableLease>, PortFailure> {
    transaction
        .query_row(
            "SELECT effect_id, provider_profile, lease_state, owner_json, lease_until, attempts FROM outbox WHERE effect_id = ?1",
            [effect_id.as_str()],
            decode_lease_row,
        )
        .optional()
        .map_err(infrastructure)
}

fn decode_lease_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DurableLease> {
    let effect_id: String = row.get(0)?;
    let provider_profile: String = row.get(1)?;
    let state: String = row.get(2)?;
    let owner_json: Option<String> = row.get(3)?;
    let lease_until: Option<i64> = row.get(4)?;
    let attempts: u32 = row.get(5)?;
    let effect_id = EffectId::new(effect_id).map_err(to_sql_conversion)?;
    let provider_profile = ProviderProfileId::new(provider_profile).map_err(to_sql_conversion)?;
    let state = parse_lease_state_core(&state).map_err(to_sql_conversion)?;
    let lease_until = lease_until
        .map(|value| {
            u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, value))
        })
        .transpose()?;
    let owner = owner_json
        .map(|value| decode_external_ref_core(&value))
        .transpose()
        .map_err(to_sql_conversion)?;
    Ok(DurableLease {
        effect_id,
        provider_profile,
        state,
        owner,
        lease_until,
        attempts,
    })
}

fn to_sql_conversion(error: Diagnostic) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.message,
        )),
    )
}

fn encode_external_ref(value: &ExternalRef) -> Result<String, PortFailure> {
    serde_json::to_string(value).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_reference_encoding",
            "sqlite.external_ref",
            "external reference encoding failed",
        ))
    })
}

fn decode_external_ref(value: &str) -> Result<ExternalRef, PortFailure> {
    decode_external_ref_core(value).map_err(PortFailure::Domain)
}

fn decode_external_ref_core(value: &str) -> CoreResult<ExternalRef> {
    let reference: ExternalRef = serde_json::from_str(value).map_err(|_| {
        Diagnostic::new(
            "sqlite_reference_decode",
            "sqlite.external_ref",
            "stored external reference is malformed",
        )
    })?;
    reference.validate()?;
    Ok(reference)
}

const fn lease_state_text(state: LeaseState) -> &'static str {
    match state {
        LeaseState::Unclaimed => LEASE_STATE_UNCLAIMED,
        LeaseState::Leased => LEASE_STATE_LEASED,
        LeaseState::Expired => LEASE_STATE_EXPIRED,
        LeaseState::Completed => LEASE_STATE_COMPLETED,
        LeaseState::Ambiguous => LEASE_STATE_AMBIGUOUS,
    }
}

fn parse_lease_state(value: &str) -> Result<LeaseState, PortFailure> {
    parse_lease_state_core(value).map_err(PortFailure::Domain)
}

fn parse_lease_state_core(value: &str) -> CoreResult<LeaseState> {
    match value {
        LEASE_STATE_UNCLAIMED => Ok(LeaseState::Unclaimed),
        LEASE_STATE_LEASED => Ok(LeaseState::Leased),
        LEASE_STATE_EXPIRED => Ok(LeaseState::Expired),
        LEASE_STATE_COMPLETED => Ok(LeaseState::Completed),
        LEASE_STATE_AMBIGUOUS => Ok(LeaseState::Ambiguous),
        _ => Err(Diagnostic::new(
            "sqlite_lease_state",
            "sqlite.lease_state",
            "stored lease state is unsupported",
        )),
    }
}

fn database_i64(value: u64, field: &'static str) -> Result<i64, PortFailure> {
    i64::try_from(value).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_integer_bound",
            field,
            "unsigned value cannot be represented by SQLite INTEGER",
        ))
    })
}

fn database_u64(value: i64, field: &'static str) -> Result<u64, PortFailure> {
    u64::try_from(value).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "sqlite_integer_bound",
            field,
            "SQLite INTEGER cannot be represented by the unsigned domain value",
        ))
    })
}

fn quick_check(connection: &Connection) -> Result<(), PortFailure> {
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(infrastructure)?;
    if result != INTEGRITY_OK {
        return Err(PortFailure::Domain(Diagnostic::new(
            "sqlite_integrity",
            "sqlite.database",
            "SQLite integrity check reported damage",
        )));
    }
    Ok(())
}

fn infrastructure(error: rusqlite::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "sqlite_infrastructure".to_string(),
        message: error.to_string(),
    })
}

fn io_infrastructure(error: std::io::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "sqlite_io".to_string(),
        message: error.to_string(),
    })
}

// r[impl kiln.sqlite.conformance]
// r[impl kiln.sqlite.nonclaims]
#[cfg(test)]
mod tests {
    use kiln_core::types::CommandId;
    use tempfile::TempDir;

    use super::*;

    const BUSY_TIMEOUT_MILLIS: u64 = 10;
    const MAX_EVENTS_PER_APPEND: usize = 8;
    const MAX_REPLAY_RECORDS: usize = 64;
    const MAX_OUTBOX_RECORDS: usize = 64;
    const MAX_EVENT_BYTES: usize = 4096;
    const MAX_SNAPSHOT_BYTES: usize = 16_384;
    const MAX_BACKUP_BYTES: usize = 4_194_304;
    const MAX_MIGRATION_STEPS: usize = 4;
    const MAX_RETENTION_DELETES: usize = 64;
    const LEASE_TICKS: u64 = 30;
    const NOW: u64 = 100;
    const UNKNOWN_SCHEMA_VERSION: u16 = 99;

    fn profile() -> SqliteProfile {
        SqliteProfile {
            rusqlite_version: RUSQLITE_VERSION.to_string(),
            libsqlite3_sys_version: LIBSQLITE3_SYS_VERSION.to_string(),
            sqlite_runtime_version: SQLITE_RUNTIME_VERSION.to_string(),
            license: SQLITE_LICENSE.to_string(),
            schema_version: STORE_SCHEMA_VERSION,
            transaction_kind: SqliteTransactionKind::Immediate,
            journal_mode: SqliteJournalMode::Wal,
            synchronous: SqliteSynchronous::Full,
            busy_timeout_millis: BUSY_TIMEOUT_MILLIS,
            max_events_per_append: MAX_EVENTS_PER_APPEND,
            max_replay_records: MAX_REPLAY_RECORDS,
            max_outbox_records: MAX_OUTBOX_RECORDS,
            max_event_bytes: MAX_EVENT_BYTES,
            max_snapshot_bytes: MAX_SNAPSHOT_BYTES,
            max_backup_bytes: MAX_BACKUP_BYTES,
            max_migration_steps: MAX_MIGRATION_STEPS,
            max_retention_deletes: MAX_RETENTION_DELETES,
            backup_required: true,
            destructive_repair: false,
        }
    }

    fn temp_store() -> (TempDir, PathBuf, SqliteEventStore) {
        let directory = TempDir::new().unwrap_or_else(|_| unreachable!());
        let path = directory.path().join("kiln.db");
        let store = SqliteEventStore::open(&path, profile()).unwrap_or_else(|_| unreachable!());
        (directory, path, store)
    }

    fn stream() -> ExternalRef {
        ExternalRef::new("kiln", "event-stream", "main").unwrap_or_else(|_| unreachable!())
    }

    fn event(command: &str, revision: u64) -> JournalRecord {
        hold_event(command, revision, true)
    }

    fn hold_event(command: &str, revision: u64, held: bool) -> JournalRecord {
        build_journal_record(
            stream(),
            revision,
            Event::HoldChanged {
                command_id: CommandId::new(command).unwrap_or_else(|_| unreachable!()),
                scope: ExternalRef::new("kiln", "scope", "global")
                    .unwrap_or_else(|_| unreachable!()),
                held,
            },
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn outbox(effect: &str) -> OutboxRecord {
        OutboxRecord {
            effect_id: EffectId::new(effect).unwrap_or_else(|_| unreachable!()),
            provider_profile: ProviderProfileId::new("provider").unwrap_or_else(|_| unreachable!()),
            idempotency_key: ExternalRef::new("kiln", "command", effect)
                .unwrap_or_else(|_| unreachable!()),
            lease: LeaseState::Unclaimed,
        }
    }

    fn append_request(command: &str, effect: &str) -> EventAppendRequest {
        EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![event(command, 0)],
            outbox: vec![outbox(effect)],
        }
    }

    #[test]
    fn pure_append_model_rejects_stale_duplicate_and_gap_inputs() {
        let request = append_request("command-a", "effect-a");
        assert!(admit_append_batch(&profile(), 1, &BTreeSet::new(), &request).is_err());

        let identity = request
            .records
            .first()
            .map(|record| record.event_identity.clone())
            .unwrap_or_else(|| unreachable!());
        let existing = BTreeSet::from([identity]);
        assert!(admit_append_batch(&profile(), 0, &existing, &request).is_err());

        let mut gap = request;
        gap.records
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .expected_revision = 1;
        assert!(admit_append_batch(&profile(), 0, &BTreeSet::new(), &gap).is_err());
    }

    // r[verify kiln.sqlite.profile.scenario.valid]
    // r[verify kiln.sqlite.atomic_append.scenario.commit]
    // r[verify kiln.sqlite.replay.scenario.valid]
    #[test]
    fn committed_events_and_outbox_replay_after_restart() {
        let (_directory, path, mut store) = temp_store();
        let request = append_request("command-a", "effect-a");
        assert_eq!(store.append(&request), Ok(1));
        drop(store);
        let reopened = SqliteEventStore::open(&path, profile()).unwrap_or_else(|_| unreachable!());
        let snapshot = reopened
            .load(&stream(), MAX_REPLAY_RECORDS)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(snapshot.records.len(), 1);
        assert_eq!(snapshot.outbox.len(), 1);
        let state = reopened
            .replay_stream(&stream(), &State::default())
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(state.revision, 1);
    }

    // r[verify kiln.sqlite.atomic_append.scenario.interrupt]
    #[test]
    fn interrupted_transaction_exposes_neither_side() {
        let (_directory, _path, mut store) = temp_store();
        let request = append_request("command-a", "effect-a");
        assert!(
            store
                .append_with_mode(&request, AppendCommitMode::InterruptAfterEvents)
                .is_err()
        );
        let snapshot = store
            .load(&stream(), MAX_REPLAY_RECORDS)
            .unwrap_or_else(|_| unreachable!());
        assert!(snapshot.records.is_empty());
        assert!(snapshot.outbox.is_empty());
    }

    // r[verify kiln.sqlite.replay.scenario.damage]
    #[test]
    fn damaged_event_bytes_stop_replay() {
        let (_directory, path, mut store) = temp_store();
        store
            .append(&append_request("command-a", "effect-a"))
            .unwrap_or_else(|_| unreachable!());
        drop(store);
        let connection = Connection::open(&path).unwrap_or_else(|_| unreachable!());
        connection
            .execute(
                "UPDATE events SET event_bytes = ?1",
                [b"damaged".as_slice()],
            )
            .unwrap_or_else(|_| unreachable!());
        drop(connection);
        let reopened = SqliteEventStore::open(&path, profile()).unwrap_or_else(|_| unreachable!());
        assert!(reopened.load(&stream(), MAX_REPLAY_RECORDS).is_err());
    }

    // r[verify kiln.sqlite.leases.scenario.restart]
    // r[verify kiln.sqlite.leases.scenario.ambiguous]
    #[test]
    fn lease_survives_restart_and_expiry_requires_reconciliation() {
        let (_directory, path, mut store) = temp_store();
        store
            .append(&append_request("command-a", "effect-a"))
            .unwrap_or_else(|_| unreachable!());
        let effect = EffectId::new("effect-a").unwrap_or_else(|_| unreachable!());
        let lease = store
            .claim_lease(
                &effect,
                ExternalRef::new("kiln", "worker", "worker-a").unwrap_or_else(|_| unreachable!()),
                NOW,
                LEASE_TICKS,
            )
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(lease.state, LeaseState::Leased);
        drop(store);
        let reopened = SqliteEventStore::open(&path, profile()).unwrap_or_else(|_| unreachable!());
        let actions = reopened
            .recovery_actions(NOW.saturating_add(LEASE_TICKS))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(actions, vec![(effect, RecoveryAction::Reconcile)]);
    }

    // r[verify kiln.sqlite.migrations.scenario.valid]
    // r[verify kiln.sqlite.migrations.scenario.unknown]
    #[test]
    fn explicit_migration_accepts_known_edge_and_rejects_unknown_version() {
        let directory = TempDir::new().unwrap_or_else(|_| unreachable!());
        let path = directory.path().join("migration.db");
        let connection = Connection::open(&path).unwrap_or_else(|_| unreachable!());
        connection
            .execute_batch("CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap_or_else(|_| unreachable!());
        connection
            .execute(
                "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
                params![METADATA_SCHEMA_KEY, INITIAL_SCHEMA_VERSION.to_string()],
            )
            .unwrap_or_else(|_| unreachable!());
        drop(connection);
        let plan = SqliteEventStore::migrate_path(&path, &profile(), STORE_SCHEMA_VERSION)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            plan.steps,
            vec![(INITIAL_SCHEMA_VERSION, STORE_SCHEMA_VERSION)]
        );

        let unknown = plan_migration(
            UNKNOWN_SCHEMA_VERSION,
            STORE_SCHEMA_VERSION,
            &[(INITIAL_SCHEMA_VERSION, STORE_SCHEMA_VERSION)],
            MAX_MIGRATION_STEPS,
        );
        assert!(unknown.is_err());
    }

    // r[verify kiln.sqlite.conformance.scenario.crash]
    #[test]
    fn lock_backup_snapshot_and_bounds_are_explicit() {
        let (directory, path, mut store) = temp_store();
        store
            .append(&append_request("command-a", "effect-a"))
            .unwrap_or_else(|_| unreachable!());
        let snapshot_identity = store
            .save_snapshot(&stream(), 1, &State::default())
            .unwrap_or_else(|_| unreachable!());
        let loaded_snapshot = store
            .load_snapshot(&stream())
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        assert_eq!(loaded_snapshot.2, snapshot_identity);

        let backup_path = directory.path().join("backup.db");
        let backup = store
            .backup_to(&backup_path)
            .unwrap_or_else(|_| unreachable!());
        assert!(backup.integrity_ok);
        assert!(backup.byte_count > 0);

        let locking = Connection::open(&path).unwrap_or_else(|_| unreachable!());
        locking
            .execute_batch("BEGIN IMMEDIATE;")
            .unwrap_or_else(|_| unreachable!());
        let mut next = append_request("command-b", "effect-b");
        next.expected_revision = 1;
        next.records = vec![event("command-b", 1)];
        assert!(store.append(&next).is_err());
        locking
            .execute_batch("ROLLBACK;")
            .unwrap_or_else(|_| unreachable!());

        assert!(store.load(&stream(), 0).is_err());
    }

    #[test]
    fn retention_preserves_stream_revision_and_replays_from_exact_snapshot() {
        let (_directory, _path, mut store) = temp_store();
        store
            .append(&append_request("command-a", "effect-a"))
            .unwrap_or_else(|_| unreachable!());
        let state_at_snapshot = store
            .replay_stream(&stream(), &State::default())
            .unwrap_or_else(|_| unreachable!());
        store
            .save_snapshot(&stream(), state_at_snapshot.revision, &state_at_snapshot)
            .unwrap_or_else(|_| unreachable!());

        let mut next = append_request("command-b", "effect-b");
        next.expected_revision = state_at_snapshot.revision;
        next.records = vec![hold_event("command-b", state_at_snapshot.revision, false)];
        let final_revision = store.append(&next).unwrap_or_else(|_| unreachable!());
        store
            .prune_before(&stream(), state_at_snapshot.revision)
            .unwrap_or_else(|_| unreachable!());
        let replayed = store.replay_stream(&stream(), &State::default());
        assert!(replayed.is_ok(), "{replayed:?}");
        assert_eq!(replayed.map(|state| state.revision), Ok(final_revision));
    }

    // r[verify kiln.sqlite.profile.scenario.conflict]
    // r[verify kiln.sqlite.nonclaims.scenario.scope]
    #[test]
    fn unsafe_profile_is_rejected_without_durability_claim() {
        let mut profile = profile();
        profile.destructive_repair = true;
        profile.backup_required = false;
        assert!(admit_profile(&profile).is_err());
        assert_eq!(rusqlite::version(), SQLITE_RUNTIME_VERSION);
    }
}

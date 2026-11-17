//! Read-only canonical implementation-work lookup for operator projections.
use std::collections::BTreeMap;

use kiln_core::implementation::campaign::CampaignState;
use kiln_core::implementation::{AttemptLedger, WorkLedger};
use kiln_core::inventory::WorkNeedsAttachment;
use kiln_core::types::ContentId;
use kiln_provider_api::inventory::{InventoryReadFailure, InventoryWorkPort};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::de::DeserializeOwned;
use std::path::Path;

use crate::native_publication::NativeCairnLifecycleReceipt;

const MAX_OBSERVATION_RECORD_BYTES: i64 = 16 * 1024 * 1024;
/// Native lifecycle receipts one snapshot reads before reporting truncation.
pub const MAX_SNAPSHOT_LIFECYCLE_RECEIPTS: usize = 4_096;
/// Total campaign bytes one snapshot decodes before reporting truncation.
pub const MAX_SNAPSHOT_CAMPAIGN_BYTES: i64 = 64 * 1024 * 1024;
const LIFECYCLE_PREFIX: &str = "native-git-lifecycle:";

fn record(
    transaction: &rusqlite::Transaction<'_>,
    key: &str,
) -> Result<Option<Vec<u8>>, InventoryReadFailure> {
    let row: Option<(i64, Option<Vec<u8>>)> = transaction.query_row(
        "SELECT length(bytes), CASE WHEN length(bytes) <= ?2 THEN bytes ELSE NULL END FROM implementation_state WHERE kind = ?1",
        params![key, MAX_OBSERVATION_RECORD_BYTES], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|_| InventoryReadFailure::Unavailable)?;
    match row {
        Some((size, _)) if !(0..=MAX_OBSERVATION_RECORD_BYTES).contains(&size) => {
            Err(InventoryReadFailure::MalformedObservation)
        }
        Some((_, bytes)) => bytes
            .map(Some)
            .ok_or(InventoryReadFailure::MalformedObservation),
        None => Ok(None),
    }
}

/// Opens an existing canonical store read-only. No schema creation, immediate
/// transaction, claim, inventory collection or resource reservation is possible.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct CurrentWorkObservation {
    pub identity: ContentId,
    pub revision: u64,
}

pub struct ImplementationObservation {
    connection: Connection,
}
impl std::fmt::Debug for ImplementationObservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImplementationObservation")
            .finish_non_exhaustive()
    }
}
impl ImplementationObservation {
    pub fn open(path: &Path) -> Result<Self, InventoryReadFailure> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| InventoryReadFailure::Unavailable)?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(Self { connection })
    }
    pub fn current_work(
        &self,
        identity: &ContentId,
    ) -> Result<CurrentWorkObservation, InventoryReadFailure> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        let bytes =
            record(&transaction, crate::WORK_KIND)?.ok_or(InventoryReadFailure::StaleWork)?;
        let ledger: WorkLedger = serde_json::from_slice(&bytes)
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let current = ledger
            .current()
            .find(|work| work.identity().is_ok_and(|current| &current == identity))
            .ok_or(InventoryReadFailure::StaleWork)?;
        let result = CurrentWorkObservation {
            identity: identity.clone(),
            revision: current.revision,
        };
        transaction
            .commit()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(result)
    }

    // r[impl kiln.cairn.workspace]
    /// One consistent read of every canonical record the Cairn workspace joins,
    /// within the default bounds.
    pub fn snapshot(&self) -> Result<CanonicalSnapshot, InventoryReadFailure> {
        self.snapshot_bounded(SnapshotLimits::DEFAULT)
    }

    /// [`Self::snapshot`] with explicit bounds. Truncation is reported, never
    /// hidden; a missing table is no records, a malformed record an error.
    pub fn snapshot_bounded(
        &self,
        limits: SnapshotLimits,
    ) -> Result<CanonicalSnapshot, InventoryReadFailure> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        let mut snapshot = CanonicalSnapshot::default();
        if table_exists(&transaction, "implementation_state")? {
            snapshot.work = record(&transaction, crate::WORK_KIND)?
                .map(|bytes| decode(&bytes))
                .transpose()?;
            snapshot.attempts = record(&transaction, crate::ATTEMPT_KIND)?
                .map(|bytes| decode(&bytes))
                .transpose()?;
            let (lifecycle, truncated) = lifecycle_receipts(&transaction, limits)?;
            snapshot.lifecycle = lifecycle;
            snapshot.lifecycle_truncated = truncated;
        }
        if table_exists(&transaction, "implementation_campaign")? {
            let (campaigns, truncated) = campaigns(&transaction, limits)?;
            snapshot.campaigns = campaigns;
            snapshot.campaigns_truncated = truncated;
        }
        transaction
            .commit()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(snapshot)
    }
}

/// Read bounds for one [`CanonicalSnapshot`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    pub lifecycle_receipts: usize,
    pub campaign_bytes: i64,
}

impl SnapshotLimits {
    pub const DEFAULT: Self = Self {
        lifecycle_receipts: MAX_SNAPSHOT_LIFECYCLE_RECEIPTS,
        campaign_bytes: MAX_SNAPSHOT_CAMPAIGN_BYTES,
    };
}

/// One consistent read of canonical work, attempts, native lifecycle receipts
/// and selected campaigns. Absent records are `None` or empty, never invented.
#[derive(Clone, Debug, Default)]
pub struct CanonicalSnapshot {
    /// Absent when no work was ever admitted.
    pub work: Option<WorkLedger>,
    /// Absent when no attempt was ever claimed.
    pub attempts: Option<AttemptLedger>,
    /// Native Cairn lifecycle receipts keyed by attempt identity.
    pub lifecycle: BTreeMap<String, NativeCairnLifecycleReceipt>,
    pub lifecycle_truncated: bool,
    /// Selected campaigns in identity order with their store versions.
    pub campaigns: Vec<(CampaignState, u64)>,
    pub campaigns_truncated: bool,
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, InventoryReadFailure> {
    serde_json::from_slice(bytes).map_err(|_| InventoryReadFailure::MalformedObservation)
}

fn table_exists(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
) -> Result<bool, InventoryReadFailure> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            params![table],
            |row| row.get(0),
        )
        .map_err(|_| InventoryReadFailure::Unavailable)
}

type LifecycleReceipts = BTreeMap<String, NativeCairnLifecycleReceipt>;

fn lifecycle_receipts(
    transaction: &rusqlite::Transaction<'_>,
    limits: SnapshotLimits,
) -> Result<(LifecycleReceipts, bool), InventoryReadFailure> {
    let mut statement = transaction
        .prepare(
            "SELECT kind, length(bytes), CASE WHEN length(bytes) <= ?2 THEN bytes ELSE NULL END FROM implementation_state WHERE substr(kind, 1, length(?1)) = ?1 ORDER BY kind LIMIT ?3",
        )
        .map_err(|_| InventoryReadFailure::Unavailable)?;
    let fetch = i64::try_from(limits.lifecycle_receipts.saturating_add(1))
        .map_err(|_| InventoryReadFailure::Unavailable)?;
    let rows = statement
        .query_map(
            params![LIFECYCLE_PREFIX, MAX_OBSERVATION_RECORD_BYTES, fetch],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )
        .map_err(|_| InventoryReadFailure::Unavailable)?;
    let mut receipts = BTreeMap::new();
    let mut truncated = false;
    for row in rows {
        let (kind, length, bytes) = row.map_err(|_| InventoryReadFailure::Unavailable)?;
        if receipts.len() == limits.lifecycle_receipts {
            truncated = true;
            break;
        }
        let bytes = bytes
            .filter(|_| (0..=MAX_OBSERVATION_RECORD_BYTES).contains(&length))
            .ok_or(InventoryReadFailure::MalformedObservation)?;
        let attempt = kind
            .strip_prefix(LIFECYCLE_PREFIX)
            .ok_or(InventoryReadFailure::MalformedObservation)?;
        // The store retains `None` until a receipt is observed.
        let Some(receipt) = decode::<Option<NativeCairnLifecycleReceipt>>(&bytes)? else {
            continue;
        };
        if receipt.attempt.as_str() != attempt {
            return Err(InventoryReadFailure::MalformedObservation);
        }
        receipts.insert(attempt.to_owned(), receipt);
    }
    Ok((receipts, truncated))
}

fn campaigns(
    transaction: &rusqlite::Transaction<'_>,
    limits: SnapshotLimits,
) -> Result<(Vec<(CampaignState, u64)>, bool), InventoryReadFailure> {
    let mut statement = transaction
        .prepare(
            "SELECT campaign_id, version, length(bytes), CASE WHEN length(bytes) <= ?1 THEN bytes ELSE NULL END FROM implementation_campaign ORDER BY campaign_id",
        )
        .map_err(|_| InventoryReadFailure::Unavailable)?;
    let rows = statement
        .query_map(params![crate::campaign::MAX_CAMPAIGN_BYTES], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })
        .map_err(|_| InventoryReadFailure::Unavailable)?;
    let mut read = Vec::new();
    let mut total: i64 = 0;
    for row in rows {
        let (campaign_id, version, length, bytes) =
            row.map_err(|_| InventoryReadFailure::Unavailable)?;
        let bytes = bytes
            .filter(|_| (0..=crate::campaign::MAX_CAMPAIGN_BYTES).contains(&length))
            .ok_or(InventoryReadFailure::MalformedObservation)?;
        total = total.saturating_add(length);
        if total > limits.campaign_bytes {
            return Ok((read, true));
        }
        let state: CampaignState = decode(&bytes)?;
        let version =
            u64::try_from(version).map_err(|_| InventoryReadFailure::MalformedObservation)?;
        if state.contract.campaign_id != campaign_id {
            return Err(InventoryReadFailure::MalformedObservation);
        }
        read.push((state, version));
    }
    Ok((read, false))
}

impl InventoryWorkPort for ImplementationObservation {
    fn current_requirements(
        &self,
        identity: &ContentId,
    ) -> Result<Option<WorkNeedsAttachment>, InventoryReadFailure> {
        // One deferred read transaction freezes work and requirement rows at
        // the same snapshot without escalating a GET into a writer.
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        let bytes =
            record(&transaction, crate::WORK_KIND)?.ok_or(InventoryReadFailure::StaleWork)?;
        let ledger: WorkLedger = serde_json::from_slice(&bytes)
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let current = ledger
            .current()
            .find(|work| work.identity().is_ok_and(|id| &id == identity))
            .ok_or(InventoryReadFailure::StaleWork)?;
        let Some(policy_bytes) = record(&transaction, "fair-admission-v1")? else {
            return Ok(None);
        };
        let admission: kiln_core::fair_admission::AdmissionLedger =
            serde_json::from_slice(&policy_bytes)
                .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let Some(policy) = admission.policy else {
            return Ok(None);
        };
        let policy = policy
            .identity()
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        let Some(bytes) = record(
            &transaction,
            &format!("needs:{}:{}", identity.as_str(), policy.as_str()),
        )?
        else {
            let historical: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM implementation_state WHERE kind LIKE ?1)",
                    params![format!("needs:{}:%", identity.as_str())],
                    |row| row.get(0),
                )
                .map_err(|_| InventoryReadFailure::Unavailable)?;
            return if historical {
                Err(InventoryReadFailure::StaleWork)
            } else {
                Ok(None)
            };
        };
        let attachment: Option<WorkNeedsAttachment> = serde_json::from_slice(&bytes)
            .map_err(|_| InventoryReadFailure::MalformedObservation)?;
        if let Some(attachment) = &attachment {
            attachment
                .validate_for(identity, current.revision, &policy)
                .map_err(|_| InventoryReadFailure::StaleWork)?;
        }
        transaction
            .commit()
            .map_err(|_| InventoryReadFailure::Unavailable)?;
        Ok(attachment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::campaign::{
        CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, MemberBinding, MemberSelector,
        WorkflowProfile,
    };
    use kiln_core::implementation::{
        AcceptanceObligation, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork,
        SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use kiln_core::types::ExternalRef;
    use std::collections::BTreeSet;

    const REPOSITORY: &str = "onix/kiln";
    const LEASE_MS: u64 = 60_000;

    fn id(byte: char) -> ContentId {
        ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    fn cairn_work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".into(),
            repository: REPOSITORY.into(),
            project: "kiln".into(),
            base: None,
            title: "change-a: task-1".into(),
            requested_outcome: "implement".into(),
            acceptance: vec![AcceptanceObligation {
                id: "a1".into(),
                statement: "verified".into(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".into()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: id('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: ExternalRef::new("cairn", "task", "change-a/task-1")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "change-a/task-1".into(),
                version: id('1'),
            }],
        }
    }

    fn contract(campaign_id: &str) -> CampaignContract {
        CampaignContract {
            schema: CAMPAIGN_SCHEMA.into(),
            campaign_id: campaign_id.into(),
            revision: 1,
            repository: REPOSITORY.into(),
            members: vec![MemberBinding {
                selector: MemberSelector::WholeChange {
                    change: "change-a".into(),
                },
                profile: WorkflowProfile::SpecDriven,
                profile_version: id('a'),
                repository_instructions: id('a'),
                policy: id('a'),
                required_checks: Vec::new(),
                required_reviews: Vec::new(),
                integration_policy: id('a'),
                goal: CompletionGoal::VerifiedCandidate,
                prerequisite_inclusion: false,
                context: Vec::new(),
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 1,
            parent_budget_id: id('a'),
            agent_definition: id('a'),
            require_one_writer_per_repository: true,
        }
    }

    fn lifecycle_receipt(attempt: &ContentId) -> NativeCairnLifecycleReceipt {
        let commit =
            ExternalRef::new("git", "commit", &"c".repeat(40)).unwrap_or_else(|_| unreachable!());
        NativeCairnLifecycleReceipt {
            schema: crate::native_publication::NATIVE_CAIRN_LIFECYCLE_SCHEMA.into(),
            integration: id('5'),
            work_identity: id('6'),
            work_revision: 1,
            attempt: attempt.clone(),
            fence: 1,
            binding: crate::publication_effects::CairnLifecycleBinding {
                source: cairn_work()
                    .aliases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| unreachable!()),
                handoff: id('3'),
                verification: id('4'),
                authoritative_completion: id('7'),
                candidate: commit.clone(),
                action: crate::publication_effects::CairnLifecycleAction::ObserveReviewedArchive,
            },
            observed_tree: ExternalRef::new("git", "tree", &"d".repeat(40))
                .unwrap_or_else(|_| unreachable!()),
            lifecycle_policy: id('8'),
            lifecycle_receipt: ExternalRef::new("cairn", "receipt", &"e".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
        }
    }

    fn insert_state(path: &Path, kind: &str, bytes: &[u8]) {
        Connection::open(path)
            .and_then(|connection| {
                connection.execute(
                    "INSERT INTO implementation_state (kind, version, bytes) VALUES (?1, 1, ?2)",
                    params![kind, bytes],
                )
            })
            .unwrap_or_else(|error| unreachable!("{error}"));
    }

    #[test]
    fn read_only_open_never_creates_a_database() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("absent.sqlite");
        assert!(ImplementationObservation::open(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    // r[verify kiln.cairn.workspace]
    fn a_store_without_canonical_tables_snapshots_as_absent_records() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("other.sqlite");
        Connection::open(&path)
            .and_then(|connection| connection.execute_batch("CREATE TABLE unrelated (x INTEGER);"))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let snapshot = ImplementationObservation::open(&path)
            .and_then(|observation| observation.snapshot())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(snapshot.work.is_none() && snapshot.attempts.is_none());
        assert!(snapshot.lifecycle.is_empty() && snapshot.campaigns.is_empty());
        assert!(!snapshot.lifecycle_truncated && !snapshot.campaigns_truncated);
    }

    #[test]
    // r[verify kiln.cairn.workspace]
    fn one_snapshot_reads_work_attempts_receipts_and_campaigns() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("implementation.sqlite");
        let enrollment = Enrollment {
            repositories: BTreeSet::from([REPOSITORY.to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        };
        let mut implementer = crate::Implementer::open(&path, enrollment)
            .unwrap_or_else(|error| unreachable!("{error}"));
        implementer
            .admit(&cairn_work(), None)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let attempt = implementer
            .claim_next(0, LEASE_MS)
            .unwrap_or_else(|error| unreachable!("{error}"))
            .unwrap_or_else(|| unreachable!("claim"));
        drop(implementer);
        let receipt = lifecycle_receipt(&attempt.identity);
        insert_state(
            &path,
            &format!("{LIFECYCLE_PREFIX}{}", attempt.identity.as_str()),
            &serde_json::to_vec(&Some(&receipt)).unwrap_or_else(|_| unreachable!()),
        );
        // An unobserved receipt slot is retained as `null` and read as absent.
        insert_state(
            &path,
            &format!("{LIFECYCLE_PREFIX}{}", "0".repeat(64)),
            b"null",
        );
        let mut campaigns = crate::campaign::CampaignStore::open(&path)
            .unwrap_or_else(|error| unreachable!("{error}"));
        campaigns
            .start(contract("campaign-1"))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let snapshot = ImplementationObservation::open(&path)
            .and_then(|observation| observation.snapshot())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            snapshot
                .work
                .as_ref()
                .map(|ledger| ledger.current().count()),
            Some(1)
        );
        assert!(
            snapshot
                .attempts
                .as_ref()
                .is_some_and(|ledger| ledger.get(&attempt.identity).is_some())
        );
        assert_eq!(
            snapshot.lifecycle.get(attempt.identity.as_str()),
            Some(&receipt)
        );
        assert_eq!(snapshot.lifecycle.len(), 1);
        let [(state, version)] = snapshot.campaigns.as_slice() else {
            unreachable!("one campaign")
        };
        assert_eq!(
            (state.contract.campaign_id.as_str(), *version),
            ("campaign-1", 1)
        );
    }

    #[test]
    // r[verify kiln.cairn.workspace]
    fn bounds_report_truncation_and_crossed_rows_are_malformed() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("implementation.sqlite");
        crate::Implementer::open(
            &path,
            Enrollment {
                repositories: BTreeSet::from([REPOSITORY.to_string()]),
                actors: BTreeSet::from(["operator".to_string()]),
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        let mut campaigns = crate::campaign::CampaignStore::open(&path)
            .unwrap_or_else(|error| unreachable!("{error}"));
        for campaign_id in ["campaign-1", "campaign-2"] {
            campaigns
                .start(contract(campaign_id))
                .unwrap_or_else(|error| unreachable!("{error}"));
        }
        for attempt in ['1', '2'] {
            let attempt = id(attempt);
            insert_state(
                &path,
                &format!("{LIFECYCLE_PREFIX}{}", attempt.as_str()),
                &serde_json::to_vec(&Some(lifecycle_receipt(&attempt)))
                    .unwrap_or_else(|_| unreachable!()),
            );
        }
        let observation = ImplementationObservation::open(&path)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let one_campaign_bytes = i64::try_from(
            serde_json::to_vec(
                &campaigns
                    .inspect("campaign-1")
                    .unwrap_or_else(|error| unreachable!("{error}"))
                    .unwrap_or_else(|| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!())
            .len(),
        )
        .unwrap_or_else(|_| unreachable!());
        let bounded = observation
            .snapshot_bounded(SnapshotLimits {
                lifecycle_receipts: 1,
                campaign_bytes: one_campaign_bytes,
            })
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(bounded.lifecycle_truncated && bounded.campaigns_truncated);
        assert_eq!((bounded.lifecycle.len(), bounded.campaigns.len()), (1, 1));
        let complete = observation
            .snapshot()
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(!complete.lifecycle_truncated && !complete.campaigns_truncated);
        assert_eq!((complete.lifecycle.len(), complete.campaigns.len()), (2, 2));

        // A receipt filed under another attempt is malformed, not re-keyed.
        insert_state(
            &path,
            &format!("{LIFECYCLE_PREFIX}{}", id('3').as_str()),
            &serde_json::to_vec(&Some(lifecycle_receipt(&id('4'))))
                .unwrap_or_else(|_| unreachable!()),
        );
        assert_eq!(
            observation.snapshot().err(),
            Some(InventoryReadFailure::MalformedObservation)
        );
    }

    #[test]
    fn a_campaign_row_under_another_identity_is_malformed() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("implementation.sqlite");
        let mut campaigns = crate::campaign::CampaignStore::open(&path)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let state = campaigns
            .start(contract("campaign-1"))
            .unwrap_or_else(|error| unreachable!("{error}"));
        Connection::open(&path)
            .and_then(|connection| {
                connection.execute(
                    "INSERT INTO implementation_campaign (campaign_id, version, bytes) VALUES ('campaign-0', 1, ?1)",
                    params![serde_json::to_vec(&state).unwrap_or_else(|_| unreachable!())],
                )
            })
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            ImplementationObservation::open(&path)
                .and_then(|observation| observation.snapshot())
                .err(),
            Some(InventoryReadFailure::MalformedObservation)
        );
    }
}

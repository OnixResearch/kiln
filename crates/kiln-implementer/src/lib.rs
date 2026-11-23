//! Durable implementation work and attempt ledger.
//!
//! This shell owns the atomic claim, lease, fence and budget transitions that
//! `kiln-core` defines. It stores the work and attempt ledgers as versioned
//! JSON rows in SQLite and uses an immediate transaction for every mutation, so
//! two schedulers cannot both claim one work unit.
//!
//! Execution and cancellation remain Lattice/provider concerns. Dedicated
//! trusted acquisition/verification modules run frozen profiles; canonical
//! Engine admission and separately journaled provider effects retain authority.

pub mod admission;
mod authority_bridge;
pub mod authority_config;
pub mod authority_state;
pub mod campaign;
pub mod campaign_consumer;
pub mod campaign_operator;
pub mod fleet;
pub mod grant_issuance;
pub mod host_owner;
pub mod native_publication;
pub mod observation;
pub mod publication_effects;
pub mod queue;
pub mod verification;

pub use fleet::{FLEET_STATE_KIND, FLEET_STATE_SCHEMA, FleetState};
pub use queue::{DeliveryQueue, QueuedDelivery};

use std::path::{Path, PathBuf};
use std::time::Duration;

use kiln_core::implementation::authority::{
    AuthorityDecision, AuthorityRequest, AuthoritySelection, AuthoritySessionId,
    NO_EXPECTED_TARGET, ProtectedAuthority, ProtectedOperation,
};
use kiln_core::implementation::{
    AdmissionDecision, AttemptLedger, AttemptMetrics, AttemptUsage, CandidateHandoff,
    CandidateVerification, DispatchReservation, Enrollment, ImplementationAttempt,
    ImplementationDispatchPlan, ImplementationWork, IntegrationReceipt, PlanFeedback,
    PlanReviewCommand, PlanReviewResult, PublicationPlan, PublicationRecord, SourceMapping,
    SourceStatusPlan, SourceStatusReceipt, VerificationOutcome, WorkLedger, WorkerObservation,
};
use kiln_core::types::ContentId;
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

const WORK_KIND: &str = "work";
const ATTEMPT_KIND: &str = "attempts";
/// Recorded first launches, kept out of the `attempts` document so a binary
/// that re-saves attempts without knowing about launches cannot erase them.
const TRANSPORT_LAUNCH_KIND: &str = "transport_launches";
/// The request file each recorded launch sent, kept out of the launch
/// document so binaries that read launches without knowing about it still can.
const TRANSPORT_REQUEST_KIND: &str = "transport_requests";
const BUSY_TIMEOUT_MS: u64 = 10_000;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS implementation_state (
    kind TEXT PRIMARY KEY,
    version INTEGER NOT NULL,
    bytes BLOB NOT NULL
);";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovery {
    /// Attempts whose lease expired during this call.
    pub expired: Vec<ContentId>,
    /// Attempts still active and needing provider reconciliation.
    pub active: Vec<ContentId>,
}

pub struct Implementer {
    connection: Connection,
    database: PathBuf,
    enrollment: Enrollment,
    mirror: Option<Box<dyn MirrorObserver>>,
    overlap: Option<OverlapCheck>,
    authority: Option<Box<dyn ProtectedAuthority>>,
    authority_profile: AuthoritySelection,
    authority_last: Option<(AuthoritySessionId, AuthorityRequest)>,
    clock_ms: Option<u64>,
}

impl std::fmt::Debug for Implementer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Implementer")
            .field("enrollment", &self.enrollment)
            .finish_non_exhaustive()
    }
}

/// External overlap check for one candidate work item.
pub type OverlapCheck = Box<dyn Fn(&ImplementationWork) -> Option<String>>;

/// Optional mirror observer for attempt claim and closure transitions.
///
/// The observer runs after a committed transition. Its failure is not an
/// attempt failure: the swarm records an outcome and never blocks admission.
pub trait MirrorObserver {
    /// Reconcile the observed attempts at `now_ms`.
    ///
    /// # Errors
    /// Returns a diagnostic when the mirror journal cannot be read or written.
    fn reconcile(
        &mut self,
        snapshots: &[kiln_core::implementation::swarm::AttemptSnapshot],
        now_ms: u64,
    ) -> Result<(), String>;
}

/// One deferred claim pass: the claimed attempt, if any, and skipped candidates.
#[derive(Clone, Debug, Default)]
pub struct ClaimSelection {
    pub attempt: Option<ImplementationAttempt>,
    pub deferred: Vec<(String, String)>,
}

impl Implementer {
    /// Open or create the durable ledger. The caller owns single-writer
    /// placement; conditional claims still serialize through SQLite.
    pub fn open(path: &Path, enrollment: Enrollment) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(text)?;
        connection
            .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(text)?;
        connection.execute_batch(SCHEMA).map_err(text)?;
        Ok(Self {
            connection,
            database: path.to_path_buf(),
            enrollment,
            mirror: None,
            overlap: None,
            authority: None,
            authority_profile: AuthoritySelection::Absent,
            authority_last: None,
            clock_ms: None,
        })
    }

    /// Install one authority port with the profile selected for it. Shells
    /// compose through [`Self::select_authority`]; this seam exists for
    /// adapters and tests that wrap a composed port.
    pub fn set_authority(
        &mut self,
        authority: Box<dyn ProtectedAuthority>,
        selection: AuthoritySelection,
    ) {
        self.authority = Some(authority);
        self.authority_profile = selection;
        self.authority_last = None;
    }

    /// Select the explicit local-operator compatibility profile on this instance.
    ///
    /// # Errors
    /// Returns a message for an invalid operator identity.
    pub fn select_local_operator(&mut self, operator: impl Into<String>) -> Result<(), String> {
        // The local-operator profile loads no holder seed.
        self.select_authority(
            &authority_config::ImplementationAuthorityConfig::local_operator(operator),
            &std::collections::BTreeSet::new(),
        )
    }

    /// Open with the explicitly selected local-operator compatibility profile.
    ///
    /// This constructor exists for local operator shells and tests. It never
    /// authorizes projection-enabled routes and is not a fallback for a missing
    /// verified credential.
    ///
    /// # Errors
    /// Returns a message when the ledger cannot open or the operator is invalid.
    pub fn open_with_local_operator(
        path: &Path,
        enrollment: Enrollment,
        operator: impl Into<String>,
    ) -> Result<Self, String> {
        let mut implementer = Self::open(path, enrollment)?;
        implementer.select_local_operator(operator)?;
        Ok(implementer)
    }

    /// Open and select the authority profile resolved from a shell's explicit
    /// local operator or exported authority configuration; neither selects the
    /// absent profile, which denies every protected operation. Under the
    /// verified profile only the holder seeds of `roles`, the calling entry
    /// point's own holder roles, are loaded.
    ///
    /// # Errors
    /// Returns a message when the ledger cannot open or the selection is invalid.
    pub fn open_with_authority(
        path: &Path,
        enrollment: Enrollment,
        local_operator: Option<&str>,
        authority_config: Option<&Path>,
        roles: &std::collections::BTreeSet<kiln_core::implementation::grant::GrantRole>,
    ) -> Result<Self, String> {
        let config = authority_config::ImplementationAuthorityConfig::resolve(
            local_operator,
            authority_config,
        )?;
        let mut implementer = Self::open(path, enrollment)?;
        implementer.select_authority(&config, roles)?;
        Ok(implementer)
    }

    /// Persisted redacted decisions of the verified profile's durable store.
    ///
    /// # Errors
    /// Returns a message when the store cannot be read.
    pub fn persisted_authority_decisions(
        &self,
        range: std::ops::Range<u64>,
    ) -> Result<Vec<(u64, AuthorityDecision)>, String> {
        use authority_state::AuthorityStateStore as _;
        authority_state::SqliteAuthorityState::open(&authority_state::authority_state_path(
            &self.database,
        ))?
        .decisions(range)
    }

    /// Fix the shell clock for deterministic replay and fixtures.
    ///
    /// Production callers leave this unset and use the wall clock. A fixed
    /// clock never creates authority; it only makes currentness observations
    /// reproducible for a given ledger.
    pub fn set_clock_ms(&mut self, now_ms: u64) {
        self.clock_ms = Some(now_ms);
    }

    /// Shell time for effect boundaries without an explicit host time.
    pub(crate) fn now_ms(&self) -> u64 {
        self.clock_ms.unwrap_or_else(current_unix_ms)
    }

    /// Bounded redacted decisions retained by the configured adapter.
    #[must_use]
    pub fn authority_decisions(&self) -> &[AuthorityDecision] {
        self.authority
            .as_ref()
            .map_or(&[], |authority| authority.decisions())
    }

    /// Admit one source observation through the pure work ledger.
    pub fn admit(
        &mut self,
        work: &ImplementationWork,
        mapping: Option<&SourceMapping>,
    ) -> Result<AdmissionDecision, String> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(text)?
            .as_millis();
        let now_ms = u64::try_from(now_ms).map_err(text)?;
        self.admit_with_supersession(
            work,
            mapping,
            kiln_core::implementation::SupersessionRule::AbortActive,
            now_ms,
        )
    }

    /// Admit a source observation and apply its configured supersession rule in
    /// the same transaction. The old attempt's contract is never rewritten.
    // r[impl kiln.implementation.current_authority]
    pub fn admit_with_supersession(
        &mut self,
        work: &ImplementationWork,
        mapping: Option<&SourceMapping>,
        rule: kiln_core::implementation::SupersessionRule,
        now_ms: u64,
    ) -> Result<AdmissionDecision, String> {
        let enrollment = self.enrollment.clone();
        self.with_ledgers(|ledger, attempts| {
            let decision = ledger.admit(work, mapping, &enrollment)?;
            if matches!(decision, AdmissionDecision::Superseded { .. }) {
                attempts.supersede(&work.canonical_key()?, work.revision, rule, now_ms)?;
            }
            Ok(decision)
        })
    }

    /// Transactional currentness observation, not a physical resource grant.
    /// A caller must separately reserve its effect before external transport.
    pub fn authorize_current(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<ImplementationAttempt, String> {
        self.with_ledgers(|_, attempts| {
            if !attempts.is_current(identity, fence, now_ms) {
                return Err(kiln_core::types::Diagnostic::new(
                    "implementation_fence_stale",
                    "attempt.fence",
                    "attempt does not own the current unexpired claim",
                ));
            }
            attempts.get(identity).cloned().ok_or_else(|| {
                kiln_core::types::Diagnostic::new(
                    "implementation_attempt_missing",
                    "attempt.identity",
                    "attempt is not retained",
                )
            })
        })
    }

    /// Resolve the frozen contract for one current active unexpired claim.
    /// Historical work is available only for an explicitly retained FinishActive
    /// supersession; this never authorizes publication of superseded results.
    // r[impl kiln.implementation.current_authority]
    pub fn active_work(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<ImplementationWork, String> {
        self.with_ledgers(|work, attempts| {
            let refusal = || kiln_core::types::Diagnostic::new(
                "implementation_frozen_work_stale", "attempt.work",
                "frozen work requires a current active claim and explicit finish-active lineage",
            );
            if !attempts.is_current(identity, fence, now_ms) { return Err(refusal()); }
            let attempt = attempts.get(identity).ok_or_else(refusal)?;
            let current = work.get(&attempt.canonical_key).ok_or_else(refusal)?;
            if current.identity()? == attempt.work_identity && current.revision == attempt.work_revision {
                return Ok(current.clone());
            }
            if attempt.superseded_by != Some(current.revision) || current.revision <= attempt.work_revision {
                return Err(refusal());
            }
            let mut frozen = work.superseded().iter().filter(|retained| {
                retained.revision == attempt.work_revision && retained.base == attempt.base
                    && retained.identity().is_ok_and(|id| id == attempt.work_identity)
                    && retained.canonical_key().is_ok_and(|key| key == attempt.canonical_key)
            });
            let retained = frozen.next().ok_or_else(refusal)?;
            if frozen.next().is_some() { return Err(refusal()); }
            Ok(retained.clone())
        })
    }

    /// Record an independently observed provider termination; never infer it
    /// from a timeout, expired lease, failed cancellation or missing process.
    pub fn reconcile_unknown(
        &mut self,
        identity: &ContentId,
        fence: u64,
        termination: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.reconcile_unknown(identity, fence, termination))
    }

    /// Retain immutable operator-admitted resource/profile requirements for an
    /// exact current work version. Readiness never grants provider resources.
    pub fn attach_work_needs(
        &mut self,
        attachment: &kiln_core::inventory::WorkNeedsAttachment,
    ) -> Result<ContentId, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (admission, _) =
            load::<kiln_core::fair_admission::AdmissionLedger>(&transaction, "fair-admission-v1")?;
        let policy = admission
            .policy
            .as_ref()
            .ok_or("requirements need a current admitted policy")?
            .identity()
            .map_err(diagnostic)?;
        let current = work
            .current()
            .find(|work| {
                work.identity()
                    .is_ok_and(|id| id == attachment.work_identity)
                    && work.revision == attachment.work_revision
            })
            .ok_or("requirements do not bind current admitted work")?;
        attachment
            .validate_for(
                &current.identity().map_err(diagnostic)?,
                current.revision,
                &policy,
            )
            .map_err(text)?;
        let identity = attachment.identity().map_err(text)?;
        let key = format!(
            "needs:{}:{}",
            attachment.work_identity.as_str(),
            policy.as_str()
        );
        let (existing, version) =
            load::<Option<kiln_core::inventory::WorkNeedsAttachment>>(&transaction, &key)?;
        if existing
            .as_ref()
            .is_some_and(|existing| existing != attachment)
        {
            return Err("work version already has different admitted requirements".to_string());
        }
        if existing.is_none() {
            save(&transaction, &key, version, &Some(attachment))?;
        }
        transaction.commit().map_err(text)?;
        Ok(identity)
    }

    /// Return only requirements matching the current work and caller-selected
    /// policy. Historical attachments remain auditable but cannot explain new work.
    pub fn work_needs(
        &mut self,
        work_identity: &ContentId,
        policy_identity: &ContentId,
    ) -> Result<Option<kiln_core::inventory::WorkNeedsAttachment>, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (admission, _) =
            load::<kiln_core::fair_admission::AdmissionLedger>(&transaction, "fair-admission-v1")?;
        let policy = admission
            .policy
            .as_ref()
            .ok_or("requirements need a current admitted policy")?
            .identity()
            .map_err(diagnostic)?;
        if &policy != policy_identity {
            return Err("requirements policy is no longer current".into());
        }
        let current = work
            .current()
            .find(|work| work.identity().is_ok_and(|id| &id == work_identity))
            .ok_or("work identity is not current")?;
        let key = format!("needs:{}:{}", work_identity.as_str(), policy.as_str());
        let (attachment, _) =
            load::<Option<kiln_core::inventory::WorkNeedsAttachment>>(&transaction, &key)?;
        if let Some(attachment) = &attachment {
            attachment
                .validate_for(work_identity, current.revision, policy_identity)
                .map_err(text)?;
        }
        transaction.commit().map_err(text)?;
        Ok(attachment)
    }

    /// Persist the transport intent and, for a fresh start, its launch in the
    /// same transaction. `Recover` after a possible prior dispatch carries the
    /// recorded launch; the caller MUST recover with it instead of dispatching.
    /// The launch sends no frozen request of its own.
    pub fn begin_dispatch_transport(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ContentId,
        launch: Option<&kiln_core::implementation::TransportLaunch>,
    ) -> Result<kiln_core::implementation::TransportStart, String> {
        self.begin_dispatch_transport_sending(identity, fence, now_ms, plan, launch, None)
    }

    /// [`Self::begin_dispatch_transport`] for a launch that sends `request`:
    /// a fresh start records it for that launch in the same transaction as
    /// the intent and launch, under its own implementation-state kind.
    pub fn begin_dispatch_transport_sending(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ContentId,
        launch: Option<&kiln_core::implementation::TransportLaunch>,
        request: Option<&kiln_core::implementation::FrozenRequest>,
    ) -> Result<kiln_core::implementation::TransportStart, String> {
        self.with_launches(|attempts, launches, requests| {
            let previously_started =
                attempts.begin_dispatch_transport(identity, fence, now_ms, plan)?;
            let start = launches.start(identity, previously_started, launch)?;
            requests.record(identity, &start, launch, request);
            Ok(start)
        })
    }

    /// The launch recorded for an attempt, if any. It directs recovery only
    /// while the attempt's transport intent exists.
    pub fn dispatch_launch(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<kiln_core::implementation::TransportLaunch>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (launches, _) = load::<kiln_core::implementation::TransportLaunches>(
            &transaction,
            TRANSPORT_LAUNCH_KIND,
        )?;
        transaction.commit().map_err(text)?;
        Ok(launches.recall(identity))
    }

    /// The request file the attempt's recorded launch sent, only while its
    /// transport intent and that exact launch are recorded. `None` for a
    /// launch an earlier binary recorded: it sent the admitted request.
    pub fn dispatch_request(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<kiln_core::implementation::FrozenRequest>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (launches, _) = load::<kiln_core::implementation::TransportLaunches>(
            &transaction,
            TRANSPORT_LAUNCH_KIND,
        )?;
        let (requests, _) = load::<kiln_core::implementation::TransportRequests>(
            &transaction,
            TRANSPORT_REQUEST_KIND,
        )?;
        transaction.commit().map_err(text)?;
        Ok(requests.recall(
            identity,
            launches.recall(identity).as_ref(),
            attempts.dispatch_requires_recovery(identity),
        ))
    }

    /// Install or remove the optional mirror observer.
    pub fn set_mirror(&mut self, mirror: Option<Box<dyn MirrorObserver>>) {
        self.mirror = mirror;
    }

    /// Install or remove the optional swarm overlap check.
    ///
    /// The check runs before an exact campaign claim and returns a cause when an
    /// open external claim overlaps the work. It cannot approve or widen work.
    pub fn set_overlap(&mut self, overlap: Option<OverlapCheck>) {
        self.overlap = overlap;
    }

    fn mirror_reconcile(&mut self, now_ms: u64) {
        if self.mirror.is_none() {
            return;
        }
        let Ok(snapshots) = self.swarm_snapshots() else {
            return;
        };
        if let Some(observer) = self.mirror.as_mut() {
            let _ = observer.reconcile(&snapshots, now_ms);
        }
    }

    /// Claim the highest-ranked eligible work unit that is not already claimed.
    ///
    /// A second scheduler receives `None` when no other eligible work remains.
    // r[impl kiln.implementation.durable_store]
    pub fn claim_next(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<ImplementationAttempt>, String> {
        Ok(self
            .claim_next_deferred(now_ms, lease_ms, &|_| None)?
            .attempt)
    }

    /// Claim the next eligible work unit, deferring candidates the caller names.
    ///
    /// A deferred candidate keeps its cause for operator output and does not
    /// consume budget or a lease. Everything else matches [`Self::claim_next`].
    /// The new claim is admitted as an exact `ClaimAcquire` invocation inside
    /// the claim transaction; a denial commits nothing.
    ///
    /// # Errors
    /// Returns the ledger diagnostic when admission fails for a reason other
    /// than another claimant, exhausted budget, prior verification or recovery,
    /// or the redacted authority refusal for the selected claim.
    pub fn claim_next_deferred(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
        defer: &dyn Fn(&ImplementationWork) -> Option<String>,
    ) -> Result<ClaimSelection, String> {
        let lease = lease_ms.to_string();
        let selection = self.with_claim_ledgers(|ledger, attempts, governed, authority, profile| {
            attempts.expire(now_ms)?;
            let mut deferred = Vec::new();
            for row in ledger.ranked() {
                let Some(work) = ledger
                    .current()
                    .find(|work| {
                        work.identity()
                            .is_ok_and(|identity| identity == row.identity)
                    })
                    .cloned()
                else {
                    continue;
                };
                if let Some(cause) = defer(&work) {
                    let identity = work
                        .identity()
                        .map(|identity| identity.as_str().to_owned())
                        .unwrap_or_default();
                    deferred.push((identity, cause));
                    continue;
                }
                // r[impl kiln.admission.host_owner]
                // A host-owner policy counts only claims made through admission.
                if governed {
                    return Err(kiln_core::types::Diagnostic::new(
                        "host_owner_admission_required",
                        "attempt.claim",
                        "a host-owner admission policy governs this store; claim through admission",
                    ));
                }
                match attempts.claim(&work, now_ms, lease_ms) {
                    Ok(attempt) => {
                        admission::authorize_claim(
                            ledger,
                            authority,
                            profile,
                            Some(&attempt),
                            &[&lease],
                            now_ms,
                        )?;
                        return Ok(ClaimSelection {
                            attempt: Some(attempt),
                            deferred,
                        });
                    }
                    Err(error)
                        if matches!(
                            error.code.as_str(),
                            "implementation_already_claimed"
                                | "implementation_budget_exhausted"
                                | "implementation_already_verified"
                                | "implementation_reconciliation_required"
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(ClaimSelection {
                attempt: None,
                deferred,
            })
        })?;
        if selection.attempt.is_some() {
            self.mirror_reconcile(now_ms);
        }
        Ok(selection)
    }

    /// Return every attempt with the work facts a swarm mirror needs.
    ///
    /// # Errors
    /// Returns the ledger diagnostic when the store cannot be read.
    pub fn swarm_snapshots(
        &mut self,
    ) -> Result<Vec<kiln_core::implementation::swarm::AttemptSnapshot>, String> {
        self.with_ledgers(|ledger, attempts| {
            let mut snapshots = Vec::new();
            for attempt in attempts.attempts() {
                let Some(work) = ledger.current().find(|work| {
                    work.identity()
                        .is_ok_and(|identity| identity == attempt.work_identity)
                }) else {
                    continue;
                };
                let scope = work
                    .permitted_paths
                    .first()
                    .cloned()
                    .unwrap_or_else(|| work.project.clone());
                let base_revision = attempt
                    .base
                    .as_ref()
                    .map(|base| base.native_id.as_str().to_owned())
                    .unwrap_or_default();
                let lease_ms = attempt.lease_until_ms.saturating_sub(attempt.claimed_at_ms);
                let lease_minutes = u32::try_from((lease_ms / 60_000).max(1)).unwrap_or(u32::MAX);
                snapshots.push(kiln_core::implementation::swarm::AttemptSnapshot {
                    identity: attempt.identity.as_str().to_owned(),
                    fence: attempt.fence,
                    terminal: !attempt.phase.is_active(),
                    repository: work.repository.clone(),
                    scope,
                    base_revision,
                    lease_minutes,
                });
            }
            Ok(snapshots)
        })
    }

    pub fn renew(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<u64, String> {
        let facts = self.authority_facts(identity, fence)?;
        let lease = lease_ms.to_string();
        let request = facts.request(
            ProtectedOperation::ClaimRenew,
            identity.as_str(),
            NO_EXPECTED_TARGET,
            &[&lease],
            None,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| attempts.renew(identity, fence, now_ms, lease_ms))
    }

    // r[impl kiln.implementation.authority.dispatch]
    /// Reserve one provider dispatch for a current attempt.
    ///
    /// The reservation's exact `Dispatch` request is admitted inside the
    /// transaction that writes it, after the attempt ledger's fence and phase
    /// checks and before anything is saved; a denial saves nothing.
    ///
    /// # Errors
    /// Returns the ledger refusal or the redacted authority denial.
    pub fn reserve_dispatch(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ContentId,
        provider_profile: &kiln_core::types::ProviderProfileId,
    ) -> Result<DispatchReservation, String> {
        self.with_dispatch_ledgers(now_ms, true, |work, attempts, gate| {
            let reservation = attempts
                .reserve_dispatch(identity, fence, now_ms, plan, provider_profile)
                .map_err(diagnostic)?;
            let attempt = attempts.get(identity).ok_or("authority: attempt missing")?;
            let work = work
                .get(&attempt.canonical_key)
                .ok_or("authority: attempt work missing")?;
            authorize_reservation(gate, work, attempt, &reservation)?;
            Ok(reservation)
        })
    }

    // r[impl kiln.implementation.authority.dispatch]
    /// Admit and reserve one bounded dispatch plan for a current attempt.
    ///
    /// Like [`Self::reserve_dispatch`], the exact `Dispatch` request is
    /// admitted inside the reserving transaction before anything is saved. A
    /// session admitted by [`Self::authorize_dispatch_plan`] for the same
    /// request is revalidated rather than granted twice.
    ///
    /// # Errors
    /// Returns the attachment, ledger or redacted authority refusal.
    pub fn reserve_dispatch_plan(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ImplementationDispatchPlan,
    ) -> Result<DispatchReservation, String> {
        if let Some(execution) = &plan.execution {
            let retained =
                self.work_needs(&plan.work_identity, &execution.requirements.policy_identity)?;
            if retained.as_ref() != Some(&execution.requirements) {
                return Err("dispatch requirements differ from current admitted attachment".into());
            }
        }
        self.with_dispatch_ledgers(now_ms, true, |_, attempts, gate| {
            let reservation = attempts
                .reserve_dispatch_plan(work, identity, fence, now_ms, plan)
                .map_err(diagnostic)?;
            let attempt = attempts.get(identity).ok_or("authority: attempt missing")?;
            authorize_reservation(gate, work, attempt, &reservation)?;
            Ok(reservation)
        })
    }

    // r[impl kiln.implementation.authority.dispatch]
    /// Admit the exact `Dispatch` request [`Self::reserve_dispatch_plan`]
    /// will make for `plan`, changing no attempt or reservation state.
    ///
    /// A shell that reads a credential before reserving calls this first, so
    /// an authority denial happens before the credential is read. The
    /// reservation then revalidates this session in its own transaction.
    ///
    /// # Errors
    /// Returns the ledger refusal the reservation would return or the
    /// redacted authority denial.
    pub fn authorize_dispatch_plan(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ImplementationDispatchPlan,
    ) -> Result<(), String> {
        self.with_dispatch_ledgers(now_ms, false, |work, attempts, gate| {
            let work = work
                .current()
                .chain(work.superseded().iter())
                .find(|work| {
                    work.identity()
                        .is_ok_and(|work_identity| work_identity == plan.work_identity)
                })
                .ok_or("authority: dispatch plan work is not retained")?;
            // The reservation is computed on this transaction's copy of the
            // ledger, which is never saved.
            let reservation = attempts
                .reserve_dispatch_plan(work, identity, fence, now_ms, plan)
                .map_err(diagnostic)?;
            let attempt = attempts.get(identity).ok_or("authority: attempt missing")?;
            authorize_reservation(gate, work, attempt, &reservation)
        })
    }

    pub fn dispatch_plan(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<ImplementationDispatchPlan>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.dispatch_plan(identity).cloned()))
    }

    pub fn dispatch_requires_recovery(&mut self, identity: &ContentId) -> Result<bool, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.dispatch_requires_recovery(identity)))
    }

    pub fn dispatch_reservation(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<DispatchReservation>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.dispatch_reservation(identity).cloned()))
    }

    // r[impl kiln.implementation.authority.dispatch]
    /// Record the started execution of a reserved dispatch: the durable
    /// launch record.
    ///
    /// The exact `Dispatch` request for the reservation's effect and this
    /// workflow is admitted inside the recording transaction, after the
    /// attempt ledger's checks and before anything is saved; a denial records
    /// nothing.
    ///
    /// # Errors
    /// Returns the ledger refusal or the redacted authority denial.
    pub fn start_execution(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        workflow: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        self.with_dispatch_ledgers(now_ms, true, |work, attempts, gate| {
            attempts
                .start_execution(identity, fence, now_ms, workflow)
                .map_err(diagnostic)?;
            let attempt = attempts.get(identity).ok_or("authority: attempt missing")?;
            let work = work
                .get(&attempt.canonical_key)
                .ok_or("authority: attempt work missing")?;
            let plan = attempt
                .dispatch
                .as_ref()
                .map(|dispatch| dispatch.plan.as_str())
                .unwrap_or_default();
            authority_bridge::authorize_dispatch(
                gate,
                work,
                attempt,
                &[
                    plan,
                    workflow.provider_namespace.as_str(),
                    workflow.native_kind.as_str(),
                    workflow.native_id.as_str(),
                ],
            )
        })
    }

    pub fn reconcile_dispatch(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        observed: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        // r[impl kiln.implementation.authority.reconcile]
        // Adopting an observed run settles the original dispatch effect: a
        // fresh invocation (new nonce) bound to the same reservation effect.
        let facts = self.authority_facts(identity, fence)?;
        let effect = facts
            .attempt
            .dispatch
            .as_ref()
            .map(|dispatch| dispatch.effect_id.as_str().to_string())
            .ok_or("dispatch reconciliation requires a durable reservation")?;
        let request = facts.request(
            ProtectedOperation::Dispatch,
            &effect,
            NO_EXPECTED_TARGET,
            &[
                observed.provider_namespace.as_str(),
                observed.native_kind.as_str(),
                observed.native_id.as_str(),
            ],
            None,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| {
            attempts.reconcile_dispatch(identity, fence, now_ms, observed)
        })
    }

    /// End an attempt whose provider states that no worker effect of it can
    /// have started; see `AttemptLedger::settle_dispatch_without_execution`.
    pub fn settle_dispatch_without_execution(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> Result<(), String> {
        let result = self.with_launches(|attempts, launches, requests| {
            attempts.settle_dispatch_without_execution(identity, fence, now_ms, cause)?;
            launches.remove(identity);
            requests.remove(identity);
            Ok(())
        });
        if result.is_ok() {
            self.mirror_reconcile(now_ms);
        }
        result
    }

    pub fn observe_metrics(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        usage: AttemptUsage,
        traces: Vec<kiln_core::types::ExternalRef>,
    ) -> Result<AttemptMetrics, String> {
        self.with_ledgers(|_, attempts| {
            attempts.observe_metrics(identity, fence, now_ms, usage, traces)
        })
    }

    pub fn record_metrics(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        usage: AttemptUsage,
        traces: Vec<kiln_core::types::ExternalRef>,
    ) -> Result<AttemptMetrics, String> {
        self.with_ledgers(|_, attempts| {
            attempts.record_metrics(identity, fence, now_ms, usage, traces)
        })
    }

    pub fn metrics(&mut self, identity: &ContentId) -> Result<Option<AttemptMetrics>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.metrics(identity).cloned()))
    }

    /// Observe a claim-time window without rewriting ledgers or row versions.
    pub fn metrics_report(
        &mut self,
        from_ms: u64,
        to_ms: u64,
    ) -> Result<kiln_core::implementation::AttemptReport, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let report = kiln_core::implementation::AttemptReport::observe(&attempts, from_ms, to_ms)
            .map_err(diagnostic)?;
        transaction.commit().map_err(text)?;
        Ok(report)
    }

    /// Retain exact-fence worker metadata atomically, without granting authority.
    pub fn record_worker_observation(
        &mut self,
        identity: &ContentId,
        fence: u64,
        observation: &WorkerObservation,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| {
            attempts.record_worker_observation(identity, fence, observation)
        })
    }

    /// Retain an independent verifier observation, not accepted CI evidence.
    pub fn record_verification_outcome(
        &mut self,
        identity: &ContentId,
        fence: u64,
        outcome: VerificationOutcome,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| {
            attempts.record_verification_outcome(identity, fence, outcome)
        })
    }

    pub fn admit_candidate_handoff(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        handoff: &CandidateHandoff,
    ) -> Result<ContentId, String> {
        let facts = self.authority_facts(identity, fence)?;
        let handoff_identity =
            kiln_core::implementation::admit_candidate_handoff(work, &facts.attempt, handoff)
                .map_err(diagnostic)?;
        let request = facts.request(
            ProtectedOperation::CandidateSubmit,
            handoff_identity.as_str(),
            NO_EXPECTED_TARGET,
            &[handoff_identity.as_str()],
            Some(&handoff.candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| {
            attempts.admit_candidate_handoff(work, identity, fence, now_ms, handoff)
        })
    }

    // The preparation boundary intentionally names every exact input; bundling
    // them into a DTO would only move the same fields behind one name.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_publication(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        handoff: &CandidateHandoff,
        verification: &CandidateVerification,
        plan: &PublicationPlan,
    ) -> Result<ContentId, String> {
        let facts = self.authority_facts(identity, fence)?;
        let expected = format!("refs/heads/{}@absent", plan.branch);
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            plan.handoff.as_str(),
            &expected,
            &[
                plan.handoff.as_str(),
                plan.verification.as_str(),
                plan.base_branch.as_str(),
                plan.branch.as_str(),
                plan.title.as_str(),
                plan.body.as_str(),
            ],
            Some(&plan.candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| {
            attempts.prepare_publication(work, identity, fence, handoff, verification, plan)
        })
    }

    pub fn publication_record(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<PublicationRecord>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.publication_record(identity).cloned()))
    }

    pub fn record_integration(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        receipt: &IntegrationReceipt,
    ) -> Result<ContentId, String> {
        let facts = self.authority_facts(identity, fence)?;
        let expected = format!(
            "{}@{}",
            receipt.base_ref,
            receipt.merge_commit.native_id.as_str()
        );
        let request = facts.request(
            ProtectedOperation::IntegrationLand,
            receipt.pull_request.native_id.as_str(),
            &expected,
            &[
                receipt.handoff.as_str(),
                receipt.verification.as_str(),
                receipt.pull_request.provider_namespace.as_str(),
                receipt.pull_request.native_id.as_str(),
                receipt.merge_commit.native_id.as_str(),
                receipt.base_ref.as_str(),
            ],
            Some(&receipt.candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| attempts.record_integration(work, identity, fence, receipt))
    }

    pub fn integration_receipt(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<IntegrationReceipt>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.integration_receipt(identity).cloned()))
    }

    pub fn record_source_status(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &SourceStatusPlan,
        receipt: &SourceStatusReceipt,
    ) -> Result<ContentId, String> {
        let facts = self.authority_facts(identity, fence)?;
        let candidate = facts
            .attempt
            .publication
            .as_ref()
            .map(|publication| &publication.plan.candidate)
            .or(facts.attempt.candidate.as_ref());
        let observed = receipt.observed_revision.to_string();
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            plan.source.native_id.as_str(),
            &observed,
            &[
                plan.source.native_id.as_str(),
                receipt.state.as_str(),
                &observed,
            ],
            candidate,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| {
            attempts.record_source_status(work, identity, fence, plan, receipt)
        })
    }

    pub fn source_status_receipt(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<SourceStatusReceipt>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.source_status_receipt(identity).cloned()))
    }

    pub fn record_branch_published(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        remote_head: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        let facts = self.authority_facts(identity, fence)?;
        let candidate = facts
            .attempt
            .publication
            .as_ref()
            .map(|publication| &publication.plan.candidate)
            .or(facts.attempt.candidate.as_ref());
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            identity.as_str(),
            remote_head.native_id.as_str(),
            &[
                remote_head.provider_namespace.as_str(),
                remote_head.native_kind.as_str(),
                remote_head.native_id.as_str(),
            ],
            candidate,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| {
            attempts.record_branch_published(identity, fence, remote_head)
        })
    }

    pub fn record_pull_request_published(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        pull_request: &kiln_core::types::ExternalRef,
        head: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        let facts = self.authority_facts(identity, fence)?;
        let candidate = facts
            .attempt
            .publication
            .as_ref()
            .map(|publication| &publication.plan.candidate)
            .or(facts.attempt.candidate.as_ref());
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            pull_request.native_id.as_str(),
            head.native_id.as_str(),
            &[
                pull_request.provider_namespace.as_str(),
                pull_request.native_id.as_str(),
                head.native_id.as_str(),
            ],
            candidate,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        self.with_ledgers(|_, attempts| {
            attempts.record_pull_request_published(identity, fence, pull_request, head)
        })
    }

    pub fn record_verification(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        handoff: &CandidateHandoff,
        verification: &CandidateVerification,
    ) -> Result<ContentId, String> {
        let facts = self.authority_facts(identity, fence)?;
        let verification_identity = kiln_core::implementation::admit_candidate_verification(
            work,
            &facts.attempt,
            handoff,
            verification,
        )
        .map_err(diagnostic)?;
        let request = facts.request(
            ProtectedOperation::VerificationAdmit,
            verification_identity.as_str(),
            NO_EXPECTED_TARGET,
            &[verification_identity.as_str()],
            Some(&handoff.candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        let result = self.with_ledgers(|_, attempts| {
            attempts.record_verification(work, identity, fence, now_ms, handoff, verification)
        });
        if result.is_ok() {
            self.mirror_reconcile(now_ms);
        }
        result
    }

    pub fn verification_receipt(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<CandidateVerification>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.verification_receipt(identity).cloned()))
    }

    pub fn candidate_handoff(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<CandidateHandoff>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.candidate_handoff(identity).cloned()))
    }

    pub fn produce_candidate(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        candidate: &kiln_core::types::ExternalRef,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| {
            attempts.produce_candidate(identity, fence, now_ms, candidate)
        })
    }

    pub fn begin_verification(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        self.with_ledgers(|_, attempts| attempts.begin_verification(identity, fence, now_ms))
    }

    pub fn fail(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        class: kiln_core::implementation::FailureClass,
        cause: &str,
    ) -> Result<(), String> {
        let result =
            self.with_ledgers(|_, attempts| attempts.fail(identity, fence, now_ms, class, cause));
        if result.is_ok() {
            self.mirror_reconcile(now_ms);
        }
        result
    }

    pub fn block(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> Result<(), String> {
        let result =
            self.with_ledgers(|_, attempts| attempts.block(identity, fence, now_ms, cause));
        if result.is_ok() {
            self.mirror_reconcile(now_ms);
        }
        result
    }

    pub fn cancel(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> Result<(), String> {
        let facts = self.authority_facts(identity, fence)?;
        let request = facts.request(
            ProtectedOperation::AttemptCancel,
            identity.as_str(),
            NO_EXPECTED_TARGET,
            &[cause],
            None,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        let result = self.cancel_with_admission(identity, fence, now_ms, cause);
        if result.is_ok() {
            self.mirror_reconcile(now_ms);
        }
        result
    }

    pub fn supersede(
        &mut self,
        canonical_key: &ContentId,
        new_revision: u64,
        rule: kiln_core::implementation::SupersessionRule,
        now_ms: u64,
    ) -> Result<Option<ContentId>, String> {
        let result = self.with_ledgers(|_, attempts| {
            attempts.supersede(canonical_key, new_revision, rule, now_ms)
        });
        if result.is_ok() {
            self.mirror_reconcile(now_ms);
        }
        result
    }

    /// Expire ended leases and return the active/reconciliation set.
    // r[impl kiln.implementation.recovery]
    pub fn recover(&mut self, now_ms: u64) -> Result<Recovery, String> {
        self.with_ledgers(|_, attempts| {
            let expired = attempts.expire(now_ms)?;
            let active = attempts
                .attempts()
                .filter(|attempt| attempt.phase.is_active())
                .chain(attempts.unresolved())
                .map(|attempt| attempt.identity.clone())
                .collect();
            Ok(Recovery { expired, active })
        })
    }

    pub fn work(&mut self) -> Result<WorkLedger, String> {
        self.with_ledgers(|ledger, _| Ok(ledger.clone()))
    }

    pub fn attempts(&mut self) -> Result<AttemptLedger, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.clone()))
    }

    pub fn attempt(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<ImplementationAttempt>, String> {
        self.with_ledgers(|_, attempts| Ok(attempts.get(identity).cloned()))
    }

    /// Apply one plan-review command in the work-ledger transaction (#93).
    /// Acceptance only records a decision; a claim still needs ready work.
    // r[impl kiln.implementation.plan_review]
    pub fn plan_review(&mut self, command: &PlanReviewCommand) -> Result<PlanReviewResult, String> {
        self.with_ledgers(|ledger, _| ledger.apply_plan_review(command))
    }

    /// Feedback on the latest plan of one exact work version, for its author.
    pub fn plan_feedback(&mut self, work: &ContentId) -> Result<PlanFeedback, String> {
        self.with_ledgers(|ledger, _| ledger.plan_feedback(work))
    }

    fn with_ledgers<T>(
        &mut self,
        apply: impl FnOnce(
            &mut WorkLedger,
            &mut AttemptLedger,
        ) -> Result<T, kiln_core::types::Diagnostic>,
    ) -> Result<T, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut work, work_version) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let result = apply(&mut work, &mut attempts)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        save(&transaction, WORK_KIND, work_version, &work)?;
        save(&transaction, ATTEMPT_KIND, attempt_version, &attempts)?;
        transaction.commit().map_err(text)?;
        Ok(result)
    }

    /// `with_ledgers` plus whether a host-owner admission policy governs every
    /// claim, read in the same immediate transaction that writes the claim,
    /// and the selected authority that admits the claim before it commits.
    fn with_claim_ledgers<T>(
        &mut self,
        apply: impl FnOnce(
            &mut WorkLedger,
            &mut AttemptLedger,
            bool,
            &mut Option<Box<dyn ProtectedAuthority>>,
            &AuthoritySelection,
        ) -> Result<T, kiln_core::types::Diagnostic>,
    ) -> Result<T, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut work, work_version) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (admission, _) = load::<kiln_core::fair_admission::AdmissionLedger>(
            &transaction,
            admission::ADMISSION_KIND,
        )?;
        let governed = kiln_core::fair_admission::host_owner_governed(&admission)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        let result = apply(
            &mut work,
            &mut attempts,
            governed,
            &mut self.authority,
            &self.authority_profile,
        )
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
        save(&transaction, WORK_KIND, work_version, &work)?;
        save(&transaction, ATTEMPT_KIND, attempt_version, &attempts)?;
        transaction.commit().map_err(text)?;
        Ok(result)
    }

    /// `with_ledgers` plus a [`authority_bridge::DispatchGate`] over the
    /// selected authority and the recorded launches, read in the same
    /// transaction. `persist` saves the ledgers after `apply` succeeds; an
    /// error from `apply`, including an authority denial, saves nothing.
    /// Without `persist` the transaction is read-only and its ledger copies
    /// are discarded.
    fn with_dispatch_ledgers<T>(
        &mut self,
        now_ms: u64,
        persist: bool,
        apply: impl FnOnce(
            &mut WorkLedger,
            &mut AttemptLedger,
            &mut authority_bridge::DispatchGate<'_>,
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        let behavior = if persist {
            TransactionBehavior::Immediate
        } else {
            TransactionBehavior::Deferred
        };
        let transaction = self
            .connection
            .transaction_with_behavior(behavior)
            .map_err(text)?;
        let (mut work, work_version) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (launches, _) = load::<kiln_core::implementation::TransportLaunches>(
            &transaction,
            TRANSPORT_LAUNCH_KIND,
        )?;
        let mut gate = authority_bridge::DispatchGate {
            authority: &mut self.authority,
            profile: &self.authority_profile,
            last: &mut self.authority_last,
            launches: &launches,
            now_ms,
        };
        let result = apply(&mut work, &mut attempts, &mut gate)?;
        if persist {
            save(&transaction, WORK_KIND, work_version, &work)?;
            save(&transaction, ATTEMPT_KIND, attempt_version, &attempts)?;
        }
        transaction.commit().map_err(text)?;
        Ok(result)
    }

    /// Apply one transition to the attempt ledger, the launch records and the
    /// records of the requests those launches sent, in a single transaction.
    /// Only these transitions touch launch and request records; every other
    /// save of `attempts` leaves them as they are.
    fn with_launches<T>(
        &mut self,
        apply: impl FnOnce(
            &mut AttemptLedger,
            &mut kiln_core::implementation::TransportLaunches,
            &mut kiln_core::implementation::TransportRequests,
        ) -> Result<T, kiln_core::types::Diagnostic>,
    ) -> Result<T, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (mut launches, launch_version) = load::<kiln_core::implementation::TransportLaunches>(
            &transaction,
            TRANSPORT_LAUNCH_KIND,
        )?;
        let (mut requests, request_version) = load::<kiln_core::implementation::TransportRequests>(
            &transaction,
            TRANSPORT_REQUEST_KIND,
        )?;
        let result = apply(&mut attempts, &mut launches, &mut requests)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        save(&transaction, ATTEMPT_KIND, attempt_version, &attempts)?;
        save(
            &transaction,
            TRANSPORT_LAUNCH_KIND,
            launch_version,
            &launches,
        )?;
        save(
            &transaction,
            TRANSPORT_REQUEST_KIND,
            request_version,
            &requests,
        )?;
        transaction.commit().map_err(text)?;
        Ok(result)
    }
}

fn load<T: serde::de::DeserializeOwned + Default>(
    transaction: &Transaction<'_>,
    kind: &str,
) -> Result<(T, u64), String> {
    let row = transaction.query_row(
        "SELECT version, bytes FROM implementation_state WHERE kind = ?1",
        params![kind],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
    );
    match row {
        Ok((version, bytes)) => Ok((
            serde_json::from_slice(&bytes).map_err(text)?,
            u64::try_from(version).map_err(text)?,
        )),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok((T::default(), 0)),
        Err(error) => Err(text(error)),
    }
}

fn save<T: serde::Serialize>(
    transaction: &Transaction<'_>,
    kind: &str,
    version: u64,
    value: &T,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(text)?;
    let version = version
        .checked_add(1)
        .ok_or("implementation state version exhausted")?;
    let version = i64::try_from(version).map_err(text)?;
    transaction
        .execute(
            "INSERT INTO implementation_state (kind, version, bytes) VALUES (?1, ?2, ?3)
             ON CONFLICT(kind) DO UPDATE SET version = excluded.version, bytes = excluded.bytes",
            params![kind, version, bytes],
        )
        .map_err(text)?;
    Ok(())
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn diagnostic(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}

/// Admit the exact `Dispatch` request of one reservation, for the attempt
/// that holds it.
fn authorize_reservation(
    gate: &mut authority_bridge::DispatchGate<'_>,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    reservation: &DispatchReservation,
) -> Result<(), String> {
    authority_bridge::authorize_dispatch(
        gate,
        work,
        attempt,
        &[
            reservation.plan.as_str(),
            reservation.provider_profile.as_str(),
        ],
    )
}

/// Shell-owned wall clock in Unix milliseconds for effect boundaries that do
/// not carry an explicit host time.
pub fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

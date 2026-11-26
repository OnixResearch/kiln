// r[impl kiln.admission.durable]
//! Bounded durable fair admission over the implementation state owner.
//! No second database, claim or physical resource ledger is created.
#[path = "placement.rs"]
mod placement;
pub(crate) use placement::moved_attempt_row;
pub use placement::{MoveState, MoveStatus, SplitStatus, UnitState, UnitStatus};

use crate::{ATTEMPT_KIND, Implementer, WORK_KIND, diagnostic, load, save, text};
use kiln_core::fair_admission::*;
use kiln_core::implementation::authority::{AuthoritySelection, ProtectedAuthority};
use kiln_core::implementation::{AttemptLedger, Eligibility, ImplementationAttempt, WorkLedger};
use kiln_core::types::{ContentId, Diagnostic, ProviderProfileId};
use rusqlite::TransactionBehavior;
use serde::{Deserialize, Serialize};
pub(crate) const ADMISSION_KIND: &str = "fair-admission-v1";
const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;
// Covers future 64-hex claim identity, u64 fence/order and longer phase/reason
// spellings without letting post-enqueue metadata growth escape the byte cap.
const QUEUE_RECORD_MUTATION_BYTES: u64 = 256;

/// The execution/resource/serving owner remains authoritative. Implementations
/// must return aggregate service admission, not a worker-local queue estimate.
/// A transport error after possible effect must be mapped to Unknown, not rejection.
pub trait AdmissionOwnerPortV1 {
    fn reserve(&mut self, request: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure>;
    fn reconcile(&mut self, request: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure>;
    fn cancel(&mut self, request: &OwnerReservation) -> Result<OwnerReply, OwnerPortFailure>;
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum OwnerPortFailure {
    Waiting { retry_at_ms: Option<u64> },
    RejectedBeforeDispatch,
    ExpiredBeforeDispatch,
    ProviderFailureBeforeEffect,
    UnknownAfterPossibleEffect,
    CancellationRequested,
    CancellationObserved,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdmissionCommand {
    Configure {
        policy: AdmissionPolicy,
    },
    Root {
        account: BudgetAccount,
    },
    Transfer {
        parent: ContentId,
        child: BudgetAccount,
    },
    Enqueue {
        item: QueueItem,
    },
    Observe {
        observation: UsageObservation,
    },
    Select {
        inputs: Vec<EligibilityInput>,
    },
    Claim {
        inputs: Vec<EligibilityInput>,
        lease_ms: u64,
    },
    ClaimExact {
        expected_work_version: ContentId,
        parent_budget_identity: ContentId,
        inputs: Vec<EligibilityInput>,
        lease_ms: u64,
    },
    Request {
        reservation: OwnerReservation,
        inputs: Vec<EligibilityInput>,
    },
    Reply {
        reply: OwnerReply,
    },
    OwnerFailure {
        operation: ContentId,
        provider: ProviderProfileId,
        generation: u64,
        reason: AdmissionReason,
        retry_at_ms: Option<u64>,
    },
    Cancel {
        work_version: ContentId,
        expired: bool,
    },
    Settle {
        operation: ContentId,
        provider: ProviderProfileId,
        generation: u64,
        receipt: ContentId,
        consumed: Amounts,
    },
    /// Retain one injected serving-owner snapshot; evidence, never a grant.
    ObserveServing {
        observation: ServingObservation,
    },
    /// Revoke an account and its descendants without refunding usage.
    Revoke {
        account: ContentId,
    },
    /// Read the conservation-checked usage of one root lineage.
    Lineage {
        root: ContentId,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AdmissionResult {
    Updated,
    Policy {
        identity: ContentId,
    },
    Queued {
        reason: AdmissionReason,
    },
    Selected {
        work_version: Option<ContentId>,
    },
    Claimed {
        attempt: Option<Box<ImplementationAttempt>>,
    },
    /// An open external claim overlaps this work; nothing was claimed.
    Overlapped {
        cause: String,
    },
    NotWinner {
        selected: Option<ContentId>,
    },
    /// An exact claim found another active or unreconciled repository writer.
    WriterBusy,
    Owner {
        reason: AdmissionReason,
    },
    Retained {
        reservation: Box<OwnerReservation>,
    },
    Revoked {
        accounts: u32,
    },
    Lineage {
        usage: Box<LineageUsage>,
    },
}
impl Implementer {
    pub(crate) fn cancel_with_admission(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> Result<(), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        let (mut ledger, version) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        attempts
            .cancel(identity, fence, now_ms, cause)
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        if ledger.policy.is_some() {
            let attempt = attempts
                .get(identity)
                .ok_or("admission: cancelled attempt missing")?;
            if ledger
                .items
                .get(&attempt.work_identity)
                .is_some_and(|item| {
                    item.claim
                        .as_ref()
                        .is_some_and(|claim| &claim.attempt == identity && claim.fence == fence)
                })
            {
                ledger
                    .cancel(&attempt.work_identity, false)
                    .map_err(diagnostic)?;
                bounded_save(&tx, version, &ledger)?;
            }
        }
        save(&tx, ATTEMPT_KIND, attempt_version, &attempts)?;
        tx.commit().map_err(text)?;
        Ok(())
    }
    pub fn admission_snapshot(&mut self) -> Result<AdmissionLedger, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        tx.commit().map_err(text)?;
        Ok(ledger)
    }
    /// Claim a selected campaign work only if it remains the winner under the
    /// canonical, persisted admission observations, in the same writer transaction.
    pub fn claim_exact_winner(
        &mut self,
        expected_work_version: &ContentId,
        parent_budget_identity: &ContentId,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<AdmissionResult, String> {
        if let Some(overlap) = self.overlap.as_ref() {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(text)?;
            let (work, _) = load::<WorkLedger>(&tx, WORK_KIND)?;
            if let Some(current) = work
                .current()
                .find(|w| w.identity().is_ok_and(|id| id == *expected_work_version))
                && let Some(cause) = overlap(current)
            {
                return Ok(AdmissionResult::Overlapped { cause });
            }
        }
        self.admission_apply(
            AdmissionCommand::ClaimExact {
                expected_work_version: expected_work_version.clone(),
                parent_budget_identity: parent_budget_identity.clone(),
                inputs: Vec::new(),
                lease_ms,
            },
            now_ms,
        )
    }
    /// Every ready/order/quota/claim change commits atomically with existing work
    /// and attempt ledgers. No provider call is allowed inside this transaction.
    pub fn admission_apply(
        &mut self,
        command: AdmissionCommand,
        now_ms: u64,
    ) -> Result<AdmissionResult, String> {
        let supplied = match &command {
            AdmissionCommand::Select { inputs }
            | AdmissionCommand::Claim { inputs, .. }
            | AdmissionCommand::ClaimExact { inputs, .. }
            | AdmissionCommand::Request { inputs, .. } => inputs.as_slice(),
            _ => &[],
        };
        if supplied.len() > MAX_RECORDS {
            return Err("admission: eligibility input bound exceeded".to_string());
        }
        let mut unique = std::collections::BTreeSet::new();
        if supplied.iter().any(|i| !unique.insert(&i.work_version)) {
            return Err("admission: duplicate per-work eligibility observation".to_string());
        }
        let enrollment = self.enrollment.clone();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (work, _) = load::<WorkLedger>(&tx, WORK_KIND)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        let (mut ledger, version) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        let authority_slot = &mut self.authority;
        let authority_profile = &self.authority_profile;
        let result = (|| -> Result<AdmissionResult, Diagnostic> {
            Ok(match command {
                AdmissionCommand::Configure { policy } => AdmissionResult::Policy {
                    identity: ledger.configure(policy)?,
                },
                AdmissionCommand::Root { account } => {
                    ledger.add_root(account)?;
                    AdmissionResult::Updated
                }
                AdmissionCommand::Transfer { parent, child } => {
                    let claim = child
                        .parent_attempt
                        .as_ref()
                        .ok_or_else(|| refusal("parent attempt reference required"))?;
                    if !attempts.is_current(&claim.attempt, claim.fence, now_ms) {
                        return Err(refusal("parent attempt/fence no longer current"));
                    }
                    // The parent account must be bound to that exact admitted attempt.
                    let parent_item = ledger
                        .items
                        .values()
                        .find(|i| i.account == parent && i.claim.as_ref() == Some(claim))
                        .ok_or_else(|| refusal("parent quota not bound to parent attempt"))?;
                    let parent_attempt = attempts
                        .get(&claim.attempt)
                        .ok_or_else(|| refusal("parent attempt missing"))?;
                    if child.deadline_ms > parent_item.deadline_ms
                        || child.deadline_ms > parent_attempt.lease_until_ms
                    {
                        return Err(refusal(
                            "child deadline exceeds current parent work/attempt bounds",
                        ));
                    }
                    ledger.transfer(&parent, child)?;
                    AdmissionResult::Updated
                }
                AdmissionCommand::Enqueue { mut item } => {
                    let current = work
                        .current()
                        .find(|w| w.identity().is_ok_and(|id| id == item.work_version))
                        .ok_or_else(|| {
                            refusal("queue item is not a current admitted work version")
                        })?;
                    item.priority = current.priority;
                    // Canonical payload plus bounded mutable-record overhead;
                    // never trust the caller's item-byte assertion.
                    item.bytes = 0;
                    item.bytes = serde_json::to_vec(&item)
                        .map_err(|_| refusal("queue item encoding"))?
                        .len() as u64
                        + QUEUE_RECORD_MUTATION_BYTES;
                    AdmissionResult::Queued {
                        reason: ledger.enqueue(item, now_ms)?,
                    }
                }
                AdmissionCommand::Observe { observation } => {
                    ledger.observe(observation)?;
                    AdmissionResult::Updated
                }
                AdmissionCommand::Select { inputs } => {
                    let inputs = eligible_inputs(
                        &work,
                        &attempts,
                        &ledger,
                        &inputs,
                        now_ms,
                        &enrollment,
                        None,
                    );
                    AdmissionResult::Selected {
                        work_version: ledger.select(&inputs, now_ms)?,
                    }
                }
                AdmissionCommand::Claim { inputs, lease_ms } => {
                    let inputs = eligible_inputs(
                        &work,
                        &attempts,
                        &ledger,
                        &inputs,
                        now_ms,
                        &enrollment,
                        None,
                    );
                    let selected = ledger.select(&inputs, now_ms)?;
                    let attempt = if let Some(selected) = selected {
                        let current = work
                            .current()
                            .find(|w| w.identity().is_ok_and(|id| id == selected))
                            .ok_or_else(|| refusal("selected work not current"))?;
                        let item = ledger
                            .items
                            .get(&selected)
                            .ok_or_else(|| refusal("selected item missing"))?;
                        if let Some(claim) = &item.claim {
                            Some(
                                attempts
                                    .get(&claim.attempt)
                                    .filter(|_| {
                                        attempts.is_current(&claim.attempt, claim.fence, now_ms)
                                    })
                                    .ok_or_else(|| {
                                        refusal("postclaim wait cannot create replacement attempt")
                                    })?
                                    .clone(),
                            )
                        } else {
                            let attempt = attempts.claim(current, now_ms, lease_ms)?;
                            ledger.bind_claim(
                                &selected,
                                ClaimRef {
                                    attempt: attempt.identity.clone(),
                                    fence: attempt.fence,
                                },
                            )?;
                            bind_owner_transport(
                                &ledger,
                                &mut attempts,
                                &selected,
                                &attempt.identity,
                            )?;
                            Some(attempt)
                        }
                    } else {
                        None
                    };
                    let lease = lease_ms.to_string();
                    authorize_claim(
                        &work,
                        authority_slot,
                        authority_profile,
                        attempt.as_ref(),
                        &[&lease],
                        now_ms,
                    )?;
                    AdmissionResult::Claimed {
                        attempt: attempt.map(Box::new),
                    }
                }
                AdmissionCommand::ClaimExact {
                    expected_work_version,
                    parent_budget_identity,
                    inputs,
                    lease_ms,
                } => {
                    if ledger.policy.is_none() {
                        return Err(refusal("campaign claim requires explicit admission policy"));
                    }
                    let current = work
                        .current()
                        .find(|w| w.identity().is_ok_and(|id| id == expected_work_version))
                        .ok_or_else(|| refusal("campaign work is not current"))?;
                    validate_parent_lineage(
                        &ledger,
                        &expected_work_version,
                        &parent_budget_identity,
                    )?;
                    let existing_claim = ledger
                        .items
                        .get(&expected_work_version)
                        .ok_or_else(|| refusal("campaign queue item is missing"))?
                        .claim
                        .as_ref();
                    if let Some(claim) = existing_claim {
                        let attempt = attempts
                            .get(&claim.attempt)
                            .filter(|_| attempts.is_current(&claim.attempt, claim.fence, now_ms))
                            .ok_or_else(|| {
                                refusal("existing campaign claim/fence is no longer current")
                            })?
                            .clone();
                        let lease = lease_ms.to_string();
                        let parent = parent_budget_identity.as_str();
                        authorize_claim(
                            &work,
                            authority_slot,
                            authority_profile,
                            Some(&attempt),
                            &[&lease, parent],
                            now_ms,
                        )?;
                        return Ok(AdmissionResult::Claimed {
                            attempt: Some(Box::new(attempt)),
                        });
                    }
                    if same_repository_writer(
                        &work,
                        &attempts,
                        &expected_work_version,
                        &current.repository,
                    )? {
                        return Ok(AdmissionResult::WriterBusy);
                    }
                    let inputs = eligible_inputs(
                        &work,
                        &attempts,
                        &ledger,
                        &inputs,
                        now_ms,
                        &enrollment,
                        Some(&expected_work_version),
                    );
                    let selected =
                        ledger.select_scoped(&inputs, now_ms, Some(&expected_work_version))?;
                    if selected.as_ref() != Some(&expected_work_version) {
                        AdmissionResult::NotWinner { selected }
                    } else {
                        let attempt = attempts.claim(current, now_ms, lease_ms)?;
                        ledger.bind_claim(
                            &expected_work_version,
                            ClaimRef {
                                attempt: attempt.identity.clone(),
                                fence: attempt.fence,
                            },
                        )?;
                        bind_owner_transport(
                            &ledger,
                            &mut attempts,
                            &expected_work_version,
                            &attempt.identity,
                        )?;
                        let lease = lease_ms.to_string();
                        let parent = parent_budget_identity.as_str();
                        authorize_claim(
                            &work,
                            authority_slot,
                            authority_profile,
                            Some(&attempt),
                            &[&lease, parent],
                            now_ms,
                        )?;
                        AdmissionResult::Claimed {
                            attempt: Some(Box::new(attempt)),
                        }
                    }
                }
                AdmissionCommand::Request {
                    reservation,
                    inputs,
                } => {
                    if let Some(retained) = ledger.reservations.get(&reservation.operation) {
                        if retained.provider != reservation.provider
                            || retained.generation != reservation.generation
                            || retained.work_version != reservation.work_version
                            || retained.claim != reservation.claim
                            || retained.subject != reservation.subject
                            || retained.kind != reservation.kind
                            || retained.account != reservation.account
                            || retained.accounting_basis != reservation.accounting_basis
                            || retained.serving_bounds != reservation.serving_bounds
                            || (retained.state != ReservationState::Settled
                                && retained.amounts != reservation.amounts)
                        {
                            return Err(refusal(
                                "retained operation differs from requested identity",
                            ));
                        }
                        // A tentative request left by an interrupted reserve
                        // is resolved by the owner after this returns; that
                        // can commit it, so it presents a fresh invocation
                        // for the original effect first.
                        if retained.state == ReservationState::Tentative {
                            if !attempts.is_current(
                                &retained.claim.attempt,
                                retained.claim.fence,
                                now_ms,
                            ) {
                                return Err(refusal("reservation claim/fence expired"));
                            }
                            authorize_owner_request(
                                &work,
                                &attempts,
                                authority_slot,
                                authority_profile,
                                retained,
                                now_ms,
                            )?;
                        }
                        return Ok(AdmissionResult::Retained {
                            reservation: Box::new(retained.clone()),
                        });
                    }
                    if !attempts.is_current(
                        &reservation.claim.attempt,
                        reservation.claim.fence,
                        now_ms,
                    ) {
                        return Err(refusal("reservation claim/fence expired"));
                    }
                    // Blocked lineage and serving backpressure are structured
                    // outcomes; nothing tentative is persisted for them.
                    let blocker = ledger.lineage_blocker(&reservation.account, Some(now_ms))?;
                    let serving = (reservation.kind == ChargeKind::WorkerRequest)
                        .then(|| ledger.request_serving_gate(&reservation, now_ms))
                        .flatten()
                        .filter(|gate| *gate != AdmissionReason::Eligible);
                    if let Some(reason) = blocker.or(serving) {
                        if let Some(item) = ledger.items.get_mut(&reservation.work_version) {
                            item.reason = reason;
                        }
                        return Ok(AdmissionResult::Owner { reason });
                    }
                    let inputs = eligible_inputs(
                        &work,
                        &attempts,
                        &ledger,
                        &inputs,
                        now_ms,
                        &enrollment,
                        None,
                    );
                    if ledger.select(&inputs, now_ms)?.as_ref() != Some(&reservation.work_version) {
                        // Report the item's exact blocker rather than generic waiting.
                        let reason = ledger
                            .items
                            .get(&reservation.work_version)
                            .map(|item| item.reason)
                            .filter(|reason| *reason != AdmissionReason::Eligible)
                            .unwrap_or(AdmissionReason::Waiting);
                        return Ok(AdmissionResult::Owner { reason });
                    }
                    let input = inputs
                        .iter()
                        .find(|i| i.work_version == reservation.work_version)
                        .ok_or_else(|| refusal("fresh offer missing"))?;
                    if input.provider != reservation.provider
                        || input.generation != reservation.generation
                    {
                        return Err(refusal("request generation differs from ranked offer"));
                    }
                    authorize_owner_request(
                        &work,
                        &attempts,
                        authority_slot,
                        authority_profile,
                        &reservation,
                        now_ms,
                    )?;
                    ledger.request(reservation, now_ms)?;
                    AdmissionResult::Updated
                }
                AdmissionCommand::Reply { reply } => AdmissionResult::Owner {
                    reason: ledger.owner_reply(&reply, now_ms)?,
                },
                AdmissionCommand::OwnerFailure {
                    operation,
                    provider,
                    generation,
                    reason,
                    retry_at_ms,
                } => AdmissionResult::Owner {
                    reason: ledger.owner_outcome(
                        &operation,
                        &provider,
                        generation,
                        reason,
                        retry_at_ms,
                        now_ms,
                    )?,
                },
                AdmissionCommand::Cancel {
                    work_version,
                    expired,
                } => AdmissionResult::Owner {
                    reason: ledger.cancel(&work_version, expired)?,
                },
                AdmissionCommand::Settle {
                    operation,
                    provider,
                    generation,
                    receipt,
                    consumed,
                } => {
                    ledger.settle(&operation, &provider, generation, receipt, consumed)?;
                    AdmissionResult::Updated
                }
                AdmissionCommand::ObserveServing { observation } => {
                    ledger.observe_serving(observation)?;
                    AdmissionResult::Updated
                }
                AdmissionCommand::Revoke { account } => AdmissionResult::Revoked {
                    accounts: ledger.revoke_account(&account)?,
                },
                AdmissionCommand::Lineage { root } => AdmissionResult::Lineage {
                    usage: Box::new(ledger.lineage_usage(&root)?),
                },
            })
        })()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
        bounded_save(&tx, version, &ledger)?;
        save(&tx, ATTEMPT_KIND, attempt_version, &attempts)?;
        tx.commit().map_err(text)?;
        Ok(result)
    }
    /// Called by the actual Lattice shell immediately before possible transport.
    /// Explicit policy enables this gate. With no policy existing defaults remain.
    /// Fresh observations are supplied by an authorized host shell, not inventory UI.
    pub fn admission_before_transport(
        &mut self,
        attempt: &ContentId,
        fence: u64,
        fresh: &[EligibilityInput],
        now_ms: u64,
    ) -> Result<(), String> {
        let facts = self.authority_facts(attempt, fence)?;
        let plan = facts
            .attempt
            .dispatch
            .as_ref()
            .map(|dispatch| dispatch.plan.as_str())
            .unwrap_or_default();
        let effect = facts
            .attempt
            .dispatch
            .as_ref()
            .map_or(attempt.as_str(), |dispatch| dispatch.effect_id.as_str());
        let request = facts.request(
            kiln_core::implementation::authority::ProtectedOperation::Dispatch,
            effect,
            kiln_core::implementation::authority::NO_EXPECTED_TARGET,
            &[plan],
            None,
        )?;
        let currentness = self.attempt_currentness(attempt, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        let enrollment = &self.enrollment;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (work, _) = load::<WorkLedger>(&tx, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        // r[impl kiln.implementation.authority.interleaving]
        // The fence is re-read inside this transaction: a change committed
        // after the authority check above refuses transport with or without
        // an admission policy.
        if !attempts.is_current(attempt, fence, now_ms) {
            return Err("admission: stale attempt/fence".to_string());
        }
        let (mut ledger, version) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        let Some(policy) = ledger.policy.as_ref() else {
            tx.commit().map_err(text)?;
            return Ok(());
        };
        if fresh.len() > MAX_RECORDS {
            return Err("admission: too many fresh observations".to_string());
        }
        let policy_id = policy.identity().map_err(diagnostic)?;
        let max_age = policy.max_observation_age_ms;
        let current = attempts
            .get(attempt)
            .filter(|_| attempts.is_current(attempt, fence, now_ms))
            .ok_or("admission: stale attempt/fence")?;
        if !work
            .ranked()
            .iter()
            .any(|w| w.identity == current.work_identity)
        {
            return Err("admission: current work no longer eligible".to_string());
        }
        let current_work = work
            .current()
            .find(|w| w.identity().is_ok_and(|id| id == current.work_identity))
            .ok_or("admission: current work missing")?;
        if !enrollment.actors.contains(&current_work.actor)
            || !enrollment.repositories.contains(&current_work.repository)
        {
            return Err("admission: current enrollment revoked work authority".to_string());
        }
        let item = ledger
            .items
            .get(&current.work_identity)
            .ok_or("admission: claimed work was not queued")?;
        if item.claim.as_ref()
            != Some(&ClaimRef {
                attempt: attempt.clone(),
                fence,
            })
        {
            return Err("admission: queue claim differs from transport attempt".to_string());
        }
        if item.required_owners.iter().any(|(owner, kind)| {
            !ledger.reservations.values().any(|r| {
                r.work_version == current.work_identity
                    && &r.provider == owner
                    && &r.kind == kind
                    && r.state == ReservationState::Committed
            })
        }) {
            return Err("admission: missing required owner grant".to_string());
        }
        let mut operations: Vec<_> = ledger
            .reservations
            .values()
            .filter(|r| {
                r.work_version == current.work_identity
                    && !matches!(
                        r.state,
                        ReservationState::Rejected
                            | ReservationState::Expired
                            | ReservationState::Released
                            | ReservationState::Settled
                    )
            })
            .map(|r| r.operation.clone())
            .collect();
        let shared = item.shared_allocations.clone();
        operations.extend(shared.iter().cloned());
        if operations.is_empty() {
            return Err("admission: no owner-confirmed grant".to_string());
        }
        // Validate every required owner reference before modifying any intent.
        for operation in &operations {
            let reservation = ledger
                .reservations
                .get(operation)
                .ok_or("admission: owner operation disappeared")?;
            let mut matches = fresh.iter().filter(|i| {
                i.work_version == current.work_identity && i.provider == reservation.provider
            });
            let input = matches
                .next()
                .ok_or("admission: missing fresh provider offer")?;
            if matches.next().is_some() {
                return Err("admission: ambiguous fresh provider offer".to_string());
            }
            if input.reason != AdmissionReason::Eligible
                || input.generation != reservation.generation
                || input.observed_at_ms > now_ms
                || now_ms.saturating_sub(input.observed_at_ms) > max_age
                || input.valid_until_ms <= now_ms
                || reservation.state != ReservationState::Committed
                || reservation.expires_at_ms <= now_ms
            {
                return Err(
                    "admission: grant generation, eligibility or freshness changed".to_string(),
                );
            }
        }
        if !operations.iter().any(|op| {
            ledger.reservations.get(op).is_some_and(|r| {
                r.work_version == current.work_identity && r.kind != ChargeKind::PhysicalDeployment
            })
        }) {
            return Err(
                "admission: physical deployment alone cannot grant worker/request dispatch"
                    .to_string(),
            );
        }
        // The dispatch intent transitions all grants together: one failure rolls back.
        for operation in &operations {
            let reservation = ledger
                .reservations
                .get(operation)
                .ok_or("admission: owner operation disappeared")?;
            if reservation.kind == ChargeKind::PhysicalDeployment {
                continue;
            }
            let generation = reservation.generation;
            ledger
                .dispatch_intent(
                    operation,
                    &policy_id,
                    &ClaimRef {
                        attempt: attempt.clone(),
                        fence,
                    },
                    generation,
                    now_ms,
                )
                .map_err(diagnostic)?;
        }
        bounded_save(&tx, version, &ledger)?;
        tx.commit().map_err(text)?;
        Ok(())
    }
    pub fn admission_execution_observed(
        &mut self,
        attempt: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> Result<(), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (attempts, _) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        let (mut ledger, version) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        if ledger.policy.is_none() {
            tx.commit().map_err(text)?;
            return Ok(());
        }
        let current = attempts
            .get(attempt)
            .filter(|_| attempts.is_current(attempt, fence, now_ms))
            .filter(|a| a.workflow.is_some())
            .ok_or("admission: execution acknowledgement has no current durable workflow")?;
        ledger
            .execution_observed(
                &current.work_identity,
                &ClaimRef {
                    attempt: attempt.clone(),
                    fence,
                },
            )
            .map_err(diagnostic)?;
        bounded_save(&tx, version, &ledger)?;
        tx.commit().map_err(text)?;
        Ok(())
    }
    /// Settle every non-physical reservation of `(attempt, fence)` from an
    /// observed worker draw (for example Lattice's `budget_lineage.usage[]`).
    ///
    /// - With [`SettlementProvenance::HostMeasured`], tracked dimensions in
    ///   `reported_charged` are billed at their exact measured draw and the
    ///   unused remainder is returned to the account.
    /// - With [`SettlementProvenance::Unknown`], the full reserved amounts stay
    ///   charged fail-closed.
    /// - Any reported draw exceeding the attempt's reserved amount on a tracked
    ///   dimension is refused as a protocol violation (`worker_draw_exceeds_reservation`).
    /// - Replaying the exact settlement is idempotent; a conflicting replay
    ///   after settlement is refused (`admission_settlement_conflict`) and
    ///   never alters the settled charge.
    pub fn admission_settle_observed_draw(
        &mut self,
        attempt: &ContentId,
        fence: u64,
        receipt: &ContentId,
        reported_charged: Option<&std::collections::BTreeMap<String, u64>>,
        provenance: SettlementProvenance,
    ) -> Result<(), String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (attempts, _) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        let (mut ledger, version) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        if ledger.policy.is_none() {
            tx.commit().map_err(text)?;
            return Ok(());
        }
        let current = attempts
            .get(attempt)
            .filter(|a| a.fence == fence && a.workflow.is_some())
            .ok_or("admission: settlement observation has no durable workflow")?;
        let claim = ClaimRef {
            attempt: attempt.clone(),
            fence,
        };
        let matching: Vec<OwnerReservation> = ledger
            .reservations
            .values()
            .filter(|r| {
                r.work_version == current.work_identity
                    && r.claim == claim
                    && r.kind != ChargeKind::PhysicalDeployment
                    && matches!(
                        r.state,
                        ReservationState::Committed
                            | ReservationState::Unknown
                            | ReservationState::Settled
                    )
            })
            .cloned()
            .collect();
        if matching.is_empty() {
            tx.commit().map_err(text)?;
            return Ok(());
        }
        // Validate reported draw bounds against unsettled reservations first,
        // before mutating any reservation.
        let mut unsettled_reserved: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        let mut has_unsettled = false;
        for reservation in &matching {
            if matches!(
                reservation.state,
                ReservationState::Committed | ReservationState::Unknown
            ) {
                has_unsettled = true;
                for (dimension, amount) in &reservation.amounts {
                    let entry = unsettled_reserved
                        .entry(dimension.as_str().to_owned())
                        .or_insert(0);
                    *entry = entry.checked_add(*amount).ok_or_else(|| {
                        "admission: reserved dimension total overflowed".to_owned()
                    })?;
                }
            }
        }
        if has_unsettled && let Some(reported) = reported_charged {
            for (dimension, &drawn) in reported {
                if let Some(&reserved) = unsettled_reserved.get(dimension)
                    && drawn > reserved
                {
                    return Err(format!(
                        "worker_draw_exceeds_reservation: reported draw {drawn} exceeds reservation {reserved} for {dimension}"
                    ));
                }
            }
        }
        let mut remaining_draw = match (provenance, reported_charged) {
            (SettlementProvenance::HostMeasured, Some(charged)) => Some(charged.clone()),
            _ => None,
        };
        for reservation in &matching {
            let consumed = match (&mut remaining_draw, reservation.state) {
                (Some(remaining), ReservationState::Settled) => {
                    let mut amounts = Amounts::new();
                    for (dimension, &prior) in &reservation.amounts {
                        let charge = if matching.len() == 1 {
                            remaining.remove(dimension.as_str()).unwrap_or(prior)
                        } else if let Some(left) = remaining.get_mut(dimension.as_str()) {
                            let take = (*left).min(prior);
                            *left -= take;
                            take
                        } else {
                            prior
                        };
                        amounts.insert(dimension.clone(), charge);
                    }
                    amounts
                }
                (Some(remaining), _) => {
                    let mut amounts = Amounts::new();
                    for (dimension, &reserved) in &reservation.amounts {
                        let charge = if let Some(left) = remaining.get_mut(dimension.as_str()) {
                            let take = (*left).min(reserved);
                            *left -= take;
                            take
                        } else {
                            reserved
                        };
                        amounts.insert(dimension.clone(), charge);
                    }
                    amounts
                }
                (None, _) => reservation.amounts.clone(),
            };
            ledger
                .settle_with_provenance(
                    &reservation.operation,
                    &reservation.provider,
                    reservation.generation,
                    receipt.clone(),
                    consumed,
                    provenance,
                )
                .map_err(diagnostic)?;
        }
        bounded_save(&tx, version, &ledger)?;
        tx.commit().map_err(text)?;
        Ok(())
    }
    /// Persist request first; a retained operation is reconciled, never blindly
    /// sent as a new owner request after reconnect/restart.
    pub fn admission_reserve_with_owner(
        &mut self,
        reservation: OwnerReservation,
        inputs: Vec<EligibilityInput>,
        owner: &mut impl AdmissionOwnerPortV1,
        now_ms: u64,
    ) -> Result<AdmissionResult, String> {
        match self.admission_apply(
            AdmissionCommand::Request {
                reservation: reservation.clone(),
                inputs,
            },
            now_ms,
        )? {
            AdmissionResult::Updated => {
                let result = owner.reserve(&reservation);
                self.admission_record_port_result(&reservation, result, now_ms)
            }
            AdmissionResult::Retained { reservation } => {
                let result = owner.reconcile(&reservation);
                self.admission_record_port_result(&reservation, result, now_ms)
            }
            result => Ok(result),
        }
    }
    /// Cancellation request is persisted before the owner call. Re-entry uses
    /// reconciliation; observing cancellation does not establish zero usage.
    pub fn admission_cancel_with_owner(
        &mut self,
        operation: &ContentId,
        owner: &mut impl AdmissionOwnerPortV1,
        now_ms: u64,
    ) -> Result<AdmissionResult, String> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (mut ledger, version) = load::<AdmissionLedger>(&tx, ADMISSION_KIND)?;
        let reservation = ledger
            .reservations
            .get(operation)
            .cloned()
            .ok_or("admission: cancellation operation missing")?;
        let item = ledger
            .items
            .get(&reservation.work_version)
            .ok_or("admission: cancellation work missing")?;
        let already_requested = matches!(
            item.reason,
            AdmissionReason::CancellationRequested | AdmissionReason::Cancelled
        );
        ledger
            .cancel(&reservation.work_version, false)
            .map_err(diagnostic)?;
        bounded_save(&tx, version, &ledger)?;
        tx.commit().map_err(text)?;
        let result = if already_requested {
            owner.reconcile(&reservation)
        } else {
            owner.cancel(&reservation)
        };
        self.admission_record_port_result(&reservation, result, now_ms)
    }
    pub(crate) fn admission_record_port_result(
        &mut self,
        reservation: &OwnerReservation,
        result: Result<OwnerReply, OwnerPortFailure>,
        now_ms: u64,
    ) -> Result<AdmissionResult, String> {
        match result {
            Ok(reply) => self.admission_apply(AdmissionCommand::Reply { reply }, now_ms),
            Err(failure) => {
                let (reason, retry_at_ms) = match failure {
                    OwnerPortFailure::Waiting { retry_at_ms } => {
                        (AdmissionReason::Waiting, retry_at_ms)
                    }
                    OwnerPortFailure::RejectedBeforeDispatch => {
                        (AdmissionReason::ProviderRejected, None)
                    }
                    OwnerPortFailure::ExpiredBeforeDispatch => (AdmissionReason::Expired, None),
                    OwnerPortFailure::ProviderFailureBeforeEffect => {
                        (AdmissionReason::ProviderFailure, None)
                    }
                    OwnerPortFailure::UnknownAfterPossibleEffect => {
                        (AdmissionReason::Unknown, None)
                    }
                    OwnerPortFailure::CancellationRequested => {
                        (AdmissionReason::CancellationRequested, None)
                    }
                    OwnerPortFailure::CancellationObserved => (AdmissionReason::Cancelled, None),
                };
                self.admission_apply(
                    AdmissionCommand::OwnerFailure {
                        operation: reservation.operation.clone(),
                        provider: reservation.provider.clone(),
                        generation: reservation.generation,
                        reason,
                        retry_at_ms,
                    },
                    now_ms,
                )
            }
        }
    }
}
fn validate_parent_lineage(
    ledger: &AdmissionLedger,
    work_identity: &ContentId,
    parent_budget_identity: &ContentId,
) -> Result<(), Diagnostic> {
    let item = ledger
        .items
        .get(work_identity)
        .ok_or_else(|| refusal("campaign work has no canonical queue item"))?;
    let parent = ledger
        .accounts
        .get(parent_budget_identity)
        .ok_or_else(|| refusal("campaign parent budget is not a canonical account"))?;
    let account = ledger
        .accounts
        .get(&item.account)
        .ok_or_else(|| refusal("campaign queue account is missing"))?;
    if account.campaign != item.campaign
        || account.tenant != item.tenant
        || parent.campaign != item.campaign
        || parent.tenant != item.tenant
    {
        return Err(refusal(
            "campaign account lineage does not match queued scope",
        ));
    }
    let mut cursor = &item.account;
    let mut found_parent = false;
    for _ in 0..ledger.accounts.len() {
        let current = ledger
            .accounts
            .get(cursor)
            .ok_or_else(|| refusal("campaign account lineage has a missing ancestor"))?;
        if current.campaign != item.campaign || current.tenant != item.tenant {
            return Err(refusal(
                "campaign account ancestor belongs to another queued scope",
            ));
        }
        found_parent |= cursor == parent_budget_identity;
        match &current.parent {
            Some(ancestor) => cursor = ancestor,
            None => break,
        }
    }
    if ledger
        .accounts
        .get(cursor)
        .is_some_and(|account| account.parent.is_some())
    {
        return Err(refusal("campaign account lineage is cyclic"));
    }
    if !found_parent {
        return Err(refusal(
            "campaign parent budget is not an ancestor of the queue account",
        ));
    }
    Ok(())
}
fn same_repository_writer(
    work: &WorkLedger,
    attempts: &AttemptLedger,
    selected_identity: &ContentId,
    repository: &str,
) -> Result<bool, Diagnostic> {
    let active = attempts
        .attempts()
        .filter(|attempt| attempt.phase.is_active());
    let unresolved = attempts
        .unresolved()
        .filter(|attempt| !attempt.phase.is_active());
    for attempt in active
        .chain(unresolved)
        .filter(|attempt| &attempt.work_identity != selected_identity)
    {
        let other = work
            .get(&attempt.canonical_key)
            .or_else(|| {
                work.superseded().iter().find(|candidate| {
                    candidate
                        .identity()
                        .is_ok_and(|identity| identity == attempt.work_identity)
                })
            })
            .ok_or_else(|| refusal("active repository writer has no canonical work record"))?;
        if other.repository == repository {
            return Ok(true);
        }
    }
    Ok(false)
}
fn bounded_save(
    tx: &rusqlite::Transaction<'_>,
    version: u64,
    ledger: &AdmissionLedger,
) -> Result<(), String> {
    // Encode once through the existing state owner. The caller's transaction
    // rolls back the write if its exact encoded length exceeds the bound.
    save(tx, ADMISSION_KIND, version, ledger)?;
    let bytes: i64 = tx
        .query_row(
            "SELECT length(bytes) FROM implementation_state WHERE kind = ?1",
            rusqlite::params![ADMISSION_KIND],
            |row| row.get(0),
        )
        .map_err(text)?;
    if u64::try_from(bytes).map_err(text)? > MAX_STATE_BYTES as u64 {
        return Err(
            "admission: bounded state journal full; explicit disposition required".to_string(),
        );
    }
    Ok(())
}
fn eligible_inputs(
    work: &WorkLedger,
    attempts: &AttemptLedger,
    ledger: &AdmissionLedger,
    inputs: &[EligibilityInput],
    now_ms: u64,
    enrollment: &kiln_core::implementation::Enrollment,
    work_scope: Option<&ContentId>,
) -> Vec<EligibilityInput> {
    let mut merged = if let Some(scope) = work_scope {
        ledger
            .eligibility
            .get(scope)
            .map(|input| [(scope.clone(), input.clone())].into_iter().collect())
            .unwrap_or_default()
    } else {
        ledger.eligibility.clone()
    };
    for input in inputs
        .iter()
        .filter(|input| work_scope.is_none_or(|scope| scope == &input.work_version))
    {
        if merged.get(&input.work_version).is_some_and(|prior| {
            input.observed_at_ms < prior.observed_at_ms
                || (input.provider == prior.provider && input.generation < prior.generation)
        }) {
            continue;
        }
        merged.insert(input.work_version.clone(), input.clone());
    }
    merged
        .into_values()
        .map(|mut input| {
            let current = work
                .current()
                .find(|w| w.identity().is_ok_and(|id| id == input.work_version));
            if let Some(current) = current {
                if !enrollment.actors.contains(&current.actor)
                    || !enrollment.repositories.contains(&current.repository)
                {
                    input.reason = AdmissionReason::Denied;
                }
                if !matches!(
                    work.eligibility_of(
                        &current
                            .canonical_key()
                            .unwrap_or_else(|_| input.work_version.clone())
                    ),
                    Some(Eligibility::Ready)
                ) {
                    input.reason = AdmissionReason::Denied;
                }
                if let Some(claim) = ledger
                    .items
                    .get(&input.work_version)
                    .and_then(|i| i.claim.as_ref())
                {
                    if !attempts.is_current(&claim.attempt, claim.fence, now_ms) {
                        input.reason = AdmissionReason::Expired;
                    }
                } else {
                    // Probe the existing state machine, not a parallel eligibility rule.
                    let mut probe = attempts.clone();
                    if probe.claim(current, now_ms, 1).is_err() {
                        input.reason = AdmissionReason::BudgetExhausted;
                    }
                }
            } else {
                input.reason = AdmissionReason::Denied;
            }
            input
        })
        .collect()
}
// r[impl kiln.implementation.authority.dispatch]
/// Admit a fresh exact `Dispatch` invocation for one owner reservation before
/// the owner may commit it: a new request, a retried one, or a tentative one
/// resumed after an interrupted reserve. Its effect identity is the owner
/// operation and it names no launch incarnation, so every invocation for the
/// same reservation presents the same request with a fresh nonce, whatever
/// dispatch reservation or launch the attempt records later.
fn authorize_owner_request(
    work: &WorkLedger,
    attempts: &AttemptLedger,
    authority: &mut Option<Box<dyn ProtectedAuthority>>,
    profile: &AuthoritySelection,
    reservation: &OwnerReservation,
    now_ms: u64,
) -> Result<(), Diagnostic> {
    let reservation_attempt = attempts
        .get(&reservation.claim.attempt)
        .ok_or_else(|| refusal("reservation attempt missing"))?;
    let reservation_work = work
        .current()
        .find(|candidate| {
            candidate
                .identity()
                .is_ok_and(|identity| identity == reservation.work_version)
        })
        .ok_or_else(|| refusal("reservation work missing"))?;
    let request = super::authority_bridge::request_for(
        kiln_core::implementation::authority::ProtectedOperation::Dispatch,
        reservation_work,
        reservation_attempt,
        super::authority_bridge::PayloadBinding {
            incarnation: None,
            effect: reservation.operation.as_str(),
            expected_target: kiln_core::implementation::authority::NO_EXPECTED_TARGET,
        },
        &[reservation.operation.as_str()],
        None,
    )
    .map_err(|message| refusal(&message))?;
    let session = super::authority_bridge::admit_with(
        authority,
        profile,
        &request,
        kiln_core::implementation::authority::AttemptCurrentness::Current,
        now_ms,
    )
    .map_err(|message| refusal(&message))?;
    super::authority_bridge::revalidate_with(
        authority,
        session,
        &request,
        kiln_core::implementation::authority::AttemptCurrentness::Current,
        now_ms,
    )
    .map_err(|message| refusal(&message))
}

impl Implementer {
    /// Admit a fresh `Dispatch` invocation for a tentative owner reservation
    /// before the owner port resolves it; nothing is saved.
    ///
    /// # Errors
    /// Returns the ledger refusal or the redacted authority denial.
    pub(crate) fn authorize_owner_reservation(
        &mut self,
        reservation: &OwnerReservation,
        now_ms: u64,
    ) -> Result<(), String> {
        let tx = self.connection.transaction().map_err(text)?;
        let (work, _) = load::<WorkLedger>(&tx, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&tx, ATTEMPT_KIND)?;
        drop(tx);
        if !attempts.is_current(&reservation.claim.attempt, reservation.claim.fence, now_ms) {
            return Err(diagnostic(refusal("reservation claim/fence expired")));
        }
        authorize_owner_request(
            &work,
            &attempts,
            &mut self.authority,
            &self.authority_profile,
            reservation,
            now_ms,
        )
        .map_err(diagnostic)
    }
}

pub(crate) fn authorize_claim(
    work: &WorkLedger,
    authority: &mut Option<Box<dyn ProtectedAuthority>>,
    profile: &AuthoritySelection,
    attempt: Option<&ImplementationAttempt>,
    payload: &[&str],
    now_ms: u64,
) -> Result<(), Diagnostic> {
    let Some(attempt) = attempt else {
        return Ok(());
    };
    let request = super::authority_bridge::request_from_ledger(
        work,
        attempt,
        kiln_core::implementation::authority::ProtectedOperation::ClaimAcquire,
        super::authority_bridge::PayloadBinding {
            incarnation: None,
            effect: attempt.identity.as_str(),
            expected_target: kiln_core::implementation::authority::NO_EXPECTED_TARGET,
        },
        payload,
        None,
    )
    .map_err(|message| refusal(&message))?;
    let session = super::authority_bridge::admit_with(
        authority,
        profile,
        &request,
        kiln_core::implementation::authority::AttemptCurrentness::Current,
        now_ms,
    )
    .map_err(|message| refusal(&message))?;
    super::authority_bridge::revalidate_with(
        authority,
        session,
        &request,
        kiln_core::implementation::authority::AttemptCurrentness::Current,
        now_ms,
    )
    .map_err(|message| refusal(&message))
}

// r[impl kiln.admission.host_owner]
/// A claim of a host-owner queue row may record a workflow only after a
/// recorded transport start. Bound in the claim's own transaction, before the
/// attempt exists anywhere else, so no out-of-band launch can precede it.
fn bind_owner_transport(
    ledger: &AdmissionLedger,
    attempts: &mut AttemptLedger,
    work: &ContentId,
    attempt: &ContentId,
) -> Result<(), Diagnostic> {
    let account = &ledger
        .items
        .get(work)
        .ok_or_else(|| refusal("claimed work is not queued"))?
        .account;
    if is_host_owner_account(ledger, account)? {
        attempts.bind_transport_start(attempt)?;
    }
    Ok(())
}

fn refusal(message: &str) -> Diagnostic {
    Diagnostic::new("admission_denied", "admission", message)
}

// r[impl kiln.admission.host_owner]
// r[impl kiln.admission.host_owner_profile]
//! One explicitly registered host-local resource owner.
//!
//! An operator-reviewed profile (typed Nickel exported to JSON) declares one
//! host's capacity, the repositories and work classes it admits and the finite
//! campaign budget. These pure functions derive the existing fair-admission
//! records from it and decide the owner's reply; the shell persists them through
//! the same ledger transitions every other owner uses. There is no second queue
//! or scheduler.
//!
//! The only capacity this owner can grant is its declared registration, and its
//! only occupancy is the set of reservations the ledger retains for it. It never
//! reports remote, provider or serving capacity. A slot is released only when the
//! attempt ledger shows the holding attempt terminal and resolved and, once
//! transport may have started, its worker observed stopped (a reconciled
//! termination or a retained candidate). Supersession, lease expiry, elapsed
//! time, restart and replay never free a slot or refund budget; a reservation
//! that provably never reached transport is returned without consumption.
use super::{
    ADMISSION_VERSION, AdmissionLedger, AdmissionPhase, AdmissionPolicy, AdmissionReason, Amounts,
    BudgetAccount, ChargeKind, ClaimRef, EligibilityInput, MAX_DIMENSIONS, MAX_RECORDS, OwnerReply,
    OwnerReservation, QueueItem, ReservationState, UsageObservation, add, err, validate_amounts,
};
use crate::implementation::{AttemptLedger, ImplementationWork};
use crate::operations::{FairnessStrategy, TenantUsage, UsageValue};
use crate::types::{Alias, ContentId, CoreResult, ProviderProfileId};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};
use serde::{Deserialize, Serialize};

/// Byte bound of one `owner/name` repository key.
pub const MAX_HOST_OWNER_REPOSITORY_BYTES: usize = 256;
/// Byte bound of one work-class project name.
pub const MAX_HOST_OWNER_PROJECT_BYTES: usize = 128;

/// Typed host-local owner registration exported from
/// `config/fair-admission-host-owner.ncl`. Exporting it grants nothing; the
/// operator applies it through the implementer's `host-owner-*` commands.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostOwnerProfile {
    pub version: u16,
    pub owner: HostOwnerRegistration,
    pub campaign: Alias,
    pub tenant: Alias,
    /// Absolute deadline of the campaign root account and every queued work.
    pub deadline_ms: u64,
    /// Finite root allocation. Replay re-admits it and never resets it.
    pub quota: Amounts,
    /// Exact amounts one admitted work reserves from this owner.
    pub per_work: Amounts,
    pub limits: HostOwnerLimits,
    pub repositories: Vec<HostOwnerRepository>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostOwnerRegistration {
    pub provider: ProviderProfileId,
    /// Operator label of the host; not a cryptographic host identity.
    pub host: Alias,
    /// Registration generation. Bump it whenever the declared capacity changes;
    /// observations and grants of another generation are stale.
    pub generation: u64,
    /// Declared host-local capacity, never a remote or provider observation.
    pub capacity: Amounts,
    /// Validity window of one eligibility observation.
    pub observation_ttl_ms: u64,
    /// Upper bound of one reservation's lifetime.
    pub reservation_ttl_ms: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostOwnerLimits {
    pub max_items: u32,
    pub max_bytes: u64,
    pub max_wait_ms: u64,
    pub max_observation_age_ms: u64,
    pub max_retry_after_ms: u64,
    pub max_children: u32,
    pub max_active: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostOwnerRepository {
    /// Kiln's lowercase durable repository key, `owner/name`.
    pub repository: String,
    pub classes: Vec<HostOwnerWorkClass>,
}
/// A work class is the exact project and frozen verification profile an
/// admitted work carries; any other work in the repository is unregistered.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostOwnerWorkClass {
    pub name: Alias,
    pub project: String,
    pub verification_profile: ContentId,
}

/// What the shell must do to hold one claim's reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostReservationPlan {
    /// This exact operation is already retained; replay makes no new request.
    Retained(OwnerReservation),
    /// Persist this tentative request, then ask the owner.
    Request(OwnerReservation),
}
/// The host owner's answer to one persisted request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostOwnerDecision {
    Committed(OwnerReply),
    /// The registered capacity is held by other retained reservations.
    OverCapacity,
    Expired,
    /// The request is not one this registration can grant.
    Mismatch,
}
/// The settlement an observed release permits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostReleasePlan {
    /// The release was already settled with this exact receipt.
    Settled(OwnerReservation),
    Settle {
        operation: ContentId,
        provider: ProviderProfileId,
        generation: u64,
        receipt: ContentId,
        consumed: Amounts,
    },
}

impl HostOwnerProfile {
    /// Re-admit the export with the Nickel contract's bounds.
    pub fn validate(&self) -> CoreResult<()> {
        let owner = &self.owner;
        if self.version != ADMISSION_VERSION
            || owner.generation == 0
            || owner.observation_ttl_ms == 0
            || owner.observation_ttl_ms > self.limits.max_observation_age_ms
            || owner.reservation_ttl_ms == 0
            || self.deadline_ms == 0
        {
            return Err(profile_error(
                "invalid host owner version, generation, window or deadline",
            ));
        }
        validate_amounts(&owner.capacity)?;
        validate_amounts(&self.per_work)?;
        validate_amounts(&self.quota)?;
        if self.per_work.keys().ne(self.quota.keys())
            || self.per_work.iter().any(|(dimension, amount)| {
                owner.capacity.get(dimension).is_none_or(|cap| amount > cap)
                    || self.quota.get(dimension).is_none_or(|quota| amount > quota)
            })
        {
            return Err(profile_error(
                "per-work amounts must use the quota dimensions and fit capacity and quota",
            ));
        }
        if self.repositories.is_empty() || self.repositories.len() > MAX_RECORDS {
            return Err(profile_error(
                "admitted repositories must be finite and non-empty",
            ));
        }
        let mut repositories = BTreeSet::new();
        for entry in &self.repositories {
            let key = entry.repository.as_str();
            if !repositories.insert(key)
                || key.is_empty()
                || key.len() > MAX_HOST_OWNER_REPOSITORY_BYTES
                || key != key.to_ascii_lowercase()
                || key.chars().any(char::is_control)
                || key.split('/').count() != 2
                || key.split('/').any(str::is_empty)
                || entry.classes.is_empty()
                || entry.classes.len() > MAX_DIMENSIONS
            {
                return Err(profile_error(
                    "repository keys must be unique lowercase owner/name with bounded classes",
                ));
            }
            let mut names = BTreeSet::new();
            let mut contracts = BTreeSet::new();
            for class in &entry.classes {
                ContentId::from_blake3_hex(class.verification_profile.as_str())?;
                if !names.insert(class.name.as_str())
                    || !contracts
                        .insert((class.project.as_str(), class.verification_profile.as_str()))
                    || class.project.is_empty()
                    || class.project.len() > MAX_HOST_OWNER_PROJECT_BYTES
                    || class.project.chars().any(char::is_control)
                {
                    return Err(profile_error(
                        "work classes must have unique names and unambiguous contracts",
                    ));
                }
            }
        }
        self.policy().map(|_| ())
    }
    /// Domain-tagged identity of the dimension contract: provider, host and
    /// capacity dimension names. A capacity value change keeps the basis.
    pub fn accounting_basis(&self) -> CoreResult<ContentId> {
        let mut parts = alloc::vec![self.owner.provider.as_str(), self.owner.host.as_str()];
        parts.extend(self.owner.capacity.keys().map(Alias::as_str));
        tagged("kiln.host-owner.accounting-basis.v1", &parts, &[])
    }
    /// The exact frozen admission policy this registration configures.
    pub fn policy(&self) -> CoreResult<AdmissionPolicy> {
        let limits = &self.limits;
        let policy = AdmissionPolicy {
            version: ADMISSION_VERSION,
            accounting_basis: self.accounting_basis()?,
            max_items: limits.max_items,
            max_bytes: limits.max_bytes,
            max_wait_ms: limits.max_wait_ms,
            max_observation_age_ms: limits.max_observation_age_ms,
            max_retry_after_ms: limits.max_retry_after_ms,
            max_children: limits.max_children,
            max_active: limits.max_active,
            strategy: FairnessStrategy::DominantShare,
            shares: BTreeMap::from([(self.campaign.clone(), 1)]),
            group_limits: Vec::new(),
            serving: None,
        };
        policy.validate()?;
        Ok(policy)
    }
    pub fn account_identity(&self) -> CoreResult<ContentId> {
        tagged(
            "kiln.host-owner.account.v1",
            &[self.campaign.as_str(), self.tenant.as_str()],
            &[],
        )
    }
    /// The campaign's one finite root account.
    pub fn root_account(&self) -> CoreResult<BudgetAccount> {
        Ok(BudgetAccount {
            identity: self.account_identity()?,
            parent: None,
            parent_attempt: None,
            campaign: self.campaign.clone(),
            tenant: self.tenant.clone(),
            deadline_ms: self.deadline_ms,
            available: self.quota.clone(),
            consumed: Amounts::new(),
            children: 0,
        })
    }
    /// The registered class of one work, or an unregistered refusal.
    pub fn work_class(&self, work: &ImplementationWork) -> CoreResult<&HostOwnerWorkClass> {
        let repository = self
            .repositories
            .iter()
            .find(|entry| entry.repository == work.repository)
            .ok_or_else(|| {
                err(
                    "host_owner_unregistered",
                    "repository is not registered with this host owner",
                )
            })?;
        repository
            .classes
            .iter()
            .find(|class| {
                class.project == work.project
                    && class.verification_profile == work.verification_profile
            })
            .ok_or_else(|| {
                err(
                    "host_owner_unregistered",
                    "work class is not registered for this repository",
                )
            })
    }
    /// The queue row of one registered work. The ledger assigns order and bytes.
    pub fn queue_item(&self, work: &ImplementationWork, now_ms: u64) -> CoreResult<QueueItem> {
        self.work_class(work)?;
        Ok(QueueItem {
            work_version: work.identity()?,
            account: self.account_identity()?,
            tenant: self.tenant.clone(),
            campaign: self.campaign.clone(),
            repository: Alias::new(work.repository.clone())?,
            priority: work.priority,
            bytes: 1,
            enqueued_at_ms: now_ms,
            deadline_ms: self.deadline_ms,
            eligible_at_ms: now_ms,
            claim: None,
            phase: AdmissionPhase::PreClaim,
            reason: AdmissionReason::Waiting,
            order: 0,
            required_owners: BTreeMap::from([(self.owner.provider.clone(), ChargeKind::Execution)]),
            shared_allocations: Vec::new(),
            budget: self.per_work.clone(),
        })
    }
    /// The campaign usage observation at `now_ms`. Measured usage is zero; the
    /// ledger composes every retained debit on top, so nothing is reset.
    pub fn usage_observation(
        &self,
        ledger: &AdmissionLedger,
        now_ms: u64,
    ) -> CoreResult<UsageObservation> {
        let prior = ledger.observations.get(&self.campaign);
        let generation = match prior {
            Some(prior) => prior
                .generation
                .checked_add(1)
                .ok_or_else(|| err("host_owner_usage", "usage generation exhausted"))?,
            None => 1,
        };
        Ok(UsageObservation {
            usage: TenantUsage {
                tenant: self.campaign.clone(),
                share: 1,
                provider: self.owner.provider.clone(),
                interval_start: prior.map_or(now_ms, |prior| prior.usage.interval_start),
                interval_end: now_ms,
                observed_at: now_ms,
                resources: self
                    .owner
                    .capacity
                    .iter()
                    .map(|(dimension, capacity)| {
                        (
                            dimension.clone(),
                            UsageValue {
                                amount: 0,
                                capacity: Some(*capacity),
                            },
                        )
                    })
                    .collect(),
            },
            generation,
            accounting_basis: self.accounting_basis()?,
            covered: BTreeMap::new(),
            complete: true,
        })
    }
    /// One fresh eligibility observation per open queued work that needs this
    /// owner. Holders stay eligible; other work is eligible only while a free
    /// registered slot remains (claimed rows first, then durable queue order),
    /// so no claim is invited that the owner would then refuse.
    pub fn eligibility(
        &self,
        ledger: &AdmissionLedger,
        now_ms: u64,
    ) -> CoreResult<Vec<EligibilityInput>> {
        let occupied = self.occupied(ledger, None)?;
        let mut free: Amounts = self
            .owner
            .capacity
            .iter()
            .map(|(dimension, capacity)| {
                let held = occupied.get(dimension).copied().unwrap_or(0);
                (dimension.clone(), capacity.saturating_sub(held))
            })
            .collect();
        let mut rows: Vec<&QueueItem> = ledger
            .items
            .values()
            .filter(|item| {
                item.required_owners.get(&self.owner.provider) == Some(&ChargeKind::Execution)
                    && matches!(
                        item.phase,
                        AdmissionPhase::PreClaim
                            | AdmissionPhase::PostClaim
                            | AdmissionPhase::Reserved
                    )
                    && now_ms < item.deadline_ms
            })
            .collect();
        rows.sort_by_key(|item| (item.claim.is_none(), item.order));
        let mut inputs = Vec::with_capacity(rows.len());
        for item in rows {
            let eligible =
                self.holder(ledger, &item.work_version).is_some() || self.take(&mut free);
            inputs.push(EligibilityInput {
                work_version: item.work_version.clone(),
                reason: if eligible {
                    AdmissionReason::Eligible
                } else {
                    AdmissionReason::Waiting
                },
                provider: self.owner.provider.clone(),
                generation: self.owner.generation,
                observed_at_ms: now_ms,
                valid_until_ms: now_ms
                    .saturating_add(self.owner.observation_ttl_ms)
                    .min(item.deadline_ms),
            });
        }
        Ok(inputs)
    }
    /// Plan the reservation for one exact claim. Refuses unregistered work, a
    /// queue/claim mismatch, a stale observation, a second reservation for the
    /// same work and a request beyond the registered capacity.
    pub fn plan_reservation(
        &self,
        ledger: &AdmissionLedger,
        work: &ImplementationWork,
        claim: &ClaimRef,
        lease_until_ms: u64,
        now_ms: u64,
    ) -> CoreResult<HostReservationPlan> {
        self.validate()?;
        self.work_class(work)?;
        let work_version = work.identity()?;
        let item = ledger.items.get(&work_version).ok_or_else(|| {
            err(
                "host_owner_unqueued",
                "work is not queued under this host owner",
            )
        })?;
        if item.required_owners.get(&self.owner.provider) != Some(&ChargeKind::Execution)
            || item.account != self.account_identity()?
        {
            return Err(err(
                "host_owner_unqueued",
                "queued work does not require this host owner",
            ));
        }
        if item.claim.as_ref() != Some(claim) {
            return Err(err(
                "host_owner_claim",
                "queue claim differs from the reserving attempt/fence",
            ));
        }
        let operation = self.operation_identity(ledger, &work_version, claim)?;
        if let Some(holder) = self.holder(ledger, &work_version) {
            return if holder.operation == operation {
                Ok(HostReservationPlan::Retained(holder.clone()))
            } else {
                Err(err(
                    "host_owner_duplicate",
                    "work already holds a host reservation under another operation",
                ))
            };
        }
        let fresh = ledger.eligibility.get(&work_version).is_some_and(|input| {
            input.provider == self.owner.provider
                && input.generation == self.owner.generation
                && input.reason == AdmissionReason::Eligible
                && input.observed_at_ms <= now_ms
                && now_ms.saturating_sub(input.observed_at_ms) <= self.limits.max_observation_age_ms
                && input.valid_until_ms > now_ms
        });
        if !fresh {
            return Err(err(
                "host_owner_stale",
                "no fresh eligible observation of this owner generation for the work",
            ));
        }
        if !self.fits(&add(&self.occupied(ledger, None)?, &self.per_work)?) {
            return Err(err(
                "host_owner_capacity",
                "registered host capacity is held by other reservations",
            ));
        }
        let expires_at_ms = item
            .deadline_ms
            .min(lease_until_ms)
            .min(now_ms.saturating_add(self.owner.reservation_ttl_ms));
        if expires_at_ms <= now_ms {
            return Err(err(
                "host_owner_expired",
                "claim lease or work deadline leaves no reservation window",
            ));
        }
        Ok(HostReservationPlan::Request(OwnerReservation {
            version: ADMISSION_VERSION,
            operation,
            provider: self.owner.provider.clone(),
            generation: self.owner.generation,
            work_version,
            claim: claim.clone(),
            account: item.account.clone(),
            kind: ChargeKind::Execution,
            subject: self.subject_identity(claim)?,
            accounting_basis: self.accounting_basis()?,
            serving_bounds: None,
            amounts: self.per_work.clone(),
            expires_at_ms,
            state: ReservationState::Tentative,
            receipt: None,
        }))
    }
    /// The owner's reply to a request persisted in `ledger`. Every other
    /// retained, unreleased reservation of this owner occupies capacity.
    pub fn decide(
        &self,
        ledger: &AdmissionLedger,
        request: &OwnerReservation,
        now_ms: u64,
    ) -> CoreResult<HostOwnerDecision> {
        if request.version != ADMISSION_VERSION
            || request.provider != self.owner.provider
            || request.generation != self.owner.generation
            || request.kind != ChargeKind::Execution
            || request.accounting_basis != self.accounting_basis()?
            || request.account != self.account_identity()?
            || request.amounts != self.per_work
            || request.subject != self.subject_identity(&request.claim)?
            || request.serving_bounds.is_some()
            || ledger
                .items
                .get(&request.work_version)
                .is_none_or(|item| item.claim.as_ref() != Some(&request.claim))
        {
            return Ok(HostOwnerDecision::Mismatch);
        }
        if request.expires_at_ms <= now_ms {
            return Ok(HostOwnerDecision::Expired);
        }
        let others = self.occupied(ledger, Some(&request.operation))?;
        if !self.fits(&add(&others, &request.amounts)?) {
            return Ok(HostOwnerDecision::OverCapacity);
        }
        Ok(HostOwnerDecision::Committed(OwnerReply {
            version: ADMISSION_VERSION,
            operation: request.operation.clone(),
            provider: self.owner.provider.clone(),
            generation: self.owner.generation,
            receipt: tagged(
                "kiln.host-owner.receipt.v1",
                &[
                    request.operation.as_str(),
                    self.owner.provider.as_str(),
                    self.owner.host.as_str(),
                ],
                &[self.owner.generation],
            )?,
            state: ReservationState::Committed,
            retry_at_ms: None,
            serving_bounds: None,
        }))
    }
    /// Settle one claim's reservation after the attempt ledger records the
    /// holding attempt terminal and resolved. A reservation that provably never
    /// reached transport (no dispatch intent, transport start or workflow) is
    /// returned without consuming the slot. Once transport may have started,
    /// release also needs an observed worker stop: a reconciled termination
    /// observation or an admitted candidate handoff. Supersession or lease
    /// expiry alone never frees the slot; the slot's budget is then consumed.
    pub fn plan_release(
        &self,
        ledger: &AdmissionLedger,
        attempts: &AttemptLedger,
        claim: &ClaimRef,
    ) -> CoreResult<HostReleasePlan> {
        let reservation = ledger
            .reservations
            .values()
            .find(|r| {
                r.provider == self.owner.provider
                    && &r.claim == claim
                    && !matches!(
                        r.state,
                        ReservationState::Rejected
                            | ReservationState::Expired
                            | ReservationState::Released
                    )
            })
            .ok_or_else(|| {
                err(
                    "host_owner_release_missing",
                    "claim holds no reservation of this host owner",
                )
            })?;
        if reservation.state == ReservationState::Settled {
            return Ok(HostReleasePlan::Settled(reservation.clone()));
        }
        let attempt = attempts
            .get(&claim.attempt)
            .filter(|attempt| attempt.fence == claim.fence);
        let dispatched = reservation.state == ReservationState::Unknown
            || ledger
                .items
                .get(&reservation.work_version)
                .is_some_and(|item| {
                    matches!(
                        item.phase,
                        AdmissionPhase::DispatchIntent | AdmissionPhase::Executing
                    )
                })
            || attempts.dispatch_requires_recovery(&claim.attempt)
            || attempt.is_none_or(|attempt| attempt.workflow.is_some());
        let observed = attempt.and_then(|attempt| {
            let stopped = !dispatched
                || attempts.termination_observed(&attempt.identity)
                || attempt.handoff.is_some()
                || attempt.candidate.is_some();
            (reservation.state != ReservationState::Tentative
                && attempt.phase.is_terminal()
                && stopped
                && !attempts
                    .unresolved()
                    .any(|unresolved| unresolved.identity == attempt.identity))
            .then_some(attempt.ended_at_ms)
            .flatten()
        });
        let Some(ended_at_ms) = observed else {
            return Err(err(
                "host_owner_release_unobserved",
                "release needs a granted reservation whose attempt is observed terminal, resolved and, after possible transport, observed stopped",
            ));
        };
        Ok(HostReleasePlan::Settle {
            operation: reservation.operation.clone(),
            provider: reservation.provider.clone(),
            generation: reservation.generation,
            receipt: tagged(
                "kiln.host-owner.release.v1",
                &[reservation.operation.as_str(), claim.attempt.as_str()],
                &[claim.fence, ended_at_ms, u64::from(dispatched)],
            )?,
            consumed: if dispatched {
                reservation.amounts.clone()
            } else {
                Amounts::new()
            },
        })
    }
    /// Deterministic per-claim operation. A safely refused earlier operation of
    /// the same claim advances the ordinal, so replay retains, never duplicates.
    fn operation_identity(
        &self,
        ledger: &AdmissionLedger,
        work_version: &ContentId,
        claim: &ClaimRef,
    ) -> CoreResult<ContentId> {
        let refused = ledger
            .reservations
            .values()
            .filter(|r| {
                r.provider == self.owner.provider
                    && &r.work_version == work_version
                    && &r.claim == claim
                    && matches!(
                        r.state,
                        ReservationState::Rejected | ReservationState::Expired
                    )
            })
            .count() as u64;
        tagged(
            "kiln.host-owner.operation.v1",
            &[
                self.owner.provider.as_str(),
                work_version.as_str(),
                claim.attempt.as_str(),
            ],
            &[self.owner.generation, claim.fence, refused],
        )
    }
    fn subject_identity(&self, claim: &ClaimRef) -> CoreResult<ContentId> {
        tagged(
            "kiln.host-owner.subject.v1",
            &[
                self.owner.provider.as_str(),
                self.owner.host.as_str(),
                claim.attempt.as_str(),
            ],
            &[claim.fence],
        )
    }
    /// The retained, unreleased reservation of this owner for one work.
    fn holder<'a>(
        &self,
        ledger: &'a AdmissionLedger,
        work_version: &ContentId,
    ) -> Option<&'a OwnerReservation> {
        ledger.reservations.values().find(|r| {
            r.provider == self.owner.provider
                && &r.work_version == work_version
                && !matches!(
                    r.state,
                    ReservationState::Rejected
                        | ReservationState::Expired
                        | ReservationState::Released
                )
        })
    }
    /// Amounts held by this owner's tentative, committed and unknown operations.
    fn occupied(
        &self,
        ledger: &AdmissionLedger,
        except: Option<&ContentId>,
    ) -> CoreResult<Amounts> {
        let mut held = Amounts::new();
        for r in ledger.reservations.values().filter(|r| {
            r.provider == self.owner.provider
                && Some(&r.operation) != except
                && matches!(
                    r.state,
                    ReservationState::Tentative
                        | ReservationState::Committed
                        | ReservationState::Unknown
                )
        }) {
            for (dimension, amount) in &r.amounts {
                let total = held.entry(dimension.clone()).or_insert(0);
                *total = total
                    .checked_add(*amount)
                    .ok_or_else(|| err("admission_arithmetic", "host occupancy overflow"))?;
            }
        }
        Ok(held)
    }
    /// Take one work's amounts from `free` when they fit.
    fn take(&self, free: &mut Amounts) -> bool {
        if !self
            .per_work
            .iter()
            .all(|(dimension, amount)| free.get(dimension).is_some_and(|left| left >= amount))
        {
            return false;
        }
        for (dimension, amount) in &self.per_work {
            if let Some(left) = free.get_mut(dimension) {
                *left -= amount;
            }
        }
        true
    }
    fn fits(&self, demand: &Amounts) -> bool {
        demand.iter().all(|(dimension, amount)| {
            self.owner
                .capacity
                .get(dimension)
                .is_some_and(|capacity| amount <= capacity)
        })
    }
}

fn profile_error(message: &str) -> crate::types::Diagnostic {
    err("host_owner_profile", message)
}

/// Domain-tagged, length-prefixed BLAKE3 identity.
fn tagged(domain: &str, parts: &[&str], numbers: &[u64]) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    for number in numbers {
        hasher.update(&number.to_le_bytes());
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
}

// r[impl kiln.admission.bounded]
//! Opt-in bounded ready-work admission, not physical allocation or serving authority.
//!
//! The shell persists every transition before an owner call. Existing operations
//! fairness is reused unchanged. Unknown effects retain quota and active slots.
use crate::operations::{FairnessStrategy, TenantUsage, rank_tenants, validate_usage_sequence};
use crate::types::{Alias, ContentId, CoreResult, Diagnostic, ProviderProfileId};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::ToString,
    vec::Vec,
};
use serde::{Deserialize, Serialize};

mod host_owner;
mod lineage;
mod serving;
pub use host_owner::{
    HostOwnerDecision, HostOwnerLimits, HostOwnerProfile, HostOwnerRegistration,
    HostOwnerRepository, HostOwnerWorkClass, HostReleasePlan, HostReservationPlan,
    MAX_HOST_OWNER_PROJECT_BYTES, MAX_HOST_OWNER_REPOSITORY_BYTES, host_owner_account_identity,
    host_owner_governed, is_host_owner_account,
};
pub use lineage::{
    LineageAccountProfile, LineageExplanation, LineageProfile, LineageRootProfile, LineageUsage,
    MAX_LINEAGE_DEPTH,
};
pub use serving::{
    ServingAdmissionPolicy, ServingCapacityRule, ServingExplanation, ServingObservation,
    ServingOrigin, ServingReadiness,
};

pub const ADMISSION_VERSION: u16 = 1;
pub const MAX_RECORDS: usize = 4096;
pub const MAX_DIMENSIONS: usize = 16;
pub const MAX_SERVING_REQUESTS: u32 = 65_536;
pub type Amounts = BTreeMap<Alias, u64>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueScope {
    Tenant,
    Repository,
    Campaign,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueGroupLimit {
    pub scope: QueueScope,
    pub group: Alias,
    pub max_items: u32,
    pub max_bytes: u64,
    pub max_active: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionPolicy {
    pub version: u16,
    pub accounting_basis: ContentId,
    pub max_items: u32,
    pub max_bytes: u64,
    pub max_wait_ms: u64,
    pub max_observation_age_ms: u64,
    pub max_retry_after_ms: u64,
    pub max_children: u32,
    pub max_active: u32,
    pub strategy: FairnessStrategy,
    /// Explicit comparable campaign shares. Queue renaming cannot change these.
    pub shares: BTreeMap<Alias, u64>,
    pub group_limits: Vec<QueueGroupLimit>,
    /// Opt-in serving-owner snapshot gate for worker requests. Absent keeps
    /// the existing request path and policy identity unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serving: Option<ServingAdmissionPolicy>,
}
impl AdmissionPolicy {
    pub fn validate(&self) -> CoreResult<()> {
        if self.version != ADMISSION_VERSION
            || self.max_items == 0
            || self.max_items as usize > MAX_RECORDS
            || self.max_bytes == 0
            || self.max_bytes > 16 * 1024 * 1024
            || self.max_wait_ms == 0
            || self.max_observation_age_ms == 0
            || self.max_retry_after_ms == 0
            || self.max_children == 0
            || self.max_children as usize > MAX_RECORDS
            || self.max_active == 0
            || self.max_active > self.max_items
            || self.shares.is_empty()
            || self.shares.len() > MAX_RECORDS
            || self.shares.values().any(|share| *share == 0)
        {
            return Err(err("admission_policy", "invalid finite admission bounds"));
        }
        let mut groups = BTreeSet::new();
        if self.group_limits.len() > MAX_RECORDS
            || self.group_limits.iter().any(|g| {
                !groups.insert((g.scope, &g.group))
                    || g.max_items == 0
                    || g.max_items > self.max_items
                    || g.max_bytes == 0
                    || g.max_bytes > self.max_bytes
                    || g.max_active == 0
                    || g.max_active > g.max_items
            })
        {
            return Err(err(
                "admission_group_bounds",
                "invalid or duplicate group limits",
            ));
        }
        if let FairnessStrategy::WeightedScalar { weights } = &self.strategy
            && (weights.is_empty()
                || weights.len() > MAX_DIMENSIONS
                || weights.values().any(|weight| *weight == 0))
        {
            return Err(err(
                "admission_weights",
                "weights must be finite and positive",
            ));
        }
        if let Some(serving) = &self.serving {
            serving.validate(self)?;
        }
        Ok(())
    }
    pub fn identity(&self) -> CoreResult<ContentId> {
        self.validate()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kiln.fair-admission-policy.v1");
        hasher.update(self.accounting_basis.as_str().as_bytes());
        for n in [
            u64::from(self.version),
            u64::from(self.max_items),
            self.max_bytes,
            self.max_wait_ms,
            self.max_observation_age_ms,
            self.max_retry_after_ms,
            u64::from(self.max_children),
            u64::from(self.max_active),
        ] {
            hasher.update(&n.to_le_bytes());
        }
        hasher.update(&(self.group_limits.len() as u64).to_le_bytes());
        for group in &self.group_limits {
            hasher.update(&[match group.scope {
                QueueScope::Tenant => 0,
                QueueScope::Repository => 1,
                QueueScope::Campaign => 2,
            }]);
            hasher.update(&(group.group.as_str().len() as u64).to_le_bytes());
            hasher.update(group.group.as_str().as_bytes());
            hasher.update(&group.max_items.to_le_bytes());
            hasher.update(&group.max_bytes.to_le_bytes());
            hasher.update(&group.max_active.to_le_bytes());
        }
        match &self.strategy {
            FairnessStrategy::DominantShare => {
                hasher.update(&[0]);
            }
            FairnessStrategy::WeightedScalar { weights } => {
                hasher.update(&[1]);
                hash_amounts(&mut hasher, weights);
            }
        }
        hash_amounts(&mut hasher, &self.shares);
        if let Some(serving) = &self.serving {
            serving.hash(&mut hasher);
        }
        ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
    }
}
fn hash_amounts(hasher: &mut blake3::Hasher, amounts: &Amounts) {
    hasher.update(&(amounts.len() as u64).to_le_bytes());
    for (key, value) in amounts {
        hasher.update(&(key.as_str().len() as u64).to_le_bytes());
        hasher.update(key.as_str().as_bytes());
        hasher.update(&value.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionReason {
    Waiting,
    QueueFull,
    Expired,
    Denied,
    Incomplete,
    ProviderRejected,
    ProviderFailure,
    Unknown,
    CancellationRequested,
    Cancelled,
    Eligible,
    BudgetExhausted,
    Incompatible,
    Stale,
    ConcurrencyFull,
    /// The serving owner's aggregate scope reports no slot and a full queue.
    Overloaded,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionPhase {
    PreClaim,
    PostClaim,
    Reserved,
    DispatchIntent,
    Executing,
    Terminal,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionExplanation {
    pub work_version: ContentId,
    pub campaign: Alias,
    pub policy_revision: ContentId,
    pub phase: AdmissionPhase,
    pub reason: AdmissionReason,
    pub position: Option<u32>,
    pub eligible_at_ms: Option<u64>,
    pub held: Amounts,
    pub measured: Amounts,
    pub observation_at_ms: Option<u64>,
    #[serde(default)]
    pub lineage: Option<LineageExplanation>,
    #[serde(default)]
    pub serving: Option<ServingExplanation>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRef {
    pub attempt: ContentId,
    pub fence: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueItem {
    pub work_version: ContentId,
    pub account: ContentId,
    pub tenant: Alias,
    pub campaign: Alias,
    pub repository: Alias,
    pub priority: u8,
    pub bytes: u64,
    pub enqueued_at_ms: u64,
    pub deadline_ms: u64,
    pub eligible_at_ms: u64,
    pub claim: Option<ClaimRef>,
    pub phase: AdmissionPhase,
    pub reason: AdmissionReason,
    pub order: u64,
    /// Required owner grants frozen with this queued work contract.
    pub required_owners: BTreeMap<ProviderProfileId, ChargeKind>,
    /// Owner-confirmed shared deployment operations, referenced without another debit.
    pub shared_allocations: Vec<ContentId>,
    /// Finite aggregate reservation/request ceiling for this semantic work item.
    pub budget: Amounts,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetAccount {
    pub identity: ContentId,
    pub parent: Option<ContentId>,
    pub parent_attempt: Option<ClaimRef>,
    pub campaign: Alias,
    pub tenant: Alias,
    pub deadline_ms: u64,
    /// Transferred quota is removed from parent.available; never consumption.
    pub available: Amounts,
    pub consumed: Amounts,
    pub children: u32,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeKind {
    PhysicalDeployment,
    WorkerRequest,
    Coordination,
    Execution,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReservationState {
    Tentative,
    Committed,
    Rejected,
    Expired,
    Unknown,
    Released,
    Settled,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServingBounds {
    /// Owner's aggregate scope across callers, never an individual worker queue.
    pub scope: ContentId,
    pub max_ongoing: u32,
    pub max_queued: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerReservation {
    pub version: u16,
    pub operation: ContentId,
    pub provider: ProviderProfileId,
    pub generation: u64,
    pub work_version: ContentId,
    pub claim: ClaimRef,
    pub account: ContentId,
    pub kind: ChargeKind,
    /// Physical resource identity for physical charges; worker/request identity otherwise.
    pub subject: ContentId,
    /// Exact admitted dimension/normalization/reporting-window contract.
    pub accounting_basis: ContentId,
    pub serving_bounds: Option<ServingBounds>,
    pub amounts: Amounts,
    pub expires_at_ms: u64,
    pub state: ReservationState,
    pub receipt: Option<ContentId>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerReply {
    pub version: u16,
    pub operation: ContentId,
    pub provider: ProviderProfileId,
    pub generation: u64,
    pub receipt: ContentId,
    pub state: ReservationState,
    pub retry_at_ms: Option<u64>,
    pub serving_bounds: Option<ServingBounds>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageObservation {
    pub usage: TenantUsage,
    pub generation: u64,
    pub accounting_basis: ContentId,
    /// Exact settled operation/receipt pairs already included in measured usage.
    pub covered: BTreeMap<ContentId, ContentId>,
    pub complete: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EligibilityInput {
    pub work_version: ContentId,
    pub reason: AdmissionReason,
    pub provider: ProviderProfileId,
    pub generation: u64,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionLedger {
    pub policy: Option<AdmissionPolicy>,
    pub items: BTreeMap<ContentId, QueueItem>,
    pub accounts: BTreeMap<ContentId, BudgetAccount>,
    /// Original finite allocation, distinct from mutable available/consumed quota.
    pub quotas: BTreeMap<ContentId, Amounts>,
    pub reservations: BTreeMap<ContentId, OwnerReservation>,
    /// A caller cannot hide a known fresh competitor by omitting its observation.
    pub eligibility: BTreeMap<ContentId, EligibilityInput>,
    pub observations: BTreeMap<Alias, UsageObservation>,
    pub next_order: u64,
    /// Latest serving-owner snapshot per owner aggregate scope; evidence only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub serving: BTreeMap<ContentId, ServingObservation>,
    /// Revoked lineage accounts; revocation never refunds outstanding usage.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub revoked: BTreeSet<ContentId>,
}
impl AdmissionLedger {
    pub fn configure(&mut self, policy: AdmissionPolicy) -> CoreResult<ContentId> {
        let identity = policy.identity()?;
        if self.policy.as_ref().is_some_and(|prior| prior != &policy) {
            return Err(err(
                "admission_policy_frozen",
                "queue policy is immutable for this ledger",
            ));
        }
        self.policy = Some(policy);
        Ok(identity)
    }
    pub fn policy_ref(&self) -> CoreResult<ContentId> {
        self.policy()?.identity()
    }
    fn policy(&self) -> CoreResult<&AdmissionPolicy> {
        self.policy
            .as_ref()
            .ok_or_else(|| err("admission_disabled", "explicit policy required"))
    }
    pub fn add_root(&mut self, account: BudgetAccount) -> CoreResult<()> {
        let policy = self.policy()?;
        validate_amounts(&account.available)?;
        if account.parent.is_some()
            || account.parent_attempt.is_some()
            || account.children != 0
            || !account.consumed.is_empty()
            || !policy.shares.contains_key(&account.campaign)
        {
            return Err(err(
                "admission_root",
                "root must have explicit finite campaign quota",
            ));
        }
        if let Some(prior) = self.accounts.get(&account.identity) {
            return if prior.parent == account.parent
                && prior.parent_attempt == account.parent_attempt
                && prior.campaign == account.campaign
                && prior.tenant == account.tenant
                && prior.deadline_ms == account.deadline_ms
                && self.quotas.get(&account.identity) == Some(&account.available)
            {
                Ok(())
            } else {
                Err(err("admission_root_conflict", "cannot reset quota"))
            };
        }
        if self.accounts.len() >= MAX_RECORDS {
            return Err(err("admission_accounts_full", "account history full"));
        }
        // One root per campaign prevents new worker/queue identities minting quota.
        if self
            .accounts
            .values()
            .any(|a| a.parent.is_none() && a.campaign == account.campaign)
        {
            return Err(err(
                "admission_campaign_root",
                "campaign already has its finite root",
            ));
        }
        self.quotas
            .insert(account.identity.clone(), account.available.clone());
        self.accounts.insert(account.identity.clone(), account);
        Ok(())
    }
    pub fn transfer(&mut self, parent: &ContentId, child: BudgetAccount) -> CoreResult<()> {
        let policy = self.policy()?;
        if let Some(prior) = self.accounts.get(&child.identity) {
            return if prior.parent.as_ref() == Some(parent)
                && prior.parent == child.parent
                && prior.parent_attempt == child.parent_attempt
                && prior.campaign == child.campaign
                && prior.tenant == child.tenant
                && prior.deadline_ms == child.deadline_ms
                && self.quotas.get(&child.identity) == Some(&child.available)
            {
                Ok(())
            } else {
                Err(err("admission_child_conflict", "child identity reused"))
            };
        }
        if let Some(blocker) = self.lineage_blocker(parent, None)? {
            return Err(err(
                "admission_lineage_blocked",
                match blocker {
                    AdmissionReason::Denied => "parent lineage budget is revoked",
                    _ => "parent lineage is cancelled or expired",
                },
            ));
        }
        let source = self
            .accounts
            .get(parent)
            .ok_or_else(|| err("admission_parent", "parent missing"))?;
        validate_amounts(&child.available)?;
        if self.accounts.len() >= MAX_RECORDS
            || source.children >= policy.max_children
            || child.parent.as_ref() != Some(parent)
            || child.parent_attempt.is_none()
            || child.campaign != source.campaign
            || child.tenant != source.tenant
            || child.deadline_ms > source.deadline_ms
            || !child.consumed.is_empty()
            || child.children != 0
        {
            return Err(err(
                "admission_child_bound",
                "child exceeds parent lineage, deadline or fanout",
            ));
        }
        let remaining = subtract(&source.available, &child.available)?;
        let source = self
            .accounts
            .get_mut(parent)
            .ok_or_else(|| err("admission_parent", "parent missing"))?;
        source.available = remaining;
        source.children += 1;
        self.quotas
            .insert(child.identity.clone(), child.available.clone());
        self.accounts.insert(child.identity.clone(), child);
        Ok(())
    }
    /// `bytes` is a canonical payload bound including future claim/transition
    /// metadata, supplied and checked by the shell.
    pub fn enqueue(&mut self, mut item: QueueItem, now_ms: u64) -> CoreResult<AdmissionReason> {
        let policy = self.policy()?;
        validate_amounts(&item.budget)?;
        if let Some(prior) = self.items.get(&item.work_version) {
            return if prior.account == item.account
                && prior.campaign == item.campaign
                && prior.repository == item.repository
                && prior.deadline_ms == item.deadline_ms
                && prior.required_owners == item.required_owners
                && prior.tenant == item.tenant
                && prior.shared_allocations == item.shared_allocations
                && prior.budget == item.budget
            {
                Ok(prior.reason)
            } else {
                Err(err(
                    "admission_work_conflict",
                    "queued work cannot change lineage",
                ))
            };
        }
        if self.items.len() >= policy.max_items as usize
            || self.items.len() >= MAX_RECORDS
            || self
                .items
                .values()
                .try_fold(item.bytes, |sum, i| sum.checked_add(i.bytes))
                .is_none_or(|sum| sum > policy.max_bytes)
        {
            return Ok(AdmissionReason::QueueFull);
        }
        for group in &policy.group_limits {
            if item_group(&item, group.scope) != &group.group {
                continue;
            }
            let mut count = 1_u32;
            let mut bytes = item.bytes;
            for prior in self
                .items
                .values()
                .filter(|i| item_group(i, group.scope) == &group.group)
            {
                count += 1;
                bytes = bytes
                    .checked_add(prior.bytes)
                    .ok_or_else(|| err("admission_arithmetic", "group byte overflow"))?;
            }
            if count > group.max_items || bytes > group.max_bytes {
                return Ok(AdmissionReason::QueueFull);
            }
        }
        let account = self
            .accounts
            .get(&item.account)
            .ok_or_else(|| err("admission_account", "quota account missing"))?;
        if item.campaign != account.campaign
            || item.tenant != account.tenant
            || item.deadline_ms > account.deadline_ms
            || item.enqueued_at_ms != now_ms
            || item.bytes == 0
            || item.claim.is_some()
            || item.eligible_at_ms < now_ms
            || item.required_owners.is_empty()
            || item.required_owners.len() > MAX_DIMENSIONS
            || item.shared_allocations.len() > MAX_DIMENSIONS
        {
            return Err(err("admission_item", "invalid queue item lineage or time"));
        }
        let mut shared = BTreeSet::new();
        for operation in &item.shared_allocations {
            let allocation = self.reservations.get(operation).ok_or_else(|| {
                err(
                    "admission_shared_missing",
                    "shared physical allocation is not retained",
                )
            })?;
            if !shared.insert(operation)
                || allocation.kind != ChargeKind::PhysicalDeployment
                || allocation.state != ReservationState::Committed
                || self.accounts.get(&allocation.account).map(|a| &a.tenant) != Some(&item.tenant)
            {
                return Err(err(
                    "admission_shared_binding",
                    "shared reference is not an admitted same-tenant physical allocation",
                ));
            }
        }
        if now_ms >= item.deadline_ms {
            return Ok(AdmissionReason::Expired);
        }
        // A revoked, cancelled or expired lineage cannot queue new descendants.
        if let Some(blocker) = self.lineage_blocker(&item.account, Some(now_ms))? {
            return Ok(blocker);
        }
        self.next_order = self
            .next_order
            .checked_add(1)
            .ok_or_else(|| err("admission_order", "order exhausted"))?;
        item.order = self.next_order;
        item.phase = AdmissionPhase::PreClaim;
        item.reason = AdmissionReason::Waiting;
        self.items.insert(item.work_version.clone(), item);
        Ok(AdmissionReason::Waiting)
    }
    pub fn observe(&mut self, observation: UsageObservation) -> CoreResult<()> {
        self.policy()?;
        if observation.covered.len() > MAX_RECORDS {
            return Err(err("admission_usage_bound", "usage coverage exceeds bound"));
        }
        let campaign = &observation.usage.tenant;
        let policy = self.policy()?;
        if policy.shares.get(campaign) != Some(&observation.usage.share)
            || observation.accounting_basis != policy.accounting_basis
            || (observation.complete && observation.usage.resources.is_empty())
            || observation.usage.resources.len() > MAX_DIMENSIONS
        {
            return Err(err(
                "admission_usage",
                "usage does not match explicit campaign or dimensions",
            ));
        }
        if !observation.complete {
            if let Some(prior) = self.observations.get_mut(campaign) {
                if observation.generation < prior.generation {
                    return Err(err(
                        "admission_usage_generation",
                        "partial page generation regressed",
                    ));
                }
                prior.complete = false;
                prior.generation = observation.generation;
            } else {
                self.observations.insert(campaign.clone(), observation);
            }
            return Ok(());
        }
        if self.observations.values().any(|prior| {
            prior.complete
                && prior.usage.tenant != *campaign
                && (prior.usage.interval_start != observation.usage.interval_start
                    || prior.usage.resources.len() != observation.usage.resources.len()
                    || prior.usage.resources.iter().any(|(d, v)| {
                        observation
                            .usage
                            .resources
                            .get(d)
                            .is_none_or(|next| next.capacity != v.capacity)
                    }))
        }) {
            return Err(err(
                "admission_capacity_basis",
                "campaigns require identical dimension and capacity normalization basis",
            ));
        }
        if let Some(prior) = self.observations.get(campaign) {
            if observation.generation == prior.generation && &observation != prior {
                return Err(err(
                    "admission_usage_ambiguous",
                    "same generation cannot change measured usage",
                ));
            }
            if observation.generation < prior.generation
                || observation.usage.provider != prior.usage.provider
                || observation.usage.interval_start != prior.usage.interval_start
                || observation.accounting_basis != prior.accounting_basis
            {
                return Err(err(
                    "admission_usage_generation",
                    "accounting window/provider cannot reset historical usage",
                ));
            }
            validate_usage_sequence(&[prior.usage.clone(), observation.usage.clone()])?;
            if prior
                .usage
                .resources
                .keys()
                .any(|dimension| !observation.usage.resources.contains_key(dimension))
                || prior
                    .covered
                    .iter()
                    .any(|(op, receipt)| observation.covered.get(op) != Some(receipt))
            {
                return Err(err(
                    "admission_usage_regression",
                    "usage cannot forget dimensions or covered receipts",
                ));
            }
        }
        let mut covered_amounts = Amounts::new();
        for (op, receipt) in &observation.covered {
            let reservation = self
                .reservations
                .get(op)
                .ok_or_else(|| err("admission_coverage", "coverage names unknown operation"))?;
            if reservation.state != ReservationState::Settled
                || reservation.receipt.as_ref() != Some(receipt)
                || reservation.accounting_basis != observation.accounting_basis
                || self.accounts.get(&reservation.account).map(|a| &a.campaign) != Some(campaign)
            {
                return Err(err(
                    "admission_coverage",
                    "coverage must name exact settled owner receipt",
                ));
            }
            covered_amounts = add(&covered_amounts, &reservation.amounts)?;
        }
        if covered_amounts.iter().any(|(d, amount)| {
            observation
                .usage
                .resources
                .get(d)
                .is_none_or(|v| v.amount < *amount)
        }) {
            return Err(err(
                "admission_coverage_amount",
                "measured usage cannot cover more settled consumption than reported",
            ));
        }
        self.observations.insert(campaign.clone(), observation);
        Ok(())
    }
    /// Compose historical measured usage plus uncovered held/conservative debit.
    /// Only explicit settled-receipt coverage removes a debit, never elapsed time.
    pub fn composed_usage(&self, campaign: &Alias) -> CoreResult<TenantUsage> {
        let observation = self
            .observations
            .get(campaign)
            .ok_or_else(|| err("admission_missing_usage", "no complete usage observation"))?;
        if !observation.complete {
            return Err(err(
                "admission_incomplete_usage",
                "usage snapshot is incomplete; retained usage is not reset",
            ));
        }
        let mut usage = observation.usage.clone();
        for reservation in self.reservations.values().filter(|r| {
            matches!(
                r.state,
                ReservationState::Committed | ReservationState::Unknown | ReservationState::Settled
            )
        }) {
            if self.accounts.get(&reservation.account).map(|a| &a.campaign) != Some(campaign) {
                continue;
            }
            if reservation.accounting_basis != observation.accounting_basis {
                return Err(err(
                    "admission_incompatible_basis",
                    "cannot combine incompatible capacity/usage dimensions",
                ));
            }
            if observation.covered.get(&reservation.operation) == reservation.receipt.as_ref()
                && reservation.receipt.is_some()
            {
                continue;
            }
            for (dimension, amount) in &reservation.amounts {
                let value = usage.resources.get_mut(dimension).ok_or_else(|| {
                    err(
                        "admission_missing_dimension",
                        "held debit has no compatible measured capacity basis",
                    )
                })?;
                value.amount = value
                    .amount
                    .checked_add(*amount)
                    .ok_or_else(|| err("admission_arithmetic", "held plus measured overflow"))?;
            }
        }
        Ok(usage)
    }
    pub fn select(
        &mut self,
        inputs: &[EligibilityInput],
        now_ms: u64,
    ) -> CoreResult<Option<ContentId>> {
        self.select_scoped(inputs, now_ms, None)
    }
    /// Rank only work inside an authoritative operation scope while retaining
    /// the complete ledger's usage, quota, eligibility, and order facts.
    pub fn select_scoped(
        &mut self,
        inputs: &[EligibilityInput],
        now_ms: u64,
        work_scope: Option<&ContentId>,
    ) -> CoreResult<Option<ContentId>> {
        self.policy()?;
        if inputs.len() > MAX_RECORDS {
            return Err(err("admission_inputs_full", "eligibility input bound"));
        }
        let mut unique = BTreeSet::new();
        if inputs.iter().any(|i| !unique.insert(&i.work_version)) {
            return Err(err(
                "admission_ambiguous_offer",
                "duplicate work eligibility",
            ));
        }
        for input in inputs {
            let item = self
                .items
                .get(&input.work_version)
                .ok_or_else(|| err("admission_work", "eligibility names unqueued work"))?;
            if !item.required_owners.contains_key(&input.provider) {
                return Err(err(
                    "admission_offer_owner",
                    "eligibility does not name a required owner",
                ));
            }
            if self
                .eligibility
                .get(&input.work_version)
                .is_some_and(|prior| {
                    input.observed_at_ms < prior.observed_at_ms
                        || (input.provider == prior.provider && input.generation < prior.generation)
                })
            {
                continue;
            }
            self.eligibility
                .insert(input.work_version.clone(), input.clone());
        }
        self.select_ranked(now_ms, work_scope)
    }
    fn select_ranked(
        &mut self,
        now_ms: u64,
        work_scope: Option<&ContentId>,
    ) -> CoreResult<Option<ContentId>> {
        let policy = self.policy()?.clone();
        // Computed before mutation: lineage blockers per account and the
        // provider-level serving gate (None when the serving policy is off).
        let mut blockers = BTreeMap::new();
        let mut gates = BTreeMap::new();
        for item in self.items.values() {
            if !blockers.contains_key(&item.account) {
                blockers.insert(
                    item.account.clone(),
                    self.lineage_blocker(&item.account, Some(now_ms))?,
                );
            }
            for (owner, kind) in &item.required_owners {
                if *kind == ChargeKind::WorkerRequest
                    && !gates.contains_key(owner)
                    && let Some(gate) = self.provider_serving_gate(owner, now_ms)
                {
                    gates.insert(owner.clone(), gate);
                }
            }
        }
        let mut eligible = Vec::new();
        for (id, item) in &mut self.items {
            if !matches!(
                item.phase,
                AdmissionPhase::PreClaim | AdmissionPhase::PostClaim | AdmissionPhase::Reserved
            ) {
                continue;
            }
            if item.reason == AdmissionReason::CancellationRequested {
                continue;
            }
            // Lineage blockers precede the fully-reserved skip so explanations
            // show them; the item's own deadline still expires it below.
            if now_ms < item.deadline_ms
                && let Some(Some(blocker)) = blockers.get(&item.account)
            {
                item.reason = *blocker;
                continue;
            }
            if item.phase == AdmissionPhase::Reserved
                && item.required_owners.iter().all(|(owner, kind)| {
                    self.reservations.values().any(|r| {
                        r.work_version == *id
                            && &r.provider == owner
                            && &r.kind == kind
                            && r.state == ReservationState::Committed
                    })
                })
            {
                continue;
            }
            if now_ms >= item.deadline_ms
                || now_ms.saturating_sub(item.enqueued_at_ms) > policy.max_wait_ms
            {
                item.reason = AdmissionReason::Expired;
                item.phase = AdmissionPhase::Terminal;
                continue;
            }
            let Some(input) = self.eligibility.get(id) else {
                item.reason = AdmissionReason::Incomplete;
                continue;
            };
            if input.observed_at_ms > now_ms
                || now_ms.saturating_sub(input.observed_at_ms) > policy.max_observation_age_ms
                || now_ms >= input.valid_until_ms
            {
                item.reason = AdmissionReason::Stale;
                continue;
            }
            item.reason = input.reason;
            let budget_available = self.accounts.get(&item.account).is_some_and(|account| {
                item.budget.iter().all(|(d, needed)| {
                    let own_held = self
                        .reservations
                        .values()
                        .filter(|r| {
                            r.work_version == *id
                                && matches!(
                                    r.state,
                                    ReservationState::Tentative
                                        | ReservationState::Committed
                                        | ReservationState::Unknown
                                )
                        })
                        .try_fold(0_u64, |sum, r| {
                            sum.checked_add(r.amounts.get(d).copied().unwrap_or(0))
                        });
                    own_held
                        .and_then(|held| {
                            account
                                .available
                                .get(d)
                                .copied()
                                .unwrap_or(0)
                                .checked_add(held)
                        })
                        .is_some_and(|available| available >= *needed)
                })
            });
            if !budget_available {
                item.reason = AdmissionReason::BudgetExhausted;
                continue;
            }
            // Serving backpressure applies before claim and before each new
            // request; an owner operation already retained is not re-gated.
            let serving_block = item
                .required_owners
                .iter()
                .filter(|(owner, kind)| {
                    **kind == ChargeKind::WorkerRequest
                        && !self.reservations.values().any(|r| {
                            r.work_version == *id
                                && &r.provider == *owner
                                && matches!(
                                    r.state,
                                    ReservationState::Tentative
                                        | ReservationState::Committed
                                        | ReservationState::Unknown
                                )
                        })
                })
                .filter_map(|(owner, _)| gates.get(owner))
                .find(|gate| **gate != AdmissionReason::Eligible);
            if input.reason == AdmissionReason::Eligible
                && let Some(gate) = serving_block
            {
                item.reason = *gate;
                continue;
            }
            if input.reason == AdmissionReason::Eligible
                && now_ms >= item.eligible_at_ms
                && work_scope.is_none_or(|scope| scope == id)
            {
                eligible.push(id.clone());
            }
        }
        let mut campaigns = BTreeSet::new();
        for id in &eligible {
            campaigns.insert(
                self.items
                    .get(id)
                    .ok_or_else(|| err("admission_work", "eligible work disappeared"))?
                    .campaign
                    .clone(),
            );
        }
        let mut usage = Vec::new();
        let mut valid_campaigns = BTreeSet::new();
        // A missing/stale campaign does not block an independently observed one.
        for campaign in campaigns {
            match self.composed_usage(&campaign).and_then(|u| {
                rank_tenants(
                    core::slice::from_ref(&u),
                    &policy.strategy,
                    now_ms,
                    policy.max_observation_age_ms,
                )?;
                Ok(u)
            }) {
                Ok(u) => {
                    valid_campaigns.insert(campaign);
                    usage.push(u);
                }
                Err(_) => {
                    for item in self
                        .items
                        .values_mut()
                        .filter(|i| i.campaign == campaign && i.reason == AdmissionReason::Eligible)
                    {
                        item.reason = AdmissionReason::Incomplete;
                    }
                }
            }
        }
        let ranked = rank_tenants(
            &usage,
            &policy.strategy,
            now_ms,
            policy.max_observation_age_ms,
        )?;
        eligible.retain(|id| {
            self.items
                .get(id)
                .is_some_and(|item| valid_campaigns.contains(&item.campaign))
        });
        eligible.sort_by(|a, b| {
            let a = self.items.get(a).unwrap_or_else(|| unreachable!());
            let b = self.items.get(b).unwrap_or_else(|| unreachable!());
            ranked
                .iter()
                .position(|c| c == &a.campaign)
                .cmp(&ranked.iter().position(|c| c == &b.campaign))
                .then(b.priority.cmp(&a.priority))
                .then(a.order.cmp(&b.order))
                .then(a.work_version.cmp(&b.work_version))
        });
        for id in eligible {
            let item = self
                .items
                .get(&id)
                .ok_or_else(|| err("admission_work", "eligible work disappeared"))?;
            let already_active = self.reservations.values().any(|r| {
                r.work_version == id
                    && matches!(
                        r.state,
                        ReservationState::Tentative
                            | ReservationState::Committed
                            | ReservationState::Unknown
                    )
            });
            if (already_active || self.active_for(&item.campaign) < policy.max_active as usize)
                && self.group_active_available(item, &policy)
            {
                return Ok(Some(id));
            }
            self.items
                .get_mut(&id)
                .ok_or_else(|| err("admission_work", "work missing"))?
                .reason = AdmissionReason::ConcurrencyFull;
        }
        Ok(None)
    }
    fn active_for(&self, campaign: &Alias) -> usize {
        self.reservations
            .values()
            .filter(|r| {
                matches!(
                    r.state,
                    ReservationState::Tentative
                        | ReservationState::Committed
                        | ReservationState::Unknown
                ) && self.accounts.get(&r.account).map(|a| &a.campaign) == Some(campaign)
            })
            .map(|r| &r.work_version)
            .collect::<BTreeSet<_>>()
            .len()
    }
    fn group_active_available(&self, item: &QueueItem, policy: &AdmissionPolicy) -> bool {
        policy.group_limits.iter().all(|group| {
            if item_group(item, group.scope) != &group.group {
                return true;
            }
            let active: BTreeSet<_> = self
                .reservations
                .values()
                .filter(|r| {
                    matches!(
                        r.state,
                        ReservationState::Tentative
                            | ReservationState::Committed
                            | ReservationState::Unknown
                    ) && self
                        .items
                        .get(&r.work_version)
                        .is_some_and(|i| item_group(i, group.scope) == &group.group)
                })
                .map(|r| &r.work_version)
                .collect();
            active.contains(&item.work_version) || active.len() < group.max_active as usize
        })
    }
    pub fn bind_claim(&mut self, work: &ContentId, claim: ClaimRef) -> CoreResult<()> {
        if claim.fence == 0 {
            return Err(err("admission_fence", "zero fence"));
        }
        let item = self
            .items
            .get_mut(work)
            .ok_or_else(|| err("admission_work", "work not queued"))?;
        if item.phase == AdmissionPhase::Terminal
            || item.claim.as_ref().is_some_and(|prior| prior != &claim)
        {
            return Err(err(
                "admission_claim",
                "cannot replace consumed attempt with queue waiting",
            ));
        }
        item.claim = Some(claim);
        item.phase = AdmissionPhase::PostClaim;
        Ok(())
    }
    /// Persist this tentative request BEFORE asking the owner. It is not a grant.
    pub fn request(&mut self, reservation: OwnerReservation, now_ms: u64) -> CoreResult<()> {
        let policy = self.policy()?;
        if let Some(prior) = self.reservations.get(&reservation.operation) {
            return if prior == &reservation {
                Ok(())
            } else {
                Err(err(
                    "admission_operation_conflict",
                    "operation already retained",
                ))
            };
        }
        if self.reservations.len() >= MAX_RECORDS {
            return Err(err("admission_reservations_full", "operation history full"));
        }
        validate_amounts(&reservation.amounts)?;
        if reservation.kind == ChargeKind::WorkerRequest
            && reservation.serving_bounds.as_ref().is_none_or(|b| {
                b.max_ongoing == 0
                    || b.max_ongoing > MAX_SERVING_REQUESTS
                    || b.max_queued > MAX_SERVING_REQUESTS
            })
        {
            return Err(err(
                "admission_serving_scope",
                "worker requests require explicit finite owner-aggregate serving bounds",
            ));
        }
        if self
            .lineage_blocker(&reservation.account, Some(now_ms))?
            .is_some()
        {
            return Err(err(
                "admission_lineage_blocked",
                "revoked, cancelled or expired lineage cannot request owner capacity",
            ));
        }
        if reservation.kind == ChargeKind::WorkerRequest
            && self
                .request_serving_gate(&reservation, now_ms)
                .is_some_and(|gate| gate != AdmissionReason::Eligible)
        {
            return Err(err(
                "admission_serving_backpressure",
                "serving owner snapshot is stale, missing, not ready or overloaded",
            ));
        }
        let item = self
            .items
            .get(&reservation.work_version)
            .ok_or_else(|| err("admission_work", "work not queued"))?;
        if reservation.version != ADMISSION_VERSION
            || reservation.state != ReservationState::Tentative
            || reservation.receipt.is_some()
            || item.claim.as_ref() != Some(&reservation.claim)
            || item.account != reservation.account
            || now_ms >= item.deadline_ms
            || now_ms >= reservation.expires_at_ms
            || reservation.expires_at_ms > item.deadline_ms
            || !matches!(
                item.phase,
                AdmissionPhase::PostClaim | AdmissionPhase::Reserved
            )
            || item.reason != AdmissionReason::Eligible
            || item.required_owners.get(&reservation.provider) != Some(&reservation.kind)
            || reservation.accounting_basis != policy.accounting_basis
            || !self.group_active_available(item, policy)
            || (self.active_for(&item.campaign) >= policy.max_active as usize
                && !self.reservations.values().any(|r| {
                    r.work_version == item.work_version
                        && matches!(
                            r.state,
                            ReservationState::Tentative
                                | ReservationState::Committed
                                | ReservationState::Unknown
                        )
                }))
        {
            return Err(err(
                "admission_request",
                "request exceeds claim, deadline, eligibility or concurrency",
            ));
        }
        let mut work_debit = reservation.amounts.clone();
        for prior in self.reservations.values().filter(|r| {
            r.work_version == item.work_version
                && !matches!(
                    r.state,
                    ReservationState::Rejected
                        | ReservationState::Expired
                        | ReservationState::Released
                )
        }) {
            work_debit = add(&work_debit, &prior.amounts)?;
        }
        subtract(&item.budget, &work_debit)?;
        if reservation.kind == ChargeKind::PhysicalDeployment
            && self.reservations.values().any(|r| {
                r.kind == ChargeKind::PhysicalDeployment
                    && r.provider == reservation.provider
                    && r.subject == reservation.subject
                    && !matches!(
                        r.state,
                        ReservationState::Rejected
                            | ReservationState::Expired
                            | ReservationState::Released
                    )
            })
        {
            return Err(err(
                "admission_shared_physical",
                "deployment physical allocation already charged; attach reference, not another charge",
            ));
        }
        let account = self
            .accounts
            .get(&reservation.account)
            .ok_or_else(|| err("admission_account", "account missing"))?;
        let available = subtract(&account.available, &reservation.amounts)?;
        self.accounts
            .get_mut(&reservation.account)
            .ok_or_else(|| err("admission_account", "account missing"))?
            .available = available;
        self.reservations
            .insert(reservation.operation.clone(), reservation);
        Ok(())
    }
    pub fn owner_reply(&mut self, reply: &OwnerReply, now_ms: u64) -> CoreResult<AdmissionReason> {
        let policy = self.policy()?;
        if reply
            .retry_at_ms
            .is_some_and(|at| at < now_ms || at - now_ms > policy.max_retry_after_ms)
        {
            return Err(err(
                "admission_retry_after",
                "owner eligibility delay exceeds named bound",
            ));
        }
        let prior = self
            .reservations
            .get(&reply.operation)
            .ok_or_else(|| err("admission_operation", "unknown operation"))?
            .clone();
        if reply.version != ADMISSION_VERSION
            || prior.provider != reply.provider
            || prior.generation != reply.generation
            || prior.serving_bounds != reply.serving_bounds
        {
            return Err(err(
                "admission_owner_generation",
                "reply crossed operation/provider/generation",
            ));
        }
        if prior.receipt.as_ref() == Some(&reply.receipt) && prior.state == reply.state {
            return Ok(state_reason(prior.state));
        }
        if prior.state != ReservationState::Tentative && prior.state != ReservationState::Unknown {
            return Err(err(
                "admission_receipt_conflict",
                "owner reply cannot replace a final or committed receipt",
            ));
        }
        if !matches!(
            reply.state,
            ReservationState::Committed
                | ReservationState::Rejected
                | ReservationState::Expired
                | ReservationState::Unknown
        ) {
            return Err(err(
                "admission_reply_state",
                "invalid reservation acknowledgement",
            ));
        }
        // A late grant is retained as unknown; it cannot revive cancelled/expired work.
        if prior.state == ReservationState::Unknown
            && matches!(
                reply.state,
                ReservationState::Rejected | ReservationState::Expired
            )
        {
            return Err(err(
                "admission_reconciliation_required",
                "unknown effect needs definitive usage settlement, not reservation refund",
            ));
        }
        let item = self
            .items
            .get(&prior.work_version)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        if item.phase == AdmissionPhase::DispatchIntent && reply.state != ReservationState::Unknown
        {
            return Err(err(
                "admission_reconciliation_required",
                "possible dispatch requires authoritative settlement, not reservation rejection/refund",
            ));
        }
        let late = now_ms >= item.deadline_ms
            || item.phase == AdmissionPhase::Terminal
            || matches!(
                item.reason,
                AdmissionReason::CancellationRequested
                    | AdmissionReason::Cancelled
                    | AdmissionReason::Expired
            );
        let cancellation_observed = item.reason == AdmissionReason::Cancelled;
        let state = if late && reply.state == ReservationState::Committed {
            ReservationState::Unknown
        } else {
            reply.state
        };
        // Only owner-confirmed rejection/expiry before grant is a safe refund.
        if matches!(
            state,
            ReservationState::Rejected | ReservationState::Expired
        ) {
            let account = self
                .accounts
                .get_mut(&prior.account)
                .ok_or_else(|| err("admission_account", "account missing"))?;
            account.available = add(&account.available, &prior.amounts)?;
        }
        let reservation = self
            .reservations
            .get_mut(&reply.operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?;
        reservation.state = state;
        reservation.receipt = Some(reply.receipt.clone());
        let item = self
            .items
            .get_mut(&prior.work_version)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        item.reason = if cancellation_observed {
            AdmissionReason::Cancelled
        } else {
            state_reason(state)
        };
        if !late && state == ReservationState::Committed {
            item.phase = AdmissionPhase::Reserved;
        }
        if let Some(at) = reply.retry_at_ms {
            item.eligible_at_ms = at;
        }
        Ok(item.reason)
    }
    /// The shell checks fresh current claim and work eligibility in the SAME transaction.
    /// Persisting intent makes a subsequent crash unknown; never blind redispatch.
    pub fn dispatch_intent(
        &mut self,
        operation: &ContentId,
        policy: &ContentId,
        claim: &ClaimRef,
        generation: u64,
        now_ms: u64,
    ) -> CoreResult<()> {
        if &self.policy_ref()? != policy {
            return Err(err("admission_policy_stale", "policy changed"));
        }
        let reservation = self
            .reservations
            .get(operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?;
        let item = self
            .items
            .get(&reservation.work_version)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        if self
            .lineage_blocker(&reservation.account, Some(now_ms))?
            .is_some()
        {
            return Err(err(
                "admission_lineage_blocked",
                "revoked, cancelled or expired lineage cannot dispatch",
            ));
        }
        if reservation.state != ReservationState::Committed
            || &reservation.claim != claim
            || reservation.generation != generation
            || now_ms >= reservation.expires_at_ms
            || now_ms >= item.deadline_ms
            || !((item.phase == AdmissionPhase::Reserved
                && item.reason == AdmissionReason::Eligible)
                || (item.phase == AdmissionPhase::DispatchIntent
                    && item.reason == AdmissionReason::Unknown))
        {
            return Err(err(
                "admission_dispatch_stale",
                "grant, claim, policy or deadline no longer permits dispatch",
            ));
        }
        let work = reservation.work_version.clone();
        self.reservations
            .get_mut(operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?
            .state = ReservationState::Unknown;
        let item = self
            .items
            .get_mut(&work)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        item.phase = AdmissionPhase::DispatchIntent;
        item.reason = AdmissionReason::Unknown;
        Ok(())
    }
    pub fn execution_observed(&mut self, work: &ContentId, claim: &ClaimRef) -> CoreResult<()> {
        let item = self
            .items
            .get_mut(work)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        if item.claim.as_ref() != Some(claim) {
            return Err(err("admission_claim", "acknowledgement crossed claim"));
        }
        if item.phase == AdmissionPhase::Terminal {
            return Ok(());
        }
        if item.phase != AdmissionPhase::DispatchIntent && item.phase != AdmissionPhase::Executing {
            return Err(err(
                "admission_execution",
                "execution acknowledgement needs persisted dispatch intent",
            ));
        }
        item.phase = AdmissionPhase::Executing;
        item.reason = AdmissionReason::Eligible;
        for r in self.reservations.values_mut().filter(|r| {
            &r.work_version == work
                && r.kind != ChargeKind::PhysicalDeployment
                && r.state == ReservationState::Unknown
        }) {
            r.state = ReservationState::Committed;
        }
        Ok(())
    }
    /// Transport classification is supplied by the owning port. Only a
    /// definite before-effect refusal can release a tentative quota lock.
    pub fn owner_outcome(
        &mut self,
        operation: &ContentId,
        provider: &ProviderProfileId,
        generation: u64,
        reason: AdmissionReason,
        retry_at_ms: Option<u64>,
        now_ms: u64,
    ) -> CoreResult<AdmissionReason> {
        let policy = self.policy()?;
        if retry_at_ms.is_some_and(|at| at < now_ms || at - now_ms > policy.max_retry_after_ms) {
            return Err(err(
                "admission_retry_after",
                "eligibility delay exceeds policy",
            ));
        }
        let prior = self
            .reservations
            .get(operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?
            .clone();
        if &prior.provider != provider || prior.generation != generation {
            return Err(err(
                "admission_owner_generation",
                "outcome crossed owner generation",
            ));
        }
        if !matches!(
            reason,
            AdmissionReason::Waiting
                | AdmissionReason::ProviderRejected
                | AdmissionReason::Expired
                | AdmissionReason::ProviderFailure
                | AdmissionReason::Unknown
                | AdmissionReason::CancellationRequested
                | AdmissionReason::Cancelled
        ) {
            return Err(err("admission_outcome", "unsupported owner outcome"));
        }
        let safe = prior.state == ReservationState::Tentative
            && matches!(
                reason,
                AdmissionReason::Waiting
                    | AdmissionReason::ProviderRejected
                    | AdmissionReason::Expired
                    | AdmissionReason::ProviderFailure
            );
        if safe {
            let account = self
                .accounts
                .get_mut(&prior.account)
                .ok_or_else(|| err("admission_account", "account missing"))?;
            account.available = add(&account.available, &prior.amounts)?;
        } else if !matches!(
            prior.state,
            ReservationState::Tentative | ReservationState::Committed | ReservationState::Unknown
        ) {
            return Err(err(
                "admission_outcome_final",
                "cannot change a final owner operation",
            ));
        }
        let state = if safe {
            ReservationState::Rejected
        } else {
            ReservationState::Unknown
        };
        self.reservations
            .get_mut(operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?
            .state = state;
        let item = self
            .items
            .get_mut(&prior.work_version)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        item.reason = if item.reason == AdmissionReason::Cancelled {
            AdmissionReason::Cancelled
        } else if safe
            || matches!(
                reason,
                AdmissionReason::CancellationRequested | AdmissionReason::Cancelled
            )
        {
            reason
        } else {
            AdmissionReason::Unknown
        };
        if matches!(
            reason,
            AdmissionReason::CancellationRequested | AdmissionReason::Cancelled
        ) {
            item.phase = AdmissionPhase::Terminal;
        }
        if safe && item.phase != AdmissionPhase::Terminal {
            item.phase = AdmissionPhase::PostClaim;
        }
        if let Some(at) = retry_at_ms {
            item.eligible_at_ms = at;
        }
        Ok(item.reason)
    }
    pub fn cancel(&mut self, work: &ContentId, expired: bool) -> CoreResult<AdmissionReason> {
        let item = self
            .items
            .get_mut(work)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        if item.reason == AdmissionReason::Cancelled {
            return Ok(AdmissionReason::Cancelled);
        }
        let pending = self.reservations.values().any(|r| {
            &r.work_version == work
                && matches!(
                    r.state,
                    ReservationState::Tentative
                        | ReservationState::Committed
                        | ReservationState::Unknown
                )
        });
        item.reason = if pending {
            AdmissionReason::CancellationRequested
        } else if expired {
            AdmissionReason::Expired
        } else {
            AdmissionReason::Cancelled
        };
        item.phase = AdmissionPhase::Terminal;
        Ok(item.reason)
    }
    /// Exact authoritative termination/usage receipt. Unknown is never refunded by timeout.
    pub fn settle(
        &mut self,
        operation: &ContentId,
        provider: &ProviderProfileId,
        generation: u64,
        receipt: ContentId,
        consumed: Amounts,
    ) -> CoreResult<()> {
        let prior = self
            .reservations
            .get(operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?
            .clone();
        if &prior.provider != provider || prior.generation != generation {
            return Err(err("admission_settlement_owner", "wrong owner generation"));
        }
        if prior.state == ReservationState::Settled {
            return if prior.receipt.as_ref() == Some(&receipt) && prior.amounts == consumed {
                Ok(())
            } else {
                Err(err(
                    "admission_settlement_conflict",
                    "settlement receipt conflict",
                ))
            };
        }
        if !matches!(
            prior.state,
            ReservationState::Committed | ReservationState::Unknown
        ) {
            return Err(err(
                "admission_settlement_state",
                "operation not owner admitted",
            ));
        }
        let refund = subtract(&prior.amounts, &consumed)?;
        let account = self
            .accounts
            .get(&prior.account)
            .ok_or_else(|| err("admission_account", "account missing"))?;
        let available = add(&account.available, &refund)?;
        let total_consumed = add(&account.consumed, &consumed)?;
        let account = self
            .accounts
            .get_mut(&prior.account)
            .ok_or_else(|| err("admission_account", "account missing"))?;
        account.available = available;
        account.consumed = total_consumed;
        let reservation = self
            .reservations
            .get_mut(operation)
            .ok_or_else(|| err("admission_operation", "operation missing"))?;
        reservation.state = ReservationState::Settled;
        reservation.receipt = Some(receipt);
        reservation.amounts = consumed;
        let work = prior.work_version;
        if !self.reservations.values().any(|r| {
            r.work_version == work
                && matches!(
                    r.state,
                    ReservationState::Tentative
                        | ReservationState::Committed
                        | ReservationState::Unknown
                )
        }) {
            let item = self
                .items
                .get_mut(&work)
                .ok_or_else(|| err("admission_work", "work missing"))?;
            item.phase = AdmissionPhase::Terminal;
            if item.reason == AdmissionReason::CancellationRequested {
                item.reason = AdmissionReason::Cancelled;
            }
        }
        Ok(())
    }
    /// Display-only explanation at `now_ms`; it grants and refreshes nothing.
    pub fn explanation(&self, work: &ContentId, now_ms: u64) -> CoreResult<AdmissionExplanation> {
        let item = self
            .items
            .get(work)
            .ok_or_else(|| err("admission_work", "work missing"))?;
        let observation = self.observations.get(&item.campaign);
        let mut held = Amounts::new();
        for r in self.reservations.values().filter(|r| {
            &r.work_version == work
                && matches!(
                    r.state,
                    ReservationState::Committed | ReservationState::Unknown
                )
        }) {
            held = add(&held, &r.amounts)?;
        }
        Ok(AdmissionExplanation {
            work_version: work.clone(),
            campaign: item.campaign.clone(),
            policy_revision: self.policy_ref()?,
            phase: item.phase,
            reason: item.reason,
            position: if matches!(
                item.phase,
                AdmissionPhase::PreClaim | AdmissionPhase::PostClaim
            ) {
                Some(
                    self.items
                        .values()
                        .filter(|i| {
                            i.campaign == item.campaign
                                && i.order < item.order
                                && matches!(
                                    i.phase,
                                    AdmissionPhase::PreClaim | AdmissionPhase::PostClaim
                                )
                        })
                        .count() as u32,
                )
            } else {
                None
            },
            eligible_at_ms: Some(item.eligible_at_ms),
            held,
            measured: observation
                .map(|o| {
                    o.usage
                        .resources
                        .iter()
                        .map(|(d, v)| (d.clone(), v.amount))
                        .collect()
                })
                .unwrap_or_default(),
            observation_at_ms: observation.map(|o| o.usage.observed_at),
            lineage: Some(self.lineage_explanation(&item.account, now_ms)?),
            serving: self.serving_explanation(item, now_ms),
        })
    }
}
fn state_reason(state: ReservationState) -> AdmissionReason {
    match state {
        ReservationState::Tentative => AdmissionReason::Waiting,
        ReservationState::Committed => AdmissionReason::Eligible,
        ReservationState::Rejected => AdmissionReason::ProviderRejected,
        ReservationState::Expired => AdmissionReason::Expired,
        ReservationState::Unknown => AdmissionReason::Unknown,
        ReservationState::Released | ReservationState::Settled => AdmissionReason::Cancelled,
    }
}
fn validate_amounts(values: &Amounts) -> CoreResult<()> {
    if values.is_empty() || values.len() > MAX_DIMENSIONS || values.values().any(|v| *v == 0) {
        return Err(err(
            "admission_amounts",
            "explicit positive finite resource dimensions required",
        ));
    }
    Ok(())
}
fn subtract(available: &Amounts, debit: &Amounts) -> CoreResult<Amounts> {
    let mut result = available.clone();
    for (dimension, amount) in debit {
        let value = result
            .get_mut(dimension)
            .ok_or_else(|| err("admission_budget_dimension", "budget dimension missing"))?;
        *value = value.checked_sub(*amount).ok_or_else(|| {
            err(
                "admission_budget_exhausted",
                "finite lineage quota exhausted",
            )
        })?;
    }
    Ok(result)
}
fn add(left: &Amounts, right: &Amounts) -> CoreResult<Amounts> {
    let mut result = left.clone();
    for (dimension, amount) in right {
        let value = result.entry(dimension.clone()).or_insert(0);
        *value = value
            .checked_add(*amount)
            .ok_or_else(|| err("admission_arithmetic", "quota or usage overflow"))?;
    }
    Ok(result)
}
fn err(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, "admission", message)
}

fn item_group(item: &QueueItem, scope: QueueScope) -> &Alias {
    match scope {
        QueueScope::Tenant => &item.tenant,
        QueueScope::Repository => &item.repository,
        QueueScope::Campaign => &item.campaign,
    }
}

#[cfg(test)]
mod tests;

// r[impl kiln.admission.lineage]
//! Recursive budget lineage: revocation, ancestor blockers and a
//! conservation-checked usage view over one root's subtree.
//!
//! Transfers are subtractive, so new child, call, worker, batch or nesting
//! identities cannot mint quota. `lineage_usage` makes that observable.
use super::{
    ADMISSION_VERSION, AdmissionLedger, AdmissionPhase, AdmissionReason, Amounts, ChargeKind,
    MAX_RECORDS, ReservationState, add, err, subtract, validate_amounts,
};
use crate::types::{Alias, ContentId, CoreResult};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::ToString,
    vec::Vec,
};
use serde::{Deserialize, Serialize};

/// Finite nesting bound shared with `config/fair-admission-lineage.ncl`.
pub const MAX_LINEAGE_DEPTH: u32 = 8;
const ROOT_NAME: &str = "root";

/// Typed recursive budget profile exported from Nickel. It grants nothing;
/// each transfer still needs a current parent attempt in the store.
// r[impl kiln.admission.profiles]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageProfile {
    pub version: u16,
    pub campaign: Alias,
    pub tenant: Alias,
    pub max_depth: u32,
    pub max_children: u32,
    pub root: LineageRootProfile,
    pub accounts: Vec<LineageAccountProfile>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageRootProfile {
    pub deadline_ms: u64,
    pub quota: Amounts,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageAccountProfile {
    pub name: Alias,
    pub parent: Alias,
    pub deadline_ms: u64,
    pub allocation: Amounts,
}
impl LineageProfile {
    /// Re-admit the export with the Nickel contract's bounds: parents precede
    /// children, allocations fit the parent's remainder, deadlines nest, and
    /// depth and fan-out stay finite.
    pub fn validate(&self) -> CoreResult<()> {
        if self.version != ADMISSION_VERSION
            || self.max_depth == 0
            || self.max_depth > MAX_LINEAGE_DEPTH
            || self.max_children == 0
            || self.max_children as usize > MAX_RECORDS
            || self.accounts.len() >= MAX_RECORDS
            || self.root.deadline_ms == 0
        {
            return Err(err("admission_lineage_profile", "invalid lineage bounds"));
        }
        validate_amounts(&self.root.quota)?;
        // name -> (depth, deadline, remaining, children)
        let mut nodes = BTreeMap::from([(
            ROOT_NAME.to_string(),
            (0_u32, self.root.deadline_ms, self.root.quota.clone(), 0_u32),
        )]);
        for account in &self.accounts {
            validate_amounts(&account.allocation)?;
            let name = account.name.as_str().to_string();
            let (depth, deadline, remaining, children) = nodes
                .get(account.parent.as_str())
                .cloned()
                .ok_or_else(|| err("admission_lineage_profile", "unknown or later parent"))?;
            if nodes.contains_key(&name)
                || depth + 1 > self.max_depth
                || children + 1 > self.max_children
                || account.deadline_ms == 0
                || account.deadline_ms > deadline
            {
                return Err(err(
                    "admission_lineage_profile",
                    "account exceeds parent deadline, depth or fan-out",
                ));
            }
            let left = subtract(&remaining, &account.allocation)?;
            nodes.insert(
                account.parent.as_str().to_string(),
                (depth, deadline, left, children + 1),
            );
            nodes.insert(
                name,
                (
                    depth + 1,
                    account.deadline_ms,
                    account.allocation.clone(),
                    0,
                ),
            );
        }
        Ok(())
    }
    /// Domain-tagged BLAKE3 account identity for `root` or a profile name.
    pub fn account_identity(&self, name: &str) -> CoreResult<ContentId> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kiln.fair-admission-lineage.v1");
        for part in [self.campaign.as_str(), name] {
            hasher.update(&(part.len() as u64).to_le_bytes());
            hasher.update(part.as_bytes());
        }
        ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
    }
}
/// Usage over every account descending from one finite root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageUsage {
    pub root: ContentId,
    pub accounts: u32,
    pub max_depth: u32,
    pub revoked: u32,
    /// The root's original finite allocation.
    pub allocated: Amounts,
    /// Unreserved quota still held by lineage accounts.
    pub available: Amounts,
    /// Persisted requests without an owner answer.
    pub tentative: Amounts,
    /// Owner-confirmed held debits.
    pub held: Amounts,
    /// Possibly accepted effects awaiting owner settlement; never zero-filled.
    pub unknown: Amounts,
    /// Authoritatively settled consumption.
    pub settled: Amounts,
    /// Owner-rejected or expired requests, already returned to `available`.
    pub rejected: Amounts,
    /// Outstanding plus settled amounts per charge kind.
    pub by_kind: BTreeMap<ChargeKind, Amounts>,
    pub unknown_operations: u32,
}

/// Lineage facts for one queued work item's account.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageExplanation {
    pub root: ContentId,
    pub account: ContentId,
    pub depth: u32,
    pub blocker: Option<AdmissionReason>,
}

impl AdmissionLedger {
    /// Revoke an account and every descendant. Idempotent; returns how many
    /// accounts were newly revoked. Outstanding and unknown operations keep
    /// their debits; nothing is refunded and no quota becomes reusable.
    pub fn revoke_account(&mut self, account: &ContentId) -> CoreResult<u32> {
        self.policy()?;
        if !self.accounts.contains_key(account) {
            return Err(err("admission_account", "revoked account missing"));
        }
        let mut newly = 0_u32;
        for member in self.descendants(account).into_keys() {
            if self.revoked.insert(member) {
                newly += 1;
            }
        }
        Ok(newly)
    }

    /// First blocker on the account's ancestor chain: revoked (`denied`),
    /// ancestor deadline reached (`expired`, only with `now_ms`), or a parent
    /// attempt whose queued work was cancelled or expired.
    pub fn lineage_blocker(
        &self,
        account: &ContentId,
        now_ms: Option<u64>,
    ) -> CoreResult<Option<AdmissionReason>> {
        let mut cursor = account;
        for _ in 0..=self.accounts.len() {
            let current = self
                .accounts
                .get(cursor)
                .ok_or_else(|| err("admission_lineage", "lineage ancestor missing"))?;
            if self.revoked.contains(cursor) {
                return Ok(Some(AdmissionReason::Denied));
            }
            if now_ms.is_some_and(|now| now >= current.deadline_ms) {
                return Ok(Some(AdmissionReason::Expired));
            }
            if let Some(claim) = &current.parent_attempt
                && let Some(parent) = self
                    .items
                    .values()
                    .find(|item| item.claim.as_ref() == Some(claim))
                    .filter(|item| item.phase == AdmissionPhase::Terminal)
            {
                match parent.reason {
                    AdmissionReason::Cancelled | AdmissionReason::CancellationRequested => {
                        return Ok(Some(AdmissionReason::Cancelled));
                    }
                    AdmissionReason::Expired => return Ok(Some(AdmissionReason::Expired)),
                    _ => {}
                }
            }
            match &current.parent {
                Some(parent) => cursor = parent,
                None => return Ok(None),
            }
        }
        Err(err("admission_lineage", "lineage is cyclic"))
    }

    /// Sum one root's subtree and fail closed unless the root allocation
    /// equals available + tentative + held + unknown + settled.
    pub fn lineage_usage(&self, root: &ContentId) -> CoreResult<LineageUsage> {
        let account = self
            .accounts
            .get(root)
            .ok_or_else(|| err("admission_account", "lineage root missing"))?;
        if account.parent.is_some() {
            return Err(err("admission_lineage_root", "account is not a root"));
        }
        let allocated = self
            .quotas
            .get(root)
            .cloned()
            .ok_or_else(|| err("admission_lineage", "root allocation missing"))?;
        let members = self.descendants(root);
        let mut usage = LineageUsage {
            root: root.clone(),
            accounts: members.len() as u32,
            max_depth: members.values().copied().max().unwrap_or(0),
            revoked: members
                .keys()
                .filter(|id| self.revoked.contains(*id))
                .count() as u32,
            allocated,
            available: Amounts::new(),
            tentative: Amounts::new(),
            held: Amounts::new(),
            unknown: Amounts::new(),
            settled: Amounts::new(),
            rejected: Amounts::new(),
            by_kind: BTreeMap::new(),
            unknown_operations: 0,
        };
        for id in members.keys() {
            let member = self
                .accounts
                .get(id)
                .ok_or_else(|| err("admission_lineage", "lineage member missing"))?;
            usage.available = add(&usage.available, &member.available)?;
            usage.settled = add(&usage.settled, &member.consumed)?;
        }
        for reservation in self
            .reservations
            .values()
            .filter(|r| members.contains_key(&r.account))
        {
            let bucket = match reservation.state {
                ReservationState::Tentative => &mut usage.tentative,
                ReservationState::Committed => &mut usage.held,
                ReservationState::Unknown => {
                    usage.unknown_operations += 1;
                    &mut usage.unknown
                }
                ReservationState::Rejected
                | ReservationState::Expired
                | ReservationState::Released => &mut usage.rejected,
                // Settled amounts are already in the accounts' consumed totals.
                ReservationState::Settled => {
                    let kind = usage.by_kind.entry(reservation.kind).or_default();
                    *kind = add(kind, &reservation.amounts)?;
                    continue;
                }
            };
            *bucket = add(bucket, &reservation.amounts)?;
            if !matches!(
                reservation.state,
                ReservationState::Rejected | ReservationState::Expired | ReservationState::Released
            ) {
                let kind = usage.by_kind.entry(reservation.kind).or_default();
                *kind = add(kind, &reservation.amounts)?;
            }
        }
        let mut accounted = add(&usage.available, &usage.tentative)?;
        accounted = add(&accounted, &usage.held)?;
        accounted = add(&accounted, &usage.unknown)?;
        accounted = add(&accounted, &usage.settled)?;
        if nonzero(&accounted) != nonzero(&usage.allocated) {
            return Err(err(
                "admission_lineage_conservation",
                "lineage usage does not conserve the root allocation",
            ));
        }
        Ok(usage)
    }

    pub(super) fn lineage_explanation(
        &self,
        account: &ContentId,
        now_ms: u64,
    ) -> CoreResult<LineageExplanation> {
        let mut cursor = account;
        let mut depth = 0_u32;
        for _ in 0..=self.accounts.len() {
            let current = self
                .accounts
                .get(cursor)
                .ok_or_else(|| err("admission_lineage", "lineage ancestor missing"))?;
            match &current.parent {
                Some(parent) => {
                    cursor = parent;
                    depth += 1;
                }
                None => {
                    return Ok(LineageExplanation {
                        root: cursor.clone(),
                        account: account.clone(),
                        depth,
                        blocker: self.lineage_blocker(account, Some(now_ms))?,
                    });
                }
            }
        }
        Err(err("admission_lineage", "lineage is cyclic"))
    }

    /// The account and every descendant, with depth below `account`.
    fn descendants(&self, account: &ContentId) -> BTreeMap<ContentId, u32> {
        let mut members = BTreeMap::from([(account.clone(), 0_u32)]);
        let mut frontier = BTreeSet::from([account.clone()]);
        while !frontier.is_empty() {
            let mut next = BTreeSet::new();
            for (id, candidate) in &self.accounts {
                if let Some(parent) = candidate.parent.as_ref().filter(|p| frontier.contains(*p))
                    && !members.contains_key(id)
                {
                    let depth = members.get(parent).copied().unwrap_or(0) + 1;
                    members.insert(id.clone(), depth);
                    next.insert(id.clone());
                }
            }
            frontier = next;
        }
        members
    }
}

fn nonzero(amounts: &Amounts) -> Amounts {
    amounts
        .iter()
        .filter(|(_, amount)| **amount != 0)
        .map(|(dimension, amount)| (dimension.clone(), *amount))
        .collect()
}

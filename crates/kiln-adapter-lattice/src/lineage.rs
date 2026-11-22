// r[impl kiln.admission.lattice_lineage]
//! Kiln's producer of Lattice's `lattice.kiln-budget-lineage.v1`.
//!
//! The lineage is derived from Kiln's own admission ledger when a dispatch
//! route builds the request it hands to Lattice: the campaign root, then the
//! queued work's account, then the attempt's owner-committed reservations.
//! The attempt's remainder in each dimension is what those reservations hold
//! for it; the account's `available` amounts are what is left for everyone
//! else after that debit, so they are never the attempt's budget.
//!
//! Kiln types are mapped, not shared: the Lattice consumer type lives in
//! Lattice [`LATTICE_REVISION`] (`src/implementation_attempt/lineage.rs`) and
//! this crate does not depend on it. `docs/fair-admission.md` records the
//! field mapping.

use kiln_core::fair_admission::{
    AdmissionLedger, AdmissionReason, ClaimRef, MAX_DIMENSIONS, MAX_LINEAGE_DEPTH,
    ReservationState, ServingExplanation,
};
use kiln_core::types::{ContentId, DEFAULT_MAX_ALIAS_BYTES};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Wire schema of Lattice's consumer type.
pub const LINEAGE_SCHEMA: &str = "lattice.kiln-budget-lineage.v1";
/// The Lattice revision whose consumer this producer targets.
pub const LATTICE_REVISION: &str = "5a8e1d51c2e36100424b761c4fef65e6cd4c68dc";
/// The `lattice.implementation-attempt-run.v3` request field that carries it.
pub const LINEAGE_FIELD: &str = "lineage";
/// The dimensions Lattice charges. Every other dimension is accepted, never
/// drawn by Lattice and reported as unaccounted.
pub const CHARGED_DIMENSIONS: [&str; 2] = ["model_requests", "tool_invocations"];

/// Lattice's `Blocker`: Kiln's lineage blocker (`LineageExplanation.blocker`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocker {
    Denied,
    Expired,
    Cancelled,
    BudgetExhausted,
}

/// Lattice's `Backpressure`: Kiln's serving gate (`ServingExplanation.gate`),
/// or `none` when the work has no worker-request serving owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backpressure {
    None,
    Eligible,
    Waiting,
    Overloaded,
    Stale,
    Incomplete,
    Incompatible,
}

/// The frozen lineage snapshot of one attempt's account, field for field in
/// Lattice's order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLineage {
    pub schema: String,
    /// `LineageExplanation.root`.
    pub root: String,
    /// `LineageExplanation.account`: the queued work's account.
    pub account: String,
    /// The account record's parent; `None` exactly for the root.
    pub parent: Option<String>,
    /// `LineageExplanation.depth`.
    pub depth: u32,
    /// The attempt's committed reservations, summed per dimension.
    pub remaining: BTreeMap<String, u64>,
    /// Always `None` when Kiln emits it: Kiln refuses a blocked lineage.
    pub blocker: Option<Blocker>,
    /// `none` or `eligible` when Kiln emits it.
    pub backpressure: Backpressure,
}

impl BudgetLineage {
    /// Dimensions Lattice cannot charge; their usage stays unknown to Kiln.
    #[must_use]
    pub fn unaccounted(&self) -> Vec<&str> {
        self.remaining
            .keys()
            .map(String::as_str)
            .filter(|dimension| !CHARGED_DIMENSIONS.contains(dimension))
            .collect()
    }

    /// Lattice's structural rules for one snapshot.
    fn validate(&self) -> Result<(), LineageRefusal> {
        let unrepresentable = |detail: &str| {
            Err(LineageRefusal::Unrepresentable {
                detail: detail.to_owned(),
            })
        };
        if self.schema != LINEAGE_SCHEMA {
            return unrepresentable("schema is not lattice.kiln-budget-lineage.v1");
        }
        let identities = [Some(&self.root), Some(&self.account), self.parent.as_ref()];
        if !identities
            .into_iter()
            .flatten()
            .all(|id| is_lattice_identity(id))
        {
            return unrepresentable("identity is not 64 lowercase hexadecimal digits");
        }
        let is_shaped = match &self.parent {
            None => self.depth == 0 && self.account == self.root,
            Some(parent) => {
                self.depth > 0
                    && self.depth <= MAX_LINEAGE_DEPTH
                    && parent != &self.account
                    && self.account != self.root
            }
        };
        if !is_shaped {
            return unrepresentable("root, parent and depth disagree");
        }
        if self.remaining.is_empty()
            || self.remaining.len() > MAX_DIMENSIONS
            || self
                .remaining
                .keys()
                .any(|name| name.is_empty() || name.len() > DEFAULT_MAX_ALIAS_BYTES)
        {
            return unrepresentable("dimensions are empty, too many or malformed");
        }
        Ok(())
    }
}

/// Why Kiln refuses to build a request, before any transport start.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "code")]
pub enum LineageRefusal {
    /// The admitted request already names a lineage; only Kiln derives it.
    RequestNamesLineage,
    /// A reservation of this attempt still awaits its owner, or a dispatch
    /// intent made its outcome unknown.
    ReservationUnsettled { operation: String },
    /// A committed reservation of this attempt draws from another account
    /// than the queued work's.
    CrossedReservation { operation: String },
    /// A required owner of the queued work has no committed grant for this
    /// attempt, so the remainder would be guessed from part of the grants.
    MissingOwnerGrant { provider: String },
    /// The ledger cannot be walked from the attempt's reservations to one
    /// root, or its amounts contradict each other.
    LedgerInconsistent { detail: String },
    /// Kiln's lineage reports a blocker.
    Blocked { blocker: Blocker },
    /// The serving gate is neither `none` nor `eligible`.
    Backpressure { gate: Backpressure },
    /// The attempt holds nothing in a dimension Lattice charges that its
    /// account tracks.
    Exhausted { dimension: String },
    /// The snapshot cannot be expressed in Lattice's schema.
    Unrepresentable { detail: String },
    /// On recovery, the frozen request is not the recorded sent bytes, or
    /// not the admitted request plus a lineage.
    FrozenRequestCrossed,
}

impl LineageRefusal {
    /// The typed reason, as serialized.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RequestNamesLineage => "request_names_lineage",
            Self::ReservationUnsettled { .. } => "reservation_unsettled",
            Self::CrossedReservation { .. } => "crossed_reservation",
            Self::MissingOwnerGrant { .. } => "missing_owner_grant",
            Self::LedgerInconsistent { .. } => "ledger_inconsistent",
            Self::Blocked { .. } => "blocked",
            Self::Backpressure { .. } => "backpressure",
            Self::Exhausted { .. } => "exhausted",
            Self::Unrepresentable { .. } => "unrepresentable",
            Self::FrozenRequestCrossed => "frozen_request_crossed",
        }
    }
}

impl std::fmt::Display for LineageRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = serde_json::to_string(self).unwrap_or_default();
        let code = self.code();
        if matches!(self, Self::FrozenRequestCrossed) {
            // Only recovery checks a frozen request, after a transport start.
            write!(
                formatter,
                "budget lineage refused on recovery ({code}): {detail}; {RECOVERY_NOT_INVOKED}"
            )
        } else {
            write!(
                formatter,
                "budget lineage refused before transport ({code}): {detail}; nothing was started"
            )
        }
    }
}

/// How a recovery refusal leaves an attempt whose transport start is recorded.
pub const RECOVERY_NOT_INVOKED: &str = "Lattice was not invoked for recovery; the first launch may have run; retain the unknown outcome and reconcile";

impl std::error::Error for LineageRefusal {}

fn inconsistent(detail: &str) -> LineageRefusal {
    LineageRefusal::LedgerInconsistent {
        detail: detail.to_owned(),
    }
}

fn is_lattice_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn lattice_identity(id: &ContentId) -> Result<String, LineageRefusal> {
    if is_lattice_identity(id.as_str()) {
        Ok(id.as_str().to_owned())
    } else {
        Err(LineageRefusal::Unrepresentable {
            detail: "identity is not 64 lowercase hexadecimal digits".to_owned(),
        })
    }
}

fn blocker(reason: AdmissionReason) -> Result<Blocker, LineageRefusal> {
    match reason {
        AdmissionReason::Denied => Ok(Blocker::Denied),
        AdmissionReason::Expired => Ok(Blocker::Expired),
        AdmissionReason::Cancelled => Ok(Blocker::Cancelled),
        AdmissionReason::BudgetExhausted => Ok(Blocker::BudgetExhausted),
        _ => Err(inconsistent("lineage blocker is not a lineage reason")),
    }
}

fn backpressure(gate: AdmissionReason) -> Result<Backpressure, LineageRefusal> {
    match gate {
        AdmissionReason::Eligible => Ok(Backpressure::Eligible),
        AdmissionReason::Waiting => Ok(Backpressure::Waiting),
        AdmissionReason::Overloaded => Ok(Backpressure::Overloaded),
        AdmissionReason::Stale => Ok(Backpressure::Stale),
        AdmissionReason::Incomplete => Ok(Backpressure::Incomplete),
        AdmissionReason::Incompatible => Ok(Backpressure::Incompatible),
        _ => Err(inconsistent("serving gate is not a serving reason")),
    }
}

/// Derive the lineage of `claim`'s attempt on `work` at `now_ms`.
///
/// `None` without an admission policy or without an owner-committed
/// reservation of this exact claim: the request is then not admitted and
/// carries no lineage. Every inconsistency is a typed refusal; nothing is
/// guessed, rounded or defaulted.
///
/// # Errors
/// See [`LineageRefusal`].
pub fn derive(
    ledger: &AdmissionLedger,
    work: &ContentId,
    claim: &ClaimRef,
    now_ms: u64,
) -> Result<Option<BudgetLineage>, LineageRefusal> {
    if ledger.policy.is_none() {
        return Ok(None);
    }
    let mut admitted = Vec::new();
    for reservation in ledger
        .reservations
        .values()
        .filter(|reservation| &reservation.work_version == work && &reservation.claim == claim)
    {
        match reservation.state {
            ReservationState::Committed => admitted.push(reservation),
            ReservationState::Tentative | ReservationState::Unknown => {
                return Err(LineageRefusal::ReservationUnsettled {
                    operation: reservation.operation.as_str().to_owned(),
                });
            }
            ReservationState::Rejected
            | ReservationState::Expired
            | ReservationState::Released
            | ReservationState::Settled => {}
        }
    }
    if admitted.is_empty() {
        return Ok(None);
    }
    let item = ledger
        .items
        .get(work)
        .ok_or_else(|| inconsistent("admitted work is not queued"))?;
    if item.claim.as_ref() != Some(claim) {
        return Err(inconsistent(
            "queue claim differs from the reserved attempt",
        ));
    }
    let mut held: BTreeMap<String, u64> = BTreeMap::new();
    for reservation in &admitted {
        if reservation.account != item.account {
            return Err(LineageRefusal::CrossedReservation {
                operation: reservation.operation.as_str().to_owned(),
            });
        }
        for (dimension, amount) in &reservation.amounts {
            let total = held.entry(dimension.as_str().to_owned()).or_insert(0);
            *total = total
                .checked_add(*amount)
                .ok_or_else(|| inconsistent("reserved amounts overflow"))?;
        }
    }
    // As the owner-grant gate does: every required owner has granted, or the
    // remainder would be read from part of the attempt's grants.
    if let Some((provider, _)) = item.required_owners.iter().find(|(provider, kind)| {
        !admitted
            .iter()
            .any(|reservation| &reservation.provider == *provider && &reservation.kind == *kind)
    }) {
        return Err(LineageRefusal::MissingOwnerGrant {
            provider: provider.as_str().to_owned(),
        });
    }
    let explanation = ledger.explanation(work, now_ms).map_err(|diagnostic| {
        LineageRefusal::LedgerInconsistent {
            detail: format!("{}: {}", diagnostic.code, diagnostic.message),
        }
    })?;
    // The explanation walks from the queue row's account to its parentless
    // root, so root, account, parent and depth agree by construction.
    let lineage = explanation
        .lineage
        .ok_or_else(|| inconsistent("queued work has no lineage explanation"))?;
    if let Some(reason) = lineage.blocker {
        return Err(LineageRefusal::Blocked {
            blocker: blocker(reason)?,
        });
    }
    let gate = admitting_gate(explanation.serving.as_ref())?;
    let account = ledger
        .accounts
        .get(&item.account)
        .filter(|account| account.identity == lineage.account)
        .ok_or_else(|| inconsistent("work account is missing or crossed"))?;
    let quota = ledger
        .quotas
        .get(&lineage.root)
        .ok_or_else(|| inconsistent("root allocation is missing"))?;
    // Lattice draws only the dimensions it charges; one the account tracks
    // that the attempt does not hold would leave Lattice unbounded in it.
    // A dimension Lattice cannot charge bounds nothing in Lattice, so one the
    // attempt does not hold (a shared allocation's) is simply omitted.
    if let Some(dimension) = account.available.keys().find(|dimension| {
        CHARGED_DIMENSIONS.contains(&dimension.as_str())
            && held
                .get(dimension.as_str())
                .is_none_or(|amount| *amount == 0)
    }) {
        return Err(LineageRefusal::Exhausted {
            dimension: dimension.as_str().to_owned(),
        });
    }
    for (dimension, amount) in &held {
        let within = |amounts: &kiln_core::fair_admission::Amounts| {
            amounts
                .iter()
                .find(|(name, _)| name.as_str() == dimension)
                .is_some_and(|(_, bound)| amount <= bound)
        };
        if !account
            .available
            .keys()
            .any(|name| name.as_str() == dimension)
        {
            return Err(inconsistent(
                "a held dimension is not tracked by the account",
            ));
        }
        if !within(&item.budget) {
            return Err(inconsistent("a held amount exceeds the work budget"));
        }
        if !within(quota) {
            return Err(inconsistent("a held amount exceeds the root allocation"));
        }
    }
    let snapshot = BudgetLineage {
        schema: LINEAGE_SCHEMA.to_owned(),
        root: lattice_identity(&lineage.root)?,
        account: lattice_identity(&account.identity)?,
        parent: account.parent.as_ref().map(lattice_identity).transpose()?,
        depth: lineage.depth,
        remaining: held,
        blocker: None,
        backpressure: gate,
    };
    snapshot.validate()?;
    Ok(Some(snapshot))
}

/// The serving gate as Lattice's `backpressure`, refused unless it admits.
fn admitting_gate(serving: Option<&ServingExplanation>) -> Result<Backpressure, LineageRefusal> {
    let gate = match serving {
        None => Backpressure::None,
        Some(serving) => backpressure(serving.gate)?,
    };
    if matches!(gate, Backpressure::None | Backpressure::Eligible) {
        Ok(gate)
    } else {
        Err(LineageRefusal::Backpressure { gate })
    }
}

/// Which `kiln-lattice-dispatch` route builds the request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    /// `--worker-request`: a `lattice.implementation-attempt-run.v3` request,
    /// which carries the lineage.
    Worker,
    /// `--dispatch-plan` workflow exchange: a `lattice.workflow-request`,
    /// which has no lineage field. The lineage is derived and checked; the
    /// request bytes never change.
    Exchange,
}

/// The exact bytes `route` sends for the admitted request bytes.
///
/// Without an admitted reservation, and always on the exchange route, these
/// are the admitted bytes, borrowed. On the worker route an admitted attempt
/// sends the admitted bytes with one appended `lineage` member.
///
/// # Errors
/// Any [`LineageRefusal`] from [`derive`]; on the worker route also a
/// request that already names a lineage or is not a JSON object.
pub fn dispatched_request<'a>(
    route: Route,
    ledger: &AdmissionLedger,
    work: &ContentId,
    claim: &ClaimRef,
    now_ms: u64,
    admitted: &'a [u8],
) -> Result<Cow<'a, [u8]>, LineageRefusal> {
    if route == Route::Exchange {
        derive(ledger, work, claim, now_ms)?;
        return Ok(Cow::Borrowed(admitted));
    }
    if admitted_object(admitted)?.contains_key(LINEAGE_FIELD) {
        return Err(LineageRefusal::RequestNamesLineage);
    }
    match derive(ledger, work, claim, now_ms)? {
        None => Ok(Cow::Borrowed(admitted)),
        Some(lineage) => compose(admitted, &lineage).map(Cow::Owned),
    }
}

/// Check a frozen worker request against its admitted bytes on recovery.
///
/// It must be the admitted bytes exactly, or exactly what
/// [`dispatched_request`] composes from them and one well-formed lineage.
/// The snapshot's values are not derived again: the grants are unknown by
/// then, and Lattice binds its retained attempt to the exact bytes it ran.
///
/// # Errors
/// [`LineageRefusal::FrozenRequestCrossed`] for anything else.
pub fn verify_frozen(
    admitted: &[u8],
    frozen: &[u8],
) -> Result<Option<BudgetLineage>, LineageRefusal> {
    if frozen == admitted {
        return Ok(None);
    }
    if admitted_object(admitted)
        .map_err(|_| LineageRefusal::FrozenRequestCrossed)?
        .contains_key(LINEAGE_FIELD)
    {
        return Err(LineageRefusal::FrozenRequestCrossed);
    }
    let lineage = serde_json::from_slice::<serde_json::Value>(frozen)
        .ok()
        .and_then(|mut value| value.as_object_mut()?.remove(LINEAGE_FIELD))
        .and_then(|value| serde_json::from_value::<BudgetLineage>(value).ok())
        .filter(|lineage| lineage.validate().is_ok())
        .ok_or(LineageRefusal::FrozenRequestCrossed)?;
    if compose(admitted, &lineage)? == frozen {
        Ok(Some(lineage))
    } else {
        Err(LineageRefusal::FrozenRequestCrossed)
    }
}

fn admitted_object(
    admitted: &[u8],
) -> Result<serde_json::Map<String, serde_json::Value>, LineageRefusal> {
    match serde_json::from_slice::<serde_json::Value>(admitted) {
        Ok(serde_json::Value::Object(object)) => Ok(object),
        _ => Err(LineageRefusal::Unrepresentable {
            detail: "admitted request is not one JSON object".to_owned(),
        }),
    }
}

/// The admitted bytes up to the end of their last member, `,"lineage":` and
/// the compact snapshot, then the rest of the admitted bytes: every admitted
/// byte is kept, in order, around one inserted member.
fn compose(admitted: &[u8], lineage: &BudgetLineage) -> Result<Vec<u8>, LineageRefusal> {
    let unrepresentable = || LineageRefusal::Unrepresentable {
        detail: "admitted request does not end its object".to_owned(),
    };
    let close = admitted
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .filter(|index| admitted.get(*index) == Some(&b'}'))
        .ok_or_else(unrepresentable)?;
    let body = admitted.get(..close).ok_or_else(unrepresentable)?;
    // The object parsed, so this is its `{` when empty, else its last value.
    let end = body
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(0, |index| index + 1);
    let (head, tail) = admitted.split_at(end);
    let snapshot =
        serde_json::to_vec(lineage).map_err(|error| LineageRefusal::Unrepresentable {
            detail: error.to_string(),
        })?;
    let mut bytes = Vec::with_capacity(admitted.len() + snapshot.len() + 12);
    bytes.extend_from_slice(head);
    if head.last() != Some(&b'{') {
        bytes.push(b',');
    }
    bytes.extend_from_slice(b"\"lineage\":");
    bytes.extend_from_slice(&snapshot);
    bytes.extend_from_slice(tail);
    Ok(bytes)
}

#[cfg(test)]
#[path = "lineage_tests.rs"]
mod tests;

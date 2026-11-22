//! Deterministic support maintenance over immutable scoped derivations.
//!
//! A claim may have several alternative support branches and several declared
//! refuting branches. Each branch carries its own premises, residual
//! assumptions and discharge paths. A bounded, ordered change log describes
//! evidence invalidation, assumption withdrawal, claim revision, policy/context
//! selection changes and newly admitted counterevidence. Full recomputation
//! yields the surviving alternatives, withdrawn alternatives with their
//! historical reasons, and the claims or lifecycle obligations that require
//! reevaluation.
//!
//! This module is advisory. It does not repair beliefs, transfer evidence
//! between requirement revisions, mutate lifecycle decisions or authorize an
//! external effect.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::evidence::canonical_json;
use super::scoped_claim_derivations::{
    claim_identity, ClaimContext, ClaimState, ConclusionLink, DerivationError, ScopedClaim,
    INDEPENDENCE_NOT_ESTABLISHED,
};

pub const SUPPORT_POLICY_SCHEMA_V1: &str = "cairn.support-maintenance-policy.v1";
pub const SUPPORT_CASE_SCHEMA_V1: &str = "cairn.support-maintenance-case.v1";
pub const SUPPORT_REPORT_SCHEMA_V1: &str = "cairn.support-maintenance-report.v1";
// r[impl cairn.support_maintenance.authority]
pub const SUPPORT_BOUNDARY: &str = "Advisory support maintenance only: surviving branches are not an accepted exception, an action authorization, readiness, or behavioral requirement satisfaction. Evidence invalidation removes current applicability without turning an old observation into a false one. Applicable counterevidence remains distinct from missing evidence. Native applicability validators, verification roles, stage validators, readiness states and the Pass | Block gate retain authority. Full recomputation is the only evaluator provided; no automatic belief repair, evidence transfer, scheduling or lifecycle mutation occurs.";

const CHANGE_KINDS: [&str; 7] = [
    "evidence_stale",
    "evidence_withdrawn",
    "assumption_withdrawn",
    "evidence_admitted",
    "claim_revised",
    "policy_changed",
    "context_changed",
];

const MAX_CLAIMS_CEILING: usize = 1024;
const MAX_BRANCHES_CEILING: usize = 4096;
const MAX_EVIDENCE_CEILING: usize = 4096;
const MAX_PREMISES_CEILING: usize = 16;
const MAX_ASSUMPTIONS_CEILING: usize = 1024;
const MAX_EVENTS_CEILING: usize = 1024;
const MAX_OBLIGATIONS_CEILING: usize = 1024;
const MAX_DEPTH_CEILING: usize = 64;

/// Checked Nickel bounds and the admitted change vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportPolicy {
    pub schema: String,
    pub change_kinds: Vec<String>,
    pub bounds: SupportBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportBounds {
    pub max_claims: usize,
    pub max_branches: usize,
    pub max_evidence: usize,
    pub max_premises: usize,
    pub max_assumptions: usize,
    pub max_events: usize,
    pub max_obligations: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchPolarity {
    Support,
    Refute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolarity {
    Support,
    Counterevidence,
}

/// One alternative conjunction. `premises` are exact scoped claim identities;
/// `assumptions` are labels shared by every branch that names them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportBranch {
    pub id: String,
    pub conclusion: String,
    pub polarity: BranchPolarity,
    pub rule: String,
    pub version: u32,
    pub premises: Vec<String>,
    pub assumptions: BTreeSet<String>,
    pub assurance_scope: String,
    pub applicability: ClaimContext,
    /// Required only when the premise claims bind a different native
    /// requirement revision. Its digest names the explicitly admitted relation.
    pub cross_revision_relation: Option<String>,
}

/// One immutable admitted observation. An invalidated record loses current
/// applicability; it is never rewritten as counterevidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub id: String,
    pub claim: String,
    pub polarity: EvidencePolarity,
    pub rule: String,
    pub version: u32,
    pub assurance_scope: String,
    pub assumptions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Change {
    EvidenceStale {
        evidence: String,
    },
    EvidenceWithdrawn {
        evidence: String,
    },
    AssumptionWithdrawn {
        assumption: String,
    },
    EvidenceAdmitted {
        evidence: EvidenceRecord,
    },
    ClaimRevised {
        from_claim: String,
        to_claim: String,
        classification: String,
        classification_identity: String,
    },
    PolicyChanged {
        previous_policy_blake3: String,
    },
    ContextChanged {
        previous_context_blake3: String,
    },
}

impl Change {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::EvidenceStale { .. } => "evidence_stale",
            Self::EvidenceWithdrawn { .. } => "evidence_withdrawn",
            Self::AssumptionWithdrawn { .. } => "assumption_withdrawn",
            Self::EvidenceAdmitted { .. } => "evidence_admitted",
            Self::ClaimRevised { .. } => "claim_revised",
            Self::PolicyChanged { .. } => "policy_changed",
            Self::ContextChanged { .. } => "context_changed",
        }
    }

    #[must_use]
    pub fn target(&self) -> String {
        match self {
            Self::EvidenceStale { evidence } | Self::EvidenceWithdrawn { evidence } => {
                evidence.clone()
            }
            Self::AssumptionWithdrawn { assumption } => assumption.clone(),
            Self::EvidenceAdmitted { evidence } => evidence.id.clone(),
            Self::ClaimRevised {
                from_claim,
                to_claim,
                ..
            } => {
                format!("{from_claim}->{to_claim}")
            }
            Self::PolicyChanged {
                previous_policy_blake3,
            } => previous_policy_blake3.clone(),
            Self::ContextChanged {
                previous_context_blake3,
            } => previous_context_blake3.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeEvent {
    pub id: String,
    pub sequence: u64,
    pub reason: String,
    pub change: Change,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleObligation {
    pub id: String,
    pub claim: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportCase {
    pub schema: String,
    pub claims: Vec<ScopedClaim>,
    pub branches: Vec<SupportBranch>,
    pub evidence: Vec<EvidenceRecord>,
    pub changes: Vec<ChangeEvent>,
    pub obligations: Vec<LifecycleObligation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchStatus {
    Survived,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventReference {
    pub event_id: String,
    pub sequence: u64,
}

/// The nearest admitted change or missing premise that withdraws a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CauseOrigin {
    pub kind: String,
    pub subject: String,
    pub event: Option<EventReference>,
    pub detail: String,
}

/// Dependency path from the affected branch or claim to the origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvalidationCause {
    pub origin: CauseOrigin,
    pub dependency_chain: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchExplanation {
    pub id: String,
    pub conclusion: String,
    pub polarity: BranchPolarity,
    pub rule: String,
    pub version: u32,
    pub premises: Vec<String>,
    pub assumptions: BTreeSet<String>,
    pub assurance_scope: String,
    pub applicability: ClaimContext,
    pub cross_revision_relation: Option<String>,
    pub status: BranchStatus,
    pub discharge_paths: Vec<String>,
    pub link: Option<ConclusionLink>,
    pub cause: Option<InvalidationCause>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceExplanation {
    pub evidence: EvidenceRecord,
    pub historical: bool,
    pub admitted_sequence: Option<u64>,
    pub effective: bool,
    pub link: Option<ConclusionLink>,
    pub cause: Option<InvalidationCause>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionExplanation {
    pub event_id: String,
    pub sequence: u64,
    pub from_claim: String,
    pub to_claim: String,
    pub classification: String,
    pub classification_identity: String,
    pub automatic_support_transfer: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedChange {
    pub event_id: String,
    pub sequence: u64,
    pub kind: String,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimMaintenance {
    pub claim_id: String,
    pub claim: ScopedClaim,
    pub baseline_state: ClaimState,
    pub state: ClaimState,
    pub state_reason: String,
    pub current: bool,
    pub superseded_by: Option<String>,
    pub revision_classification: Option<String>,
    pub support: Vec<ConclusionLink>,
    pub counterevidence: Vec<ConclusionLink>,
    pub unresolved_premises: BTreeSet<String>,
    pub residual_assumptions: BTreeSet<String>,
    pub surviving_branches: Vec<String>,
    pub withdrawn_branches: Vec<String>,
    pub obligations: Vec<String>,
    pub cause: Option<InvalidationCause>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationImpact {
    pub id: String,
    pub claim: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimAnalysisStatus {
    pub mode: String,
    pub complete: bool,
    pub incremental: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundUsage {
    pub limits: SupportBounds,
    pub claims: usize,
    pub branches: usize,
    pub evidence: usize,
    pub events: usize,
    pub obligations: usize,
    pub max_depth_observed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceReport {
    pub schema: String,
    pub advisory: bool,
    pub claims: Vec<ClaimMaintenance>,
    pub branches: Vec<BranchExplanation>,
    pub evidence: Vec<EvidenceExplanation>,
    pub revisions: Vec<RevisionExplanation>,
    pub events: Vec<AppliedChange>,
    pub newly_unresolved: BTreeSet<String>,
    pub affected_claims: BTreeSet<String>,
    pub affected_obligations: Vec<ObligationImpact>,
    pub trust_dependencies: BTreeSet<String>,
    pub independence: String,
    pub analysis: ClaimAnalysisStatus,
    pub bounds: BoundUsage,
    pub input_blake3: String,
    pub report_blake3: String,
    pub boundary: String,
}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, DerivationError> {
    let value = serde_json::to_value(value)
        .map_err(|error| DerivationError::InvalidInput(error.to_string()))?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(canonical_json(&value).as_bytes());
    Ok(hash.finalize().to_hex().to_string())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

/// Stable identity of the exact selected claim context. Policy/context change
/// events name this value instead of an informal environment label.
// r[impl cairn.support_maintenance.scope]
pub fn support_context_identity(context: &ClaimContext) -> Result<String, DerivationError> {
    identity("cairn.support-maintenance-context.v1", context)
}

// r[impl cairn.support_maintenance.bounds]
pub fn admit_support_policy(policy: &SupportPolicy) -> Result<(), DerivationError> {
    if policy.schema != SUPPORT_POLICY_SCHEMA_V1 {
        return Err(DerivationError::Unsupported(format!(
            "unsupported support-maintenance policy schema {}",
            policy.schema
        )));
    }
    let bounds = &policy.bounds;
    if bounds.max_claims == 0
        || bounds.max_claims > MAX_CLAIMS_CEILING
        || bounds.max_branches == 0
        || bounds.max_branches > MAX_BRANCHES_CEILING
        || bounds.max_evidence == 0
        || bounds.max_evidence > MAX_EVIDENCE_CEILING
        || bounds.max_premises == 0
        || bounds.max_premises > MAX_PREMISES_CEILING
        || bounds.max_assumptions == 0
        || bounds.max_assumptions > MAX_ASSUMPTIONS_CEILING
        || bounds.max_events == 0
        || bounds.max_events > MAX_EVENTS_CEILING
        || bounds.max_obligations == 0
        || bounds.max_obligations > MAX_OBLIGATIONS_CEILING
        || bounds.max_depth == 0
        || bounds.max_depth > MAX_DEPTH_CEILING
    {
        return Err(DerivationError::BoundExhaustion(
            "support-maintenance policy bounds are outside supported implementation ceilings"
                .into(),
        ));
    }
    if policy.change_kinds.is_empty() {
        return Err(DerivationError::Unsupported(
            "policy admits no support-maintenance change kinds".into(),
        ));
    }
    let mut kinds = BTreeSet::new();
    for kind in &policy.change_kinds {
        if !CHANGE_KINDS.contains(&kind.as_str()) {
            return Err(DerivationError::Unsupported(format!(
                "policy admits unknown change kind {kind}"
            )));
        }
        if !kinds.insert(kind.as_str()) {
            return Err(DerivationError::Unsupported(format!(
                "duplicate admitted change kind {kind}"
            )));
        }
    }
    Ok(())
}

fn validate_record(
    record: &EvidenceRecord,
    claims: &BTreeMap<String, &ScopedClaim>,
    known: &BTreeSet<String>,
) -> Result<(), DerivationError> {
    if !non_empty(&record.id)
        || !non_empty(&record.rule)
        || !non_empty(&record.assurance_scope)
        || record.version == 0
    {
        return Err(DerivationError::InvalidInput(
            "evidence record requires a non-empty identity, rule, version and assurance scope"
                .into(),
        ));
    }
    if known.contains(&record.id) {
        return Err(DerivationError::InvalidInput(format!(
            "duplicate evidence identity {}",
            record.id
        )));
    }
    if !claims.contains_key(&record.claim) {
        return Err(DerivationError::InvalidInput(format!(
            "evidence {} references an undeclared claim",
            record.id
        )));
    }
    if record
        .assumptions
        .iter()
        .any(|assumption| !non_empty(assumption))
    {
        return Err(DerivationError::InvalidInput(format!(
            "evidence {} has an empty assumption",
            record.id
        )));
    }
    Ok(())
}

struct EffectiveChanges {
    invalidated_evidence: BTreeMap<String, CauseOrigin>,
    withdrawn_assumptions: BTreeMap<String, CauseOrigin>,
    policy_changes: BTreeMap<String, CauseOrigin>,
    context_changes: BTreeMap<String, CauseOrigin>,
    superseded: BTreeMap<String, RevisionExplanation>,
    admitted_evidence: Vec<EvidenceRecord>,
    revisions: Vec<RevisionExplanation>,
    applied: Vec<AppliedChange>,
    admitted_sequences: BTreeMap<String, u64>,
}

fn event_reference(event: &ChangeEvent) -> EventReference {
    EventReference {
        event_id: event.id.clone(),
        sequence: event.sequence,
    }
}

fn event_origin(event: &ChangeEvent, kind: &str, subject: String, detail: String) -> CauseOrigin {
    CauseOrigin {
        kind: kind.into(),
        subject,
        event: Some(event_reference(event)),
        detail,
    }
}

fn apply_changes(
    policy: &SupportPolicy,
    case: &SupportCase,
    claims: &BTreeMap<String, &ScopedClaim>,
    context_ids: &BTreeMap<String, String>,
) -> Result<EffectiveChanges, DerivationError> {
    let mut ordered = case.changes.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        (left.sequence, left.id.as_str()).cmp(&(right.sequence, right.id.as_str()))
    });
    let mut sequences = BTreeSet::new();
    let mut event_ids = BTreeSet::new();
    for event in &ordered {
        if !non_empty(&event.id) || !non_empty(&event.reason) || event.sequence == 0 {
            return Err(DerivationError::InvalidInput(
                "change event requires a non-empty identity, sequence and reason".into(),
            ));
        }
        if !sequences.insert(event.sequence) {
            return Err(DerivationError::InvalidInput(format!(
                "duplicate change sequence {}",
                event.sequence
            )));
        }
        if !event_ids.insert(event.id.as_str()) {
            return Err(DerivationError::InvalidInput(format!(
                "duplicate change identity {}",
                event.id
            )));
        }
        if !policy
            .change_kinds
            .iter()
            .any(|kind| kind.as_str() == event.change.tag())
        {
            return Err(DerivationError::Unsupported(format!(
                "change kind {} is not admitted by policy",
                event.change.tag()
            )));
        }
    }

    let mut known_evidence = case
        .evidence
        .iter()
        .map(|record| record.id.clone())
        .collect::<BTreeSet<_>>();
    let mut known_assumptions = case
        .evidence
        .iter()
        .flat_map(|record| record.assumptions.iter().cloned())
        .chain(
            case.branches
                .iter()
                .flat_map(|branch| branch.assumptions.iter().cloned()),
        )
        .collect::<BTreeSet<_>>();
    let known_policies = case
        .claims
        .iter()
        .map(|claim| claim.context.policy_blake3.clone())
        .collect::<BTreeSet<_>>();
    let known_contexts = context_ids.values().cloned().collect::<BTreeSet<_>>();
    let mut effective = EffectiveChanges {
        invalidated_evidence: BTreeMap::new(),
        withdrawn_assumptions: BTreeMap::new(),
        policy_changes: BTreeMap::new(),
        context_changes: BTreeMap::new(),
        superseded: BTreeMap::new(),
        admitted_evidence: Vec::new(),
        revisions: Vec::new(),
        applied: Vec::new(),
        admitted_sequences: BTreeMap::new(),
    };

    for event in ordered {
        match &event.change {
            Change::EvidenceStale { evidence } | Change::EvidenceWithdrawn { evidence } => {
                if !known_evidence.contains(evidence) {
                    return Err(DerivationError::InvalidInput(format!(
                        "change {} targets unknown evidence {evidence}",
                        event.id
                    )));
                }
                if effective.invalidated_evidence.contains_key(evidence) {
                    return Err(DerivationError::InvalidInput(format!(
                        "evidence {evidence} is already inapplicable"
                    )));
                }
                effective.invalidated_evidence.insert(
                    evidence.clone(),
                    event_origin(
                        event,
                        event.change.tag(),
                        evidence.clone(),
                        format!(
                            "{}: evidence loses current applicability but remains a historical observation",
                            event.reason
                        ),
                    ),
                );
            }
            Change::AssumptionWithdrawn { assumption } => {
                if !non_empty(assumption) || !known_assumptions.contains(assumption) {
                    return Err(DerivationError::InvalidInput(format!(
                        "change {} targets undeclared assumption {assumption}",
                        event.id
                    )));
                }
                if effective.withdrawn_assumptions.contains_key(assumption) {
                    return Err(DerivationError::InvalidInput(format!(
                        "assumption {assumption} is already withdrawn"
                    )));
                }
                effective.withdrawn_assumptions.insert(
                    assumption.clone(),
                    event_origin(
                        event,
                        event.change.tag(),
                        assumption.clone(),
                        event.reason.clone(),
                    ),
                );
            }
            Change::EvidenceAdmitted { evidence } => {
                validate_record(evidence, claims, &known_evidence)?;
                if effective.invalidated_evidence.contains_key(&evidence.id) {
                    return Err(DerivationError::InvalidInput(format!(
                        "evidence {} was already invalidated; admission is not a reversal",
                        evidence.id
                    )));
                }
                known_evidence.insert(evidence.id.clone());
                known_assumptions.extend(evidence.assumptions.iter().cloned());
                effective
                    .admitted_sequences
                    .insert(evidence.id.clone(), event.sequence);
                effective.admitted_evidence.push((*evidence).clone());
            }
            Change::ClaimRevised {
                from_claim,
                to_claim,
                classification,
                classification_identity,
            } => {
                if !claims.contains_key(from_claim) || !claims.contains_key(to_claim) {
                    return Err(DerivationError::InvalidInput(format!(
                        "change {} revises an undeclared claim",
                        event.id
                    )));
                }
                if from_claim == to_claim {
                    return Err(DerivationError::InvalidInput(
                        "a claim cannot revise to itself".into(),
                    ));
                }
                if effective.superseded.contains_key(from_claim) {
                    return Err(DerivationError::InvalidInput(format!(
                        "claim {from_claim} is already superseded"
                    )));
                }
                if effective.superseded.contains_key(to_claim) {
                    return Err(DerivationError::InvalidInput(format!(
                        "claim {to_claim} is already historical and cannot become current"
                    )));
                }
                if !non_empty(classification) || !digest(classification_identity) {
                    return Err(DerivationError::InvalidInput(
                        "claim revision requires a classification and exact classification identity"
                            .into(),
                    ));
                }
                let revision = RevisionExplanation {
                    event_id: event.id.clone(),
                    sequence: event.sequence,
                    from_claim: from_claim.clone(),
                    to_claim: to_claim.clone(),
                    classification: classification.clone(),
                    classification_identity: classification_identity.clone(),
                    automatic_support_transfer: false,
                    detail: "revision classification alone does not transfer evidence; any current support for the new claim must be an explicit branch with current applicability".into(),
                };
                effective
                    .superseded
                    .insert(from_claim.clone(), revision.clone());
                effective.revisions.push(revision);
            }
            Change::PolicyChanged {
                previous_policy_blake3,
            } => {
                if !digest(previous_policy_blake3)
                    || !known_policies.contains(previous_policy_blake3)
                {
                    return Err(DerivationError::InvalidInput(format!(
                        "change {} targets no selected policy {previous_policy_blake3}",
                        event.id
                    )));
                }
                if effective
                    .policy_changes
                    .contains_key(previous_policy_blake3)
                {
                    return Err(DerivationError::InvalidInput(format!(
                        "policy {previous_policy_blake3} is already superseded"
                    )));
                }
                effective.policy_changes.insert(
                    previous_policy_blake3.clone(),
                    event_origin(
                        event,
                        event.change.tag(),
                        previous_policy_blake3.clone(),
                        event.reason.clone(),
                    ),
                );
            }
            Change::ContextChanged {
                previous_context_blake3,
            } => {
                if !digest(previous_context_blake3)
                    || !known_contexts.contains(previous_context_blake3)
                {
                    return Err(DerivationError::InvalidInput(format!(
                        "change {} targets no selected context {previous_context_blake3}",
                        event.id
                    )));
                }
                if effective
                    .context_changes
                    .contains_key(previous_context_blake3)
                {
                    return Err(DerivationError::InvalidInput(format!(
                        "context {previous_context_blake3} is already superseded"
                    )));
                }
                effective.context_changes.insert(
                    previous_context_blake3.clone(),
                    event_origin(
                        event,
                        event.change.tag(),
                        previous_context_blake3.clone(),
                        event.reason.clone(),
                    ),
                );
            }
        }
        effective.applied.push(AppliedChange {
            event_id: event.id.clone(),
            sequence: event.sequence,
            kind: event.change.tag().into(),
            target: event.change.target(),
            reason: event.reason.clone(),
        });
    }
    Ok(effective)
}

// r[impl cairn.support_maintenance.assumptions]
fn withdrawn_assumption_cause(
    assumptions: &BTreeSet<String>,
    withdrawn: &BTreeMap<String, CauseOrigin>,
) -> Option<CauseOrigin> {
    assumptions
        .iter()
        .filter_map(|assumption| withdrawn.get(assumption))
        .min_by(|left, right| {
            let left_sequence = left.event.as_ref().map_or(u64::MAX, |event| event.sequence);
            let right_sequence = right
                .event
                .as_ref()
                .map_or(u64::MAX, |event| event.sequence);
            (left_sequence, left.subject.as_str()).cmp(&(right_sequence, right.subject.as_str()))
        })
        .cloned()
}

fn context_cause(
    context: &ClaimContext,
    context_id: &str,
    graph: &Graph<'_>,
) -> Option<CauseOrigin> {
    graph
        .policy_changes
        .get(&context.policy_blake3)
        .or_else(|| graph.context_changes.get(context_id))
        .cloned()
}

fn state_tag(state: ClaimState) -> &'static str {
    match state {
        ClaimState::Unresolved => "unresolved",
        ClaimState::Supported => "supported",
        ClaimState::Refuted => "refuted",
        ClaimState::Disputed => "disputed",
    }
}

struct Graph<'a> {
    claims: &'a BTreeMap<String, &'a ScopedClaim>,
    branches: &'a BTreeMap<&'a str, Vec<&'a SupportBranch>>,
    evidence: &'a BTreeMap<String, &'a EvidenceRecord>,
    evidence_by_claim: &'a BTreeMap<&'a str, Vec<&'a EvidenceRecord>>,
    premises: &'a BTreeMap<&'a str, Vec<String>>,
    context_ids: &'a BTreeMap<String, String>,
    invalidated_evidence: &'a BTreeMap<String, CauseOrigin>,
    withdrawn_assumptions: &'a BTreeMap<String, CauseOrigin>,
    policy_changes: &'a BTreeMap<String, CauseOrigin>,
    context_changes: &'a BTreeMap<String, CauseOrigin>,
    max_depth: usize,
}

#[derive(Default)]
struct EvalState {
    cache: BTreeMap<String, ClaimEval>,
    visiting: BTreeSet<String>,
    branch_results: BTreeMap<String, BranchResult>,
    evidence_results: BTreeMap<String, EvidenceResult>,
    max_depth_observed: usize,
}

#[derive(Clone)]
struct ClaimEval {
    state: ClaimState,
    support: Vec<ConclusionLink>,
    counterevidence: Vec<ConclusionLink>,
    unresolved_premises: BTreeSet<String>,
    residual_assumptions: BTreeSet<String>,
    surviving_branches: BTreeSet<String>,
    withdrawn_branches: BTreeSet<String>,
    cause: Option<InvalidationCause>,
}

#[derive(Clone)]
struct BranchResult {
    status: BranchStatus,
    discharge_paths: Vec<String>,
    link: Option<ConclusionLink>,
    cause: Option<InvalidationCause>,
}

#[derive(Clone)]
struct EvidenceResult {
    effective: bool,
    link: Option<ConclusionLink>,
    cause: Option<InvalidationCause>,
}

#[allow(clippy::too_many_arguments)]
fn link(
    source: String,
    rule: &str,
    version: u32,
    premises: Vec<String>,
    applicability: &ClaimContext,
    assurance_scope: &str,
    assumptions: BTreeSet<String>,
) -> ConclusionLink {
    ConclusionLink {
        source,
        rule: rule.into(),
        version,
        premises,
        applicability: applicability.clone(),
        assurance_scope: assurance_scope.into(),
        residual_assumptions: assumptions,
    }
}

fn cause(origin: CauseOrigin, dependency_chain: Vec<String>) -> InvalidationCause {
    InvalidationCause {
        origin,
        dependency_chain,
    }
}

fn missing_origin(kind: &str, subject: String, detail: String) -> CauseOrigin {
    CauseOrigin {
        kind: kind.into(),
        subject,
        event: None,
        detail,
    }
}

/// Evaluates every claim. Callers must invoke this for every claim identity so
/// that all branches and evidence receive a deterministic explanation.
fn eval_claim(id: &str, graph: &Graph<'_>, state: &mut EvalState) -> Result<(), DerivationError> {
    if state.cache.contains_key(id) {
        return Ok(());
    }
    if !state.visiting.insert(id.to_string()) {
        return Err(DerivationError::Cycle(id.to_string()));
    }
    state.max_depth_observed = state.max_depth_observed.max(state.visiting.len());
    if state.visiting.len() > graph.max_depth {
        state.visiting.remove(id);
        return Err(DerivationError::BoundExhaustion(format!(
            "support dependency depth exceeds the admitted bound {} at {id}",
            graph.max_depth
        )));
    }
    let claim = graph
        .claims
        .get(id)
        .ok_or_else(|| DerivationError::InvalidInput(format!("undeclared claim {id}")))?;
    let context_id = graph
        .context_ids
        .get(id)
        .cloned()
        .ok_or_else(|| DerivationError::InvalidInput(format!("missing context for {id}")))?;
    let claim_context_cause = context_cause(&claim.context, &context_id, graph);

    let mut support = Vec::new();
    let mut counterevidence = Vec::new();
    let mut unresolved_premises = BTreeSet::new();
    let mut surviving_branches = BTreeSet::new();
    let mut withdrawn_branches = BTreeSet::new();

    if let Some(records) = graph.evidence_by_claim.get(id) {
        for record in records {
            let direct = claim_context_cause
                .clone()
                .or_else(|| graph.invalidated_evidence.get(&record.id).cloned())
                .or_else(|| {
                    withdrawn_assumption_cause(&record.assumptions, graph.withdrawn_assumptions)
                });
            let result = if let Some(origin) = direct {
                EvidenceResult {
                    effective: false,
                    link: None,
                    cause: Some(cause(origin, vec![id.to_string()])),
                }
            } else {
                let admitted = link(
                    record.id.clone(),
                    &record.rule,
                    record.version,
                    Vec::new(),
                    &claim.context,
                    &record.assurance_scope,
                    record.assumptions.clone(),
                );
                match record.polarity {
                    EvidencePolarity::Support => support.push(admitted.clone()),
                    EvidencePolarity::Counterevidence => counterevidence.push(admitted.clone()),
                }
                EvidenceResult {
                    effective: true,
                    link: Some(admitted),
                    cause: None,
                }
            };
            state.evidence_results.insert(record.id.clone(), result);
        }
    }

    if claim_context_cause.is_none() {
        if let Some(branches) = graph.branches.get(id) {
            for branch in branches {
                let mut branch_cause =
                    withdrawn_assumption_cause(&branch.assumptions, graph.withdrawn_assumptions)
                        .map(|origin| cause(origin, vec![branch.id.clone()]));
                let mut discharge_paths = Vec::new();
                let mut transitive_assumptions = branch.assumptions.clone();
                if branch_cause.is_none() {
                    let premises = graph.premises.get(branch.id.as_str()).ok_or_else(|| {
                        DerivationError::InvalidInput(format!(
                            "branch {} has no validated premise index",
                            branch.id
                        ))
                    })?;
                    for premise in premises {
                        eval_claim(premise, graph, state)?;
                        let premise_eval = state.cache.get(premise).ok_or_else(|| {
                            DerivationError::InvalidInput(format!(
                                "premise {premise} has no evaluation"
                            ))
                        })?;
                        for admitted in &premise_eval.support {
                            discharge_paths.push(format!("{premise} via {}", admitted.source));
                        }
                        for admitted in premise_eval
                            .support
                            .iter()
                            .chain(&premise_eval.counterevidence)
                        {
                            transitive_assumptions
                                .extend(admitted.residual_assumptions.iter().cloned());
                        }
                        if premise_eval.state != ClaimState::Supported {
                            unresolved_premises.insert((*premise).to_string());
                            unresolved_premises
                                .extend(premise_eval.unresolved_premises.iter().cloned());
                            if branch_cause.is_none() {
                                let origin = premise_eval.cause.as_ref().map_or_else(
                                    || {
                                        missing_origin(
                                            "premise_not_established",
                                            (*premise).to_string(),
                                            format!(
                                                "premise {premise} is {} rather than cleanly supported",
                                                state_tag(premise_eval.state)
                                            ),
                                        )
                                    },
                                    |cause| cause.origin.clone(),
                                );
                                let mut chain = vec![branch.id.clone()];
                                if let Some(existing) = &premise_eval.cause {
                                    chain.extend(existing.dependency_chain.iter().cloned());
                                } else {
                                    chain.push((*premise).to_string());
                                }
                                branch_cause = Some(cause(origin, chain));
                            }
                        }
                        if premise_eval.state == ClaimState::Supported {
                            transitive_assumptions
                                .extend(premise_eval.residual_assumptions.iter().cloned());
                        }
                    }
                }
                discharge_paths.sort();
                discharge_paths.dedup();
                let premise_list = graph
                    .premises
                    .get(branch.id.as_str())
                    .map(|premises| {
                        premises
                            .iter()
                            .map(|premise| (*premise).to_string())
                            .collect::<Vec<String>>()
                    })
                    .ok_or_else(|| {
                        DerivationError::InvalidInput(format!(
                            "branch {} has no validated premise index",
                            branch.id
                        ))
                    })?;
                let result = if let Some(cause) = branch_cause {
                    BranchResult {
                        status: BranchStatus::Withdrawn,
                        discharge_paths,
                        link: None,
                        cause: Some(cause),
                    }
                } else {
                    let admitted = link(
                        branch.id.clone(),
                        &branch.rule,
                        branch.version,
                        premise_list,
                        &branch.applicability,
                        &branch.assurance_scope,
                        transitive_assumptions,
                    );
                    match branch.polarity {
                        BranchPolarity::Support => support.push(admitted.clone()),
                        BranchPolarity::Refute => counterevidence.push(admitted.clone()),
                    }
                    BranchResult {
                        status: BranchStatus::Survived,
                        discharge_paths,
                        link: Some(admitted),
                        cause: None,
                    }
                };
                match result.status {
                    BranchStatus::Survived => {
                        surviving_branches.insert(branch.id.clone());
                    }
                    BranchStatus::Withdrawn => {
                        withdrawn_branches.insert(branch.id.clone());
                    }
                }
                state.branch_results.insert(branch.id.clone(), result);
            }
        }
    } else {
        // The claim context no longer applies. Existing branches remain
        // historical records, and are explained as withdrawn by that event.
        if let Some(branches) = graph.branches.get(id) {
            for branch in branches {
                let origin = claim_context_cause.clone().ok_or_else(|| {
                    DerivationError::InvalidInput(
                        "claim context cause disappeared during evaluation".into(),
                    )
                })?;
                withdrawn_branches.insert(branch.id.clone());
                state.branch_results.insert(
                    branch.id.clone(),
                    BranchResult {
                        status: BranchStatus::Withdrawn,
                        discharge_paths: Vec::new(),
                        link: None,
                        cause: Some(cause(origin, vec![branch.id.clone()])),
                    },
                );
            }
        }
    }

    let state_value = match (support.is_empty(), counterevidence.is_empty()) {
        (true, true) => ClaimState::Unresolved,
        (false, true) => ClaimState::Supported,
        (true, false) => ClaimState::Refuted,
        (false, false) => ClaimState::Disputed,
    };
    let claim_cause = match state_value {
        ClaimState::Supported | ClaimState::Disputed => None,
        ClaimState::Refuted => Some(cause(
            missing_origin(
                "counterevidence_without_support",
                counterevidence
                    .first()
                    .map_or_else(|| id.to_string(), |link| link.source.clone()),
                "applicable counterevidence remains but no admitted support applies".into(),
            ),
            vec![id.to_string()],
        )),
        ClaimState::Unresolved => {
            let branch_cause = withdrawn_branches
                .iter()
                .filter_map(|branch| state.branch_results.get(branch))
                .find(|result| result.status == BranchStatus::Withdrawn)
                .and_then(|result| result.cause.clone());
            let evidence_cause = state
                .evidence_results
                .iter()
                .filter(|(record, result)| {
                    !result.effective
                        && graph
                            .evidence
                            .get(record.as_str())
                            .is_some_and(|evidence| evidence.claim == id)
                })
                .find_map(|(_, result)| result.cause.clone());
            claim_context_cause
                .clone()
                .map(|origin| cause(origin, vec![id.to_string()]))
                .or(branch_cause)
                .or(evidence_cause)
                .or_else(|| {
                    Some(cause(
                        missing_origin(
                            "no_applicable_support",
                            id.to_string(),
                            "no admitted support applies to this exact proposition and context"
                                .into(),
                        ),
                        vec![id.to_string()],
                    ))
                })
        }
    };
    let residual_assumptions = support
        .iter()
        .chain(&counterevidence)
        .flat_map(|admitted| admitted.residual_assumptions.iter().cloned())
        .collect::<BTreeSet<_>>();
    state.cache.insert(
        id.to_string(),
        ClaimEval {
            state: state_value,
            support,
            counterevidence,
            unresolved_premises,
            residual_assumptions,
            surviving_branches,
            withdrawn_branches,
            cause: claim_cause,
        },
    );
    state.visiting.remove(id);
    Ok(())
}

fn run_graph(graph: &Graph<'_>) -> Result<EvalState, DerivationError> {
    let mut state = EvalState::default();
    for id in graph.claims.keys() {
        eval_claim(id, graph, &mut state)?;
    }
    // A claim may have been skipped after a cycle-free run only if it was not
    // declared; validation guarantees every claim is reachable here.
    Ok(state)
}

fn state_reason(state: ClaimState) -> &'static str {
    match state {
        ClaimState::Supported => {
            "at least one admitted support alternative applies and no applicable counterevidence remains"
        }
        ClaimState::Refuted => {
            "only applicable counterevidence remains; no admitted support alternative applies"
        }
        ClaimState::Disputed => {
            "applicable support and counterevidence are both retained; a surviving alternative does not cancel declared counterevidence"
        }
        ClaimState::Unresolved => {
            "no admitted support alternative and no applicable counterevidence remain for this exact proposition, revision and context"
        }
    }
}

fn obligation_status(claim: &ClaimMaintenance, affected: bool) -> (&'static str, String) {
    if !claim.current {
        return (
            "historical_claim_superseded",
            "the obligation names a superseded claim revision; advisory reevaluation applies to the current revision"
                .into(),
        );
    }
    if claim.state == ClaimState::Unresolved {
        return (
            "support_lost",
            "no current support alternative survives; the obligation requires advisory reevaluation"
                .into(),
        );
    }
    if claim.state == ClaimState::Refuted {
        return (
            "support_refuted",
            "current counterevidence applies without surviving support; the obligation requires advisory reevaluation"
                .into(),
        );
    }
    if claim.state == ClaimState::Disputed {
        return (
            "disputed_support",
            "support and counterevidence both remain; the dispute is not resolved by this analysis"
                .into(),
        );
    }
    if affected {
        return (
            "alternative_withdrawn",
            "the claim still has support, but at least one alternative or dependency was withdrawn"
                .into(),
        );
    }
    ("unchanged", "no support-maintenance effect applies".into())
}

// r[impl cairn.support_maintenance.alternatives]
// r[impl cairn.support_maintenance.invalidation]
// r[impl cairn.support_maintenance.dispute]
// r[impl cairn.support_maintenance.revisions]
// r[impl cairn.support_maintenance.determinism]
pub fn evaluate_support_maintenance(
    policy: &SupportPolicy,
    case: &SupportCase,
) -> Result<MaintenanceReport, DerivationError> {
    admit_support_policy(policy)?;
    if case.schema != SUPPORT_CASE_SCHEMA_V1 {
        return Err(DerivationError::Unsupported(format!(
            "unsupported support-maintenance case schema {}",
            case.schema
        )));
    }
    let bounds = &policy.bounds;
    if case.claims.len() > bounds.max_claims {
        return Err(DerivationError::BoundExhaustion(format!(
            "claims {} exceed the admitted bound {}",
            case.claims.len(),
            bounds.max_claims
        )));
    }
    if case.branches.len() > bounds.max_branches {
        return Err(DerivationError::BoundExhaustion(format!(
            "branches {} exceed the admitted bound {}",
            case.branches.len(),
            bounds.max_branches
        )));
    }
    if case.evidence.len() > bounds.max_evidence {
        return Err(DerivationError::BoundExhaustion(format!(
            "evidence {} exceeds the admitted bound {}",
            case.evidence.len(),
            bounds.max_evidence
        )));
    }
    if case.changes.len() > bounds.max_events {
        return Err(DerivationError::BoundExhaustion(format!(
            "changes {} exceed the admitted bound {}",
            case.changes.len(),
            bounds.max_events
        )));
    }
    if case.obligations.len() > bounds.max_obligations {
        return Err(DerivationError::BoundExhaustion(format!(
            "obligations {} exceed the admitted bound {}",
            case.obligations.len(),
            bounds.max_obligations
        )));
    }

    let mut claims = BTreeMap::new();
    let mut context_ids = BTreeMap::new();
    let mut context_cache: BTreeMap<&ClaimContext, String> = BTreeMap::new();
    for claim in &case.claims {
        let id = claim_identity(claim)?;
        if claims.insert(id.clone(), claim).is_some() {
            return Err(DerivationError::InvalidInput(
                "duplicate claim identity in support case".into(),
            ));
        }
        let context_id = if let Some(context_id) = context_cache.get(&claim.context) {
            context_id.clone()
        } else {
            let context_id = support_context_identity(&claim.context)?;
            context_cache.insert(&claim.context, context_id.clone());
            context_id
        };
        context_ids.insert(id, context_id);
    }
    let mut known_evidence = BTreeSet::new();
    let mut evidence = BTreeMap::new();
    for record in &case.evidence {
        validate_record(record, &claims, &known_evidence)?;
        known_evidence.insert(record.id.clone());
        evidence.insert(record.id.clone(), record);
    }
    let mut obligations = BTreeMap::new();
    for obligation in &case.obligations {
        if !non_empty(&obligation.id)
            || !claims.contains_key(&obligation.claim)
            || !non_empty(&obligation.description)
        {
            return Err(DerivationError::InvalidInput(
                "obligation requires a non-empty identity, description and declared claim".into(),
            ));
        }
        if obligations
            .insert(obligation.id.as_str(), obligation)
            .is_some()
        {
            return Err(DerivationError::InvalidInput(format!(
                "duplicate obligation identity {}",
                obligation.id
            )));
        }
    }

    let mut branch_ids = BTreeSet::new();
    let mut prerequisites: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut branches: BTreeMap<&str, Vec<&SupportBranch>> = BTreeMap::new();
    let mut assumptions = BTreeSet::new();
    for branch in &case.branches {
        if !non_empty(&branch.id)
            || !non_empty(&branch.rule)
            || !non_empty(&branch.assurance_scope)
            || branch.version == 0
        {
            return Err(DerivationError::InvalidInput(
                "branch requires a non-empty identity, rule, version and assurance scope".into(),
            ));
        }
        if !branch_ids.insert(branch.id.as_str()) {
            return Err(DerivationError::InvalidInput(format!(
                "duplicate branch identity {}",
                branch.id
            )));
        }
        let conclusion = claims.get(&branch.conclusion).ok_or_else(|| {
            DerivationError::InvalidInput(format!(
                "branch {} conclusion is not a declared claim",
                branch.id
            ))
        })?;
        if branch.applicability != conclusion.context {
            return Err(DerivationError::ScopeMismatch(format!(
                "branch {} applicability differs from its conclusion scope",
                branch.id
            )));
        }
        if branch.premises.is_empty() {
            return Err(DerivationError::InvalidInput(format!(
                "branch {} requires at least one premise",
                branch.id
            )));
        }
        if branch.premises.len() > bounds.max_premises {
            return Err(DerivationError::BoundExhaustion(format!(
                "branch {} premises {} exceed the admitted bound {}",
                branch.id,
                branch.premises.len(),
                bounds.max_premises
            )));
        }
        let mut unique = BTreeSet::new();
        for premise in &branch.premises {
            if !unique.insert(premise.as_str()) {
                return Err(DerivationError::InvalidInput(format!(
                    "branch {} repeats a premise",
                    branch.id
                )));
            }
            if premise == &branch.conclusion {
                return Err(DerivationError::InvalidInput(format!(
                    "branch {} cannot use its own conclusion as a premise",
                    branch.id
                )));
            }
            let premise_claim = claims.get(premise).ok_or_else(|| {
                DerivationError::InvalidInput(format!(
                    "branch {} references undeclared premise {premise}",
                    branch.id
                ))
            })?;
            if premise_claim.context != conclusion.context {
                return Err(DerivationError::ScopeMismatch(format!(
                    "branch {} combines premise {premise} from another subject, environment or bound scope",
                    branch.id
                )));
            }
        }
        if branch
            .assumptions
            .iter()
            .any(|assumption| !non_empty(assumption))
        {
            return Err(DerivationError::InvalidInput(format!(
                "branch {} has an empty assumption",
                branch.id
            )));
        }
        assumptions.extend(branch.assumptions.iter().cloned());
        let mut premises = branch.premises.clone();
        premises.sort();
        prerequisites.insert(branch.id.as_str(), premises);
        branches
            .entry(branch.conclusion.as_str())
            .or_default()
            .push(branch);
    }
    assumptions.extend(
        case.evidence
            .iter()
            .flat_map(|record| record.assumptions.iter().cloned()),
    );
    if assumptions.len() > bounds.max_assumptions {
        return Err(DerivationError::BoundExhaustion(format!(
            "distinct assumptions {} exceed the admitted bound {}",
            assumptions.len(),
            bounds.max_assumptions
        )));
    }

    // Cross-revision relations are explicit branches, never inferred from
    // revision classifications.
    for branch in &case.branches {
        let conclusion = claims.get(&branch.conclusion).ok_or_else(|| {
            DerivationError::InvalidInput(format!(
                "branch {} conclusion is not a declared claim",
                branch.id
            ))
        })?;
        let mut cross_revision = false;
        for premise in &branch.premises {
            let premise_claim = claims.get(premise).ok_or_else(|| {
                DerivationError::InvalidInput(format!(
                    "branch {} references undeclared premise {premise}",
                    branch.id
                ))
            })?;
            if premise_claim.requirement != conclusion.requirement
                || premise_claim.interpretation_blake3 != conclusion.interpretation_blake3
                || premise_claim.contract_blake3 != conclusion.contract_blake3
            {
                cross_revision = true;
            }
        }
        match (&branch.cross_revision_relation, cross_revision) {
            (None, true) => {
                return Err(DerivationError::ScopeMismatch(format!(
                    "branch {} crosses requirement or interpretation revisions without an explicitly admitted relation",
                    branch.id
                )));
            }
            (Some(relation), true) if !digest(relation) => {
                return Err(DerivationError::InvalidInput(format!(
                    "branch {} requires an exact admitted relation identity",
                    branch.id
                )));
            }
            (Some(_), false) => {
                return Err(DerivationError::InvalidInput(format!(
                    "branch {} declares a cross-revision relation for one requirement revision",
                    branch.id
                )));
            }
            _ => (),
        }
    }

    for values in branches.values_mut() {
        values.sort_by(|left, right| left.id.cmp(&right.id));
    }
    let evidence_by_claim = {
        let mut index: BTreeMap<&str, Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in evidence.values() {
            index
                .entry(record.claim.as_str())
                .or_default()
                .push(*record);
        }
        for values in index.values_mut() {
            values.sort_by(|left, right| left.id.cmp(&right.id));
        }
        index
    };

    let effective = apply_changes(policy, case, &claims, &context_ids)?;
    let mut all_evidence = evidence.clone();
    let mut admitted = Vec::new();
    for record in &effective.admitted_evidence {
        admitted.push(record);
    }
    for record in &admitted {
        all_evidence.insert(record.id.clone(), *record);
    }
    if all_evidence.len() > bounds.max_evidence {
        return Err(DerivationError::BoundExhaustion(format!(
            "effective evidence {} exceeds the admitted bound {}",
            all_evidence.len(),
            bounds.max_evidence
        )));
    }
    for record in &effective.admitted_evidence {
        assumptions.extend(record.assumptions.iter().cloned());
    }
    if assumptions.len() > bounds.max_assumptions {
        return Err(DerivationError::BoundExhaustion(format!(
            "effective assumptions {} exceed the admitted bound {}",
            assumptions.len(),
            bounds.max_assumptions
        )));
    }

    let no_invalidated_evidence = BTreeMap::new();
    let no_withdrawn_assumptions = BTreeMap::new();
    let no_policy_changes = BTreeMap::new();
    let no_context_changes = BTreeMap::new();
    let baseline_graph = Graph {
        claims: &claims,
        branches: &branches,
        evidence: &evidence,
        evidence_by_claim: &evidence_by_claim,
        premises: &prerequisites,
        context_ids: &context_ids,
        invalidated_evidence: &no_invalidated_evidence,
        withdrawn_assumptions: &no_withdrawn_assumptions,
        policy_changes: &no_policy_changes,
        context_changes: &no_context_changes,
        max_depth: bounds.max_depth,
    };
    let baseline = run_graph(&baseline_graph)?;

    let current_evidence_by_claim = {
        let mut index: BTreeMap<&str, Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in all_evidence.values() {
            index
                .entry(record.claim.as_str())
                .or_default()
                .push(*record);
        }
        for values in index.values_mut() {
            values.sort_by(|left, right| left.id.cmp(&right.id));
        }
        index
    };
    let current = {
        let current_graph = Graph {
            claims: &claims,
            branches: &branches,
            evidence: &all_evidence,
            evidence_by_claim: &current_evidence_by_claim,
            premises: &prerequisites,
            context_ids: &context_ids,
            invalidated_evidence: &effective.invalidated_evidence,
            withdrawn_assumptions: &effective.withdrawn_assumptions,
            policy_changes: &effective.policy_changes,
            context_changes: &effective.context_changes,
            max_depth: bounds.max_depth,
        };
        run_graph(&current_graph)?
    };

    let invalidated_claim_ids = effective
        .invalidated_evidence
        .keys()
        .filter_map(|evidence_id| all_evidence.get(evidence_id.as_str()))
        .map(|record| record.claim.clone())
        .collect::<BTreeSet<_>>();

    let mut branches_out = Vec::with_capacity(case.branches.len());
    let mut affected_claims = BTreeSet::new();
    for branch in &case.branches {
        let result = current.branch_results.get(&branch.id).cloned();
        let status = result
            .as_ref()
            .map_or(BranchStatus::Withdrawn, |result| result.status);
        let link = result.as_ref().and_then(|result| result.link.clone());
        let cause = result
            .as_ref()
            .and_then(|result| result.cause.clone())
            .or_else(|| {
                (status == BranchStatus::Withdrawn).then(|| {
                    cause(
                        missing_origin(
                            "not_evaluated",
                            branch.id.clone(),
                            "branch was not reached by the bounded evaluation".into(),
                        ),
                        vec![branch.id.clone()],
                    )
                })
            });
        let discharge_paths = result
            .as_ref()
            .map(|result| result.discharge_paths.clone())
            .unwrap_or_default();
        if status == BranchStatus::Withdrawn {
            affected_claims.insert(branch.conclusion.clone());
        }
        let mut premises = branch.premises.clone();
        premises.sort();
        branches_out.push(BranchExplanation {
            id: branch.id.clone(),
            conclusion: branch.conclusion.clone(),
            polarity: branch.polarity,
            rule: branch.rule.clone(),
            version: branch.version,
            premises,
            assumptions: branch.assumptions.clone(),
            assurance_scope: branch.assurance_scope.clone(),
            applicability: branch.applicability.clone(),
            cross_revision_relation: branch.cross_revision_relation.clone(),
            status,
            discharge_paths,
            link,
            cause,
        });
    }

    let mut claims_out = Vec::with_capacity(claims.len());
    let mut newly_unresolved = BTreeSet::new();
    let mut trust_dependencies = BTreeSet::new();
    branches_out.sort_by(|left, right| left.id.cmp(&right.id));
    for (id, claim) in &claims {
        let baseline_eval = baseline.cache.get(id).cloned().ok_or_else(|| {
            DerivationError::InvalidInput(format!("claim {id} has no baseline evaluation"))
        })?;
        let current_eval = current.cache.get(id).cloned().ok_or_else(|| {
            DerivationError::InvalidInput(format!("claim {id} has no current evaluation"))
        })?;
        let revision = effective.superseded.get(id);
        let current_flag = revision.is_none();
        if current_flag
            && baseline_eval.state != ClaimState::Unresolved
            && current_eval.state == ClaimState::Unresolved
        {
            newly_unresolved.insert(id.clone());
        }
        if baseline_eval.state != current_eval.state {
            affected_claims.insert(id.clone());
        }
        if !current_flag || !current_eval.withdrawn_branches.is_empty() {
            affected_claims.insert(id.clone());
        }
        if effective
            .policy_changes
            .contains_key(&claim.context.policy_blake3)
            || context_ids
                .get(id)
                .is_some_and(|context_id| effective.context_changes.contains_key(context_id))
        {
            affected_claims.insert(id.clone());
        }
        let mut support = current_eval.support;
        let mut counterevidence = current_eval.counterevidence;
        support.sort();
        counterevidence.sort();
        let obligations = obligations
            .values()
            .filter(|obligation| &obligation.claim == id)
            .map(|obligation| obligation.id.clone())
            .collect::<Vec<_>>();
        let mut surviving_branches = current_eval
            .surviving_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        surviving_branches.sort();
        let mut withdrawn_branches = current_eval
            .withdrawn_branches
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        withdrawn_branches.sort();
        let residual_assumptions = support
            .iter()
            .chain(&counterevidence)
            .flat_map(|link| link.residual_assumptions.iter().cloned())
            .collect::<BTreeSet<_>>();
        trust_dependencies.extend(residual_assumptions.iter().cloned());
        claims_out.push(ClaimMaintenance {
            claim_id: id.clone(),
            claim: (**claim).clone(),
            baseline_state: baseline_eval.state,
            state: current_eval.state,
            state_reason: state_reason(current_eval.state).into(),
            current: current_flag,
            superseded_by: revision.map(|revision| revision.to_claim.clone()),
            revision_classification: revision.map(|revision| revision.classification.clone()),
            support,
            counterevidence,
            unresolved_premises: current_eval.unresolved_premises,
            residual_assumptions,
            surviving_branches,
            withdrawn_branches,
            obligations,
            cause: current_eval.cause,
        });
    }
    affected_claims.extend(invalidated_claim_ids);

    let mut evidence_out = Vec::with_capacity(all_evidence.len());
    for (id, record) in &all_evidence {
        let result = current.evidence_results.get(id).cloned();
        let (effective_flag, link, cause) = result.map_or((false, None, None), |result| {
            (result.effective, result.link, result.cause)
        });
        evidence_out.push(EvidenceExplanation {
            evidence: (**record).clone(),
            historical: evidence.contains_key(id),
            admitted_sequence: effective.admitted_sequences.get(id).copied(),
            effective: effective_flag,
            link,
            cause,
        });
    }

    let mut obligations_out = Vec::with_capacity(obligations.len());
    for obligation in obligations.values() {
        let claim = claims_out
            .iter()
            .find(|claim| claim.claim_id == obligation.claim)
            .ok_or_else(|| {
                DerivationError::InvalidInput(format!(
                    "obligation {} references an unevaluated claim",
                    obligation.id
                ))
            })?;
        let affected = affected_claims.contains(&obligation.claim);
        let (status, detail) = obligation_status(claim, affected);
        obligations_out.push(ObligationImpact {
            id: obligation.id.clone(),
            claim: obligation.claim.clone(),
            status: status.into(),
            detail,
        });
    }

    #[derive(Serialize)]
    struct CanonicalBranch<'a> {
        id: &'a str,
        conclusion: &'a str,
        polarity: BranchPolarity,
        rule: &'a str,
        version: u32,
        premises: Vec<&'a str>,
        assumptions: &'a BTreeSet<String>,
        assurance_scope: &'a str,
        applicability: &'a ClaimContext,
        cross_revision_relation: &'a Option<String>,
    }
    let canonical_claims = claims.values().cloned().collect::<Vec<_>>();
    let canonical_branches = {
        let mut values = case.branches.iter().collect::<Vec<_>>();
        values.sort_by(|left, right| left.id.cmp(&right.id));
        values
            .into_iter()
            .map(|branch| {
                let mut premises = branch
                    .premises
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                premises.sort_unstable();
                CanonicalBranch {
                    id: &branch.id,
                    conclusion: &branch.conclusion,
                    polarity: branch.polarity,
                    rule: &branch.rule,
                    version: branch.version,
                    premises,
                    assumptions: &branch.assumptions,
                    assurance_scope: &branch.assurance_scope,
                    applicability: &branch.applicability,
                    cross_revision_relation: &branch.cross_revision_relation,
                }
            })
            .collect::<Vec<_>>()
    };
    let canonical_evidence = {
        let mut values = case.evidence.iter().collect::<Vec<_>>();
        values.sort_by(|left, right| left.id.cmp(&right.id));
        values
    };
    let canonical_changes = {
        let mut values = case.changes.iter().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            (left.sequence, left.id.as_str()).cmp(&(right.sequence, right.id.as_str()))
        });
        values
    };
    let canonical_obligations = {
        let mut values = case.obligations.iter().collect::<Vec<_>>();
        values.sort_by(|left, right| left.id.cmp(&right.id));
        values
    };
    #[derive(Serialize)]
    struct CanonicalInput<'a> {
        schema: &'a str,
        claims: Vec<&'a ScopedClaim>,
        branches: Vec<CanonicalBranch<'a>>,
        evidence: Vec<&'a EvidenceRecord>,
        changes: Vec<&'a ChangeEvent>,
        obligations: Vec<&'a LifecycleObligation>,
    }
    let input_blake3 = identity(
        "cairn.support-maintenance-input.v1",
        &(
            policy,
            CanonicalInput {
                schema: &case.schema,
                claims: canonical_claims,
                branches: canonical_branches,
                evidence: canonical_evidence,
                changes: canonical_changes,
                obligations: canonical_obligations,
            },
        ),
    )?;

    let max_depth_observed = baseline.max_depth_observed.max(current.max_depth_observed);
    let mut report = MaintenanceReport {
        schema: SUPPORT_REPORT_SCHEMA_V1.into(),
        advisory: true,
        claims: claims_out,
        branches: branches_out,
        evidence: evidence_out,
        revisions: effective.revisions,
        events: effective.applied,
        newly_unresolved,
        affected_claims,
        affected_obligations: obligations_out,
        trust_dependencies,
        independence: INDEPENDENCE_NOT_ESTABLISHED.into(),
        analysis: ClaimAnalysisStatus {
            mode: "full_recomputation".into(),
            complete: true,
            incremental: false,
            note: "Full deterministic recomputation is the only evaluator; no incremental implementation or automatic belief repair is provided.".into(),
        },
        bounds: BoundUsage {
            limits: bounds.clone(),
            claims: case.claims.len(),
            branches: case.branches.len(),
            evidence: all_evidence.len(),
            events: case.changes.len(),
            obligations: case.obligations.len(),
            max_depth_observed,
        },
        input_blake3,
        report_blake3: String::new(),
        boundary: SUPPORT_BOUNDARY.into(),
    };
    let report_blake3 = identity("cairn.support-maintenance-report.v1", &report)?;
    report.report_blake3 = report_blake3;
    Ok(report)
}

#[cfg(test)]
mod tests;

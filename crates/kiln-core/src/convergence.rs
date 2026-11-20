//! Frozen evidence campaigns selected from a Cairn convergence receipt.
//!
//! The adapter admits the receipt facts; this module owns the provider-neutral
//! selection, the frozen revision identity, and the typed Verify-stage build
//! requests. It performs no I/O and requests no effect.
//!
//! A Cairn `current` row, a Mantle success and a cache hit are advisory. Only
//! the checked relations below are decided here.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::build::{BuildEffectPlan, validate_build_effect_plan};
use crate::domain::Outcome;
use crate::gating::CandidateSet;
use crate::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderRole, admit_operation,
};
use crate::types::{
    Alias, AttemptId, CONTRACT_VERSION_V1, CandidateId, ContentId, CoreResult, Diagnostic,
    EffectId, PipelineRevisionId, ProviderProfileId,
};

/// Schema of the frozen evidence campaign revision.
pub const EVIDENCE_CAMPAIGN_SCHEMA_V1: &str = "kiln.evidence-campaign.v1";

/// Named bounds for admitted convergence rows and campaign items.
pub const MAX_CONVERGENCE_ROWS: usize = 4096;
pub const MAX_CONVERGENCE_ITEMS: usize = 4096;
pub const MAX_CONVERGENCE_ID_BYTES: usize = 128;
pub const MAX_CONVERGENCE_PATH_BYTES: usize = 512;
pub const MAX_CONVERGENCE_REVISION_BYTES: usize = 128;
/// Products one evidence build may require.
pub const MAX_EVIDENCE_PRODUCTS: usize = 64;

/// One row state from `cairn.convergence.v1`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Untargeted,
    NotProjected,
    Current,
    Unrealized,
    Stale,
    Failed,
    Unsupported,
    Incomplete,
}

impl EvidenceState {
    /// True only for a targeted row that lacks current bound evidence.
    #[must_use]
    pub const fn is_unsatisfied(self) -> bool {
        matches!(
            self,
            Self::Unrealized | Self::Stale | Self::Failed | Self::Unsupported | Self::Incomplete
        )
    }
}

/// Why a row was not selected into the unsatisfied set.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    /// Current bound evidence exists for the declared assurance target.
    Current,
    /// A reviewed target's row binding holds; no evidence job exists.
    NotProjected,
    /// The registry row declares no assurance target.
    Untargeted,
}

impl ExclusionReason {
    /// The exclusion reason for one state, when the state is excluded.
    #[must_use]
    pub const fn for_state(state: EvidenceState) -> Option<Self> {
        match state {
            EvidenceState::Current => Some(Self::Current),
            EvidenceState::NotProjected => Some(Self::NotProjected),
            EvidenceState::Untargeted => Some(Self::Untargeted),
            EvidenceState::Unrealized
            | EvidenceState::Stale
            | EvidenceState::Failed
            | EvidenceState::Unsupported
            | EvidenceState::Incomplete => None,
        }
    }
}

/// Provider-neutral row facts Kiln needs for selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRow {
    pub requirement_id: String,
    pub specification_path: String,
    pub row_blake3: ContentId,
    pub state: EvidenceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

/// One excluded row with its retained reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedRow {
    pub requirement_id: String,
    pub state: EvidenceState,
    pub reason: ExclusionReason,
}

/// The unsatisfied set and every exclusion, in registry order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsatisfiedSelection {
    pub items: Vec<EvidenceRow>,
    pub exclusions: Vec<ExcludedRow>,
}

// r[impl kiln.convergence.unsatisfied]
/// Select exactly the targeted rows without current evidence.
pub fn select_unsatisfied(rows: &[EvidenceRow]) -> CoreResult<UnsatisfiedSelection> {
    if rows.is_empty() {
        return Err(Diagnostic::new(
            "convergence_rows_empty",
            "convergence.rows",
            "a convergence receipt must carry registry rows",
        ));
    }
    if rows.len() > MAX_CONVERGENCE_ROWS {
        return Err(Diagnostic::new(
            "convergence_row_bound",
            "convergence.rows",
            "convergence row count exceeds its named bound",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    let mut exclusions = Vec::new();
    for row in rows {
        validate_row(row)?;
        if !seen.insert(row.requirement_id.as_str()) {
            return Err(Diagnostic::new(
                "convergence_duplicate_requirement",
                "convergence.rows.requirement_id",
                "a requirement identity appears more than once",
            ));
        }
        match ExclusionReason::for_state(row.state) {
            Some(reason) => exclusions.push(ExcludedRow {
                requirement_id: row.requirement_id.clone(),
                state: row.state,
                reason,
            }),
            None => items.push(row.clone()),
        }
    }
    Ok(UnsatisfiedSelection { items, exclusions })
}

fn validate_row(row: &EvidenceRow) -> CoreResult<()> {
    bounded(
        &row.requirement_id,
        MAX_CONVERGENCE_ID_BYTES,
        "convergence.rows.requirement_id",
    )?;
    bounded(
        &row.specification_path,
        MAX_CONVERGENCE_PATH_BYTES,
        "convergence.rows.specification_path",
    )?;
    row.row_blake3.validate()?;
    if let Some(contract) = &row.contract_id {
        bounded(
            contract,
            MAX_CONVERGENCE_ID_BYTES,
            "convergence.rows.contract_id",
        )?;
    }
    if let Some(job) = &row.job_id {
        bounded(job, MAX_CONVERGENCE_ID_BYTES, "convergence.rows.job_id")?;
    }
    Ok(())
}

/// Identity binding for one frozen campaign revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvergenceCampaignBinding {
    pub candidate_id: CandidateId,
    /// BLAKE3 of the existing `CandidateSet` value.
    pub candidate_set_identity: ContentId,
    pub receipt_blake3: ContentId,
    pub revision_kind: String,
    pub revision: String,
    pub registry_blake3: ContentId,
    pub case_blake3: ContentId,
    pub policy_blake3: ContentId,
}

/// One item that the Verify stage must build again.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCampaignItem {
    pub requirement_id: String,
    pub specification_path: String,
    pub row_blake3: ContentId,
    pub state: EvidenceState,
    pub contract_id: String,
    pub job_id: String,
}

/// One immutable campaign revision minted from an unsatisfied set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCampaignRevision {
    pub schema: String,
    pub campaign_id: ContentId,
    pub candidate_id: CandidateId,
    pub candidate_set_identity: ContentId,
    pub receipt_blake3: ContentId,
    pub revision_kind: String,
    pub revision: String,
    pub registry_blake3: ContentId,
    pub case_blake3: ContentId,
    pub policy_blake3: ContentId,
    pub items: Vec<EvidenceCampaignItem>,
}

// r[impl kiln.convergence.campaign]
/// Mint at most one frozen revision from the selected set.
///
/// An empty set mints no revision. The returned revision is immutable by value;
/// repeating the same facts returns an equal revision.
pub fn mint_evidence_campaign(
    binding: &ConvergenceCampaignBinding,
    selection: &UnsatisfiedSelection,
) -> CoreResult<Option<EvidenceCampaignRevision>> {
    validate_binding(binding)?;
    if selection.items.is_empty() {
        return Ok(None);
    }
    if selection.items.len() > MAX_CONVERGENCE_ITEMS {
        return Err(Diagnostic::new(
            "campaign_item_bound",
            "convergence.campaign.items",
            "campaign item count exceeds its named bound",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut items = Vec::with_capacity(selection.items.len());
    for row in &selection.items {
        if !row.state.is_unsatisfied() {
            return Err(Diagnostic::new(
                "campaign_item_state",
                "convergence.campaign.items",
                "a campaign item must name an unsatisfied evidence state",
            ));
        }
        if !seen.insert(row.requirement_id.as_str()) {
            return Err(Diagnostic::new(
                "campaign_duplicate_requirement",
                "convergence.campaign.items.requirement_id",
                "a campaign repeats a requirement identity",
            ));
        }
        let Some(contract_id) = row.contract_id.clone() else {
            return Err(Diagnostic::new(
                "campaign_item_job_missing",
                "convergence.campaign.items.contract_id",
                "a campaign item must name its evidence contract",
            ));
        };
        let Some(job_id) = row.job_id.clone() else {
            return Err(Diagnostic::new(
                "campaign_item_job_missing",
                "convergence.campaign.items.job_id",
                "a campaign item must name its evidence job",
            ));
        };
        items.push(EvidenceCampaignItem {
            requirement_id: row.requirement_id.clone(),
            specification_path: row.specification_path.clone(),
            row_blake3: row.row_blake3.clone(),
            state: row.state,
            contract_id,
            job_id,
        });
    }
    let campaign_id = campaign_identity(binding, &items)?;
    Ok(Some(EvidenceCampaignRevision {
        schema: EVIDENCE_CAMPAIGN_SCHEMA_V1.to_string(),
        campaign_id,
        candidate_id: binding.candidate_id.clone(),
        candidate_set_identity: binding.candidate_set_identity.clone(),
        receipt_blake3: binding.receipt_blake3.clone(),
        revision_kind: binding.revision_kind.clone(),
        revision: binding.revision.clone(),
        registry_blake3: binding.registry_blake3.clone(),
        case_blake3: binding.case_blake3.clone(),
        policy_blake3: binding.policy_blake3.clone(),
        items,
    }))
}

fn validate_binding(binding: &ConvergenceCampaignBinding) -> CoreResult<()> {
    binding.candidate_set_identity.validate()?;
    binding.receipt_blake3.validate()?;
    binding.registry_blake3.validate()?;
    binding.case_blake3.validate()?;
    binding.policy_blake3.validate()?;
    bounded(
        &binding.revision_kind,
        MAX_CONVERGENCE_REVISION_BYTES,
        "convergence.campaign.revision_kind",
    )?;
    bounded(
        &binding.revision,
        MAX_CONVERGENCE_REVISION_BYTES,
        "convergence.campaign.revision",
    )?;
    Ok(())
}

fn campaign_identity(
    binding: &ConvergenceCampaignBinding,
    items: &[EvidenceCampaignItem],
) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln.evidence-campaign.v1");
    update_str(&mut hasher, binding.candidate_id.as_str());
    update_str(&mut hasher, binding.candidate_set_identity.as_str());
    update_str(&mut hasher, binding.receipt_blake3.as_str());
    update_str(&mut hasher, &binding.revision_kind);
    update_str(&mut hasher, &binding.revision);
    update_str(&mut hasher, binding.registry_blake3.as_str());
    update_str(&mut hasher, binding.case_blake3.as_str());
    update_str(&mut hasher, binding.policy_blake3.as_str());
    update_usize(&mut hasher, items.len());
    for item in items {
        update_str(&mut hasher, &item.requirement_id);
        update_str(&mut hasher, &item.specification_path);
        update_str(&mut hasher, item.row_blake3.as_str());
        update_str(&mut hasher, state_name(item.state));
        update_str(&mut hasher, &item.contract_id);
        update_str(&mut hasher, &item.job_id);
    }
    digest(hasher)
}

/// Domain-separated BLAKE3 of the existing candidate-set value.
///
/// A moved target, policy or frozen source produces a different identity, so a
/// campaign cannot transfer to another candidate.
pub fn candidate_set_identity(candidate: &CandidateSet) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln.candidate-set.v1");
    update_str(&mut hasher, candidate.id.as_str());
    update_str(&mut hasher, candidate.policy_id.as_str());
    update_usize(&mut hasher, candidate.ordered_changes.len());
    for change in &candidate.ordered_changes {
        update_external_ref(&mut hasher, &change.change)?;
        update_external_ref(&mut hasher, &change.target_revision)?;
        update_str(&mut hasher, change.source_snapshot.as_str());
        update_str(&mut hasher, change.pipeline_revision.as_str());
        update_str(&mut hasher, change.collaboration_provider.as_str());
    }
    update_usize(&mut hasher, candidate.dependencies.len());
    for edge in &candidate.dependencies {
        update_usize(&mut hasher, edge.before_index);
        update_usize(&mut hasher, edge.after_index);
    }
    update_usize(&mut hasher, candidate.frozen_sources.len());
    for (source, snapshot) in &candidate.frozen_sources {
        update_external_ref(&mut hasher, source)?;
        update_str(&mut hasher, snapshot.as_str());
    }
    digest(hasher)
}

/// How the Verify stage wants the evidence job realized.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRealization {
    /// A fresh `mantle build --plan` over the pinned cohort.
    BuildFromPlan,
    /// A preserve/realize route the pinned adapter does not expose.
    PreserveCacheOnly,
}

impl EvidenceRealization {
    const fn name(self) -> &'static str {
        match self {
            Self::BuildFromPlan => "build-from-plan",
            Self::PreserveCacheOnly => "preserve-cache-only",
        }
    }
}

/// The plan and products for one evidence job.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePlanBinding {
    pub plan_identity: ContentId,
    pub required_products: Vec<Alias>,
}

/// Operator-selected provider facts for one Verify stage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBuildSelection {
    pub provider_profile: ProviderProfileId,
    pub policy_identity: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub platform: Alias,
    pub realization: EvidenceRealization,
    /// Exact plan per evidence job id.
    pub plans: BTreeMap<String, EvidencePlanBinding>,
}

/// One typed Verify-stage build request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBuildRequest {
    pub campaign_id: ContentId,
    pub candidate_set_identity: ContentId,
    pub candidate_id: CandidateId,
    pub requirement_id: String,
    pub specification_path: String,
    pub row_blake3: ContentId,
    pub contract_id: String,
    pub job_id: String,
    pub effect_id: EffectId,
    pub idempotency_key: ContentId,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub provider_profile: ProviderProfileId,
    pub platform: Alias,
    pub required_products: Vec<Alias>,
    pub realization: EvidenceRealization,
}

impl EvidenceBuildRequest {
    /// Lower this request to the existing build effect plan.
    pub fn into_build_effect_plan(&self, attempt_id: AttemptId) -> CoreResult<BuildEffectPlan> {
        let plan = BuildEffectPlan {
            effect_id: self.effect_id.clone(),
            attempt_id,
            idempotency_key: self.idempotency_key.clone(),
            candidate_id: self.candidate_id.clone(),
            pipeline_revision: self.pipeline_revision.clone(),
            plan_identity: self.plan_identity.clone(),
            policy_identity: self.policy_identity.clone(),
            provider_profile: self.provider_profile.clone(),
            platform: self.platform.clone(),
            required_products: self.required_products.clone(),
            constraints: None,
        };
        validate_build_effect_plan(&plan, MAX_EVIDENCE_PRODUCTS)?;
        Ok(plan)
    }
}

// r[impl kiln.convergence.verify]
/// Plan one typed evidence build per campaign item.
///
/// Capability, role, profile, outcome and realization admission all run before
/// any request is returned, so an unsupported provider or route has no effect.
pub fn plan_evidence_builds(
    revision: &EvidenceCampaignRevision,
    selection: &EvidenceBuildSelection,
    capability: &CapabilityProfile,
) -> CoreResult<Vec<EvidenceBuildRequest>> {
    admit_evidence_build_capability(selection, capability)?;
    if revision.items.is_empty() {
        return Err(Diagnostic::new(
            "campaign_empty",
            "convergence.verify.items",
            "a Verify stage requires at least one campaign item",
        ));
    }
    if revision.schema != EVIDENCE_CAMPAIGN_SCHEMA_V1 {
        return Err(Diagnostic::new(
            "campaign_schema",
            "convergence.verify.schema",
            "the campaign revision schema is not admitted",
        ));
    }
    selection.policy_identity.validate()?;
    for plan in selection.plans.values() {
        plan.plan_identity.validate()?;
        validate_products(&plan.required_products)?;
    }
    let jobs: BTreeSet<&str> = revision
        .items
        .iter()
        .map(|item| item.job_id.as_str())
        .collect();
    for job in &jobs {
        if !selection.plans.contains_key(*job) {
            return Err(Diagnostic::new(
                "evidence_plan_missing",
                "convergence.verify.plans",
                "a campaign evidence job has no exact plan",
            ));
        }
    }
    for job in selection.plans.keys() {
        if !jobs.contains(job.as_str()) {
            return Err(Diagnostic::new(
                "evidence_plan_unexpected",
                "convergence.verify.plans",
                "a selected plan does not belong to this campaign",
            ));
        }
    }
    let mut requests = Vec::with_capacity(revision.items.len());
    for item in &revision.items {
        let Some(plan) = selection.plans.get(&item.job_id) else {
            return Err(Diagnostic::new(
                "evidence_plan_missing",
                "convergence.verify.plans",
                "a campaign evidence job has no exact plan",
            ));
        };
        let idempotency_key = request_identity(revision, selection, item, plan)?;
        let effect_id = effect_identity(&idempotency_key)?;
        requests.push(EvidenceBuildRequest {
            campaign_id: revision.campaign_id.clone(),
            candidate_set_identity: revision.candidate_set_identity.clone(),
            candidate_id: revision.candidate_id.clone(),
            requirement_id: item.requirement_id.clone(),
            specification_path: item.specification_path.clone(),
            row_blake3: item.row_blake3.clone(),
            contract_id: item.contract_id.clone(),
            job_id: item.job_id.clone(),
            effect_id,
            idempotency_key,
            plan_identity: plan.plan_identity.clone(),
            policy_identity: selection.policy_identity.clone(),
            pipeline_revision: selection.pipeline_revision.clone(),
            provider_profile: selection.provider_profile.clone(),
            platform: selection.platform.clone(),
            required_products: plan.required_products.clone(),
            realization: selection.realization,
        });
    }
    Ok(requests)
}

fn admit_evidence_build_capability(
    selection: &EvidenceBuildSelection,
    capability: &CapabilityProfile,
) -> CoreResult<()> {
    let request = OperationRequest {
        contract_version: CONTRACT_VERSION_V1,
        selected_profile: selection.provider_profile.clone(),
        role: ProviderRole::Build,
        operation: Operation::Dispatch,
        request_bytes: 0,
        required_outcomes: required_outcomes(),
    };
    admit_operation(capability, &request)?;
    if selection.realization != EvidenceRealization::BuildFromPlan {
        return Err(Diagnostic::new(
            "unsupported_capability",
            "convergence.verify.realization",
            "the pinned Mantle adapter exposes only build --plan, not a preserve or realize operation",
        ));
    }
    Ok(())
}

fn required_outcomes() -> BTreeSet<Outcome> {
    [
        Outcome::Success,
        Outcome::DomainFailure,
        Outcome::Cancelled,
        Outcome::TimedOut,
        Outcome::ProviderError,
        Outcome::Unknown,
    ]
    .into_iter()
    .collect()
}

fn validate_products(products: &[Alias]) -> CoreResult<()> {
    if products.is_empty() || products.len() > MAX_EVIDENCE_PRODUCTS {
        return Err(Diagnostic::new(
            "build_product_bound",
            "build.required_products",
            "build product count is empty or exceeds its named bound",
        ));
    }
    let mut previous = None;
    for product in products {
        if previous.is_some_and(|previous| previous >= product) {
            return Err(Diagnostic::new(
                "build_product_order",
                "build.required_products",
                "build products must be sorted and unique",
            ));
        }
        previous = Some(product);
    }
    Ok(())
}

fn request_identity(
    revision: &EvidenceCampaignRevision,
    selection: &EvidenceBuildSelection,
    item: &EvidenceCampaignItem,
    plan: &EvidencePlanBinding,
) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln.evidence-build.v1");
    update_str(&mut hasher, revision.campaign_id.as_str());
    update_str(&mut hasher, revision.candidate_set_identity.as_str());
    update_str(&mut hasher, revision.candidate_id.as_str());
    update_str(&mut hasher, &item.requirement_id);
    update_str(&mut hasher, item.row_blake3.as_str());
    update_str(&mut hasher, &item.job_id);
    update_str(&mut hasher, plan.plan_identity.as_str());
    update_str(&mut hasher, selection.policy_identity.as_str());
    update_str(&mut hasher, selection.provider_profile.as_str());
    update_str(&mut hasher, selection.platform.as_str());
    update_str(&mut hasher, selection.realization.name());
    update_usize(&mut hasher, plan.required_products.len());
    for product in &plan.required_products {
        update_str(&mut hasher, product.as_str());
    }
    digest(hasher)
}

fn effect_identity(idempotency_key: &ContentId) -> CoreResult<EffectId> {
    let prefix: String = idempotency_key.as_str().chars().take(32).collect();
    EffectId::new(format!("evidence-{prefix}"))
}

fn bounded(value: &str, max_bytes: usize, field: &str) -> CoreResult<()> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(Diagnostic::new(
            "convergence_text_bound",
            field,
            "value is empty, oversized or contains control characters",
        ));
    }
    Ok(())
}

fn state_name(state: EvidenceState) -> &'static str {
    match state {
        EvidenceState::Untargeted => "untargeted",
        EvidenceState::NotProjected => "not_projected",
        EvidenceState::Current => "current",
        EvidenceState::Unrealized => "unrealized",
        EvidenceState::Stale => "stale",
        EvidenceState::Failed => "failed",
        EvidenceState::Unsupported => "unsupported",
        EvidenceState::Incomplete => "incomplete",
    }
}

fn digest(hasher: blake3::Hasher) -> CoreResult<ContentId> {
    ContentId::from_blake3_hex(hasher.finalize().to_hex().as_str())
}

fn update_str(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn update_usize(hasher: &mut blake3::Hasher, value: usize) {
    hasher.update(&(value as u64).to_be_bytes());
}

fn update_external_ref(
    hasher: &mut blake3::Hasher,
    reference: &crate::types::ExternalRef,
) -> CoreResult<()> {
    reference.validate()?;
    update_str(hasher, reference.provider_namespace.as_str());
    update_str(hasher, reference.native_kind.as_str());
    update_str(hasher, reference.native_id.as_str());
    Ok(())
}

#[cfg(test)]
mod tests;

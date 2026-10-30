//! Four-version semantic merge check orchestration.
//!
//! Kiln binds an externally produced semantic checker result to one frozen quartet of exact
//! sources (base O, variants A and B, proposed candidate M), its property and its policy, and
//! interprets the outcome as a CI decision. Kiln never generates or mutates M and never
//! implements merge semantics itself. See `docs/semantic-merge-orchestration.md`.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::source::{SourceCapability, SourceNativeFacts, SourceProviderProfile, SourceResolution};
use crate::types::{
    Alias, AttemptId, CandidateId, ContentId, CoreResult, Diagnostic, EffectId, EvaluationId,
    ExternalRef, PipelineRevisionId, PolicyId, ProviderProfileId,
};
use crate::workflow::{
    WorkflowBounds, WorkflowControls, WorkflowEffectClass, WorkflowEffectPlan, WorkflowNode,
    validate_workflow_plan,
};

mod identity;
mod state;

pub use identity::{
    ArtifactKind, AttemptBinding, BindingComponent, RequestBinding, artifact_binding,
    attempt_binding, require_binding,
};
pub use state::{
    AdmittedResult, AttemptRecord, AttemptStatus, BoundArtifact, CacheEntry, CheckerObservation,
    CheckerRequest, CheckerVerdict, CombinedGateDecision, EvidenceKind, ExhaustionBound,
    IndependentGate, IndependentGateKind, ProducerFailure, RetryDecision, SemanticCheckCommand,
    SemanticCheckDecision, SemanticCheckEffect, SemanticCheckEvent, SemanticCheckState,
    SemanticGateVerdict, SemanticOutcome, admit_cache_entry, cache_entry, classify_retry,
    combine_gates, decide, decide_gate, evolve,
};

pub const SEMANTIC_MERGE_CONTRACT_VERSION: u16 = 1;
/// Maximum required coverage units in one semantic subject.
pub const MAX_COVERAGE_UNITS: usize = 256;
/// Maximum assumptions attached to one semantic property.
pub const MAX_ASSUMPTIONS: usize = 64;
/// Maximum entries in each policy admission list.
pub const MAX_POLICY_ADMISSIONS: usize = 16;
/// Absolute ceiling on attempts for one request, including retries.
pub const MAX_ATTEMPTS_CEILING: u32 = 8;
/// Absolute ceiling on extra candidate changes beyond A ∪ B under containment.
pub const MAX_CANDIDATE_EXTRA_CHANGES_CEILING: usize = 64;
/// Absolute ceiling on bound products per attempt.
pub const MAX_PRODUCTS_CEILING: usize = 64;
/// Absolute ceiling on independently requested gates combined with the semantic gate.
pub const MAX_INDEPENDENT_GATES: usize = 64;

/// Claims that a semantic merge CI result never makes.
// r[impl kiln.semantic_merge.independence]
pub const SEMANTIC_MERGE_NON_CLAIMS: [&str; 6] = [
    "semantic merge CI success proves only the checked property for the exact role-tagged quartet, assumptions, dependencies, model, checker and policy",
    "semantic merge CI success does not prove source trust, signatures or complete history",
    "semantic merge CI success grants no merge, publish, status or source mutation authority",
    "semantic merge CI success does not grant blanket release eligibility",
    "an unknown, unsupported, exhausted, cancelled, stale or tool-error outcome is neither success nor a demonstrated conflict",
    "a clean text merge, equal source trees, passing tests or an accepted evidence envelope is not a semantic merge result",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeRole {
    Base,
    VariantA,
    VariantB,
    Candidate,
}

impl MergeRole {
    pub const ALL: [Self; 4] = [Self::Base, Self::VariantA, Self::VariantB, Self::Candidate];

    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Base => "base-o",
            Self::VariantA => "variant-a",
            Self::VariantB => "variant-b",
            Self::Candidate => "candidate-m",
        }
    }
}

/// One exact, acquired source in its declared role.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RoleSource {
    pub role: MergeRole,
    pub resolution: SourceResolution,
    pub acquisition_profile: SourceProviderProfile,
}

/// Who proposed candidate M and which exact state they proposed. Kiln never generates M.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CandidateSupply {
    pub supplier: ProviderProfileId,
    pub proposal: ExternalRef,
    pub supplied_state: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticSubject {
    pub language: Alias,
    pub unit: ExternalRef,
    pub required_coverage: Vec<Alias>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticProperty {
    pub id: Alias,
    pub revision: ContentId,
    pub assumptions: Vec<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticInputs {
    pub dependencies: ContentId,
    pub model: ContentId,
}

/// Exact producer, checker and toolchain identities for one admitted checker.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CheckerIdentity {
    pub producer_profile: ProviderProfileId,
    pub checker: ContentId,
    pub toolchain: ContentId,
    pub semantic_profile: Alias,
    pub receipt_profile: Alias,
}

/// Resource bounds; the shell maps them onto Bounded Exec limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckBounds {
    pub timeout_ms: u64,
    pub max_memory_bytes: u64,
    pub stdout_max_bytes: u64,
    pub stderr_max_bytes: u64,
    pub max_products: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckMode {
    Advisory,
    Required,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineagePolicy {
    /// The operator declares O, A and B; Kiln checks no ancestry.
    DeclaredRoles,
    /// Pijul change-set containment: O ⊆ A, O ⊆ B, A ∪ B ⊆ M.
    PijulChangeContainment {
        require_greatest_common_base: bool,
        max_candidate_extra_changes: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub retry_tool_error: bool,
    pub retry_exhausted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticMergePolicy {
    pub id: PolicyId,
    pub mode: CheckMode,
    pub lineage: LineagePolicy,
    pub admitted_checkers: Vec<CheckerIdentity>,
    pub admitted_source_profiles: Vec<ProviderProfileId>,
    pub admitted_candidate_suppliers: Vec<ProviderProfileId>,
    pub retry: RetryPolicy,
    pub bound_ceilings: CheckBounds,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticMergeRequest {
    pub contract_version: u16,
    pub evaluation: EvaluationId,
    pub base: RoleSource,
    pub variant_a: RoleSource,
    pub variant_b: RoleSource,
    pub candidate: RoleSource,
    pub candidate_supply: CandidateSupply,
    pub subject: SemanticSubject,
    pub property: SemanticProperty,
    pub inputs: SemanticInputs,
    pub checker: CheckerIdentity,
    pub policy: SemanticMergePolicy,
    pub bounds: CheckBounds,
}

impl SemanticMergeRequest {
    #[must_use]
    pub const fn role(&self, role: MergeRole) -> &RoleSource {
        match role {
            MergeRole::Base => &self.base,
            MergeRole::VariantA => &self.variant_a,
            MergeRole::VariantB => &self.variant_b,
            MergeRole::Candidate => &self.candidate,
        }
    }
}

/// An admitted, immutable request and its binding. Only `admit_request` constructs it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdmittedRequest {
    request: SemanticMergeRequest,
    binding: RequestBinding,
}

impl AdmittedRequest {
    #[must_use]
    pub const fn request(&self) -> &SemanticMergeRequest {
        &self.request
    }

    #[must_use]
    pub const fn binding(&self) -> &RequestBinding {
        &self.binding
    }

    /// The cache key is the full request identity; see `admit_cache_entry`.
    #[must_use]
    pub const fn cache_key(&self) -> &ContentId {
        &self.binding.identity
    }
}

fn error(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

fn role_field(role: MergeRole, suffix: &str) -> alloc::string::String {
    format!(
        "semantic_merge.{}.{suffix}",
        BindingComponent::for_role(role).name()
    )
}

/// Admits one immutable four-role request before any effect is requested.
// r[impl kiln.semantic_merge.request]
pub fn admit_request(request: SemanticMergeRequest) -> CoreResult<AdmittedRequest> {
    if request.contract_version != SEMANTIC_MERGE_CONTRACT_VERSION {
        return Err(error(
            "semantic_contract_version",
            "semantic_merge.contract_version",
            "semantic merge request contract version is unsupported",
        ));
    }
    admit_policy(&request.policy)?;
    for role in MergeRole::ALL {
        admit_role(&request, role)?;
    }
    admit_candidate_supply(&request)?;
    admit_subject_and_property(&request)?;
    if !request.policy.admitted_checkers.contains(&request.checker) {
        return Err(error(
            "semantic_checker_unadmitted",
            "semantic_merge.checker",
            "checker, toolchain or profile is not admitted by the semantic merge policy",
        ));
    }
    admit_bounds(&request.bounds, &request.policy.bound_ceilings)?;
    admit_lineage(&request)?;
    let binding = identity::compute_binding(&request)?;
    Ok(AdmittedRequest { request, binding })
}

/// Binding of a fresh read-only re-observation, used to judge freshness.
///
/// It validates each role resolution but not policy or lineage, so a moved input still yields
/// a comparable binding that names the changed component.
// r[impl kiln.semantic_merge.freshness]
pub fn reobserve_binding(request: &SemanticMergeRequest) -> CoreResult<RequestBinding> {
    for role in MergeRole::ALL {
        crate::source::validate_resolution(&request.role(role).resolution)?;
    }
    identity::compute_binding(request)
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| match pair {
        [left, right] => left < right,
        _ => true,
    })
}

fn admit_policy(policy: &SemanticMergePolicy) -> CoreResult<()> {
    for (field, length, sorted) in [
        (
            "semantic_merge.policy.admitted_checkers",
            policy.admitted_checkers.len(),
            strictly_sorted(&policy.admitted_checkers),
        ),
        (
            "semantic_merge.policy.admitted_source_profiles",
            policy.admitted_source_profiles.len(),
            strictly_sorted(&policy.admitted_source_profiles),
        ),
        (
            "semantic_merge.policy.admitted_candidate_suppliers",
            policy.admitted_candidate_suppliers.len(),
            strictly_sorted(&policy.admitted_candidate_suppliers),
        ),
    ] {
        if length == 0 || length > MAX_POLICY_ADMISSIONS || !sorted {
            return Err(error(
                "semantic_policy_admissions",
                field,
                "policy admission list must be non-empty, bounded, sorted and unique",
            ));
        }
    }
    if policy.retry.max_attempts == 0 || policy.retry.max_attempts > MAX_ATTEMPTS_CEILING {
        return Err(error(
            "semantic_policy_retry",
            "semantic_merge.policy.retry.max_attempts",
            "policy attempt bound must be positive and within the named ceiling",
        ));
    }
    let ceilings = &policy.bound_ceilings;
    if ceilings.timeout_ms == 0
        || ceilings.max_memory_bytes == 0
        || ceilings.stdout_max_bytes == 0
        || ceilings.stderr_max_bytes == 0
        || ceilings.max_products == 0
        || ceilings.max_products > MAX_PRODUCTS_CEILING
    {
        return Err(error(
            "semantic_policy_bounds",
            "semantic_merge.policy.bound_ceilings",
            "policy bound ceilings must be positive and within the named ceilings",
        ));
    }
    if let LineagePolicy::PijulChangeContainment {
        max_candidate_extra_changes,
        ..
    } = policy.lineage
        && max_candidate_extra_changes > MAX_CANDIDATE_EXTRA_CHANGES_CEILING
    {
        return Err(error(
            "semantic_policy_lineage",
            "semantic_merge.policy.lineage",
            "extra candidate change bound exceeds its named ceiling",
        ));
    }
    Ok(())
}

const READ_ONLY_CAPABILITIES: [SourceCapability; 4] = [
    SourceCapability::Discover,
    SourceCapability::ResolveExact,
    SourceCapability::Acquire,
    SourceCapability::ObserveIdentity,
];
const REQUIRED_ACQUISITION_CAPABILITIES: [SourceCapability; 3] = [
    SourceCapability::ResolveExact,
    SourceCapability::Acquire,
    SourceCapability::ObserveIdentity,
];

// r[impl kiln.semantic_merge.request]
// r[impl kiln.semantic_merge.candidate]
fn admit_role(request: &SemanticMergeRequest, slot: MergeRole) -> CoreResult<()> {
    let source = request.role(slot);
    if source.role != slot {
        return Err(error(
            "semantic_role_slot",
            &role_field(slot, "role"),
            "source role tag differs from its request slot",
        ));
    }
    crate::source::validate_resolution(&source.resolution)?;
    if source.resolution.source_provider != request.base.resolution.source_provider {
        return Err(error(
            "semantic_role_provider",
            &role_field(slot, "source_provider"),
            "all four roles must be acquired through the same source provider profile",
        ));
    }
    let profile = &source.acquisition_profile;
    if profile.id != source.resolution.source_provider
        || request
            .policy
            .admitted_source_profiles
            .binary_search(&profile.id)
            .is_err()
    {
        return Err(error(
            "semantic_acquisition_profile",
            &role_field(slot, "acquisition_profile"),
            "source was not acquired through a policy-admitted source profile",
        ));
    }
    if profile
        .capabilities
        .iter()
        .any(|capability| !READ_ONLY_CAPABILITIES.contains(capability))
    {
        return Err(error(
            "semantic_acquisition_authority",
            &role_field(slot, "acquisition_profile.capabilities"),
            "acquisition profile grants authority beyond bounded exact reads",
        ));
    }
    if !profile.immutable_resolution
        || REQUIRED_ACQUISITION_CAPABILITIES
            .iter()
            .any(|capability| !profile.capabilities.contains(capability))
    {
        return Err(error(
            "semantic_acquisition_capability",
            &role_field(slot, "acquisition_profile.capabilities"),
            "acquisition profile cannot freeze an exact immutable source",
        ));
    }
    profile.native_system.validate()?;
    Ok(())
}

// r[impl kiln.semantic_merge.candidate]
fn admit_candidate_supply(request: &SemanticMergeRequest) -> CoreResult<()> {
    let supply = &request.candidate_supply;
    supply.proposal.validate()?;
    supply.supplied_state.validate()?;
    if request
        .policy
        .admitted_candidate_suppliers
        .binary_search(&supply.supplier)
        .is_err()
    {
        return Err(error(
            "semantic_candidate_supplier",
            "semantic_merge.candidate_supply.supplier",
            "candidate supplier is not authorized by the semantic merge policy",
        ));
    }
    if supply.supplied_state != request.candidate.resolution.exact_revision {
        return Err(error(
            "semantic_candidate_supplied_state",
            "semantic_merge.candidate_supply.supplied_state",
            "supplied candidate state differs from the frozen candidate acquisition",
        ));
    }
    Ok(())
}

fn admit_subject_and_property(request: &SemanticMergeRequest) -> CoreResult<()> {
    request.subject.unit.validate()?;
    let coverage = &request.subject.required_coverage;
    if coverage.is_empty() || coverage.len() > MAX_COVERAGE_UNITS || !strictly_sorted(coverage) {
        return Err(error(
            "semantic_subject_coverage",
            "semantic_merge.subject.required_coverage",
            "required coverage must be non-empty, bounded, sorted and unique",
        ));
    }
    let assumptions = &request.property.assumptions;
    if assumptions.len() > MAX_ASSUMPTIONS || !strictly_sorted(assumptions) {
        return Err(error(
            "semantic_property_assumptions",
            "semantic_merge.property.assumptions",
            "property assumptions must be bounded, sorted and unique",
        ));
    }
    Ok(())
}

fn admit_bounds(bounds: &CheckBounds, ceilings: &CheckBounds) -> CoreResult<()> {
    for (field, value, ceiling) in [
        ("timeout_ms", bounds.timeout_ms, ceilings.timeout_ms),
        (
            "max_memory_bytes",
            bounds.max_memory_bytes,
            ceilings.max_memory_bytes,
        ),
        (
            "stdout_max_bytes",
            bounds.stdout_max_bytes,
            ceilings.stdout_max_bytes,
        ),
        (
            "stderr_max_bytes",
            bounds.stderr_max_bytes,
            ceilings.stderr_max_bytes,
        ),
        (
            "max_products",
            u64::try_from(bounds.max_products).unwrap_or(u64::MAX),
            u64::try_from(ceilings.max_products).unwrap_or(u64::MAX),
        ),
    ] {
        if value == 0 || value > ceiling {
            return Err(error(
                "semantic_bounds",
                &format!("semantic_merge.bounds.{field}"),
                "check bound must be positive and within the policy ceiling",
            ));
        }
    }
    Ok(())
}

fn pijul_changes(request: &SemanticMergeRequest, role: MergeRole) -> CoreResult<BTreeSet<&str>> {
    match &request.role(role).resolution.native_facts {
        SourceNativeFacts::Pijul(facts) => Ok(facts
            .change_sequence
            .iter()
            .map(|change| change.native_id.as_str())
            .collect()),
        SourceNativeFacts::Git(_) | SourceNativeFacts::Jujutsu(_) => Err(error(
            "semantic_lineage_unavailable",
            &role_field(role, "native_facts"),
            "lineage policy needs Pijul channel-state facts that this role does not carry",
        )),
    }
}

/// Checks explicit lineage. Kiln never infers or substitutes a base.
// r[impl kiln.semantic_merge.lineage]
fn admit_lineage(request: &SemanticMergeRequest) -> CoreResult<()> {
    let LineagePolicy::PijulChangeContainment {
        require_greatest_common_base,
        max_candidate_extra_changes,
    } = request.policy.lineage
    else {
        return Ok(());
    };
    let base = pijul_changes(request, MergeRole::Base)?;
    let variant_a = pijul_changes(request, MergeRole::VariantA)?;
    let variant_b = pijul_changes(request, MergeRole::VariantB)?;
    let candidate = pijul_changes(request, MergeRole::Candidate)?;
    for (role, variant) in [
        (MergeRole::VariantA, &variant_a),
        (MergeRole::VariantB, &variant_b),
    ] {
        if !base.is_subset(variant) {
            return Err(error(
                "semantic_lineage_base",
                &role_field(role, "native_facts.change_sequence"),
                "base change set is not contained in this variant",
            ));
        }
    }
    if require_greatest_common_base && variant_a.intersection(&variant_b).count() != base.len() {
        return Err(error(
            "semantic_lineage_ambiguous",
            "semantic_merge.base.native_facts.change_sequence",
            "variants share changes outside the declared base, so the base is ambiguous",
        ));
    }
    let union: BTreeSet<&str> = variant_a.union(&variant_b).copied().collect();
    if !union.is_subset(&candidate) {
        return Err(error(
            "semantic_lineage_candidate",
            "semantic_merge.candidate.native_facts.change_sequence",
            "candidate does not contain every change of both variants",
        ));
    }
    if candidate.difference(&union).count() > max_candidate_extra_changes {
        return Err(error(
            "semantic_lineage_candidate_extra",
            "semantic_merge.candidate.native_facts.change_sequence",
            "candidate carries more extra changes than the policy admits",
        ));
    }
    Ok(())
}

/// Exact state and tree that a downstream operation must observe again.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextExpectation {
    pub role: MergeRole,
    pub state: ExternalRef,
    pub tree: ContentId,
}

/// Precondition for any downstream merge or publish shell. It is not an authorization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RevalidationObligation {
    pub request: ContentId,
    pub expectations: Vec<ContextExpectation>,
}

impl RevalidationObligation {
    #[must_use]
    pub fn for_request(request: &AdmittedRequest) -> Self {
        Self {
            request: request.binding.identity.clone(),
            expectations: MergeRole::ALL
                .into_iter()
                .map(|role| {
                    let resolution = &request.request.role(role).resolution;
                    ContextExpectation {
                        role,
                        state: resolution.exact_revision.clone(),
                        tree: resolution.canonical_tree.clone(),
                    }
                })
                .collect(),
        }
    }

    /// Semantic merge orchestration never grants mutation authority.
    // r[impl kiln.semantic_merge.independence]
    #[must_use]
    pub const fn grants_mutation_authority(&self) -> bool {
        false
    }
}

/// Pure precondition a downstream shell evaluates inside its own race-safe protocol.
// r[impl kiln.semantic_merge.independence]
pub fn check_revalidation(
    obligation: &RevalidationObligation,
    observed: &[ContextExpectation],
) -> CoreResult<()> {
    if observed.len() != obligation.expectations.len() {
        return Err(error(
            "semantic_revalidation_context",
            "semantic_merge.revalidation",
            "downstream observation must cover exactly the candidate and its expected context",
        ));
    }
    for expected in &obligation.expectations {
        let mut matching = observed.iter().filter(|item| item.role == expected.role);
        let (Some(current), None) = (matching.next(), matching.next()) else {
            return Err(error(
                "semantic_revalidation_context",
                &role_field(expected.role, "revalidation"),
                "downstream observation must name each role exactly once",
            ));
        };
        if current != expected {
            return Err(error(
                "semantic_revalidation_moved",
                &role_field(expected.role, "revalidation"),
                "candidate or expected context moved after the semantic check",
            ));
        }
    }
    Ok(())
}

/// Identities a shell supplies to dispatch one checker attempt through Lattice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckerDispatchIdentity {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub candidate_id: CandidateId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub provider_profile: ProviderProfileId,
    pub platform: Alias,
    pub bounds: WorkflowBounds,
}

const CHECK_NODE_ID: &str = "semantic-check";
const CHECK_NODE_KIND: &str = "semantic-merge-check";

/// Maps one checker attempt onto the existing versioned Lattice workflow plan.
// r[impl kiln.semantic_merge.dispatch]
pub fn checker_workflow_plan(
    checker_request: &CheckerRequest,
    dispatch: &CheckerDispatchIdentity,
) -> CoreResult<WorkflowEffectPlan> {
    let request = &checker_request.request;
    if checker_request.attempt.request != request.binding.identity {
        return Err(error(
            "semantic_attempt_binding",
            "semantic_merge.attempt.request",
            "attempt is bound to a different semantic merge request",
        ));
    }
    let plan = WorkflowEffectPlan {
        effect_id: dispatch.effect_id.clone(),
        attempt_id: dispatch.attempt_id.clone(),
        candidate_id: dispatch.candidate_id.clone(),
        pipeline_revision_id: dispatch.pipeline_revision_id.clone(),
        workflow_policy_id: request.request.policy.id.clone(),
        provider_profile: dispatch.provider_profile.clone(),
        idempotency_key: checker_request.attempt.identity.clone(),
        platform: dispatch.platform.clone(),
        source: None,
        nodes: alloc::vec![WorkflowNode {
            id: Alias::new(CHECK_NODE_ID)?,
            kind: Alias::new(CHECK_NODE_KIND)?,
            effect_class: WorkflowEffectClass::Test,
            adapter_kind: request.request.checker.semantic_profile.clone(),
            operation_identity: request.binding.identity.clone(),
        }],
        edges: Vec::new(),
        controls: WorkflowControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        bounds: dispatch.bounds,
    };
    validate_workflow_plan(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests;

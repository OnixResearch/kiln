//! Content-bound requirement references and selected-stack trace contexts.
//!
//! The core consumes already-loaded Cairn registry values. It performs no file,
//! environment, process, clock, network, or terminal operations.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::VerificationRole;
use crate::provenance::ProvenanceLink;
use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;

pub const CAIRN_CONTENT_BOUND_REGISTRY_CONTRACT_REVISION: &str = "d953fe11ab620f3a42bdad1db51bc7672dd29824";
pub const CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1: &str = "cairn.content-bound-requirement-registry.v1";
pub const CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1: &str =
    "cairn.content-bound-requirement-registry-receipt.v1";
pub const CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY: &str = "content-bound requirement registry validation proves canonical identity, current-root freshness, and declared linkage only; it does not prove revision authenticity, requirement satisfaction, source correctness, implementation correctness, or release eligibility";
pub const REQUIREMENT_REF_SCHEMA_V1: &str = "valence.requirement-ref.v1";
pub const REQUIREMENT_LINKAGE_INPUT_SCHEMA_V1: &str = "valence.requirement-linkage-input.v1";
pub const STACK_REVISION_SET_SCHEMA_V1: &str = "valence.stack-revision-set.v1";
pub const STACK_TRACE_CONTEXT_SCHEMA_V1: &str = "valence.stack-trace-context.v1";
pub const STACK_TRACE_VALIDATION_ENVELOPE_SCHEMA_V1: &str = "valence.stack-trace-validation-envelope.v1";
pub const CONTENT_BOUND_REQUIREMENT_BOUNDARY: &str = "content-bound requirement validation proves typed identity, supplied-registry membership, and selected-stack linkage only; it does not prove registry freshness beyond the supplied artifact, revision authenticity, requirement satisfaction, evidence truth, runtime authority, lifecycle readiness, or release eligibility";
pub const LEGACY_REQUIREMENT_BOUNDARY: &str = "legacy requirement IDs remain recorded-only strings; validation does not invent repository, revision, specification, policy, registry, requirement-row, satisfaction, or release facts";

pub const CONTENT_BOUND_REQUIREMENT_NON_CLAIMS: &[&str] = &[
    "identity and linkage evidence only",
    "supplied registry freshness was not independently established",
    "declared revision authenticity was not established",
    "not requirement satisfaction",
    "not evidence truth",
    "not runtime authority",
    "not lifecycle readiness",
    "not release eligibility",
];
pub const STACK_TRACE_CONTEXT_NON_CLAIMS: &[&str] = &[
    "selected stack identity and linkage only",
    "supplied registry freshness was not independently established",
    "declared revision authenticity was not established",
    "not requirement satisfaction",
    "not evidence truth",
    "not runtime authority",
    "not lifecycle readiness",
    "not release eligibility",
];

const CAIRN_REGISTRY_NON_CLAIMS: &[&str] = &[
    "declared revision authenticity was not established",
    "not requirement satisfaction",
    "not source correctness",
    "not implementation correctness",
    "not release eligibility",
];
const BLAKE3_HEX_LENGTH: usize = 64;
const GIT_SHA1_HEX_LENGTH: usize = 40;
const GIT_SHA256_HEX_LENGTH: usize = 64;
const JUJUTSU_REVISION_MIN_HEX_LENGTH: usize = 16;
const JUJUTSU_REVISION_MAX_HEX_LENGTH: usize = 64;
const MAX_REPOSITORY_ID_BYTES: usize = 256;
const MAX_OPAQUE_REVISION_BYTES: usize = 256;
const MAX_REQUIREMENT_ID_BYTES: usize = 512;
const DOMAIN_SEPARATOR: u8 = 0;
const CAIRN_SPECIFICATION_SET_DOMAIN: &str = "cairn.requirement-registry.specification-set.v1";
const CAIRN_REQUIREMENT_ROW_DOMAIN: &str = "cairn.requirement-registry.row.v1";
const CAIRN_REGISTRY_DOMAIN: &str = "cairn.requirement-registry.registry.v1";
const CAIRN_RECEIPT_DOMAIN: &str = "cairn.requirement-registry.receipt.v1";
const REQUIREMENT_REF_DOMAIN: &str = "valence.requirement-ref.v1";
const STACK_REVISION_SET_DOMAIN: &str = "valence.stack-revision-set.v1";
const STACK_TRACE_CONTEXT_DOMAIN: &str = "valence.stack-trace-context.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementDigestDomainV1 {
    Specification,
    RequirementBody,
    Policy,
    RequirementRow,
    RequirementRegistry,
    RequirementRegistryReceipt,
    RequirementReference,
    StackRevisionSet,
    StackTraceContext,
    EvidenceGraph,
    Release,
    Lifecycle,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementDigestV1 {
    pub domain: RequirementDigestDomainV1,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackRevisionKindV1 {
    Git,
    Jujutsu,
    Opaque,
}

impl StackRevisionKindV1 {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Jujutsu => "jujutsu",
            Self::Opaque => "opaque",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackRevisionV1 {
    pub kind: StackRevisionKindV1,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnContentBoundRevisionV1 {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnContentBoundSpecificationIdentityV1 {
    pub path: String,
    pub content_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnContentBoundSourceSpanV1 {
    pub start_byte: u64,
    pub end_byte: u64,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnContentBoundRequirementRowV1 {
    pub requirement_id: String,
    pub specification_path: String,
    pub specification_blake3: String,
    pub source_span: CairnContentBoundSourceSpanV1,
    pub requirement_body_blake3: String,
    pub row_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnContentBoundRequirementRegistryReceiptV1 {
    pub schema_version: String,
    pub registry_blake3: String,
    pub repository_id: String,
    pub revision_kind: String,
    pub revision: String,
    pub specification_set_blake3: String,
    pub policy_blake3: String,
    pub row_count: usize,
    pub non_claims: Vec<String>,
    pub receipt_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnContentBoundRequirementRegistryV1 {
    pub schema_version: String,
    pub boundary: String,
    pub repository_id: String,
    pub revision: CairnContentBoundRevisionV1,
    pub specification_set_blake3: String,
    pub specifications: Vec<CairnContentBoundSpecificationIdentityV1>,
    pub policy_blake3: String,
    pub rows: Vec<CairnContentBoundRequirementRowV1>,
    pub non_claims: Vec<String>,
    pub registry_blake3: String,
    pub receipt: CairnContentBoundRequirementRegistryReceiptV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRefV1 {
    pub schema: String,
    pub repository_id: String,
    pub revision: StackRevisionV1,
    pub requirement_id: String,
    pub specification_path: String,
    pub specification_identity: RequirementDigestV1,
    pub requirement_body_identity: RequirementDigestV1,
    pub policy_identity: RequirementDigestV1,
    pub row_identity: RequirementDigestV1,
    pub registry_identity: RequirementDigestV1,
    pub registry_receipt_identity: RequirementDigestV1,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
    pub reference_identity: RequirementDigestV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementValidationIssueV1 {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRefValidationReportV1 {
    pub valid: bool,
    pub reference_identity: RequirementDigestV1,
    pub registry_identity: RequirementDigestV1,
    pub issue_count: usize,
    pub issues: Vec<RequirementValidationIssueV1>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementLinkageProfileV1 {
    LegacyRecordedOnly,
    StrictContentBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementLinkageInputV1 {
    pub schema: String,
    pub profile: RequirementLinkageProfileV1,
    #[serde(default)]
    pub legacy_requirement_ids: Vec<String>,
    #[serde(default)]
    pub requirement_refs: Vec<RequirementRefV1>,
    pub verification_role: VerificationRole,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementLinkageValidationReportV1 {
    pub valid: bool,
    pub strict_profile_satisfied: bool,
    pub verification_role: VerificationRole,
    pub legacy_requirement_count: usize,
    pub content_bound_reference_count: usize,
    pub issue_count: usize,
    pub issues: Vec<RequirementValidationIssueV1>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackRevisionEntryV1 {
    pub repository_id: String,
    pub revision: StackRevisionV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackRevisionSetV1 {
    pub schema: String,
    pub entries: Vec<StackRevisionEntryV1>,
    pub identity: RequirementDigestV1,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackTraceLinkRoleV1 {
    Release,
    Lifecycle,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackTraceContextV1 {
    pub schema: String,
    pub revision_set: StackRevisionSetV1,
    pub requirement_refs: Vec<RequirementRefV1>,
    pub evidence_graph_identity: RequirementDigestV1,
    pub release_identity: Option<RequirementDigestV1>,
    pub lifecycle_identity: Option<RequirementDigestV1>,
    pub runtime_identity: Option<RequirementDigestV1>,
    pub required_links: Vec<StackTraceLinkRoleV1>,
    pub non_claims: Vec<String>,
    pub context_identity: RequirementDigestV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackTraceContextDraftV1 {
    pub revision_set: StackRevisionSetV1,
    pub requirement_refs: Vec<RequirementRefV1>,
    pub evidence_graph_identity: RequirementDigestV1,
    pub release_identity: Option<RequirementDigestV1>,
    pub lifecycle_identity: Option<RequirementDigestV1>,
    pub runtime_identity: Option<RequirementDigestV1>,
    pub required_links: Vec<StackTraceLinkRoleV1>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackTraceExpectationsV1 {
    pub evidence_graph_identity: RequirementDigestV1,
    pub release_identity: Option<RequirementDigestV1>,
    pub lifecycle_identity: Option<RequirementDigestV1>,
    pub runtime_identity: Option<RequirementDigestV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackTraceValidationEnvelopeV1 {
    pub schema: String,
    pub context: StackTraceContextV1,
    pub registries: Vec<CairnContentBoundRequirementRegistryV1>,
    pub expectations: StackTraceExpectationsV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackTraceValidationReportV1 {
    pub valid: bool,
    pub context_identity: RequirementDigestV1,
    pub repository_count: usize,
    pub requirement_reference_count: usize,
    pub issue_count: usize,
    pub issues: Vec<RequirementValidationIssueV1>,
    pub boundary: String,
}

#[derive(Serialize)]
struct CairnRequirementRowHashMaterial<'a> {
    requirement_id: &'a str,
    specification_path: &'a str,
    specification_blake3: &'a str,
    source_span: &'a CairnContentBoundSourceSpanV1,
    requirement_body_blake3: &'a str,
}

#[derive(Serialize)]
struct CairnRegistryHashMaterial<'a> {
    schema_version: &'a str,
    boundary: &'a str,
    repository_id: &'a str,
    revision: &'a CairnContentBoundRevisionV1,
    specification_set_blake3: &'a str,
    specifications: &'a [CairnContentBoundSpecificationIdentityV1],
    policy_blake3: &'a str,
    rows: &'a [CairnContentBoundRequirementRowV1],
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct CairnReceiptHashMaterial<'a> {
    schema_version: &'a str,
    registry_blake3: &'a str,
    repository_id: &'a str,
    revision_kind: &'a str,
    revision: &'a str,
    specification_set_blake3: &'a str,
    policy_blake3: &'a str,
    row_count: usize,
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct RequirementRefHashMaterial<'a> {
    schema: &'a str,
    repository_id: &'a str,
    revision: &'a StackRevisionV1,
    requirement_id: &'a str,
    specification_path: &'a str,
    specification_identity: &'a RequirementDigestV1,
    requirement_body_identity: &'a RequirementDigestV1,
    policy_identity: &'a RequirementDigestV1,
    row_identity: &'a RequirementDigestV1,
    registry_identity: &'a RequirementDigestV1,
    registry_receipt_identity: &'a RequirementDigestV1,
    verification_role: &'a VerificationRole,
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct StackRevisionSetHashMaterial<'a> {
    schema: &'a str,
    entries: &'a [StackRevisionEntryV1],
}

#[derive(Serialize)]
struct StackTraceContextHashMaterial<'a> {
    schema: &'a str,
    revision_set: &'a StackRevisionSetV1,
    requirement_refs: &'a [RequirementRefV1],
    evidence_graph_identity: &'a RequirementDigestV1,
    release_identity: &'a Option<RequirementDigestV1>,
    lifecycle_identity: &'a Option<RequirementDigestV1>,
    runtime_identity: &'a Option<RequirementDigestV1>,
    required_links: &'a [StackTraceLinkRoleV1],
    non_claims: &'a [String],
}

#[must_use]
pub fn validate_cairn_content_bound_requirement_registry(
    registry: &CairnContentBoundRequirementRegistryV1,
) -> Vec<RequirementValidationIssueV1> {
    let mut issues = Vec::new();
    if registry.schema_version != CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "registry_schema_unsupported",
            "registry.schema_version",
            "supplied Cairn registry schema is unsupported",
        );
    }
    if registry.boundary != CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY {
        push_issue(
            &mut issues,
            "registry_boundary_weakened",
            "registry.boundary",
            "supplied Cairn registry boundary is missing or changed",
        );
    }
    validate_repository_id(&registry.repository_id, "registry.repository_id", &mut issues);
    validate_cairn_revision(&registry.revision, "registry.revision", &mut issues);
    validate_required_non_claims(&registry.non_claims, CAIRN_REGISTRY_NON_CLAIMS, "registry.non_claims", &mut issues);
    validate_hash(&registry.specification_set_blake3, "registry.specification_set_blake3", &mut issues);
    validate_hash(&registry.policy_blake3, "registry.policy_blake3", &mut issues);
    validate_hash(&registry.registry_blake3, "registry.registry_blake3", &mut issues);
    validate_cairn_specifications(registry, &mut issues);
    validate_cairn_rows(registry, &mut issues);
    validate_cairn_registry_hash(registry, &mut issues);
    validate_cairn_receipt(registry, &mut issues);
    sort_issues(&mut issues);
    issues
}

pub fn build_requirement_ref(
    registry: &CairnContentBoundRequirementRegistryV1,
    requirement_id: &str,
) -> Result<RequirementRefV1, Vec<RequirementValidationIssueV1>> {
    let mut issues = validate_cairn_content_bound_requirement_registry(registry);
    let matches = registry.rows.iter().filter(|row| row.requirement_id == requirement_id).collect::<Vec<_>>();
    if matches.is_empty() {
        push_issue(
            &mut issues,
            "requirement_missing",
            "requirement_id",
            "requirement is absent from the supplied registry",
        );
    }
    if matches.len() > 1 {
        push_issue(
            &mut issues,
            "requirement_duplicate",
            "requirement_id",
            "requirement occurs more than once in the supplied registry",
        );
    }
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let row = matches[0];
    let revision = stack_revision_from_cairn(&registry.revision).ok_or_else(|| {
        vec![issue(
            "revision_kind_unsupported",
            "registry.revision.kind",
            "registry revision kind is unsupported",
        )]
    })?;
    let mut reference = RequirementRefV1 {
        schema: REQUIREMENT_REF_SCHEMA_V1.to_owned(),
        repository_id: registry.repository_id.clone(),
        revision,
        requirement_id: row.requirement_id.clone(),
        specification_path: row.specification_path.clone(),
        specification_identity: digest(RequirementDigestDomainV1::Specification, &row.specification_blake3),
        requirement_body_identity: digest(RequirementDigestDomainV1::RequirementBody, &row.requirement_body_blake3),
        policy_identity: digest(RequirementDigestDomainV1::Policy, &registry.policy_blake3),
        row_identity: digest(RequirementDigestDomainV1::RequirementRow, &row.row_blake3),
        registry_identity: digest(RequirementDigestDomainV1::RequirementRegistry, &registry.registry_blake3),
        registry_receipt_identity: digest(
            RequirementDigestDomainV1::RequirementRegistryReceipt,
            &registry.receipt.receipt_blake3,
        ),
        verification_role: VerificationRole::RecordedOnly,
        non_claims: required_non_claims(CONTENT_BOUND_REQUIREMENT_NON_CLAIMS),
        reference_identity: empty_digest(RequirementDigestDomainV1::RequirementReference),
    };
    reference.reference_identity.blake3 = requirement_ref_hash(&reference).map_err(|issue| vec![issue])?;
    Ok(reference)
}

#[must_use]
pub fn validate_requirement_ref(
    reference: &RequirementRefV1,
    registry: &CairnContentBoundRequirementRegistryV1,
) -> RequirementRefValidationReportV1 {
    // r[impl valence.provenance.content_bound_requirement_ref]
    // r[impl valence.provenance.requirement_registry_validation]
    let mut issues = validate_cairn_content_bound_requirement_registry(registry);
    validate_requirement_ref_shape(reference, &mut issues);
    compare_requirement_ref_to_registry(reference, registry, &mut issues);
    sort_issues(&mut issues);
    RequirementRefValidationReportV1 {
        valid: issues.is_empty(),
        reference_identity: reference.reference_identity.clone(),
        registry_identity: reference.registry_identity.clone(),
        issue_count: issues.len(),
        issues,
        boundary: CONTENT_BOUND_REQUIREMENT_BOUNDARY.to_owned(),
    }
}

#[must_use]
pub fn legacy_requirement_linkage_from_provenance(link: &ProvenanceLink) -> RequirementLinkageInputV1 {
    RequirementLinkageInputV1 {
        schema: REQUIREMENT_LINKAGE_INPUT_SCHEMA_V1.to_owned(),
        profile: RequirementLinkageProfileV1::LegacyRecordedOnly,
        legacy_requirement_ids: link.requirement_ids.clone(),
        requirement_refs: Vec::new(),
        verification_role: VerificationRole::RecordedOnly,
        non_claims: vec![LEGACY_REQUIREMENT_BOUNDARY.to_owned()],
    }
}

#[must_use]
pub fn validate_requirement_linkage(
    input: &RequirementLinkageInputV1,
    registries: &[CairnContentBoundRequirementRegistryV1],
) -> RequirementLinkageValidationReportV1 {
    // r[impl valence.provenance.legacy_requirement_id_boundary]
    let mut issues = Vec::new();
    if input.schema != REQUIREMENT_LINKAGE_INPUT_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "linkage_schema_unsupported",
            "schema",
            "requirement linkage input schema is unsupported",
        );
    }
    if input.verification_role != VerificationRole::RecordedOnly {
        push_issue(
            &mut issues,
            "verification_role_promotion_denied",
            "verification_role",
            "requirement linkage is recorded-only identity evidence",
        );
    }
    let registry_index = registry_index(registries, &mut issues);
    let strict_profile_satisfied = match input.profile {
        RequirementLinkageProfileV1::LegacyRecordedOnly => {
            validate_legacy_linkage(input, &mut issues);
            false
        }
        RequirementLinkageProfileV1::StrictContentBound => {
            validate_strict_linkage(input, &registry_index, &mut issues);
            !input.requirement_refs.is_empty()
        }
    };
    sort_issues(&mut issues);
    RequirementLinkageValidationReportV1 {
        valid: issues.is_empty(),
        strict_profile_satisfied: strict_profile_satisfied && issues.is_empty(),
        verification_role: VerificationRole::RecordedOnly,
        legacy_requirement_count: input.legacy_requirement_ids.len(),
        content_bound_reference_count: input.requirement_refs.len(),
        issue_count: issues.len(),
        issues,
        boundary: match input.profile {
            RequirementLinkageProfileV1::LegacyRecordedOnly => LEGACY_REQUIREMENT_BOUNDARY.to_owned(),
            RequirementLinkageProfileV1::StrictContentBound => CONTENT_BOUND_REQUIREMENT_BOUNDARY.to_owned(),
        },
    }
}

pub fn build_stack_revision_set(
    mut entries: Vec<StackRevisionEntryV1>,
) -> Result<StackRevisionSetV1, Vec<RequirementValidationIssueV1>> {
    entries.sort();
    let mut issues = Vec::new();
    validate_stack_revision_entries(&entries, &mut issues);
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let identity = domain_hash_json(
        STACK_REVISION_SET_DOMAIN,
        &StackRevisionSetHashMaterial {
            schema: STACK_REVISION_SET_SCHEMA_V1,
            entries: &entries,
        },
        "revision_set.identity",
    )
    .map_err(|issue| vec![issue])?;
    Ok(StackRevisionSetV1 {
        schema: STACK_REVISION_SET_SCHEMA_V1.to_owned(),
        entries,
        identity: digest(RequirementDigestDomainV1::StackRevisionSet, &identity),
    })
}

#[must_use]
pub fn validate_stack_revision_set(revision_set: &StackRevisionSetV1) -> Vec<RequirementValidationIssueV1> {
    let mut issues = Vec::new();
    if revision_set.schema != STACK_REVISION_SET_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "revision_set_schema_unsupported",
            "revision_set.schema",
            "stack revision set schema is unsupported",
        );
    }
    validate_digest(
        &revision_set.identity,
        RequirementDigestDomainV1::StackRevisionSet,
        "revision_set.identity",
        &mut issues,
    );
    let mut canonical_entries = revision_set.entries.clone();
    canonical_entries.sort();
    validate_stack_revision_entries(&canonical_entries, &mut issues);
    if domain_hash_json(
        STACK_REVISION_SET_DOMAIN,
        &StackRevisionSetHashMaterial {
            schema: STACK_REVISION_SET_SCHEMA_V1,
            entries: &canonical_entries,
        },
        "revision_set.identity",
    )
    .is_ok_and(|expected| revision_set.identity.blake3 != expected)
    {
        push_issue(
            &mut issues,
            "revision_set_identity_stale",
            "revision_set.identity.blake3",
            "stack revision set identity does not match canonical entries",
        );
    }
    sort_issues(&mut issues);
    issues
}

pub fn build_stack_trace_context(
    mut draft: StackTraceContextDraftV1,
) -> Result<StackTraceContextV1, Vec<RequirementValidationIssueV1>> {
    let mut issues = validate_stack_revision_set(&draft.revision_set);
    draft.requirement_refs.sort_by(requirement_ref_order);
    draft.required_links.sort();
    draft.required_links.dedup();
    validate_required_non_claims(&draft.non_claims, STACK_TRACE_CONTEXT_NON_CLAIMS, "context.non_claims", &mut issues);
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let mut context = StackTraceContextV1 {
        schema: STACK_TRACE_CONTEXT_SCHEMA_V1.to_owned(),
        revision_set: draft.revision_set,
        requirement_refs: draft.requirement_refs,
        evidence_graph_identity: draft.evidence_graph_identity,
        release_identity: draft.release_identity,
        lifecycle_identity: draft.lifecycle_identity,
        runtime_identity: draft.runtime_identity,
        required_links: draft.required_links,
        non_claims: draft.non_claims,
        context_identity: empty_digest(RequirementDigestDomainV1::StackTraceContext),
    };
    context.context_identity.blake3 = stack_trace_context_hash(&context).map_err(|issue| vec![issue])?;
    Ok(context)
}

#[must_use]
pub fn validate_stack_trace_context(envelope: &StackTraceValidationEnvelopeV1) -> StackTraceValidationReportV1 {
    // r[impl valence.provenance.stack_trace_context]
    let context = &envelope.context;
    let mut issues = Vec::new();
    if envelope.schema != STACK_TRACE_VALIDATION_ENVELOPE_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "context_envelope_schema_unsupported",
            "schema",
            "stack trace validation envelope schema is unsupported",
        );
    }
    if context.schema != STACK_TRACE_CONTEXT_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "context_schema_unsupported",
            "context.schema",
            "stack trace context schema is unsupported",
        );
    }
    issues.extend(validate_stack_revision_set(&context.revision_set));
    validate_required_non_claims(
        &context.non_claims,
        STACK_TRACE_CONTEXT_NON_CLAIMS,
        "context.non_claims",
        &mut issues,
    );
    validate_digest(
        &context.context_identity,
        RequirementDigestDomainV1::StackTraceContext,
        "context.context_identity",
        &mut issues,
    );
    validate_context_links(context, &envelope.expectations, &mut issues);
    let registry_index = registry_index(&envelope.registries, &mut issues);
    validate_context_requirement_refs(context, &registry_index, &mut issues);
    if stack_trace_context_hash(context).is_ok_and(|expected| context.context_identity.blake3 != expected) {
        push_issue(
            &mut issues,
            "context_identity_stale",
            "context.context_identity.blake3",
            "stack trace context identity does not match canonical context fields",
        );
    }
    sort_issues(&mut issues);
    StackTraceValidationReportV1 {
        valid: issues.is_empty(),
        context_identity: context.context_identity.clone(),
        repository_count: context.revision_set.entries.len(),
        requirement_reference_count: context.requirement_refs.len(),
        issue_count: issues.len(),
        issues,
        boundary: CONTENT_BOUND_REQUIREMENT_BOUNDARY.to_owned(),
    }
}

fn validate_cairn_specifications(
    registry: &CairnContentBoundRequirementRegistryV1,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    let mut canonical = registry.specifications.clone();
    canonical.sort_by(|left, right| left.path.cmp(&right.path));
    if canonical != registry.specifications {
        push_issue(
            issues,
            "registry_order_invalid",
            "registry.specifications",
            "registry specifications are not in canonical path order",
        );
    }
    let mut seen = BTreeSet::new();
    for (index, specification) in registry.specifications.iter().enumerate() {
        validate_specification_path(&specification.path, &format!("registry.specifications[{index}].path"), issues);
        validate_hash(
            &specification.content_blake3,
            &format!("registry.specifications[{index}].content_blake3"),
            issues,
        );
        if !seen.insert(specification.path.as_str()) {
            push_issue(
                issues,
                "registry_specification_duplicate",
                &format!("registry.specifications[{index}].path"),
                "registry specification path occurs more than once",
            );
        }
    }
    if domain_hash_json(CAIRN_SPECIFICATION_SET_DOMAIN, &registry.specifications, "registry.specification_set_blake3")
        .is_ok_and(|expected| registry.specification_set_blake3 != expected)
    {
        push_issue(
            issues,
            "registry_specification_set_hash_invalid",
            "registry.specification_set_blake3",
            "registry specification-set identity is not self-consistent",
        );
    }
}

fn validate_cairn_rows(
    registry: &CairnContentBoundRequirementRegistryV1,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if registry.rows.is_empty() {
        push_issue(issues, "registry_requirement_missing", "registry.rows", "registry has no requirement rows");
    }
    let mut canonical = registry.rows.clone();
    canonical.sort_by(|left, right| {
        left.requirement_id
            .cmp(&right.requirement_id)
            .then(left.specification_path.cmp(&right.specification_path))
    });
    if canonical != registry.rows {
        push_issue(
            issues,
            "registry_order_invalid",
            "registry.rows",
            "registry requirement rows are not in canonical order",
        );
    }
    let specification_index = registry
        .specifications
        .iter()
        .map(|specification| (specification.path.as_str(), specification.content_blake3.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for (index, row) in registry.rows.iter().enumerate() {
        let row_path = format!("registry.rows[{index}]");
        validate_requirement_id(&row.requirement_id, &format!("{row_path}.requirement_id"), issues);
        if !seen.insert(row.requirement_id.as_str()) {
            push_issue(
                issues,
                "registry_requirement_duplicate",
                &format!("{row_path}.requirement_id"),
                "registry requirement ID occurs more than once",
            );
        }
        validate_specification_path(&row.specification_path, &format!("{row_path}.specification_path"), issues);
        validate_hash(&row.specification_blake3, &format!("{row_path}.specification_blake3"), issues);
        validate_hash(&row.requirement_body_blake3, &format!("{row_path}.requirement_body_blake3"), issues);
        validate_hash(&row.row_blake3, &format!("{row_path}.row_blake3"), issues);
        if row.source_span.start_byte >= row.source_span.end_byte
            || row.source_span.start_line > row.source_span.end_line
        {
            push_issue(
                issues,
                "registry_source_span_invalid",
                &format!("{row_path}.source_span"),
                "registry requirement source span is empty or unordered",
            );
        }
        match specification_index.get(row.specification_path.as_str()) {
            Some(expected) if *expected == row.specification_blake3 => {}
            Some(_) => push_issue(
                issues,
                "registry_specification_hash_invalid",
                &format!("{row_path}.specification_blake3"),
                "registry row specification identity does not match its specification",
            ),
            None => push_issue(
                issues,
                "registry_specification_missing",
                &format!("{row_path}.specification_path"),
                "registry row references an absent specification",
            ),
        }
        let material = CairnRequirementRowHashMaterial {
            requirement_id: &row.requirement_id,
            specification_path: &row.specification_path,
            specification_blake3: &row.specification_blake3,
            source_span: &row.source_span,
            requirement_body_blake3: &row.requirement_body_blake3,
        };
        if domain_hash_json(CAIRN_REQUIREMENT_ROW_DOMAIN, &material, &format!("{row_path}.row_blake3"))
            .is_ok_and(|expected| row.row_blake3 != expected)
        {
            push_issue(
                issues,
                "registry_row_hash_invalid",
                &format!("{row_path}.row_blake3"),
                "registry row identity is not self-consistent",
            );
        }
    }
}

fn validate_cairn_registry_hash(
    registry: &CairnContentBoundRequirementRegistryV1,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    let material = CairnRegistryHashMaterial {
        schema_version: &registry.schema_version,
        boundary: &registry.boundary,
        repository_id: &registry.repository_id,
        revision: &registry.revision,
        specification_set_blake3: &registry.specification_set_blake3,
        specifications: &registry.specifications,
        policy_blake3: &registry.policy_blake3,
        rows: &registry.rows,
        non_claims: &registry.non_claims,
    };
    if domain_hash_json(CAIRN_REGISTRY_DOMAIN, &material, "registry.registry_blake3")
        .is_ok_and(|expected| registry.registry_blake3 != expected)
    {
        push_issue(
            issues,
            "registry_hash_invalid",
            "registry.registry_blake3",
            "registry identity is not self-consistent",
        );
    }
}

fn validate_cairn_receipt(
    registry: &CairnContentBoundRequirementRegistryV1,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    let receipt = &registry.receipt;
    if receipt.schema_version != CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1 {
        push_issue(
            issues,
            "registry_receipt_schema_unsupported",
            "registry.receipt.schema_version",
            "supplied Cairn registry receipt schema is unsupported",
        );
    }
    validate_hash(&receipt.receipt_blake3, "registry.receipt.receipt_blake3", issues);
    validate_required_non_claims(&receipt.non_claims, CAIRN_REGISTRY_NON_CLAIMS, "registry.receipt.non_claims", issues);
    let links_match = receipt.registry_blake3 == registry.registry_blake3
        && receipt.repository_id == registry.repository_id
        && receipt.revision_kind == registry.revision.kind
        && receipt.revision == registry.revision.value
        && receipt.specification_set_blake3 == registry.specification_set_blake3
        && receipt.policy_blake3 == registry.policy_blake3
        && receipt.row_count == registry.rows.len()
        && receipt.non_claims == registry.non_claims;
    if !links_match {
        push_issue(
            issues,
            "registry_receipt_link_mismatch",
            "registry.receipt",
            "registry receipt fields do not match the registry",
        );
    }
    let material = CairnReceiptHashMaterial {
        schema_version: &receipt.schema_version,
        registry_blake3: &receipt.registry_blake3,
        repository_id: &receipt.repository_id,
        revision_kind: &receipt.revision_kind,
        revision: &receipt.revision,
        specification_set_blake3: &receipt.specification_set_blake3,
        policy_blake3: &receipt.policy_blake3,
        row_count: receipt.row_count,
        non_claims: &receipt.non_claims,
    };
    if domain_hash_json(CAIRN_RECEIPT_DOMAIN, &material, "registry.receipt.receipt_blake3")
        .is_ok_and(|expected| receipt.receipt_blake3 != expected)
    {
        push_issue(
            issues,
            "registry_receipt_hash_invalid",
            "registry.receipt.receipt_blake3",
            "registry receipt identity is not self-consistent",
        );
    }
}

fn validate_requirement_ref_shape(reference: &RequirementRefV1, issues: &mut Vec<RequirementValidationIssueV1>) {
    if reference.schema != REQUIREMENT_REF_SCHEMA_V1 {
        push_issue(
            issues,
            "reference_schema_unsupported",
            "reference.schema",
            "requirement reference schema is unsupported",
        );
    }
    validate_repository_id(&reference.repository_id, "reference.repository_id", issues);
    validate_stack_revision(&reference.revision, "reference.revision", issues);
    validate_requirement_id(&reference.requirement_id, "reference.requirement_id", issues);
    validate_specification_path(&reference.specification_path, "reference.specification_path", issues);
    for (digest, domain, field_path) in [
        (
            &reference.specification_identity,
            RequirementDigestDomainV1::Specification,
            "reference.specification_identity",
        ),
        (
            &reference.requirement_body_identity,
            RequirementDigestDomainV1::RequirementBody,
            "reference.requirement_body_identity",
        ),
        (&reference.policy_identity, RequirementDigestDomainV1::Policy, "reference.policy_identity"),
        (&reference.row_identity, RequirementDigestDomainV1::RequirementRow, "reference.row_identity"),
        (
            &reference.registry_identity,
            RequirementDigestDomainV1::RequirementRegistry,
            "reference.registry_identity",
        ),
        (
            &reference.registry_receipt_identity,
            RequirementDigestDomainV1::RequirementRegistryReceipt,
            "reference.registry_receipt_identity",
        ),
        (
            &reference.reference_identity,
            RequirementDigestDomainV1::RequirementReference,
            "reference.reference_identity",
        ),
    ] {
        validate_digest(digest, domain, field_path, issues);
    }
    if reference.verification_role != VerificationRole::RecordedOnly {
        push_issue(
            issues,
            "verification_role_promotion_denied",
            "reference.verification_role",
            "content-bound requirement references remain recorded-only",
        );
    }
    validate_required_non_claims(
        &reference.non_claims,
        CONTENT_BOUND_REQUIREMENT_NON_CLAIMS,
        "reference.non_claims",
        issues,
    );
    if requirement_ref_hash(reference).is_ok_and(|expected| reference.reference_identity.blake3 != expected) {
        push_issue(
            issues,
            "reference_identity_stale",
            "reference.reference_identity.blake3",
            "requirement reference identity does not match canonical fields",
        );
    }
}

fn compare_requirement_ref_to_registry(
    reference: &RequirementRefV1,
    registry: &CairnContentBoundRequirementRegistryV1,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if reference.repository_id != registry.repository_id {
        push_issue(
            issues,
            "repository_mismatch",
            "reference.repository_id",
            "requirement reference repository does not match the supplied registry",
        );
    }
    if reference.revision.kind.as_str() != registry.revision.kind || reference.revision.value != registry.revision.value
    {
        push_issue(
            issues,
            "revision_mismatch",
            "reference.revision",
            "requirement reference revision does not match the supplied registry",
        );
    }
    compare_digest_value(
        &reference.policy_identity,
        &registry.policy_blake3,
        "stale_policy",
        "reference.policy_identity.blake3",
        issues,
    );
    compare_digest_value(
        &reference.registry_identity,
        &registry.registry_blake3,
        "stale_registry",
        "reference.registry_identity.blake3",
        issues,
    );
    compare_digest_value(
        &reference.registry_receipt_identity,
        &registry.receipt.receipt_blake3,
        "stale_registry_receipt",
        "reference.registry_receipt_identity.blake3",
        issues,
    );
    let matches = registry
        .rows
        .iter()
        .filter(|row| row.requirement_id == reference.requirement_id)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        push_issue(
            issues,
            "requirement_missing",
            "reference.requirement_id",
            "requirement is absent from the supplied registry",
        );
        return;
    }
    if matches.len() > 1 {
        push_issue(
            issues,
            "requirement_duplicate",
            "reference.requirement_id",
            "requirement occurs more than once in the supplied registry",
        );
        return;
    }
    let row = matches[0];
    if reference.specification_path != row.specification_path {
        push_issue(
            issues,
            "stale_specification_path",
            "reference.specification_path",
            "requirement reference specification path does not match the registry row",
        );
    }
    compare_digest_value(
        &reference.specification_identity,
        &row.specification_blake3,
        "stale_specification",
        "reference.specification_identity.blake3",
        issues,
    );
    compare_digest_value(
        &reference.requirement_body_identity,
        &row.requirement_body_blake3,
        "stale_requirement",
        "reference.requirement_body_identity.blake3",
        issues,
    );
    compare_digest_value(
        &reference.row_identity,
        &row.row_blake3,
        "stale_row",
        "reference.row_identity.blake3",
        issues,
    );
}

fn validate_legacy_linkage(input: &RequirementLinkageInputV1, issues: &mut Vec<RequirementValidationIssueV1>) {
    if input.legacy_requirement_ids.is_empty() {
        push_issue(
            issues,
            "legacy_requirement_id_missing",
            "legacy_requirement_ids",
            "legacy recorded-only profile requires at least one requirement ID string",
        );
    }
    if !input.requirement_refs.is_empty() {
        push_issue(
            issues,
            "legacy_profile_content_bound_refs_forbidden",
            "requirement_refs",
            "legacy profile cannot silently promote typed requirement references",
        );
    }
    for (index, requirement_id) in input.legacy_requirement_ids.iter().enumerate() {
        if !valid_legacy_requirement_id(requirement_id) {
            push_issue(
                issues,
                "legacy_requirement_id_invalid",
                &format!("legacy_requirement_ids[{index}]"),
                "legacy requirement ID must retain bounded r[...] spelling",
            );
        }
    }
    if !input.non_claims.iter().any(|claim| claim == LEGACY_REQUIREMENT_BOUNDARY) {
        push_issue(issues, "legacy_boundary_missing", "non_claims", "legacy linkage boundary must remain explicit");
    }
}

fn validate_strict_linkage(
    input: &RequirementLinkageInputV1,
    registries: &BTreeMap<&str, &CairnContentBoundRequirementRegistryV1>,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if input.requirement_refs.is_empty() {
        push_issue(
            issues,
            "missing_content_bound_reference",
            "requirement_refs",
            "strict profile requires at least one content-bound requirement reference",
        );
    }
    validate_required_non_claims(&input.non_claims, CONTENT_BOUND_REQUIREMENT_NON_CLAIMS, "non_claims", issues);
    let mut seen = BTreeSet::new();
    for (index, reference) in input.requirement_refs.iter().enumerate() {
        let key = (reference.repository_id.as_str(), reference.requirement_id.as_str());
        if !seen.insert(key) {
            push_issue(
                issues,
                "requirement_reference_duplicate",
                &format!("requirement_refs[{index}]"),
                "strict requirement reference occurs more than once",
            );
        }
        match registries.get(reference.repository_id.as_str()) {
            Some(registry) => {
                let report = validate_requirement_ref(reference, registry);
                extend_prefixed_issues(issues, &format!("requirement_refs[{index}]"), report.issues);
            }
            None => push_issue(
                issues,
                "registry_missing",
                &format!("requirement_refs[{index}].repository_id"),
                "strict reference has no supplied registry for its repository",
            ),
        }
    }
}

fn validate_stack_revision_entries(entries: &[StackRevisionEntryV1], issues: &mut Vec<RequirementValidationIssueV1>) {
    if entries.is_empty() {
        push_issue(
            issues,
            "revision_set_empty",
            "revision_set.entries",
            "selected stack must contain at least one repository revision",
        );
    }
    let mut seen = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        validate_repository_id(&entry.repository_id, &format!("revision_set.entries[{index}].repository_id"), issues);
        validate_stack_revision(&entry.revision, &format!("revision_set.entries[{index}].revision"), issues);
        if !seen.insert(entry.repository_id.as_str()) {
            push_issue(
                issues,
                "duplicate_repository",
                &format!("revision_set.entries[{index}].repository_id"),
                "selected stack contains more than one revision for a repository",
            );
        }
    }
}

fn validate_context_links(
    context: &StackTraceContextV1,
    expectations: &StackTraceExpectationsV1,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    validate_digest(
        &context.evidence_graph_identity,
        RequirementDigestDomainV1::EvidenceGraph,
        "context.evidence_graph_identity",
        issues,
    );
    validate_digest(
        &expectations.evidence_graph_identity,
        RequirementDigestDomainV1::EvidenceGraph,
        "expectations.evidence_graph_identity",
        issues,
    );
    if context.evidence_graph_identity != expectations.evidence_graph_identity {
        push_issue(
            issues,
            "evidence_graph_stale",
            "context.evidence_graph_identity",
            "context evidence graph does not match the expected graph identity",
        );
    }
    validate_optional_context_link(
        context.release_identity.as_ref(),
        expectations.release_identity.as_ref(),
        RequirementDigestDomainV1::Release,
        "context.release_identity",
        context.required_links.contains(&StackTraceLinkRoleV1::Release),
        issues,
    );
    validate_optional_context_link(
        context.lifecycle_identity.as_ref(),
        expectations.lifecycle_identity.as_ref(),
        RequirementDigestDomainV1::Lifecycle,
        "context.lifecycle_identity",
        context.required_links.contains(&StackTraceLinkRoleV1::Lifecycle),
        issues,
    );
    validate_optional_context_link(
        context.runtime_identity.as_ref(),
        expectations.runtime_identity.as_ref(),
        RequirementDigestDomainV1::Runtime,
        "context.runtime_identity",
        context.required_links.contains(&StackTraceLinkRoleV1::Runtime),
        issues,
    );
    let mut canonical_roles = context.required_links.clone();
    canonical_roles.sort();
    let mut deduplicated = canonical_roles.clone();
    deduplicated.dedup();
    if deduplicated != canonical_roles {
        push_issue(
            issues,
            "required_link_duplicate",
            "context.required_links",
            "required context link role occurs more than once",
        );
    }
}

fn validate_optional_context_link(
    actual: Option<&RequirementDigestV1>,
    expected: Option<&RequirementDigestV1>,
    domain: RequirementDigestDomainV1,
    field_path: &str,
    required: bool,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if required && actual.is_none() {
        push_issue(issues, "required_link_missing", field_path, "profile-required context link is absent");
    }
    if let Some(actual) = actual {
        validate_digest(actual, domain.clone(), field_path, issues);
        match expected {
            Some(expected) => {
                validate_digest(expected, domain, &field_path.replacen("context.", "expectations.", 1), issues);
                if actual != expected {
                    push_issue(
                        issues,
                        "optional_link_stale",
                        field_path,
                        "present context link does not match its expected identity",
                    );
                }
            }
            None => push_issue(
                issues,
                "optional_link_unverified",
                field_path,
                "present optional context link has no expected identity",
            ),
        }
    }
}

fn validate_context_requirement_refs(
    context: &StackTraceContextV1,
    registries: &BTreeMap<&str, &CairnContentBoundRequirementRegistryV1>,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if context.requirement_refs.is_empty() {
        push_issue(
            issues,
            "missing_content_bound_reference",
            "context.requirement_refs",
            "stack trace context requires at least one content-bound reference",
        );
    }
    let revision_index = context
        .revision_set
        .entries
        .iter()
        .map(|entry| (entry.repository_id.as_str(), &entry.revision))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for (index, reference) in context.requirement_refs.iter().enumerate() {
        let key = (reference.repository_id.as_str(), reference.requirement_id.as_str());
        if !seen.insert(key) {
            push_issue(
                issues,
                "requirement_reference_duplicate",
                &format!("context.requirement_refs[{index}]"),
                "context requirement reference occurs more than once",
            );
        }
        match revision_index.get(reference.repository_id.as_str()) {
            Some(revision) if **revision == reference.revision => {}
            Some(_) => push_issue(
                issues,
                "selected_revision_mismatch",
                &format!("context.requirement_refs[{index}].revision"),
                "requirement reference does not match the selected stack revision",
            ),
            None => push_issue(
                issues,
                "selected_repository_missing",
                &format!("context.requirement_refs[{index}].repository_id"),
                "requirement reference repository is absent from the selected stack",
            ),
        }
        match registries.get(reference.repository_id.as_str()) {
            Some(registry) => {
                let report = validate_requirement_ref(reference, registry);
                extend_prefixed_issues(issues, &format!("context.requirement_refs[{index}]"), report.issues);
            }
            None => push_issue(
                issues,
                "registry_missing",
                &format!("context.requirement_refs[{index}].repository_id"),
                "context reference has no supplied registry for its repository",
            ),
        }
    }
}

fn registry_index<'a>(
    registries: &'a [CairnContentBoundRequirementRegistryV1],
    issues: &mut Vec<RequirementValidationIssueV1>,
) -> BTreeMap<&'a str, &'a CairnContentBoundRequirementRegistryV1> {
    let mut index = BTreeMap::new();
    for (registry_index, registry) in registries.iter().enumerate() {
        if index.insert(registry.repository_id.as_str(), registry).is_some() {
            push_issue(
                issues,
                "registry_repository_duplicate",
                &format!("registries[{registry_index}].repository_id"),
                "more than one registry was supplied for a repository",
            );
        }
    }
    index
}

fn requirement_ref_hash(reference: &RequirementRefV1) -> Result<String, RequirementValidationIssueV1> {
    domain_hash_json(
        REQUIREMENT_REF_DOMAIN,
        &RequirementRefHashMaterial {
            schema: &reference.schema,
            repository_id: &reference.repository_id,
            revision: &reference.revision,
            requirement_id: &reference.requirement_id,
            specification_path: &reference.specification_path,
            specification_identity: &reference.specification_identity,
            requirement_body_identity: &reference.requirement_body_identity,
            policy_identity: &reference.policy_identity,
            row_identity: &reference.row_identity,
            registry_identity: &reference.registry_identity,
            registry_receipt_identity: &reference.registry_receipt_identity,
            verification_role: &reference.verification_role,
            non_claims: &reference.non_claims,
        },
        "reference.reference_identity",
    )
}

fn stack_trace_context_hash(context: &StackTraceContextV1) -> Result<String, RequirementValidationIssueV1> {
    let mut revision_set = context.revision_set.clone();
    revision_set.entries.sort();
    let mut requirement_refs = context.requirement_refs.clone();
    requirement_refs.sort_by(requirement_ref_order);
    let mut required_links = context.required_links.clone();
    required_links.sort();
    domain_hash_json(
        STACK_TRACE_CONTEXT_DOMAIN,
        &StackTraceContextHashMaterial {
            schema: &context.schema,
            revision_set: &revision_set,
            requirement_refs: &requirement_refs,
            evidence_graph_identity: &context.evidence_graph_identity,
            release_identity: &context.release_identity,
            lifecycle_identity: &context.lifecycle_identity,
            runtime_identity: &context.runtime_identity,
            required_links: &required_links,
            non_claims: &context.non_claims,
        },
        "context.context_identity",
    )
}

fn requirement_ref_order(left: &RequirementRefV1, right: &RequirementRefV1) -> std::cmp::Ordering {
    left.repository_id
        .cmp(&right.repository_id)
        .then(left.requirement_id.cmp(&right.requirement_id))
        .then(left.reference_identity.blake3.cmp(&right.reference_identity.blake3))
}

fn validate_digest(
    digest: &RequirementDigestV1,
    expected_domain: RequirementDigestDomainV1,
    field_path: &str,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if digest.domain != expected_domain {
        push_issue(
            issues,
            "digest_domain_substitution",
            &format!("{field_path}.domain"),
            "digest role does not match the field's semantic domain",
        );
    }
    validate_hash(&digest.blake3, &format!("{field_path}.blake3"), issues);
}

fn compare_digest_value(
    digest: &RequirementDigestV1,
    expected: &str,
    code: &str,
    field_path: &str,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    if digest.blake3 != expected {
        push_issue(issues, code, field_path, "typed digest does not match the supplied registry identity");
    }
}

fn validate_repository_id(repository_id: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    let components = repository_id.split('/').collect::<Vec<_>>();
    let valid = !repository_id.is_empty()
        && repository_id.len() <= MAX_REPOSITORY_ID_BYTES
        && repository_id.trim() == repository_id
        && repository_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        && !repository_id.starts_with('/')
        && components.iter().all(|component| !component.is_empty() && *component != "." && *component != "..");
    if !valid {
        push_issue(
            issues,
            "repository_id_invalid",
            field_path,
            "repository identity is not a bounded portable identifier",
        );
    }
}

fn validate_cairn_revision(
    revision: &CairnContentBoundRevisionV1,
    field_path: &str,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    match revision.kind.as_str() {
        "git" => validate_git_revision(&revision.value, field_path, issues),
        "jujutsu" => validate_jujutsu_revision(&revision.value, field_path, issues),
        "opaque" => validate_opaque_revision(&revision.value, field_path, issues),
        _ => push_issue(
            issues,
            "revision_kind_unsupported",
            &format!("{field_path}.kind"),
            "revision kind must be git, jujutsu, or opaque",
        ),
    }
}

fn validate_stack_revision(
    revision: &StackRevisionV1,
    field_path: &str,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    match revision.kind {
        StackRevisionKindV1::Git => validate_git_revision(&revision.value, field_path, issues),
        StackRevisionKindV1::Jujutsu => validate_jujutsu_revision(&revision.value, field_path, issues),
        StackRevisionKindV1::Opaque => validate_opaque_revision(&revision.value, field_path, issues),
    }
}

fn validate_git_revision(revision: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    let valid_length = matches!(revision.len(), GIT_SHA1_HEX_LENGTH | GIT_SHA256_HEX_LENGTH);
    if !valid_length || !is_lower_hex(revision) {
        push_issue(
            issues,
            "revision_invalid",
            &format!("{field_path}.value"),
            "Git revision must be a complete lowercase hexadecimal identity",
        );
    }
}

fn validate_jujutsu_revision(revision: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    let valid_length = (JUJUTSU_REVISION_MIN_HEX_LENGTH..=JUJUTSU_REVISION_MAX_HEX_LENGTH).contains(&revision.len());
    if !valid_length || !is_lower_hex(revision) {
        push_issue(
            issues,
            "revision_invalid",
            &format!("{field_path}.value"),
            "Jujutsu revision must be a bounded lowercase hexadecimal identity",
        );
    }
}

fn validate_opaque_revision(revision: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    let valid = !revision.is_empty()
        && revision.len() <= MAX_OPAQUE_REVISION_BYTES
        && revision.trim() == revision
        && !revision.chars().any(char::is_control);
    if !valid {
        push_issue(
            issues,
            "revision_invalid",
            &format!("{field_path}.value"),
            "opaque revision must be bounded, trimmed, and free of control characters",
        );
    }
}

fn stack_revision_from_cairn(revision: &CairnContentBoundRevisionV1) -> Option<StackRevisionV1> {
    let kind = match revision.kind.as_str() {
        "git" => StackRevisionKindV1::Git,
        "jujutsu" => StackRevisionKindV1::Jujutsu,
        "opaque" => StackRevisionKindV1::Opaque,
        _ => return None,
    };
    Some(StackRevisionV1 {
        kind,
        value: revision.value.clone(),
    })
}

fn validate_requirement_id(requirement_id: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    let valid = !requirement_id.is_empty()
        && requirement_id.len() <= MAX_REQUIREMENT_ID_BYTES
        && requirement_id.contains('.')
        && requirement_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'));
    if !valid {
        push_issue(issues, "requirement_id_invalid", field_path, "requirement ID is not a bounded portable identifier");
    }
}

fn valid_legacy_requirement_id(requirement_id: &str) -> bool {
    let Some(inner) = requirement_id.strip_prefix("r[").and_then(|value| value.strip_suffix(']')) else {
        return false;
    };
    let mut issues = Vec::new();
    validate_requirement_id(inner, "legacy_requirement_id", &mut issues);
    issues.is_empty()
}

fn validate_specification_path(path: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    let components = path.split('/').collect::<Vec<_>>();
    let valid = !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && components.iter().all(|component| !component.is_empty() && *component != "." && *component != "..")
        && path.starts_with("cairn/specs/")
        && path.ends_with("/spec.md");
    if !valid {
        push_issue(
            issues,
            "specification_path_unsafe",
            field_path,
            "specification path is not a safe accepted-spec path",
        );
    }
}

fn validate_required_non_claims(
    non_claims: &[String],
    required_non_claims: &[&str],
    field_path: &str,
    issues: &mut Vec<RequirementValidationIssueV1>,
) {
    for required in required_non_claims {
        if !non_claims.iter().any(|claim| claim == required) {
            push_issue(issues, "non_claim_missing", field_path, "required non-claim boundary is missing");
        }
    }
}

fn validate_hash(hash: &str, field_path: &str, issues: &mut Vec<RequirementValidationIssueV1>) {
    if hash.len() != BLAKE3_HEX_LENGTH || !is_lower_hex(hash) {
        push_issue(
            issues,
            "blake3_invalid",
            field_path,
            "BLAKE3 identity must use 64 lowercase hexadecimal characters",
        );
    }
}

fn is_lower_hex(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn digest(domain: RequirementDigestDomainV1, blake3: &str) -> RequirementDigestV1 {
    RequirementDigestV1 {
        domain,
        blake3: blake3.to_owned(),
    }
}

fn empty_digest(domain: RequirementDigestDomainV1) -> RequirementDigestV1 {
    RequirementDigestV1 {
        domain,
        blake3: String::new(),
    }
}

fn required_non_claims(required: &[&str]) -> Vec<String> {
    required.iter().map(|claim| (*claim).to_owned()).collect()
}

fn domain_hash_json<T: Serialize>(
    domain: &str,
    value: &T,
    field_path: &str,
) -> Result<String, RequirementValidationIssueV1> {
    let value = serde_json::to_value(value).map_err(|_error| {
        issue("canonical_serialization_failed", field_path, "canonical hash material could not be serialized")
    })?;
    let bytes = canonical_json_bytes(&value).map_err(|_error| {
        issue("canonical_serialization_failed", field_path, "canonical hash material could not be encoded")
    })?;
    Ok(domain_hash_bytes(domain, &bytes))
}

fn domain_hash_bytes(domain: &str, bytes: &[u8]) -> String {
    let mut material = Vec::with_capacity(domain.len().saturating_add(1).saturating_add(bytes.len()));
    material.extend_from_slice(domain.as_bytes());
    material.push(DOMAIN_SEPARATOR);
    material.extend_from_slice(bytes);
    blake3_hex(&material)
}

fn issue(code: &str, field_path: &str, message: &str) -> RequirementValidationIssueV1 {
    RequirementValidationIssueV1 {
        code: code.to_owned(),
        field_path: field_path.to_owned(),
        message: message.to_owned(),
    }
}

fn push_issue(issues: &mut Vec<RequirementValidationIssueV1>, code: &str, field_path: &str, message: &str) {
    issues.push(issue(code, field_path, message));
}

fn extend_prefixed_issues(
    issues: &mut Vec<RequirementValidationIssueV1>,
    prefix: &str,
    nested: Vec<RequirementValidationIssueV1>,
) {
    issues.extend(nested.into_iter().map(|nested_issue| RequirementValidationIssueV1 {
        code: nested_issue.code,
        field_path: format!("{prefix}.{}", nested_issue.field_path),
        message: nested_issue.message,
    }));
}

fn sort_issues(issues: &mut Vec<RequirementValidationIssueV1>) {
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPOSITORY_ID: &str = "OnixResearch/valence";
    const OTHER_REPOSITORY_ID: &str = "OnixResearch/other";
    const REVISION_VALUE: &str = "valence-fixture-v1";
    const REQUIREMENT_ID: &str = "valence.provenance.content_bound_requirement_ref";
    const SPECIFICATION_PATH: &str = "cairn/specs/content-bound-requirements/spec.md";

    fn fixture_registry() -> CairnContentBoundRequirementRegistryV1 {
        let revision = CairnContentBoundRevisionV1 {
            kind: "opaque".to_owned(),
            value: REVISION_VALUE.to_owned(),
        };
        let specification_hash =
            domain_hash_bytes("cairn.requirement-registry.specification.v1", b"fixture specification bytes");
        let requirement_body_hash =
            domain_hash_bytes("cairn.requirement-registry.requirement-body.v1", b"fixture requirement body");
        let source_span = CairnContentBoundSourceSpanV1 {
            start_byte: 1,
            end_byte: 25,
            start_line: 1,
            end_line: 1,
        };
        let row_hash = domain_hash_json(
            CAIRN_REQUIREMENT_ROW_DOMAIN,
            &CairnRequirementRowHashMaterial {
                requirement_id: REQUIREMENT_ID,
                specification_path: SPECIFICATION_PATH,
                specification_blake3: &specification_hash,
                source_span: &source_span,
                requirement_body_blake3: &requirement_body_hash,
            },
            "fixture.row",
        )
        .expect("row hash");
        let specifications = vec![CairnContentBoundSpecificationIdentityV1 {
            path: SPECIFICATION_PATH.to_owned(),
            content_blake3: specification_hash.clone(),
        }];
        let rows = vec![CairnContentBoundRequirementRowV1 {
            requirement_id: REQUIREMENT_ID.to_owned(),
            specification_path: SPECIFICATION_PATH.to_owned(),
            specification_blake3: specification_hash,
            source_span,
            requirement_body_blake3: requirement_body_hash,
            row_blake3: row_hash,
        }];
        let specification_set_hash =
            domain_hash_json(CAIRN_SPECIFICATION_SET_DOMAIN, &specifications, "fixture.specifications")
                .expect("specification-set hash");
        let policy_hash = domain_hash_bytes("cairn.requirement-registry.policy.v1", b"fixture policy bytes");
        let non_claims = required_non_claims(CAIRN_REGISTRY_NON_CLAIMS);
        let registry_hash = domain_hash_json(
            CAIRN_REGISTRY_DOMAIN,
            &CairnRegistryHashMaterial {
                schema_version: CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1,
                boundary: CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY,
                repository_id: REPOSITORY_ID,
                revision: &revision,
                specification_set_blake3: &specification_set_hash,
                specifications: &specifications,
                policy_blake3: &policy_hash,
                rows: &rows,
                non_claims: &non_claims,
            },
            "fixture.registry",
        )
        .expect("registry hash");
        let receipt_hash = domain_hash_json(
            CAIRN_RECEIPT_DOMAIN,
            &CairnReceiptHashMaterial {
                schema_version: CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1,
                registry_blake3: &registry_hash,
                repository_id: REPOSITORY_ID,
                revision_kind: &revision.kind,
                revision: &revision.value,
                specification_set_blake3: &specification_set_hash,
                policy_blake3: &policy_hash,
                row_count: rows.len(),
                non_claims: &non_claims,
            },
            "fixture.receipt",
        )
        .expect("receipt hash");
        CairnContentBoundRequirementRegistryV1 {
            schema_version: CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1.to_owned(),
            boundary: CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY.to_owned(),
            repository_id: REPOSITORY_ID.to_owned(),
            revision: revision.clone(),
            specification_set_blake3: specification_set_hash.clone(),
            specifications,
            policy_blake3: policy_hash.clone(),
            rows,
            non_claims: non_claims.clone(),
            registry_blake3: registry_hash.clone(),
            receipt: CairnContentBoundRequirementRegistryReceiptV1 {
                schema_version: CAIRN_CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1.to_owned(),
                registry_blake3: registry_hash,
                repository_id: REPOSITORY_ID.to_owned(),
                revision_kind: revision.kind,
                revision: revision.value,
                specification_set_blake3: specification_set_hash,
                policy_blake3: policy_hash,
                row_count: 1,
                non_claims,
                receipt_blake3: receipt_hash,
            },
        }
    }

    fn fixture_reference() -> RequirementRefV1 {
        build_requirement_ref(&fixture_registry(), REQUIREMENT_ID).expect("requirement reference")
    }

    fn fixture_revision_entry() -> StackRevisionEntryV1 {
        StackRevisionEntryV1 {
            repository_id: REPOSITORY_ID.to_owned(),
            revision: StackRevisionV1 {
                kind: StackRevisionKindV1::Opaque,
                value: REVISION_VALUE.to_owned(),
            },
        }
    }

    fn graph_digest(label: &str) -> RequirementDigestV1 {
        digest(RequirementDigestDomainV1::EvidenceGraph, &blake3_hex(label.as_bytes()))
    }

    fn context_envelope() -> StackTraceValidationEnvelopeV1 {
        let revision_set = build_stack_revision_set(vec![fixture_revision_entry()]).expect("revision set");
        let evidence_graph_identity = graph_digest("current-graph");
        let context = build_stack_trace_context(StackTraceContextDraftV1 {
            revision_set,
            requirement_refs: vec![fixture_reference()],
            evidence_graph_identity: evidence_graph_identity.clone(),
            release_identity: None,
            lifecycle_identity: None,
            runtime_identity: None,
            required_links: Vec::new(),
            non_claims: required_non_claims(STACK_TRACE_CONTEXT_NON_CLAIMS),
        })
        .expect("stack trace context");
        StackTraceValidationEnvelopeV1 {
            schema: STACK_TRACE_VALIDATION_ENVELOPE_SCHEMA_V1.to_owned(),
            context,
            registries: vec![fixture_registry()],
            expectations: StackTraceExpectationsV1 {
                evidence_graph_identity,
                release_identity: None,
                lifecycle_identity: None,
                runtime_identity: None,
            },
        }
    }

    fn codes(issues: &[RequirementValidationIssueV1]) -> Vec<&str> {
        issues.iter().map(|issue| issue.code.as_str()).collect()
    }

    #[test]
    fn current_typed_reference_validates_against_frozen_cairn_contract() {
        // r[verify valence.provenance.content_bound_requirement_ref]
        // r[verify valence.provenance.requirement_registry_validation]
        let registry = fixture_registry();
        assert!(validate_cairn_content_bound_requirement_registry(&registry).is_empty());
        let reference = build_requirement_ref(&registry, REQUIREMENT_ID).expect("reference");
        let report = validate_requirement_ref(&reference, &registry);
        assert!(report.valid, "issues: {:?}", report.issues);
        assert_eq!(reference.verification_role, VerificationRole::RecordedOnly);
        assert_eq!(CAIRN_CONTENT_BOUND_REGISTRY_CONTRACT_REVISION, "d953fe11ab620f3a42bdad1db51bc7672dd29824");
    }

    #[test]
    fn stale_cross_repository_duplicate_and_domain_substitution_fail_closed() {
        // r[verify valence.provenance.requirement_registry_validation]
        let registry = fixture_registry();
        let mut reference = fixture_reference();
        reference.repository_id = OTHER_REPOSITORY_ID.to_owned();
        reference.revision.value = "wrong-revision-v1".to_owned();
        reference.specification_identity.blake3 = reference.row_identity.blake3.clone();
        reference.row_identity.domain = RequirementDigestDomainV1::Policy;
        reference.non_claims.clear();
        let report = validate_requirement_ref(&reference, &registry);
        let issue_codes = codes(&report.issues);
        assert!(!report.valid);
        assert!(issue_codes.contains(&"repository_mismatch"));
        assert!(issue_codes.contains(&"revision_mismatch"));
        assert!(issue_codes.contains(&"stale_specification"));
        assert!(issue_codes.contains(&"digest_domain_substitution"));
        assert!(issue_codes.contains(&"non_claim_missing"));
        assert!(issue_codes.contains(&"reference_identity_stale"));

        let mut missing = fixture_reference();
        missing.requirement_id = "valence.provenance.missing".to_owned();
        let missing_report = validate_requirement_ref(&missing, &registry);
        assert!(!missing_report.valid);
        assert!(codes(&missing_report.issues).contains(&"requirement_missing"));

        let mut stale_registry = registry.clone();
        stale_registry.registry_blake3 = blake3_hex(b"stale-registry");
        let stale_report = validate_requirement_ref(&fixture_reference(), &stale_registry);
        assert!(!stale_report.valid);
        assert!(codes(&stale_report.issues).contains(&"registry_hash_invalid"));
        assert!(codes(&stale_report.issues).contains(&"registry_receipt_link_mismatch"));
        assert!(codes(&stale_report.issues).contains(&"stale_registry"));

        let mut duplicate_registry = registry;
        duplicate_registry.rows.push(duplicate_registry.rows[0].clone());
        let duplicate = validate_requirement_ref(&fixture_reference(), &duplicate_registry);
        assert!(!duplicate.valid);
        assert!(codes(&duplicate.issues).contains(&"registry_requirement_duplicate"));
        assert!(codes(&duplicate.issues).contains(&"requirement_duplicate"));
    }

    #[test]
    fn stack_context_identity_is_order_independent_for_a_selected_stack() {
        // r[verify valence.provenance.stack_trace_context]
        let first_entry = fixture_revision_entry();
        let second_entry = StackRevisionEntryV1 {
            repository_id: OTHER_REPOSITORY_ID.to_owned(),
            revision: StackRevisionV1 {
                kind: StackRevisionKindV1::Opaque,
                value: "other-fixture-v1".to_owned(),
            },
        };
        let first =
            build_stack_revision_set(vec![first_entry.clone(), second_entry.clone()]).expect("first revision set");
        let second = build_stack_revision_set(vec![second_entry, first_entry]).expect("second revision set");
        assert_eq!(first, second);

        let report = validate_stack_trace_context(&context_envelope());
        assert!(report.valid, "issues: {:?}", report.issues);
    }

    #[test]
    fn duplicate_repository_and_stale_optional_link_fail_closed() {
        // r[verify valence.provenance.requirement_registry_validation]
        let duplicate = build_stack_revision_set(vec![fixture_revision_entry(), fixture_revision_entry()]);
        assert!(duplicate.is_err());
        assert!(codes(&duplicate.expect_err("duplicate must fail")).contains(&"duplicate_repository"));

        let mut envelope = context_envelope();
        envelope.context.release_identity =
            Some(digest(RequirementDigestDomainV1::Release, &blake3_hex(b"stale-release")));
        envelope.expectations.release_identity =
            Some(digest(RequirementDigestDomainV1::Release, &blake3_hex(b"current-release")));
        let report = validate_stack_trace_context(&envelope);
        assert!(!report.valid);
        assert!(codes(&report.issues).contains(&"optional_link_stale"));
        assert!(codes(&report.issues).contains(&"context_identity_stale"));

        let mut unverified = context_envelope();
        unverified.context.release_identity =
            Some(digest(RequirementDigestDomainV1::Release, &blake3_hex(b"unverified-release")));
        let unverified_report = validate_stack_trace_context(&unverified);
        assert!(!unverified_report.valid);
        assert!(codes(&unverified_report.issues).contains(&"optional_link_unverified"));
    }

    #[test]
    fn legacy_ids_remain_recorded_only_and_cannot_satisfy_strict_profile() {
        // r[verify valence.provenance.legacy_requirement_id_boundary]
        let link = ProvenanceLink {
            source_id: "source:fixture".to_owned(),
            function_object_id: "function:fixture".to_owned(),
            requirement_ids: vec![format!("r[{REQUIREMENT_ID}]")],
            artifact_hash: None,
            binary_hash: None,
        };
        let serialized_link = serde_json::to_value(&link).expect("serialize legacy provenance link");
        assert!(serialized_link.get("requirement_refs").is_none());
        let legacy = legacy_requirement_linkage_from_provenance(&link);
        let legacy_report = validate_requirement_linkage(&legacy, &[]);
        assert!(legacy_report.valid, "issues: {:?}", legacy_report.issues);
        assert!(!legacy_report.strict_profile_satisfied);
        assert_eq!(legacy.legacy_requirement_ids, link.requirement_ids);

        let mut strict = legacy;
        strict.profile = RequirementLinkageProfileV1::StrictContentBound;
        strict.non_claims = required_non_claims(CONTENT_BOUND_REQUIREMENT_NON_CLAIMS);
        let strict_report = validate_requirement_linkage(&strict, &[fixture_registry()]);
        assert!(!strict_report.valid);
        assert!(codes(&strict_report.issues).contains(&"missing_content_bound_reference"));
    }
}

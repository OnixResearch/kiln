//! Pure Aeneas/Lean translation-chain identity and linkage validation.
//!
//! The shell supplies measured digests and parsed JSON observations for
//! confined local artifacts. This module validates those in-memory values and
//! computes canonical identities without filesystem, process, environment,
//! clock, or network access. Passing validation establishes bounded identity, linkage,
//! role-policy, and non-claim conformance only.
//!
//! r[impl valence.provenance.aeneas_translation_chain.profile]
//! r[impl valence.provenance.aeneas_translation_chain.linkage]
//! r[impl valence.provenance.aeneas_translation_chain.charon_contract]
//! r[impl valence.provenance.aeneas_translation_chain.translation_manifest]
//! r[impl valence.provenance.aeneas_translation_chain.identity]
//! r[impl valence.provenance.aeneas_translation_chain.digest_roles]
//! r[impl valence.provenance.aeneas_translation_chain.role_policy]
//! r[impl valence.provenance.aeneas_translation_chain.theorem_link]
//! r[impl valence.provenance.aeneas_translation_chain.adequacy_link]
//! r[impl valence.provenance.aeneas_translation_chain.cli]
//! r[impl valence.provenance.aeneas_translation_chain.fixture]
//! r[impl valence.provenance.aeneas_translation_chain.handoff]
//! r[impl valence.provenance.aeneas_translation_chain.non_claims]

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;

pub const AENEAS_CHAIN_SCHEMA: &str = "valence.formal-proof.aeneas-lean-chain.v1";
pub const AENEAS_PROFILE: &str = "aeneas-lean/v1";
pub const AENEAS_SUPPORTED_CLAIM: &str = "Aeneas translation-chain validation proves canonical identity, exact typed linkage, role policy, and required non-claim conformance only";

const OCTET_ADAPTER_SCHEMA: &str = "octet-aeneas-lean-valence-adapter/v1";
const OCTET_RECEIPT_SCHEMA: &str = "octet-formal-proof-envelope/v1";
const OCTET_PROVENANCE_SCHEMA: &str = "octet-valence-provenance-export/v1";
const OCTET_PROVIDER_PROFILE_SCHEMA: &str = "octet-aeneas-lean-provider-profile/v1";
const OCTET_PROVIDER: &str = "aeneas-lean";
const OCTET_OWNER: &str = "octet";
const FORMAL_PROOF_TIER: &str = "formal_proof";
const VALID_STATUS: &str = "valid";
const PASSED_STATUS: &str = "passed";
const NON_VACUOUS_DECISION: &str = "non_vacuous";

const REVIEWED_TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const REVIEWED_POINTER_WIDTH_BITS: u16 = 64;
const REVIEWED_ENDIANNESS: &str = "little";
const REVIEWED_MIR_START_STAGE: &str = "borrowck";
const REVIEWED_DROP_POLICY: &str = "include-drops";
const REVIEWED_START_FROM: &str = "crate";
const REVIEWED_INCLUDE_RULE: &str = "src/verified";
const REVIEWED_TRANSFORMATION: &str = "preserve-spans";
const REVIEWED_SERIALIZATION_FORMAT: &str = "llbc-json-v1";

const EXPECTED_PROPERTY_COUNT: u32 = 1;
const EXPECTED_RECORDED_ONLY_COUNT: u32 = 6;
const EXPECTED_BOUNDARY_COUNT: u32 = 10;
const EXPECTED_NODE_COUNT: usize = 17;
const EXPECTED_ADAPTER_STAGE_COUNT: usize = 6;

const REQUIRED_NON_CLAIMS: &[AeneasNonClaim] = &[
    AeneasNonClaim::RustCorrectness,
    AeneasNonClaim::CharonSoundness,
    AeneasNonClaim::AeneasSoundness,
    AeneasNonClaim::LeanSoundness,
    AeneasNonClaim::TranslationSoundness,
    AeneasNonClaim::ExternalModelSoundness,
    AeneasNonClaim::TheoremTruth,
    AeneasNonClaim::AdequacyPolicyCorrectness,
    AeneasNonClaim::RequirementSelectionCorrectness,
    AeneasNonClaim::VerifierSoundness,
    AeneasNonClaim::SemanticEquivalence,
    AeneasNonClaim::RequirementSatisfaction,
    AeneasNonClaim::WholeProgramCorrectness,
    AeneasNonClaim::LifecycleReadiness,
    AeneasNonClaim::ReleaseEligibility,
];

const REQUIRED_ROLES: &[AeneasNodeRole] = &[
    AeneasNodeRole::ProviderProfile,
    AeneasNodeRole::RustSource,
    AeneasNodeRole::CharonContract,
    AeneasNodeRole::CharonExtraction,
    AeneasNodeRole::Llbc,
    AeneasNodeRole::AeneasTranslation,
    AeneasNodeRole::TranslationManifest,
    AeneasNodeRole::GeneratedLean,
    AeneasNodeRole::ExternalModel,
    AeneasNodeRole::MaintainedTheorem,
    AeneasNodeRole::AdequacyDecision,
    AeneasNodeRole::TrustClosure,
    AeneasNodeRole::PackageLock,
    AeneasNodeRole::LeanChecker,
    AeneasNodeRole::OctetReceipt,
    AeneasNodeRole::ProofCard,
    AeneasNodeRole::ProvenanceProjection,
];

const CLAIM_BINDING_ROLES: &[AeneasNodeRole] = &[
    AeneasNodeRole::MaintainedTheorem,
    AeneasNodeRole::AdequacyDecision,
    AeneasNodeRole::TrustClosure,
    AeneasNodeRole::LeanChecker,
    AeneasNodeRole::OctetReceipt,
    AeneasNodeRole::ProofCard,
    AeneasNodeRole::ProvenanceProjection,
];

const REQUIRED_EDGES: &[(AeneasNodeRole, AeneasNodeRole, AeneasEdgeKind)] = &[
    (AeneasNodeRole::ProviderProfile, AeneasNodeRole::CharonContract, AeneasEdgeKind::ConfiguredBy),
    (AeneasNodeRole::RustSource, AeneasNodeRole::CharonExtraction, AeneasEdgeKind::ExtractedFrom),
    (AeneasNodeRole::CharonContract, AeneasNodeRole::CharonExtraction, AeneasEdgeKind::ConfiguredBy),
    (AeneasNodeRole::CharonExtraction, AeneasNodeRole::Llbc, AeneasEdgeKind::GeneratedAs),
    (AeneasNodeRole::Llbc, AeneasNodeRole::AeneasTranslation, AeneasEdgeKind::TranslatedFrom),
    (
        AeneasNodeRole::AeneasTranslation,
        AeneasNodeRole::TranslationManifest,
        AeneasEdgeKind::DeclaredInManifest,
    ),
    (AeneasNodeRole::TranslationManifest, AeneasNodeRole::GeneratedLean, AeneasEdgeKind::GeneratedAs),
    (AeneasNodeRole::GeneratedLean, AeneasNodeRole::ExternalModel, AeneasEdgeKind::ModeledBy),
    (AeneasNodeRole::GeneratedLean, AeneasNodeRole::MaintainedTheorem, AeneasEdgeKind::TheoremOver),
    (AeneasNodeRole::ExternalModel, AeneasNodeRole::TrustClosure, AeneasEdgeKind::Trusts),
    (AeneasNodeRole::MaintainedTheorem, AeneasNodeRole::AdequacyDecision, AeneasEdgeKind::ApprovedAs),
    (AeneasNodeRole::MaintainedTheorem, AeneasNodeRole::TrustClosure, AeneasEdgeKind::Trusts),
    (AeneasNodeRole::MaintainedTheorem, AeneasNodeRole::LeanChecker, AeneasEdgeKind::CheckedWith),
    (AeneasNodeRole::AdequacyDecision, AeneasNodeRole::LeanChecker, AeneasEdgeKind::ApprovedAs),
    (AeneasNodeRole::TrustClosure, AeneasNodeRole::LeanChecker, AeneasEdgeKind::Trusts),
    (AeneasNodeRole::PackageLock, AeneasNodeRole::LeanChecker, AeneasEdgeKind::CheckedWith),
    (AeneasNodeRole::LeanChecker, AeneasNodeRole::OctetReceipt, AeneasEdgeKind::AttestedBy),
    (AeneasNodeRole::OctetReceipt, AeneasNodeRole::ProofCard, AeneasEdgeKind::AttestedBy),
    (AeneasNodeRole::OctetReceipt, AeneasNodeRole::ProvenanceProjection, AeneasEdgeKind::AttestedBy),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AeneasNodeRole {
    ProviderProfile,
    RustSource,
    CharonContract,
    CharonExtraction,
    Llbc,
    AeneasTranslation,
    TranslationManifest,
    GeneratedLean,
    ExternalModel,
    MaintainedTheorem,
    AdequacyDecision,
    TrustClosure,
    PackageLock,
    LeanChecker,
    OctetReceipt,
    ProofCard,
    ProvenanceProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AeneasDigestDomain {
    ProviderProfile,
    RustSource,
    ExtractionProfile,
    ExtractionObservation,
    Llbc,
    TranslationObservation,
    TranslationManifest,
    GeneratedModel,
    ExternalModel,
    Theorem,
    AdequacyDecision,
    TrustClosure,
    PackageLock,
    CheckerReceipt,
    OctetReceipt,
    ProofCard,
    ProvenanceProjection,
    CompleteChain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AeneasVerificationRole {
    Property,
    RecordedOnly,
    Boundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AeneasProducer {
    Octet,
    Charon,
    Aeneas,
    Lean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AeneasNodeStatus {
    Approved,
    Complete,
    Passed,
    Recorded,
    Reviewed,
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AeneasEdgeKind {
    ApprovedAs,
    AttestedBy,
    CheckedWith,
    ConfiguredBy,
    DeclaredInManifest,
    ExtractedFrom,
    GeneratedAs,
    ModeledBy,
    TheoremOver,
    TranslatedFrom,
    Trusts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AeneasNonClaim {
    RustCorrectness,
    CharonSoundness,
    AeneasSoundness,
    LeanSoundness,
    TranslationSoundness,
    ExternalModelSoundness,
    TheoremTruth,
    AdequacyPolicyCorrectness,
    RequirementSelectionCorrectness,
    VerifierSoundness,
    SemanticEquivalence,
    RequirementSatisfaction,
    WholeProgramCorrectness,
    LifecycleReadiness,
    ReleaseEligibility,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasDigest {
    pub domain: AeneasDigestDomain,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasSourceSpan {
    pub source_id: String,
    pub begin_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasClaimBinding {
    pub theorem_id: String,
    pub approved_statement_blake3: String,
    pub requirement_id: String,
    pub claim_class: String,
    pub adequacy_policy_id: String,
    pub adequacy_decision: AeneasDigest,
    pub trust_closure: AeneasDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasChainNode {
    pub stable_id: String,
    pub role: AeneasNodeRole,
    pub digest: AeneasDigest,
    pub artifact: String,
    pub display_label: String,
    pub producer: AeneasProducer,
    pub verification_role: AeneasVerificationRole,
    pub status: AeneasNodeStatus,
    pub source_span: Option<AeneasSourceSpan>,
    pub claim: Option<AeneasClaimBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasChainEdge {
    pub from: String,
    pub to: String,
    pub kind: AeneasEdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasProducerFixture {
    pub octet_revision: String,
    pub schema: String,
    pub artifact: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasCharonContract {
    pub reviewed_profile_id: String,
    pub extraction_profile_identity: AeneasDigest,
    pub has_errors: bool,
    pub skip_borrowck: bool,
    pub target_triple: String,
    pub pointer_width_bits: u16,
    pub endianness: String,
    pub mir_start_stage: String,
    pub drop_policy: String,
    pub start_from: String,
    pub include_rules: Vec<String>,
    pub exclude_rules: Vec<String>,
    pub opaque_rules: Vec<String>,
    pub transformation_settings: Vec<String>,
    pub serialization_format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasManifestDeclaration {
    pub def_id: u64,
    pub run_identity: String,
    pub declaration_identity: String,
    pub rust_name: String,
    pub lean_name: String,
    pub lean_file: String,
    pub source: AeneasSourceSpan,
    pub is_opaque: bool,
    pub can_fail: bool,
    pub can_diverge: bool,
    pub is_recursive: bool,
    pub loop_parent_def_id: Option<u64>,
    pub loop_parent_declaration_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasTranslationMetadata {
    pub manifest_run_identity: String,
    pub aeneas_version: String,
    pub charon_version: String,
    pub crate_name: String,
    pub type_count: u32,
    pub global_count: u32,
    pub trait_declaration_count: u32,
    pub trait_implementation_count: u32,
    pub declarations: Vec<AeneasManifestDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasRoleSummary {
    pub property_count: u32,
    pub recorded_only_count: u32,
    pub boundary_count: u32,
    pub verified_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeneasTranslationChain {
    pub schema: String,
    pub profile: String,
    pub producer_fixture: AeneasProducerFixture,
    pub nodes: Vec<AeneasChainNode>,
    pub edges: Vec<AeneasChainEdge>,
    pub charon_contract: AeneasCharonContract,
    pub translation_metadata: AeneasTranslationMetadata,
    pub claim: AeneasClaimBinding,
    pub octet_artifact_verified: bool,
    pub role_summary: AeneasRoleSummary,
    pub non_claims: Vec<AeneasNonClaim>,
    pub declared_chain_identity: AeneasDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedAeneasArtifact {
    pub artifact: String,
    pub measured_blake3: String,
    pub parsed_json: Option<serde_json::Value>,
}

type ObservedArtifactMap<'a> = BTreeMap<&'a str, &'a LoadedAeneasArtifact>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AeneasChainIssue {
    pub field: String,
    pub code: &'static str,
}

impl AeneasChainIssue {
    #[must_use]
    pub fn new(field: impl Into<String>, code: &'static str) -> Self {
        Self {
            field: field.into(),
            code,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AeneasChainHandoff {
    pub chain_identity: String,
    pub extraction_profile_identity: String,
    pub translation_manifest_identity: String,
    pub adequacy_decision_identity: String,
    pub octet_receipt_identity: String,
    pub theorem_id: String,
    pub property_count: u32,
    pub recorded_only_count: u32,
    pub boundary_count: u32,
    pub non_claims: Vec<AeneasNonClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AeneasTranslationChainReport {
    pub valid: bool,
    pub computed_chain_identity: Option<String>,
    pub property_count: u32,
    pub recorded_only_count: u32,
    pub boundary_count: u32,
    pub issues: Vec<AeneasChainIssue>,
    pub handoff: Option<AeneasChainHandoff>,
    pub supported_claim: &'static str,
}

/// Return every relative artifact locator required by the read-only shell.
#[must_use]
pub fn aeneas_translation_chain_artifacts(chain: &AeneasTranslationChain) -> Vec<String> {
    let mut artifacts = chain.nodes.iter().map(|node| node.artifact.clone()).collect::<Vec<_>>();
    artifacts.push(chain.producer_fixture.artifact.clone());
    artifacts.sort();
    artifacts.dedup();
    artifacts
}

/// Validate a complete Aeneas/Lean translation chain over already-loaded bytes.
#[must_use]
pub fn validate_aeneas_translation_chain(
    chain: &AeneasTranslationChain,
    loaded: &[LoadedAeneasArtifact],
) -> AeneasTranslationChainReport {
    let mut issues = Vec::new();
    validate_header(chain, &mut issues);
    let nodes_by_role = validate_nodes(chain, &mut issues);
    let loaded_by_artifact = validate_loaded_artifacts(chain, loaded, &mut issues);
    validate_charon_contract(chain, &nodes_by_role, &loaded_by_artifact, &mut issues);
    validate_translation_manifest(chain, &nodes_by_role, &loaded_by_artifact, &mut issues);
    validate_graph(chain, &nodes_by_role, &mut issues);
    validate_claim_links(chain, &nodes_by_role, &loaded_by_artifact, &mut issues);
    validate_producer_fixture(chain, &nodes_by_role, &loaded_by_artifact, &mut issues);
    let (property_count, recorded_only_count, boundary_count) = validate_role_summary(chain, &mut issues);
    validate_non_claims(chain, &mut issues);

    let computed_chain_identity = canonical_aeneas_chain_identity(chain, &mut issues);
    if let Some(computed) = &computed_chain_identity
        && chain.declared_chain_identity.value != *computed
    {
        issues.push(AeneasChainIssue::new("declared_chain_identity", "chain_identity_mismatch"));
    }
    if chain.declared_chain_identity.domain != AeneasDigestDomain::CompleteChain {
        issues.push(AeneasChainIssue::new("declared_chain_identity.domain", "digest_role_mismatch"));
    }

    issues.sort();
    issues.dedup();
    let valid = issues.is_empty();
    let handoff = if valid {
        computed_chain_identity.as_ref().map(|identity| build_handoff(chain, &nodes_by_role, identity))
    } else {
        None
    };
    AeneasTranslationChainReport {
        valid,
        computed_chain_identity,
        property_count,
        recorded_only_count,
        boundary_count,
        issues,
        handoff,
        supported_claim: AENEAS_SUPPORTED_CLAIM,
    }
}

fn validate_header(chain: &AeneasTranslationChain, issues: &mut Vec<AeneasChainIssue>) {
    if chain.schema != AENEAS_CHAIN_SCHEMA {
        issues.push(AeneasChainIssue::new("schema", "unsupported_schema"));
    }
    if chain.profile != AENEAS_PROFILE {
        issues.push(AeneasChainIssue::new("profile", "unsupported_profile"));
    }
    if chain.producer_fixture.octet_revision.len() != 40
        || !chain.producer_fixture.octet_revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        issues.push(AeneasChainIssue::new("producer_fixture.octet_revision", "invalid_producer_revision"));
    }
    validate_b3(&chain.producer_fixture.blake3, "producer_fixture.blake3", issues);
    if chain.producer_fixture.schema != OCTET_ADAPTER_SCHEMA {
        issues.push(AeneasChainIssue::new("producer_fixture.schema", "producer_schema_mismatch"));
    }
}

fn validate_nodes<'a>(
    chain: &'a AeneasTranslationChain,
    issues: &mut Vec<AeneasChainIssue>,
) -> BTreeMap<AeneasNodeRole, &'a AeneasChainNode> {
    let mut ids = BTreeSet::new();
    let mut artifact_locators = BTreeSet::new();
    let mut roles = BTreeMap::new();
    if chain.nodes.len() != EXPECTED_NODE_COUNT {
        issues.push(AeneasChainIssue::new("nodes", "node_count_mismatch"));
    }
    for node in &chain.nodes {
        let field = format!("nodes.{}", node.stable_id);
        if node.stable_id.is_empty() {
            issues.push(AeneasChainIssue::new(&field, "stable_id_missing"));
        }
        if !ids.insert(node.stable_id.clone()) {
            issues.push(AeneasChainIssue::new(&field, "duplicate_stable_id"));
        }
        if roles.insert(node.role, node).is_some() {
            issues.push(AeneasChainIssue::new(format!("nodes.{:?}", node.role), "duplicate_stage_role"));
        }
        validate_b3(&node.digest.value, format!("{field}.digest"), issues);
        if node.digest.domain != digest_domain_for_role(node.role) {
            issues.push(AeneasChainIssue::new(format!("{field}.digest.domain"), "digest_role_mismatch"));
        }
        let (producer, verification_role, status) = expected_node_policy(node.role);
        if node.producer != producer {
            issues.push(AeneasChainIssue::new(format!("{field}.producer"), "producer_mismatch"));
        }
        if node.verification_role != verification_role {
            issues.push(AeneasChainIssue::new(format!("{field}.verification_role"), "role_promotion"));
        }
        if node.status != status {
            issues.push(AeneasChainIssue::new(format!("{field}.status"), "stage_status_mismatch"));
        }
        if node.artifact.is_empty()
            || node.artifact.starts_with('/')
            || node.artifact.split('/').any(|part| part == "..")
        {
            issues.push(AeneasChainIssue::new(format!("{field}.artifact"), "artifact_locator_unconfined"));
        }
        if !artifact_locators.insert(node.artifact.as_str()) {
            issues.push(AeneasChainIssue::new(format!("{field}.artifact"), "artifact_role_alias"));
        }
        let needs_claim = CLAIM_BINDING_ROLES.contains(&node.role);
        if needs_claim && node.claim.as_ref() != Some(&chain.claim) {
            issues.push(AeneasChainIssue::new(format!("{field}.claim"), "theorem_link_mismatch"));
        }
        if !needs_claim && node.claim.is_some() {
            issues.push(AeneasChainIssue::new(format!("{field}.claim"), "unexpected_theorem_binding"));
        }
    }
    for role in REQUIRED_ROLES {
        if !roles.contains_key(role) {
            issues.push(AeneasChainIssue::new(format!("nodes.{role:?}"), "required_stage_missing"));
        }
    }
    roles
}

fn validate_loaded_artifacts<'a>(
    chain: &AeneasTranslationChain,
    loaded: &'a [LoadedAeneasArtifact],
    issues: &mut Vec<AeneasChainIssue>,
) -> ObservedArtifactMap<'a> {
    let mut loaded_by_artifact = BTreeMap::new();
    for artifact in loaded {
        if loaded_by_artifact.insert(artifact.artifact.as_str(), artifact).is_some() {
            issues.push(AeneasChainIssue::new(format!("artifacts.{}", artifact.artifact), "duplicate_loaded_artifact"));
        }
    }
    let node_digests = chain
        .nodes
        .iter()
        .map(|node| (node.artifact.as_str(), node.digest.value.as_str()))
        .collect::<BTreeMap<_, _>>();
    for locator in aeneas_translation_chain_artifacts(chain) {
        let Some(observed) = loaded_by_artifact.get(locator.as_str()) else {
            issues.push(AeneasChainIssue::new(format!("artifacts.{locator}"), "artifact_missing"));
            continue;
        };
        let expected = if locator == chain.producer_fixture.artifact {
            chain.producer_fixture.blake3.as_str()
        } else {
            node_digests.get(locator.as_str()).copied().unwrap_or_default()
        };
        if observed.measured_blake3 != expected {
            issues.push(AeneasChainIssue::new(format!("artifacts.{locator}"), "artifact_digest_mismatch"));
        }
    }
    loaded_by_artifact
}

fn parse_json_artifact<T>(loaded: &ObservedArtifactMap<'_>, artifact: &str) -> Result<T, ()>
where T: DeserializeOwned {
    let value = loaded.get(artifact).and_then(|observed| observed.parsed_json.as_ref()).ok_or(())?;
    serde_json::from_value(value.clone()).map_err(|_| ())
}

fn validate_charon_contract(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let contract = &chain.charon_contract;
    if contract.has_errors {
        issues.push(AeneasChainIssue::new("charon_contract.has_errors", "errorful_extraction"));
    }
    if contract.skip_borrowck {
        issues.push(AeneasChainIssue::new("charon_contract.skip_borrowck", "borrow_check_skipped"));
    }
    if contract.target_triple != REVIEWED_TARGET_TRIPLE
        || contract.pointer_width_bits != REVIEWED_POINTER_WIDTH_BITS
        || contract.endianness != REVIEWED_ENDIANNESS
    {
        issues.push(AeneasChainIssue::new("charon_contract.target", "target_metadata_mismatch"));
    }
    if contract.mir_start_stage != REVIEWED_MIR_START_STAGE
        || contract.drop_policy != REVIEWED_DROP_POLICY
        || contract.start_from != REVIEWED_START_FROM
        || contract.include_rules != [REVIEWED_INCLUDE_RULE]
        || !contract.exclude_rules.is_empty()
        || !contract.opaque_rules.is_empty()
        || contract.transformation_settings != [REVIEWED_TRANSFORMATION]
        || contract.serialization_format != REVIEWED_SERIALIZATION_FORMAT
    {
        issues.push(AeneasChainIssue::new("charon_contract.options", "unreviewed_extraction_setting"));
    }
    if contract.extraction_profile_identity.domain != AeneasDigestDomain::ProviderProfile {
        issues
            .push(AeneasChainIssue::new("charon_contract.extraction_profile_identity.domain", "digest_role_mismatch"));
    }
    if let Some(profile) = nodes.get(&AeneasNodeRole::ProviderProfile)
        && contract.extraction_profile_identity.value != profile.digest.value
    {
        issues
            .push(AeneasChainIssue::new("charon_contract.extraction_profile_identity", "extraction_profile_mismatch"));
    }

    let Some(contract_node) = nodes.get(&AeneasNodeRole::CharonContract) else {
        return;
    };
    match parse_json_artifact::<ObservedCharonContract>(loaded, &contract_node.artifact) {
        Ok(observed) if observed.matches(contract) => {}
        Ok(_) => issues.push(AeneasChainIssue::new("charon_contract", "observed_contract_mismatch")),
        Err(_) => issues.push(AeneasChainIssue::new("charon_contract", "artifact_parse_failed")),
    }

    let Some(profile_node) = nodes.get(&AeneasNodeRole::ProviderProfile) else {
        return;
    };
    match parse_json_artifact::<ObservedProviderProfile>(loaded, &profile_node.artifact) {
        Ok(profile)
            if profile.schema_version == OCTET_PROVIDER_PROFILE_SCHEMA
                && profile.provider_id == OCTET_PROVIDER
                && profile.profile_id == contract.reviewed_profile_id
                && profile.extraction_policy.matches(contract) => {}
        Ok(_) => issues.push(AeneasChainIssue::new("provider_profile", "reviewed_profile_mismatch")),
        Err(_) => issues.push(AeneasChainIssue::new("provider_profile", "artifact_parse_failed")),
    }
}

fn validate_translation_manifest(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let Some(manifest_node) = nodes.get(&AeneasNodeRole::TranslationManifest) else {
        return;
    };
    if chain.translation_metadata.manifest_run_identity != manifest_node.digest.value {
        issues.push(AeneasChainIssue::new("translation_metadata.manifest_run_identity", "cross_run_def_id"));
    }
    let mut declaration_ids = BTreeSet::new();
    for declaration in &chain.translation_metadata.declarations {
        if declaration.run_identity != chain.translation_metadata.manifest_run_identity {
            issues.push(AeneasChainIssue::new(
                format!("translation_metadata.declarations.{}", declaration.def_id),
                "cross_run_def_id",
            ));
        }
        validate_b3(
            &declaration.declaration_identity,
            format!("translation_metadata.declarations.{}.declaration_identity", declaration.def_id),
            issues,
        );
        if !declaration_ids.insert(declaration.declaration_identity.clone()) {
            issues.push(AeneasChainIssue::new("translation_metadata.declarations", "duplicate_declaration_identity"));
        }
        let expected_identity = declaration_identity(declaration);
        if declaration.declaration_identity != expected_identity {
            issues.push(AeneasChainIssue::new(
                format!("translation_metadata.declarations.{}.declaration_identity", declaration.def_id),
                "declaration_identity_mismatch",
            ));
        }
        let expected_loop_parent = declaration.loop_parent_def_id.and_then(|parent_def_id| {
            chain
                .translation_metadata
                .declarations
                .iter()
                .find(|candidate| candidate.def_id == parent_def_id)
                .map(|candidate| candidate.declaration_identity.as_str())
        });
        if declaration.loop_parent_declaration_identity.as_deref() != expected_loop_parent {
            issues.push(AeneasChainIssue::new(
                format!("translation_metadata.declarations.{}.loop_parent", declaration.def_id),
                "loop_parent_identity_mismatch",
            ));
        }
        let forbidden_stable_id = format!("def-id:{}", declaration.def_id);
        if chain.nodes.iter().any(|node| node.stable_id == forbidden_stable_id) {
            issues.push(AeneasChainIssue::new("nodes.stable_id", "run_local_def_id_used_as_stable_identity"));
        }
    }
    if chain.translation_metadata.declarations.is_empty() {
        issues.push(AeneasChainIssue::new("translation_metadata.declarations", "manifest_declarations_missing"));
    }
    match parse_json_artifact::<ObservedTranslationManifest>(loaded, &manifest_node.artifact) {
        Ok(observed) if observed.matches(&chain.translation_metadata) => {}
        Ok(_) => issues.push(AeneasChainIssue::new("translation_metadata", "observed_manifest_mismatch")),
        Err(_) => issues.push(AeneasChainIssue::new("translation_metadata", "artifact_parse_failed")),
    }
}

fn validate_graph(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let node_ids = chain.nodes.iter().map(|node| node.stable_id.as_str()).collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for edge in &chain.edges {
        if !seen.insert(edge.clone()) {
            issues.push(AeneasChainIssue::new("edges", "duplicate_edge"));
        }
        if !node_ids.contains(edge.from.as_str()) || !node_ids.contains(edge.to.as_str()) {
            issues.push(AeneasChainIssue::new("edges", "dangling_edge"));
            continue;
        }
        let from_role = chain.nodes.iter().find(|node| node.stable_id == edge.from).map(|node| node.role);
        let to_role = chain.nodes.iter().find(|node| node.stable_id == edge.to).map(|node| node.role);
        if let (Some(from_role), Some(to_role)) = (from_role, to_role)
            && !REQUIRED_EDGES.contains(&(from_role, to_role, edge.kind))
        {
            issues.push(AeneasChainIssue::new("edges", "role_incompatible_edge"));
        }
    }
    for (from_role, to_role, kind) in REQUIRED_EDGES {
        let (Some(from), Some(to)) = (nodes.get(from_role), nodes.get(to_role)) else {
            continue;
        };
        if !chain
            .edges
            .iter()
            .any(|edge| edge.from == from.stable_id && edge.to == to.stable_id && edge.kind == *kind)
        {
            issues.push(AeneasChainIssue::new(format!("edges.{from_role:?}.{to_role:?}"), "required_edge_missing"));
        }
    }
    if graph_has_cycle(&chain.nodes, &chain.edges) {
        issues.push(AeneasChainIssue::new("edges", "cycle_detected"));
    }
}

fn validate_claim_links(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    validate_b3(&chain.claim.approved_statement_blake3, "claim.approved_statement_blake3", issues);
    if chain.claim.adequacy_decision.domain != AeneasDigestDomain::AdequacyDecision
        || chain.claim.trust_closure.domain != AeneasDigestDomain::TrustClosure
    {
        issues.push(AeneasChainIssue::new("claim", "digest_role_mismatch"));
    }
    compare_claim_digest(
        chain,
        nodes,
        AeneasNodeRole::MaintainedTheorem,
        &chain.claim.approved_statement_blake3,
        issues,
    );
    compare_claim_digest(chain, nodes, AeneasNodeRole::AdequacyDecision, &chain.claim.adequacy_decision.value, issues);
    compare_claim_digest(chain, nodes, AeneasNodeRole::TrustClosure, &chain.claim.trust_closure.value, issues);
    if chain.claim.adequacy_policy_id != chain.claim.requirement_id {
        issues.push(AeneasChainIssue::new("claim.adequacy_policy_id", "adequacy_policy_mismatch"));
    }
    if !chain.octet_artifact_verified {
        issues.push(AeneasChainIssue::new("octet_artifact_verified", "octet_verification_missing"));
    }

    validate_observed_theorem_source(chain, nodes, loaded, issues);
    validate_observed_adequacy(chain, nodes, loaded, issues);
    validate_observed_model_trust_agreement(nodes, loaded, issues);
    validate_observed_checker(chain, nodes, loaded, issues);
    validate_observed_receipt(chain, nodes, loaded, issues);
    validate_observed_proof_card(chain, nodes, loaded, issues);
    validate_observed_provenance(chain, nodes, loaded, issues);
}

fn validate_producer_fixture(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let adapter = match parse_json_artifact::<OctetAdapterFixture>(loaded, &chain.producer_fixture.artifact) {
        Ok(adapter) => adapter,
        Err(_) => {
            issues.push(AeneasChainIssue::new("producer_fixture", "artifact_parse_failed"));
            return;
        }
    };
    if adapter.schema_version != OCTET_ADAPTER_SCHEMA
        || adapter.provider_id != OCTET_PROVIDER
        || adapter.stages.len() != EXPECTED_ADAPTER_STAGE_COUNT
        || adapter.octet_export.artifact_verification_status != VALID_STATUS
        || !adapter.octet_export.exported_after_verification
        || adapter.octet_export.proof_receipt_status != PASSED_STATUS
        || adapter.property_candidate.owner != OCTET_OWNER
        || adapter.property_candidate.evidence_tier != FORMAL_PROOF_TIER
        || adapter.property_candidate.theorem_id != chain.claim.theorem_id
        || adapter.property_candidate.requirement_id != chain.claim.requirement_id
    {
        issues.push(AeneasChainIssue::new("producer_fixture", "producer_contract_mismatch"));
    }
    let mappings = [
        ("rust_source", AeneasNodeRole::RustSource, "recorded_only"),
        ("charon_extraction", AeneasNodeRole::CharonContract, "boundary"),
        ("aeneas_translation", AeneasNodeRole::TranslationManifest, "recorded_only"),
        ("generated_lean", AeneasNodeRole::GeneratedLean, "recorded_only"),
        ("theorem", AeneasNodeRole::MaintainedTheorem, "boundary"),
        ("lean_check", AeneasNodeRole::OctetReceipt, "property_candidate"),
    ];
    for (stage_kind, role, evidence_role) in mappings {
        let Some(node) = nodes.get(&role) else {
            continue;
        };
        let matching_count = adapter
            .stages
            .iter()
            .filter(|stage| {
                stage.stage_kind == stage_kind
                    && stage.blake3 == node.digest.value
                    && stage.evidence_role == evidence_role
            })
            .count();
        if matching_count != 1 {
            issues.push(AeneasChainIssue::new(
                format!("producer_fixture.stages.{stage_kind}"),
                "producer_stage_mismatch",
            ));
        }
    }
}

fn validate_role_summary(chain: &AeneasTranslationChain, issues: &mut Vec<AeneasChainIssue>) -> (u32, u32, u32) {
    let property_count = count_role(&chain.nodes, AeneasVerificationRole::Property);
    let recorded_only_count = count_role(&chain.nodes, AeneasVerificationRole::RecordedOnly);
    let boundary_count = count_role(&chain.nodes, AeneasVerificationRole::Boundary);
    if property_count != EXPECTED_PROPERTY_COUNT
        || recorded_only_count != EXPECTED_RECORDED_ONLY_COUNT
        || boundary_count != EXPECTED_BOUNDARY_COUNT
    {
        issues.push(AeneasChainIssue::new("role_summary", "provider_role_count_mismatch"));
    }
    if chain.role_summary.property_count != property_count
        || chain.role_summary.recorded_only_count != recorded_only_count
        || chain.role_summary.boundary_count != boundary_count
        || chain.role_summary.verified_count != property_count
    {
        issues.push(AeneasChainIssue::new("role_summary", "declared_role_count_mismatch"));
    }
    (property_count, recorded_only_count, boundary_count)
}

fn validate_non_claims(chain: &AeneasTranslationChain, issues: &mut Vec<AeneasChainIssue>) {
    let declared = chain.non_claims.iter().copied().collect::<BTreeSet<_>>();
    let required = REQUIRED_NON_CLAIMS.iter().copied().collect::<BTreeSet<_>>();
    if declared != required {
        issues.push(AeneasChainIssue::new("non_claims", "non_claim_boundary_weakened"));
    }
}

fn canonical_aeneas_chain_identity(
    chain: &AeneasTranslationChain,
    issues: &mut Vec<AeneasChainIssue>,
) -> Option<String> {
    let mut nodes = chain.nodes.iter().map(CanonicalNode::from).collect::<Vec<_>>();
    nodes.sort_by(|left, right| (left.role, left.stable_id.as_str()).cmp(&(right.role, right.stable_id.as_str())));
    let mut edges = chain.edges.clone();
    edges.sort();
    let mut declarations =
        chain.translation_metadata.declarations.iter().map(CanonicalDeclaration::from).collect::<Vec<_>>();
    declarations.sort_by(|left, right| left.declaration_identity.cmp(&right.declaration_identity));
    let mut non_claims = chain.non_claims.clone();
    non_claims.sort();
    let material = CanonicalChainMaterial {
        schema: &chain.schema,
        profile: &chain.profile,
        producer_octet_revision: &chain.producer_fixture.octet_revision,
        producer_schema: &chain.producer_fixture.schema,
        producer_fixture_blake3: &chain.producer_fixture.blake3,
        nodes,
        edges,
        charon_contract: &chain.charon_contract,
        translation_aeneas_version: &chain.translation_metadata.aeneas_version,
        translation_charon_version: &chain.translation_metadata.charon_version,
        translation_crate_name: &chain.translation_metadata.crate_name,
        translation_type_count: chain.translation_metadata.type_count,
        translation_global_count: chain.translation_metadata.global_count,
        translation_trait_declaration_count: chain.translation_metadata.trait_declaration_count,
        translation_trait_implementation_count: chain.translation_metadata.trait_implementation_count,
        declarations,
        claim: &chain.claim,
        octet_artifact_verified: chain.octet_artifact_verified,
        role_summary: &chain.role_summary,
        non_claims,
    };
    let value = match serde_json::to_value(material) {
        Ok(value) => value,
        Err(_) => {
            issues.push(AeneasChainIssue::new("declared_chain_identity", "canonical_serialization_failed"));
            return None;
        }
    };
    match canonical_json_bytes(&value) {
        Ok(bytes) => Some(format!("b3:{}", blake3_hex(&bytes))),
        Err(_) => {
            issues.push(AeneasChainIssue::new("declared_chain_identity", "canonical_serialization_failed"));
            None
        }
    }
}

fn build_handoff(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    identity: &str,
) -> AeneasChainHandoff {
    AeneasChainHandoff {
        chain_identity: identity.to_owned(),
        extraction_profile_identity: chain.charon_contract.extraction_profile_identity.value.clone(),
        translation_manifest_identity: node_digest(nodes, AeneasNodeRole::TranslationManifest),
        adequacy_decision_identity: chain.claim.adequacy_decision.value.clone(),
        octet_receipt_identity: node_digest(nodes, AeneasNodeRole::OctetReceipt),
        theorem_id: chain.claim.theorem_id.clone(),
        property_count: chain.role_summary.property_count,
        recorded_only_count: chain.role_summary.recorded_only_count,
        boundary_count: chain.role_summary.boundary_count,
        non_claims: chain.non_claims.clone(),
    }
}

fn graph_has_cycle(nodes: &[AeneasChainNode], edges: &[AeneasChainEdge]) -> bool {
    let mut indegree = nodes.iter().map(|node| (node.stable_id.as_str(), 0_usize)).collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&str>>::new();
    for edge in edges {
        if indegree.contains_key(edge.from.as_str()) && indegree.contains_key(edge.to.as_str()) {
            if let Some(count) = indegree.get_mut(edge.to.as_str()) {
                *count = count.saturating_add(1);
            }
            outgoing.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
        }
    }
    let mut ready = indegree.iter().filter_map(|(node, count)| (*count == 0).then_some(*node)).collect::<Vec<_>>();
    let mut visited = 0_usize;
    while let Some(node) = ready.pop() {
        visited = visited.saturating_add(1);
        if let Some(targets) = outgoing.get(node) {
            for target in targets {
                if let Some(count) = indegree.get_mut(target) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        ready.push(target);
                    }
                }
            }
        }
    }
    visited != nodes.len()
}

fn compare_claim_digest(
    _chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    role: AeneasNodeRole,
    expected: &str,
    issues: &mut Vec<AeneasChainIssue>,
) {
    if let Some(node) = nodes.get(&role)
        && node.digest.value != expected
    {
        issues.push(AeneasChainIssue::new(format!("nodes.{role:?}.digest"), "claim_artifact_mismatch"));
    }
}

fn validate_observed_theorem_source(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    _loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    if let Some(theorem) = nodes.get(&AeneasNodeRole::MaintainedTheorem)
        && theorem.digest.value != chain.claim.approved_statement_blake3
    {
        issues.push(AeneasChainIssue::new("claim.approved_statement_blake3", "theorem_approval_mismatch"));
    }
}

fn validate_observed_adequacy(
    _chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let Some(node) = nodes.get(&AeneasNodeRole::AdequacyDecision) else {
        return;
    };
    match parse_json_artifact::<ObservedAdequacy>(loaded, &node.artifact) {
        Ok(adequacy) if adequacy.decision == NON_VACUOUS_DECISION && adequacy.forbidden_declarations.is_empty() => {}
        Ok(_) => issues.push(AeneasChainIssue::new("adequacy_decision", "adequacy_decision_mismatch")),
        Err(_) => issues.push(AeneasChainIssue::new("adequacy_decision", "artifact_parse_failed")),
    }
}

fn validate_observed_model_trust_agreement(
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let (Some(model_node), Some(trust_node)) =
        (nodes.get(&AeneasNodeRole::ExternalModel), nodes.get(&AeneasNodeRole::TrustClosure))
    else {
        return;
    };
    let models = parse_json_artifact::<Vec<ObservedExternalInput>>(loaded, &model_node.artifact);
    let closure = parse_json_artifact::<ObservedTrustClosure>(loaded, &trust_node.artifact);
    match (models, closure) {
        (Ok(models), Ok(closure)) if closure.is_complete() && closure.matches_inputs(&models) => {}
        (Ok(_), Ok(closure)) if !closure.is_complete() => {
            issues.push(AeneasChainIssue::new("trust_closure", "trust_closure_incomplete"));
        }
        (Ok(_), Ok(_)) => issues.push(AeneasChainIssue::new("trust_closure", "trust_input_mismatch")),
        _ => issues.push(AeneasChainIssue::new("trust_closure", "artifact_parse_failed")),
    }
}

fn validate_observed_checker(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let Some(node) = nodes.get(&AeneasNodeRole::LeanChecker) else {
        return;
    };
    match parse_json_artifact::<ObservedChecker>(loaded, &node.artifact) {
        Ok(checker) if checker.theorem_id == chain.claim.theorem_id && checker.status == PASSED_STATUS => {}
        Ok(_) => issues.push(AeneasChainIssue::new("lean_checker", "checker_theorem_mismatch")),
        Err(_) => issues.push(AeneasChainIssue::new("lean_checker", "artifact_parse_failed")),
    }
}

fn validate_observed_receipt(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let Some(node) = nodes.get(&AeneasNodeRole::OctetReceipt) else {
        return;
    };
    let receipt = match parse_json_artifact::<ObservedReceipt>(loaded, &node.artifact) {
        Ok(receipt) => receipt,
        Err(_) => {
            issues.push(AeneasChainIssue::new("octet_receipt", "artifact_parse_failed"));
            return;
        }
    };
    if receipt.schema_version != OCTET_RECEIPT_SCHEMA
        || receipt.provider_id != OCTET_PROVIDER
        || receipt.status != PASSED_STATUS
        || receipt.evidence_tier != FORMAL_PROOF_TIER
        || !receipt.claim.matches(&chain.claim)
        || receipt.provider.kind != OCTET_PROVIDER
        || !receipt.provider.payload.theorem_admission.matches(&chain.claim)
        || !receipt.provider.payload.extraction_policy.matches(&chain.charon_contract)
    {
        issues.push(AeneasChainIssue::new("octet_receipt", "receipt_claim_mismatch"));
    }
    if let Some(package_lock) = nodes.get(&AeneasNodeRole::PackageLock)
        && normalize_b3(&package_lock.digest.value) != receipt.package_lock_identity
    {
        issues.push(AeneasChainIssue::new("octet_receipt.package_lock_identity", "receipt_stage_mismatch"));
    }
    for (role, suffix) in receipt_role_suffixes() {
        let Some(expected) = nodes.get(&role) else {
            continue;
        };
        let observed = receipt
            .artifact_identities
            .iter()
            .chain(receipt.source_bindings.iter())
            .find(|identity| identity.locator.ends_with(suffix));
        if observed.is_none_or(|identity| identity.blake3 != normalize_b3(&expected.digest.value)) {
            issues.push(AeneasChainIssue::new(format!("octet_receipt.artifacts.{role:?}"), "receipt_stage_mismatch"));
        }
    }
}

fn validate_observed_proof_card(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let Some(node) = nodes.get(&AeneasNodeRole::ProofCard) else {
        return;
    };
    let cards = match parse_json_artifact::<ObservedProofCards>(loaded, &node.artifact) {
        Ok(cards) => cards,
        Err(_) => {
            issues.push(AeneasChainIssue::new("proof_card", "artifact_parse_failed"));
            return;
        }
    };
    let aeneas_card_count = cards
        .cards
        .iter()
        .filter(|card| card.formal_proof_envelope.as_ref().is_some_and(|receipt| receipt.provider_id == OCTET_PROVIDER))
        .count();
    let matching_count = cards
        .cards
        .iter()
        .filter(|card| {
            card.proof_or_property_id == chain.claim.theorem_id
                && card.source_requirement_or_policy_id == chain.claim.requirement_id
                && card.formal_proof_envelope.as_ref().is_some_and(|receipt| {
                    receipt.schema_version == OCTET_RECEIPT_SCHEMA
                        && receipt.provider_id == OCTET_PROVIDER
                        && receipt.status == PASSED_STATUS
                        && receipt.claim.matches(&chain.claim)
                        && receipt.provider.payload.theorem_admission.matches(&chain.claim)
                })
        })
        .count();
    if aeneas_card_count != 1 || matching_count != 1 {
        issues.push(AeneasChainIssue::new("proof_card", "proof_card_claim_mismatch"));
    }
}

fn validate_observed_provenance(
    chain: &AeneasTranslationChain,
    nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>,
    loaded: &ObservedArtifactMap<'_>,
    issues: &mut Vec<AeneasChainIssue>,
) {
    let Some(node) = nodes.get(&AeneasNodeRole::ProvenanceProjection) else {
        return;
    };
    let Some(receipt_node) = nodes.get(&AeneasNodeRole::OctetReceipt) else {
        return;
    };
    let provenance = match parse_json_artifact::<ObservedProvenance>(loaded, &node.artifact) {
        Ok(provenance) => provenance,
        Err(_) => {
            issues.push(AeneasChainIssue::new("provenance_projection", "artifact_parse_failed"));
            return;
        }
    };
    let receipt_records = provenance
        .provenance_records
        .iter()
        .filter(|record| record.artifact.ends_with(".receipt"))
        .collect::<Vec<_>>();
    let matching_count = receipt_records
        .iter()
        .filter(|record| {
            record.blake3 == normalize_b3(&receipt_node.digest.value)
                && record.receipt.status == VALID_STATUS
                && record.proof_artifact_identity.as_deref() == Some(record.artifact.as_str())
        })
        .count();
    if provenance.schema_version != OCTET_PROVENANCE_SCHEMA
        || provenance.verification_status != VALID_STATUS
        || receipt_records.len() != 1
        || matching_count != 1
    {
        issues.push(AeneasChainIssue::new("provenance_projection", "provenance_receipt_mismatch"));
    }
    let _ = chain;
}

fn declaration_identity(declaration: &AeneasManifestDeclaration) -> String {
    let material = DeclarationIdentityMaterial::from(declaration);
    let value = serde_json::to_value(material).expect("declaration identity material serializes");
    let bytes = canonical_json_bytes(&value).expect("declaration identity JSON is supported");
    format!("b3:{}", blake3_hex(&bytes))
}

fn count_role(nodes: &[AeneasChainNode], role: AeneasVerificationRole) -> u32 {
    nodes
        .iter()
        .filter(|node| node.verification_role == role)
        .fold(u32::default(), |count, _| count.saturating_add(1))
}

fn validate_b3(value: &str, field: impl Into<String>, issues: &mut Vec<AeneasChainIssue>) {
    let valid = value.strip_prefix("b3:").is_some_and(|hex| {
        hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if !valid {
        issues.push(AeneasChainIssue::new(field, "malformed_blake3"));
    }
}

fn normalize_b3(value: &str) -> &str {
    value.strip_prefix("b3:").unwrap_or(value)
}

fn node_digest(nodes: &BTreeMap<AeneasNodeRole, &AeneasChainNode>, role: AeneasNodeRole) -> String {
    nodes.get(&role).map_or_else(String::new, |node| node.digest.value.clone())
}

const fn digest_domain_for_role(role: AeneasNodeRole) -> AeneasDigestDomain {
    match role {
        AeneasNodeRole::ProviderProfile => AeneasDigestDomain::ProviderProfile,
        AeneasNodeRole::RustSource => AeneasDigestDomain::RustSource,
        AeneasNodeRole::CharonContract => AeneasDigestDomain::ExtractionProfile,
        AeneasNodeRole::CharonExtraction => AeneasDigestDomain::ExtractionObservation,
        AeneasNodeRole::Llbc => AeneasDigestDomain::Llbc,
        AeneasNodeRole::AeneasTranslation => AeneasDigestDomain::TranslationObservation,
        AeneasNodeRole::TranslationManifest => AeneasDigestDomain::TranslationManifest,
        AeneasNodeRole::GeneratedLean => AeneasDigestDomain::GeneratedModel,
        AeneasNodeRole::ExternalModel => AeneasDigestDomain::ExternalModel,
        AeneasNodeRole::MaintainedTheorem => AeneasDigestDomain::Theorem,
        AeneasNodeRole::AdequacyDecision => AeneasDigestDomain::AdequacyDecision,
        AeneasNodeRole::TrustClosure => AeneasDigestDomain::TrustClosure,
        AeneasNodeRole::PackageLock => AeneasDigestDomain::PackageLock,
        AeneasNodeRole::LeanChecker => AeneasDigestDomain::CheckerReceipt,
        AeneasNodeRole::OctetReceipt => AeneasDigestDomain::OctetReceipt,
        AeneasNodeRole::ProofCard => AeneasDigestDomain::ProofCard,
        AeneasNodeRole::ProvenanceProjection => AeneasDigestDomain::ProvenanceProjection,
    }
}

const fn expected_node_policy(role: AeneasNodeRole) -> (AeneasProducer, AeneasVerificationRole, AeneasNodeStatus) {
    match role {
        AeneasNodeRole::ProviderProfile => {
            (AeneasProducer::Octet, AeneasVerificationRole::Boundary, AeneasNodeStatus::Reviewed)
        }
        AeneasNodeRole::RustSource => {
            (AeneasProducer::Octet, AeneasVerificationRole::RecordedOnly, AeneasNodeStatus::Recorded)
        }
        AeneasNodeRole::CharonContract => {
            (AeneasProducer::Octet, AeneasVerificationRole::Boundary, AeneasNodeStatus::Reviewed)
        }
        AeneasNodeRole::CharonExtraction => {
            (AeneasProducer::Charon, AeneasVerificationRole::RecordedOnly, AeneasNodeStatus::Passed)
        }
        AeneasNodeRole::Llbc => {
            (AeneasProducer::Charon, AeneasVerificationRole::RecordedOnly, AeneasNodeStatus::Recorded)
        }
        AeneasNodeRole::AeneasTranslation => {
            (AeneasProducer::Aeneas, AeneasVerificationRole::RecordedOnly, AeneasNodeStatus::Passed)
        }
        AeneasNodeRole::TranslationManifest | AeneasNodeRole::GeneratedLean => {
            (AeneasProducer::Aeneas, AeneasVerificationRole::RecordedOnly, AeneasNodeStatus::Recorded)
        }
        AeneasNodeRole::ExternalModel | AeneasNodeRole::MaintainedTheorem | AeneasNodeRole::PackageLock => {
            (AeneasProducer::Octet, AeneasVerificationRole::Boundary, AeneasNodeStatus::Reviewed)
        }
        AeneasNodeRole::AdequacyDecision => {
            (AeneasProducer::Octet, AeneasVerificationRole::Boundary, AeneasNodeStatus::Approved)
        }
        AeneasNodeRole::TrustClosure => {
            (AeneasProducer::Octet, AeneasVerificationRole::Boundary, AeneasNodeStatus::Complete)
        }
        AeneasNodeRole::LeanChecker => {
            (AeneasProducer::Lean, AeneasVerificationRole::Boundary, AeneasNodeStatus::Passed)
        }
        AeneasNodeRole::OctetReceipt => {
            (AeneasProducer::Octet, AeneasVerificationRole::Property, AeneasNodeStatus::Passed)
        }
        AeneasNodeRole::ProofCard | AeneasNodeRole::ProvenanceProjection => {
            (AeneasProducer::Octet, AeneasVerificationRole::Boundary, AeneasNodeStatus::Verified)
        }
    }
}

fn receipt_role_suffixes() -> [(AeneasNodeRole, &'static str); 14] {
    [
        (AeneasNodeRole::ProviderProfile, "provider-profile.json"),
        (AeneasNodeRole::RustSource, "source/src/lib.rs"),
        (AeneasNodeRole::CharonContract, "charon-options.json"),
        (AeneasNodeRole::CharonExtraction, "charon-command.json"),
        (AeneasNodeRole::Llbc, "llbc.llbc"),
        (AeneasNodeRole::AeneasTranslation, "aeneas-command.json"),
        (AeneasNodeRole::TranslationManifest, "translation.json"),
        (AeneasNodeRole::GeneratedLean, "Llbc.lean"),
        (AeneasNodeRole::ExternalModel, "external-models.json"),
        (AeneasNodeRole::MaintainedTheorem, "theorem-statement.lean"),
        (AeneasNodeRole::AdequacyDecision, "theorem-adequacy.json"),
        (AeneasNodeRole::TrustClosure, "trust-closure.json"),
        (AeneasNodeRole::PackageLock, "package-lock.json"),
        (AeneasNodeRole::LeanChecker, "checker-result.json"),
    ]
}

#[derive(Serialize)]
struct CanonicalChainMaterial<'a> {
    schema: &'a str,
    profile: &'a str,
    producer_octet_revision: &'a str,
    producer_schema: &'a str,
    producer_fixture_blake3: &'a str,
    nodes: Vec<CanonicalNode>,
    edges: Vec<AeneasChainEdge>,
    charon_contract: &'a AeneasCharonContract,
    translation_aeneas_version: &'a str,
    translation_charon_version: &'a str,
    translation_crate_name: &'a str,
    translation_type_count: u32,
    translation_global_count: u32,
    translation_trait_declaration_count: u32,
    translation_trait_implementation_count: u32,
    declarations: Vec<CanonicalDeclaration>,
    claim: &'a AeneasClaimBinding,
    octet_artifact_verified: bool,
    role_summary: &'a AeneasRoleSummary,
    non_claims: Vec<AeneasNonClaim>,
}

#[derive(Serialize)]
struct CanonicalNode {
    stable_id: String,
    role: AeneasNodeRole,
    digest: AeneasDigest,
    producer: AeneasProducer,
    verification_role: AeneasVerificationRole,
    status: AeneasNodeStatus,
    source_span: Option<AeneasSourceSpan>,
    claim: Option<AeneasClaimBinding>,
}

impl From<&AeneasChainNode> for CanonicalNode {
    fn from(node: &AeneasChainNode) -> Self {
        Self {
            stable_id: node.stable_id.clone(),
            role: node.role,
            digest: node.digest.clone(),
            producer: node.producer,
            verification_role: node.verification_role,
            status: node.status,
            source_span: node.source_span.clone(),
            claim: node.claim.clone(),
        }
    }
}

#[derive(Serialize)]
struct CanonicalDeclaration {
    declaration_identity: String,
    rust_name: String,
    lean_name: String,
    lean_file: String,
    source: AeneasSourceSpan,
    is_opaque: bool,
    can_fail: bool,
    can_diverge: bool,
    is_recursive: bool,
    loop_parent_declaration_identity: Option<String>,
}

impl From<&AeneasManifestDeclaration> for CanonicalDeclaration {
    fn from(declaration: &AeneasManifestDeclaration) -> Self {
        Self {
            declaration_identity: declaration.declaration_identity.clone(),
            rust_name: declaration.rust_name.clone(),
            lean_name: declaration.lean_name.clone(),
            lean_file: declaration.lean_file.clone(),
            source: declaration.source.clone(),
            is_opaque: declaration.is_opaque,
            can_fail: declaration.can_fail,
            can_diverge: declaration.can_diverge,
            is_recursive: declaration.is_recursive,
            loop_parent_declaration_identity: declaration.loop_parent_declaration_identity.clone(),
        }
    }
}

#[derive(Serialize)]
struct DeclarationIdentityMaterial {
    rust_name: String,
    lean_name: String,
    lean_file: String,
    source: AeneasSourceSpan,
    is_opaque: bool,
    can_fail: bool,
    can_diverge: bool,
    is_recursive: bool,
    loop_parent_declaration_identity: Option<String>,
}

impl From<&AeneasManifestDeclaration> for DeclarationIdentityMaterial {
    fn from(declaration: &AeneasManifestDeclaration) -> Self {
        Self {
            rust_name: declaration.rust_name.clone(),
            lean_name: declaration.lean_name.clone(),
            lean_file: declaration.lean_file.clone(),
            source: declaration.source.clone(),
            is_opaque: declaration.is_opaque,
            can_fail: declaration.can_fail,
            can_diverge: declaration.can_diverge,
            is_recursive: declaration.is_recursive,
            loop_parent_declaration_identity: declaration.loop_parent_declaration_identity.clone(),
        }
    }
}

#[derive(Deserialize)]
struct ObservedCharonContract {
    has_errors: bool,
    skip_borrowck: bool,
    target_triple: String,
    pointer_width_bits: u16,
    endianness: String,
    mir_start_stage: String,
    drop_policy: String,
    start_from: String,
    include_rules: Vec<String>,
    exclude_rules: Vec<String>,
    opaque_rules: Vec<String>,
    transformation_settings: Vec<String>,
    serialization_format: String,
}

impl ObservedCharonContract {
    fn matches(&self, contract: &AeneasCharonContract) -> bool {
        self.has_errors == contract.has_errors
            && self.skip_borrowck == contract.skip_borrowck
            && self.target_triple == contract.target_triple
            && self.pointer_width_bits == contract.pointer_width_bits
            && self.endianness == contract.endianness
            && self.mir_start_stage == contract.mir_start_stage
            && self.drop_policy == contract.drop_policy
            && self.start_from == contract.start_from
            && self.include_rules == contract.include_rules
            && self.exclude_rules == contract.exclude_rules
            && self.opaque_rules == contract.opaque_rules
            && self.transformation_settings == contract.transformation_settings
            && self.serialization_format == contract.serialization_format
    }
}

#[derive(Deserialize)]
struct ObservedProviderProfile {
    schema_version: String,
    provider_id: String,
    profile_id: String,
    extraction_policy: ObservedCharonContract,
}

#[derive(Deserialize)]
struct ObservedTranslationManifest {
    aeneas_version: String,
    charon_version: String,
    #[serde(rename = "crate")]
    crate_name: String,
    functions: Vec<ObservedTranslationDeclaration>,
    types: Vec<serde_json::Value>,
    globals: Vec<serde_json::Value>,
    trait_decls: Vec<serde_json::Value>,
    trait_impls: Vec<serde_json::Value>,
}

impl ObservedTranslationManifest {
    fn matches(&self, metadata: &AeneasTranslationMetadata) -> bool {
        self.aeneas_version == metadata.aeneas_version
            && self.charon_version == metadata.charon_version
            && self.crate_name == metadata.crate_name
            && self.types.len() == metadata.type_count as usize
            && self.globals.len() == metadata.global_count as usize
            && self.trait_decls.len() == metadata.trait_declaration_count as usize
            && self.trait_impls.len() == metadata.trait_implementation_count as usize
            && self.functions.len() == metadata.declarations.len()
            && self
                .functions
                .iter()
                .all(|observed| metadata.declarations.iter().any(|declared| observed.matches(declared)))
    }
}

#[derive(Deserialize)]
struct ObservedTranslationDeclaration {
    def_id: u64,
    rust_name: String,
    lean_name: String,
    lean_file: String,
    source: ObservedSourceSpan,
    is_opaque: bool,
    can_fail: bool,
    can_diverge: bool,
    is_rec: bool,
    #[serde(default)]
    loop_parent: Option<u64>,
}

impl ObservedTranslationDeclaration {
    fn matches(&self, declared: &AeneasManifestDeclaration) -> bool {
        self.def_id == declared.def_id
            && self.rust_name == declared.rust_name
            && self.lean_name == declared.lean_name
            && self.lean_file == declared.lean_file
            && self.source.file == declared.source.source_id
            && self.source.begin_line == declared.source.begin_line
            && self.source.end_line == declared.source.end_line
            && self.is_opaque == declared.is_opaque
            && self.can_fail == declared.can_fail
            && self.can_diverge == declared.can_diverge
            && self.is_rec == declared.is_recursive
            && self.loop_parent == declared.loop_parent_def_id
    }
}

#[derive(Deserialize)]
struct ObservedSourceSpan {
    file: String,
    begin_line: u32,
    end_line: u32,
}

#[derive(Deserialize)]
struct ObservedAdequacy {
    decision: String,
    forbidden_declarations: Vec<String>,
}

#[derive(Deserialize)]
struct ObservedTrustClosure {
    complete: bool,
    declared_external_models: Vec<String>,
    declared_axioms: Vec<String>,
    declared_opaque_definitions: Vec<String>,
    declared_admitted_declarations: Vec<String>,
    undeclared_inputs: Vec<String>,
}

impl ObservedTrustClosure {
    fn is_complete(&self) -> bool {
        self.complete && self.undeclared_inputs.is_empty()
    }

    fn matches_inputs(&self, inputs: &[ObservedExternalInput]) -> bool {
        let observed = inputs.iter().map(|input| (input.kind.as_str(), input.name.as_str())).collect::<BTreeSet<_>>();
        let mut declared = BTreeSet::new();
        declared.extend(self.declared_external_models.iter().map(|name| ("external_model", name.as_str())));
        declared.extend(self.declared_axioms.iter().map(|name| ("axiom", name.as_str())));
        declared.extend(self.declared_opaque_definitions.iter().map(|name| ("opaque_definition", name.as_str())));
        declared.extend(self.declared_admitted_declarations.iter().map(|name| ("admitted_declaration", name.as_str())));
        observed == declared
    }
}

#[derive(Deserialize)]
struct ObservedExternalInput {
    kind: String,
    name: String,
}

#[derive(Deserialize)]
struct ObservedChecker {
    theorem_id: String,
    status: String,
}

#[derive(Deserialize)]
struct ObservedReceiptClaim {
    theorem_or_property_id: String,
    requirement_id: String,
    claim_class: String,
}

impl ObservedReceiptClaim {
    fn matches(&self, claim: &AeneasClaimBinding) -> bool {
        self.theorem_or_property_id == claim.theorem_id
            && self.requirement_id == claim.requirement_id
            && self.claim_class == claim.claim_class
    }
}

#[derive(Deserialize)]
struct ObservedArtifactIdentity {
    locator: String,
    blake3: String,
}

#[derive(Deserialize)]
struct ObservedTheoremAdmission {
    theorem_id: String,
    requirement_id: String,
    claim_class: String,
    approved_statement_blake3: String,
    statement_approved: bool,
    adequacy: ObservedAdequacy,
    trust_closure: ObservedTrustClosure,
    lean_check: ObservedChecker,
}

impl ObservedTheoremAdmission {
    fn matches(&self, claim: &AeneasClaimBinding) -> bool {
        self.theorem_id == claim.theorem_id
            && self.requirement_id == claim.requirement_id
            && self.claim_class == claim.claim_class
            && self.approved_statement_blake3 == normalize_b3(&claim.approved_statement_blake3)
            && self.statement_approved
            && self.adequacy.decision == NON_VACUOUS_DECISION
            && self.adequacy.forbidden_declarations.is_empty()
            && self.trust_closure.is_complete()
            && self.lean_check.theorem_id == claim.theorem_id
            && self.lean_check.status == PASSED_STATUS
    }
}

#[derive(Deserialize)]
struct ObservedReceiptProviderPayload {
    extraction_policy: ObservedCharonContract,
    theorem_admission: ObservedTheoremAdmission,
}

#[derive(Deserialize)]
struct ObservedReceiptProvider {
    kind: String,
    payload: ObservedReceiptProviderPayload,
}

#[derive(Deserialize)]
struct ObservedReceipt {
    schema_version: String,
    provider_id: String,
    status: String,
    evidence_tier: String,
    claim: ObservedReceiptClaim,
    package_lock_identity: String,
    source_bindings: Vec<ObservedArtifactIdentity>,
    artifact_identities: Vec<ObservedArtifactIdentity>,
    provider: ObservedReceiptProvider,
}

#[derive(Deserialize)]
struct ObservedProofCards {
    cards: Vec<ObservedProofCard>,
}

#[derive(Deserialize)]
struct ObservedProofCard {
    proof_or_property_id: String,
    source_requirement_or_policy_id: String,
    formal_proof_envelope: Option<ObservedReceipt>,
}

#[derive(Deserialize)]
struct ObservedProvenance {
    schema_version: String,
    verification_status: String,
    provenance_records: Vec<ObservedProvenanceRecord>,
}

#[derive(Deserialize)]
struct ObservedProvenanceRecord {
    artifact: String,
    blake3: String,
    receipt: ObservedProvenanceReceipt,
    proof_artifact_identity: Option<String>,
}

#[derive(Deserialize)]
struct ObservedProvenanceReceipt {
    status: String,
}

#[derive(Deserialize)]
struct OctetAdapterFixture {
    schema_version: String,
    provider_id: String,
    octet_export: OctetAdapterExport,
    property_candidate: OctetAdapterProperty,
    stages: Vec<OctetAdapterStage>,
}

#[derive(Deserialize)]
struct OctetAdapterExport {
    artifact_verification_status: String,
    proof_receipt_status: String,
    exported_after_verification: bool,
}

#[derive(Deserialize)]
struct OctetAdapterProperty {
    theorem_id: String,
    requirement_id: String,
    evidence_tier: String,
    owner: String,
}

#[derive(Deserialize)]
struct OctetAdapterStage {
    stage_kind: String,
    blake3: String,
    evidence_role: String,
}

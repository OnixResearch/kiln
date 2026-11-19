//! Advisory, bounded derivations over native-admitted conclusions.
//!
//! Claims bind an exact native requirement revision, interpretation, subject,
//! environment and bounds. Native producers are re-admitted through their
//! existing validators; no serialized admission is trusted. Derivations replay
//! a closed acyclic rule vocabulary and explain support, counterevidence,
//! unresolved premises and residual trust assumptions.
//!
//! This module is deliberately not an authority: support is not an accepted
//! exception, an action authorization, readiness, or behavioral requirement
//! satisfaction. `cairn.scoped_claims.authority` owns that boundary.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::content_bound_requirement_registry::ContentBoundRequirementRow;
use super::typed_requirement_contracts::{
    admit_correspondence_review, normalize_contract, CorrespondenceReview, SemanticContract,
    SemanticError, SemanticProfile,
};

pub const CLAIM_SCHEMA_V1: &str = "cairn.scoped-claim.v1";
pub const CLAIM_POLICY_SCHEMA_V1: &str = "cairn.scoped-claim-policy.v1";
pub const DERIVATION_REPORT_SCHEMA_V1: &str = "cairn.scoped-derivation-report.v1";
pub const RULE_VERSION_V1: u32 = 1;
// r[impl cairn.scoped_claims.authority]
pub const INDEPENDENCE_NOT_ESTABLISHED: &str = "distinct actors, artifact counts and separate derivation paths do not establish statistical, oracle or implementation independence";
pub const DERIVATION_BOUNDARY: &str = "Advisory explanation only: claim support is not an accepted exception, an action authorization, readiness, or behavioral requirement satisfaction. Native applicability validators, verification roles, stage validators, readiness states and the Pass | Block gate retain authority. Observation provenance, native execution honesty, test-oracle adequacy, interpreter and verifier soundness, reviewed-correspondence fidelity, and native requirement freshness remain explicit trust dependencies.";

const MAX_CLAIMS_CEILING: usize = 1024;
const MAX_DERIVATIONS_CEILING: usize = 2048;
const MAX_PRODUCERS_CEILING: usize = 1024;
const MAX_PREMISES_CEILING: usize = 16;
const MAX_DEPTH_CEILING: usize = 64;

const PROPOSITION_TAGS: [&str; 5] = [
    "check_passed",
    "receipt_structurally_valid",
    "native_completion_eligible",
    "reviewed_correspondence",
    "reviewed_completion_decision",
];

/// Exact scope a claim is asserted in. Every component is mandatory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimContext {
    pub subject_blake3: String,
    pub policy_blake3: String,
    pub target: String,
    pub environment: String,
    pub bounds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Observation,
    StructuralRelationship,
    BoundedProperty,
    CheckerAcceptedProperty,
    ReviewedRequirementCorrespondence,
}

impl ClaimKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::StructuralRelationship => "structural_relationship",
            Self::BoundedProperty => "bounded_property",
            Self::CheckerAcceptedProperty => "checker_accepted_property",
            Self::ReviewedRequirementCorrespondence => "reviewed_requirement_correspondence",
        }
    }
}

/// Closed proposition vocabulary. Each variant is one exact scoped statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Proposition {
    CheckPassed { check_id: String },
    ReceiptStructurallyValid { receipt_blake3: String },
    NativeCompletionEligible,
    ReviewedCorrespondence,
    ReviewedCompletionDecision,
}

impl Proposition {
    #[must_use]
    pub fn claim_kind(&self) -> ClaimKind {
        match self {
            Self::CheckPassed { .. } => ClaimKind::Observation,
            Self::ReceiptStructurallyValid { .. } => ClaimKind::StructuralRelationship,
            Self::NativeCompletionEligible => ClaimKind::CheckerAcceptedProperty,
            Self::ReviewedCorrespondence => ClaimKind::ReviewedRequirementCorrespondence,
            Self::ReviewedCompletionDecision => ClaimKind::BoundedProperty,
        }
    }

    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::CheckPassed { .. } => "check_passed",
            Self::ReceiptStructurallyValid { .. } => "receipt_structurally_valid",
            Self::NativeCompletionEligible => "native_completion_eligible",
            Self::ReviewedCorrespondence => "reviewed_correspondence",
            Self::ReviewedCompletionDecision => "reviewed_completion_decision",
        }
    }
}

/// Versioned claim. `kind` is declared and checked against the proposition so a
/// recorded-only or structural statement cannot masquerade as behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedClaim {
    pub schema: String,
    pub kind: ClaimKind,
    pub requirement: ContentBoundRequirementRow,
    pub interpretation_blake3: String,
    pub contract_blake3: String,
    pub context: ClaimContext,
    pub proposition: Proposition,
}

/// Closed derivation-rule vocabulary. Versions are part of the identity.
///
/// v1 deliberately admits one composition rule. Its premises can never be its
/// conclusion, so admitted graphs are acyclic by construction. The replay
/// cycle check below remains for future rules that add new edge shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    ReviewedCompletionV1,
}

impl Rule {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::ReviewedCompletionV1 => "reviewed_completion_v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Derivation {
    pub id: String,
    pub rule: Rule,
    pub premises: Vec<String>,
    pub conclusion: String,
    pub applicability: ClaimContext,
    pub assurance_scope: String,
    pub residual_assumptions: BTreeSet<String>,
}

/// Caller-supplied native producer payload. The adapter re-runs the native
/// validator; nothing is admitted from these fields directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "producer", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProducerInput {
    AuthoritativeCompletion {
        context: ClaimContext,
        contract: serde_json::Value,
        results: serde_json::Value,
        trust_dependencies: BTreeSet<String>,
    },
    StructuralAstGrep {
        context: ClaimContext,
        repository_scope: String,
        receipt: serde_json::Value,
        trust_dependencies: BTreeSet<String>,
    },
}

/// Admitted policy: Nickel owns the bounds and the enabled vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimPolicy {
    pub schema: String,
    pub claims: ClaimPolicyClaims,
    pub rules: Vec<RulePolicy>,
    pub bounds: EvaluationBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimPolicyClaims {
    pub schema: String,
    pub kinds: Vec<ClaimKind>,
    pub propositions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulePolicy {
    pub id: Rule,
    pub version: u32,
    pub assurance_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBounds {
    pub max_claims: usize,
    pub max_derivations: usize,
    pub max_producers: usize,
    pub max_premises: usize,
    pub max_depth: usize,
}

/// One advisory evaluation. The typed contract supplies the interpretation and
/// the native requirement revision; no second registry is introduced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRequest {
    pub profile: SemanticProfile,
    pub native_requirement: ContentBoundRequirementRow,
    pub semantic_contract: SemanticContract,
    pub correspondence_review: Option<CorrespondenceReview>,
    pub authorized_reviewers: Vec<String>,
    pub claims: Vec<ScopedClaim>,
    pub producers: Vec<ProducerInput>,
    pub derivations: Vec<Derivation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Unresolved,
    Supported,
    Refuted,
    Disputed,
}

/// One exact premise/rule/applicability/assurance/assumption link.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConclusionLink {
    pub source: String,
    pub rule: String,
    pub version: u32,
    pub premises: Vec<String>,
    pub applicability: ClaimContext,
    pub assurance_scope: String,
    pub residual_assumptions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimExplanation {
    pub claim_id: String,
    pub claim: ScopedClaim,
    pub kind: ClaimKind,
    pub state: ClaimState,
    pub state_reason: String,
    pub support: Vec<ConclusionLink>,
    pub counterevidence: Vec<ConclusionLink>,
    pub unresolved_premises: BTreeSet<String>,
    pub residual_assumptions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationReport {
    pub schema: String,
    pub advisory: bool,
    pub claims: Vec<ClaimExplanation>,
    pub unresolved: BTreeSet<String>,
    pub admission_diagnostics: BTreeMap<String, Vec<String>>,
    pub trust_dependencies: BTreeSet<String>,
    pub independence: String,
    pub input_blake3: String,
    pub report_blake3: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum DerivationError {
    Unsupported(String),
    ScopeMismatch(String),
    InvalidInput(String),
    BoundExhaustion(String),
    Cycle(String),
}

impl DerivationError {
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::ScopeMismatch(_) => "scope_mismatch",
            Self::InvalidInput(_) => "invalid_input",
            Self::BoundExhaustion(_) => "bound_exhaustion",
            Self::Cycle(_) => "cycle",
        }
    }
}

impl std::fmt::Display for DerivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.outcome())
    }
}

impl std::error::Error for DerivationError {}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, DerivationError> {
    let value = serde_json::to_value(value)
        .map_err(|error| DerivationError::InvalidInput(error.to_string()))?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(super::evidence::canonical_json(&value).as_bytes());
    Ok(hash.finalize().to_hex().to_string())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_context(context: &ClaimContext) -> Result<(), DerivationError> {
    if !digest(&context.subject_blake3)
        || !digest(&context.policy_blake3)
        || context.target.trim().is_empty()
        || context.environment.trim().is_empty()
        || context.bounds.is_empty()
        || context
            .bounds
            .iter()
            .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
    {
        return Err(DerivationError::InvalidInput(
            "explicit subject, policy, target, environment and bounds are required".into(),
        ));
    }
    Ok(())
}

fn validate_requirement(requirement: &ContentBoundRequirementRow) -> Result<(), DerivationError> {
    if requirement.requirement_id.trim().is_empty()
        || requirement.specification_path.trim().is_empty()
        || !digest(&requirement.specification_blake3)
        || !digest(&requirement.requirement_body_blake3)
        || !digest(&requirement.row_blake3)
        || requirement.source_span.start_byte > requirement.source_span.end_byte
        || requirement.source_span.start_line == 0
        || requirement.source_span.end_line < requirement.source_span.start_line
    {
        return Err(DerivationError::InvalidInput(
            "claim requires an exact native requirement row identity".into(),
        ));
    }
    Ok(())
}

/// Versioned claim identity over requirement revision, interpretation,
/// subject, environment and bounds. `deny_unknown_fields` plus the explicit
/// schema make an unknown claim version an explicit rejection.
// r[impl cairn.scoped_claims.identity]
pub fn claim_identity(claim: &ScopedClaim) -> Result<String, DerivationError> {
    if claim.schema != CLAIM_SCHEMA_V1 {
        return Err(DerivationError::Unsupported(format!(
            "unsupported claim schema {}",
            claim.schema
        )));
    }
    if claim.kind != claim.proposition.claim_kind() {
        return Err(DerivationError::InvalidInput(format!(
            "claim kind {} does not match proposition {}",
            claim.kind.tag(),
            claim.proposition.tag()
        )));
    }
    validate_context(&claim.context)?;
    validate_requirement(&claim.requirement)?;
    if !digest(&claim.interpretation_blake3) || !digest(&claim.contract_blake3) {
        return Err(DerivationError::InvalidInput(
            "claim requires exact interpretation and contract identities".into(),
        ));
    }
    match &claim.proposition {
        Proposition::CheckPassed { check_id } if check_id.trim().is_empty() => {
            return Err(DerivationError::InvalidInput(
                "check observation requires a check identity".into(),
            ));
        }
        Proposition::ReceiptStructurallyValid { receipt_blake3 } if !digest(receipt_blake3) => {
            return Err(DerivationError::InvalidInput(
                "structural claim requires a receipt identity".into(),
            ));
        }
        _ => (),
    }
    identity("cairn.scoped-claim.identity.v1", claim)
}

fn context_identity(context: &ClaimContext) -> Result<String, DerivationError> {
    identity("cairn.scoped-claim-context.v1", context)
}

/// Admits the policy vocabulary and bounds. Core keeps only implementation
/// ceilings; every admitted number comes from the checked Nickel export.
// r[impl cairn.scoped_claims.bounds]
pub fn admit_claim_policy(policy: &ClaimPolicy) -> Result<(), DerivationError> {
    if policy.schema != CLAIM_POLICY_SCHEMA_V1 {
        return Err(DerivationError::Unsupported(format!(
            "unsupported claim policy schema {}",
            policy.schema
        )));
    }
    if policy.claims.schema != CLAIM_SCHEMA_V1 {
        return Err(DerivationError::Unsupported(format!(
            "policy admits unsupported claim schema {}",
            policy.claims.schema
        )));
    }
    let bounds = &policy.bounds;
    if bounds.max_claims == 0
        || bounds.max_claims > MAX_CLAIMS_CEILING
        || bounds.max_derivations == 0
        || bounds.max_derivations > MAX_DERIVATIONS_CEILING
        || bounds.max_producers == 0
        || bounds.max_producers > MAX_PRODUCERS_CEILING
        || bounds.max_premises == 0
        || bounds.max_premises > MAX_PREMISES_CEILING
        || bounds.max_depth == 0
        || bounds.max_depth > MAX_DEPTH_CEILING
    {
        return Err(DerivationError::BoundExhaustion(
            "admitted policy bounds are outside supported implementation ceilings".into(),
        ));
    }
    if policy.claims.kinds.is_empty() {
        return Err(DerivationError::Unsupported(
            "policy admits no claim kinds".into(),
        ));
    }
    let mut kinds = BTreeSet::new();
    for kind in &policy.claims.kinds {
        if !kinds.insert(*kind) {
            return Err(DerivationError::Unsupported(format!(
                "duplicate admitted claim kind {}",
                kind.tag()
            )));
        }
    }
    if policy.claims.propositions.is_empty() {
        return Err(DerivationError::Unsupported(
            "policy admits no propositions".into(),
        ));
    }
    let mut propositions = BTreeSet::new();
    for tag in &policy.claims.propositions {
        if !PROPOSITION_TAGS.contains(&tag.as_str()) {
            return Err(DerivationError::Unsupported(format!(
                "policy admits unknown proposition {tag}"
            )));
        }
        if !propositions.insert(tag.as_str()) {
            return Err(DerivationError::Unsupported(format!(
                "duplicate admitted proposition {tag}"
            )));
        }
    }
    if policy.rules.is_empty() {
        return Err(DerivationError::Unsupported(
            "policy admits no derivation rules".into(),
        ));
    }
    let mut rules = BTreeSet::new();
    for rule in &policy.rules {
        if !rules.insert(rule.id) {
            return Err(DerivationError::Unsupported(format!(
                "duplicate admitted rule {}",
                rule.id.tag()
            )));
        }
        if rule.version != RULE_VERSION_V1 {
            return Err(DerivationError::Unsupported(format!(
                "rule {} has unsupported version {}",
                rule.id.tag(),
                rule.version
            )));
        }
        if rule.assurance_scope.trim().is_empty() {
            return Err(DerivationError::InvalidInput(format!(
                "rule {} requires a non-empty assurance scope",
                rule.id.tag()
            )));
        }
    }
    Ok(())
}

fn typed_error(error: SemanticError) -> DerivationError {
    match error {
        SemanticError::UnknownProfile(id) => {
            DerivationError::Unsupported(format!("typed requirement profile {id}"))
        }
        SemanticError::UnsupportedConstruct(construct) => {
            DerivationError::Unsupported(format!("unsupported typed construct {construct}"))
        }
        SemanticError::BoundExhaustion(detail) => {
            DerivationError::BoundExhaustion(format!("typed requirement contract: {detail}"))
        }
        SemanticError::RequirementRevisionMismatch => DerivationError::ScopeMismatch(
            "typed contract does not bind the supplied native requirement row".into(),
        ),
        other => DerivationError::InvalidInput(format!("typed requirement contract: {other:?}")),
    }
}

#[derive(Default)]
struct Evidence {
    support: Vec<ConclusionLink>,
    counterevidence: Vec<ConclusionLink>,
    unresolved: BTreeSet<String>,
}

fn link(
    source: String,
    rule: &str,
    version: u32,
    context: &ClaimContext,
    assurance_scope: &str,
    residual_assumptions: BTreeSet<String>,
    premises: Vec<String>,
) -> ConclusionLink {
    ConclusionLink {
        source,
        rule: rule.into(),
        version,
        premises,
        applicability: context.clone(),
        assurance_scope: assurance_scope.into(),
        residual_assumptions,
    }
}

/// Attaches one admitted conclusion only to claims with the exact same
/// proposition and context. Cross-scope evidence never leaks sideways.
// r[impl cairn.scoped_claims.scope]
fn add_fact(
    claims: &BTreeMap<String, &ScopedClaim>,
    evidence: &mut BTreeMap<String, Evidence>,
    context: &ClaimContext,
    proposition: Proposition,
    positive: bool,
    admitted: ConclusionLink,
) {
    for (id, claim) in claims {
        if claim.context != *context || claim.proposition != proposition {
            continue;
        }
        let entry = evidence.entry(id.clone()).or_default();
        if positive {
            entry.support.push(admitted.clone());
        } else {
            entry.counterevidence.push(admitted.clone());
        }
    }
}

fn validate_dependencies(dependencies: &BTreeSet<String>) -> Result<(), DerivationError> {
    if dependencies
        .iter()
        .any(|dependency| dependency.trim().is_empty())
    {
        return Err(DerivationError::InvalidInput(
            "trust dependencies must be non-empty labels".into(),
        ));
    }
    Ok(())
}

fn admission_diagnostic(
    diagnostics: &mut BTreeMap<String, Vec<String>>,
    source: &str,
    code: &str,
    summary: &str,
) {
    diagnostics
        .entry(source.to_string())
        .or_default()
        .push(format!("{code}: {summary}"));
}

/// Re-admits every producer through its native validator and records only the
/// conclusions that validator actually established.
// r[impl cairn.scoped_claims.native_producers]
fn admit_producer(
    input: &ProducerInput,
    claims: &BTreeMap<String, &ScopedClaim>,
    evidence: &mut BTreeMap<String, Evidence>,
    diagnostics: &mut BTreeMap<String, Vec<String>>,
) -> Result<(), DerivationError> {
    use super::agent_receipt::{evaluate_authoritative_agent_completion, AgentExecutionResult};

    match input {
        ProducerInput::AuthoritativeCompletion {
            context,
            contract,
            results,
            trust_dependencies,
        } => {
            validate_context(context)?;
            validate_dependencies(trust_dependencies)?;
            let report = evaluate_authoritative_agent_completion(
                contract,
                results,
                &context.subject_blake3,
                &context.policy_blake3,
            );
            if !report.repository_id.is_empty() && report.repository_id != context.target {
                return Err(DerivationError::ScopeMismatch(
                    "native completion repository identity differs from the declared claim target"
                        .into(),
                ));
            }
            let mut assumptions = trust_dependencies.clone();
            assumptions.insert("observation-provenance:caller-retained-results".into());
            assumptions.insert("native-execution-honesty:not-established".into());
            assumptions.insert("native-test-oracle:adequacy-not-established".into());
            assumptions.insert("verifier:cairn.native-authoritative-agent-completion.v2".into());
            assumptions.insert("native-requirement-row-freshness:caller-supplied".into());
            assumptions.insert(format!(
                "declared-claim-context:{}",
                context_identity(context)?
            ));
            let source = report.receipt_hash.clone();
            for diagnostic in &report.diagnostics {
                admission_diagnostic(diagnostics, &source, &diagnostic.code, &diagnostic.summary);
            }
            // Contract-level or result-set defects invalidate every observation;
            // they are not silently reduced to per-check evidence.
            if report.diagnostics.iter().any(|diagnostic| {
                diagnostic.check_id.is_none()
                    || diagnostic.code.starts_with("verification_contract.")
            }) {
                return Ok(());
            }
            let rows: &[serde_json::Value] = match results {
                serde_json::Value::Array(rows) => rows.as_slice(),
                serde_json::Value::Object(_) => {
                    match results.get("results").and_then(serde_json::Value::as_array) {
                        Some(rows) => rows.as_slice(),
                        None => return Ok(()),
                    }
                }
                _ => return Ok(()),
            };
            for row in rows {
                let Ok(result) = serde_json::from_value::<AgentExecutionResult>(row.clone()) else {
                    continue;
                };
                let mut relevant = report.diagnostics.iter().filter(|diagnostic| {
                    diagnostic.check_id.as_deref() == Some(result.check_id.as_str())
                });
                // Only an unambiguous single pass/non-pass row is an observation.
                // Timeout, unsupported, missing, duplicate, blocked and malformed
                // rows establish neither polarity.
                if relevant.any(|diagnostic| diagnostic.code != "verification_result.non_passing") {
                    continue;
                }
                if report.passed_checks.contains(&result.check_id) {
                    add_fact(
                        claims,
                        evidence,
                        context,
                        Proposition::CheckPassed {
                            check_id: result.check_id.clone(),
                        },
                        true,
                        link(
                            source.clone(),
                            "native-check-observation.v1",
                            1,
                            context,
                            "exact saved selected check status only; no quantified behavior",
                            assumptions.clone(),
                            Vec::new(),
                        ),
                    );
                } else if result.status == "failed" {
                    add_fact(
                        claims,
                        evidence,
                        context,
                        Proposition::CheckPassed {
                            check_id: result.check_id.clone(),
                        },
                        false,
                        link(
                            source.clone(),
                            "native-check-observation.v1",
                            1,
                            context,
                            "exact saved selected check status only; no quantified behavior",
                            assumptions.clone(),
                            Vec::new(),
                        ),
                    );
                }
            }
            if report.valid {
                add_fact(
                    claims,
                    evidence,
                    context,
                    Proposition::NativeCompletionEligible,
                    true,
                    link(
                        source,
                        "native-completion-admission.v1",
                        1,
                        context,
                        "repository-owned authoritative completion evaluator accepted these exact saved inputs",
                        assumptions,
                        Vec::new(),
                    ),
                );
            }
        }
        ProducerInput::StructuralAstGrep {
            context,
            repository_scope,
            receipt,
            trust_dependencies,
        } => {
            validate_context(context)?;
            validate_dependencies(trust_dependencies)?;
            if repository_scope.trim().is_empty() {
                return Err(DerivationError::InvalidInput(
                    "structural producer requires an explicit repository scope".into(),
                ));
            }
            let policy = super::ast_grep_gate::AstGrepGatePolicy {
                mode: "inventory".into(),
                allowed_output_formats: vec!["json".into(), "jsonl".into(), "sarif".into()],
                expected_repository_scope: repository_scope.clone(),
                required_non_claim_fragments: vec![
                    "source correctness".into(),
                    "requirement satisfaction".into(),
                    "proof discharge".into(),
                    "release eligibility".into(),
                ],
            };
            let report = super::ast_grep_gate::validate_ast_grep_receipt(&policy, receipt);
            for issue in &report.issues {
                admission_diagnostic(
                    diagnostics,
                    &report.receipt_hash,
                    &issue.code,
                    &issue.field_path,
                );
            }
            if report.valid {
                let mut assumptions = trust_dependencies.clone();
                assumptions.insert("structural-receipt-provenance:declared-not-executed".into());
                assumptions.insert("verifier:cairn.native-ast-grep-receipt-shape.v1".into());
                assumptions.insert("recorded-only:no-behavior".into());
                assumptions.insert(format!(
                    "declared-claim-context:{}",
                    context_identity(context)?
                ));
                add_fact(
                    claims,
                    evidence,
                    context,
                    Proposition::ReceiptStructurallyValid {
                        receipt_blake3: report.receipt_hash.clone(),
                    },
                    true,
                    link(
                        report.receipt_hash,
                        "native-structural-admission.v1",
                        1,
                        context,
                        "recorded_only: exact receipt shape and declared scope only; no behavior or requirement satisfaction",
                        assumptions,
                        Vec::new(),
                    ),
                );
            }
        }
    }
    Ok(())
}

fn rule_policy(policy: &ClaimPolicy, rule: Rule) -> Option<&RulePolicy> {
    policy.rules.iter().find(|candidate| candidate.id == rule)
}

fn validate_rule(
    derivation: &Derivation,
    claims: &BTreeMap<String, &ScopedClaim>,
) -> Result<(), DerivationError> {
    let Some(conclusion) = claims.get(&derivation.conclusion) else {
        return Ok(());
    };
    let mut premises = Vec::with_capacity(derivation.premises.len());
    for premise in &derivation.premises {
        let Some(claim) = claims.get(premise) else {
            // An undeclared premise cannot support anything; replay reports it
            // as an explicit unresolved premise instead of inventing a rule.
            return Ok(());
        };
        premises.push(claim.proposition.clone());
    }
    let valid = match derivation.rule {
        Rule::ReviewedCompletionV1 => {
            premises.len() == 2
                && conclusion.proposition == Proposition::ReviewedCompletionDecision
                && premises
                    .iter()
                    .filter(|proposition| **proposition == Proposition::NativeCompletionEligible)
                    .count()
                    == 1
                && premises
                    .iter()
                    .filter(|proposition| **proposition == Proposition::ReviewedCorrespondence)
                    .count()
                    == 1
        }
    };
    if !valid {
        return Err(DerivationError::Unsupported(format!(
            "rule {} does not establish the declared conclusion of derivation {}",
            derivation.rule.tag(),
            derivation.id
        )));
    }
    Ok(())
}

/// Replays all derivations for one claim. Premise evidence is fully replayed
/// first; derivation order is canonical, so identical admitted inputs yield
/// identical conclusions and residual assumptions.
// r[impl cairn.scoped_claims.rules]
fn replay<'a>(
    id: &'a str,
    incoming: &BTreeMap<&'a str, Vec<&'a Derivation>>,
    evidence: &mut BTreeMap<String, Evidence>,
    declared: &BTreeSet<String>,
    visiting: &mut BTreeSet<&'a str>,
    depths: &mut BTreeMap<&'a str, usize>,
    max_depth: usize,
) -> Result<usize, DerivationError> {
    if let Some(depth) = depths.get(id) {
        return Ok(*depth);
    }
    if !visiting.insert(id) {
        return Err(DerivationError::Cycle(id.to_string()));
    }
    if visiting.len() > max_depth {
        return Err(DerivationError::BoundExhaustion(format!(
            "derivation depth exceeds the admitted bound {max_depth} at {id}"
        )));
    }
    let mut depth = 0usize;
    if let Some(derivations) = incoming.get(id) {
        let mut ordered = derivations.clone();
        ordered.sort_by(|left, right| left.id.cmp(&right.id));
        for derivation in ordered {
            let mut assumptions = derivation.residual_assumptions.clone();
            let mut missing = BTreeSet::new();
            let mut supported = true;
            for premise in &derivation.premises {
                if !declared.contains(premise) {
                    supported = false;
                    missing.insert(premise.clone());
                    continue;
                }
                let premise_depth = replay(
                    premise, incoming, evidence, declared, visiting, depths, max_depth,
                )?;
                depth = depth.max(premise_depth + 1);
                let entry = evidence.get(premise.as_str()).ok_or_else(|| {
                    DerivationError::InvalidInput(format!(
                        "declared premise {premise} has no evidence entry"
                    ))
                })?;
                if entry.support.is_empty() || !entry.counterevidence.is_empty() {
                    supported = false;
                    missing.insert(premise.clone());
                }
                missing.extend(entry.unresolved.iter().cloned());
                for admitted in entry.support.iter().chain(&entry.counterevidence) {
                    assumptions.extend(admitted.residual_assumptions.iter().cloned());
                }
            }
            if depth > max_depth {
                return Err(DerivationError::BoundExhaustion(format!(
                    "derivation depth exceeds the admitted bound {max_depth} at {id}"
                )));
            }
            let entry = evidence.entry(id.to_string()).or_default();
            entry.unresolved.extend(missing);
            if supported {
                entry.support.push(link(
                    derivation.id.clone(),
                    derivation.rule.tag(),
                    RULE_VERSION_V1,
                    &derivation.applicability,
                    &derivation.assurance_scope,
                    assumptions,
                    derivation.premises.clone(),
                ));
            }
            // A premise that is refuted or disputed never supports a derived
            // conclusion: the conclusion stays unresolved and names that
            // premise. A composite judgment is not itself refuted here.
        }
    }
    if depth > max_depth {
        return Err(DerivationError::BoundExhaustion(format!(
            "derivation depth exceeds the admitted bound {max_depth} at {id}"
        )));
    }
    visiting.remove(id);
    depths.insert(id, depth);
    Ok(depth)
}

fn state_reason(state: ClaimState) -> &'static str {
    match state {
        ClaimState::Supported => {
            "at least one admitted support link applies to this exact proposition and context; no applicable counterevidence"
        }
        ClaimState::Refuted => {
            "only applicable counterevidence; no admitted support for this exact proposition and context"
        }
        ClaimState::Disputed => {
            "applicable support and counterevidence are both retained; neither cancels the other"
        }
        ClaimState::Unresolved => {
            "no admitted support and no applicable counterevidence for this exact proposition, context and revision"
        }
    }
}

/// Re-admits the typed contract, review and every native producer, then
/// replays the bounded acyclic derivation graph.
// r[impl cairn.scoped_claims.states]
pub fn evaluate_derivations(
    policy: &ClaimPolicy,
    request: &EvaluationRequest,
) -> Result<DerivationReport, DerivationError> {
    admit_claim_policy(policy)?;
    let bounds = &policy.bounds;
    if request.claims.len() > bounds.max_claims {
        return Err(DerivationError::BoundExhaustion(format!(
            "claims {} exceed the admitted bound {}",
            request.claims.len(),
            bounds.max_claims
        )));
    }
    if request.derivations.len() > bounds.max_derivations {
        return Err(DerivationError::BoundExhaustion(format!(
            "derivations {} exceed the admitted bound {}",
            request.derivations.len(),
            bounds.max_derivations
        )));
    }
    if request.producers.len() > bounds.max_producers {
        return Err(DerivationError::BoundExhaustion(format!(
            "producers {} exceed the admitted bound {}",
            request.producers.len(),
            bounds.max_producers
        )));
    }
    let normalized = normalize_contract(
        &request.profile,
        &request.native_requirement,
        &request.semantic_contract,
    )
    .map_err(typed_error)?;
    let review_identity = match &request.correspondence_review {
        Some(review) => Some(
            admit_correspondence_review(&normalized, review, &request.authorized_reviewers)
                .map_err(typed_error)?,
        ),
        None => None,
    };
    let mut claims: BTreeMap<String, &ScopedClaim> = BTreeMap::new();
    for claim in &request.claims {
        let id = claim_identity(claim)?;
        if claim.requirement != request.native_requirement {
            return Err(DerivationError::ScopeMismatch(format!(
                "claim {id} names a different native requirement revision"
            )));
        }
        if claim.interpretation_blake3 != normalized.interpretation_blake3
            || claim.contract_blake3 != normalized.contract_blake3
        {
            return Err(DerivationError::ScopeMismatch(format!(
                "claim {id} does not bind the admitted interpretation and contract"
            )));
        }
        if !policy.claims.kinds.contains(&claim.kind) {
            return Err(DerivationError::Unsupported(format!(
                "claim kind {} is not admitted by policy",
                claim.kind.tag()
            )));
        }
        if !policy
            .claims
            .propositions
            .iter()
            .any(|tag| tag == claim.proposition.tag())
        {
            return Err(DerivationError::Unsupported(format!(
                "proposition {} is not admitted by policy",
                claim.proposition.tag()
            )));
        }
        if claims.insert(id.clone(), claim).is_some() {
            return Err(DerivationError::InvalidInput(
                "duplicate claim identity in request".into(),
            ));
        }
    }
    let declared = claims.keys().cloned().collect::<BTreeSet<_>>();
    let mut evidence: BTreeMap<String, Evidence> = claims
        .keys()
        .map(|id| (id.clone(), Evidence::default()))
        .collect();
    let mut diagnostics: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for producer in &request.producers {
        admit_producer(producer, &claims, &mut evidence, &mut diagnostics)?;
    }
    if let (Some(review), Some(review_identity)) = (
        request.correspondence_review.as_ref(),
        review_identity.as_ref(),
    ) {
        for (id, claim) in &claims {
            if claim.proposition != Proposition::ReviewedCorrespondence {
                continue;
            }
            let assumptions = BTreeSet::from([
                format!(
                    "review-fidelity:{}:{}",
                    review.reviewer, review.review_artifact_blake3
                ),
                "review-authenticity:externally-retained-artifact".into(),
                "native-requirement-row-freshness:caller-supplied".into(),
                format!("interpreter:{}", claim.interpretation_blake3),
                format!(
                    "declared-claim-context:{}",
                    context_identity(&claim.context)?
                ),
            ]);
            evidence.entry(id.clone()).or_default().support.push(link(
                review_identity.to_string(),
                "native-correspondence-admission.v1",
                1,
                &claim.context,
                "reviewed prose-to-finite-contract correspondence only",
                assumptions,
                Vec::new(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    let mut incoming: BTreeMap<&str, Vec<&Derivation>> = BTreeMap::new();
    for derivation in &request.derivations {
        if derivation.id.trim().is_empty() {
            return Err(DerivationError::InvalidInput(
                "derivation identity must be non-empty".into(),
            ));
        }
        if !ids.insert(&derivation.id) {
            return Err(DerivationError::InvalidInput(format!(
                "duplicate derivation identity {}",
                derivation.id
            )));
        }
        if derivation
            .residual_assumptions
            .iter()
            .any(|assumption| assumption.trim().is_empty())
        {
            return Err(DerivationError::InvalidInput(format!(
                "derivation {} has an empty residual assumption",
                derivation.id
            )));
        }
        if derivation.premises.len() > bounds.max_premises {
            return Err(DerivationError::BoundExhaustion(format!(
                "derivation {} exceeds the admitted premise bound {}",
                derivation.id, bounds.max_premises
            )));
        }
        let Some(conclusion) = claims.get(&derivation.conclusion) else {
            return Err(DerivationError::InvalidInput(format!(
                "derivation {} conclusion is not a declared claim",
                derivation.id
            )));
        };
        if conclusion.context != derivation.applicability {
            return Err(DerivationError::ScopeMismatch(format!(
                "derivation {} applicability differs from its conclusion scope",
                derivation.id
            )));
        }
        let Some(rule) = rule_policy(policy, derivation.rule) else {
            return Err(DerivationError::Unsupported(format!(
                "rule {} is not admitted by policy",
                derivation.rule.tag()
            )));
        };
        if derivation.assurance_scope != rule.assurance_scope {
            return Err(DerivationError::Unsupported(format!(
                "derivation {} declares an assurance scope other than the admitted rule scope",
                derivation.id
            )));
        }
        let unique = derivation.premises.iter().collect::<BTreeSet<_>>();
        if unique.len() != derivation.premises.len() {
            return Err(DerivationError::InvalidInput(format!(
                "derivation {} repeats a premise",
                derivation.id
            )));
        }
        for premise in &derivation.premises {
            if let Some(premise_claim) = claims.get(premise) {
                if premise_claim.context != conclusion.context {
                    return Err(DerivationError::ScopeMismatch(format!(
                        "derivation {} combines premise {} from another subject, environment or bound scope",
                        derivation.id, premise
                    )));
                }
            }
        }
        validate_rule(derivation, &claims)?;
        incoming
            .entry(derivation.conclusion.as_str())
            .or_default()
            .push(derivation);
    }
    let mut visiting = BTreeSet::new();
    let mut depths = BTreeMap::new();
    for id in claims.keys() {
        replay(
            id,
            &incoming,
            &mut evidence,
            &declared,
            &mut visiting,
            &mut depths,
            bounds.max_depth,
        )?;
    }
    let mut explanations = Vec::with_capacity(claims.len());
    let mut unresolved = BTreeSet::new();
    let mut trust_dependencies = BTreeSet::new();
    for (id, claim) in &claims {
        let mut entry = evidence.remove(id).unwrap_or_default();
        entry.support.sort();
        entry.counterevidence.sort();
        let state = match (entry.support.is_empty(), entry.counterevidence.is_empty()) {
            (true, true) => ClaimState::Unresolved,
            (false, true) => ClaimState::Supported,
            (true, false) => ClaimState::Refuted,
            (false, false) => ClaimState::Disputed,
        };
        if state == ClaimState::Unresolved {
            unresolved.insert(id.clone());
        }
        let residual_assumptions = entry
            .support
            .iter()
            .chain(&entry.counterevidence)
            .flat_map(|admitted| admitted.residual_assumptions.iter().cloned())
            .collect::<BTreeSet<_>>();
        trust_dependencies.extend(residual_assumptions.iter().cloned());
        explanations.push(ClaimExplanation {
            claim_id: id.clone(),
            claim: (*claim).clone(),
            kind: claim.kind,
            state,
            state_reason: state_reason(state).into(),
            support: entry.support,
            counterevidence: entry.counterevidence,
            unresolved_premises: entry.unresolved,
            residual_assumptions,
        });
    }
    for values in diagnostics.values_mut() {
        values.sort();
        values.dedup();
    }
    let input_blake3 = identity("cairn.scoped-derivation-input.v1", &(policy, request))?;
    let mut report = DerivationReport {
        schema: DERIVATION_REPORT_SCHEMA_V1.into(),
        advisory: true,
        claims: explanations,
        unresolved,
        admission_diagnostics: diagnostics,
        trust_dependencies,
        independence: INDEPENDENCE_NOT_ESTABLISHED.into(),
        input_blake3,
        report_blake3: String::new(),
        boundary: DERIVATION_BOUNDARY.into(),
    };
    // r[impl cairn.scoped_claims.determinism]
    let report_blake3 = identity("cairn.scoped-derivation-report.v1", &report)?;
    report.report_blake3 = report_blake3;
    Ok(report)
}

#[cfg(test)]
mod tests;

//! Pure stack schema, role, verification-role, and compatibility-alias registry.
//!
//! The registry is data validation only. It does not read policy files, execute
//! release gates, decide runtime authority, decide lifecycle readiness, or prove
//! producer/consumer semantic correctness.
//!
//! r[impl valence.stack_schema_role_registry.contract]
//! r[impl valence.evidence_ir.verification_role_vocabulary.contract]
//! r[impl valence.evidence_ir.stack_role_registry_core.contract]
//! r[impl valence.evidence_ir.stack_role_registry_core.validation]
//! r[impl valence.evidence_ir.stack_role_registry_core.adapters]

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::shared_identity::contains_overclaim;

const FIELD_ALIAS: &str = "alias";
const FIELD_ALLOWED_CONSUMERS: &str = "allowed_consumers";
const FIELD_ALLOWED_ROLES: &str = "allowed_roles";
const FIELD_COMPATIBILITY_CLASS: &str = "compatibility_class";
const FIELD_CONSTANT: &str = "constant";
const FIELD_COUNTING: &str = "counting";
const FIELD_EVIDENCE_TIER: &str = "evidence_tier";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_OWNER: &str = "owner";
const FIELD_ROLE: &str = "role";
const FIELD_SCHEMA: &str = "schema";
const FIELD_STATUS: &str = "status";
const FIELD_VERIFICATION_ROLE: &str = "verification_role";

pub const ROLE_REGISTRY_REQUIRED_NON_CLAIM: &str =
    "role registry validation proves role/schema/vocabulary compatibility only";
pub const ROLE_REGISTRY_SUPPORTED_CLAIM: &str = "role registry validation proves role/schema/vocabulary compatibility only and does not prove evidence truth, verifier soundness, source-code correctness, release eligibility, lifecycle readiness, runtime authority, or whole-system safety";
pub const VERIFICATION_ROLE_SUPPORTED_CLAIM: &str = "only configured property roles count as verified property evidence; recorded-only, boundary, and manual-review roles remain context";
pub const STACK_SCHEMA_SUPPORTED_CLAIM: &str =
    "registry validation prevents literal drift only and does not prove producer or consumer schema semantics";

const ACTIVE_STATUS: &str = "active";
const LEGACY_COMPATIBLE_STATUS: &str = "legacy-compatible";
const MIGRATION_EXCEPTION_STATUS: &str = "migration-exception";
const RETIRED_STATUS: &str = "retired";

const CANONICAL_STACK_ROLES: &[&str] = &[
    "basalt",
    "ucan",
    "trellis",
    "octet",
    "valence",
    "cairn",
    "mantle",
    "external",
    "manual-review",
];

const CANONICAL_VERIFICATION_ROLES: &[&str] = &["property", "recorded_only", "boundary", "manual_review"];

const CANONICAL_COMPATIBILITY_CLASSES: &[&str] = &[
    "canonical",
    "legacy-compatible",
    "migration-exception",
    "external-reference",
];

const SUPPORTED_CONSUMERS: &[&str] = &[
    "valence", "octet", "mantle", "cairn", "trellis", "basalt", "ucan", "molten", "aspen",
];

/// How a verification role contributes to verified-property counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationCounting {
    CountsAsProperty,
    RecordedOnly,
    BoundaryOnly,
    ManualReviewOnly,
}

/// One row in the verification-role vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRoleVocabularyRow {
    pub role_id: String,
    pub evidence_tier: String,
    pub counting: VerificationCounting,
    pub display_label: String,
    pub non_claims: Vec<String>,
}

/// One stack schema/role registry row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackSchemaRoleRegistryRow {
    pub registry_id: String,
    pub schema_id: String,
    pub role_name: String,
    pub claim_scope: String,
    pub owner_repo: String,
    pub allowed_consumers: Vec<String>,
    pub status: String,
    pub non_claims: Vec<String>,
}

/// Role/schema binding row used by the reusable stack role registry core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackRoleRegistryRow {
    pub role_id: String,
    pub schema_id: String,
    pub verification_role: String,
    pub compatibility_class: String,
    pub required: bool,
    pub non_claims: Vec<String>,
}

/// Explicit compatibility alias preserving legacy public spellings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleAliasRow {
    pub alias: String,
    pub canonical_role: String,
    pub owner_repo: String,
    pub status: String,
    pub non_claims: Vec<String>,
}

/// Schema row naming the roles accepted for that schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRoleBindingRow {
    pub schema_id: String,
    pub allowed_roles: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Consumer constants submitted for literal drift validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerConstantRow {
    pub name: String,
    pub value: String,
    pub expected_value: String,
    pub registry_digest: String,
}

/// Field-scoped deterministic registry issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RoleRegistryIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub subject: String,
}

impl RoleRegistryIssue {
    #[must_use]
    pub fn new(field: &'static str, code: &'static str, subject: impl Into<String>) -> Self {
        Self {
            field,
            code,
            subject: subject.into(),
        }
    }
}

/// Validation report for role and schema registry checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoleRegistryReport {
    pub valid: bool,
    pub issues: Vec<RoleRegistryIssue>,
    pub supported_claim: String,
}

/// Parse result for a legacy role spelling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoleAliasResolution {
    pub public_role: String,
    pub canonical_role: String,
    pub owner_repo: String,
    pub status: String,
    pub supported_claim: String,
}

/// Validate stack schema/role registry rows.
#[must_use]
pub fn validate_stack_schema_role_registry(rows: &[StackSchemaRoleRegistryRow]) -> RoleRegistryReport {
    let mut issues = Vec::new();
    for row in rows {
        validate_non_claims(&row.registry_id, &row.non_claims, &mut issues);
        validate_known_status(&row.registry_id, &row.status, &mut issues);
        validate_known_role(&row.registry_id, &row.role_name, &mut issues);
        if row.schema_id.trim().is_empty() {
            issues.push(RoleRegistryIssue::new(FIELD_SCHEMA, "schema_missing", row.registry_id.clone()));
        }
        if row.claim_scope.trim().is_empty() {
            issues.push(RoleRegistryIssue::new("claim_scope", "claim_scope_missing", row.registry_id.clone()));
        }
        if row.owner_repo.trim().is_empty() {
            issues.push(RoleRegistryIssue::new(FIELD_OWNER, "owner_missing", row.registry_id.clone()));
        }
        if row.allowed_consumers.is_empty() {
            issues.push(RoleRegistryIssue::new(
                FIELD_ALLOWED_CONSUMERS,
                "allowed_consumers_missing",
                row.registry_id.clone(),
            ));
        }
        for consumer in &row.allowed_consumers {
            if !SUPPORTED_CONSUMERS.contains(&consumer.as_str()) {
                issues.push(RoleRegistryIssue::new(FIELD_ALLOWED_CONSUMERS, "unsupported_consumer", consumer.clone()));
            }
        }
    }
    finish_report(issues, STACK_SCHEMA_SUPPORTED_CLAIM)
}

/// Validate verification-role vocabulary rows and role-counting behavior.
#[must_use]
pub fn validate_verification_role_vocabulary(rows: &[VerificationRoleVocabularyRow]) -> RoleRegistryReport {
    let mut issues = Vec::new();
    for row in rows {
        validate_non_claims(&row.role_id, &row.non_claims, &mut issues);
        if !CANONICAL_VERIFICATION_ROLES.contains(&row.role_id.as_str()) {
            issues.push(RoleRegistryIssue::new(
                FIELD_VERIFICATION_ROLE,
                "unknown_verification_role",
                row.role_id.clone(),
            ));
        }
        if row.display_label.trim().is_empty() {
            issues.push(RoleRegistryIssue::new(FIELD_VERIFICATION_ROLE, "display_label_missing", row.role_id.clone()));
        }
        validate_counting(row, &mut issues);
    }
    finish_report(issues, VERIFICATION_ROLE_SUPPORTED_CLAIM)
}

/// Validate reusable stack role registry rows, schema bindings, and aliases.
#[must_use]
pub fn validate_stack_role_registry(
    rows: &[StackRoleRegistryRow],
    schemas: &[SchemaRoleBindingRow],
    aliases: &[RoleAliasRow],
) -> RoleRegistryReport {
    let schema_map = schema_binding_map(schemas);
    let mut issues = Vec::new();
    let mut required_roles = BTreeSet::new();

    for schema in schemas {
        validate_non_claims(&schema.schema_id, &schema.non_claims, &mut issues);
        if schema.allowed_roles.is_empty() {
            issues.push(RoleRegistryIssue::new(FIELD_ALLOWED_ROLES, "allowed_roles_missing", schema.schema_id.clone()));
        }
    }

    for row in rows {
        validate_non_claims(&row.role_id, &row.non_claims, &mut issues);
        validate_known_role(&row.role_id, &row.role_id, &mut issues);
        if !CANONICAL_VERIFICATION_ROLES.contains(&row.verification_role.as_str()) {
            issues.push(RoleRegistryIssue::new(
                FIELD_VERIFICATION_ROLE,
                "missing_verification_role",
                row.role_id.clone(),
            ));
        }
        if !CANONICAL_COMPATIBILITY_CLASSES.contains(&row.compatibility_class.as_str()) {
            issues.push(RoleRegistryIssue::new(
                FIELD_COMPATIBILITY_CLASS,
                "unknown_compatibility_class",
                row.role_id.clone(),
            ));
        }
        if let Some(allowed_roles) = schema_map.get(row.schema_id.as_str()) {
            if !allowed_roles.contains(&row.role_id) {
                issues.push(RoleRegistryIssue::new(FIELD_SCHEMA, "schema_role_mismatch", row.schema_id.clone()));
            }
        } else {
            issues.push(RoleRegistryIssue::new(FIELD_SCHEMA, "schema_missing", row.schema_id.clone()));
        }
        if row.required && !required_roles.insert(row.role_id.clone()) {
            issues.push(RoleRegistryIssue::new(FIELD_ROLE, "duplicate_required_role", row.role_id.clone()));
        }
    }

    for alias in aliases {
        validate_alias(alias, &mut issues);
    }

    finish_report(issues, ROLE_REGISTRY_SUPPORTED_CLAIM)
}

/// Validate consumer constants against expected values and a registry digest.
#[must_use]
pub fn validate_consumer_constants(rows: &[ConsumerConstantRow]) -> RoleRegistryReport {
    let mut issues = Vec::new();
    for row in rows {
        if row.registry_digest.trim().is_empty() {
            issues.push(RoleRegistryIssue::new(FIELD_CONSTANT, "registry_digest_missing", row.name.clone()));
        }
        if row.value != row.expected_value {
            issues.push(RoleRegistryIssue::new(FIELD_CONSTANT, "constant_drift", row.name.clone()));
        }
    }
    finish_report(issues, STACK_SCHEMA_SUPPORTED_CLAIM)
}

/// Resolve an approved legacy role spelling through explicit alias rows.
pub fn resolve_role_alias(
    public_role: &str,
    aliases: &[RoleAliasRow],
) -> Result<RoleAliasResolution, RoleRegistryReport> {
    let Some(alias) = aliases.iter().find(|row| row.alias == public_role) else {
        let issue = RoleRegistryIssue::new(FIELD_ALIAS, "alias_missing", public_role.to_owned());
        return Err(finish_report(vec![issue], ROLE_REGISTRY_SUPPORTED_CLAIM));
    };
    let mut issues = Vec::new();
    validate_alias(alias, &mut issues);
    if !issues.is_empty() {
        return Err(finish_report(issues, ROLE_REGISTRY_SUPPORTED_CLAIM));
    }
    Ok(RoleAliasResolution {
        public_role: alias.alias.clone(),
        canonical_role: alias.canonical_role.clone(),
        owner_repo: alias.owner_repo.clone(),
        status: alias.status.clone(),
        supported_claim: ROLE_REGISTRY_SUPPORTED_CLAIM.to_owned(),
    })
}

fn validate_alias(alias: &RoleAliasRow, issues: &mut Vec<RoleRegistryIssue>) {
    validate_non_claims(&alias.alias, &alias.non_claims, issues);
    validate_known_role(&alias.alias, &alias.canonical_role, issues);
    if alias.status != LEGACY_COMPATIBLE_STATUS && alias.status != MIGRATION_EXCEPTION_STATUS {
        issues.push(RoleRegistryIssue::new(FIELD_STATUS, "stale_alias", alias.alias.clone()));
    }
    if alias.owner_repo.trim().is_empty() {
        issues.push(RoleRegistryIssue::new(FIELD_OWNER, "owner_missing", alias.alias.clone()));
    }
}

fn schema_binding_map(schemas: &[SchemaRoleBindingRow]) -> BTreeMap<&str, BTreeSet<String>> {
    schemas
        .iter()
        .map(|schema| (schema.schema_id.as_str(), schema.allowed_roles.iter().cloned().collect::<BTreeSet<_>>()))
        .collect()
}

fn validate_counting(row: &VerificationRoleVocabularyRow, issues: &mut Vec<RoleRegistryIssue>) {
    match (row.role_id.as_str(), row.counting) {
        ("property", VerificationCounting::CountsAsProperty)
        | ("recorded_only", VerificationCounting::RecordedOnly)
        | ("boundary", VerificationCounting::BoundaryOnly)
        | ("manual_review", VerificationCounting::ManualReviewOnly) => {}
        _ => issues.push(RoleRegistryIssue::new(FIELD_COUNTING, "role_counting_drift", row.role_id.clone())),
    }
    if row.evidence_tier.trim().is_empty() {
        issues.push(RoleRegistryIssue::new(FIELD_EVIDENCE_TIER, "evidence_tier_missing", row.role_id.clone()));
    }
}

fn validate_known_status(subject: &str, status: &str, issues: &mut Vec<RoleRegistryIssue>) {
    if !matches!(status, ACTIVE_STATUS | LEGACY_COMPATIBLE_STATUS | MIGRATION_EXCEPTION_STATUS | RETIRED_STATUS) {
        issues.push(RoleRegistryIssue::new(FIELD_STATUS, "unknown_status", subject.to_owned()));
    }
}

fn validate_known_role(subject: &str, role: &str, issues: &mut Vec<RoleRegistryIssue>) {
    if !CANONICAL_STACK_ROLES.contains(&role) {
        issues.push(RoleRegistryIssue::new(FIELD_ROLE, "unknown_role", subject.to_owned()));
    }
}

fn validate_non_claims(subject: &str, non_claims: &[String], issues: &mut Vec<RoleRegistryIssue>) {
    if non_claims.is_empty() {
        issues.push(RoleRegistryIssue::new(FIELD_NON_CLAIMS, "non_claims_missing", subject.to_owned()));
        return;
    }
    if !non_claims.iter().any(|claim| claim.contains(ROLE_REGISTRY_REQUIRED_NON_CLAIM)) {
        issues.push(RoleRegistryIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened", subject.to_owned()));
    }
    if non_claims.iter().any(|claim| contains_overclaim(claim)) {
        issues.push(RoleRegistryIssue::new(FIELD_NON_CLAIMS, "overclaim_detected", subject.to_owned()));
    }
}

fn finish_report(mut issues: Vec<RoleRegistryIssue>, supported_claim: &str) -> RoleRegistryReport {
    issues.sort();
    issues.dedup();
    RoleRegistryReport {
        valid: issues.is_empty(),
        issues,
        supported_claim: supported_claim.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY_DIGEST: &str = "b3:registry";

    fn non_claims() -> Vec<String> {
        vec![ROLE_REGISTRY_REQUIRED_NON_CLAIM.to_owned()]
    }

    fn schema_row() -> StackSchemaRoleRegistryRow {
        StackSchemaRoleRegistryRow {
            registry_id: "valence.stack.schema.stack-provenance".to_owned(),
            schema_id: "valence.stack-provenance-sidecar.v1".to_owned(),
            role_name: "valence".to_owned(),
            claim_scope: "identity-and-linkage-only".to_owned(),
            owner_repo: "valence".to_owned(),
            allowed_consumers: vec!["valence".to_owned(), "mantle".to_owned(), "cairn".to_owned()],
            status: ACTIVE_STATUS.to_owned(),
            non_claims: non_claims(),
        }
    }

    fn role_vocab_rows() -> Vec<VerificationRoleVocabularyRow> {
        vec![
            VerificationRoleVocabularyRow {
                role_id: "property".to_owned(),
                evidence_tier: "formal_proof".to_owned(),
                counting: VerificationCounting::CountsAsProperty,
                display_label: "Property".to_owned(),
                non_claims: non_claims(),
            },
            VerificationRoleVocabularyRow {
                role_id: "recorded_only".to_owned(),
                evidence_tier: "manual_review_required".to_owned(),
                counting: VerificationCounting::RecordedOnly,
                display_label: "Recorded only".to_owned(),
                non_claims: non_claims(),
            },
            VerificationRoleVocabularyRow {
                role_id: "boundary".to_owned(),
                evidence_tier: "boundary".to_owned(),
                counting: VerificationCounting::BoundaryOnly,
                display_label: "Boundary".to_owned(),
                non_claims: non_claims(),
            },
            VerificationRoleVocabularyRow {
                role_id: "manual_review".to_owned(),
                evidence_tier: "manual_review_required".to_owned(),
                counting: VerificationCounting::ManualReviewOnly,
                display_label: "Manual review".to_owned(),
                non_claims: non_claims(),
            },
        ]
    }

    fn binding_row() -> SchemaRoleBindingRow {
        SchemaRoleBindingRow {
            schema_id: "valence.stack-provenance-sidecar.v1".to_owned(),
            allowed_roles: vec!["valence".to_owned(), "mantle".to_owned()],
            non_claims: non_claims(),
        }
    }

    fn stack_role_row() -> StackRoleRegistryRow {
        StackRoleRegistryRow {
            role_id: "valence".to_owned(),
            schema_id: "valence.stack-provenance-sidecar.v1".to_owned(),
            verification_role: "boundary".to_owned(),
            compatibility_class: "canonical".to_owned(),
            required: true,
            non_claims: non_claims(),
        }
    }

    fn alias_row() -> RoleAliasRow {
        RoleAliasRow {
            alias: "stack-provenance-trace".to_owned(),
            canonical_role: "valence".to_owned(),
            owner_repo: "mantle".to_owned(),
            status: LEGACY_COMPATIBLE_STATUS.to_owned(),
            non_claims: non_claims(),
        }
    }

    #[test]
    fn known_stack_schema_registry_rows_pass() {
        // r[verify valence.stack_schema_role_registry.fixtures.positive]
        let report = validate_stack_schema_role_registry(&[schema_row()]);

        assert!(report.valid);
        assert!(report.supported_claim.contains("literal drift only"));
    }

    #[test]
    fn invalid_stack_schema_registry_rows_fail_closed() {
        // r[verify valence.stack_schema_role_registry.fixtures.negative]
        let mut row = schema_row();
        row.role_name = "unknown".to_owned();
        row.allowed_consumers = vec!["unsupported".to_owned()];
        row.non_claims = vec!["proves evidence truth".to_owned()];

        let report = validate_stack_schema_role_registry(&[row]);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_role"));
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_consumer"));
        assert!(report.issues.iter().any(|issue| issue.code == "overclaim_detected"));
    }

    #[test]
    fn verification_role_vocabulary_preserves_counting_boundary() {
        // r[verify valence.evidence_ir.verification_role_vocabulary.fixtures.positive]
        let report = validate_verification_role_vocabulary(&role_vocab_rows());

        assert!(report.valid);
        assert!(report.supported_claim.contains("only configured property roles count"));
    }

    #[test]
    fn verification_role_counting_drift_fails_closed() {
        // r[verify valence.evidence_ir.verification_role_vocabulary.fixtures.negative]
        let mut row = role_vocab_rows().remove(0);
        row.counting = VerificationCounting::RecordedOnly;

        let report = validate_verification_role_vocabulary(&[row]);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "role_counting_drift"));
    }

    #[test]
    fn stack_role_registry_accepts_schema_role_and_alias_rows() {
        // r[verify valence.evidence_ir.stack_role_registry_core.fixtures.positive]
        let report = validate_stack_role_registry(&[stack_role_row()], &[binding_row()], &[alias_row()]);
        let alias = resolve_role_alias("stack-provenance-trace", &[alias_row()]).expect("alias should resolve");

        assert!(report.valid);
        assert_eq!(alias.canonical_role, "valence");
        assert!(alias.supported_claim.contains("compatibility only"));
    }

    #[test]
    fn stack_role_registry_rejects_role_schema_and_alias_failures() {
        // r[verify valence.evidence_ir.stack_role_registry_core.fixtures.negative]
        let duplicate = stack_role_row();
        let mut mismatched = stack_role_row();
        mismatched.role_id = "cairn".to_owned();
        mismatched.verification_role = "missing".to_owned();
        let mut stale_alias = alias_row();
        stale_alias.status = "stale".to_owned();

        let report =
            validate_stack_role_registry(&[duplicate.clone(), duplicate, mismatched], &[binding_row()], &[stale_alias]);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "duplicate_required_role"));
        assert!(report.issues.iter().any(|issue| issue.code == "schema_role_mismatch"));
        assert!(report.issues.iter().any(|issue| issue.code == "missing_verification_role"));
        assert!(report.issues.iter().any(|issue| issue.code == "stale_alias"));
    }

    #[test]
    fn consumer_constant_drift_validation_records_digest() {
        // r[verify valence.stack_schema_role_registry.constants.checked]
        let pass = ConsumerConstantRow {
            name: "VALENCE_STACK_ROLE".to_owned(),
            value: "valence".to_owned(),
            expected_value: "valence".to_owned(),
            registry_digest: REGISTRY_DIGEST.to_owned(),
        };
        let fail = ConsumerConstantRow {
            name: "MANTLE_STACK_ROLE".to_owned(),
            value: "mantle-old".to_owned(),
            expected_value: "mantle".to_owned(),
            registry_digest: String::new(),
        };

        let passing = validate_consumer_constants(&[pass]);
        let failing = validate_consumer_constants(&[fail]);

        assert!(passing.valid);
        assert!(!failing.valid);
        assert!(failing.issues.iter().any(|issue| issue.code == "constant_drift"));
        assert!(failing.issues.iter().any(|issue| issue.code == "registry_digest_missing"));
    }
}

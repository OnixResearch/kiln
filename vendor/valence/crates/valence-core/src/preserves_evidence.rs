//! Pure Preserves evidence identity and Preserves-to-Valence bridge validation.
//!
//! This module validates already-loaded Preserves values and metadata. It does
//! no I/O and grants no behavioral, authority, or release claims.

use serde::Deserialize;
use serde::Serialize;

use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::PreservesMetadataDigest;
use crate::shared_identity::blake3_hex;

const FIELD_ARTIFACT_ROLE: &str = "artifact_role";
const FIELD_CLAIM_BOUNDARY: &str = "claim_boundary";
const FIELD_CONTENT_HASH: &str = "preserves_content_hash";
const FIELD_EXCLUSION_PROFILE: &str = "self_hash_exclusion_profile";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_PRESERVES_TEXT: &str = "preserves_text";
const FIELD_SCHEMA: &str = "schema";
const FIELD_VERIFICATION_ROLE: &str = "verification_role";
const HASH_PREFIX: &str = "b3:";
const PACKED_LIST_TAG: u8 = 0x80;
const PACKED_STRING_TAG: u8 = 0xb1;
const PACKED_SYMBOL_TAG: u8 = 0xb0;

pub const PRESERVES_EVIDENCE_REQUIRED_NON_CLAIM: &str =
    "Preserves evidence validation proves only canonical Preserves content identity and bounded envelope compatibility";
pub const PRESERVES_BRIDGE_REQUIRED_NON_CLAIM: &str =
    "Preserves bridge validation proves only identity, schema, role, and non-claim mapping";
pub const CORRECTED_CANONICALIZATION_PROFILE: &str = "preserves-packed-structural-v2";
pub const CORRECTED_RECEIPT_EXCLUSION_PROFILE: &str = "receipt-self-hash-structural-v2";
pub const CORRECTED_CONTENT_EXCLUSION_PROFILE: &str = "content-hash-self-structural-v2";
pub const NO_SELF_HASH_EXCLUSION_PROFILE: &str = "none-structural-v2";
pub const LEGACY_CANONICALIZATION_PROFILE: &str = "preserves-text-v1";
pub const LEGACY_MIGRATION_PROFILE: &str = "preserves-integrity-migration-v1";
pub const REQUIRED_IDENTITY_BOUNDARY: &str = "valence.identity-linkage-only.v1";

const PRESERVES_SUPPORTED_CLAIM: &str = "Preserves evidence validation proves only canonical Preserves content identity and bounded envelope compatibility; it does not prove receipt truth, release eligibility, lifecycle readiness, semantic equivalence, behavioral correctness, verifier soundness, or whole-program safety";
const PRESERVES_BRIDGE_SUPPORTED_CLAIM: &str = "Preserves bridge validation proves identity, schema, role, and non-claim mapping only and does not prove Molten runtime authority, transport correctness, or Preserves application semantics";

const SUPPORTED_PRESERVES_SCHEMAS: &[&str] = &[
    "valence.preserves-evidence.v1",
    "cairn.preserves-receipt.v1",
    "mantle.release-evidence.preserves.v1",
    "octet.evidence-receipt.preserves.v1",
    "molten.preserves-envelope.v1",
];
const SUPPORTED_ARTIFACT_ROLES: &[&str] = &[
    "artifact_identity",
    "receipt_identity",
    "preserves_metadata",
    "compatibility_reference",
];
const SUPPORTED_VERIFICATION_ROLES: &[&str] = &["property", "recorded_only", "boundary"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservesEvidenceEnvelope {
    pub schema: String,
    pub canonicalization_profile: String,
    pub preserves_text: String,
    pub preserves_content_hash: String,
    pub self_hash_exclusion_profile: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservesBridgeRow {
    pub schema_label: String,
    pub canonical_bytes: Vec<u8>,
    pub preserves_content_hash: String,
    pub artifact_role: String,
    pub valence_artifact_kind: String,
    pub verification_role: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedClaimClass {
    Identity,
    Linkage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedVerificationRole {
    RecordedOnly,
    Boundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProhibitedCapability {
    BehavioralCorrectness,
    SemanticEquivalence,
    Authority,
    VerifierSoundness,
    ReleaseEligibility,
}

impl ProhibitedCapability {
    const fn phrase(self) -> &'static str {
        match self {
            Self::BehavioralCorrectness => "behavioral correctness",
            Self::SemanticEquivalence => "semantic equivalence",
            Self::Authority => "authority",
            Self::VerifierSoundness => "verifier soundness",
            Self::ReleaseEligibility => "release eligibility",
        }
    }
}

const PROHIBITED_CAPABILITIES: &[ProhibitedCapability] = &[
    ProhibitedCapability::BehavioralCorrectness,
    ProhibitedCapability::SemanticEquivalence,
    ProhibitedCapability::Authority,
    ProhibitedCapability::VerifierSoundness,
    ProhibitedCapability::ReleaseEligibility,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedClaimBoundary {
    pub claim_class: SupportedClaimClass,
    pub verification_role: TypedVerificationRole,
    pub required_boundary_ids: Vec<String>,
    pub asserted_capabilities: Vec<ProhibitedCapability>,
    pub display_metadata: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservesMigrationReceipt {
    pub migration_profile: String,
    pub original_bytes_hash: String,
    pub legacy_identity: String,
    pub corrected_identity: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PreservesEvidenceIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl PreservesEvidenceIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreservesEvidenceReport {
    pub valid: bool,
    pub normalized_content_hash: Option<String>,
    pub issues: Vec<PreservesEvidenceIssue>,
    pub supported_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreservesValue {
    Symbol(String),
    String(String),
    List(Vec<Self>),
}

struct Parser<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<PreservesValue, &'static str> {
        self.skip_space();
        let value = self.parse_value()?;
        self.skip_space();
        if self.cursor != self.input.len() {
            return Err("trailing_input");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<PreservesValue, &'static str> {
        match self.peek() {
            Some(b'(') => self.parse_list(),
            Some(b'\"') => self.parse_string(),
            Some(_) => self.parse_symbol(),
            None => Err("unexpected_end"),
        }
    }

    fn parse_list(&mut self) -> Result<PreservesValue, &'static str> {
        self.cursor += 1;
        let mut values = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                Some(b')') => {
                    self.cursor += 1;
                    return Ok(PreservesValue::List(values));
                }
                Some(_) => values.push(self.parse_value()?),
                None => return Err("unterminated_list"),
            }
        }
    }

    fn parse_string(&mut self) -> Result<PreservesValue, &'static str> {
        self.cursor += 1;
        let mut value = String::new();
        loop {
            let byte = self.take().ok_or("unterminated_string")?;
            match byte {
                b'\"' => return Ok(PreservesValue::String(value)),
                b'\\' => {
                    let escaped = self.take().ok_or("unterminated_escape")?;
                    match escaped {
                        b'\"' => value.push('\"'),
                        b'\\' => value.push('\\'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        _ => return Err("unsupported_escape"),
                    }
                }
                byte if byte.is_ascii_control() => return Err("control_in_string"),
                byte => value.push(char::from(byte)),
            }
        }
    }

    fn parse_symbol(&mut self) -> Result<PreservesValue, &'static str> {
        let start = self.cursor;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_whitespace() || matches!(byte, b'(' | b')' | b'\"') {
                break;
            }
            self.cursor += 1;
        }
        if start == self.cursor {
            return Err("empty_symbol");
        }
        let bytes = &self.input[start..self.cursor];
        let symbol = std::str::from_utf8(bytes).map_err(|_| "invalid_utf8")?;
        Ok(PreservesValue::Symbol(symbol.to_owned()))
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.cursor += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.cursor).copied()
    }

    fn take(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.cursor += 1;
        Some(byte)
    }
}

#[must_use]
pub fn validate_preserves_evidence(envelope: &PreservesEvidenceEnvelope) -> PreservesEvidenceReport {
    let mut issues = Vec::new();
    validate_schema(&envelope.schema, &mut issues);
    if envelope.canonicalization_profile != CORRECTED_CANONICALIZATION_PROFILE {
        issues.push(PreservesEvidenceIssue::new(FIELD_SCHEMA, "unsupported_canonicalization_profile"));
    }
    if !is_supported_exclusion_profile(&envelope.self_hash_exclusion_profile) {
        issues.push(PreservesEvidenceIssue::new(FIELD_EXCLUSION_PROFILE, "unknown_self_hash_exclusion_profile"));
    }
    validate_non_claim_metadata(&envelope.non_claims, PRESERVES_EVIDENCE_REQUIRED_NON_CLAIM, &mut issues);

    let expected = match canonical_preserves_bytes(&envelope.preserves_text, &envelope.self_hash_exclusion_profile) {
        Ok(bytes) => Some(blake3_hex(&bytes)),
        Err(code) => {
            issues.push(PreservesEvidenceIssue::new(FIELD_PRESERVES_TEXT, code));
            None
        }
    };
    validate_declared_hash(&envelope.preserves_content_hash, expected.as_deref(), &mut issues);
    finish_report(expected, issues, PRESERVES_SUPPORTED_CLAIM)
}

#[must_use]
pub fn validate_preserves_bridge(row: &PreservesBridgeRow) -> PreservesEvidenceReport {
    let mut issues = Vec::new();
    validate_schema(&row.schema_label, &mut issues);
    if row.valence_artifact_kind.trim().is_empty() {
        issues.push(PreservesEvidenceIssue::new(FIELD_SCHEMA, "artifact_kind_missing"));
    }
    if !SUPPORTED_ARTIFACT_ROLES.contains(&row.artifact_role.as_str()) {
        issues.push(PreservesEvidenceIssue::new(FIELD_ARTIFACT_ROLE, "unsupported_artifact_role"));
    }
    if !SUPPORTED_VERIFICATION_ROLES.contains(&row.verification_role.as_str()) {
        issues.push(PreservesEvidenceIssue::new(FIELD_VERIFICATION_ROLE, "unsupported_verification_role"));
    }
    validate_non_claim_metadata(&row.non_claims, PRESERVES_BRIDGE_REQUIRED_NON_CLAIM, &mut issues);
    let expected = Some(blake3_hex(&row.canonical_bytes));
    validate_declared_hash(&row.preserves_content_hash, expected.as_deref(), &mut issues);
    finish_report(expected, issues, PRESERVES_BRIDGE_SUPPORTED_CLAIM)
}

#[must_use]
pub fn preserves_content_hash(text: &str, exclusion_profile: &str) -> Option<String> {
    canonical_preserves_bytes(text, exclusion_profile).ok().as_deref().map(blake3_hex)
}

pub fn canonical_preserves_bytes(text: &str, exclusion_profile: &str) -> Result<Vec<u8>, &'static str> {
    if !is_supported_exclusion_profile(exclusion_profile) {
        return Err("unknown_self_hash_exclusion_profile");
    }
    let parsed = Parser::new(text).parse().map_err(|_| "malformed_preserves_structure")?;
    let transformed = remove_declared_self_hash_fields(parsed, exclusion_profile)?;
    let mut bytes = Vec::new();
    encode_packed(&transformed, &mut bytes);
    Ok(bytes)
}

fn remove_declared_self_hash_fields(value: PreservesValue, profile: &str) -> Result<PreservesValue, &'static str> {
    let excluded = match profile {
        CORRECTED_RECEIPT_EXCLUSION_PROFILE => &["receipt_hash", "preserves_content_hash"][..],
        CORRECTED_CONTENT_EXCLUSION_PROFILE => &["preserves_content_hash"][..],
        NO_SELF_HASH_EXCLUSION_PROFILE => &[][..],
        _ => return Err("unknown_self_hash_exclusion_profile"),
    };
    let PreservesValue::List(children) = value else {
        return Err("top_level_envelope_required");
    };
    if children.is_empty() {
        return Err("empty_envelope");
    }
    let mut retained = Vec::with_capacity(children.len());
    let mut removed: Vec<String> = Vec::new();
    for (index, child) in children.into_iter().enumerate() {
        let field = direct_field_name(&child).map(str::to_owned);
        if index > 0 && field.as_deref().is_some_and(|name| excluded.contains(&name)) {
            let name = field.expect("field was checked");
            if removed.contains(&name) {
                return Err("duplicate_self_hash_field");
            }
            removed.push(name);
        } else {
            if contains_nested_field(&child, excluded, index == 0) {
                return Err("misplaced_self_hash_field");
            }
            retained.push(child);
        }
    }
    Ok(PreservesValue::List(retained))
}

fn direct_field_name(value: &PreservesValue) -> Option<&str> {
    let PreservesValue::List(items) = value else {
        return None;
    };
    match items.first() {
        Some(PreservesValue::Symbol(name)) => Some(name),
        _ => None,
    }
}

fn contains_nested_field(value: &PreservesValue, excluded: &[&str], root_tag: bool) -> bool {
    match value {
        PreservesValue::List(items) => {
            if !root_tag && direct_field_name(value).is_some_and(|name| excluded.contains(&name)) {
                return true;
            }
            items.iter().any(|item| contains_nested_field(item, excluded, false))
        }
        PreservesValue::Symbol(_) | PreservesValue::String(_) => false,
    }
}

fn encode_packed(value: &PreservesValue, output: &mut Vec<u8>) {
    match value {
        PreservesValue::Symbol(symbol) => encode_atom(PACKED_SYMBOL_TAG, symbol.as_bytes(), output),
        PreservesValue::String(string) => encode_atom(PACKED_STRING_TAG, string.as_bytes(), output),
        PreservesValue::List(items) => {
            output.push(PACKED_LIST_TAG);
            encode_length(items.len(), output);
            for item in items {
                encode_packed(item, output);
            }
        }
    }
}

fn encode_atom(tag: u8, bytes: &[u8], output: &mut Vec<u8>) {
    output.push(tag);
    encode_length(bytes.len(), output);
    output.extend_from_slice(bytes);
}

fn encode_length(length: usize, output: &mut Vec<u8>) {
    output.extend_from_slice(&u64::try_from(length).expect("usize fits in u64").to_be_bytes());
}

#[must_use]
pub fn validate_typed_claim_boundary(boundary: &TypedClaimBoundary) -> Vec<PreservesEvidenceIssue> {
    let mut issues = Vec::new();
    if !boundary.required_boundary_ids.iter().any(|id| id == REQUIRED_IDENTITY_BOUNDARY) {
        issues.push(PreservesEvidenceIssue::new(FIELD_CLAIM_BOUNDARY, "required_boundary_missing"));
    }
    if !boundary.asserted_capabilities.is_empty() {
        issues.push(PreservesEvidenceIssue::new(FIELD_CLAIM_BOUNDARY, "prohibited_capability_asserted"));
    }
    let prose_overclaim = boundary.display_metadata.iter().any(|text| {
        let normalized = text.to_ascii_lowercase();
        PROHIBITED_CAPABILITIES.iter().any(|capability| normalized.contains(capability.phrase()))
    });
    if prose_overclaim {
        issues.push(PreservesEvidenceIssue::new(FIELD_CLAIM_BOUNDARY, "prohibited_capability_in_metadata"));
    }
    issues.sort();
    issues.dedup();
    issues
}

pub fn migrate_legacy_preserves_identity(
    original_bytes: &[u8],
    declared_legacy_identity: &str,
    exclusion_profile: &str,
) -> Result<PreservesMigrationReceipt, &'static str> {
    if original_bytes.is_empty() {
        return Err("original_bytes_missing");
    }
    let text = std::str::from_utf8(original_bytes).map_err(|_| "original_bytes_invalid_utf8")?;
    let legacy_identity = format!("{HASH_PREFIX}{}", legacy_rendered_line_hash(text, exclusion_profile)?);
    if declared_legacy_identity != legacy_identity {
        return Err("legacy_identity_mismatch");
    }
    let corrected = preserves_content_hash(text, exclusion_profile).ok_or("corrected_identity_unavailable")?;
    Ok(PreservesMigrationReceipt {
        migration_profile: LEGACY_MIGRATION_PROFILE.to_owned(),
        original_bytes_hash: format!("{HASH_PREFIX}{}", blake3_hex(original_bytes)),
        legacy_identity,
        corrected_identity: format!("{HASH_PREFIX}{corrected}"),
        non_claims: vec![PRESERVES_EVIDENCE_REQUIRED_NON_CLAIM.to_owned()],
    })
}

fn legacy_rendered_line_hash(text: &str, exclusion_profile: &str) -> Result<String, &'static str> {
    if !is_supported_exclusion_profile(exclusion_profile) {
        return Err("unknown_self_hash_exclusion_profile");
    }
    let material = if exclusion_profile == NO_SELF_HASH_EXCLUSION_PROFILE {
        text.trim().to_owned()
    } else {
        text.lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.contains("preserves_content_hash") && !trimmed.contains("receipt_hash")
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned()
    };
    Ok(blake3_hex(material.as_bytes()))
}

fn validate_declared_hash(declared: &str, expected: Option<&str>, issues: &mut Vec<PreservesEvidenceIssue>) {
    let parsed = match PreservesMetadataDigest::parse(declared, MIXED_BLAKE3_PROFILE, FIELD_CONTENT_HASH) {
        Ok(parsed) => parsed,
        Err(_) => {
            issues.push(PreservesEvidenceIssue::new(FIELD_CONTENT_HASH, "malformed_blake3"));
            return;
        }
    };
    let Some(expected) = expected else {
        issues.push(PreservesEvidenceIssue::new(FIELD_CONTENT_HASH, "hash_material_unavailable"));
        return;
    };
    if parsed.as_hex() != expected {
        issues.push(PreservesEvidenceIssue::new(FIELD_CONTENT_HASH, "profile_digest_mismatch"));
    }
}

fn validate_schema(schema: &str, issues: &mut Vec<PreservesEvidenceIssue>) {
    if !SUPPORTED_PRESERVES_SCHEMAS.contains(&schema) {
        issues.push(PreservesEvidenceIssue::new(FIELD_SCHEMA, "unsupported_schema"));
    }
}

fn is_supported_exclusion_profile(profile: &str) -> bool {
    matches!(
        profile,
        CORRECTED_RECEIPT_EXCLUSION_PROFILE | CORRECTED_CONTENT_EXCLUSION_PROFILE | NO_SELF_HASH_EXCLUSION_PROFILE
    )
}

fn validate_non_claim_metadata(non_claims: &[String], required: &str, issues: &mut Vec<PreservesEvidenceIssue>) {
    if non_claims.is_empty() {
        issues.push(PreservesEvidenceIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
    } else if !non_claims.iter().any(|claim| claim.contains(required)) {
        issues.push(PreservesEvidenceIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
}

fn finish_report(
    normalized_content_hash: Option<String>,
    mut issues: Vec<PreservesEvidenceIssue>,
    supported_claim: &str,
) -> PreservesEvidenceReport {
    issues.sort();
    issues.dedup();
    PreservesEvidenceReport {
        valid: issues.is_empty(),
        normalized_content_hash,
        issues,
        supported_claim: supported_claim.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TEXT: &str = "(envelope (schema \"valence.preserves-evidence.v1\") (payload \"ok\"))";

    fn envelope() -> PreservesEvidenceEnvelope {
        let hash = preserves_content_hash(VALID_TEXT, NO_SELF_HASH_EXCLUSION_PROFILE).unwrap();
        PreservesEvidenceEnvelope {
            schema: "valence.preserves-evidence.v1".to_owned(),
            canonicalization_profile: CORRECTED_CANONICALIZATION_PROFILE.to_owned(),
            preserves_text: VALID_TEXT.to_owned(),
            preserves_content_hash: format!("{HASH_PREFIX}{hash}"),
            self_hash_exclusion_profile: NO_SELF_HASH_EXCLUSION_PROFILE.to_owned(),
            non_claims: vec![PRESERVES_EVIDENCE_REQUIRED_NON_CLAIM.to_owned()],
        }
    }

    #[test]
    fn equivalent_renderings_have_equal_corrected_identity() {
        let compact = "(receipt (payload \"ok\") (receipt_hash \"stale\"))";
        let spaced = "( receipt\n ( payload \"ok\" )\n ( receipt_hash \"other\" ) )";
        assert_eq!(
            preserves_content_hash(compact, CORRECTED_RECEIPT_EXCLUSION_PROFILE),
            preserves_content_hash(spaced, CORRECTED_RECEIPT_EXCLUSION_PROFILE)
        );
    }

    #[test]
    fn payload_occurrences_of_self_hash_names_remain_hash_bearing() {
        let benign = "(receipt (payload \"receipt_hash benign\"))";
        let altered = "(receipt (payload \"receipt_hash attacker\"))";
        assert_ne!(
            preserves_content_hash(benign, CORRECTED_RECEIPT_EXCLUSION_PROFILE),
            preserves_content_hash(altered, CORRECTED_RECEIPT_EXCLUSION_PROFILE)
        );
    }

    #[test]
    fn duplicate_and_nested_self_hash_fields_fail_closed() {
        let duplicate = "(receipt (receipt_hash \"a\") (receipt_hash \"b\"))";
        let nested = "(receipt (payload (receipt_hash \"hidden\")))";
        assert_eq!(
            canonical_preserves_bytes(duplicate, CORRECTED_RECEIPT_EXCLUSION_PROFILE),
            Err("duplicate_self_hash_field")
        );
        assert_eq!(
            canonical_preserves_bytes(nested, CORRECTED_RECEIPT_EXCLUSION_PROFILE),
            Err("misplaced_self_hash_field")
        );
    }

    #[test]
    fn valid_envelope_passes_and_legacy_profile_fails() {
        let valid = envelope();
        assert!(validate_preserves_evidence(&valid).valid);
        let mut legacy = valid;
        legacy.canonicalization_profile = LEGACY_CANONICALIZATION_PROFILE.to_owned();
        assert!(!validate_preserves_evidence(&legacy).valid);
    }

    #[test]
    fn malformed_or_stale_envelope_fails_closed() {
        let mut candidate = envelope();
        candidate.preserves_text = "(unterminated".to_owned();
        candidate.preserves_content_hash = format!("{HASH_PREFIX}{}", "f".repeat(64));
        let report = validate_preserves_evidence(&candidate);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "malformed_preserves_structure"));
    }

    #[test]
    fn typed_identity_boundary_passes() {
        let boundary = TypedClaimBoundary {
            claim_class: SupportedClaimClass::Identity,
            verification_role: TypedVerificationRole::RecordedOnly,
            required_boundary_ids: vec![REQUIRED_IDENTITY_BOUNDARY.to_owned()],
            asserted_capabilities: Vec::new(),
            display_metadata: vec!["records canonical identity only".to_owned()],
        };
        assert!(validate_typed_claim_boundary(&boundary).is_empty());
    }

    #[test]
    fn typed_and_prose_overclaims_fail() {
        let boundary = TypedClaimBoundary {
            claim_class: SupportedClaimClass::Linkage,
            verification_role: TypedVerificationRole::Boundary,
            required_boundary_ids: Vec::new(),
            asserted_capabilities: vec![ProhibitedCapability::ReleaseEligibility],
            display_metadata: vec!["grants semantic equivalence".to_owned()],
        };
        let issues = validate_typed_claim_boundary(&boundary);
        assert!(issues.iter().any(|issue| issue.code == "required_boundary_missing"));
        assert!(issues.iter().any(|issue| issue.code == "prohibited_capability_asserted"));
        assert!(issues.iter().any(|issue| issue.code == "prohibited_capability_in_metadata"));
    }

    #[test]
    fn migration_binds_original_bytes_and_both_identities() {
        let text = "(receipt\n (payload \"ok\")\n (receipt_hash \"stale\")\n)";
        let legacy =
            format!("{HASH_PREFIX}{}", legacy_rendered_line_hash(text, CORRECTED_RECEIPT_EXCLUSION_PROFILE).unwrap());
        let receipt =
            migrate_legacy_preserves_identity(text.as_bytes(), &legacy, CORRECTED_RECEIPT_EXCLUSION_PROFILE).unwrap();
        assert_eq!(receipt.legacy_identity, legacy);
        assert_ne!(receipt.corrected_identity, receipt.legacy_identity);
        assert_eq!(receipt.original_bytes_hash, format!("{HASH_PREFIX}{}", blake3_hex(text.as_bytes())));
    }

    #[test]
    fn migration_without_bytes_or_with_wrong_identity_fails() {
        assert_eq!(
            migrate_legacy_preserves_identity(&[], "b3:none", CORRECTED_RECEIPT_EXCLUSION_PROFILE),
            Err("original_bytes_missing")
        );
        assert_eq!(
            migrate_legacy_preserves_identity(
                VALID_TEXT.as_bytes(),
                "b3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                NO_SELF_HASH_EXCLUSION_PROFILE,
            ),
            Err("legacy_identity_mismatch")
        );
    }

    #[test]
    fn bridge_validation_keeps_bounded_role_checks() {
        let bytes = canonical_preserves_bytes(VALID_TEXT, NO_SELF_HASH_EXCLUSION_PROFILE).unwrap();
        let row = PreservesBridgeRow {
            schema_label: "molten.preserves-envelope.v1".to_owned(),
            preserves_content_hash: blake3_hex(&bytes),
            canonical_bytes: bytes,
            artifact_role: "preserves_metadata".to_owned(),
            valence_artifact_kind: "preserves-envelope".to_owned(),
            verification_role: "recorded_only".to_owned(),
            non_claims: vec![PRESERVES_BRIDGE_REQUIRED_NON_CLAIM.to_owned()],
        };
        assert!(validate_preserves_bridge(&row).valid);
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::verified::evidence::{blake3_hex, canonical_json};

pub const CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1: &str =
    "cairn.content-bound-requirement-registry.v1";
pub const CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1: &str =
    "cairn.content-bound-requirement-registry-receipt.v1";
pub const CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY: &str = "content-bound requirement registry validation proves canonical identity, current-root freshness, and declared linkage only; it does not prove revision authenticity, requirement satisfaction, source correctness, implementation correctness, or release eligibility";
pub const CONTENT_BOUND_REVISION_KIND_GIT: &str = "git";
pub const CONTENT_BOUND_REVISION_KIND_JUJUTSU: &str = "jujutsu";
pub const CONTENT_BOUND_REVISION_KIND_OPAQUE: &str = "opaque";
pub const CONTENT_BOUND_REQUIREMENT_REGISTRY_NON_CLAIMS: &[&str] = &[
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
const REQUIREMENT_HEADING_LEVEL: usize = 3;
const REQUIREMENT_BODY_END_HEADING_LEVEL: usize = 4;
const REQUIREMENT_HEADING_PREFIX: &str = "Requirement:";
const MARKER_PREFIX: &str = "r[";
const MARKER_SUFFIX: u8 = b']';
const DOMAIN_SEPARATOR: u8 = 0;
const SPECIFICATION_DOMAIN: &str = "cairn.requirement-registry.specification.v1";
const SPECIFICATION_SET_DOMAIN: &str = "cairn.requirement-registry.specification-set.v1";
const POLICY_DOMAIN: &str = "cairn.requirement-registry.policy.v1";
const REQUIREMENT_BODY_DOMAIN: &str = "cairn.requirement-registry.requirement-body.v1";
const REQUIREMENT_ROW_DOMAIN: &str = "cairn.requirement-registry.row.v1";
const REGISTRY_DOMAIN: &str = "cairn.requirement-registry.registry.v1";
const RECEIPT_DOMAIN: &str = "cairn.requirement-registry.receipt.v1";
const ISSUE_SCHEMA_UNSUPPORTED: &str = "content_requirement_registry.schema_unsupported";
const ISSUE_RECEIPT_SCHEMA_UNSUPPORTED: &str =
    "content_requirement_registry.receipt_schema_unsupported";
const ISSUE_REPOSITORY_INVALID: &str = "content_requirement_registry.repository_invalid";
const ISSUE_REPOSITORY_MISMATCH: &str = "content_requirement_registry.repository_mismatch";
const ISSUE_REVISION_KIND_UNSUPPORTED: &str =
    "content_requirement_registry.revision_kind_unsupported";
const ISSUE_REVISION_INVALID: &str = "content_requirement_registry.revision_invalid";
const ISSUE_SPECIFICATION_PATH_UNSAFE: &str =
    "content_requirement_registry.specification_path_unsafe";
const ISSUE_SPECIFICATION_DUPLICATE: &str = "content_requirement_registry.specification_duplicate";
const ISSUE_SPECIFICATION_HASH_INVALID: &str =
    "content_requirement_registry.specification_hash_invalid";
const ISSUE_SPECIFICATION_STALE: &str = "content_requirement_registry.stale_specification";
const ISSUE_SPECIFICATION_MISSING: &str = "content_requirement_registry.specification_missing";
const ISSUE_SPECIFICATION_UNEXPECTED: &str =
    "content_requirement_registry.specification_unexpected";
const ISSUE_POLICY_HASH_INVALID: &str = "content_requirement_registry.policy_hash_invalid";
const ISSUE_POLICY_STALE: &str = "content_requirement_registry.stale_policy";
const ISSUE_REQUIREMENT_BODY_MISSING: &str =
    "content_requirement_registry.requirement_body_missing";
const ISSUE_REQUIREMENT_MARKER_MISSING: &str =
    "content_requirement_registry.requirement_marker_missing";
const ISSUE_REQUIREMENT_MARKER_DUPLICATE: &str =
    "content_requirement_registry.requirement_marker_duplicate";
const ISSUE_REQUIREMENT_ID_INVALID: &str = "content_requirement_registry.requirement_id_invalid";
const ISSUE_REQUIREMENT_ID_DUPLICATE: &str =
    "content_requirement_registry.requirement_id_duplicate";
const ISSUE_REQUIREMENT_STALE: &str = "content_requirement_registry.stale_requirement";
const ISSUE_REQUIREMENT_MISSING: &str = "content_requirement_registry.requirement_missing";
const ISSUE_REQUIREMENT_UNEXPECTED: &str = "content_requirement_registry.requirement_unexpected";
const ISSUE_ROW_HASH_INVALID: &str = "content_requirement_registry.row_hash_invalid";
const ISSUE_REGISTRY_HASH_INVALID: &str = "content_requirement_registry.registry_hash_invalid";
const ISSUE_RECEIPT_HASH_INVALID: &str = "content_requirement_registry.receipt_hash_invalid";
const ISSUE_RECEIPT_LINK_MISMATCH: &str = "content_requirement_registry.receipt_link_mismatch";
const ISSUE_NON_CLAIM_MISSING: &str = "content_requirement_registry.non_claim_missing";
const ISSUE_ORDER_INVALID: &str = "content_requirement_registry.order_invalid";
const ISSUE_SOURCE_SPAN_INVALID: &str = "content_requirement_registry.source_span_invalid";
const ISSUE_SERIALIZATION: &str = "content_requirement_registry.serialization_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundSpecificationInput {
    pub path: String,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundRevision {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundSpecificationIdentity {
    pub path: String,
    pub content_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundSourceSpan {
    pub start_byte: u64,
    pub end_byte: u64,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundRequirementRow {
    pub requirement_id: String,
    pub specification_path: String,
    pub specification_blake3: String,
    pub source_span: ContentBoundSourceSpan,
    pub requirement_body_blake3: String,
    pub row_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundRequirementRegistryReceipt {
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
pub struct ContentBoundRequirementRegistry {
    pub schema_version: String,
    pub boundary: String,
    pub repository_id: String,
    pub revision: ContentBoundRevision,
    pub specification_set_blake3: String,
    pub specifications: Vec<ContentBoundSpecificationIdentity>,
    pub policy_blake3: String,
    pub rows: Vec<ContentBoundRequirementRow>,
    pub non_claims: Vec<String>,
    pub registry_blake3: String,
    pub receipt: ContentBoundRequirementRegistryReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundRequirementRegistryIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundRequirementRegistryEvaluation {
    pub registry: Option<ContentBoundRequirementRegistry>,
    pub issues: Vec<ContentBoundRequirementRegistryIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundRequirementRegistryVerification {
    pub valid: bool,
    pub registry_blake3: String,
    pub receipt_blake3: String,
    pub repository_id: String,
    pub revision_kind: String,
    pub revision: String,
    pub specification_count: usize,
    pub row_count: usize,
    pub issues: Vec<ContentBoundRequirementRegistryIssue>,
    pub boundary: String,
}

#[derive(Serialize)]
struct RequirementRowHashMaterial<'a> {
    requirement_id: &'a str,
    specification_path: &'a str,
    specification_blake3: &'a str,
    source_span: &'a ContentBoundSourceSpan,
    requirement_body_blake3: &'a str,
}

#[derive(Serialize)]
struct RegistryHashMaterial<'a> {
    schema_version: &'a str,
    boundary: &'a str,
    repository_id: &'a str,
    revision: &'a ContentBoundRevision,
    specification_set_blake3: &'a str,
    specifications: &'a [ContentBoundSpecificationIdentity],
    policy_blake3: &'a str,
    rows: &'a [ContentBoundRequirementRow],
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct ReceiptHashMaterial<'a> {
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

#[derive(Debug, Clone, Copy)]
struct MarkdownLine<'a> {
    text: &'a str,
    start_byte: usize,
    end_byte: usize,
    line_number: usize,
}

#[must_use]
pub fn evaluate_content_bound_requirement_registry(
    mut specifications: Vec<ContentBoundSpecificationInput>,
    policy_bytes: &[u8],
    repository_id: &str,
    revision: ContentBoundRevision,
) -> ContentBoundRequirementRegistryEvaluation {
    // r[impl cairn.requirement_registry.content_bound_export]
    let mut issues = Vec::new();
    validate_repository_id(repository_id, "repository_id", &mut issues);
    validate_revision(&revision, "revision", &mut issues);
    specifications.sort_by(|left, right| left.path.cmp(&right.path));

    let mut seen_specification_paths = BTreeSet::new();
    let mut identities = Vec::with_capacity(specifications.len());
    let mut rows = Vec::new();
    for (index, specification) in specifications.iter().enumerate() {
        let field_path = format!("specifications[{index}].path");
        validate_specification_path(&specification.path, &field_path, &mut issues);
        if !seen_specification_paths.insert(specification.path.clone()) {
            push_issue(
                &mut issues,
                ISSUE_SPECIFICATION_DUPLICATE,
                &field_path,
                "accepted specification paths must be unique",
            );
        }
        let specification_blake3 =
            domain_hash_bytes(SPECIFICATION_DOMAIN, specification.markdown.as_bytes());
        identities.push(ContentBoundSpecificationIdentity {
            path: specification.path.clone(),
            content_blake3: specification_blake3.clone(),
        });
        rows.extend(parse_requirement_rows(
            specification,
            &specification_blake3,
            &mut issues,
        ));
    }

    rows.sort_by(|left, right| {
        left.requirement_id
            .cmp(&right.requirement_id)
            .then(left.specification_path.cmp(&right.specification_path))
    });
    if rows.is_empty() {
        push_issue(
            &mut issues,
            ISSUE_REQUIREMENT_MISSING,
            "rows",
            "registry must contain at least one accepted requirement row",
        );
    }
    reject_duplicate_requirement_ids(&rows, &mut issues);
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return ContentBoundRequirementRegistryEvaluation {
            registry: None,
            issues,
        };
    }

    let specification_set_blake3 = match domain_hash_json(SPECIFICATION_SET_DOMAIN, &identities) {
        Ok(hash) => hash,
        Err(issue) => {
            return ContentBoundRequirementRegistryEvaluation {
                registry: None,
                issues: vec![issue],
            }
        }
    };
    let policy_blake3 = domain_hash_bytes(POLICY_DOMAIN, policy_bytes);
    let non_claims = required_non_claims();
    let registry_blake3 = match registry_hash(
        repository_id,
        &revision,
        &specification_set_blake3,
        &identities,
        &policy_blake3,
        &rows,
        &non_claims,
    ) {
        Ok(hash) => hash,
        Err(issue) => {
            return ContentBoundRequirementRegistryEvaluation {
                registry: None,
                issues: vec![issue],
            }
        }
    };
    let receipt = match build_receipt(
        &registry_blake3,
        repository_id,
        &revision,
        &specification_set_blake3,
        &policy_blake3,
        rows.len(),
        &non_claims,
    ) {
        Ok(receipt) => receipt,
        Err(issue) => {
            return ContentBoundRequirementRegistryEvaluation {
                registry: None,
                issues: vec![issue],
            }
        }
    };
    let registry = ContentBoundRequirementRegistry {
        schema_version: CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1.to_string(),
        boundary: CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY.to_string(),
        repository_id: repository_id.to_string(),
        revision,
        specification_set_blake3,
        specifications: identities,
        policy_blake3,
        rows,
        non_claims,
        registry_blake3,
        receipt,
    };
    debug_assert!(!registry.rows.is_empty());
    ContentBoundRequirementRegistryEvaluation {
        registry: Some(registry),
        issues: Vec::new(),
    }
}

#[must_use]
pub fn verify_content_bound_requirement_registry(
    registry: &ContentBoundRequirementRegistry,
    specifications: Vec<ContentBoundSpecificationInput>,
    policy_bytes: &[u8],
    expected_repository_id: &str,
) -> ContentBoundRequirementRegistryVerification {
    // r[impl cairn.requirement_registry.current_root_verification]
    let mut issues = validate_content_bound_requirement_registry(registry);
    if registry.repository_id != expected_repository_id {
        push_issue(
            &mut issues,
            ISSUE_REPOSITORY_MISMATCH,
            "repository_id",
            "registry repository identity does not match the explicit expected repository",
        );
    }
    let expected = evaluate_content_bound_requirement_registry(
        specifications,
        policy_bytes,
        expected_repository_id,
        registry.revision.clone(),
    );
    issues.extend(expected.issues);
    if let Some(expected_registry) = expected.registry {
        compare_registry_freshness(registry, &expected_registry, &mut issues);
    }
    sort_issues(&mut issues);
    ContentBoundRequirementRegistryVerification {
        valid: issues.is_empty(),
        registry_blake3: registry.registry_blake3.clone(),
        receipt_blake3: registry.receipt.receipt_blake3.clone(),
        repository_id: registry.repository_id.clone(),
        revision_kind: registry.revision.kind.clone(),
        revision: registry.revision.value.clone(),
        specification_count: registry.specifications.len(),
        row_count: registry.rows.len(),
        issues,
        boundary: CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY.to_string(),
    }
}

#[must_use]
pub fn validate_content_bound_requirement_registry(
    registry: &ContentBoundRequirementRegistry,
) -> Vec<ContentBoundRequirementRegistryIssue> {
    // r[impl cairn.requirement_registry.downstream_boundary]
    let mut issues = Vec::new();
    if registry.schema_version != CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1 {
        push_issue(
            &mut issues,
            ISSUE_SCHEMA_UNSUPPORTED,
            "schema_version",
            "content-bound requirement registry schema version is unsupported",
        );
    }
    if registry.boundary != CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY {
        push_issue(
            &mut issues,
            ISSUE_NON_CLAIM_MISSING,
            "boundary",
            "registry boundary must preserve the reviewed identity-only claim",
        );
    }
    validate_repository_id(&registry.repository_id, "repository_id", &mut issues);
    validate_revision(&registry.revision, "revision", &mut issues);
    validate_non_claims(&registry.non_claims, "non_claims", &mut issues);
    validate_hash(
        &registry.specification_set_blake3,
        "specification_set_blake3",
        ISSUE_SPECIFICATION_HASH_INVALID,
        &mut issues,
    );
    validate_hash(
        &registry.policy_blake3,
        "policy_blake3",
        ISSUE_POLICY_HASH_INVALID,
        &mut issues,
    );
    validate_specification_identities(registry, &mut issues);
    validate_rows(registry, &mut issues);
    validate_registry_hash(registry, &mut issues);
    validate_receipt(registry, &mut issues);
    sort_issues(&mut issues);
    issues
}

pub fn parse_content_bound_requirement_registry_value(
    value: &Value,
) -> Result<ContentBoundRequirementRegistry, ContentBoundRequirementRegistryIssue> {
    serde_json::from_value(value.clone()).map_err(|error| {
        issue(
            ISSUE_SCHEMA_UNSUPPORTED,
            "registry",
            &format!("content-bound registry JSON does not match the closed schema: {error}"),
        )
    })
}

#[must_use]
pub fn content_bound_requirement_registry_issues_json(
    issues: &[ContentBoundRequirementRegistryIssue],
) -> Vec<Value> {
    issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "code": issue.code,
                "field_path": issue.field_path,
                "message": issue.message,
            })
        })
        .collect()
}

fn parse_requirement_rows(
    specification: &ContentBoundSpecificationInput,
    specification_blake3: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) -> Vec<ContentBoundRequirementRow> {
    let lines = markdown_lines(&specification.markdown);
    let requirement_headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let (level, heading) = markdown_heading(line.text)?;
            (level == REQUIREMENT_HEADING_LEVEL && heading.starts_with(REQUIREMENT_HEADING_PREFIX))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(requirement_headings.len());
    for (ordinal, heading_index) in requirement_headings.iter().copied().enumerate() {
        if let Some(row) = parse_requirement_row(
            specification,
            specification_blake3,
            &lines,
            heading_index,
            ordinal,
            issues,
        ) {
            rows.push(row);
        }
    }
    rows
}

fn parse_requirement_row(
    specification: &ContentBoundSpecificationInput,
    specification_blake3: &str,
    lines: &[MarkdownLine<'_>],
    heading_index: usize,
    ordinal: usize,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) -> Option<ContentBoundRequirementRow> {
    let row_path = format!("requirements[{ordinal}]");
    let block_end = next_heading_at_or_above(
        lines,
        heading_index.saturating_add(1),
        REQUIREMENT_HEADING_LEVEL,
    );
    let body_end = next_heading_at_or_above(
        lines,
        heading_index.saturating_add(1),
        REQUIREMENT_BODY_END_HEADING_LEVEL,
    )
    .min(block_end);
    let body_lines = trim_blank_lines(&lines[heading_index.saturating_add(1)..body_end]);
    if body_lines.is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIREMENT_BODY_MISSING,
            &format!("{row_path}.source_span"),
            "requirement body must contain a marker and normative text",
        );
        return None;
    }
    // r[impl cairn.trace_index.live_proof.declaration_marker]
    let mut identity_markers = declaration_markers(lines[heading_index].text)
        .into_iter()
        .filter(|marker| valid_requirement_id(marker))
        .collect::<Vec<_>>();
    if identity_markers.is_empty() {
        identity_markers = body_lines
            .iter()
            .find_map(|line| {
                let markers = declaration_markers(line.text)
                    .into_iter()
                    .filter(|marker| valid_requirement_id(marker))
                    .collect::<Vec<_>>();
                (!markers.is_empty()).then_some(markers)
            })
            .unwrap_or_default();
    }
    identity_markers.sort();
    identity_markers.dedup();
    if identity_markers.is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIREMENT_MARKER_MISSING,
            &format!("{row_path}.requirement_id"),
            "requirement declaration must contain exactly one bare requirement marker",
        );
        return None;
    }
    if identity_markers.len() != 1 {
        push_issue(
            issues,
            ISSUE_REQUIREMENT_MARKER_DUPLICATE,
            &format!("{row_path}.requirement_id"),
            "requirement declaration must not contain distinct bare requirement markers",
        );
        return None;
    }
    let requirement_id = identity_markers.remove(0);
    if !valid_requirement_id(&requirement_id) {
        push_issue(
            issues,
            ISSUE_REQUIREMENT_ID_INVALID,
            &format!("{row_path}.requirement_id"),
            "requirement ID has an invalid or unbounded spelling",
        );
        return None;
    }
    content_bound_requirement_row(
        specification,
        specification_blake3,
        &requirement_id,
        body_lines.first()?,
        body_lines.last()?,
        &row_path,
        issues,
    )
}

fn content_bound_requirement_row(
    specification: &ContentBoundSpecificationInput,
    specification_blake3: &str,
    requirement_id: &str,
    first_line: &MarkdownLine<'_>,
    last_line: &MarkdownLine<'_>,
    row_path: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) -> Option<ContentBoundRequirementRow> {
    let body = specification
        .markdown
        .get(first_line.start_byte..last_line.end_byte)?;
    let source_span = source_span(first_line, last_line, row_path, issues)?;
    let requirement_body_blake3 = domain_hash_bytes(REQUIREMENT_BODY_DOMAIN, body.as_bytes());
    let material = RequirementRowHashMaterial {
        requirement_id,
        specification_path: &specification.path,
        specification_blake3,
        source_span: &source_span,
        requirement_body_blake3: &requirement_body_blake3,
    };
    let row_blake3 = match domain_hash_json(REQUIREMENT_ROW_DOMAIN, &material) {
        Ok(hash) => hash,
        Err(issue) => {
            issues.push(issue);
            return None;
        }
    };
    Some(ContentBoundRequirementRow {
        requirement_id: requirement_id.to_string(),
        specification_path: specification.path.clone(),
        specification_blake3: specification_blake3.to_string(),
        source_span,
        requirement_body_blake3,
        row_blake3,
    })
}

fn source_span(
    first_line: &MarkdownLine<'_>,
    last_line: &MarkdownLine<'_>,
    row_path: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) -> Option<ContentBoundSourceSpan> {
    if first_line.start_byte >= last_line.end_byte || first_line.line_number > last_line.line_number
    {
        push_issue(
            issues,
            ISSUE_SOURCE_SPAN_INVALID,
            &format!("{row_path}.source_span"),
            "requirement source span must be non-empty and ordered",
        );
        return None;
    }
    let start_byte = match u64::try_from(first_line.start_byte) {
        Ok(value) => value,
        Err(_) => {
            push_issue(
                issues,
                ISSUE_SOURCE_SPAN_INVALID,
                &format!("{row_path}.source_span.start_byte"),
                "requirement source offset exceeds the portable integer range",
            );
            return None;
        }
    };
    let end_byte = match u64::try_from(last_line.end_byte) {
        Ok(value) => value,
        Err(_) => {
            push_issue(
                issues,
                ISSUE_SOURCE_SPAN_INVALID,
                &format!("{row_path}.source_span.end_byte"),
                "requirement source offset exceeds the portable integer range",
            );
            return None;
        }
    };
    let start_line = u64::try_from(first_line.line_number).ok()?;
    let end_line = u64::try_from(last_line.line_number).ok()?;
    Some(ContentBoundSourceSpan {
        start_byte,
        end_byte,
        start_line,
        end_line,
    })
}

fn markdown_lines(markdown: &str) -> Vec<MarkdownLine<'_>> {
    if markdown.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start_byte: usize = 0;
    for (index, segment) in markdown.split_inclusive('\n').enumerate() {
        let segment_end = start_byte.saturating_add(segment.len());
        let content = segment.strip_suffix('\n').unwrap_or(segment);
        let content = content.strip_suffix('\r').unwrap_or(content);
        let end_byte = start_byte.saturating_add(content.len());
        lines.push(MarkdownLine {
            text: content,
            start_byte,
            end_byte,
            line_number: index.saturating_add(1),
        });
        start_byte = segment_end;
    }
    lines
}

fn trim_blank_lines<'a>(lines: &'a [MarkdownLine<'a>]) -> &'a [MarkdownLine<'a>] {
    let start = lines
        .iter()
        .position(|line| !line.text.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.text.trim().is_empty())
        .map(|index| index.saturating_add(1))
        .unwrap_or(start);
    &lines[start..end]
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    let heading = trimmed.get(level..)?;
    if level == 0 || !heading.starts_with(' ') {
        return None;
    }
    let heading = heading.trim_start();
    (!heading.is_empty()).then_some((level, heading))
}

fn next_heading_at_or_above(lines: &[MarkdownLine<'_>], start: usize, level: usize) -> usize {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| {
            let (candidate_level, _heading) = markdown_heading(line.text)?;
            (candidate_level <= level).then_some(index)
        })
        .unwrap_or(lines.len())
}

fn declaration_markers(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut markers = Vec::new();
    let mut index: usize = 0;
    let mut code_delimiter: Option<u8> = None;
    while index.saturating_add(MARKER_PREFIX.len()) <= bytes.len() {
        if bytes[index] == b'`' {
            code_delimiter = match code_delimiter {
                Some(delimiter) if delimiter == bytes[index] => None,
                None => Some(bytes[index]),
                current => current,
            };
            index = index.saturating_add(1);
            continue;
        }
        if code_delimiter.is_none()
            && bytes.get(index..index.saturating_add(MARKER_PREFIX.len()))
                == Some(MARKER_PREFIX.as_bytes())
        {
            let content_start = index.saturating_add(MARKER_PREFIX.len());
            let Some(relative_end) = bytes[content_start..]
                .iter()
                .position(|byte| *byte == MARKER_SUFFIX)
            else {
                break;
            };
            let content_end = content_start.saturating_add(relative_end);
            if let Some(content) = text.get(content_start..content_end) {
                if !content.contains(char::is_whitespace) {
                    markers.push(content.to_string());
                }
            }
            index = content_end.saturating_add(1);
        } else {
            index = index.saturating_add(1);
        }
    }
    markers
}

fn valid_requirement_id(requirement_id: &str) -> bool {
    !requirement_id.is_empty()
        && requirement_id.len() <= MAX_REQUIREMENT_ID_BYTES
        && requirement_id.contains('.')
        && requirement_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn reject_duplicate_requirement_ids(
    rows: &[ContentBoundRequirementRow],
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    let mut seen = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        if !seen.insert(row.requirement_id.as_str()) {
            push_issue(
                issues,
                ISSUE_REQUIREMENT_ID_DUPLICATE,
                &format!("rows[{index}].requirement_id"),
                "accepted requirement IDs must be unique across the registry",
            );
        }
    }
}

fn registry_hash(
    repository_id: &str,
    revision: &ContentBoundRevision,
    specification_set_blake3: &str,
    specifications: &[ContentBoundSpecificationIdentity],
    policy_blake3: &str,
    rows: &[ContentBoundRequirementRow],
    non_claims: &[String],
) -> Result<String, ContentBoundRequirementRegistryIssue> {
    domain_hash_json(
        REGISTRY_DOMAIN,
        &RegistryHashMaterial {
            schema_version: CONTENT_BOUND_REQUIREMENT_REGISTRY_SCHEMA_V1,
            boundary: CONTENT_BOUND_REQUIREMENT_REGISTRY_BOUNDARY,
            repository_id,
            revision,
            specification_set_blake3,
            specifications,
            policy_blake3,
            rows,
            non_claims,
        },
    )
}

fn build_receipt(
    registry_blake3: &str,
    repository_id: &str,
    revision: &ContentBoundRevision,
    specification_set_blake3: &str,
    policy_blake3: &str,
    row_count: usize,
    non_claims: &[String],
) -> Result<ContentBoundRequirementRegistryReceipt, ContentBoundRequirementRegistryIssue> {
    let material = ReceiptHashMaterial {
        schema_version: CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1,
        registry_blake3,
        repository_id,
        revision_kind: &revision.kind,
        revision: &revision.value,
        specification_set_blake3,
        policy_blake3,
        row_count,
        non_claims,
    };
    let receipt_blake3 = domain_hash_json(RECEIPT_DOMAIN, &material)?;
    Ok(ContentBoundRequirementRegistryReceipt {
        schema_version: CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1.to_string(),
        registry_blake3: registry_blake3.to_string(),
        repository_id: repository_id.to_string(),
        revision_kind: revision.kind.clone(),
        revision: revision.value.clone(),
        specification_set_blake3: specification_set_blake3.to_string(),
        policy_blake3: policy_blake3.to_string(),
        row_count,
        non_claims: non_claims.to_vec(),
        receipt_blake3,
    })
}

fn validate_specification_identities(
    registry: &ContentBoundRequirementRegistry,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    let mut sorted = registry.specifications.clone();
    sorted.sort_by(|left, right| left.path.cmp(&right.path));
    if sorted != registry.specifications {
        push_issue(
            issues,
            ISSUE_ORDER_INVALID,
            "specifications",
            "specification identities must use canonical path order",
        );
    }
    let mut seen = BTreeSet::new();
    for (index, specification) in registry.specifications.iter().enumerate() {
        validate_specification_path(
            &specification.path,
            &format!("specifications[{index}].path"),
            issues,
        );
        validate_hash(
            &specification.content_blake3,
            &format!("specifications[{index}].content_blake3"),
            ISSUE_SPECIFICATION_HASH_INVALID,
            issues,
        );
        if !seen.insert(specification.path.as_str()) {
            push_issue(
                issues,
                ISSUE_SPECIFICATION_DUPLICATE,
                &format!("specifications[{index}].path"),
                "specification identity paths must be unique",
            );
        }
    }
    if let Ok(expected) = domain_hash_json(SPECIFICATION_SET_DOMAIN, &registry.specifications) {
        if registry.specification_set_blake3 != expected {
            push_issue(
                issues,
                ISSUE_SPECIFICATION_HASH_INVALID,
                "specification_set_blake3",
                "specification-set identity does not match canonical specification rows",
            );
        }
    }
}

fn validate_rows(
    registry: &ContentBoundRequirementRegistry,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    if registry.rows.is_empty() {
        push_issue(
            issues,
            ISSUE_REQUIREMENT_MISSING,
            "rows",
            "registry must contain at least one accepted requirement row",
        );
    }
    let mut sorted = registry.rows.clone();
    sorted.sort_by(|left, right| {
        left.requirement_id
            .cmp(&right.requirement_id)
            .then(left.specification_path.cmp(&right.specification_path))
    });
    if sorted != registry.rows {
        push_issue(
            issues,
            ISSUE_ORDER_INVALID,
            "rows",
            "requirement rows must use canonical requirement and path order",
        );
    }
    let specification_hashes = registry
        .specifications
        .iter()
        .map(|specification| {
            (
                specification.path.as_str(),
                specification.content_blake3.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for (index, row) in registry.rows.iter().enumerate() {
        let row_path = format!("rows[{index}]");
        if !valid_requirement_id(&row.requirement_id) {
            push_issue(
                issues,
                ISSUE_REQUIREMENT_ID_INVALID,
                &format!("{row_path}.requirement_id"),
                "requirement ID has an invalid or unbounded spelling",
            );
        }
        if !seen.insert(row.requirement_id.as_str()) {
            push_issue(
                issues,
                ISSUE_REQUIREMENT_ID_DUPLICATE,
                &format!("{row_path}.requirement_id"),
                "registry requirement IDs must be unique",
            );
        }
        validate_specification_path(
            &row.specification_path,
            &format!("{row_path}.specification_path"),
            issues,
        );
        validate_hash(
            &row.specification_blake3,
            &format!("{row_path}.specification_blake3"),
            ISSUE_SPECIFICATION_HASH_INVALID,
            issues,
        );
        validate_hash(
            &row.requirement_body_blake3,
            &format!("{row_path}.requirement_body_blake3"),
            ISSUE_REQUIREMENT_STALE,
            issues,
        );
        validate_hash(
            &row.row_blake3,
            &format!("{row_path}.row_blake3"),
            ISSUE_ROW_HASH_INVALID,
            issues,
        );
        if row.source_span.start_byte >= row.source_span.end_byte
            || row.source_span.start_line > row.source_span.end_line
        {
            push_issue(
                issues,
                ISSUE_SOURCE_SPAN_INVALID,
                &format!("{row_path}.source_span"),
                "requirement source span must be non-empty and ordered",
            );
        }
        match specification_hashes.get(row.specification_path.as_str()) {
            Some(expected_hash) if *expected_hash == row.specification_blake3 => {}
            Some(_) => push_issue(
                issues,
                ISSUE_SPECIFICATION_HASH_INVALID,
                &format!("{row_path}.specification_blake3"),
                "requirement row specification identity does not match its specification row",
            ),
            None => push_issue(
                issues,
                ISSUE_SPECIFICATION_MISSING,
                &format!("{row_path}.specification_path"),
                "requirement row references an absent specification identity",
            ),
        }
        let material = RequirementRowHashMaterial {
            requirement_id: &row.requirement_id,
            specification_path: &row.specification_path,
            specification_blake3: &row.specification_blake3,
            source_span: &row.source_span,
            requirement_body_blake3: &row.requirement_body_blake3,
        };
        if let Ok(expected_hash) = domain_hash_json(REQUIREMENT_ROW_DOMAIN, &material) {
            if row.row_blake3 != expected_hash {
                push_issue(
                    issues,
                    ISSUE_ROW_HASH_INVALID,
                    &format!("{row_path}.row_blake3"),
                    "requirement row identity does not match canonical row fields",
                );
            }
        }
    }
}

fn validate_registry_hash(
    registry: &ContentBoundRequirementRegistry,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    validate_hash(
        &registry.registry_blake3,
        "registry_blake3",
        ISSUE_REGISTRY_HASH_INVALID,
        issues,
    );
    let expected = registry_hash(
        &registry.repository_id,
        &registry.revision,
        &registry.specification_set_blake3,
        &registry.specifications,
        &registry.policy_blake3,
        &registry.rows,
        &registry.non_claims,
    );
    if expected
        .as_ref()
        .is_ok_and(|hash| hash != &registry.registry_blake3)
    {
        push_issue(
            issues,
            ISSUE_REGISTRY_HASH_INVALID,
            "registry_blake3",
            "registry identity does not match canonical registry fields",
        );
    }
}

fn validate_receipt(
    registry: &ContentBoundRequirementRegistry,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    let receipt = &registry.receipt;
    if receipt.schema_version != CONTENT_BOUND_REQUIREMENT_REGISTRY_RECEIPT_SCHEMA_V1 {
        push_issue(
            issues,
            ISSUE_RECEIPT_SCHEMA_UNSUPPORTED,
            "receipt.schema_version",
            "content-bound registry receipt schema version is unsupported",
        );
    }
    validate_non_claims(&receipt.non_claims, "receipt.non_claims", issues);
    validate_hash(
        &receipt.receipt_blake3,
        "receipt.receipt_blake3",
        ISSUE_RECEIPT_HASH_INVALID,
        issues,
    );
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
            ISSUE_RECEIPT_LINK_MISMATCH,
            "receipt",
            "registry receipt fields must match the complete registry identity",
        );
    }
    let material = ReceiptHashMaterial {
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
    if let Ok(expected_hash) = domain_hash_json(RECEIPT_DOMAIN, &material) {
        if receipt.receipt_blake3 != expected_hash {
            push_issue(
                issues,
                ISSUE_RECEIPT_HASH_INVALID,
                "receipt.receipt_blake3",
                "registry receipt identity does not match canonical receipt fields",
            );
        }
    }
}

fn compare_registry_freshness(
    actual: &ContentBoundRequirementRegistry,
    expected: &ContentBoundRequirementRegistry,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    if actual.policy_blake3 != expected.policy_blake3 {
        push_issue(
            issues,
            ISSUE_POLICY_STALE,
            "policy_blake3",
            "registry policy identity does not match the current generated policy",
        );
    }
    let actual_specs = actual
        .specifications
        .iter()
        .map(|row| (row.path.as_str(), row.content_blake3.as_str()))
        .collect::<BTreeMap<_, _>>();
    let expected_specs = expected
        .specifications
        .iter()
        .map(|row| (row.path.as_str(), row.content_blake3.as_str()))
        .collect::<BTreeMap<_, _>>();
    for (path, expected_hash) in &expected_specs {
        match actual_specs.get(path) {
            Some(actual_hash) if actual_hash == expected_hash => {}
            Some(_) => push_issue(
                issues,
                ISSUE_SPECIFICATION_STALE,
                &format!("specifications[{path}]"),
                "accepted specification identity is stale",
            ),
            None => push_issue(
                issues,
                ISSUE_SPECIFICATION_MISSING,
                &format!("specifications[{path}]"),
                "current accepted specification is absent from the registry",
            ),
        }
    }
    for path in actual_specs.keys() {
        if !expected_specs.contains_key(path) {
            push_issue(
                issues,
                ISSUE_SPECIFICATION_UNEXPECTED,
                &format!("specifications[{path}]"),
                "registry contains a specification absent from the current root",
            );
        }
    }
    let actual_rows = actual
        .rows
        .iter()
        .map(|row| (row.requirement_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let expected_rows = expected
        .rows
        .iter()
        .map(|row| (row.requirement_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    for (requirement_id, expected_row) in &expected_rows {
        match actual_rows.get(requirement_id) {
            Some(actual_row) if *actual_row == *expected_row => {}
            Some(_) => push_issue(
                issues,
                ISSUE_REQUIREMENT_STALE,
                &format!("rows[{requirement_id}]"),
                "requirement body or source identity is stale",
            ),
            None => push_issue(
                issues,
                ISSUE_REQUIREMENT_MISSING,
                &format!("rows[{requirement_id}]"),
                "current accepted requirement is absent from the registry",
            ),
        }
    }
    for requirement_id in actual_rows.keys() {
        if !expected_rows.contains_key(requirement_id) {
            push_issue(
                issues,
                ISSUE_REQUIREMENT_UNEXPECTED,
                &format!("rows[{requirement_id}]"),
                "registry contains a requirement absent from the current root",
            );
        }
    }
}

fn validate_repository_id(
    repository_id: &str,
    field_path: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    let valid = !repository_id.is_empty()
        && repository_id.len() <= MAX_REPOSITORY_ID_BYTES
        && repository_id.trim() == repository_id
        && repository_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        && !repository_id.starts_with('/')
        && !repository_id.split('/').any(|segment| segment == "..");
    if !valid {
        push_issue(
            issues,
            ISSUE_REPOSITORY_INVALID,
            field_path,
            "repository identity must be a bounded portable identifier",
        );
    }
}

fn validate_revision(
    revision: &ContentBoundRevision,
    field_path: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    match revision.kind.as_str() {
        CONTENT_BOUND_REVISION_KIND_GIT => {
            let valid_length = matches!(
                revision.value.len(),
                GIT_SHA1_HEX_LENGTH | GIT_SHA256_HEX_LENGTH
            );
            if !valid_length || !is_lower_hex(&revision.value) {
                push_issue(
                    issues,
                    ISSUE_REVISION_INVALID,
                    &format!("{field_path}.value"),
                    "Git revision must be a complete lowercase hexadecimal object identity",
                );
            }
        }
        CONTENT_BOUND_REVISION_KIND_JUJUTSU => {
            let valid_length = (JUJUTSU_REVISION_MIN_HEX_LENGTH..=JUJUTSU_REVISION_MAX_HEX_LENGTH)
                .contains(&revision.value.len());
            if !valid_length || !is_lower_hex(&revision.value) {
                push_issue(
                    issues,
                    ISSUE_REVISION_INVALID,
                    &format!("{field_path}.value"),
                    "Jujutsu revision must be a bounded lowercase hexadecimal commit identity",
                );
            }
        }
        CONTENT_BOUND_REVISION_KIND_OPAQUE => {
            let valid = !revision.value.is_empty()
                && revision.value.len() <= MAX_OPAQUE_REVISION_BYTES
                && revision.value.trim() == revision.value
                && !revision.value.chars().any(char::is_control);
            if !valid {
                push_issue(
                    issues,
                    ISSUE_REVISION_INVALID,
                    &format!("{field_path}.value"),
                    "opaque revision must be non-empty, bounded, trimmed, and free of control characters",
                );
            }
        }
        _ => push_issue(
            issues,
            ISSUE_REVISION_KIND_UNSUPPORTED,
            &format!("{field_path}.kind"),
            "revision kind must be git, jujutsu, or opaque",
        ),
    }
}

fn validate_specification_path(
    path: &str,
    field_path: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    let components = path.split('/').collect::<Vec<_>>();
    let valid = !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && components
            .iter()
            .all(|component| !component.is_empty() && *component != "." && *component != "..")
        && path.starts_with(".cairn/specs/")
        && path.ends_with("/spec.md");
    if !valid {
        push_issue(
            issues,
            ISSUE_SPECIFICATION_PATH_UNSAFE,
            field_path,
            "specification path must be a safe repository-relative accepted-spec path",
        );
    }
}

fn validate_non_claims(
    non_claims: &[String],
    field_path: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    for required in CONTENT_BOUND_REQUIREMENT_REGISTRY_NON_CLAIMS {
        if !non_claims.iter().any(|claim| claim == required) {
            push_issue(
                issues,
                ISSUE_NON_CLAIM_MISSING,
                field_path,
                &format!("registry non-claims must include `{required}`"),
            );
        }
    }
}

fn validate_hash(
    hash: &str,
    field_path: &str,
    code: &str,
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
) {
    if hash.len() != BLAKE3_HEX_LENGTH || !is_lower_hex(hash) {
        push_issue(
            issues,
            code,
            field_path,
            "BLAKE3 identity must be 64 lowercase hexadecimal characters",
        );
    }
}

fn is_lower_hex(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn required_non_claims() -> Vec<String> {
    CONTENT_BOUND_REQUIREMENT_REGISTRY_NON_CLAIMS
        .iter()
        .map(|claim| (*claim).to_string())
        .collect()
}

fn domain_hash_bytes(domain: &str, bytes: &[u8]) -> String {
    let mut material =
        Vec::with_capacity(domain.len().saturating_add(1).saturating_add(bytes.len()));
    material.extend_from_slice(domain.as_bytes());
    material.push(DOMAIN_SEPARATOR);
    material.extend_from_slice(bytes);
    blake3_hex(&material)
}

fn domain_hash_json<T: Serialize>(
    domain: &str,
    value: &T,
) -> Result<String, ContentBoundRequirementRegistryIssue> {
    let value = serde_json::to_value(value).map_err(|_error| {
        issue(
            ISSUE_SERIALIZATION,
            "hash_material",
            "canonical registry hash material could not be serialized",
        )
    })?;
    Ok(domain_hash_bytes(domain, canonical_json(&value).as_bytes()))
}

fn issue(code: &str, field_path: &str, message: &str) -> ContentBoundRequirementRegistryIssue {
    ContentBoundRequirementRegistryIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<ContentBoundRequirementRegistryIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(issue(code, field_path, message));
}

fn sort_issues(issues: &mut Vec<ContentBoundRequirementRegistryIssue>) {
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

    const REPOSITORY_ID: &str = "OnixResearch/example";
    const REVISION_VALUE: &str = "reviewed-revision-1";
    const POLICY_BYTES: &[u8] = br#"{"name":"fixture-policy"}"#;
    const SPEC_PATH: &str = ".cairn/specs/example/spec.md";
    const REQUIREMENT_ID: &str = "example.registry.current";
    const SECOND_REQUIREMENT_ID: &str = "example.registry.second";

    fn revision() -> ContentBoundRevision {
        ContentBoundRevision {
            kind: CONTENT_BOUND_REVISION_KIND_OPAQUE.to_string(),
            value: REVISION_VALUE.to_string(),
        }
    }

    fn specification(requirement_id: &str, body: &str) -> ContentBoundSpecificationInput {
        ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Example Specification\n\n## Requirements\n\n### Requirement: Current identity\n{MARKER_PREFIX}{requirement_id}] {body}\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST pass\n"
            ),
        }
    }

    fn evaluated_registry(
        specification: ContentBoundSpecificationInput,
        policy_bytes: &[u8],
        repository_id: &str,
        revision: ContentBoundRevision,
    ) -> ContentBoundRequirementRegistry {
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![specification],
            policy_bytes,
            repository_id,
            revision,
        );
        assert!(
            evaluation.issues.is_empty(),
            "issues: {:?}",
            evaluation.issues
        );
        evaluation.registry.expect("valid registry")
    }

    fn registry() -> ContentBoundRequirementRegistry {
        evaluated_registry(
            specification(REQUIREMENT_ID, "The registry MUST bind this body."),
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        )
    }

    fn issue_codes(issues: &[ContentBoundRequirementRegistryIssue]) -> Vec<&str> {
        issues.iter().map(|issue| issue.code.as_str()).collect()
    }

    #[test]
    fn deterministic_export_sorts_rows_and_binds_all_identity_domains() {
        // r[verify cairn.requirement_registry.content_bound_export]
        let first = evaluate_content_bound_requirement_registry(
            vec![specification(
                REQUIREMENT_ID,
                "The registry MUST bind this body.",
            )],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        let second = evaluate_content_bound_requirement_registry(
            vec![specification(
                REQUIREMENT_ID,
                "The registry MUST bind this body.",
            )],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(first.issues.is_empty(), "issues: {:?}", first.issues);
        assert_eq!(first.registry, second.registry);
        let registry = first.registry.expect("registry");
        assert_eq!(registry.rows.len(), 1);
        assert_eq!(registry.rows[0].requirement_id, REQUIREMENT_ID);
        assert_eq!(registry.registry_blake3.len(), BLAKE3_HEX_LENGTH);
        assert_eq!(registry.receipt.receipt_blake3.len(), BLAKE3_HEX_LENGTH);
        assert_eq!(registry.receipt.registry_blake3, registry.registry_blake3);
        assert!(validate_content_bound_requirement_registry(&registry).is_empty());
    }

    #[test]
    fn every_bound_input_changes_the_expected_identities() {
        // r[verify cairn.requirement_registry.content_bound_export]
        let baseline = registry();
        let changed_repository = evaluated_registry(
            specification(REQUIREMENT_ID, "The registry MUST bind this body."),
            POLICY_BYTES,
            "OnixResearch/other",
            revision(),
        );
        assert_eq!(
            changed_repository.rows[0].row_blake3,
            baseline.rows[0].row_blake3
        );
        assert_ne!(changed_repository.registry_blake3, baseline.registry_blake3);
        assert_ne!(
            changed_repository.receipt.receipt_blake3,
            baseline.receipt.receipt_blake3
        );

        let changed_revision = evaluated_registry(
            specification(REQUIREMENT_ID, "The registry MUST bind this body."),
            POLICY_BYTES,
            REPOSITORY_ID,
            ContentBoundRevision {
                kind: CONTENT_BOUND_REVISION_KIND_OPAQUE.to_string(),
                value: "reviewed-revision-2".to_string(),
            },
        );
        assert_eq!(
            changed_revision.rows[0].row_blake3,
            baseline.rows[0].row_blake3
        );
        assert_ne!(changed_revision.registry_blake3, baseline.registry_blake3);
        assert_ne!(
            changed_revision.receipt.receipt_blake3,
            baseline.receipt.receipt_blake3
        );

        let changed_policy = evaluated_registry(
            specification(REQUIREMENT_ID, "The registry MUST bind this body."),
            br#"{"name":"changed-policy"}"#,
            REPOSITORY_ID,
            revision(),
        );
        assert_eq!(
            changed_policy.rows[0].row_blake3,
            baseline.rows[0].row_blake3
        );
        assert_ne!(changed_policy.policy_blake3, baseline.policy_blake3);
        assert_ne!(changed_policy.registry_blake3, baseline.registry_blake3);
        assert_ne!(
            changed_policy.receipt.receipt_blake3,
            baseline.receipt.receipt_blake3
        );

        let changed_body = evaluated_registry(
            specification(REQUIREMENT_ID, "The registry MUST bind another body."),
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert_ne!(
            changed_body.specification_set_blake3,
            baseline.specification_set_blake3
        );
        assert_ne!(
            changed_body.rows[0].requirement_body_blake3,
            baseline.rows[0].requirement_body_blake3
        );
        assert_ne!(changed_body.rows[0].row_blake3, baseline.rows[0].row_blake3);
        assert_ne!(changed_body.registry_blake3, baseline.registry_blake3);
        assert_ne!(
            changed_body.receipt.receipt_blake3,
            baseline.receipt.receipt_blake3
        );

        let mut changed_path_input =
            specification(REQUIREMENT_ID, "The registry MUST bind this body.");
        changed_path_input.path = ".cairn/specs/renamed/spec.md".to_string();
        let changed_path =
            evaluated_registry(changed_path_input, POLICY_BYTES, REPOSITORY_ID, revision());
        assert_eq!(
            changed_path.specifications[0].content_blake3,
            baseline.specifications[0].content_blake3
        );
        assert_ne!(
            changed_path.specification_set_blake3,
            baseline.specification_set_blake3
        );
        assert_ne!(changed_path.rows[0].row_blake3, baseline.rows[0].row_blake3);
        assert_ne!(changed_path.registry_blake3, baseline.registry_blake3);
        assert_ne!(
            changed_path.receipt.receipt_blake3,
            baseline.receipt.receipt_blake3
        );
    }

    #[test]
    fn duplicate_requirement_ids_fail_without_a_registry() {
        // r[verify cairn.requirement_registry.content_bound_export]
        let mut first = specification(REQUIREMENT_ID, "The registry MUST bind this body.");
        first.path = ".cairn/specs/first/spec.md".to_string();
        let mut second = specification(REQUIREMENT_ID, "The registry MUST bind another body.");
        second.path = ".cairn/specs/second/spec.md".to_string();
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![second, first],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(evaluation.registry.is_none());
        assert!(issue_codes(&evaluation.issues).contains(&ISSUE_REQUIREMENT_ID_DUPLICATE));
    }

    #[test]
    fn malformed_revision_and_unsafe_path_fail_closed() {
        // r[verify cairn.requirement_registry.current_root_verification]
        let mut input = specification(REQUIREMENT_ID, "The registry MUST bind this body.");
        input.path = "../outside/spec.md".to_string();
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![input],
            POLICY_BYTES,
            REPOSITORY_ID,
            ContentBoundRevision {
                kind: CONTENT_BOUND_REVISION_KIND_GIT.to_string(),
                value: "not-a-git-object".to_string(),
            },
        );
        let codes = issue_codes(&evaluation.issues);
        assert!(evaluation.registry.is_none());
        assert!(codes.contains(&ISSUE_REVISION_INVALID));
        assert!(codes.contains(&ISSUE_SPECIFICATION_PATH_UNSAFE));
    }

    #[test]
    fn current_registry_verifies_and_stale_body_fails() {
        // r[verify cairn.requirement_registry.current_root_verification]
        let registry = registry();
        let current = verify_content_bound_requirement_registry(
            &registry,
            vec![specification(
                REQUIREMENT_ID,
                "The registry MUST bind this body.",
            )],
            POLICY_BYTES,
            REPOSITORY_ID,
        );
        assert!(current.valid, "issues: {:?}", current.issues);

        let stale = verify_content_bound_requirement_registry(
            &registry,
            vec![specification(
                REQUIREMENT_ID,
                "The registry MUST bind the changed body.",
            )],
            POLICY_BYTES,
            REPOSITORY_ID,
        );
        assert!(!stale.valid);
        assert!(issue_codes(&stale.issues).contains(&ISSUE_REQUIREMENT_STALE));
        assert!(stale
            .issues
            .iter()
            .all(|issue| !issue.message.contains("changed body")));
    }

    #[test]
    fn wrong_repository_policy_drift_and_tampered_row_fail_closed() {
        // r[verify cairn.requirement_registry.current_root_verification]
        let mut registry = registry();
        registry.rows[0].requirement_body_blake3 =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        let report = verify_content_bound_requirement_registry(
            &registry,
            vec![specification(
                REQUIREMENT_ID,
                "The registry MUST bind this body.",
            )],
            br#"{"name":"changed-policy"}"#,
            "OnixResearch/other",
        );
        let codes = issue_codes(&report.issues);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_REPOSITORY_MISMATCH));
        assert!(codes.contains(&ISSUE_POLICY_STALE));
        assert!(codes.contains(&ISSUE_ROW_HASH_INVALID));
        assert!(codes.contains(&ISSUE_REQUIREMENT_STALE));
    }

    #[test]
    fn rows_sort_by_requirement_id_not_source_order() {
        let input = ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Example\n\n## Requirements\n\n### Requirement: Second\nr[{SECOND_REQUIREMENT_ID}] The system MUST keep the second row.\n\n#### Scenario: Second\n- GIVEN input\n- WHEN ordered\n- THEN output MUST pass\n\n### Requirement: First\nr[{REQUIREMENT_ID}] The system MUST keep the first row.\n\n#### Scenario: First\n- GIVEN input\n- WHEN ordered\n- THEN output MUST pass\n"
            ),
        };
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![input],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(
            evaluation.issues.is_empty(),
            "issues: {:?}",
            evaluation.issues
        );
        let rows = evaluation.registry.expect("registry").rows;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].requirement_id, REQUIREMENT_ID);
        assert_eq!(rows[1].requirement_id, SECOND_REQUIREMENT_ID);
    }

    #[test]
    fn declaration_marker_owns_identity_when_later_prose_contains_markers() {
        let input = ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Example\n\n## Requirements\n\n### Requirement: Declaration identity\nr[{REQUIREMENT_ID}] The registry MUST use this declaration.\n\nLater prose can discuss r[{SECOND_REQUIREMENT_ID}] without granting identity authority.\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST pass\n"
            ),
        };
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![input],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(
            evaluation.issues.is_empty(),
            "issues: {:?}",
            evaluation.issues
        );
        let row = &evaluation.registry.expect("registry").rows[0];
        assert_eq!(row.requirement_id, REQUIREMENT_ID);
    }

    #[test]
    fn declaration_line_ignores_marker_syntax_inside_inline_code() {
        let input = ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Example\n\n## Requirements\n\n### Requirement: Marker syntax\nr[{REQUIREMENT_ID}] The registry MUST validate `r[...]` references relative to the declaration.\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST pass\n"
            ),
        };
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![input],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(
            evaluation.issues.is_empty(),
            "issues: {:?}",
            evaluation.issues
        );
        let row = &evaluation.registry.expect("registry").rows[0];
        assert_eq!(row.requirement_id, REQUIREMENT_ID);
    }

    #[test]
    fn declaration_marker_can_follow_normative_text_or_appear_in_the_heading() {
        let input = ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Example\n\n## Requirements\n\n### Requirement: Trailing declaration\nThe registry MUST accept a trailing declaration. r[{SECOND_REQUIREMENT_ID}]\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST pass\n\n### Requirement: Heading declaration r[{REQUIREMENT_ID}]\nThe registry MUST accept a heading declaration.\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST pass\n"
            ),
        };
        let evaluation = evaluate_content_bound_requirement_registry(
            vec![input],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(
            evaluation.issues.is_empty(),
            "issues: {:?}",
            evaluation.issues
        );
        let rows = evaluation.registry.expect("registry").rows;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].requirement_id, REQUIREMENT_ID);
        assert_eq!(rows[1].requirement_id, SECOND_REQUIREMENT_ID);
    }

    #[test]
    fn distinct_declaration_markers_and_late_only_markers_fail_closed() {
        // r[verify cairn.trace_index.live_proof.declaration_marker]
        // r[verify cairn.trace_index.live_proof.repository_check]
        // r[verify cairn.trace_index.live_proof.receipts]
        // r[verify cairn.trace_index.live_proof.validation]
        // r[verify cairn.trace_index.live_proof.docs]
        let conflicting = ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Example\n\n## Requirements\n\n### Requirement: Conflict\nr[{REQUIREMENT_ID}] r[{SECOND_REQUIREMENT_ID}] The registry MUST reject distinct declarations.\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST fail\n"
            ),
        };
        let conflict = evaluate_content_bound_requirement_registry(
            vec![conflicting],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(conflict.registry.is_none());
        assert!(issue_codes(&conflict.issues).contains(&ISSUE_REQUIREMENT_MARKER_DUPLICATE));

        let late_only = ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: "# Example\n\n## Requirements\n\n### Requirement: Missing declaration\nThe registry MUST reject a missing declaration marker.\n\nLater prose has no marker.\n\n#### Scenario: Current\n- GIVEN a current source\n- WHEN validation runs\n- THEN the result MUST fail\n".to_string(),
        };
        let missing = evaluate_content_bound_requirement_registry(
            vec![late_only],
            POLICY_BYTES,
            REPOSITORY_ID,
            revision(),
        );
        assert!(missing.registry.is_none());
        assert!(issue_codes(&missing.issues).contains(&ISSUE_REQUIREMENT_MARKER_MISSING));
    }
}

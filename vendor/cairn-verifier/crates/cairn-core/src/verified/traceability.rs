use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::verified::requirement_id_registry::{
    validate_requirement_id_registry, RequirementIdRegistry, RequirementReference,
};

pub const TRACEABILITY_PROFILE_CAIRN_DEFAULT: &str = "cairn-default";
pub const TRACEABILITY_PROFILE_TRELLIS_RUST: &str = "trellis-rust";
pub const TRACEABILITY_VERB_IMPL: &str = "impl";
pub const TRACEABILITY_VERB_VERIFY: &str = "verify";
pub const TRACEABILITY_VERB_DEPENDS: &str = "depends";
pub const TRACEABILITY_VERB_RELATED: &str = "related";
pub const TRACEABILITY_MARKER_VERB_REQUIREMENT: &str = "requirement";
pub const TRACEABILITY_COVERAGE_VERBS: &[&str] =
    &[TRACEABILITY_VERB_IMPL, TRACEABILITY_VERB_VERIFY];
pub const TRACEABILITY_MARKER_CORE_BOUNDARY: &str = "traceability marker validation proves only marker syntax, vocabulary conformance, and declared linkage health; it does not prove requirement satisfaction, implementation correctness, verification coverage, proof soundness, release eligibility, lifecycle readiness, or downstream behavioral correctness";
pub const TRACEABILITY_MARKER_NON_CLAIMS: &[&str] = &[
    "not requirement satisfaction",
    "not implementation correctness",
    "not verification coverage",
    "not proof soundness",
    "not release eligibility",
];

const TRACEABILITY_MARKER_PREFIX: &str = "r[";
const TRACEABILITY_MARKER_SUFFIX: char = ']';
const TRACEABILITY_ID_EXTENSION_SEPARATOR: char = '+';
const TRACEABILITY_LINE_NUMBER_ORIGIN: usize = 1;
const TRACEABILITY_PLACEHOLDER_ID: &str = "...";
const TRACEABILITY_MARKER_BODY_MAX_PARTS: usize = 2;
const TRACEABILITY_MARKER_REGISTRY_HASH: &str = "in-memory-traceability-marker-registry";
const TRACEABILITY_MARKER_ISSUE_MISSING: &str = "traceability_marker.missing_marker";
const TRACEABILITY_MARKER_ISSUE_MALFORMED: &str = "traceability_marker.malformed";
const TRACEABILITY_MARKER_ISSUE_UNSUPPORTED_VERB: &str = "traceability_marker.unsupported_verb";
const TRACEABILITY_MARKER_ISSUE_MALFORMED_ID: &str = "traceability_marker.malformed_id";
const TRACEABILITY_MARKER_ISSUE_WRONG_NAMESPACE: &str = "traceability_marker.wrong_namespace";
const TRACEABILITY_MARKER_ISSUE_DUPLICATE: &str = "traceability_marker.duplicate_link";
const TRACEABILITY_MARKER_ISSUE_MISSING_REGISTRY: &str = "traceability_marker.missing_registry";
const TRACEABILITY_MARKER_ISSUE_OVERCLAIM: &str = "traceability_marker.overclaim";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceabilityProfile {
    pub id: String,
    pub requirement_sources: Vec<TraceabilitySource>,
    pub evidence_sources: Vec<TraceabilitySource>,
    pub marker_verbs: Vec<String>,
    pub assurance_level: String,
    pub anchor_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceabilitySource {
    pub root: String,
    pub source_kind: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityFile {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityRequirementOccurrence {
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityReferenceOccurrence {
    pub verb: String,
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityReferenceMarker {
    pub verb: String,
    pub requirement_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityMarkerLink {
    pub raw: String,
    pub verb: String,
    pub requirement_id: String,
    pub consumer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityMarkerIssue {
    pub code: String,
    pub field_path: String,
    pub marker: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityMarkerValidationInput {
    pub marker_strings: Vec<String>,
    pub vocabulary: Vec<String>,
    pub allowed_namespaces: Vec<String>,
    pub registry: Option<RequirementIdRegistry>,
    pub consumer: String,
    pub expected_owner: String,
    pub require_registry: bool,
    pub allow_duplicate_links: bool,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityMarkerValidationReport {
    pub valid: bool,
    pub links: Vec<TraceabilityMarkerLink>,
    pub issues: Vec<TraceabilityMarkerIssue>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityCoverageReport {
    pub requirements: BTreeMap<String, Vec<TraceabilityRequirementOccurrence>>,
    pub coverage_references: BTreeMap<String, Vec<TraceabilityReferenceOccurrence>>,
    pub all_references: BTreeMap<String, Vec<TraceabilityReferenceOccurrence>>,
    pub missing: Vec<String>,
    pub dangling: Vec<String>,
    pub requirement_count: usize,
    pub referenced_count: usize,
    pub valid: bool,
}

pub fn classify_traceability_coverage(
    profile: &TraceabilityProfile,
    requirement_files: &[TraceabilityFile],
    evidence_files: &[TraceabilityFile],
) -> TraceabilityCoverageReport {
    // r[impl cairn.supersede.traceability_ownership.authoritative_checker]
    let requirements = collect_requirement_markers(requirement_files);
    let all_references = collect_reference_markers(evidence_files, &profile.marker_verbs);
    let coverage_references = coverage_references(&all_references);
    let missing = missing_requirements(&requirements, &coverage_references);
    let dangling = dangling_references(&requirements, &all_references);
    let requirement_count = requirements.len();
    let referenced_count = requirements
        .keys()
        .filter(|requirement_id| coverage_references.contains_key(*requirement_id))
        .count();
    let valid = missing.is_empty() && dangling.is_empty();
    TraceabilityCoverageReport {
        requirements,
        coverage_references,
        all_references,
        missing,
        dangling,
        requirement_count,
        referenced_count,
        valid,
    }
}

pub fn default_traceability_marker_vocabulary() -> Vec<String> {
    vec![
        TRACEABILITY_MARKER_VERB_REQUIREMENT.to_string(),
        TRACEABILITY_VERB_IMPL.to_string(),
        TRACEABILITY_VERB_VERIFY.to_string(),
        TRACEABILITY_VERB_DEPENDS.to_string(),
        TRACEABILITY_VERB_RELATED.to_string(),
    ]
}

pub fn validate_traceability_marker_links(
    input: &TraceabilityMarkerValidationInput,
) -> TraceabilityMarkerValidationReport {
    // r[impl cairn.requirement_id_registry.traceability_marker_core.contract]
    // r[impl cairn.requirement_id_registry.traceability_marker_core.validation]
    let mut issues = Vec::new();
    let mut links = Vec::new();
    for (source_index, marker_string) in input.marker_strings.iter().enumerate() {
        let extracted = extract_marker_bodies(marker_string);
        if extracted.is_empty() {
            push_marker_issue(
                &mut issues,
                TRACEABILITY_MARKER_ISSUE_MISSING,
                &format!("markers[{source_index}]"),
                marker_string,
                "marker string must contain at least one r[...] marker",
            );
            continue;
        }
        for (marker_index, (raw, body)) in extracted.into_iter().enumerate() {
            if let Some(link) = parse_traceability_marker_body(
                &raw,
                &body,
                source_index,
                marker_index,
                input,
                &mut issues,
            ) {
                links.push(link);
            }
        }
    }
    validate_marker_non_claims(&input.non_claims, &mut issues);
    validate_duplicate_links(&links, input.allow_duplicate_links, &mut issues);
    validate_registry_links(input, &links, &mut issues);
    sort_marker_issues(&mut issues);
    TraceabilityMarkerValidationReport {
        valid: issues.is_empty(),
        links,
        issues,
        boundary: TRACEABILITY_MARKER_CORE_BOUNDARY.to_string(),
    }
}

pub fn adapt_cairn_traceability_markers(markers: &[String]) -> TraceabilityMarkerValidationReport {
    // r[impl cairn.requirement_id_registry.traceability_marker_core.adapters]
    validate_traceability_marker_links(&TraceabilityMarkerValidationInput {
        marker_strings: markers.to_vec(),
        vocabulary: default_traceability_marker_vocabulary(),
        allowed_namespaces: vec!["cairn".to_string()],
        registry: None,
        consumer: "cairn".to_string(),
        expected_owner: "cairn".to_string(),
        require_registry: false,
        allow_duplicate_links: true,
        non_claims: TRACEABILITY_MARKER_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
    })
}

pub fn adapt_octet_traceability_markers(markers: &[String]) -> TraceabilityMarkerValidationReport {
    // r[impl cairn.requirement_id_registry.traceability_marker_core.adapters]
    validate_traceability_marker_links(&TraceabilityMarkerValidationInput {
        marker_strings: markers.to_vec(),
        vocabulary: default_traceability_marker_vocabulary(),
        allowed_namespaces: vec!["octet".to_string(), "cairn".to_string()],
        registry: None,
        consumer: "octet".to_string(),
        expected_owner: String::new(),
        require_registry: false,
        allow_duplicate_links: true,
        non_claims: TRACEABILITY_MARKER_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
    })
}

pub fn adapt_valence_traceability_markers(
    markers: &[String],
) -> TraceabilityMarkerValidationReport {
    // r[impl cairn.requirement_id_registry.traceability_marker_core.adapters]
    validate_traceability_marker_links(&TraceabilityMarkerValidationInput {
        marker_strings: markers.to_vec(),
        vocabulary: default_traceability_marker_vocabulary(),
        allowed_namespaces: vec!["valence".to_string(), "cairn".to_string()],
        registry: None,
        consumer: "valence".to_string(),
        expected_owner: String::new(),
        require_registry: false,
        allow_duplicate_links: true,
        non_claims: TRACEABILITY_MARKER_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
    })
}

pub fn traceability_spec_marker(line: &str) -> Option<String> {
    let rest = line.strip_prefix(TRACEABILITY_MARKER_PREFIX)?;
    let end = rest.find(TRACEABILITY_MARKER_SUFFIX)?;
    let marker = &rest[..end];
    let id = marker
        .split(TRACEABILITY_ID_EXTENSION_SEPARATOR)
        .next()
        .unwrap_or(marker);
    if traceability_id_is_valid(id) {
        Some(id.to_string())
    } else {
        None
    }
}

pub fn traceability_ref_markers(
    line: &str,
    allowed_verbs: &[String],
) -> Vec<TraceabilityReferenceMarker> {
    let mut markers = Vec::new();
    let mut start = 0usize;
    while let Some(offset) = line[start..].find(TRACEABILITY_MARKER_PREFIX) {
        let marker_start = start + offset + TRACEABILITY_MARKER_PREFIX.len();
        let Some(end_offset) = line[marker_start..].find(TRACEABILITY_MARKER_SUFFIX) else {
            break;
        };
        let marker = &line[marker_start..marker_start + end_offset];
        let mut parts = marker.split_whitespace();
        let verb = parts.next().unwrap_or_default();
        let raw_id = parts.next().unwrap_or_default();
        let requirement_id = raw_id
            .split(TRACEABILITY_ID_EXTENSION_SEPARATOR)
            .next()
            .unwrap_or(raw_id);
        if allowed_verbs.iter().any(|allowed| allowed == verb)
            && requirement_id != TRACEABILITY_PLACEHOLDER_ID
            && traceability_id_is_valid(requirement_id)
        {
            markers.push(TraceabilityReferenceMarker {
                verb: verb.to_string(),
                requirement_id: requirement_id.to_string(),
            });
        }
        start = marker_start + end_offset + TRACEABILITY_MARKER_SUFFIX.len_utf8();
    }
    markers
}

pub fn traceability_id_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn extract_marker_bodies(marker_string: &str) -> Vec<(String, String)> {
    let mut markers = Vec::new();
    let mut start = 0usize;
    while let Some(offset) = marker_string[start..].find(TRACEABILITY_MARKER_PREFIX) {
        let raw_start = start + offset;
        let body_start = raw_start + TRACEABILITY_MARKER_PREFIX.len();
        let Some(end_offset) = marker_string[body_start..].find(TRACEABILITY_MARKER_SUFFIX) else {
            break;
        };
        let body_end = body_start + end_offset;
        let raw_end = body_end + TRACEABILITY_MARKER_SUFFIX.len_utf8();
        markers.push((
            marker_string[raw_start..raw_end].to_string(),
            marker_string[body_start..body_end].to_string(),
        ));
        start = raw_end;
    }
    markers
}

fn parse_traceability_marker_body(
    raw: &str,
    body: &str,
    source_index: usize,
    marker_index: usize,
    input: &TraceabilityMarkerValidationInput,
    issues: &mut Vec<TraceabilityMarkerIssue>,
) -> Option<TraceabilityMarkerLink> {
    let field_path = format!("markers[{source_index}].links[{marker_index}]");
    let parts = body.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > TRACEABILITY_MARKER_BODY_MAX_PARTS {
        push_marker_issue(
            issues,
            TRACEABILITY_MARKER_ISSUE_MALFORMED,
            &field_path,
            raw,
            "marker must be r[id] or r[verb id]",
        );
        return None;
    }
    let (verb, raw_id) = if parts.len() == TRACEABILITY_MARKER_BODY_MAX_PARTS {
        (parts[0], parts[1])
    } else {
        (TRACEABILITY_MARKER_VERB_REQUIREMENT, parts[0])
    };
    if !input.vocabulary.iter().any(|allowed| allowed == verb) {
        push_marker_issue(
            issues,
            TRACEABILITY_MARKER_ISSUE_UNSUPPORTED_VERB,
            &format!("{field_path}.verb"),
            raw,
            "marker verb is not in the declared vocabulary",
        );
        return None;
    }
    let requirement_id = raw_id
        .split(TRACEABILITY_ID_EXTENSION_SEPARATOR)
        .next()
        .unwrap_or(raw_id);
    if requirement_id == TRACEABILITY_PLACEHOLDER_ID || !traceability_id_is_valid(requirement_id) {
        push_marker_issue(
            issues,
            TRACEABILITY_MARKER_ISSUE_MALFORMED_ID,
            &format!("{field_path}.requirement_id"),
            raw,
            "marker requirement id is malformed",
        );
        return None;
    }
    if !namespace_is_allowed(requirement_id, &input.allowed_namespaces) {
        push_marker_issue(
            issues,
            TRACEABILITY_MARKER_ISSUE_WRONG_NAMESPACE,
            &format!("{field_path}.requirement_id"),
            raw,
            "marker requirement id is outside the allowed namespaces",
        );
        return None;
    }
    Some(TraceabilityMarkerLink {
        raw: raw.to_string(),
        verb: verb.to_string(),
        requirement_id: requirement_id.to_string(),
        consumer: input.consumer.clone(),
    })
}

fn namespace_is_allowed(requirement_id: &str, namespaces: &[String]) -> bool {
    if namespaces.is_empty() {
        return true;
    }
    namespaces.iter().any(|namespace| {
        requirement_id == namespace || requirement_id.starts_with(&format!("{namespace}."))
    })
}

fn validate_marker_non_claims(non_claims: &[String], issues: &mut Vec<TraceabilityMarkerIssue>) {
    if non_claims.is_empty() {
        push_marker_issue(
            issues,
            TRACEABILITY_MARKER_ISSUE_OVERCLAIM,
            "non_claims",
            "",
            "traceability marker validation requires explicit non-claims",
        );
        return;
    }
    let joined = non_claims.join("; ").to_ascii_lowercase();
    for required in TRACEABILITY_MARKER_NON_CLAIMS {
        if !joined.contains(required) {
            push_marker_issue(
                issues,
                TRACEABILITY_MARKER_ISSUE_OVERCLAIM,
                "non_claims",
                "",
                &format!("traceability marker non-claims must mention `{required}`"),
            );
        }
    }
}

fn validate_duplicate_links(
    links: &[TraceabilityMarkerLink],
    allow_duplicate_links: bool,
    issues: &mut Vec<TraceabilityMarkerIssue>,
) {
    if allow_duplicate_links {
        return;
    }
    let mut seen = BTreeSet::new();
    for link in links {
        let key = format!("{}:{}", link.verb, link.requirement_id);
        if !seen.insert(key) {
            push_marker_issue(
                issues,
                TRACEABILITY_MARKER_ISSUE_DUPLICATE,
                "links",
                &link.raw,
                "duplicate traceability marker link is disallowed by policy",
            );
        }
    }
}

fn validate_registry_links(
    input: &TraceabilityMarkerValidationInput,
    links: &[TraceabilityMarkerLink],
    issues: &mut Vec<TraceabilityMarkerIssue>,
) {
    let Some(registry) = &input.registry else {
        if input.require_registry && !links.is_empty() {
            push_marker_issue(
                issues,
                TRACEABILITY_MARKER_ISSUE_MISSING_REGISTRY,
                "registry",
                "",
                "marker validation policy requires registry rows",
            );
        }
        return;
    };
    let references = links
        .iter()
        .map(|link| RequirementReference {
            requirement_id: link.requirement_id.clone(),
            consumer: input.consumer.clone(),
            expected_owner: input.expected_owner.clone(),
        })
        .collect::<Vec<_>>();
    let report = validate_requirement_id_registry(
        registry,
        &references,
        TRACEABILITY_MARKER_REGISTRY_HASH.to_string(),
    );
    for registry_issue in report.issues {
        push_marker_issue(
            issues,
            &format!("traceability_marker.registry.{}", registry_issue.code),
            &registry_issue.field_path,
            "",
            &registry_issue.message,
        );
    }
}

fn push_marker_issue(
    issues: &mut Vec<TraceabilityMarkerIssue>,
    code: &str,
    field_path: &str,
    marker: &str,
    message: &str,
) {
    issues.push(TraceabilityMarkerIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        marker: marker.to_string(),
        message: message.to_string(),
    });
}

fn sort_marker_issues(issues: &mut [TraceabilityMarkerIssue]) {
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.marker.cmp(&right.marker))
    });
}

fn collect_requirement_markers(
    files: &[TraceabilityFile],
) -> BTreeMap<String, Vec<TraceabilityRequirementOccurrence>> {
    let mut requirements = BTreeMap::new();
    for file in files {
        for (line_index, line) in file.text.lines().enumerate() {
            if let Some(requirement_id) = traceability_spec_marker(line.trim()) {
                requirements
                    .entry(requirement_id)
                    .or_insert_with(Vec::new)
                    .push(TraceabilityRequirementOccurrence {
                        path: file.path.clone(),
                        line: line_index + TRACEABILITY_LINE_NUMBER_ORIGIN,
                    });
            }
        }
    }
    requirements
}

fn collect_reference_markers(
    files: &[TraceabilityFile],
    allowed_verbs: &[String],
) -> BTreeMap<String, Vec<TraceabilityReferenceOccurrence>> {
    let mut references = BTreeMap::new();
    for file in files {
        for (line_index, line) in file.text.lines().enumerate() {
            for marker in traceability_ref_markers(line, allowed_verbs) {
                references
                    .entry(marker.requirement_id)
                    .or_insert_with(Vec::new)
                    .push(TraceabilityReferenceOccurrence {
                        verb: marker.verb,
                        path: file.path.clone(),
                        line: line_index + TRACEABILITY_LINE_NUMBER_ORIGIN,
                    });
            }
        }
    }
    references
}

fn coverage_references(
    all_references: &BTreeMap<String, Vec<TraceabilityReferenceOccurrence>>,
) -> BTreeMap<String, Vec<TraceabilityReferenceOccurrence>> {
    all_references
        .iter()
        .filter_map(|(requirement_id, references)| {
            let covering = references
                .iter()
                .filter(|reference| {
                    TRACEABILITY_COVERAGE_VERBS
                        .iter()
                        .any(|verb| *verb == reference.verb)
                })
                .cloned()
                .collect::<Vec<_>>();
            (!covering.is_empty()).then(|| (requirement_id.clone(), covering))
        })
        .collect()
}

fn missing_requirements(
    requirements: &BTreeMap<String, Vec<TraceabilityRequirementOccurrence>>,
    coverage_references: &BTreeMap<String, Vec<TraceabilityReferenceOccurrence>>,
) -> Vec<String> {
    requirements
        .keys()
        .filter(|requirement_id| !coverage_references.contains_key(*requirement_id))
        .cloned()
        .collect()
}

fn dangling_references(
    requirements: &BTreeMap<String, Vec<TraceabilityRequirementOccurrence>>,
    all_references: &BTreeMap<String, Vec<TraceabilityReferenceOccurrence>>,
) -> Vec<String> {
    all_references
        .keys()
        .filter(|requirement_id| !requirements.contains_key(*requirement_id))
        .cloned()
        .collect()
}

pub fn unique_sorted_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::verified::requirement_id_registry::{
        RequirementIdAlias, RequirementIdRegistry, RequirementRegistryRow,
        REQUIREMENT_ID_REGISTRY_BOUNDARY, REQUIREMENT_ID_REGISTRY_SCHEMA_V1,
    };

    use super::*;

    fn profile() -> TraceabilityProfile {
        TraceabilityProfile {
            id: TRACEABILITY_PROFILE_CAIRN_DEFAULT.to_string(),
            requirement_sources: vec![TraceabilitySource {
                root: ".cairn/specs".to_string(),
                source_kind: "accepted_specs".to_string(),
                extensions: vec!["md".to_string()],
            }],
            evidence_sources: vec![TraceabilitySource {
                root: "crates".to_string(),
                source_kind: "rust_source".to_string(),
                extensions: vec!["rs".to_string()],
            }],
            marker_verbs: vec![
                TRACEABILITY_VERB_IMPL.to_string(),
                TRACEABILITY_VERB_VERIFY.to_string(),
                TRACEABILITY_VERB_DEPENDS.to_string(),
                TRACEABILITY_VERB_RELATED.to_string(),
            ],
            assurance_level: "declared".to_string(),
            anchor_kind: "exact_marker_bytes".to_string(),
        }
    }

    fn file(path: &str, text: &str) -> TraceabilityFile {
        TraceabilityFile {
            path: path.to_string(),
            text: text.to_string(),
        }
    }

    fn raw_marker(body: &str) -> String {
        format!("{}{}{}]", "r", "[", body)
    }

    fn bare_marker(requirement_id: &str) -> String {
        raw_marker(requirement_id)
    }

    fn role_marker(verb: &str, requirement_id: &str) -> String {
        raw_marker(&format!("{verb} {requirement_id}"))
    }

    fn ref_marker(verb: &str, requirement_id: &str) -> String {
        format!("// {}", role_marker(verb, requirement_id))
    }

    fn marker_input(markers: Vec<String>) -> TraceabilityMarkerValidationInput {
        TraceabilityMarkerValidationInput {
            marker_strings: markers,
            vocabulary: default_traceability_marker_vocabulary(),
            allowed_namespaces: vec!["cairn".to_string()],
            registry: None,
            consumer: "cairn".to_string(),
            expected_owner: "cairn".to_string(),
            require_registry: false,
            allow_duplicate_links: false,
            non_claims: TRACEABILITY_MARKER_NON_CLAIMS
                .iter()
                .map(|claim| (*claim).to_string())
                .collect(),
        }
    }

    fn marker_registry() -> RequirementIdRegistry {
        RequirementIdRegistry {
            schema_version: REQUIREMENT_ID_REGISTRY_SCHEMA_V1.to_string(),
            boundary: REQUIREMENT_ID_REGISTRY_BOUNDARY.to_string(),
            rows: vec![RequirementRegistryRow {
                requirement_id: "cairn.marker.contract".to_string(),
                namespace: "cairn.marker".to_string(),
                owner_repo: "cairn".to_string(),
                accepted_spec: ".cairn/specs/requirement-id-registry/spec.md".to_string(),
                status: "active".to_string(),
                aliases: vec![RequirementIdAlias {
                    id: "cairn.marker.compat".to_string(),
                    status: "active".to_string(),
                    reason: "compatibility spelling".to_string(),
                }],
                allowed_consumers: vec!["cairn".to_string()],
            }],
            non_claims: vec![
                "not requirement satisfaction".to_string(),
                "not implementation correctness".to_string(),
                "not test coverage".to_string(),
                "not release eligibility".to_string(),
            ],
        }
    }

    fn marker_issue_codes(report: &TraceabilityMarkerValidationReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn traceability_marker_core_accepts_supported_links_and_registry_rows() {
        // r[verify cairn.requirement_id_registry.traceability_marker_core.fixtures.positive]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.contract]
        let mut input = marker_input(vec![
            bare_marker("cairn.marker.contract"),
            format!(
                "// {} {}",
                role_marker(TRACEABILITY_VERB_IMPL, "cairn.marker.contract"),
                role_marker(TRACEABILITY_VERB_VERIFY, "cairn.marker.compat")
            ),
            format!(
                "// {} {}",
                role_marker(TRACEABILITY_VERB_DEPENDS, "cairn.marker.contract"),
                role_marker(TRACEABILITY_VERB_RELATED, "cairn.marker.compat")
            ),
        ]);
        input.registry = Some(marker_registry());
        let report = validate_traceability_marker_links(&input);

        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert_eq!(report.links.len(), 5);
        assert!(report
            .links
            .iter()
            .any(|link| link.raw == role_marker(TRACEABILITY_VERB_VERIFY, "cairn.marker.compat")));
        assert!(report.boundary.contains("declared linkage health"));
    }

    #[test]
    fn traceability_marker_core_rejects_bad_markers_and_overclaims() {
        // r[verify cairn.requirement_id_registry.traceability_marker_core.fixtures.negative]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.verbs]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.verbs.unsupported]
        let mut input = marker_input(vec![
            role_marker("prove", "cairn.marker.contract"),
            role_marker(TRACEABILITY_VERB_IMPL, "bad/name"),
            role_marker(TRACEABILITY_VERB_IMPL, "valence.marker.contract"),
            role_marker(TRACEABILITY_VERB_VERIFY, "cairn.marker.contract"),
            role_marker(TRACEABILITY_VERB_VERIFY, "cairn.marker.contract"),
        ]);
        input.registry = Some(marker_registry());
        input.non_claims = vec!["proves requirement satisfaction".to_string()];
        let report = validate_traceability_marker_links(&input);
        let codes = marker_issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&TRACEABILITY_MARKER_ISSUE_UNSUPPORTED_VERB.to_string()));
        assert!(codes.contains(&TRACEABILITY_MARKER_ISSUE_MALFORMED_ID.to_string()));
        assert!(codes.contains(&TRACEABILITY_MARKER_ISSUE_WRONG_NAMESPACE.to_string()));
        assert!(codes.contains(&TRACEABILITY_MARKER_ISSUE_DUPLICATE.to_string()));
        assert!(codes.contains(&TRACEABILITY_MARKER_ISSUE_OVERCLAIM.to_string()));
    }

    #[test]
    fn traceability_marker_core_reports_registry_and_boundary_failures() {
        // r[verify cairn.requirement_id_registry.traceability_marker_core.validation]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.validation.shell]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.docs]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.final_validation]
        let mut missing_registry = marker_input(vec![role_marker(
            TRACEABILITY_VERB_IMPL,
            "cairn.marker.contract",
        )]);
        missing_registry.require_registry = true;
        let missing_registry_report = validate_traceability_marker_links(&missing_registry);
        assert!(!missing_registry_report.valid);
        assert!(marker_issue_codes(&missing_registry_report)
            .contains(&TRACEABILITY_MARKER_ISSUE_MISSING_REGISTRY.to_string()));

        let mut dangling = marker_input(vec![role_marker(
            TRACEABILITY_VERB_IMPL,
            "cairn.marker.missing",
        )]);
        dangling.registry = Some(marker_registry());
        let dangling_report = validate_traceability_marker_links(&dangling);
        assert!(!dangling_report.valid);
        assert!(marker_issue_codes(&dangling_report)
            .iter()
            .any(|code| code.contains("requirement_registry.unknown_requirement")));
        assert!(dangling_report
            .boundary
            .contains("does not prove requirement satisfaction"));
        assert!(dangling_report.boundary.contains("proof soundness"));
    }

    #[test]
    fn traceability_marker_adapters_preserve_current_spellings() {
        // r[verify cairn.requirement_id_registry.traceability_marker_core.adapters]
        // r[verify cairn.requirement_id_registry.traceability_marker_core.adapters.shape]
        let cairn = adapt_cairn_traceability_markers(&[ref_marker(
            TRACEABILITY_VERB_IMPL,
            "cairn.demo.alpha",
        )]);
        assert!(cairn.valid, "unexpected issues: {:?}", cairn.issues);
        assert_eq!(
            cairn.links[0].raw,
            role_marker(TRACEABILITY_VERB_IMPL, "cairn.demo.alpha")
        );

        let octet = adapt_octet_traceability_markers(&[ref_marker(
            TRACEABILITY_VERB_VERIFY,
            "octet.policy.lint",
        )]);
        assert!(octet.valid, "unexpected issues: {:?}", octet.issues);
        assert_eq!(octet.links[0].consumer, "octet");

        let valence = adapt_valence_traceability_markers(&[ref_marker(
            TRACEABILITY_VERB_RELATED,
            "valence.evidence.id",
        )]);
        assert!(valence.valid, "unexpected issues: {:?}", valence.issues);
        assert_eq!(valence.links[0].consumer, "valence");
    }

    #[test]
    fn complete_impl_or_verify_coverage_passes() {
        // r[verify cairn.supersede.traceability_ownership.authoritative_checker.complete]
        let report = classify_traceability_coverage(
            &profile(),
            &[file(
                ".cairn/specs/demo/spec.md",
                "r[cairn.demo.alpha] The system MUST work.\nr[cairn.demo.beta] The system MUST also work.",
            )],
            &[file(
                "crates/demo/src/lib.rs",
                &format!(
                    "{}\n{}",
                    ref_marker(TRACEABILITY_VERB_IMPL, "cairn.demo.alpha"),
                    ref_marker(TRACEABILITY_VERB_VERIFY, "cairn.demo.beta")
                ),
            )],
        );

        assert!(report.valid);
        assert_eq!(report.requirement_count, report.referenced_count);
        assert!(report.missing.is_empty());
        assert!(report.dangling.is_empty());
    }

    #[test]
    fn depends_or_related_does_not_satisfy_required_coverage() {
        // r[verify cairn.supersede.traceability_ownership.authoritative_checker.missing]
        let report = classify_traceability_coverage(
            &profile(),
            &[file(
                ".cairn/specs/demo/spec.md",
                "r[cairn.demo.alpha] The system MUST work.",
            )],
            &[file(
                "crates/demo/src/lib.rs",
                &ref_marker(TRACEABILITY_VERB_DEPENDS, "cairn.demo.alpha"),
            )],
        );

        assert!(!report.valid);
        assert_eq!(report.missing, vec!["cairn.demo.alpha".to_string()]);
        assert!(report.dangling.is_empty());
    }

    #[test]
    fn unknown_reference_is_reported_as_dangling() {
        // r[verify cairn.supersede.traceability_ownership.authoritative_checker.dangling]
        let report = classify_traceability_coverage(
            &profile(),
            &[file(
                ".cairn/specs/demo/spec.md",
                "r[cairn.demo.alpha] The system MUST work.",
            )],
            &[file(
                "crates/demo/src/lib.rs",
                &format!(
                    "{}\n{}",
                    ref_marker(TRACEABILITY_VERB_IMPL, "cairn.demo.alpha"),
                    ref_marker(TRACEABILITY_VERB_RELATED, "cairn.demo.missing")
                ),
            )],
        );

        assert!(!report.valid);
        assert!(report.missing.is_empty());
        assert_eq!(report.dangling, vec!["cairn.demo.missing".to_string()]);
    }

    #[test]
    fn marker_parser_rejects_placeholders_and_invalid_ids() {
        let verbs = profile().marker_verbs;

        assert_eq!(
            traceability_spec_marker("r[cairn.demo.alpha+meta] text"),
            Some("cairn.demo.alpha".to_string())
        );
        assert_eq!(
            traceability_spec_marker(&format!(
                "r[{} {}]",
                TRACEABILITY_VERB_IMPL, "cairn.demo.alpha"
            )),
            None
        );
        assert!(
            traceability_ref_markers(&ref_marker(TRACEABILITY_VERB_IMPL, "..."), &verbs).is_empty()
        );
        assert!(traceability_ref_markers(
            &ref_marker(TRACEABILITY_VERB_IMPL, "cairn.demo/alpha"),
            &verbs
        )
        .is_empty());
    }
}

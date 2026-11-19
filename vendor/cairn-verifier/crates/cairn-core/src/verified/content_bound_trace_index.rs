pub const CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1: &str = "cairn.content-bound-trace-index.v1";
pub const CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2: &str = "cairn.content-bound-trace-index.v2";
pub const TRACEABILITY_ANCHOR_RUST_ITEM_V1: &str = "rust_item_v1";
const STRUCTURAL_INDEX_DOMAIN: &str = "cairn.trace-index.structural-index.v2";
const STRUCTURAL_BINDING_DOMAIN: &str = "cairn.trace-index.rust-item-binding.v1";
const STRUCTURAL_NON_CLAIMS: &[&str] = &[
    "not callee behavior",
    "not reusable proof freshness",
    "not executed tests",
];
pub const CONTENT_BOUND_TRACE_INDEX_BOUNDARY: &str = "content-bound trace index validation proves canonical identity, current-input freshness, and declared marker linkage only; it does not prove requirement satisfaction, implementation correctness, test adequacy, proof soundness, release eligibility, lifecycle readiness, or behavioral correctness";
pub const TRACEABILITY_ASSURANCE_DECLARED: &str = "declared";
pub const TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES: &str = "exact_marker_bytes";
pub const CONTENT_BOUND_TRACE_INDEX_NON_CLAIMS: &[&str] = &[
    "not requirement satisfaction",
    "not implementation correctness",
    "not test adequacy",
    "not proof soundness",
    "not release eligibility",
];

const BLAKE3_HEX_LENGTH: usize = 64;
const MAX_SOURCE_PATH_BYTES: usize = 4_096;
const MAX_STRUCTURAL_BINDINGS: usize = 65_536;
const MAX_CONFIGURATION_VALUES: usize = 256;
const LINE_NUMBER_ORIGIN: u64 = 1;
const DOMAIN_SEPARATOR: u8 = 0;
const SOURCE_FILE_DOMAIN: &str = "cairn.trace-index.source-file.v1";
const MARKER_DOMAIN: &str = "cairn.trace-index.marker.v1";
const LINK_DOMAIN: &str = "cairn.trace-index.link.v1";
const PROFILE_DOMAIN: &str = "cairn.trace-index.profile.v1";
const INDEX_DOMAIN: &str = "cairn.trace-index.index.v1";
const MARKER_PREFIX: &str = "r[";
const MARKER_SUFFIX: u8 = b']';
const MARKER_BODY_PART_COUNT: usize = 2;
const REQUIREMENT_EXTENSION_SEPARATOR: char = '+';
const PLACEHOLDER_REQUIREMENT_ID: &str = "...";

const ISSUE_SCHEMA_UNSUPPORTED: &str = "trace_index.schema_unsupported";
const ISSUE_BOUNDARY_INVALID: &str = "trace_index.boundary_invalid";
const ISSUE_PROFILE_UNSUPPORTED: &str = "trace_index.profile_unsupported";
const ISSUE_PROFILE_STALE: &str = "trace_index.profile_stale";
const ISSUE_POLICY_STALE: &str = "trace_index.policy_stale";
const ISSUE_REGISTRY_INVALID: &str = "trace_index.registry_invalid";
const ISSUE_REGISTRY_STALE: &str = "trace_index.registry_stale";
const ISSUE_PATH_UNSAFE: &str = "trace_index.source_path_unsafe";
const ISSUE_SOURCE_DUPLICATE: &str = "trace_index.source_duplicate";
const ISSUE_SOURCE_STALE: &str = "trace_index.source_stale";
const ISSUE_SOURCE_MISSING: &str = "trace_index.source_missing";
const ISSUE_SOURCE_UNEXPECTED: &str = "trace_index.source_unexpected";
const ISSUE_MARKER_DANGLING: &str = "trace_index.marker_dangling";
const ISSUE_MARKER_SPAN_INVALID: &str = "trace_index.marker_span_invalid";
const ISSUE_LINK_DUPLICATE: &str = "trace_index.link_duplicate";
const ISSUE_LINK_STALE: &str = "trace_index.link_stale";
const ISSUE_HASH_INVALID: &str = "trace_index.hash_invalid";
const ISSUE_ORDER_INVALID: &str = "trace_index.order_invalid";
const ISSUE_NON_CLAIM_MISSING: &str = "trace_index.non_claim_missing";
const ISSUE_QUERY_UNKNOWN: &str = "trace_index.query_unknown_requirement";
const ISSUE_SERIALIZATION: &str = "trace_index.serialization_failed";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundTraceSourceIdentity {
    pub path: String,
    pub content_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundTraceMarkerSpan {
    pub start_byte: u64,
    pub end_byte: u64,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundTraceLink {
    pub requirement_id: String,
    pub requirement_row_blake3: String,
    pub role: String,
    pub source_path: String,
    pub source_file_blake3: String,
    pub marker_span: ContentBoundTraceMarkerSpan,
    pub marker_blake3: String,
    pub link_blake3: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemConfiguration {
    pub source_path: String,
    pub file_blake3: String,
    pub schema_version: String,
    pub target: String,
    pub features: Vec<String>,
    pub declared_features: Vec<String>,
    pub cfg: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemFact {
    pub source_path: String,
    pub marker_start_byte: u64,
    pub start_byte: u64,
    pub end_byte: u64,
    pub item_kind: String,
    pub scoped_locator: String,
    pub normalized_item_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustItemBinding {
    pub link_blake3: String,
    pub requirement_id: String,
    pub requirement_row_blake3: String,
    pub source_path: String,
    pub source_file_blake3: String,
    pub marker_span: ContentBoundTraceMarkerSpan,
    pub item: RustItemFact,
    pub configuration: RustItemConfiguration,
    pub binding_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralTraceData {
    pub adapter: String,
    pub parser: String,
    pub schema: String,
    pub configuration: RustItemConfiguration,
    pub bindings: Vec<RustItemBinding>,
    pub history: Vec<RustItemBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundTraceIndex {
    pub schema_version: String,
    pub boundary: String,
    pub profile: crate::verified::traceability::TraceabilityProfile,
    pub profile_blake3: String,
    pub policy_blake3: String,
    pub requirement_registry_blake3: String,
    pub requirements:
        Vec<crate::verified::content_bound_requirement_registry::ContentBoundRequirementRow>,
    pub sources: Vec<ContentBoundTraceSourceIdentity>,
    pub links: Vec<ContentBoundTraceLink>,
    pub non_claims: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural: Option<StructuralTraceData>,
    pub index_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentBoundTraceIndexIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundTraceIndexEvaluation {
    pub index: Option<ContentBoundTraceIndex>,
    pub issues: Vec<ContentBoundTraceIndexIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundTraceIndexVerification {
    pub valid: bool,
    pub index_blake3: String,
    pub requirement_count: usize,
    pub source_count: usize,
    pub link_count: usize,
    pub issues: Vec<ContentBoundTraceIndexIssue>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundTraceQuery {
    pub requirement:
        crate::verified::content_bound_requirement_registry::ContentBoundRequirementRow,
    pub links: Vec<ContentBoundTraceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBoundTraceQueryEvaluation {
    pub query: Option<ContentBoundTraceQuery>,
    pub issues: Vec<ContentBoundTraceIndexIssue>,
}

#[derive(serde::Serialize)]
struct LinkHashMaterial<'a> {
    requirement_id: &'a str,
    requirement_row_blake3: &'a str,
    role: &'a str,
    source_path: &'a str,
    source_file_blake3: &'a str,
    start_byte: u64,
    end_byte: u64,
    marker_blake3: &'a str,
}

#[derive(serde::Serialize)]
struct IndexHashMaterial<'a> {
    schema_version: &'a str,
    boundary: &'a str,
    profile: &'a crate::verified::traceability::TraceabilityProfile,
    profile_blake3: &'a str,
    policy_blake3: &'a str,
    requirement_registry_blake3: &'a str,
    requirements:
        &'a [crate::verified::content_bound_requirement_registry::ContentBoundRequirementRow],
    sources: &'a [ContentBoundTraceSourceIdentity],
    links: &'a [ContentBoundTraceLink],
    non_claims: &'a [String],
}

struct LocatedMarker<'a> {
    role: &'a str,
    requirement_id: &'a str,
    raw: &'a str,
    start_byte: usize,
    end_byte: usize,
}

#[must_use]
pub fn evaluate_content_bound_trace_index(
    registry: &crate::verified::content_bound_requirement_registry::ContentBoundRequirementRegistry,
    profile: &crate::verified::traceability::TraceabilityProfile,
    mut evidence_files: Vec<crate::verified::traceability::TraceabilityFile>,
) -> ContentBoundTraceIndexEvaluation {
    // r[impl cairn.trace_index.canonical_index]
    // r[impl cairn.trace_index.validation]
    let mut issues = Vec::new();
    validate_profile(profile, "profile", &mut issues);
    for registry_issue in crate::verified::content_bound_requirement_registry::validate_content_bound_requirement_registry(registry) {
        push_issue(
            &mut issues,
            ISSUE_REGISTRY_INVALID,
            &format!("requirement_registry.{}", registry_issue.field_path),
            &registry_issue.message,
        );
    }

    evidence_files.sort_by(|left, right| left.path.cmp(&right.path));
    reject_duplicate_source_paths(&evidence_files, &mut issues);
    let requirement_rows = registry
        .rows
        .iter()
        .map(|row| (row.requirement_id.as_str(), row))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut sources = Vec::with_capacity(evidence_files.len());
    let mut links = Vec::new();

    for (source_index, file) in evidence_files.iter().enumerate() {
        let field_path = format!("sources[{source_index}].path");
        if !safe_relative_path(&file.path) {
            push_issue(
                &mut issues,
                ISSUE_PATH_UNSAFE,
                &field_path,
                "trace-index source paths must be bounded safe relative paths",
            );
            continue;
        }
        let source_file_blake3 = domain_hash_bytes(SOURCE_FILE_DOMAIN, file.text.as_bytes());
        sources.push(ContentBoundTraceSourceIdentity {
            path: file.path.clone(),
            content_blake3: source_file_blake3.clone(),
        });
        collect_file_links(
            file,
            &source_file_blake3,
            profile,
            &requirement_rows,
            &mut links,
            &mut issues,
        );
    }

    links.sort_by(compare_links);
    reject_duplicate_links(&links, &mut issues);
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return ContentBoundTraceIndexEvaluation {
            index: None,
            issues,
        };
    }

    let mut requirements = registry.rows.clone();
    requirements.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));
    let profile_blake3 = match domain_hash_json(PROFILE_DOMAIN, profile) {
        Ok(hash) => hash,
        Err(issue) => {
            return ContentBoundTraceIndexEvaluation {
                index: None,
                issues: vec![issue],
            }
        }
    };
    let non_claims = required_non_claims();
    let index_blake3 = match index_hash(
        profile,
        &profile_blake3,
        &registry.policy_blake3,
        &registry.registry_blake3,
        &requirements,
        &sources,
        &links,
        &non_claims,
    ) {
        Ok(hash) => hash,
        Err(issue) => {
            return ContentBoundTraceIndexEvaluation {
                index: None,
                issues: vec![issue],
            }
        }
    };
    let index = ContentBoundTraceIndex {
        schema_version: CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1.to_string(),
        boundary: CONTENT_BOUND_TRACE_INDEX_BOUNDARY.to_string(),
        profile: profile.clone(),
        profile_blake3,
        policy_blake3: registry.policy_blake3.clone(),
        requirement_registry_blake3: registry.registry_blake3.clone(),
        requirements,
        sources,
        links,
        non_claims,
        structural: None,
        index_blake3,
    };
    if profile.anchor_kind == TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES {
        debug_assert!(validate_content_bound_trace_index(&index).is_empty());
    }
    ContentBoundTraceIndexEvaluation {
        index: Some(index),
        issues: Vec::new(),
    }
}

/// Consumes parser facts from bounded saved inputs. It never parses source or infers cfg.
#[must_use]
pub fn evaluate_structural_trace_index(
    registry: &crate::verified::content_bound_requirement_registry::ContentBoundRequirementRegistry,
    profile: &crate::verified::traceability::TraceabilityProfile,
    evidence_files: Vec<crate::verified::traceability::TraceabilityFile>,
    facts: Vec<RustItemFact>,
    configuration: RustItemConfiguration,
    previous: Option<&ContentBoundTraceIndex>,
) -> ContentBoundTraceIndexEvaluation {
    let source_lengths = evidence_files
        .iter()
        .map(|file| (file.path.clone(), file.text.len()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut evaluation = evaluate_content_bound_trace_index(registry, profile, evidence_files);
    if profile.anchor_kind != TRACEABILITY_ANCHOR_RUST_ITEM_V1 {
        evaluation.issues.push(issue(
            ISSUE_PROFILE_UNSUPPORTED,
            "profile.anchor_kind",
            "structural evaluation requires rust_item_v1",
        ));
        evaluation.index = None;
        return evaluation;
    }
    let Some(mut index) = evaluation.index.take() else {
        return evaluation;
    };
    if facts.len() != index.links.len() {
        evaluation.issues.push(issue(
            "trace_index.item_fact_count",
            "structural.facts",
            "parser facts must correspond one-to-one with selected exact marker occurrences",
        ));
        return evaluation;
    }
    let mut issues = Vec::new();
    let mut bindings = Vec::new();
    for link in &index.links {
        let mut matches = facts.iter().filter(|fact| {
            fact.source_path == link.source_path
                && fact.marker_start_byte == link.marker_span.start_byte
        });
        let Some(item) = matches.next() else {
            push_issue(
                &mut issues,
                "trace_index.item_selection",
                &format!("links.{}", link.link_blake3),
                "marker has no supported Rust declaration",
            );
            continue;
        };
        if matches.next().is_some() {
            push_issue(
                &mut issues,
                "trace_index.item_selection",
                &format!("links.{}", link.link_blake3),
                "marker resolves ambiguously to multiple Rust declarations",
            );
            continue;
        }
        let item = item.clone();
        if !safe_structural_path(&item.source_path)
            || item.start_byte >= item.end_byte
            || source_lengths
                .get(item.source_path.as_str())
                .is_none_or(|size| item.end_byte > *size as u64)
            || item.normalized_item_blake3.len() != BLAKE3_HEX_LENGTH
            || item.scoped_locator.is_empty()
        {
            push_issue(
                &mut issues,
                "trace_index.item_invalid",
                &format!("links.{}", link.link_blake3),
                "parser fact has invalid source path, bounds, locator or identity",
            );
            continue;
        }
        let mut binding = RustItemBinding {
            link_blake3: link.link_blake3.clone(),
            requirement_id: link.requirement_id.clone(),
            requirement_row_blake3: link.requirement_row_blake3.clone(),
            source_path: link.source_path.clone(),
            source_file_blake3: link.source_file_blake3.clone(),
            marker_span: link.marker_span.clone(),
            item,
            configuration: configuration.clone(),
            binding_blake3: String::new(),
        };
        binding.binding_blake3 = structural_binding_hash(&binding);
        bindings.push(binding);
    }
    let mut history = Vec::new();
    if let Some(previous) = previous {
        if previous.schema_version != CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2
            || !validate_content_bound_trace_index(previous).is_empty()
        {
            push_issue(
                &mut issues,
                "trace_index.history_invalid",
                "previous",
                "previous structural index must be a valid self-bound v2 artifact",
            );
        } else if let Some(prior) = &previous.structural {
            history.extend(prior.history.iter().cloned());
            history.extend(
                prior
                    .bindings
                    .iter()
                    .filter(|binding| {
                        !bindings
                            .iter()
                            .any(|current| current.binding_blake3 == binding.binding_blake3)
                    })
                    .cloned(),
            );
        }
    }
    history.sort_by(|a, b| a.binding_blake3.cmp(&b.binding_blake3));
    history.dedup_by(|a, b| a.binding_blake3 == b.binding_blake3);
    if bindings.len() > MAX_STRUCTURAL_BINDINGS || history.len() > MAX_STRUCTURAL_BINDINGS {
        push_issue(
            &mut issues,
            "trace_index.binding_budget_exceeded",
            "structural",
            "current or historical Rust binding budget exceeded",
        );
    }
    if configuration.schema_version != "cairn.rust-item-configuration.v1"
        || configuration.target.is_empty()
        || !safe_structural_path(&configuration.source_path)
        || configuration.file_blake3.len() != BLAKE3_HEX_LENGTH
        || configuration
            .features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || configuration
            .declared_features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || configuration.features.iter().any(|feature| {
            configuration
                .declared_features
                .binary_search(feature)
                .is_err()
        })
        || configuration.cfg.windows(2).any(|pair| pair[0] >= pair[1])
    {
        push_issue(&mut issues, "trace_index.configuration_invalid", "configuration", "configuration requires v1 schema, explicit target and sorted unique feature/cfg values");
    }
    if !issues.is_empty() {
        sort_issues(&mut issues);
        return ContentBoundTraceIndexEvaluation {
            index: None,
            issues,
        };
    }
    index.non_claims.extend(
        STRUCTURAL_NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string()),
    );
    index.schema_version = CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2.to_string();
    index.structural = Some(StructuralTraceData {
        adapter: "cairn.saved-rust-bytes.v1".into(),
        parser: "syn-2.0.117".into(),
        schema: "cairn.rust-item-binding.v1".into(),
        configuration,
        bindings,
        history,
    });
    index.index_blake3 = structural_index_hash(&index);
    let validation = validate_content_bound_trace_index(&index);
    if !validation.is_empty() {
        return ContentBoundTraceIndexEvaluation {
            index: None,
            issues: validation,
        };
    }
    evaluation.index = Some(index);
    evaluation
}

#[must_use]
pub fn verify_structural_trace_index(
    saved: &ContentBoundTraceIndex,
    registry: &crate::verified::content_bound_requirement_registry::ContentBoundRequirementRegistry,
    profile: &crate::verified::traceability::TraceabilityProfile,
    evidence_files: Vec<crate::verified::traceability::TraceabilityFile>,
    facts: Vec<RustItemFact>,
    configuration: RustItemConfiguration,
) -> ContentBoundTraceIndexVerification {
    let mut issues = validate_content_bound_trace_index(saved);
    let current = evaluate_structural_trace_index(
        registry,
        profile,
        evidence_files,
        facts,
        configuration,
        None,
    );
    issues.extend(current.issues);
    if let Some(current) = current.index {
        compare_current_index(saved, &current, &mut issues);
        if saved.requirement_registry_blake3 != current.requirement_registry_blake3 {
            push_issue(
                &mut issues,
                "trace_index.requirement_revision_stale",
                "requirement_registry_blake3",
                &format!(
                    "expected {}; actual {}",
                    saved.requirement_registry_blake3, current.requirement_registry_blake3
                ),
            );
        }
        if saved.structural.as_ref().map(|data| &data.configuration)
            != current.structural.as_ref().map(|data| &data.configuration)
        {
            push_issue(
                &mut issues,
                "trace_index.configuration_stale",
                "structural.configuration",
                &format!(
                    "expected {}; actual {}",
                    saved
                        .structural
                        .as_ref()
                        .map_or("absent", |data| data.configuration.file_blake3.as_str()),
                    current
                        .structural
                        .as_ref()
                        .map_or("absent", |data| data.configuration.file_blake3.as_str())
                ),
            );
        }
        let old = saved.structural.as_ref().map(|data| &data.bindings);
        let new = current.structural.as_ref().map(|data| &data.bindings);
        if old != new {
            let expected = old.map_or_else(String::new, |bindings| {
                bindings
                    .iter()
                    .map(|binding| binding.binding_blake3.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            });
            let actual = new.map_or_else(String::new, |bindings| {
                bindings
                    .iter()
                    .map(|binding| binding.binding_blake3.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            });
            push_issue(
                &mut issues,
                "trace_index.binding_stale",
                "structural.bindings",
                &format!("saved bindings {expected}; current bindings {actual}"),
            );
        }
    }
    sort_issues(&mut issues);
    ContentBoundTraceIndexVerification {
        valid: issues.is_empty(),
        index_blake3: saved.index_blake3.clone(),
        requirement_count: saved.requirements.len(),
        source_count: saved.sources.len(),
        link_count: saved.links.len(),
        issues,
        boundary: CONTENT_BOUND_TRACE_INDEX_BOUNDARY.to_string(),
    }
}

fn structural_binding_hash(binding: &RustItemBinding) -> String {
    domain_hash_json(
        STRUCTURAL_BINDING_DOMAIN,
        &(
            binding.link_blake3.as_str(),
            binding.requirement_row_blake3.as_str(),
            binding.source_path.as_str(),
            binding.source_file_blake3.as_str(),
            &binding.marker_span,
            &binding.item,
            "cairn.saved-rust-bytes.v1",
            "syn-2.0.117",
            "cairn.rust-item-binding.v1",
            &binding.configuration,
        ),
    )
    .unwrap_or_default()
}

fn structural_index_hash(index: &ContentBoundTraceIndex) -> String {
    let mut value = serde_json::to_value(index).expect("trace index is serializable");
    value
        .as_object_mut()
        .expect("trace index is an object")
        .remove("index_blake3");
    domain_hash_json(STRUCTURAL_INDEX_DOMAIN, &value).unwrap_or_default()
}

#[must_use]
pub fn validate_content_bound_trace_index(
    index: &ContentBoundTraceIndex,
) -> Vec<ContentBoundTraceIndexIssue> {
    // r[impl cairn.trace_index.freshness]
    let mut issues = Vec::new();
    if index.schema_version != CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1
        && index.schema_version != CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2
    {
        push_issue(
            &mut issues,
            ISSUE_SCHEMA_UNSUPPORTED,
            "schema_version",
            "content-bound trace-index schema version is unsupported",
        );
    }
    if index.boundary != CONTENT_BOUND_TRACE_INDEX_BOUNDARY {
        push_issue(
            &mut issues,
            ISSUE_BOUNDARY_INVALID,
            "boundary",
            "trace-index boundary must preserve the declared-linkage claim",
        );
    }
    validate_profile(&index.profile, "profile", &mut issues);
    if (index.schema_version == CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1) != index.structural.is_none()
        || (index.schema_version == CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1
            && index.profile.anchor_kind != TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES)
        || (index.schema_version == CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2
            && index.profile.anchor_kind != TRACEABILITY_ANCHOR_RUST_ITEM_V1)
    {
        push_issue(
            &mut issues,
            ISSUE_SCHEMA_UNSUPPORTED,
            "structural",
            "index schema, profile anchor and structural payload must agree",
        );
    }
    validate_hash(&index.profile_blake3, "profile_blake3", &mut issues);
    validate_hash(&index.policy_blake3, "policy_blake3", &mut issues);
    validate_hash(
        &index.requirement_registry_blake3,
        "requirement_registry_blake3",
        &mut issues,
    );
    validate_non_claims(&index.non_claims, &mut issues);
    validate_requirement_rows(index, &mut issues);
    validate_sources(index, &mut issues);
    validate_links(index, &mut issues);
    if let Some(structural) = &index.structural {
        validate_structural(index, structural, &mut issues);
    }
    validate_profile_hash(index, &mut issues);
    validate_index_hash(index, &mut issues);
    validate_order(index, &mut issues);
    sort_issues(&mut issues);
    issues
}

#[must_use]
pub fn verify_content_bound_trace_index(
    index: &ContentBoundTraceIndex,
    registry: &crate::verified::content_bound_requirement_registry::ContentBoundRequirementRegistry,
    profile: &crate::verified::traceability::TraceabilityProfile,
    evidence_files: Vec<crate::verified::traceability::TraceabilityFile>,
) -> ContentBoundTraceIndexVerification {
    // r[impl cairn.trace_index.freshness]
    let mut issues = validate_content_bound_trace_index(index);
    if index.schema_version == CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2
        || profile.anchor_kind == TRACEABILITY_ANCHOR_RUST_ITEM_V1
    {
        push_issue(&mut issues, "trace_index.item_facts_required", "structural",
            "structural freshness requires recomputed saved-byte parser facts and explicit configuration");
    }
    let current = evaluate_content_bound_trace_index(registry, profile, evidence_files);
    issues.extend(current.issues);
    if let Some(current_index) = current.index {
        compare_current_index(index, &current_index, &mut issues);
    }
    sort_issues(&mut issues);
    ContentBoundTraceIndexVerification {
        valid: issues.is_empty(),
        index_blake3: index.index_blake3.clone(),
        requirement_count: index.requirements.len(),
        source_count: index.sources.len(),
        link_count: index.links.len(),
        issues,
        boundary: CONTENT_BOUND_TRACE_INDEX_BOUNDARY.to_string(),
    }
}

#[must_use]
pub fn query_content_bound_trace_index(
    index: &ContentBoundTraceIndex,
    requirement_id: &str,
) -> ContentBoundTraceQueryEvaluation {
    // r[impl cairn.trace_index.query]
    let issues = validate_content_bound_trace_index(index);
    if !issues.is_empty() {
        return ContentBoundTraceQueryEvaluation {
            query: None,
            issues,
        };
    }
    let Some(requirement) = index
        .requirements
        .iter()
        .find(|row| row.requirement_id == requirement_id)
        .cloned()
    else {
        return ContentBoundTraceQueryEvaluation {
            query: None,
            issues: vec![issue(
                ISSUE_QUERY_UNKNOWN,
                "requirement_id",
                "trace index does not contain the exact requirement ID",
            )],
        };
    };
    let links = index
        .links
        .iter()
        .filter(|link| link.requirement_id == requirement_id)
        .cloned()
        .collect();
    ContentBoundTraceQueryEvaluation {
        query: Some(ContentBoundTraceQuery { requirement, links }),
        issues: Vec::new(),
    }
}

pub fn parse_content_bound_trace_index_value(
    value: &serde_json::Value,
) -> Result<ContentBoundTraceIndex, ContentBoundTraceIndexIssue> {
    serde_json::from_value(value.clone()).map_err(|error| {
        issue(
            ISSUE_SCHEMA_UNSUPPORTED,
            "index",
            &format!("trace-index JSON does not match the closed schema: {error}"),
        )
    })
}

fn collect_file_links<'a>(
    file: &crate::verified::traceability::TraceabilityFile,
    source_file_blake3: &str,
    profile: &crate::verified::traceability::TraceabilityProfile,
    requirement_rows: &std::collections::BTreeMap<
        &'a str,
        &'a crate::verified::content_bound_requirement_registry::ContentBoundRequirementRow,
    >,
    links: &mut Vec<ContentBoundTraceLink>,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    let mut line_start_byte = 0usize;
    for (line_index, segment) in file.text.split_inclusive('\n').enumerate() {
        let line_number = match u64::try_from(line_index.saturating_add(1)) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for marker in located_reference_markers(segment, &profile.marker_verbs) {
            let Some(requirement_row) = requirement_rows.get(marker.requirement_id) else {
                continue;
            };
            let start = line_start_byte.saturating_add(marker.start_byte);
            let end = line_start_byte.saturating_add(marker.end_byte);
            let (Ok(start_byte), Ok(end_byte)) = (u64::try_from(start), u64::try_from(end)) else {
                push_issue(
                    issues,
                    ISSUE_MARKER_SPAN_INVALID,
                    &format!("links.{}.{}", file.path, marker.start_byte),
                    "trace marker byte span exceeds the portable integer range",
                );
                continue;
            };
            let marker_span = ContentBoundTraceMarkerSpan {
                start_byte,
                end_byte,
                start_line: line_number,
                end_line: line_number,
            };
            let marker_blake3 = domain_hash_bytes(MARKER_DOMAIN, marker.raw.as_bytes());
            let link_blake3 = match link_hash(
                requirement_row,
                marker.role,
                &file.path,
                source_file_blake3,
                &marker_span,
                &marker_blake3,
            ) {
                Ok(hash) => hash,
                Err(link_issue) => {
                    issues.push(link_issue);
                    continue;
                }
            };
            links.push(ContentBoundTraceLink {
                requirement_id: marker.requirement_id.to_string(),
                requirement_row_blake3: requirement_row.row_blake3.clone(),
                role: marker.role.to_string(),
                source_path: file.path.clone(),
                source_file_blake3: source_file_blake3.to_string(),
                marker_span,
                marker_blake3,
                link_blake3,
            });
        }
        line_start_byte = line_start_byte.saturating_add(segment.len());
    }
}

fn located_reference_markers<'a>(
    line: &'a str,
    allowed_roles: &[String],
) -> Vec<LocatedMarker<'a>> {
    let mut markers = Vec::new();
    let mut search_start = 0usize;
    while let Some(offset) = line[search_start..].find(MARKER_PREFIX) {
        let raw_start = search_start.saturating_add(offset);
        let body_start = raw_start.saturating_add(MARKER_PREFIX.len());
        let Some(end_offset) = line[body_start..].find(char::from(MARKER_SUFFIX)) else {
            break;
        };
        let body_end = body_start.saturating_add(end_offset);
        let raw_end = body_end.saturating_add(1);
        let body = &line[body_start..body_end];
        let parts = body.split_whitespace().collect::<Vec<_>>();
        if parts.len() == MARKER_BODY_PART_COUNT {
            let role = parts[0];
            let raw_requirement_id = parts[1];
            let requirement_id = raw_requirement_id
                .split(REQUIREMENT_EXTENSION_SEPARATOR)
                .next()
                .unwrap_or(raw_requirement_id);
            if allowed_roles.iter().any(|allowed| allowed == role)
                && requirement_id != PLACEHOLDER_REQUIREMENT_ID
                && crate::verified::traceability::traceability_id_is_valid(requirement_id)
            {
                markers.push(LocatedMarker {
                    role,
                    requirement_id,
                    raw: &line[raw_start..raw_end],
                    start_byte: raw_start,
                    end_byte: raw_end,
                });
            }
        }
        search_start = raw_end;
    }
    markers
}

fn compare_links(
    left: &ContentBoundTraceLink,
    right: &ContentBoundTraceLink,
) -> std::cmp::Ordering {
    left.requirement_id
        .cmp(&right.requirement_id)
        .then(left.role.cmp(&right.role))
        .then(left.source_path.cmp(&right.source_path))
        .then(
            left.marker_span
                .start_byte
                .cmp(&right.marker_span.start_byte),
        )
        .then(left.marker_span.end_byte.cmp(&right.marker_span.end_byte))
        .then(left.link_blake3.cmp(&right.link_blake3))
}

fn reject_duplicate_source_paths(
    files: &[crate::verified::traceability::TraceabilityFile],
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    for pair in files.windows(MARKER_BODY_PART_COUNT) {
        if pair[0].path == pair[1].path {
            push_issue(
                issues,
                ISSUE_SOURCE_DUPLICATE,
                &format!("sources.{}", pair[0].path),
                "trace-index source paths must be unique",
            );
        }
    }
}

fn reject_duplicate_links(
    links: &[ContentBoundTraceLink],
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    let mut seen = std::collections::BTreeSet::new();
    for link in links {
        if !seen.insert(link.link_blake3.as_str()) {
            push_issue(
                issues,
                ISSUE_LINK_DUPLICATE,
                "links",
                "trace-index links must have unique content identities",
            );
        }
    }
}

fn validate_profile(
    profile: &crate::verified::traceability::TraceabilityProfile,
    field_path: &str,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    if profile.assurance_level != TRACEABILITY_ASSURANCE_DECLARED
        || !matches!(
            profile.anchor_kind.as_str(),
            TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES | TRACEABILITY_ANCHOR_RUST_ITEM_V1
        )
        || (profile.anchor_kind == TRACEABILITY_ANCHOR_RUST_ITEM_V1)
            != profile.rust_configuration_path.is_some()
        || profile.id.is_empty()
        || profile.marker_verbs.is_empty()
    {
        push_issue(
            issues,
            ISSUE_PROFILE_UNSUPPORTED,
            field_path,
            "trace-index profile must use declared assurance and a supported versioned anchor",
        );
    }
}

fn validate_requirement_rows(
    index: &ContentBoundTraceIndex,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    let mut seen = std::collections::BTreeSet::new();
    for (row_index, row) in index.requirements.iter().enumerate() {
        validate_hash(
            &row.row_blake3,
            &format!("requirements[{row_index}].row_blake3"),
            issues,
        );
        if !seen.insert(row.requirement_id.as_str()) {
            push_issue(
                issues,
                ISSUE_REGISTRY_INVALID,
                &format!("requirements[{row_index}].requirement_id"),
                "trace-index requirement IDs must be unique",
            );
        }
    }
}

fn validate_sources(index: &ContentBoundTraceIndex, issues: &mut Vec<ContentBoundTraceIndexIssue>) {
    let mut seen = std::collections::BTreeSet::new();
    for (source_index, source) in index.sources.iter().enumerate() {
        if !safe_relative_path(&source.path) {
            push_issue(
                issues,
                ISSUE_PATH_UNSAFE,
                &format!("sources[{source_index}].path"),
                "trace-index source paths must be bounded safe relative paths",
            );
        }
        validate_hash(
            &source.content_blake3,
            &format!("sources[{source_index}].content_blake3"),
            issues,
        );
        if !seen.insert(source.path.as_str()) {
            push_issue(
                issues,
                ISSUE_SOURCE_DUPLICATE,
                &format!("sources[{source_index}].path"),
                "trace-index source paths must be unique",
            );
        }
    }
}

fn validate_links(index: &ContentBoundTraceIndex, issues: &mut Vec<ContentBoundTraceIndexIssue>) {
    let requirements = index
        .requirements
        .iter()
        .map(|row| (row.requirement_id.as_str(), row))
        .collect::<std::collections::BTreeMap<_, _>>();
    let sources = index
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut seen = std::collections::BTreeSet::new();
    for (link_index, link) in index.links.iter().enumerate() {
        let field_path = format!("links[{link_index}]");
        validate_hash(
            &link.requirement_row_blake3,
            &format!("{field_path}.requirement_row_blake3"),
            issues,
        );
        validate_hash(
            &link.source_file_blake3,
            &format!("{field_path}.source_file_blake3"),
            issues,
        );
        validate_hash(
            &link.marker_blake3,
            &format!("{field_path}.marker_blake3"),
            issues,
        );
        validate_hash(
            &link.link_blake3,
            &format!("{field_path}.link_blake3"),
            issues,
        );
        if link.marker_span.start_byte >= link.marker_span.end_byte
            || link.marker_span.start_line < LINE_NUMBER_ORIGIN
            || link.marker_span.start_line > link.marker_span.end_line
        {
            push_issue(
                issues,
                ISSUE_MARKER_SPAN_INVALID,
                &format!("{field_path}.marker_span"),
                "trace marker span must be non-empty and ordered",
            );
        }
        if !index
            .profile
            .marker_verbs
            .iter()
            .any(|role| role == &link.role)
        {
            push_issue(
                issues,
                ISSUE_PROFILE_UNSUPPORTED,
                &format!("{field_path}.role"),
                "trace link role is absent from the selected profile",
            );
        }
        let Some(row) = requirements.get(link.requirement_id.as_str()) else {
            push_issue(
                issues,
                ISSUE_MARKER_DANGLING,
                &format!("{field_path}.requirement_id"),
                "trace link requirement ID is absent from the index requirements",
            );
            continue;
        };
        if row.row_blake3 != link.requirement_row_blake3 {
            push_issue(
                issues,
                ISSUE_REGISTRY_STALE,
                &format!("{field_path}.requirement_row_blake3"),
                "trace link requirement-row identity does not match the index row",
            );
        }
        let Some(source) = sources.get(link.source_path.as_str()) else {
            push_issue(
                issues,
                ISSUE_SOURCE_MISSING,
                &format!("{field_path}.source_path"),
                "trace link source path is absent from the index source manifest",
            );
            continue;
        };
        if source.content_blake3 != link.source_file_blake3 {
            push_issue(
                issues,
                ISSUE_SOURCE_STALE,
                &format!("{field_path}.source_file_blake3"),
                "trace link source identity does not match the index source manifest",
            );
        }
        if let Ok(expected_hash) = link_hash(
            row,
            &link.role,
            &link.source_path,
            &link.source_file_blake3,
            &link.marker_span,
            &link.marker_blake3,
        ) {
            if expected_hash != link.link_blake3 {
                push_issue(
                    issues,
                    ISSUE_HASH_INVALID,
                    &format!("{field_path}.link_blake3"),
                    "trace link BLAKE3 does not match its canonical hash material",
                );
            }
        }
        if !seen.insert(link.link_blake3.as_str()) {
            push_issue(
                issues,
                ISSUE_LINK_DUPLICATE,
                &format!("{field_path}.link_blake3"),
                "trace-index links must have unique content identities",
            );
        }
    }
}

fn validate_profile_hash(
    index: &ContentBoundTraceIndex,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    if let Ok(expected) = domain_hash_json(PROFILE_DOMAIN, &index.profile) {
        if expected != index.profile_blake3 {
            push_issue(
                issues,
                ISSUE_HASH_INVALID,
                "profile_blake3",
                "trace-index profile BLAKE3 does not match the canonical profile",
            );
        }
    }
}

fn validate_structural(
    index: &ContentBoundTraceIndex,
    data: &StructuralTraceData,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    if data.adapter != "cairn.saved-rust-bytes.v1"
        || data.parser != "syn-2.0.117"
        || data.schema != "cairn.rust-item-binding.v1"
        || data.configuration.schema_version != "cairn.rust-item-configuration.v1"
        || !safe_structural_path(&data.configuration.source_path)
        || data.configuration.file_blake3.len() != BLAKE3_HEX_LENGTH
        || data.configuration.target.is_empty()
    {
        push_issue(
            issues,
            "trace_index.interpretation_unsupported",
            "structural",
            "unsupported adapter, parser, schema or configuration",
        );
    }
    validate_hash(
        &data.configuration.file_blake3,
        "structural.configuration.file_blake3",
        issues,
    );
    for claim in STRUCTURAL_NON_CLAIMS {
        if !index.non_claims.iter().any(|present| present == claim) {
            push_issue(
                issues,
                ISSUE_NON_CLAIM_MISSING,
                "non_claims",
                &format!("structural index must retain `{claim}`"),
            );
        }
    }
    for source in &index.sources {
        if !safe_structural_path(&source.path) {
            push_issue(
                issues,
                ISSUE_PATH_UNSAFE,
                "sources",
                "structural source path is not a safe logical path",
            );
        }
    }
    if data
        .configuration
        .features
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || data
            .configuration
            .declared_features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || data.configuration.features.iter().any(|feature| {
            data.configuration
                .declared_features
                .binary_search(feature)
                .is_err()
        })
        || data
            .configuration
            .cfg
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || index.profile.rust_configuration_path.as_deref()
            != Some(data.configuration.source_path.as_str())
    {
        push_issue(
            issues,
            "trace_index.configuration_invalid",
            "structural.configuration",
            "configuration must be canonical and match the selected profile path",
        );
    }
    if data.bindings.len() > MAX_STRUCTURAL_BINDINGS
        || data.history.len() > MAX_STRUCTURAL_BINDINGS
        || data.configuration.features.len() > MAX_CONFIGURATION_VALUES
        || data.configuration.declared_features.len() > MAX_CONFIGURATION_VALUES
        || data.configuration.cfg.len() > MAX_CONFIGURATION_VALUES
    {
        push_issue(
            issues,
            "trace_index.binding_budget_exceeded",
            "structural",
            "structural binding or configuration budget exceeded",
        );
    }
    let links = index
        .links
        .iter()
        .map(|link| (link.link_blake3.as_str(), link))
        .collect::<std::collections::BTreeMap<_, _>>();
    if data.bindings.len() != index.links.len() {
        push_issue(
            issues,
            "trace_index.binding_missing",
            "structural.bindings",
            "each current marker link requires one current Rust item binding",
        );
    }
    if data
        .bindings
        .iter()
        .map(|binding| binding.link_blake3.as_str())
        .collect::<Vec<_>>()
        != index
            .links
            .iter()
            .map(|link| link.link_blake3.as_str())
            .collect::<Vec<_>>()
    {
        push_issue(
            issues,
            ISSUE_ORDER_INVALID,
            "structural.bindings",
            "current item attachments must follow canonical marker-link order",
        );
    }
    let mut selected = std::collections::BTreeSet::new();
    for (position, binding) in data.bindings.iter().enumerate() {
        let field = format!("structural.bindings[{position}]");
        if !selected.insert(binding.link_blake3.as_str()) {
            push_issue(
                issues,
                "trace_index.binding_duplicate",
                &field,
                "current marker link is bound more than once",
            );
        }
        match links.get(binding.link_blake3.as_str()) {
            Some(link) if binding.requirement_id == link.requirement_id
                && binding.requirement_row_blake3 == link.requirement_row_blake3
                && binding.source_path == link.source_path
                && binding.source_file_blake3 == link.source_file_blake3
                && binding.marker_span == link.marker_span
                && binding.item.source_path == link.source_path
                && binding.item.marker_start_byte == link.marker_span.start_byte => {},
            _ => push_issue(issues, "trace_index.binding_link_invalid", &field, "current binding must match exactly one current marker occurrence and requirement revision"),
        }
        if binding.configuration != data.configuration
            || !safe_structural_path(&binding.source_path)
            || binding.item.start_byte < binding.marker_span.end_byte
            || binding.item.start_byte >= binding.item.end_byte
            || binding.item.scoped_locator.is_empty()
            || !matches!(
                binding.item.item_kind.as_str(),
                "fn" | "struct"
                    | "enum"
                    | "union"
                    | "mod"
                    | "trait"
                    | "impl"
                    | "type"
                    | "const"
                    | "static"
                    | "trait_fn"
                    | "trait_const"
                    | "trait_type"
                    | "impl_fn"
                    | "impl_const"
                    | "impl_type"
            )
        {
            push_issue(
                issues,
                "trace_index.binding_invalid",
                &field,
                "current interpretation configuration, source path, span or locator is invalid",
            );
        }
        validate_hash(
            &binding.item.normalized_item_blake3,
            &format!("{field}.item.normalized_item_blake3"),
            issues,
        );
        validate_hash(
            &binding.binding_blake3,
            &format!("{field}.binding_blake3"),
            issues,
        );
        if structural_binding_hash(binding) != binding.binding_blake3 {
            push_issue(
                issues,
                ISSUE_HASH_INVALID,
                &field,
                "binding does not match its versioned content identity",
            );
        }
    }
    let current_hashes = data
        .bindings
        .iter()
        .map(|binding| binding.binding_blake3.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut previous_hash: Option<&str> = None;
    for (position, binding) in data.history.iter().enumerate() {
        let field = format!("structural.history[{position}]");
        if previous_hash.is_some_and(|previous| previous >= binding.binding_blake3.as_str())
            || current_hashes.contains(binding.binding_blake3.as_str())
            || structural_binding_hash(binding) != binding.binding_blake3
        {
            push_issue(
                issues,
                "trace_index.history_invalid",
                &field,
                "history must be sorted, unique, distinct from current attachments and self-bound",
            );
        }
        previous_hash = Some(&binding.binding_blake3);
    }
}

fn validate_index_hash(
    index: &ContentBoundTraceIndex,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    validate_hash(&index.index_blake3, "index_blake3", issues);
    let expected = if index.schema_version == CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2 {
        Some(structural_index_hash(index))
    } else {
        index_hash(
            &index.profile,
            &index.profile_blake3,
            &index.policy_blake3,
            &index.requirement_registry_blake3,
            &index.requirements,
            &index.sources,
            &index.links,
            &index.non_claims,
        )
        .ok()
    };
    if let Some(expected) = expected {
        if expected != index.index_blake3 {
            push_issue(
                issues,
                ISSUE_HASH_INVALID,
                "index_blake3",
                "trace-index BLAKE3 does not match its canonical hash material",
            );
        }
    }
}

fn validate_order(index: &ContentBoundTraceIndex, issues: &mut Vec<ContentBoundTraceIndexIssue>) {
    let mut requirements = index.requirements.clone();
    requirements.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));
    let mut sources = index.sources.clone();
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let mut links = index.links.clone();
    links.sort_by(compare_links);
    if requirements != index.requirements || sources != index.sources || links != index.links {
        push_issue(
            issues,
            ISSUE_ORDER_INVALID,
            "index",
            "trace-index requirements, sources, and links must use canonical order",
        );
    }
}

fn compare_current_index(
    saved: &ContentBoundTraceIndex,
    current: &ContentBoundTraceIndex,
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    if saved.profile_blake3 != current.profile_blake3 {
        push_issue(
            issues,
            ISSUE_PROFILE_STALE,
            "profile_blake3",
            "saved trace-index profile does not match the current selected profile",
        );
    }
    if saved.policy_blake3 != current.policy_blake3 {
        push_issue(
            issues,
            ISSUE_POLICY_STALE,
            "policy_blake3",
            "saved trace-index policy does not match the current registry policy",
        );
    }
    if saved.requirement_registry_blake3 != current.requirement_registry_blake3
        || saved.requirements != current.requirements
    {
        push_issue(
            issues,
            ISSUE_REGISTRY_STALE,
            "requirement_registry_blake3",
            "saved trace-index requirement registry does not match the current registry",
        );
    }
    compare_sources(&saved.sources, &current.sources, issues);
    if saved.links != current.links {
        push_issue(
            issues,
            ISSUE_LINK_STALE,
            "links",
            "saved trace links do not match current marker content or locations",
        );
    }
}

fn compare_sources(
    saved: &[ContentBoundTraceSourceIdentity],
    current: &[ContentBoundTraceSourceIdentity],
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
) {
    let saved_by_path = saved
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<std::collections::BTreeMap<_, _>>();
    let current_by_path = current
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (path, saved_source) in &saved_by_path {
        match current_by_path.get(path) {
            None => push_issue(
                issues,
                ISSUE_SOURCE_MISSING,
                &format!("sources.{path}"),
                "saved trace-index source is absent from the current source snapshot",
            ),
            Some(current_source)
                if saved_source.content_blake3 != current_source.content_blake3 =>
            {
                push_issue(
                    issues,
                    ISSUE_SOURCE_STALE,
                    &format!("sources.{path}.content_blake3"),
                    "saved trace-index source content does not match current bytes",
                );
            }
            Some(_) => {}
        }
    }
    for path in current_by_path.keys() {
        if !saved_by_path.contains_key(path) {
            push_issue(
                issues,
                ISSUE_SOURCE_UNEXPECTED,
                &format!("sources.{path}"),
                "current source snapshot contains a source absent from the saved trace index",
            );
        }
    }
}

fn link_hash(
    requirement_row: &crate::verified::content_bound_requirement_registry::ContentBoundRequirementRow,
    role: &str,
    source_path: &str,
    source_file_blake3: &str,
    marker_span: &ContentBoundTraceMarkerSpan,
    marker_blake3: &str,
) -> Result<String, ContentBoundTraceIndexIssue> {
    domain_hash_json(
        LINK_DOMAIN,
        &LinkHashMaterial {
            requirement_id: &requirement_row.requirement_id,
            requirement_row_blake3: &requirement_row.row_blake3,
            role,
            source_path,
            source_file_blake3,
            start_byte: marker_span.start_byte,
            end_byte: marker_span.end_byte,
            marker_blake3,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn index_hash(
    profile: &crate::verified::traceability::TraceabilityProfile,
    profile_blake3: &str,
    policy_blake3: &str,
    requirement_registry_blake3: &str,
    requirements: &[crate::verified::content_bound_requirement_registry::ContentBoundRequirementRow],
    sources: &[ContentBoundTraceSourceIdentity],
    links: &[ContentBoundTraceLink],
    non_claims: &[String],
) -> Result<String, ContentBoundTraceIndexIssue> {
    domain_hash_json(
        INDEX_DOMAIN,
        &IndexHashMaterial {
            schema_version: CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1,
            boundary: CONTENT_BOUND_TRACE_INDEX_BOUNDARY,
            profile,
            profile_blake3,
            policy_blake3,
            requirement_registry_blake3,
            requirements,
            sources,
            links,
            non_claims,
        },
    )
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<ContentBoundTraceIndexIssue>) {
    let joined = non_claims.join("; ").to_ascii_lowercase();
    for required in CONTENT_BOUND_TRACE_INDEX_NON_CLAIMS {
        if !joined.contains(required) {
            push_issue(
                issues,
                ISSUE_NON_CLAIM_MISSING,
                "non_claims",
                &format!("trace-index non-claims must mention `{required}`"),
            );
        }
    }
}

fn required_non_claims() -> Vec<String> {
    CONTENT_BOUND_TRACE_INDEX_NON_CLAIMS
        .iter()
        .map(|claim| (*claim).to_string())
        .collect()
}

fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_SOURCE_PATH_BYTES
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}

fn safe_structural_path(path: &str) -> bool {
    safe_relative_path(path) && !path.contains(':') && !path.contains('\0')
}

fn validate_hash(value: &str, field_path: &str, issues: &mut Vec<ContentBoundTraceIndexIssue>) {
    if value.len() != BLAKE3_HEX_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        push_issue(
            issues,
            ISSUE_HASH_INVALID,
            field_path,
            "trace-index identity must be lowercase BLAKE3 hex",
        );
    }
}

fn domain_hash_bytes(domain: &str, bytes: &[u8]) -> String {
    let mut material =
        Vec::with_capacity(domain.len().saturating_add(1).saturating_add(bytes.len()));
    material.extend_from_slice(domain.as_bytes());
    material.push(DOMAIN_SEPARATOR);
    material.extend_from_slice(bytes);
    crate::verified::evidence::blake3_hex(&material)
}

fn domain_hash_json<T: serde::Serialize>(
    domain: &str,
    value: &T,
) -> Result<String, ContentBoundTraceIndexIssue> {
    let value = serde_json::to_value(value).map_err(|_error| {
        issue(
            ISSUE_SERIALIZATION,
            "hash_material",
            "canonical trace-index hash material could not be serialized",
        )
    })?;
    Ok(domain_hash_bytes(
        domain,
        crate::verified::evidence::canonical_json(&value).as_bytes(),
    ))
}

fn issue(code: &str, field_path: &str, message: &str) -> ContentBoundTraceIndexIssue {
    ContentBoundTraceIndexIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<ContentBoundTraceIndexIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(issue(code, field_path, message));
}

fn sort_issues(issues: &mut Vec<ContentBoundTraceIndexIssue>) {
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

    const REQUIREMENT_ID: &str = "cairn.trace.demo";
    const REPOSITORY_ID: &str = "onixresearch/cairn";
    const REVISION: &str = "trace-index-test-revision";
    const POLICY_BYTES: &[u8] = b"policy-one";
    const CHANGED_POLICY_BYTES: &[u8] = b"policy-two";
    const SPEC_PATH: &str = ".cairn/specs/trace/spec.md";
    const SOURCE_PATH: &str = "crates/demo/src/lib.rs";
    const SECOND_SOURCE_PATH: &str = "crates/demo/src/other.rs";
    const EXPECTED_LINK_COUNT: usize = 2;

    fn specification(
    ) -> crate::verified::content_bound_requirement_registry::ContentBoundSpecificationInput {
        crate::verified::content_bound_requirement_registry::ContentBoundSpecificationInput {
            path: SPEC_PATH.to_string(),
            markdown: format!(
                "# Trace\n\n## Requirements\n\n### Requirement: Demo\nr[{REQUIREMENT_ID}] Cairn MUST bind the declared link.\n\n#### Scenario: Link\n- GIVEN a marker\n- WHEN indexed\n- THEN the link MUST be content-bound\n"
            ),
        }
    }

    fn registry(
        policy: &[u8],
    ) -> crate::verified::content_bound_requirement_registry::ContentBoundRequirementRegistry {
        crate::verified::content_bound_requirement_registry::evaluate_content_bound_requirement_registry(
            vec![specification()],
            policy,
            REPOSITORY_ID,
            crate::verified::content_bound_requirement_registry::ContentBoundRevision {
                kind: crate::verified::content_bound_requirement_registry::CONTENT_BOUND_REVISION_KIND_OPAQUE.to_string(),
                value: REVISION.to_string(),
            },
        )
        .registry
        .expect("valid registry")
    }

    fn profile() -> crate::verified::traceability::TraceabilityProfile {
        crate::verified::traceability::TraceabilityProfile {
            id: "test".to_string(),
            requirement_sources: vec![crate::verified::traceability::TraceabilitySource {
                root: ".cairn/specs".to_string(),
                source_kind: "accepted_specs".to_string(),
                extensions: vec!["md".to_string()],
            }],
            evidence_sources: vec![crate::verified::traceability::TraceabilitySource {
                root: "crates".to_string(),
                source_kind: "rust_source".to_string(),
                extensions: vec!["rs".to_string()],
            }],
            marker_verbs: vec![
                crate::verified::traceability::TRACEABILITY_VERB_IMPL.to_string(),
                crate::verified::traceability::TRACEABILITY_VERB_VERIFY.to_string(),
                crate::verified::traceability::TRACEABILITY_VERB_DEPENDS.to_string(),
                crate::verified::traceability::TRACEABILITY_VERB_RELATED.to_string(),
            ],
            assurance_level: TRACEABILITY_ASSURANCE_DECLARED.to_string(),
            anchor_kind: TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES.to_string(),
            rust_configuration_path: None,
        }
    }

    fn source(path: &str, prefix: &str) -> crate::verified::traceability::TraceabilityFile {
        crate::verified::traceability::TraceabilityFile {
            path: path.to_string(),
            text: format!(
                "{prefix}// r[{} {REQUIREMENT_ID}]\n// r[{} {REQUIREMENT_ID}]\n",
                crate::verified::traceability::TRACEABILITY_VERB_IMPL,
                crate::verified::traceability::TRACEABILITY_VERB_VERIFY
            ),
        }
    }

    fn index() -> ContentBoundTraceIndex {
        evaluate_content_bound_trace_index(
            &registry(POLICY_BYTES),
            &profile(),
            vec![source(SOURCE_PATH, "")],
        )
        .index
        .expect("valid trace index")
    }

    fn issue_codes(issues: &[ContentBoundTraceIndexIssue]) -> Vec<String> {
        issues.iter().map(|issue| issue.code.clone()).collect()
    }

    #[test]
    fn canonical_index_is_deterministic_and_sorted() {
        // r[verify cairn.trace_index.canonical_index]
        let registry = registry(POLICY_BYTES);
        let first = evaluate_content_bound_trace_index(
            &registry,
            &profile(),
            vec![
                source(SECOND_SOURCE_PATH, ""),
                source(SOURCE_PATH, "// unrelated\n"),
            ],
        )
        .index
        .expect("first index");
        let second = evaluate_content_bound_trace_index(
            &registry,
            &profile(),
            vec![
                source(SOURCE_PATH, "// unrelated\n"),
                source(SECOND_SOURCE_PATH, ""),
            ],
        )
        .index
        .expect("second index");

        assert_eq!(first, second);
        assert_eq!(first.index_blake3.len(), BLAKE3_HEX_LENGTH);
        assert!(first.sources[0].path < first.sources[1].path);
        assert!(validate_content_bound_trace_index(&first).is_empty());
    }

    #[test]
    fn link_identity_excludes_display_lines_but_binds_byte_span() {
        // r[verify cairn.trace_index.canonical_index]
        let index = index();
        let link = &index.links[0];
        let row = &index.requirements[0];
        let mut display_only_change = link.marker_span.clone();
        display_only_change.start_line = display_only_change.start_line.saturating_add(1);
        display_only_change.end_line = display_only_change.end_line.saturating_add(1);
        let display_hash = link_hash(
            row,
            &link.role,
            &link.source_path,
            &link.source_file_blake3,
            &display_only_change,
            &link.marker_blake3,
        )
        .expect("display hash");
        assert_eq!(display_hash, link.link_blake3);

        let mut byte_change = link.marker_span.clone();
        byte_change.start_byte = byte_change.start_byte.saturating_add(1);
        let byte_hash = link_hash(
            row,
            &link.role,
            &link.source_path,
            &link.source_file_blake3,
            &byte_change,
            &link.marker_blake3,
        )
        .expect("byte hash");
        assert_ne!(byte_hash, link.link_blake3);
    }

    #[test]
    fn current_index_passes_and_stale_source_fails() {
        // r[verify cairn.trace_index.freshness]
        let registry = registry(POLICY_BYTES);
        let index = index();
        let current = verify_content_bound_trace_index(
            &index,
            &registry,
            &profile(),
            vec![source(SOURCE_PATH, "")],
        );
        assert!(current.valid, "issues: {:?}", current.issues);

        let stale = verify_content_bound_trace_index(
            &index,
            &registry,
            &profile(),
            vec![source(SOURCE_PATH, "// changed\n")],
        );
        let codes = issue_codes(&stale.issues);
        assert!(!stale.valid);
        assert!(codes.contains(&ISSUE_SOURCE_STALE.to_string()));
        assert!(codes.contains(&ISSUE_LINK_STALE.to_string()));
    }

    #[test]
    fn moved_and_copied_markers_fail_freshness() {
        // r[verify cairn.trace_index.freshness]
        let registry = registry(POLICY_BYTES);
        let index = index();
        let moved = verify_content_bound_trace_index(
            &index,
            &registry,
            &profile(),
            vec![source(SOURCE_PATH, "\n")],
        );
        assert!(issue_codes(&moved.issues).contains(&ISSUE_LINK_STALE.to_string()));

        let copied = verify_content_bound_trace_index(
            &index,
            &registry,
            &profile(),
            vec![
                source(SOURCE_PATH, ""),
                crate::verified::traceability::TraceabilityFile {
                    path: SECOND_SOURCE_PATH.to_string(),
                    text: format!(
                        "// r[{} {REQUIREMENT_ID}]\n",
                        crate::verified::traceability::TRACEABILITY_VERB_IMPL
                    ),
                },
            ],
        );
        let codes = issue_codes(&copied.issues);
        assert!(codes.contains(&ISSUE_SOURCE_UNEXPECTED.to_string()));
        assert!(codes.contains(&ISSUE_LINK_STALE.to_string()));
    }

    #[test]
    fn changed_policy_and_registry_fail_freshness() {
        // r[verify cairn.trace_index.freshness]
        let index = index();
        let changed_registry = registry(CHANGED_POLICY_BYTES);
        let report = verify_content_bound_trace_index(
            &index,
            &changed_registry,
            &profile(),
            vec![source(SOURCE_PATH, "")],
        );
        let codes = issue_codes(&report.issues);
        assert!(!report.valid);
        assert!(codes.contains(&ISSUE_POLICY_STALE.to_string()));
        assert!(codes.contains(&ISSUE_REGISTRY_STALE.to_string()));
    }

    #[test]
    fn unsafe_and_duplicate_sources_fail_closed_while_unselected_markers_are_ignored() {
        // r[verify cairn.trace_index.validation]
        let registry = registry(POLICY_BYTES);
        let unsafe_path = evaluate_content_bound_trace_index(
            &registry,
            &profile(),
            vec![source("../outside.rs", "")],
        );
        assert!(issue_codes(&unsafe_path.issues).contains(&ISSUE_PATH_UNSAFE.to_string()));

        let duplicate = evaluate_content_bound_trace_index(
            &registry,
            &profile(),
            vec![source(SOURCE_PATH, ""), source(SOURCE_PATH, "")],
        );
        assert!(issue_codes(&duplicate.issues).contains(&ISSUE_SOURCE_DUPLICATE.to_string()));

        let unselected = evaluate_content_bound_trace_index(
            &registry,
            &profile(),
            vec![crate::verified::traceability::TraceabilityFile {
                path: SOURCE_PATH.to_string(),
                text: format!(
                    "// r[{} cairn.trace.missing]\n",
                    crate::verified::traceability::TRACEABILITY_VERB_IMPL
                ),
            }],
        );
        let index = unselected.index.expect("index with no selected links");
        assert!(unselected.issues.is_empty());
        assert!(index.links.is_empty());
    }

    #[test]
    fn exact_query_accepts_known_and_rejects_unknown_requirements() {
        // r[verify cairn.trace_index.query]
        let index = index();
        let known = query_content_bound_trace_index(&index, REQUIREMENT_ID);
        let query = known.query.expect("known requirement");
        assert_eq!(query.requirement.requirement_id, REQUIREMENT_ID);
        assert_eq!(query.links.len(), EXPECTED_LINK_COUNT);

        let unknown = query_content_bound_trace_index(&index, "cairn.trace.unknown");
        assert!(unknown.query.is_none());
        assert!(issue_codes(&unknown.issues).contains(&ISSUE_QUERY_UNKNOWN.to_string()));
    }

    #[test]
    fn malformed_index_hash_and_closed_schema_are_rejected() {
        // r[verify cairn.trace_index.validation]
        let mut index = index();
        index.links[0].marker_blake3 = "bad".to_string();
        let issues = validate_content_bound_trace_index(&index);
        assert!(issue_codes(&issues).contains(&ISSUE_HASH_INVALID.to_string()));

        let value = serde_json::json!({
            "schema_version": CONTENT_BOUND_TRACE_INDEX_SCHEMA_V1,
            "unexpected": true,
        });
        assert!(parse_content_bound_trace_index_value(&value).is_err());
    }
    #[test]
    fn structural_binding_is_versioned_and_preserves_history() {
        let mut profile = profile();
        profile.anchor_kind = TRACEABILITY_ANCHOR_RUST_ITEM_V1.into();
        profile.rust_configuration_path = Some("rust-config.json".into());
        let configuration = RustItemConfiguration {
            source_path: "rust-config.json".into(),
            file_blake3: "a".repeat(64),
            schema_version: "cairn.rust-item-configuration.v1".into(),
            target: "x86_64-unknown-linux-gnu".into(),
            features: vec![],
            declared_features: vec![],
            cfg: vec![],
        };
        let text = format!("// r[impl {REQUIREMENT_ID}]\nfn x() {{}}\n");
        let file = crate::verified::traceability::TraceabilityFile {
            path: SOURCE_PATH.into(),
            text: text.clone(),
        };
        let fact = RustItemFact {
            source_path: SOURCE_PATH.into(),
            marker_start_byte: 3,
            start_byte: text.find("fn x").unwrap() as u64,
            end_byte: text.len() as u64,
            item_kind: "fn".into(),
            scoped_locator: "crate/fn:x#0".into(),
            normalized_item_blake3: "a".repeat(64),
        };
        let original = evaluate_structural_trace_index(
            &registry(POLICY_BYTES),
            &profile,
            vec![file.clone()],
            vec![fact.clone()],
            configuration.clone(),
            None,
        )
        .index
        .unwrap();
        assert_eq!(original.schema_version, CONTENT_BOUND_TRACE_INDEX_SCHEMA_V2);
        assert!(validate_content_bound_trace_index(&original).is_empty());
        assert!(
            verify_structural_trace_index(
                &original,
                &registry(POLICY_BYTES),
                &profile,
                vec![file.clone()],
                vec![fact.clone()],
                configuration.clone()
            )
            .valid
        );
        let incomplete = verify_content_bound_trace_index(
            &original,
            &registry(POLICY_BYTES),
            &profile,
            vec![file.clone()],
        );
        assert!(!incomplete.valid);
        assert!(issue_codes(&incomplete.issues).contains(&"trace_index.item_facts_required".into()));

        let copied = crate::verified::traceability::TraceabilityFile {
            path: SECOND_SOURCE_PATH.into(),
            text: text.clone(),
        };
        let copied_fact = RustItemFact {
            source_path: SECOND_SOURCE_PATH.into(),
            ..fact.clone()
        };
        let copied_report = verify_structural_trace_index(
            &original,
            &registry(POLICY_BYTES),
            &profile,
            vec![file.clone(), copied],
            vec![fact.clone(), copied_fact],
            configuration.clone(),
        );
        assert!(!copied_report.valid);
        assert!(
            issue_codes(&copied_report.issues).contains(&"trace_index.source_unexpected".into())
        );

        let mut edited_spec = specification();
        edited_spec.markdown = edited_spec
            .markdown
            .replace("declared link", "reviewed link");
        let changed_registry = crate::verified::content_bound_requirement_registry::evaluate_content_bound_requirement_registry(
            vec![edited_spec], POLICY_BYTES, REPOSITORY_ID,
            crate::verified::content_bound_requirement_registry::ContentBoundRevision {
                kind: crate::verified::content_bound_requirement_registry::CONTENT_BOUND_REVISION_KIND_OPAQUE.into(),
                value: REVISION.into(),
            },
        ).registry.unwrap();
        let requirement_stale = verify_structural_trace_index(
            &original,
            &changed_registry,
            &profile,
            vec![file],
            vec![fact.clone()],
            configuration.clone(),
        );
        assert!(issue_codes(&requirement_stale.issues)
            .contains(&"trace_index.requirement_revision_stale".into()));
        assert!(requirement_stale
            .issues
            .iter()
            .any(|issue| issue.message.contains("expected ") && issue.message.contains("actual ")));

        let formatted = format!("// r[impl {REQUIREMENT_ID}]\nfn x() {{ }}\n");
        let changed_file = crate::verified::traceability::TraceabilityFile {
            path: SOURCE_PATH.into(),
            text: formatted.clone(),
        };
        let changed_fact = RustItemFact {
            end_byte: formatted.len() as u64,
            ..fact
        };
        let stale = verify_structural_trace_index(
            &original,
            &registry(POLICY_BYTES),
            &profile,
            vec![changed_file.clone()],
            vec![changed_fact.clone()],
            configuration.clone(),
        );
        assert!(!stale.valid);
        assert!(issue_codes(&stale.issues).contains(&"trace_index.binding_stale".into()));
        let next = evaluate_structural_trace_index(
            &registry(POLICY_BYTES),
            &profile,
            vec![changed_file],
            vec![changed_fact],
            configuration,
            Some(&original),
        )
        .index
        .unwrap();
        assert_eq!(
            next.structural.as_ref().unwrap().history[0].binding_blake3,
            original.structural.as_ref().unwrap().bindings[0].binding_blake3
        );
        assert!(validate_content_bound_trace_index(&next).is_empty());
        let mut corrupted = next;
        corrupted.structural.as_mut().unwrap().bindings[0]
            .item
            .normalized_item_blake3 = "b".repeat(64);
        assert!(issue_codes(&validate_content_bound_trace_index(&corrupted))
            .contains(&"trace_index.hash_invalid".into()));
    }
}

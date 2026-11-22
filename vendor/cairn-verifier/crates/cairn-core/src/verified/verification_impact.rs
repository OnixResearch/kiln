pub mod adapters;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::verified::check_evidence::{
    valid_applicability, whole_subject_evidence_current, Applicability, MAX_CHECKS,
};
use crate::verified::{
    content_bound_trace_index::{
        validate_content_bound_trace_index, ContentBoundTraceIndex, ContentBoundTraceLink,
    },
    evidence::canonical_json_hash,
};

pub const VERIFICATION_IMPACT_SCHEMA_V1: &str = "cairn.verification-impact.v1";
pub const VERIFICATION_CHECK_MAP_SCHEMA_V1: &str = "cairn.verification-check-map.v1";
pub const VERIFICATION_IMPACT_BOUNDARY: &str = "verification impact guidance is advisory check selection over admitted source, trace, and check-map facts; it does not execute checks, prove independence, satisfy full gates, or authorize evidence reuse";
pub const MAX_IMPACT_SOURCE_MEMBERS: usize = 2_048;
pub const MAX_IMPACT_CHANGED_PATHS: usize = 512;
pub const MAX_IMPACT_CHECKS: usize = 128;
pub const MAX_IMPACT_REQUIREMENT_BINDINGS: usize = 4_096;
pub const MAX_IMPACT_EDGES: usize = 4_096;
pub const MAX_IMPACT_FEATURES: usize = 64;
pub const MAX_IMPACT_PATH_BYTES: usize = 4_096;
pub const MAX_IMPACT_TEXT_BYTES: usize = 512;
pub const MAX_IMPACT_PRIOR_EVIDENCE: usize = 128;

const HEX_DIGEST_LENGTH: usize = 64;
const ISSUE_BOUNDS: &str = "verification_impact.bounds";
const ISSUE_CHECK_MAP: &str = "verification_impact.check_map";
const ISSUE_CHANGED_PATHS: &str = "verification_impact.changed_paths_mismatch";
const ISSUE_DUPLICATE: &str = "verification_impact.duplicate";
const ISSUE_FEATURE_SCOPE: &str = "verification_impact.feature_scope";
const ISSUE_GRAPH_CYCLE: &str = "verification_impact.graph_cycle";
const ISSUE_HASH: &str = "verification_impact.hash";
const ISSUE_PATH: &str = "verification_impact.path";
const ISSUE_SOURCE_STALE: &str = "verification_impact.source_stale";
const ISSUE_TRACE_INDEX: &str = "verification_impact.trace_index";
const ISSUE_UNKNOWN_CHECK: &str = "verification_impact.unknown_check";
const ISSUE_UNKNOWN_REQUIREMENT: &str = "verification_impact.unknown_requirement";
const ISSUE_PRIOR_EVIDENCE: &str = "verification_impact.prior_evidence";

const STALE_REASON_DEPENDENCIES: &str = "dependency_drift";
const STALE_REASON_POLICY: &str = "policy_changed";
const STALE_REASON_PROJECTION: &str = "projection_changed";
const STALE_REASON_SUBJECT: &str = "subject_changed";

const UNCERTAINTY_GENERATED_SOURCE: &str = "generated_source";
const UNCERTAINTY_INCOMPLETE_COVERAGE: &str = "incomplete_coverage";
const UNCERTAINTY_MACRO_OPAQUE: &str = "macro_opaque";
const UNCERTAINTY_MISSING_CHECK_MAPPING: &str = "missing_check_mapping";
const UNCERTAINTY_SCOPE_MISMATCH: &str = "scope_mismatch";
const UNCERTAINTY_UNLINKED_SOURCE: &str = "unlinked_source";

const NON_CLAIMS: &[&str] = &[
    "advisory guidance does not execute checks",
    "selected checks do not prove omitted checks are safe",
    "declared marker links are not behavioral proof",
    "whole-subject evidence reuse still requires the original applicability validator",
    "full gate obligations remain authoritative",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationImpactScope {
    pub repository_id: String,
    pub target: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationSourceMember {
    pub path: String,
    pub content_blake3: String,
    pub generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationSourceManifest {
    pub identity: String,
    pub scope: VerificationImpactScope,
    pub members: Vec<VerificationSourceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheckDefinition {
    pub check_id: String,
    pub target: String,
    pub features: Vec<String>,
    pub mandatory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRequirementCheck {
    pub requirement_id: String,
    pub check_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRequirementEdge {
    pub from_requirement_id: String,
    pub to_requirement_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheckMap {
    pub schema_version: String,
    pub map_id: String,
    pub coverage_complete: bool,
    pub default_check_ids: Vec<String>,
    pub checks: Vec<VerificationCheckDefinition>,
    pub requirement_checks: Vec<VerificationRequirementCheck>,
    pub dependency_edges: Vec<VerificationRequirementEdge>,
    /// Declared build-dependency identities for applicability evaluation.
    /// Sorted unique BLAKE3 digests; bounded like the producer applicability.
    pub dependency_hashes: Vec<String>,
    pub macro_opaque_paths: Vec<String>,
    pub generated_source_paths: Vec<String>,
    pub incomplete_source_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationImpactInput {
    pub schema_version: String,
    pub before_manifest: VerificationSourceManifest,
    pub after_manifest: VerificationSourceManifest,
    pub asserted_changed_paths: Vec<String>,
    pub before_trace_index: ContentBoundTraceIndex,
    pub after_trace_index: ContentBoundTraceIndex,
    pub check_map: VerificationCheckMap,
    /// Prior whole-subject completion receipts re-evaluated against the
    /// current state. Records carry the producer applicability they were
    /// admitted under; freshness never depends on marker links.
    pub prior_evidence: Vec<VerificationPriorEvidence>,
}

/// A prior whole-subject completion receipt bound to the applicability it was
/// admitted under and the declared checks it covered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationPriorEvidence {
    pub evidence_id: String,
    pub check_ids: Vec<String>,
    pub applicability: Applicability,
}

/// Applicability verdict for one prior receipt under the current state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEvidenceRefresh {
    pub evidence_id: String,
    pub check_ids: Vec<String>,
    pub current: bool,
    pub stale_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSourceChangeKind {
    Added,
    Deleted,
    Modified,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationSourceChange {
    pub path: String,
    pub kind: VerificationSourceChangeKind,
    pub before_blake3: Option<String>,
    pub after_blake3: Option<String>,
    pub counterpart_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationImpactCausalPath {
    pub source_path: String,
    pub requirement_id: String,
    pub check_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationImpactUncertainty {
    pub code: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationImpactIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationImpactReport {
    pub schema_version: String,
    pub valid: bool,
    pub boundary: String,
    pub input_hash: String,
    pub before_source_identity: String,
    pub after_source_identity: String,
    pub source_delta: Vec<VerificationSourceChange>,
    pub affected_requirements: Vec<String>,
    pub selected_checks: Vec<String>,
    pub full_required_checks: Vec<String>,
    pub causal_paths: Vec<VerificationImpactCausalPath>,
    pub uncertainty: Vec<VerificationImpactUncertainty>,
    pub evidence_refresh: Vec<VerificationEvidenceRefresh>,
    pub current_applicability: Option<Applicability>,
    pub blockers: Vec<VerificationImpactIssue>,
    pub non_claims: Vec<String>,
    pub semantic_hash: String,
}

#[must_use]
pub fn derive_verification_impact(input: &VerificationImpactInput) -> VerificationImpactReport {
    // r[impl cairn.verification_impact.inputs]
    // r[impl cairn.verification_impact.selection]
    let mut blockers = validate_input(input);
    let source_delta = derive_source_delta(&input.before_manifest, &input.after_manifest);
    let derived_changed_paths = changed_path_set(&source_delta);
    let asserted_changed_paths = input
        .asserted_changed_paths
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if derived_changed_paths != asserted_changed_paths {
        blockers.push(issue(
            ISSUE_CHANGED_PATHS,
            "asserted_changed_paths",
            "asserted changed paths must match the before/after source delta exactly",
        ));
    }
    sort_issues(&mut blockers);

    let input_hash = canonical_json_hash(&json!(input));
    let full_required_checks = full_required_checks(&input.check_map);
    if !blockers.is_empty() {
        return finalize_report(VerificationImpactReport {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            valid: false,
            boundary: VERIFICATION_IMPACT_BOUNDARY.to_string(),
            input_hash,
            before_source_identity: input.before_manifest.identity.clone(),
            after_source_identity: input.after_manifest.identity.clone(),
            source_delta,
            affected_requirements: Vec::new(),
            selected_checks: Vec::new(),
            full_required_checks,
            causal_paths: Vec::new(),
            uncertainty: Vec::new(),
            evidence_refresh: Vec::new(),
            current_applicability: None,
            blockers,
            non_claims: NON_CLAIMS
                .iter()
                .map(|claim| (*claim).to_string())
                .collect(),
            semantic_hash: String::new(),
        });
    }

    let changed_paths = changed_path_set(&source_delta);
    let changed_members = changed_member_map(&input.before_manifest, &input.after_manifest);
    let affected_requirement_paths = affected_requirement_paths(
        &input.before_trace_index.links,
        &input.after_trace_index.links,
        &changed_paths,
    );
    let mut affected_requirements = affected_requirement_paths
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let dependency_closure = requirement_closure(&affected_requirements, &input.check_map);
    affected_requirements.extend(dependency_closure.iter().cloned());

    let mut uncertainty = selection_uncertainty(
        &input.check_map,
        &changed_paths,
        &changed_members,
        &affected_requirements,
    );
    let requirement_checks = requirement_check_map(&input.check_map);
    let mut selected = mandatory_checks(&input.check_map);
    let mut causal_paths = Vec::new();
    for requirement_id in &affected_requirements {
        match requirement_checks.get(requirement_id) {
            Some(checks) => {
                for check_id in checks {
                    selected.insert(check_id.clone());
                    for source_path in affected_requirement_paths
                        .get(requirement_id)
                        .cloned()
                        .unwrap_or_default()
                    {
                        causal_paths.push(VerificationImpactCausalPath {
                            source_path,
                            requirement_id: requirement_id.clone(),
                            check_id: check_id.clone(),
                            reason: "source_to_requirement_to_check".to_string(),
                        });
                    }
                }
            }
            None => uncertainty.push(uncertainty_entry(
                UNCERTAINTY_MISSING_CHECK_MAPPING,
                requirement_id,
                "affected requirement has no admitted check mapping",
            )),
        }
    }
    for check_id in mandatory_checks(&input.check_map) {
        causal_paths.push(VerificationImpactCausalPath {
            source_path: "*".to_string(),
            requirement_id: "*".to_string(),
            check_id,
            reason: "mandatory_check_preserved".to_string(),
        });
    }
    if !uncertainty.is_empty() {
        selected.extend(input.check_map.default_check_ids.iter().cloned());
        for check_id in &input.check_map.default_check_ids {
            causal_paths.push(VerificationImpactCausalPath {
                source_path: "*".to_string(),
                requirement_id: "*".to_string(),
                check_id: check_id.clone(),
                reason: "conservative_default_due_to_uncertainty".to_string(),
            });
        }
    }
    sort_uncertainty(&mut uncertainty);
    sort_causal_paths(&mut causal_paths);
    let current_applicability = derive_current_applicability(input);
    let evidence_refresh = derive_evidence_refresh(input, &current_applicability);
    finalize_report(VerificationImpactReport {
        schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
        valid: true,
        boundary: VERIFICATION_IMPACT_BOUNDARY.to_string(),
        input_hash,
        before_source_identity: input.before_manifest.identity.clone(),
        after_source_identity: input.after_manifest.identity.clone(),
        source_delta,
        affected_requirements: affected_requirements.into_iter().collect(),
        selected_checks: selected.into_iter().collect(),
        full_required_checks,
        causal_paths,
        uncertainty,
        evidence_refresh,
        current_applicability: Some(current_applicability),
        blockers,
        non_claims: NON_CLAIMS
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
        semantic_hash: String::new(),
    })
}

fn validate_input(input: &VerificationImpactInput) -> Vec<VerificationImpactIssue> {
    let mut issues = Vec::new();
    if input.schema_version != VERIFICATION_IMPACT_SCHEMA_V1 {
        issues.push(issue(
            ISSUE_CHECK_MAP,
            "schema_version",
            "unsupported verification impact input schema",
        ));
    }
    validate_manifest(&input.before_manifest, "before_manifest", &mut issues);
    validate_manifest(&input.after_manifest, "after_manifest", &mut issues);
    validate_check_map(&input.check_map, &mut issues);
    validate_prior_evidence(input, &mut issues);
    validate_trace_index(
        &input.before_trace_index,
        &input.before_manifest,
        "before_trace_index",
        &mut issues,
    );
    validate_trace_index(
        &input.after_trace_index,
        &input.after_manifest,
        "after_trace_index",
        &mut issues,
    );
    if input.before_manifest.scope.repository_id != input.after_manifest.scope.repository_id {
        issues.push(issue(
            ISSUE_CHECK_MAP,
            "after_manifest.scope.repository_id",
            "before and after source manifests must name the same repository",
        ));
    }
    if input.before_manifest.scope.target != input.after_manifest.scope.target
        || input.before_manifest.scope.features != input.after_manifest.scope.features
    {
        issues.push(issue(
            ISSUE_FEATURE_SCOPE,
            "after_manifest.scope",
            "before and after source manifests must name the same target and feature scope",
        ));
    }
    issues
}

fn validate_manifest(
    manifest: &VerificationSourceManifest,
    field: &str,
    issues: &mut Vec<VerificationImpactIssue>,
) {
    if !digest(&manifest.identity) {
        issues.push(issue(
            ISSUE_HASH,
            &format!("{field}.identity"),
            "source manifest identity must be a lowercase BLAKE3 hex digest",
        ));
    }
    validate_scope(&manifest.scope, &format!("{field}.scope"), issues);
    if manifest.members.len() > MAX_IMPACT_SOURCE_MEMBERS {
        issues.push(issue(
            ISSUE_BOUNDS,
            &format!("{field}.members"),
            "source manifest member count exceeds the named verification-impact bound",
        ));
    }
    let mut seen = BTreeSet::new();
    for (index, member) in manifest.members.iter().enumerate() {
        let member_field = format!("{field}.members[{index}]");
        validate_path(&member.path, &format!("{member_field}.path"), issues);
        if !digest(&member.content_blake3) {
            issues.push(issue(
                ISSUE_HASH,
                &format!("{member_field}.content_blake3"),
                "source member content identity must be a lowercase BLAKE3 hex digest",
            ));
        }
        if !seen.insert(member.path.clone()) {
            issues.push(issue(
                ISSUE_DUPLICATE,
                &format!("{member_field}.path"),
                "source manifest paths must be unique",
            ));
        }
    }
}

fn validate_scope(
    scope: &VerificationImpactScope,
    field: &str,
    issues: &mut Vec<VerificationImpactIssue>,
) {
    if scope.repository_id.is_empty() || scope.repository_id.len() > MAX_IMPACT_TEXT_BYTES {
        issues.push(issue(
            ISSUE_BOUNDS,
            &format!("{field}.repository_id"),
            "repository identity must be non-empty and bounded",
        ));
    }
    if !label(&scope.target) {
        issues.push(issue(
            ISSUE_CHECK_MAP,
            &format!("{field}.target"),
            "target must use the verification-impact label grammar",
        ));
    }
    if !canonical(&scope.features) || scope.features.len() > MAX_IMPACT_FEATURES {
        issues.push(issue(
            ISSUE_FEATURE_SCOPE,
            &format!("{field}.features"),
            "feature scope must be sorted, unique, and bounded",
        ));
    }
    for feature in &scope.features {
        if !label(feature) {
            issues.push(issue(
                ISSUE_FEATURE_SCOPE,
                &format!("{field}.features"),
                "feature identifiers must use the verification-impact label grammar",
            ));
        }
    }
}

fn validate_check_map(map: &VerificationCheckMap, issues: &mut Vec<VerificationImpactIssue>) {
    if map.schema_version != VERIFICATION_CHECK_MAP_SCHEMA_V1 {
        issues.push(issue(
            ISSUE_CHECK_MAP,
            "check_map.schema_version",
            "unsupported verification check-map schema",
        ));
    }
    if !label(&map.map_id) {
        issues.push(issue(
            ISSUE_CHECK_MAP,
            "check_map.map_id",
            "check-map identity must use the verification-impact label grammar",
        ));
    }
    if map.checks.len() > MAX_IMPACT_CHECKS
        || map.requirement_checks.len() > MAX_IMPACT_REQUIREMENT_BINDINGS
        || map.dependency_edges.len() > MAX_IMPACT_EDGES
    {
        issues.push(issue(
            ISSUE_BOUNDS,
            "check_map",
            "check-map entries exceed named verification-impact bounds",
        ));
    }
    let mut check_ids = BTreeSet::new();
    for (index, check) in map.checks.iter().enumerate() {
        let field = format!("check_map.checks[{index}]");
        if !check_ids.insert(check.check_id.clone()) {
            issues.push(issue(
                ISSUE_DUPLICATE,
                &format!("{field}.check_id"),
                "check ids must be unique",
            ));
        }
        if !label(&check.check_id) || !label(&check.target) {
            issues.push(issue(
                ISSUE_CHECK_MAP,
                &field,
                "check id and target must use the verification-impact label grammar",
            ));
        }
        if !canonical(&check.features) || check.features.len() > MAX_IMPACT_FEATURES {
            issues.push(issue(
                ISSUE_FEATURE_SCOPE,
                &format!("{field}.features"),
                "check feature scope must be sorted, unique, and bounded",
            ));
        }
    }
    for check_id in &map.default_check_ids {
        if !check_ids.contains(check_id) {
            issues.push(issue(
                ISSUE_UNKNOWN_CHECK,
                "check_map.default_check_ids",
                "default verification check must be declared",
            ));
        }
    }
    for binding in &map.requirement_checks {
        if !trace_requirement_id(&binding.requirement_id) {
            issues.push(issue(
                ISSUE_UNKNOWN_REQUIREMENT,
                "check_map.requirement_checks.requirement_id",
                "requirement-check binding must use a bounded requirement id",
            ));
        }
        if !check_ids.contains(&binding.check_id) {
            issues.push(issue(
                ISSUE_UNKNOWN_CHECK,
                "check_map.requirement_checks.check_id",
                "requirement-check binding references an unknown check",
            ));
        }
    }
    validate_path_lists(map, issues);
    if map.dependency_hashes.len() > MAX_CHECKS {
        issues.push(issue(
            ISSUE_BOUNDS,
            "check_map.dependency_hashes",
            "declared dependency identities exceed the shared applicability bound",
        ));
    }
    if !canonical(&map.dependency_hashes) || map.dependency_hashes.iter().any(|h| !digest(h)) {
        issues.push(issue(
            ISSUE_CHECK_MAP,
            "check_map.dependency_hashes",
            "declared dependency identities must be sorted, unique BLAKE3 digests",
        ));
    }
    if dependency_cycle(map) {
        issues.push(issue(
            ISSUE_GRAPH_CYCLE,
            "check_map.dependency_edges",
            "requirement dependency edges must be acyclic",
        ));
    }
}

fn validate_path_lists(map: &VerificationCheckMap, issues: &mut Vec<VerificationImpactIssue>) {
    for (field, paths) in [
        ("check_map.macro_opaque_paths", &map.macro_opaque_paths),
        (
            "check_map.generated_source_paths",
            &map.generated_source_paths,
        ),
        (
            "check_map.incomplete_source_paths",
            &map.incomplete_source_paths,
        ),
    ] {
        if !canonical(paths) {
            issues.push(issue(
                ISSUE_CHECK_MAP,
                field,
                "path lists must be sorted and unique",
            ));
        }
        for path in paths {
            validate_path(path, field, issues);
        }
    }
}

fn validate_trace_index(
    index: &ContentBoundTraceIndex,
    manifest: &VerificationSourceManifest,
    field: &str,
    issues: &mut Vec<VerificationImpactIssue>,
) {
    let trace_issues = validate_content_bound_trace_index(index);
    if !trace_issues.is_empty() {
        issues.push(issue(
            ISSUE_TRACE_INDEX,
            field,
            "trace index failed content-bound validation",
        ));
    }
    let trace_sources = index
        .sources
        .iter()
        .map(|source| (source.path.clone(), source.content_blake3.clone()))
        .collect::<BTreeMap<_, _>>();
    let manifest_sources = manifest
        .members
        .iter()
        .map(|source| (source.path.clone(), source.content_blake3.clone()))
        .collect::<BTreeMap<_, _>>();
    if trace_sources != manifest_sources {
        issues.push(issue(
            ISSUE_SOURCE_STALE,
            field,
            "trace-index sources must match the admitted source manifest exactly",
        ));
    }
}

fn validate_prior_evidence(
    input: &VerificationImpactInput,
    issues: &mut Vec<VerificationImpactIssue>,
) {
    if input.prior_evidence.len() > MAX_IMPACT_PRIOR_EVIDENCE {
        issues.push(issue(
            ISSUE_BOUNDS,
            "prior_evidence",
            "prior evidence record count exceeds the named verification-impact bound",
        ));
    }
    let declared_checks = input
        .check_map
        .checks
        .iter()
        .map(|check| check.check_id.clone())
        .collect::<BTreeSet<_>>();
    let mut seen_evidence = BTreeSet::new();
    for (index, record) in input.prior_evidence.iter().enumerate() {
        let field = format!("prior_evidence[{index}]");
        if !label(&record.evidence_id) {
            issues.push(issue(
                ISSUE_PRIOR_EVIDENCE,
                &format!("{field}.evidence_id"),
                "prior evidence identity must use the verification-impact label grammar",
            ));
        }
        if !seen_evidence.insert(record.evidence_id.clone()) {
            issues.push(issue(
                ISSUE_DUPLICATE,
                &format!("{field}.evidence_id"),
                "prior evidence identities must be unique",
            ));
        }
        if record.check_ids.is_empty()
            || !canonical(&record.check_ids)
            || record
                .check_ids
                .iter()
                .any(|check_id| !declared_checks.contains(check_id))
        {
            issues.push(issue(
                ISSUE_PRIOR_EVIDENCE,
                &format!("{field}.check_ids"),
                "prior evidence must cover declared checks in sorted unique order",
            ));
        }
        if !valid_applicability(&record.applicability) {
            issues.push(issue(
                ISSUE_PRIOR_EVIDENCE,
                &format!("{field}.applicability"),
                "prior evidence applicability failed the shared applicability validator",
            ));
        }
    }
}

fn derive_current_applicability(input: &VerificationImpactInput) -> Applicability {
    // The repository-owned check map is the declared policy. Its identity is
    // derived from sorted canonical content so input ordering cannot drift it.
    let checks = input
        .check_map
        .checks
        .iter()
        .map(|check| check.check_id.as_str())
        .collect::<BTreeSet<_>>();
    let bindings = input
        .check_map
        .requirement_checks
        .iter()
        .map(|binding| (binding.requirement_id.as_str(), binding.check_id.as_str()))
        .collect::<BTreeSet<_>>();
    let edges = input
        .check_map
        .dependency_edges
        .iter()
        .map(|edge| {
            (
                edge.from_requirement_id.as_str(),
                edge.to_requirement_id.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let defaults = input
        .check_map
        .default_check_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    Applicability {
        subject_hash: input.after_manifest.identity.clone(),
        policy_hash: canonical_json_hash(&json!({
            "schema": input.check_map.schema_version,
            "map_id": input.check_map.map_id,
            "coverage_complete": input.check_map.coverage_complete,
            "default_check_ids": defaults,
            "checks": checks,
            "requirement_checks": bindings,
            "dependency_edges": edges,
        })),
        dependency_hashes: input.check_map.dependency_hashes.clone(),
        source_projection_hash: projection_hash(&input.after_manifest),
    }
}

fn projection_hash(manifest: &VerificationSourceManifest) -> String {
    let paths = manifest
        .members
        .iter()
        .map(|member| member.path.as_str())
        .collect::<BTreeSet<_>>();
    canonical_json_hash(&json!(paths))
}

fn derive_evidence_refresh(
    input: &VerificationImpactInput,
    current: &Applicability,
) -> Vec<VerificationEvidenceRefresh> {
    let mut refresh = input
        .prior_evidence
        .iter()
        .map(|record| {
            // Freshness comes from the shared whole-subject validator, never
            // from marker links: an unlinked source change still changes the
            // subject and keeps old whole-subject receipts stale.
            let current_evidence = whole_subject_evidence_current(&record.applicability, current);
            let mut stale_reasons = Vec::new();
            if record.applicability.subject_hash != current.subject_hash {
                stale_reasons.push(STALE_REASON_SUBJECT.to_string());
            }
            if record.applicability.policy_hash != current.policy_hash {
                stale_reasons.push(STALE_REASON_POLICY.to_string());
            }
            if record.applicability.dependency_hashes != current.dependency_hashes {
                stale_reasons.push(STALE_REASON_DEPENDENCIES.to_string());
            }
            if record.applicability.source_projection_hash != current.source_projection_hash {
                stale_reasons.push(STALE_REASON_PROJECTION.to_string());
            }
            stale_reasons.sort();
            VerificationEvidenceRefresh {
                evidence_id: record.evidence_id.clone(),
                check_ids: record.check_ids.clone(),
                current: current_evidence,
                stale_reasons,
            }
        })
        .collect::<Vec<_>>();
    refresh.sort_by(|left, right| {
        right
            .current
            .cmp(&left.current)
            .then(left.evidence_id.cmp(&right.evidence_id))
    });
    refresh
}

fn derive_source_delta(
    before: &VerificationSourceManifest,
    after: &VerificationSourceManifest,
) -> Vec<VerificationSourceChange> {
    let before_members = member_map(before);
    let after_members = member_map(after);
    let mut added = after_members
        .keys()
        .filter(|path| !before_members.contains_key(*path))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut deleted = before_members
        .keys()
        .filter(|path| !after_members.contains_key(*path))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();

    let added_by_hash = added
        .iter()
        .filter_map(|path| {
            after_members
                .get(path)
                .map(|member| (member.content_blake3.clone(), path))
        })
        .collect::<BTreeMap<_, _>>();
    let mut renamed_added = BTreeSet::new();
    let mut renamed_deleted = BTreeSet::new();
    for deleted_path in &deleted {
        let Some(before_member) = before_members.get(deleted_path) else {
            continue;
        };
        if let Some(added_path) = added_by_hash.get(&before_member.content_blake3) {
            renamed_added.insert((*added_path).clone());
            renamed_deleted.insert(deleted_path.clone());
            changes.push(VerificationSourceChange {
                path: deleted_path.clone(),
                kind: VerificationSourceChangeKind::Renamed,
                before_blake3: Some(before_member.content_blake3.clone()),
                after_blake3: Some(before_member.content_blake3.clone()),
                counterpart_path: Some((*added_path).clone()),
            });
        }
    }
    added.retain(|path| !renamed_added.contains(path));
    deleted.retain(|path| !renamed_deleted.contains(path));

    for path in deleted {
        let before_member = before_members
            .get(&path)
            .expect("path from before map exists");
        changes.push(VerificationSourceChange {
            path,
            kind: VerificationSourceChangeKind::Deleted,
            before_blake3: Some(before_member.content_blake3.clone()),
            after_blake3: None,
            counterpart_path: None,
        });
    }
    for path in added {
        let after_member = after_members
            .get(&path)
            .expect("path from after map exists");
        changes.push(VerificationSourceChange {
            path,
            kind: VerificationSourceChangeKind::Added,
            before_blake3: None,
            after_blake3: Some(after_member.content_blake3.clone()),
            counterpart_path: None,
        });
    }
    for path in before_members
        .keys()
        .filter(|path| after_members.contains_key(*path))
    {
        let before_member = before_members
            .get(path)
            .expect("path from before map exists");
        let after_member = after_members.get(path).expect("path from after map exists");
        if before_member.content_blake3 != after_member.content_blake3 {
            changes.push(VerificationSourceChange {
                path: path.clone(),
                kind: VerificationSourceChangeKind::Modified,
                before_blake3: Some(before_member.content_blake3.clone()),
                after_blake3: Some(after_member.content_blake3.clone()),
                counterpart_path: None,
            });
        }
    }
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    changes
}

fn member_map(manifest: &VerificationSourceManifest) -> BTreeMap<String, VerificationSourceMember> {
    manifest
        .members
        .iter()
        .map(|member| (member.path.clone(), member.clone()))
        .collect()
}

fn changed_path_set(source_delta: &[VerificationSourceChange]) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for change in source_delta {
        paths.insert(change.path.clone());
        if let Some(counterpart) = &change.counterpart_path {
            paths.insert(counterpart.clone());
        }
    }
    paths
}

fn changed_member_map(
    before: &VerificationSourceManifest,
    after: &VerificationSourceManifest,
) -> BTreeMap<String, VerificationSourceMember> {
    let mut members = BTreeMap::new();
    for manifest in [before, after] {
        for member in &manifest.members {
            members.insert(member.path.clone(), member.clone());
        }
    }
    members
}

fn affected_requirement_paths(
    before_links: &[ContentBoundTraceLink],
    after_links: &[ContentBoundTraceLink],
    changed_paths: &BTreeSet<String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut affected = BTreeMap::new();
    for link in before_links.iter().chain(after_links.iter()) {
        if changed_paths.contains(&link.source_path) {
            affected
                .entry(link.requirement_id.clone())
                .or_insert_with(BTreeSet::new)
                .insert(link.source_path.clone());
        }
    }
    affected
}

fn requirement_closure(
    requirements: &BTreeSet<String>,
    map: &VerificationCheckMap,
) -> BTreeSet<String> {
    let mut closure = requirements.clone();
    loop {
        let before = closure.len();
        for edge in &map.dependency_edges {
            if closure.contains(&edge.from_requirement_id) {
                closure.insert(edge.to_requirement_id.clone());
            }
        }
        if before == closure.len() {
            break;
        }
    }
    closure
}

fn requirement_check_map(map: &VerificationCheckMap) -> BTreeMap<String, BTreeSet<String>> {
    let mut mapped = BTreeMap::new();
    for binding in &map.requirement_checks {
        mapped
            .entry(binding.requirement_id.clone())
            .or_insert_with(BTreeSet::new)
            .insert(binding.check_id.clone());
    }
    mapped
}

fn mandatory_checks(map: &VerificationCheckMap) -> BTreeSet<String> {
    map.checks
        .iter()
        .filter(|check| check.mandatory)
        .map(|check| check.check_id.clone())
        .collect()
}

fn full_required_checks(map: &VerificationCheckMap) -> Vec<String> {
    let mut checks = mandatory_checks(map);
    checks.extend(map.default_check_ids.iter().cloned());
    checks.into_iter().collect()
}

fn selection_uncertainty(
    map: &VerificationCheckMap,
    changed_paths: &BTreeSet<String>,
    changed_members: &BTreeMap<String, VerificationSourceMember>,
    affected_requirements: &BTreeSet<String>,
) -> Vec<VerificationImpactUncertainty> {
    let mut uncertainty = Vec::new();
    if !map.coverage_complete {
        uncertainty.push(uncertainty_entry(
            UNCERTAINTY_INCOMPLETE_COVERAGE,
            &map.map_id,
            "check-map declares incomplete dependency coverage",
        ));
    }
    for path in changed_paths {
        if map.incomplete_source_paths.contains(path) {
            uncertainty.push(uncertainty_entry(
                UNCERTAINTY_INCOMPLETE_COVERAGE,
                path,
                "changed source path has incomplete dependency coverage",
            ));
        }
        if map.macro_opaque_paths.contains(path) {
            uncertainty.push(uncertainty_entry(
                UNCERTAINTY_MACRO_OPAQUE,
                path,
                "changed source path crosses an opaque macro expansion boundary",
            ));
        }
        if map.generated_source_paths.contains(path)
            || changed_members
                .get(path)
                .is_some_and(|member| member.generated)
        {
            uncertainty.push(uncertainty_entry(
                UNCERTAINTY_GENERATED_SOURCE,
                path,
                "changed source path is generated or has an unavailable generated-source edge",
            ));
        }
        if !affected_requirements.iter().any(|requirement_id| {
            requirement_check_map(map)
                .get(requirement_id)
                .is_some_and(|checks| !checks.is_empty())
        }) {
            uncertainty.push(uncertainty_entry(
                UNCERTAINTY_UNLINKED_SOURCE,
                path,
                "changed source path has no admitted requirement-to-check explanation",
            ));
        }
    }
    for check in &map.checks {
        if changed_scope_mismatch(check) {
            uncertainty.push(uncertainty_entry(
                UNCERTAINTY_SCOPE_MISMATCH,
                &check.check_id,
                "check definition has no admitted target or feature scope",
            ));
        }
    }
    uncertainty
}

fn changed_scope_mismatch(check: &VerificationCheckDefinition) -> bool {
    check.target.is_empty() || !canonical(&check.features)
}

fn dependency_cycle(map: &VerificationCheckMap) -> bool {
    let mut edges = BTreeMap::<&str, Vec<&str>>::new();
    for edge in &map.dependency_edges {
        edges
            .entry(edge.from_requirement_id.as_str())
            .or_default()
            .push(edge.to_requirement_id.as_str());
    }
    let mut nodes = BTreeSet::new();
    for edge in &map.dependency_edges {
        nodes.insert(edge.from_requirement_id.as_str());
        nodes.insert(edge.to_requirement_id.as_str());
    }
    for node in nodes {
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        if visit_cycle(node, &edges, &mut visiting, &mut visited) {
            return true;
        }
    }
    false
}

fn visit_cycle<'a>(
    node: &'a str,
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    if visited.contains(node) {
        return false;
    }
    if !visiting.insert(node) {
        return true;
    }
    if let Some(children) = edges.get(node) {
        for child in children {
            if visit_cycle(child, edges, visiting, visited) {
                return true;
            }
        }
    }
    visiting.remove(node);
    visited.insert(node);
    false
}

fn finalize_report(mut report: VerificationImpactReport) -> VerificationImpactReport {
    let mut semantic = report.clone();
    semantic.semantic_hash.clear();
    report.semantic_hash = canonical_json_hash(&json!(semantic));
    report
}

fn uncertainty_entry(code: &str, target: &str, message: &str) -> VerificationImpactUncertainty {
    VerificationImpactUncertainty {
        code: code.to_string(),
        target: target.to_string(),
        message: message.to_string(),
    }
}

fn issue(code: &str, field_path: &str, message: &str) -> VerificationImpactIssue {
    VerificationImpactIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn sort_issues(issues: &mut Vec<VerificationImpactIssue>) {
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
}

fn sort_uncertainty(uncertainty: &mut Vec<VerificationImpactUncertainty>) {
    uncertainty.sort_by(|left, right| {
        left.target
            .cmp(&right.target)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    uncertainty.dedup();
}

fn sort_causal_paths(paths: &mut Vec<VerificationImpactCausalPath>) {
    paths.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then(left.requirement_id.cmp(&right.requirement_id))
            .then(left.check_id.cmp(&right.check_id))
            .then(left.reason.cmp(&right.reason))
    });
    paths.dedup();
}

fn validate_path(path: &str, field: &str, issues: &mut Vec<VerificationImpactIssue>) {
    if !safe_relative_path(path) {
        issues.push(issue(
            ISSUE_PATH,
            field,
            "source paths must be non-empty safe relative paths within the admitted projection",
        ));
    }
}

fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_IMPACT_PATH_BYTES
        && !path.starts_with('/')
        && !path.contains("//")
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
}

fn digest(value: &str) -> bool {
    value.len() == HEX_DIGEST_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IMPACT_TEXT_BYTES
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

fn trace_requirement_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IMPACT_TEXT_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

#[cfg(test)]
mod tests {
    use crate::verified::{
        content_bound_requirement_registry::{
            evaluate_content_bound_requirement_registry, ContentBoundRevision,
            ContentBoundSpecificationInput, CONTENT_BOUND_REVISION_KIND_OPAQUE,
        },
        content_bound_trace_index::evaluate_content_bound_trace_index,
        traceability::{TraceabilityFile, TraceabilityProfile, TraceabilitySource},
    };

    use super::*;

    const REPOSITORY_ID: &str = "onixresearch/cairn";
    const REQUIREMENT_ID: &str = "cairn.verification.demo";
    const RELATED_REQUIREMENT_ID: &str = "cairn.verification.related";
    const SOURCE_PATH: &str = "crates/demo/src/lib.rs";
    const RENAME_PATH: &str = "crates/demo/src/renamed.rs";
    const UNLINKED_PATH: &str = "crates/demo/src/unlinked.rs";
    const DEFAULT_CHECK: &str = "workspace-tests";
    const LINKED_CHECK: &str = "demo-tests";
    const RELATED_CHECK: &str = "related-tests";
    const MANDATORY_CHECK: &str = "policy-fresh";

    fn hash(seed: char) -> String {
        seed.to_string().repeat(HEX_DIGEST_LENGTH)
    }

    fn scope() -> VerificationImpactScope {
        VerificationImpactScope {
            repository_id: REPOSITORY_ID.to_string(),
            target: "x86_64-linux".to_string(),
            features: vec!["default".to_string()],
        }
    }

    fn spec_input() -> ContentBoundSpecificationInput {
        ContentBoundSpecificationInput {
            path: ".cairn/specs/verification/spec.md".to_string(),
            markdown: format!(
                "# Verification\n\n## Requirements\n\n### Requirement: Demo\nr[{REQUIREMENT_ID}] The system MUST select checks.\n\n#### Scenario: Linked\n- GIVEN a link\n- WHEN changed\n- THEN it selects a check\n\n### Requirement: Related\nr[{RELATED_REQUIREMENT_ID}] The system MUST preserve dependencies.\n\n#### Scenario: Related\n- GIVEN an edge\n- WHEN changed\n- THEN it keeps the edge\n"
            ),
        }
    }

    fn registry(policy: &[u8]) -> crate::ContentBoundRequirementRegistry {
        evaluate_content_bound_requirement_registry(
            vec![spec_input()],
            policy,
            REPOSITORY_ID,
            ContentBoundRevision {
                kind: CONTENT_BOUND_REVISION_KIND_OPAQUE.to_string(),
                value: "verification-impact-test".to_string(),
            },
        )
        .registry
        .expect("valid registry")
    }

    fn profile() -> TraceabilityProfile {
        TraceabilityProfile {
            id: "verification-impact".to_string(),
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
            marker_verbs: vec!["impl".to_string(), "verify".to_string()],
            assurance_level: "declared".to_string(),
            anchor_kind: "exact_marker_bytes".to_string(),
            rust_configuration_path: None,
        }
    }

    fn trace_file(path: &str, body: &str) -> TraceabilityFile {
        TraceabilityFile {
            path: path.to_string(),
            text: body.to_string(),
        }
    }

    fn trace_index(files: Vec<TraceabilityFile>) -> ContentBoundTraceIndex {
        evaluate_content_bound_trace_index(&registry(b"policy"), &profile(), files)
            .index
            .expect("valid trace index")
    }

    fn manifest(index: &ContentBoundTraceIndex, identity: &str) -> VerificationSourceManifest {
        VerificationSourceManifest {
            identity: identity.to_string(),
            scope: scope(),
            members: index
                .sources
                .iter()
                .map(|source| VerificationSourceMember {
                    path: source.path.clone(),
                    content_blake3: source.content_blake3.clone(),
                    generated: false,
                })
                .collect(),
        }
    }

    fn check_map() -> VerificationCheckMap {
        VerificationCheckMap {
            schema_version: VERIFICATION_CHECK_MAP_SCHEMA_V1.to_string(),
            map_id: "cairn-local".to_string(),
            coverage_complete: true,
            default_check_ids: vec![DEFAULT_CHECK.to_string()],
            checks: vec![
                check(DEFAULT_CHECK, false),
                check(LINKED_CHECK, false),
                check(MANDATORY_CHECK, true),
                check(RELATED_CHECK, false),
            ],
            requirement_checks: vec![
                VerificationRequirementCheck {
                    requirement_id: REQUIREMENT_ID.to_string(),
                    check_id: LINKED_CHECK.to_string(),
                },
                VerificationRequirementCheck {
                    requirement_id: RELATED_REQUIREMENT_ID.to_string(),
                    check_id: RELATED_CHECK.to_string(),
                },
            ],
            dependency_edges: Vec::new(),
            dependency_hashes: Vec::new(),
            macro_opaque_paths: Vec::new(),
            generated_source_paths: Vec::new(),
            incomplete_source_paths: Vec::new(),
        }
    }

    fn check(check_id: &str, mandatory: bool) -> VerificationCheckDefinition {
        VerificationCheckDefinition {
            check_id: check_id.to_string(),
            target: "x86_64-linux".to_string(),
            features: vec!["default".to_string()],
            mandatory,
        }
    }

    fn linked_body(prefix: &str) -> String {
        format!("{prefix}// r[impl {REQUIREMENT_ID}]\n")
    }

    fn base_input() -> VerificationImpactInput {
        let before_trace_index = trace_index(vec![trace_file(SOURCE_PATH, &linked_body(""))]);
        let after_trace_index =
            trace_index(vec![trace_file(SOURCE_PATH, &linked_body("// changed\n"))]);
        VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(&before_trace_index, &hash('a')),
            after_manifest: manifest(&after_trace_index, &hash('b')),
            asserted_changed_paths: vec![SOURCE_PATH.to_string()],
            before_trace_index,
            after_trace_index,
            check_map: check_map(),
            prior_evidence: Vec::new(),
        }
    }

    fn codes(report: &VerificationImpactReport) -> Vec<String> {
        report
            .blockers
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    // r[verify cairn.verification_impact.uncertainty]
    fn structural_admission_preserves_fallback_and_rejects_substitutions() {
        use super::adapters::*;
        use crate::verified::check_evidence::OCTET_REVISION;
        let input = base_input();
        let receipt = serde_json::from_str(include_str!(
            "../../../../fixtures/check-evidence/octet-producer-fixture.json"
        ))
        .expect("published Octet fixture");
        let evidence = VerificationStructuralEvidence {
            schema_version: STRUCTURAL_EVIDENCE_SCHEMA.to_string(),
            source_identity: input.after_manifest.identity.clone(),
            scope: input.after_manifest.scope.clone(),
            octet: Some(BoundOctetReceipt {
                producer_revision: OCTET_REVISION.to_string(),
                receipt_hash: canonical_json_hash(&receipt),
                receipt,
            }),
            valence: None,
        };
        let admitted = admit_verification_structural_evidence(&input, Some(&evidence));
        assert!(admitted.valid);
        assert!(!admitted.completion_eligible);
        assert!(!admitted.octet.unwrap().completion_eligible);
        assert_eq!(admitted.selection_scope, DECLARED_LINK_FALLBACK);
        let fallback = admit_verification_structural_evidence(&input, None);
        assert!(fallback.valid);
        assert_eq!(fallback.selection_scope, DECLARED_LINK_FALLBACK);
        for field in [
            "schema", "source", "target", "features", "cohort", "hash", "receipt", "valence",
        ] {
            let mut bad = evidence.clone();
            match field {
                "schema" => bad.schema_version = "substituted".into(),
                "source" => bad.source_identity = hash('c'),
                "target" => bad.scope.target = "wasm32-unknown-unknown".into(),
                "features" => bad.scope.features.clear(),
                "cohort" => bad.octet.as_mut().unwrap().producer_revision = "substituted".into(),
                "hash" => bad.octet.as_mut().unwrap().receipt_hash = hash('c'),
                "receipt" => {
                    let octet = bad.octet.as_mut().unwrap();
                    octet.receipt["schema_version"] = json!("substituted");
                    octet.receipt_hash = canonical_json_hash(&octet.receipt);
                }
                "valence" => {
                    bad.valence =
                        Some(json!({"schema":"valence.evidence-admission.v1", "valid":true}))
                }
                _ => unreachable!(),
            }
            let report = admit_verification_structural_evidence(&input, Some(&bad));
            assert!(!report.valid, "admitted {field}");
            assert!(report.octet.is_none());
            assert!(!report.completion_eligible);
        }
    }

    #[test]
    fn linked_edit_selects_declared_and_mandatory_checks() {
        // r[verify cairn.verification_impact.selection]
        let report = derive_verification_impact(&base_input());
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert_eq!(
            report.affected_requirements,
            vec![REQUIREMENT_ID.to_string()]
        );
        assert_eq!(
            report.selected_checks,
            vec![LINKED_CHECK.to_string(), MANDATORY_CHECK.to_string()]
        );
        assert!(report.causal_paths.iter().any(|path| {
            path.source_path == SOURCE_PATH
                && path.requirement_id == REQUIREMENT_ID
                && path.check_id == LINKED_CHECK
        }));
        assert!(report.uncertainty.is_empty());
    }

    #[test]
    fn deletion_uses_before_trace_links() {
        // r[verify cairn.verification_impact.selection]
        let before_trace_index = trace_index(vec![trace_file(SOURCE_PATH, &linked_body(""))]);
        let after_trace_index = trace_index(Vec::new());
        let input = VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(&before_trace_index, &hash('a')),
            after_manifest: manifest(&after_trace_index, &hash('b')),
            asserted_changed_paths: vec![SOURCE_PATH.to_string()],
            before_trace_index,
            after_trace_index,
            check_map: check_map(),
            prior_evidence: Vec::new(),
        };
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert_eq!(
            report.source_delta[0].kind,
            VerificationSourceChangeKind::Deleted
        );
        assert!(report.selected_checks.contains(&LINKED_CHECK.to_string()));
    }

    #[test]
    fn renames_preserve_both_paths_and_do_not_trust_names() {
        // r[verify cairn.verification_impact.selection]
        let before_trace_index = trace_index(vec![trace_file(SOURCE_PATH, &linked_body(""))]);
        let after_trace_index = trace_index(vec![trace_file(RENAME_PATH, &linked_body(""))]);
        let input = VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(&before_trace_index, &hash('a')),
            after_manifest: manifest(&after_trace_index, &hash('b')),
            asserted_changed_paths: vec![RENAME_PATH.to_string(), SOURCE_PATH.to_string()],
            before_trace_index,
            after_trace_index,
            check_map: check_map(),
            prior_evidence: Vec::new(),
        };
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert_eq!(
            report.source_delta[0].kind,
            VerificationSourceChangeKind::Renamed
        );
        assert_eq!(
            report.source_delta[0].counterpart_path,
            Some(RENAME_PATH.to_string())
        );
        assert!(report.selected_checks.contains(&LINKED_CHECK.to_string()));
    }

    #[test]
    fn missing_mapping_and_unlinked_source_select_the_broader_default() {
        // r[verify cairn.verification_impact.uncertainty]
        let before_trace_index = trace_index(vec![trace_file(UNLINKED_PATH, "fn before() {}\n")]);
        let after_trace_index = trace_index(vec![trace_file(UNLINKED_PATH, "fn after() {}\n")]);
        let input = VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(&before_trace_index, &hash('a')),
            after_manifest: manifest(&after_trace_index, &hash('b')),
            asserted_changed_paths: vec![UNLINKED_PATH.to_string()],
            before_trace_index,
            after_trace_index,
            check_map: check_map(),
            prior_evidence: Vec::new(),
        };
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert!(report.selected_checks.contains(&DEFAULT_CHECK.to_string()));
        assert!(report
            .selected_checks
            .contains(&MANDATORY_CHECK.to_string()));
        assert!(report
            .uncertainty
            .iter()
            .any(|entry| entry.code == UNCERTAINTY_UNLINKED_SOURCE));
    }

    #[test]
    fn stale_trace_manifest_comparison_blocks_admission() {
        // r[verify cairn.verification_impact.inputs]
        let mut input = base_input();
        input.after_manifest.members[0].content_blake3 = hash('c');
        let report = derive_verification_impact(&input);
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_SOURCE_STALE.to_string()));
    }

    #[test]
    fn changed_path_assertion_cannot_hide_extra_delta() {
        // r[verify cairn.verification_impact.inputs]
        let mut input = base_input();
        input.asserted_changed_paths.clear();
        let report = derive_verification_impact(&input);
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_CHANGED_PATHS.to_string()));
    }

    #[test]
    fn copied_marker_is_visible_as_a_new_changed_path() {
        // r[verify cairn.verification_impact.selection]
        let before_trace_index = trace_index(vec![trace_file(SOURCE_PATH, &linked_body(""))]);
        let after_trace_index = trace_index(vec![
            trace_file(SOURCE_PATH, &linked_body("")),
            trace_file(RENAME_PATH, &linked_body("")),
        ]);
        let input = VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(&before_trace_index, &hash('a')),
            after_manifest: manifest(&after_trace_index, &hash('b')),
            asserted_changed_paths: vec![RENAME_PATH.to_string()],
            before_trace_index,
            after_trace_index,
            check_map: check_map(),
            prior_evidence: Vec::new(),
        };
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert!(report.selected_checks.contains(&LINKED_CHECK.to_string()));
        assert!(report.causal_paths.iter().any(|path| {
            path.source_path == RENAME_PATH
                && path.requirement_id == REQUIREMENT_ID
                && path.check_id == LINKED_CHECK
        }));
    }

    #[test]
    fn missing_dependency_coverage_selects_broader_default() {
        // r[verify cairn.verification_impact.uncertainty]
        let mut input = base_input();
        input.check_map.coverage_complete = false;
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert!(report.selected_checks.contains(&DEFAULT_CHECK.to_string()));
        assert!(report
            .uncertainty
            .iter()
            .any(|entry| entry.code == UNCERTAINTY_INCOMPLETE_COVERAGE));
    }

    #[test]
    fn target_and_feature_scope_drift_blocks_admission() {
        // r[verify cairn.verification_impact.inputs]
        let mut input = base_input();
        input.after_manifest.scope.target = "wasm32-unknown-unknown".to_string();
        let report = derive_verification_impact(&input);
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_FEATURE_SCOPE.to_string()));
    }

    #[test]
    fn dependency_edges_extend_requirement_selection() {
        // r[verify cairn.verification_impact.selection]
        let mut input = base_input();
        input.check_map.dependency_edges = vec![VerificationRequirementEdge {
            from_requirement_id: REQUIREMENT_ID.to_string(),
            to_requirement_id: RELATED_REQUIREMENT_ID.to_string(),
        }];
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert!(report
            .affected_requirements
            .contains(&RELATED_REQUIREMENT_ID.to_string()));
        assert!(report.selected_checks.contains(&RELATED_CHECK.to_string()));
    }

    #[test]
    fn dependency_cycles_block_instead_of_inventing_guidance() {
        // r[verify cairn.verification_impact.uncertainty]
        let mut input = base_input();
        input.check_map.dependency_edges = vec![
            VerificationRequirementEdge {
                from_requirement_id: REQUIREMENT_ID.to_string(),
                to_requirement_id: RELATED_REQUIREMENT_ID.to_string(),
            },
            VerificationRequirementEdge {
                from_requirement_id: RELATED_REQUIREMENT_ID.to_string(),
                to_requirement_id: REQUIREMENT_ID.to_string(),
            },
        ];
        let report = derive_verification_impact(&input);
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_GRAPH_CYCLE.to_string()));
    }

    #[test]
    fn macro_and_generated_sources_expand_to_default_work() {
        // r[verify cairn.verification_impact.uncertainty]
        let mut input = base_input();
        input.check_map.macro_opaque_paths = vec![SOURCE_PATH.to_string()];
        input.check_map.generated_source_paths = vec![SOURCE_PATH.to_string()];
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert!(report.selected_checks.contains(&DEFAULT_CHECK.to_string()));
        assert!(report
            .uncertainty
            .iter()
            .any(|entry| entry.code == UNCERTAINTY_MACRO_OPAQUE));
        assert!(report
            .uncertainty
            .iter()
            .any(|entry| entry.code == UNCERTAINTY_GENERATED_SOURCE));
    }

    #[test]
    fn reordered_inputs_preserve_semantic_identity() {
        // r[verify cairn.verification_impact.conformance]
        let mut first = base_input();
        let mut second = base_input();
        first.check_map.checks.reverse();
        second.check_map.requirement_checks.reverse();
        let first_report = derive_verification_impact(&first);
        let second_report = derive_verification_impact(&second);
        assert_eq!(first_report.selected_checks, second_report.selected_checks);
        assert_eq!(first_report.causal_paths, second_report.causal_paths);
        assert_eq!(first_report.uncertainty, second_report.uncertainty);
    }

    fn prior_record(
        evidence_id: &str,
        check_ids: &[&str],
        applicability: Applicability,
    ) -> VerificationPriorEvidence {
        let mut check_ids = check_ids
            .iter()
            .map(|id| (*id).to_string())
            .collect::<Vec<_>>();
        check_ids.sort();
        VerificationPriorEvidence {
            evidence_id: evidence_id.to_string(),
            check_ids,
            applicability,
        }
    }

    #[test]
    fn current_whole_subject_evidence_reports_applicable() {
        // r[verify cairn.verification_impact.applicability]
        let probe = derive_verification_impact(&base_input());
        assert!(probe.valid, "blockers: {:?}", probe.blockers);
        let current = probe
            .current_applicability
            .clone()
            .expect("valid reports carry the derived current applicability");
        let mut input = base_input();
        input.prior_evidence = vec![prior_record(
            "after-receipt",
            &[DEFAULT_CHECK, LINKED_CHECK],
            current,
        )];
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert_eq!(report.evidence_refresh.len(), 1);
        let refresh = &report.evidence_refresh[0];
        assert!(
            refresh.current,
            "stale reasons: {:?}",
            refresh.stale_reasons
        );
        assert!(refresh.stale_reasons.is_empty());
        assert_eq!(
            refresh.check_ids,
            vec![LINKED_CHECK.to_string(), DEFAULT_CHECK.to_string()]
        );
    }

    #[test]
    fn unlinked_edit_cannot_preserve_whole_subject_evidence() {
        // r[verify cairn.verification_impact.applicability]
        let before_trace_index = trace_index(vec![trace_file(UNLINKED_PATH, "fn before() {}\n")]);
        let after_trace_index = trace_index(vec![trace_file(UNLINKED_PATH, "fn after() {}\n")]);
        let current = derive_current_applicability_from(&after_trace_index, &hash('b'));
        let mut input = VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(&before_trace_index, &hash('a')),
            after_manifest: manifest(&after_trace_index, &hash('b')),
            asserted_changed_paths: vec![UNLINKED_PATH.to_string()],
            before_trace_index,
            after_trace_index,
            check_map: check_map(),
            prior_evidence: vec![prior_record(
                "before-receipt",
                &[DEFAULT_CHECK, LINKED_CHECK],
                current,
            )],
        };
        input.prior_evidence[0].applicability.subject_hash = hash('a');
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert!(report
            .uncertainty
            .iter()
            .any(|entry| entry.code == UNCERTAINTY_UNLINKED_SOURCE));
        assert_eq!(report.evidence_refresh.len(), 1);
        let refresh = &report.evidence_refresh[0];
        assert!(!refresh.current);
        assert_eq!(
            refresh.stale_reasons,
            vec![STALE_REASON_SUBJECT.to_string()]
        );
    }

    fn derive_current_applicability_from(
        index: &ContentBoundTraceIndex,
        identity: &str,
    ) -> Applicability {
        let input = VerificationImpactInput {
            schema_version: VERIFICATION_IMPACT_SCHEMA_V1.to_string(),
            before_manifest: manifest(index, identity),
            after_manifest: manifest(index, identity),
            asserted_changed_paths: Vec::new(),
            before_trace_index: index.clone(),
            after_trace_index: index.clone(),
            check_map: check_map(),
            prior_evidence: Vec::new(),
        };
        derive_current_applicability(&input)
    }

    #[test]
    fn policy_and_dependency_drift_report_exact_stale_reasons() {
        // r[verify cairn.verification_impact.applicability]
        let probe = derive_verification_impact(&base_input());
        let mut current = probe.current_applicability.clone().expect("current");
        current.policy_hash = hash('e');
        current.dependency_hashes = vec![hash('f')];
        let mut input = base_input();
        input.prior_evidence = vec![prior_record("drifted", &[DEFAULT_CHECK], current)];
        let report = derive_verification_impact(&input);
        assert!(report.valid, "blockers: {:?}", report.blockers);
        assert_eq!(
            report.evidence_refresh[0].stale_reasons,
            vec![
                STALE_REASON_DEPENDENCIES.to_string(),
                STALE_REASON_POLICY.to_string(),
            ]
        );
    }

    #[test]
    fn invalid_prior_evidence_applicability_blocks_admission() {
        // r[verify cairn.verification_impact.applicability]
        let mut drifted = Applicability {
            subject_hash: hash('b'),
            policy_hash: hash('e'),
            dependency_hashes: vec![hash('f'), hash('e')],
            source_projection_hash: hash('p'),
        };
        drifted.dependency_hashes.sort();
        drifted.dependency_hashes.reverse();
        let mut input = base_input();
        input.prior_evidence = vec![prior_record("unsorted", &[DEFAULT_CHECK], drifted)];
        let report = derive_verification_impact(&input);
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_PRIOR_EVIDENCE.to_string()));
    }

    #[test]
    fn prior_evidence_for_unknown_check_blocks_admission() {
        // r[verify cairn.verification_impact.applicability]
        let mut input = base_input();
        input.prior_evidence = vec![prior_record(
            "unknown-check",
            &["no-such-check"],
            Applicability {
                subject_hash: hash('b'),
                policy_hash: hash('e'),
                dependency_hashes: Vec::new(),
                source_projection_hash: hash('p'),
            },
        )];
        let report = derive_verification_impact(&input);
        assert!(!report.valid);
        assert!(codes(&report).contains(&ISSUE_PRIOR_EVIDENCE.to_string()));
    }

    #[test]
    fn reordered_prior_evidence_preserves_refresh_ordering() {
        // r[verify cairn.verification_impact.applicability]
        let probe = derive_verification_impact(&base_input());
        let current = probe.current_applicability.clone().expect("current");
        let mut stale = current.clone();
        stale.subject_hash = hash('a');
        let mut first = base_input();
        first.prior_evidence = vec![
            prior_record("aaa-stale", &[DEFAULT_CHECK], stale.clone()),
            prior_record("zzz-current", &[LINKED_CHECK], current.clone()),
        ];
        let mut second = base_input();
        second.prior_evidence = vec![
            prior_record("zzz-current", &[LINKED_CHECK], current),
            prior_record("aaa-stale", &[DEFAULT_CHECK], stale),
        ];
        let first_report = derive_verification_impact(&first);
        let second_report = derive_verification_impact(&second);
        assert_eq!(
            first_report.evidence_refresh,
            second_report.evidence_refresh
        );
        assert_eq!(first_report.evidence_refresh.len(), 2);
        assert!(first_report.evidence_refresh[0].current);
        assert!(!first_report.evidence_refresh[1].current);
    }
}

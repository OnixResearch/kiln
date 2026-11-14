// r[impl cairn.nominal_evidence_identities.receipt_chains]
// r[verify cairn.nominal_evidence_identities.receipt_chains.validation]

mod graph;
mod query;
mod types;

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::json;

use crate::verified::evidence::canonical_json_hash;
use crate::verified::nominal_identity::{
    ArtifactDigestRole, Blake3Digest, DigestRoleMarker, ReceiptDigestRole, StateDigestRole,
    WorkspaceId, RAW_BLAKE3_PROFILE,
};

pub use graph::evaluate_registry_graph;
pub use query::{query_registry_exact_id, query_registry_tags};
pub use types::*;

pub const GLOBAL_REGISTRY_SCHEMA: &str = "cairn.global-registry-snapshot.v1";
pub const EMPTY_REGISTRY_SNAPSHOT_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const PASS_VERDICT: &str = "PASS";
const FAIL_VERDICT: &str = "FAIL";
const ALLOWED_EDGE_STATUSES: [&str; 3] = ["declared", "blocked", "unresolved"];

// r[impl cairn.global_registry.snapshot]
// r[impl cairn.global_registry.snapshot.deterministic]
// r[impl cairn.global_registry.snapshot.conflict]
// r[impl cairn.global_registry.snapshot.rebuild]
// r[impl cairn.global_registry.fcis]
// r[impl cairn.global_registry.fcis.commit]
pub fn build_global_registry_snapshot(
    scope_id: &str,
    candidates: RegistryCandidateRecords,
    policy: &RegistryCorePolicy,
) -> RegistrySnapshotReport {
    let input_hash = canonical_json_hash(&json!({
        "scope_id": scope_id,
        "candidates": candidates,
        "policy": policy,
    }));
    let mut issues = Vec::new();
    if policy.required_non_claims.is_empty() || policy.allowed_vocabulary_owners.is_empty() {
        issues.push(issue(
            "registry.policy_incomplete",
            "policy",
            "global_registry_policy",
            "required_non_claims",
            Some("non-empty vocabulary owners and non-claims"),
            None,
            "registry core policy must preserve vocabulary and claim boundaries",
        ));
    }
    if WorkspaceId::parse(scope_id, "registry.scope_id").is_err() {
        issues.push(issue(
            "registry.scope_empty",
            "snapshot",
            "",
            "scope_id",
            Some("canonical workspace scope"),
            Some(scope_id),
            "registry scope must be an explicit workspace identity",
        ));
    }

    let sources = normalize_sources(candidates.sources, policy, &mut issues);
    let nodes = normalize_nodes(candidates.nodes, &sources, policy, &mut issues);
    let edges = normalize_edges(candidates.edges, &sources, &nodes, policy, &mut issues);
    let node_ids = nodes
        .iter()
        .map(|node| node.stable_id.as_str())
        .collect::<BTreeSet<_>>();
    for edge in &edges {
        if node_ids.contains(edge.stable_id.as_str()) {
            issues.push(issue(
                "registry.global_identity_conflict",
                "edge",
                &edge.stable_id,
                "stable_id",
                Some("globally unique node or edge ID"),
                Some(&edge.stable_id),
                "exact-ID registry identity is ambiguous across node and edge kinds",
            ));
        }
    }
    let tags = normalize_tags(
        candidates.tags,
        &sources,
        &nodes,
        &edges,
        policy,
        &mut issues,
    );
    let adapters = normalize_adapters(candidates.adapters, &mut issues);
    issues.sort_by(issue_order);
    issues.dedup();

    let snapshot = if issues.is_empty() {
        let mut snapshot = GlobalRegistrySnapshot {
            schema: GLOBAL_REGISTRY_SCHEMA.to_string(),
            scope_id: scope_id.to_string(),
            sources,
            nodes,
            tags,
            edges,
            adapters,
            non_claims: sorted_unique(policy.required_non_claims.clone()),
            snapshot_hash: String::new(),
        };
        snapshot.snapshot_hash = global_registry_snapshot_hash(&snapshot);
        Some(snapshot)
    } else {
        None
    };
    finalize_snapshot_report(snapshot, issues, input_hash, policy)
}

pub fn merge_global_registry_snapshot(
    observed: &GlobalRegistrySnapshot,
    candidates: RegistryCandidateRecords,
    policy: &RegistryCorePolicy,
) -> RegistrySnapshotReport {
    let mut merged = RegistryCandidateRecords {
        sources: observed.sources.clone(),
        nodes: observed.nodes.clone(),
        tags: observed.tags.clone(),
        edges: observed.edges.clone(),
        adapters: observed.adapters.clone(),
    };
    merged.sources.extend(candidates.sources);
    merged.nodes.extend(candidates.nodes);
    merged.tags.extend(candidates.tags);
    merged.edges.extend(candidates.edges);
    merged.adapters.extend(candidates.adapters);
    build_global_registry_snapshot(&observed.scope_id, merged, policy)
}

pub fn empty_global_registry_snapshot(
    scope_id: &str,
    policy: &RegistryCorePolicy,
) -> GlobalRegistrySnapshot {
    build_global_registry_snapshot(scope_id, RegistryCandidateRecords::default(), policy)
        .snapshot
        .expect("non-empty scope and valid policy produce an empty snapshot")
}

// r[impl cairn.global_registry.portability.verify]
// r[impl cairn.global_registry.portability.tamper]
pub fn verify_global_registry_snapshot(
    candidate: &GlobalRegistrySnapshot,
    policy: &RegistryCorePolicy,
) -> RegistrySnapshotReport {
    let stated_hash = candidate.snapshot_hash.clone();
    if candidate.schema != GLOBAL_REGISTRY_SCHEMA {
        return finalize_snapshot_report(
            None,
            vec![issue(
                "registry.schema_unsupported",
                "snapshot",
                &candidate.scope_id,
                "schema",
                Some(GLOBAL_REGISTRY_SCHEMA),
                Some(&candidate.schema),
                "portable registry schema is unsupported",
            )],
            canonical_json_hash(&json!(candidate)),
            policy,
        );
    }
    let report = build_global_registry_snapshot(
        &candidate.scope_id,
        RegistryCandidateRecords {
            sources: candidate.sources.clone(),
            nodes: candidate.nodes.clone(),
            tags: candidate.tags.clone(),
            edges: candidate.edges.clone(),
            adapters: candidate.adapters.clone(),
        },
        policy,
    );
    let Some(snapshot) = report.snapshot.as_ref() else {
        return report;
    };
    if candidate.non_claims != snapshot.non_claims {
        return finalize_snapshot_report(
            None,
            vec![issue(
                "registry.snapshot_non_claims_mismatch",
                "snapshot",
                &candidate.scope_id,
                "non_claims",
                Some(&snapshot.non_claims.join(" | ")),
                Some(&candidate.non_claims.join(" | ")),
                "portable snapshot claim boundary differs from policy-owned registry non-claims",
            )],
            report.input_hash,
            policy,
        );
    }
    if same_digest::<StateDigestRole>(&stated_hash, &snapshot.snapshot_hash) {
        report
    } else {
        finalize_snapshot_report(
            None,
            vec![issue(
                "registry.snapshot_hash_mismatch",
                "snapshot",
                &candidate.scope_id,
                "snapshot_hash",
                Some(&snapshot.snapshot_hash),
                Some(&stated_hash),
                "portable snapshot hash does not bind canonical registry content",
            )],
            report.input_hash,
            policy,
        )
    }
}

pub fn global_registry_snapshot_hash(snapshot: &GlobalRegistrySnapshot) -> String {
    let mut value = serde_json::to_value(snapshot).expect("registry snapshot serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("snapshot_hash");
    }
    canonical_json_hash(&value)
}

fn normalize_sources(
    values: Vec<RegistrySourceRecord>,
    policy: &RegistryCorePolicy,
    issues: &mut Vec<RegistryIssue>,
) -> Vec<RegistrySourceRecord> {
    let mut normalized = Vec::new();
    for mut value in values {
        validate_common(
            "source",
            &value.source_id,
            &value.schema,
            &value.content_hash,
            &value.source_ref,
            &value.non_claims,
            issues,
        );
        validate_vocabulary_owner(
            "source",
            &value.source_id,
            &value.vocabulary_owner,
            policy,
            issues,
        );
        value.non_claims = sorted_unique(value.non_claims);
        let expected = record_hash(&value, "record_hash");
        validate_stated_record_hash(
            "source",
            &value.source_id,
            &value.record_hash,
            &expected,
            issues,
        );
        value.record_hash = expected;
        normalized.push(value);
    }
    deduplicate_by_id(
        normalized,
        |row| &row.source_id,
        |row| &row.record_hash,
        "source",
        issues,
    )
}

fn normalize_nodes(
    values: Vec<RegistryNode>,
    sources: &[RegistrySourceRecord],
    policy: &RegistryCorePolicy,
    issues: &mut Vec<RegistryIssue>,
) -> Vec<RegistryNode> {
    let source_ids = sources
        .iter()
        .map(|row| row.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut normalized = Vec::new();
    for mut value in values {
        validate_common(
            "node",
            &value.stable_id,
            &value.schema,
            &value.content_hash,
            &value.payload_ref,
            &value.non_claims,
            issues,
        );
        validate_vocabulary_owner(
            "node",
            &value.stable_id,
            &value.vocabulary_owner,
            policy,
            issues,
        );
        require_reference(
            "node",
            &value.stable_id,
            "source_id",
            &value.source_id,
            &source_ids,
            issues,
        );
        if value.kind.is_empty() {
            issues.push(issue(
                "registry.node_kind_empty",
                "node",
                &value.stable_id,
                "kind",
                Some("non-empty kind"),
                Some(&value.kind),
                "registry node kind must be non-empty",
            ));
        }
        value.non_claims = sorted_unique(value.non_claims);
        let expected = record_hash(&value, "record_hash");
        validate_stated_record_hash(
            "node",
            &value.stable_id,
            &value.record_hash,
            &expected,
            issues,
        );
        value.record_hash = expected;
        normalized.push(value);
    }
    deduplicate_by_id(
        normalized,
        |row| &row.stable_id,
        |row| &row.record_hash,
        "node",
        issues,
    )
}

fn normalize_edges(
    values: Vec<RegistryEdge>,
    sources: &[RegistrySourceRecord],
    nodes: &[RegistryNode],
    policy: &RegistryCorePolicy,
    issues: &mut Vec<RegistryIssue>,
) -> Vec<RegistryEdge> {
    let source_ids = sources
        .iter()
        .map(|row| row.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let node_ids = nodes
        .iter()
        .map(|row| row.stable_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut normalized = Vec::new();
    for mut value in values {
        if value.stable_id.is_empty() || value.kind.is_empty() || value.non_claims.is_empty() {
            issues.push(issue(
                "registry.edge_malformed",
                "edge",
                &value.stable_id,
                "identity",
                Some("non-empty ID, kind, and non-claims"),
                None,
                "registry edge identity and claim boundary must be explicit",
            ));
        }
        validate_vocabulary_owner(
            "edge",
            &value.stable_id,
            &value.vocabulary_owner,
            policy,
            issues,
        );
        require_reference(
            "edge",
            &value.stable_id,
            "source_id",
            &value.source_id,
            &source_ids,
            issues,
        );
        if value.from_id == value.to_id {
            issues.push(issue(
                "registry.edge_self_reference",
                "edge",
                &value.stable_id,
                "to_id",
                Some("different endpoint"),
                Some(&value.to_id),
                "self-edges are not valid registry dependency relationships",
            ));
        }
        for (field, endpoint) in [("from_id", &value.from_id), ("to_id", &value.to_id)] {
            if !node_ids.contains(endpoint.as_str()) {
                issues.push(issue(
                    "registry.edge_unresolved_endpoint",
                    "edge",
                    &value.stable_id,
                    field,
                    Some("present node stable ID"),
                    Some(endpoint),
                    "unresolved graph endpoints remain explicit blockers",
                ));
            }
        }
        if !ALLOWED_EDGE_STATUSES.contains(&value.status.as_str()) {
            issues.push(issue(
                "registry.edge_status_unsupported",
                "edge",
                &value.stable_id,
                "status",
                Some("declared, blocked, or unresolved"),
                Some(&value.status),
                "registry edge status is unsupported",
            ));
        }
        value.non_claims = sorted_unique(value.non_claims);
        let expected = record_hash(&value, "record_hash");
        validate_stated_record_hash(
            "edge",
            &value.stable_id,
            &value.record_hash,
            &expected,
            issues,
        );
        value.record_hash = expected;
        normalized.push(value);
    }
    deduplicate_by_id(
        normalized,
        |row| &row.stable_id,
        |row| &row.record_hash,
        "edge",
        issues,
    )
}

fn normalize_tags(
    values: Vec<RegistryTagAssignment>,
    sources: &[RegistrySourceRecord],
    nodes: &[RegistryNode],
    edges: &[RegistryEdge],
    policy: &RegistryCorePolicy,
    issues: &mut Vec<RegistryIssue>,
) -> Vec<RegistryTagAssignment> {
    let source_by_id = sources
        .iter()
        .map(|row| (row.source_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let node_ids = nodes
        .iter()
        .map(|row| row.stable_id.as_str())
        .collect::<BTreeSet<_>>();
    let edge_ids = edges
        .iter()
        .map(|row| row.stable_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut normalized = Vec::new();
    for mut value in values {
        if value.assignment_id.is_empty()
            || value.namespace.is_empty()
            || value.key.is_empty()
            || value.value.is_empty()
            || value.non_claims.is_empty()
        {
            issues.push(issue(
                "registry.tag_malformed",
                "tag",
                &value.assignment_id,
                "identity",
                Some("non-empty typed assignment fields and non-claims"),
                None,
                "registry tag assignment is malformed",
            ));
        }
        let subject_present = match value.subject_kind.as_str() {
            "node" => node_ids.contains(value.subject_id.as_str()),
            "edge" => edge_ids.contains(value.subject_id.as_str()),
            _ => false,
        };
        if !subject_present {
            issues.push(issue(
                "registry.tag_subject_missing",
                "tag",
                &value.assignment_id,
                "subject_id",
                Some("present node or edge matching subject_kind"),
                Some(&value.subject_id),
                "tag subject is absent or has an unsupported subject kind",
            ));
        }
        let source = source_by_id.get(value.source_id.as_str()).copied();
        if source.is_none() {
            issues.push(issue(
                "registry.tag_source_missing",
                "tag",
                &value.assignment_id,
                "source_id",
                Some("present source record"),
                Some(&value.source_id),
                "tag source provenance is absent",
            ));
        }
        if policy.reserved_tag_namespaces.contains(&value.namespace)
            && !source.is_some_and(|row| row.family == "policy" && row.vocabulary_owner == "cairn")
        {
            issues.push(issue(
                "registry.tag_reserved_namespace",
                "tag",
                &value.assignment_id,
                "namespace",
                Some("Cairn policy-owned source"),
                Some(&value.namespace),
                "reserved namespace assignment lacks policy provenance",
            ));
        }
        value.non_claims = sorted_unique(value.non_claims);
        let expected = record_hash(&value, "record_hash");
        validate_stated_record_hash(
            "tag",
            &value.assignment_id,
            &value.record_hash,
            &expected,
            issues,
        );
        value.record_hash = expected;
        normalized.push(value);
    }
    deduplicate_by_id(
        normalized,
        |row| &row.assignment_id,
        |row| &row.record_hash,
        "tag",
        issues,
    )
}

fn normalize_adapters(
    values: Vec<RegistryAdapterMetadata>,
    issues: &mut Vec<RegistryIssue>,
) -> Vec<RegistryAdapterMetadata> {
    let mut normalized = values
        .into_iter()
        .map(|mut value| {
            value.capabilities = sorted_unique(value.capabilities);
            value.record_families = sorted_unique(value.record_families);
            if value.adapter_id.is_empty()
                || value.adapter_kind.is_empty()
                || value.schema.is_empty()
            {
                issues.push(issue(
                    "registry.adapter_malformed",
                    "adapter",
                    &value.adapter_id,
                    "identity",
                    Some("non-empty adapter ID, kind, and schema"),
                    None,
                    "registry adapter metadata is malformed",
                ));
            }
            value
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.adapter_id.cmp(&right.adapter_id));
    let mut result: Vec<RegistryAdapterMetadata> = Vec::new();
    for value in normalized {
        if let Some(previous) = result.last() {
            if previous == &value {
                continue;
            }
            if previous.adapter_id == value.adapter_id {
                issues.push(issue(
                    "registry.adapter_conflict",
                    "adapter",
                    &value.adapter_id,
                    "adapter_id",
                    None,
                    None,
                    "adapter ID resolves to conflicting metadata",
                ));
                continue;
            }
        }
        result.push(value);
    }
    result
}

fn validate_common(
    kind: &str,
    id: &str,
    schema: &str,
    content_hash: &str,
    reference: &str,
    non_claims: &[String],
    issues: &mut Vec<RegistryIssue>,
) {
    if id.is_empty() || schema.is_empty() {
        issues.push(issue(
            "registry.record_identity_empty",
            kind,
            id,
            "identity",
            Some("non-empty stable ID and schema"),
            None,
            "registry record identity must be explicit",
        ));
    }
    if !is_blake3_hex(content_hash) {
        issues.push(issue(
            "registry.content_hash_invalid",
            kind,
            id,
            "content_hash",
            Some("64-character BLAKE3 hex"),
            Some(content_hash),
            "registry content hash is malformed",
        ));
    }
    if !is_safe_reference(reference) {
        issues.push(issue(
            "registry.reference_unsafe",
            kind,
            id,
            "source_ref",
            Some("safe logical or repository-relative reference"),
            Some(reference),
            "registry reference exposes an unsafe path or payload",
        ));
    }
    if non_claims.is_empty() {
        issues.push(issue(
            "registry.non_claims_missing",
            kind,
            id,
            "non_claims",
            Some("at least one source-owned claim boundary"),
            None,
            "registry records cannot promote claims by omitting source boundaries",
        ));
    }
}

fn validate_vocabulary_owner(
    kind: &str,
    id: &str,
    owner: &str,
    policy: &RegistryCorePolicy,
    issues: &mut Vec<RegistryIssue>,
) {
    if owner.is_empty()
        || !policy
            .allowed_vocabulary_owners
            .contains(&owner.to_string())
    {
        issues.push(issue(
            "registry.vocabulary_owner_unsupported",
            kind,
            id,
            "vocabulary_owner",
            Some(&policy.allowed_vocabulary_owners.join(",")),
            Some(owner),
            "registry vocabulary ownership is absent or unsupported",
        ));
    }
}

fn require_reference(
    kind: &str,
    id: &str,
    field: &str,
    reference: &str,
    present: &BTreeSet<&str>,
    issues: &mut Vec<RegistryIssue>,
) {
    if !present.contains(reference) {
        issues.push(issue(
            "registry.source_missing",
            kind,
            id,
            field,
            Some("present source record"),
            Some(reference),
            "registry record source provenance is absent",
        ));
    }
}

fn validate_stated_record_hash(
    kind: &str,
    id: &str,
    stated: &str,
    expected: &str,
    issues: &mut Vec<RegistryIssue>,
) {
    if !stated.is_empty() && !same_digest::<ReceiptDigestRole>(stated, expected) {
        issues.push(issue(
            "registry.record_hash_mismatch",
            kind,
            id,
            "record_hash",
            Some(expected),
            Some(stated),
            "stated record hash does not bind normalized record content",
        ));
    }
}

fn deduplicate_by_id<T, I, H>(
    mut values: Vec<T>,
    id: I,
    hash: H,
    kind: &str,
    issues: &mut Vec<RegistryIssue>,
) -> Vec<T>
where
    T: Clone + PartialEq,
    I: Fn(&T) -> &String,
    H: Fn(&T) -> &String,
{
    values.sort_by(|left, right| id(left).cmp(id(right)));
    let mut result = Vec::new();
    for value in values {
        if let Some(previous) = result.last() {
            if id(previous) == id(&value) {
                if hash(previous) == hash(&value) && previous == &value {
                    continue;
                }
                issues.push(issue(
                    "registry.record_conflict",
                    kind,
                    id(&value),
                    "stable_id",
                    Some(hash(previous)),
                    Some(hash(&value)),
                    "stable registry ID resolves to conflicting canonical records",
                ));
                continue;
            }
        }
        result.push(value);
    }
    result
}

fn record_hash<T: Serialize>(value: &T, hash_field: &str) -> String {
    let mut json = serde_json::to_value(value).expect("registry record serializes");
    if let Some(object) = json.as_object_mut() {
        object.remove(hash_field);
    }
    canonical_json_hash(&json)
}

fn finalize_snapshot_report(
    snapshot: Option<GlobalRegistrySnapshot>,
    mut issues: Vec<RegistryIssue>,
    input_hash: String,
    policy: &RegistryCorePolicy,
) -> RegistrySnapshotReport {
    issues.sort_by(issue_order);
    let valid = issues.is_empty() && snapshot.is_some();
    let mut report = RegistrySnapshotReport {
        valid,
        verdict: if valid { PASS_VERDICT } else { FAIL_VERDICT }.to_string(),
        issues,
        snapshot,
        input_hash,
        receipt_hash: String::new(),
        non_claims: sorted_unique(policy.required_non_claims.clone()),
    };
    report.receipt_hash = report_hash(&report);
    report
}

pub(super) fn issue(
    code: &str,
    record_kind: &str,
    stable_id: &str,
    field: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    message: &str,
) -> RegistryIssue {
    RegistryIssue {
        code: code.to_string(),
        record_kind: record_kind.to_string(),
        stable_id: stable_id.to_string(),
        field: field.to_string(),
        expected: expected.map(ToOwned::to_owned),
        actual: actual.map(ToOwned::to_owned),
        message: message.to_string(),
    }
}

pub(super) fn issue_order(left: &RegistryIssue, right: &RegistryIssue) -> std::cmp::Ordering {
    (
        &left.code,
        &left.record_kind,
        &left.stable_id,
        &left.field,
        &left.expected,
        &left.actual,
    )
        .cmp(&(
            &right.code,
            &right.record_kind,
            &right.stable_id,
            &right.field,
            &right.expected,
            &right.actual,
        ))
}

pub(super) fn report_hash<T: Serialize>(report: &T) -> String {
    let mut value = serde_json::to_value(report).expect("registry report serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("receipt_hash");
    }
    canonical_json_hash(&value)
}

pub(super) fn registry_non_claims(snapshot: &GlobalRegistrySnapshot) -> Vec<String> {
    snapshot.non_claims.clone()
}

pub(super) fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

pub(super) fn is_blake3_hex(value: &str) -> bool {
    is_digest::<ArtifactDigestRole>(value)
}

fn is_digest<Role: DigestRoleMarker>(value: &str) -> bool {
    Blake3Digest::<Role>::parse_compatible(value, RAW_BLAKE3_PROFILE, "registry.digest").is_ok()
}

fn same_digest<Role: DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left =
        Blake3Digest::<Role>::parse_compatible(left, RAW_BLAKE3_PROFILE, "registry.left_digest");
    let right =
        Blake3Digest::<Role>::parse_compatible(right, RAW_BLAKE3_PROFILE, "registry.right_digest");
    matches!(
        (left, right),
        (Ok(left), Ok(right)) if left.as_hex() == right.as_hex()
    )
}

fn is_safe_reference(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.starts_with('~')
        && !value.split('/').any(|part| part == "..")
        && !value.contains('\n')
        && !value.contains('\r')
}

#[cfg(test)]
mod tests;

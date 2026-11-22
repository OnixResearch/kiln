pub const DOWNSTREAM_PILOT_SCHEMA_V1: &str = "octet.downstream-pilot.v1";

const DOWNSTREAM_PILOT_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "pilot_hash",
    "pilot_kind",
    "evidence_object_hash",
    "source_object_hash",
    "bounded_claim_class",
    "non_claims",
];
const CRUNCH_REQUIRED_FIELDS: &[&str] = &["checker_hash", "input_evidence_hash"];
const ASPEN_REQUIRED_FIELDS: &[&str] = &["config_hash", "source_hash", "executor_identity_hash"];
const CRUNCH_PILOT_KIND: &str = "crunch-mantle";
const ASPEN_PILOT_KIND: &str = "aspen-ci-job";
const CRUNCH_BOUNDED_CLAIM: &str = "checker-evidence-linkage";
const ASPEN_BOUNDED_CLAIM: &str = "ci-job-identity-linkage";
const DOWNSTREAM_PILOT_ISSUE_CAPACITY: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownstreamPilotIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownstreamPilotReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<DownstreamPilotIssue>,
    pub computed_pilot_hash: String,
}

// Active Cairn refs: content_addressed_rust_evidence.downstream_pilots,
// content_addressed_rust_evidence.downstream_pilots.crunch,
// content_addressed_rust_evidence.downstream_pilots.aspen.
// These become Tracey r[impl ...] refs only after the delta is synced into accepted specs.
pub fn validate_downstream_pilot_receipt(receipt: &serde_json::Value) -> DownstreamPilotReport {
    let computed_pilot_hash = downstream_pilot_hash(receipt);
    let issues = downstream_pilot_issues(receipt, &computed_pilot_hash);
    let is_valid = issues.is_empty();
    DownstreamPilotReport {
        valid: is_valid,
        verdict: if is_valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        computed_pilot_hash,
    }
}

pub fn downstream_pilot_hash(receipt: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&downstream_pilot_hash_material(receipt))
}

pub fn downstream_pilot_hash_material(receipt: &serde_json::Value) -> serde_json::Value {
    let mut material = receipt.clone();
    super::common::strip_non_hash_fields(&mut material);
    material
}

fn downstream_pilot_issues(
    receipt: &serde_json::Value,
    computed_pilot_hash: &str,
) -> Vec<DownstreamPilotIssue> {
    let Some(object) = receipt.as_object() else {
        return vec![pilot_issue(
            "downstream_pilot.not_object",
            "downstream pilot receipt must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(DOWNSTREAM_PILOT_ISSUE_CAPACITY);
    issues.extend(downstream_pilot_required_issues(object));
    issues.extend(downstream_pilot_hash_issues(object, computed_pilot_hash));
    issues.extend(downstream_pilot_kind_issues(object));
    issues.extend(downstream_pilot_non_claim_issues(object));
    issues.extend(downstream_pilot_entropy_issues(
        "$",
        &downstream_pilot_hash_material(receipt),
    ));
    issues
}

fn downstream_pilot_required_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<DownstreamPilotIssue> {
    let mut issues = Vec::with_capacity(DOWNSTREAM_PILOT_REQUIRED_FIELDS.len() + 1);
    for field in DOWNSTREAM_PILOT_REQUIRED_FIELDS {
        if !object.contains_key(*field) {
            issues.push(pilot_issue(
                "downstream_pilot.missing_field",
                format!("missing required field: {field}"),
            ));
        }
    }
    if object.get("schema").and_then(serde_json::Value::as_str) != Some(DOWNSTREAM_PILOT_SCHEMA_V1)
    {
        issues.push(pilot_issue(
            "downstream_pilot.unsupported_schema",
            "schema must be octet.downstream-pilot.v1",
        ));
    }
    issues
}

fn downstream_pilot_hash_issues(
    object: &serde_json::Map<String, serde_json::Value>,
    computed_pilot_hash: &str,
) -> Vec<DownstreamPilotIssue> {
    let mut issues = Vec::with_capacity(4);
    let stated_hash = object
        .get("pilot_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if stated_hash != computed_pilot_hash || !super::common::is_blake3_hex(stated_hash) {
        issues.push(pilot_issue(
            "downstream_pilot.hash_mismatch",
            "pilot_hash must equal the BLAKE3 hash of canonical downstream-pilot content with non-hash fields excluded",
        ));
    }
    for field in ["evidence_object_hash", "source_object_hash"] {
        if !object
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(super::common::is_blake3_hex)
        {
            issues.push(pilot_issue(
                "downstream_pilot.invalid_hash_field",
                format!("{field} must be a BLAKE3 hex string"),
            ));
        }
    }
    issues
}

fn downstream_pilot_kind_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<DownstreamPilotIssue> {
    let Some(kind) = object.get("pilot_kind").and_then(serde_json::Value::as_str) else {
        return vec![pilot_issue(
            "downstream_pilot.invalid_kind",
            "pilot_kind must be a known downstream pilot kind",
        )];
    };
    match kind {
        CRUNCH_PILOT_KIND => crunch_pilot_issues(object),
        ASPEN_PILOT_KIND => aspen_pilot_issues(object),
        _ => vec![pilot_issue(
            "downstream_pilot.invalid_kind",
            "pilot_kind must be crunch-mantle or aspen-ci-job",
        )],
    }
}

fn crunch_pilot_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<DownstreamPilotIssue> {
    let mut issues = Vec::with_capacity(CRUNCH_REQUIRED_FIELDS.len() + 1);
    issues.extend(pilot_required_hash_issues(object, CRUNCH_REQUIRED_FIELDS));
    if object
        .get("bounded_claim_class")
        .and_then(serde_json::Value::as_str)
        != Some(CRUNCH_BOUNDED_CLAIM)
    {
        issues.push(pilot_issue(
            "downstream_pilot.invalid_bounded_claim",
            "Crunch/Mantle pilots must use checker-evidence-linkage claim class",
        ));
    }
    issues
}

fn aspen_pilot_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<DownstreamPilotIssue> {
    let mut issues = Vec::with_capacity(ASPEN_REQUIRED_FIELDS.len() + 2);
    issues.extend(pilot_required_hash_issues(object, ASPEN_REQUIRED_FIELDS));
    if object
        .get("bounded_claim_class")
        .and_then(serde_json::Value::as_str)
        != Some(ASPEN_BOUNDED_CLAIM)
    {
        issues.push(pilot_issue(
            "downstream_pilot.invalid_bounded_claim",
            "Aspen CI pilots must use ci-job-identity-linkage claim class",
        ));
    }
    if object
        .get("replay_cache_key_material")
        .and_then(serde_json::Value::as_object)
        .is_none()
    {
        issues.push(pilot_issue(
            "downstream_pilot.missing_replay_cache_key_material",
            "Aspen CI pilots must record replay/cache key material as an object",
        ));
    }
    issues
}

fn pilot_required_hash_issues(
    object: &serde_json::Map<String, serde_json::Value>,
    fields: &[&str],
) -> Vec<DownstreamPilotIssue> {
    let mut issues = Vec::with_capacity(fields.len());
    for field in fields {
        if !object
            .get(*field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(super::common::is_blake3_hex)
        {
            issues.push(pilot_issue(
                "downstream_pilot.invalid_hash_field",
                format!("{field} must be a BLAKE3 hex string"),
            ));
        }
    }
    issues
}

fn downstream_pilot_non_claim_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<DownstreamPilotIssue> {
    let Some(non_claims) = object
        .get("non_claims")
        .and_then(serde_json::Value::as_object)
    else {
        return vec![pilot_issue(
            "downstream_pilot.missing_object",
            "non_claims must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(2);
    if non_claims
        .get("build_correctness")
        .and_then(serde_json::Value::as_str)
        != Some(super::common::NO_CORRECTNESS_CLAIM)
    {
        issues.push(pilot_issue(
            "downstream_pilot.build_correctness_overclaim",
            "pilot object identity must not claim downstream build correctness",
        ));
    }
    if non_claims
        .get("distributed_execution_correctness")
        .and_then(serde_json::Value::as_str)
        != Some(super::common::NO_CORRECTNESS_CLAIM)
    {
        issues.push(pilot_issue(
            "downstream_pilot.distributed_correctness_overclaim",
            "pilot object identity must not claim distributed execution correctness",
        ));
    }
    issues
}

fn downstream_pilot_entropy_issues(
    root_path: &str,
    root: &serde_json::Value,
) -> Vec<DownstreamPilotIssue> {
    super::common::entropy_hash_material_messages(root_path, root)
        .into_iter()
        .map(|message| pilot_issue("downstream_pilot.ambient_entropy_in_hash_material", message))
        .collect()
}

fn pilot_issue(code: impl Into<String>, message: impl Into<String>) -> DownstreamPilotIssue {
    DownstreamPilotIssue {
        code: code.into(),
        message: message.into(),
    }
}

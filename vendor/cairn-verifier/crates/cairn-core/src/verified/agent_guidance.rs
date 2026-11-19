//! Pure admission, selection, and identity construction for local agent guidance.
//!
//! The root facade preserves the public names. Remote derives target those
//! aliases so Serde retains wire names and both type and sequence-length errors.
//! `rename` alone does not preserve errors; `expecting` loses sequence lengths.

pub const AGENT_GUIDANCE_POLICY_SCHEMA_V1: &str = "cairn.agent-guidance-policy.v1";
pub const AGENT_GUIDANCE_SOURCE_PROJECT: &str = "project";
pub const AGENT_GUIDANCE_SOURCE_LIFECYCLE: &str = "lifecycle";
pub const AGENT_GUIDANCE_AUTHORITY_POLICY: &str = "policy_constraint";
pub const AGENT_GUIDANCE_AUTHORITY_FACT: &str = "project_fact";
pub const AGENT_GUIDANCE_AUTHORITY_ADVISORY: &str = "advisory_guidance";
pub const AGENT_GUIDANCE_OPERATION_CONTEXT: &str = "context";
pub const AGENT_GUIDANCE_OPERATION_WORKFLOW_INSTRUCTIONS: &str = "workflow-instructions";
pub const AGENT_GUIDANCE_ISSUE_DECLARATION: &str = "agent_guidance.declaration.invalid";
pub const AGENT_GUIDANCE_ISSUE_SELECTOR: &str = "agent_guidance.selector.unknown";
pub const AGENT_GUIDANCE_ISSUE_PATH: &str = "agent_guidance.source.path_invalid";
pub const AGENT_GUIDANCE_ISSUE_SOURCE: &str = "agent_guidance.source.invalid";
pub const AGENT_GUIDANCE_ISSUE_SOURCE_MISSING: &str = "agent_guidance.source.missing";
pub const AGENT_GUIDANCE_ISSUE_AUTHORITY: &str = "agent_guidance.authority.invalid";
pub const AGENT_GUIDANCE_ISSUE_OBSERVATION: &str = "agent_guidance.observation.invalid";
pub const AGENT_GUIDANCE_ISSUE_PORTABLE_RAW: &str = "agent_guidance.portable.raw_forbidden";
pub const AGENT_GUIDANCE_ISSUE_PORTABLE_INVALID: &str = "agent_guidance.portable.invalid";

const AGENT_GUIDANCE_REQUIRED_RECEIPT_FIELDS: &[&str] = &[
    "valid",
    "request",
    "bounds",
    "entries",
    "portable_entries",
    "issues",
    "local_result_hash",
    "portable_identity",
    "project_identity",
    "policy_hash",
    "non_claims",
];
const AUTHORITY_RANK_POLICY: u8 = 0;
const AUTHORITY_RANK_FACT: u8 = 1;
const AUTHORITY_RANK_ADVISORY: u8 = 2;
const POLICY_CONSTRAINT_PRIORITY: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidanceBounds",
    rename = "AgentGuidanceBounds",
    deny_unknown_fields
)]
pub struct Bounds {
    pub max_context_refs: usize,
    pub max_artifact_guidance: usize,
    pub max_operation_guidance: usize,
    pub max_selectors_per_entry: usize,
    pub max_id_bytes: usize,
    pub max_path_bytes: usize,
    pub max_inline_text_bytes: usize,
    pub max_source_text_bytes: usize,
    pub max_priority: u32,
    pub max_issues: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidancePolicy",
    rename = "AgentGuidancePolicy",
    deny_unknown_fields
)]
pub struct Policy {
    pub schema: String,
    pub allowed_source_roots: Vec<String>,
    pub allowed_authority_classes: Vec<String>,
    pub known_profiles: Vec<String>,
    pub known_artifacts: Vec<String>,
    pub known_operations: Vec<String>,
    pub bounds: Bounds,
    pub receipt_fields: Vec<String>,
    pub portable_excluded_fields: Vec<String>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(
    remote = "crate::AgentGuidanceDeclaration",
    rename = "AgentGuidanceDeclaration",
    deny_unknown_fields
)]
pub struct Declaration {
    #[serde(default)]
    pub context_refs: Vec<ContextReference>,
    #[serde(default)]
    pub artifact_guidance: Vec<ArtifactAdvice>,
    #[serde(default)]
    pub operation_guidance: Vec<OperationAdvice>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentContextReference",
    rename = "AgentContextReference",
    deny_unknown_fields
)]
pub struct ContextReference {
    pub id: String,
    pub source_root: String,
    pub path: String,
    pub required: bool,
    pub priority: u32,
    pub authority: String,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentArtifactGuidance",
    rename = "AgentArtifactGuidance",
    deny_unknown_fields
)]
pub struct ArtifactAdvice {
    pub id: String,
    pub text: String,
    pub priority: u32,
    pub authority: String,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentOperationGuidance",
    rename = "AgentOperationGuidance",
    deny_unknown_fields
)]
pub struct OperationAdvice {
    pub id: String,
    pub text: String,
    pub priority: u32,
    pub authority: String,
    #[serde(default)]
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(
    remote = "crate::AgentGuidanceRequest",
    rename = "AgentGuidanceRequest",
    deny_unknown_fields
)]
pub struct Request {
    pub profile: Option<String>,
    pub artifact: Option<String>,
    pub operation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidanceObservationStatus",
    rename = "AgentGuidanceObservationStatus",
    rename_all = "snake_case"
)]
pub enum ObservationStatus {
    Present,
    MissingOptional,
    MissingRequired,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidanceSourceObservation",
    rename = "AgentGuidanceSourceObservation",
    deny_unknown_fields
)]
pub struct SourceObservation {
    pub id: String,
    pub status: ObservationStatus,
    pub text: Option<String>,
    pub byte_length: Option<usize>,
    pub content_blake3: Option<String>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidanceIssue",
    rename = "AgentGuidanceIssue",
    deny_unknown_fields
)]
pub struct Issue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted {
    pub declaration: Declaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionPlan {
    pub request: Request,
    pub context_refs: Vec<ContextReference>,
    pub artifact_guidance: Vec<ArtifactAdvice>,
    pub operation_guidance: Vec<OperationAdvice>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidanceEntry",
    rename = "AgentGuidanceEntry",
    deny_unknown_fields
)]
pub struct Entry {
    pub id: String,
    pub authority: String,
    pub priority: u32,
    pub source_root: Option<String>,
    pub path: Option<String>,
    pub required: Option<bool>,
    pub profiles: Vec<String>,
    pub artifacts: Vec<String>,
    pub operations: Vec<String>,
    pub status: ObservationStatus,
    pub byte_length: Option<usize>,
    pub content_blake3: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::PortableAgentGuidanceEntry",
    rename = "PortableAgentGuidanceEntry",
    deny_unknown_fields
)]
pub struct PortableEntry {
    pub id: String,
    pub authority: String,
    pub priority: u32,
    pub source_root: Option<String>,
    pub path: Option<String>,
    pub required: Option<bool>,
    pub profiles: Vec<String>,
    pub artifacts: Vec<String>,
    pub operations: Vec<String>,
    pub status: ObservationStatus,
    pub byte_length: Option<usize>,
    pub content_blake3: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::AgentGuidanceResult",
    rename = "AgentGuidanceResult",
    deny_unknown_fields
)]
pub struct Evaluation {
    pub valid: bool,
    pub request: Request,
    pub bounds: Bounds,
    pub entries: Vec<Entry>,
    pub portable_entries: Vec<PortableEntry>,
    pub issues: Vec<Issue>,
    pub local_result_hash: String,
    pub portable_identity: String,
    pub project_identity: Option<String>,
    pub policy_hash: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    remote = "crate::PortableAgentGuidanceResult",
    rename = "PortableAgentGuidanceResult",
    deny_unknown_fields
)]
pub struct PortableEvaluation {
    pub request: Request,
    pub bounds: Bounds,
    pub entries: Vec<PortableEntry>,
    pub portable_identity: String,
    pub project_identity: Option<String>,
    pub policy_hash: String,
    pub non_claims: Vec<String>,
}

// Remote derives provide inherent methods. Delegate the ordinary traits to them
// without duplicating fields, changing admission, or rewriting error strings.
macro_rules! impl_wire_traits {
    ($($name:ident),+ $(,)?) => {$(
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                Self::serialize(self, serializer)
            }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::deserialize(deserializer)
            }
        }
    )+};
}

impl_wire_traits!(
    Bounds,
    Policy,
    Declaration,
    ContextReference,
    ArtifactAdvice,
    OperationAdvice,
    Request,
    ObservationStatus,
    SourceObservation,
    Issue,
    Entry,
    PortableEntry,
    Evaluation,
    PortableEvaluation,
);

pub fn validate_policy(policy: &Policy) -> Result<(), &'static str> {
    if policy.schema != AGENT_GUIDANCE_POLICY_SCHEMA_V1 {
        return Err("agent_guidance_policy.schema");
    }
    if policy.allowed_source_roots
        != [
            AGENT_GUIDANCE_SOURCE_PROJECT,
            AGENT_GUIDANCE_SOURCE_LIFECYCLE,
        ]
        || policy.allowed_authority_classes
            != [
                AGENT_GUIDANCE_AUTHORITY_FACT,
                AGENT_GUIDANCE_AUTHORITY_ADVISORY,
            ]
    {
        return Err("agent_guidance_policy.authority");
    }
    if !contains_required(
        &policy.known_operations,
        &[
            AGENT_GUIDANCE_OPERATION_CONTEXT,
            AGENT_GUIDANCE_OPERATION_WORKFLOW_INSTRUCTIONS,
        ],
    ) || !unique_nonempty(&policy.known_profiles)
        || !unique_nonempty(&policy.known_artifacts)
        || !unique_nonempty(&policy.known_operations)
        || !unique_nonempty(&policy.receipt_fields)
        || !unique_nonempty(&policy.portable_excluded_fields)
        || !unique_nonempty(&policy.required_non_claims)
    {
        return Err("agent_guidance_policy.known_values");
    }
    let bounds = &policy.bounds;
    if bounds.max_context_refs == 0
        || bounds.max_artifact_guidance == 0
        || bounds.max_operation_guidance == 0
        || bounds.max_selectors_per_entry == 0
        || bounds.max_id_bytes == 0
        || bounds.max_path_bytes == 0
        || bounds.max_inline_text_bytes == 0
        || bounds.max_source_text_bytes == 0
        || bounds.max_priority == 0
        || bounds.max_issues == 0
    {
        return Err("agent_guidance_policy.bounds");
    }
    if !contains_required(
        &policy.receipt_fields,
        AGENT_GUIDANCE_REQUIRED_RECEIPT_FIELDS,
    ) {
        return Err("agent_guidance_policy.receipt_fields");
    }
    if !contains_required(&policy.portable_excluded_fields, &["text", "absolute_path"]) {
        return Err("agent_guidance_policy.portable_excluded_fields");
    }
    Ok(())
}

// r[impl cairn.agent_guidance_context.admission]
// r[impl cairn.agent_guidance_context.functional_boundary]
pub fn admit_declaration(
    declaration: &Declaration,
    policy: &Policy,
) -> Result<Admitted, Vec<Issue>> {
    let mut issues = Vec::new();
    if let Err(field) = validate_policy(policy) {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            field,
            "agent guidance policy is invalid",
        ));
        return Err(issues);
    }
    if declaration.context_refs.len() > policy.bounds.max_context_refs {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            "context_refs",
            "context reference count exceeds the policy bound",
        ));
    }
    if declaration.artifact_guidance.len() > policy.bounds.max_artifact_guidance {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            "artifact_guidance",
            "artifact guidance count exceeds the policy bound",
        ));
    }
    if declaration.operation_guidance.len() > policy.bounds.max_operation_guidance {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            "operation_guidance",
            "operation guidance count exceeds the policy bound",
        ));
    }

    let mut normalized = declaration.clone();
    let mut ids = std::collections::BTreeSet::new();
    for (index, reference) in normalized.context_refs.iter_mut().enumerate() {
        let path = format!("context_refs[{index}]");
        validate_common_entry(
            CommonEntryAdmission {
                id: &reference.id,
                priority: reference.priority,
                authority: &reference.authority,
                expected_authority: AGENT_GUIDANCE_AUTHORITY_FACT,
                path: &path,
            },
            policy,
            &mut ids,
            &mut issues,
        );
        normalize_selectors(
            &mut reference.profiles,
            &policy.known_profiles,
            &format!("{path}.profiles"),
            policy,
            &mut issues,
        );
        normalize_selectors(
            &mut reference.artifacts,
            &policy.known_artifacts,
            &format!("{path}.artifacts"),
            policy,
            &mut issues,
        );
        normalize_selectors(
            &mut reference.operations,
            &policy.known_operations,
            &format!("{path}.operations"),
            policy,
            &mut issues,
        );
        if !policy.allowed_source_roots.contains(&reference.source_root) {
            issues.push(issue(
                AGENT_GUIDANCE_ISSUE_SOURCE,
                format!("{path}.source_root"),
                "context source root is not policy-approved",
            ));
        }
        if !safe_relative_context_path(&reference.path, policy.bounds.max_path_bytes) {
            issues.push(issue(
                AGENT_GUIDANCE_ISSUE_PATH,
                format!("{path}.path"),
                "context path must be a safe literal relative path",
            ));
        }
    }
    for (index, guidance) in normalized.artifact_guidance.iter_mut().enumerate() {
        let path = format!("artifact_guidance[{index}]");
        validate_common_entry(
            CommonEntryAdmission {
                id: &guidance.id,
                priority: guidance.priority,
                authority: &guidance.authority,
                expected_authority: AGENT_GUIDANCE_AUTHORITY_ADVISORY,
                path: &path,
            },
            policy,
            &mut ids,
            &mut issues,
        );
        validate_inline_text(&guidance.text, &path, policy, &mut issues);
        normalize_selectors(
            &mut guidance.profiles,
            &policy.known_profiles,
            &format!("{path}.profiles"),
            policy,
            &mut issues,
        );
        normalize_selectors(
            &mut guidance.artifacts,
            &policy.known_artifacts,
            &format!("{path}.artifacts"),
            policy,
            &mut issues,
        );
    }
    for (index, guidance) in normalized.operation_guidance.iter_mut().enumerate() {
        let path = format!("operation_guidance[{index}]");
        validate_common_entry(
            CommonEntryAdmission {
                id: &guidance.id,
                priority: guidance.priority,
                authority: &guidance.authority,
                expected_authority: AGENT_GUIDANCE_AUTHORITY_ADVISORY,
                path: &path,
            },
            policy,
            &mut ids,
            &mut issues,
        );
        validate_inline_text(&guidance.text, &path, policy, &mut issues);
        normalize_selectors(
            &mut guidance.operations,
            &policy.known_operations,
            &format!("{path}.operations"),
            policy,
            &mut issues,
        );
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    issues.truncate(policy.bounds.max_issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    normalized.context_refs.sort_by(context_ref_order);
    normalized.artifact_guidance.sort_by(artifact_order);
    normalized.operation_guidance.sort_by(operation_order);
    Ok(Admitted {
        declaration: normalized,
    })
}

// r[impl cairn.agent_guidance_context.selection]
// r[impl cairn.agent_guidance_context.authority]
pub fn select(
    admitted: &Admitted,
    request: &Request,
    policy: &Policy,
) -> Result<SelectionPlan, Vec<Issue>> {
    let mut issues = validate_request(request, policy);
    issues.truncate(policy.bounds.max_issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let context_refs = admitted
        .declaration
        .context_refs
        .iter()
        .filter(|entry| {
            selectors_match(&entry.profiles, request.profile.as_deref())
                && selectors_match(&entry.artifacts, request.artifact.as_deref())
                && selectors_match(&entry.operations, request.operation.as_deref())
        })
        .cloned()
        .collect();
    let artifact_guidance = admitted
        .declaration
        .artifact_guidance
        .iter()
        .filter(|entry| {
            selectors_match(&entry.profiles, request.profile.as_deref())
                && selectors_match(&entry.artifacts, request.artifact.as_deref())
        })
        .cloned()
        .collect();
    let operation_guidance = admitted
        .declaration
        .operation_guidance
        .iter()
        .filter(|entry| selectors_match(&entry.operations, request.operation.as_deref()))
        .cloned()
        .collect();
    Ok(SelectionPlan {
        request: request.clone(),
        context_refs,
        artifact_guidance,
        operation_guidance,
    })
}

// r[impl cairn.agent_guidance_context.receipts]
// r[impl cairn.agent_guidance_context.functional_boundary.pure]
pub fn build_result(
    plan: &SelectionPlan,
    observations: &[SourceObservation],
    policy_constraints: &[String],
    project_identity: Option<String>,
    policy_hash: String,
    policy: &Policy,
) -> Evaluation {
    let mut issues = Vec::new();
    let mut observed = std::collections::BTreeMap::new();
    for observation in observations {
        if observed
            .insert(observation.id.as_str(), observation)
            .is_some()
        {
            issues.push(issue(
                AGENT_GUIDANCE_ISSUE_OBSERVATION,
                format!("observations.{}", observation.id),
                "source observation ID is duplicated",
            ));
        }
    }
    let selected_ids = plan
        .context_refs
        .iter()
        .map(|reference| reference.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for observation in observations {
        if !selected_ids.contains(observation.id.as_str()) {
            issues.push(issue(
                AGENT_GUIDANCE_ISSUE_OBSERVATION,
                format!("observations.{}", observation.id),
                "source observation was not selected",
            ));
        }
    }

    let mut entries = policy_constraints
        .iter()
        .map(policy_constraint_entry)
        .collect::<Vec<_>>();
    for reference in &plan.context_refs {
        let Some(observation) = observed.get(reference.id.as_str()) else {
            issues.push(issue(
                AGENT_GUIDANCE_ISSUE_OBSERVATION,
                format!("context_refs.{}", reference.id),
                "selected context source has no observation",
            ));
            continue;
        };
        validate_observation(reference, observation, &mut issues);
        entries.push(context_entry(reference, observation));
    }
    entries.extend(plan.artifact_guidance.iter().map(artifact_entry));
    entries.extend(plan.operation_guidance.iter().map(operation_entry));
    entries.sort_by(entry_order);
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    issues.truncate(policy.bounds.max_issues);

    let portable_entries = entries.iter().map(portable_entry).collect::<Vec<_>>();
    let portable_identity = super::evidence::canonical_json_hash(&serde_json::json!({
        "request": plan.request,
        "bounds": policy.bounds,
        "entries": portable_entries,
        "project_identity": project_identity,
        "policy_hash": policy_hash,
        "non_claims": policy.required_non_claims,
    }));
    let local_result_hash = super::evidence::canonical_json_hash(&serde_json::json!({
        "request": plan.request,
        "bounds": policy.bounds,
        "entries": entries,
        "issues": issues,
        "project_identity": project_identity,
        "policy_hash": policy_hash,
        "non_claims": policy.required_non_claims,
    }));
    Evaluation {
        valid: issues.is_empty(),
        request: plan.request.clone(),
        bounds: policy.bounds.clone(),
        entries,
        portable_entries,
        issues,
        local_result_hash,
        portable_identity,
        project_identity,
        policy_hash,
        non_claims: policy.required_non_claims.clone(),
    }
}

pub fn portable_result(result: &Evaluation) -> PortableEvaluation {
    PortableEvaluation {
        request: result.request.clone(),
        bounds: result.bounds.clone(),
        entries: result.portable_entries.clone(),
        portable_identity: result.portable_identity.clone(),
        project_identity: result.project_identity.clone(),
        policy_hash: result.policy_hash.clone(),
        non_claims: result.non_claims.clone(),
    }
}

// r[impl cairn.agent_guidance_context.receipts]
// r[impl cairn.agent_guidance_context.boundary]
pub fn admit_portable_result(value: &serde_json::Value) -> Result<PortableEvaluation, Issue> {
    if contains_forbidden_portable_field(value) {
        return Err(issue(
            AGENT_GUIDANCE_ISSUE_PORTABLE_RAW,
            "portable_guidance",
            "portable guidance must not contain raw text or absolute paths",
        ));
    }
    serde_json::from_value(value.clone()).map_err(|error| {
        issue(
            AGENT_GUIDANCE_ISSUE_PORTABLE_INVALID,
            "portable_guidance",
            format!("portable guidance is malformed: {error}"),
        )
    })
}

pub fn safe_relative_context_path(path: &str, max_bytes: usize) -> bool {
    if path.is_empty()
        || path.len() > max_bytes
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains("//")
        || path.contains("://")
        || path.contains('\\')
        || path.contains(['*', '?', '[', ']', '$', ':', ';', '|', '&', '>', '<'])
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return false;
    }
    path.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/')
    })
}

struct CommonEntryAdmission<'a> {
    id: &'a str,
    priority: u32,
    authority: &'a str,
    expected_authority: &'a str,
    path: &'a str,
}

fn validate_common_entry(
    entry: CommonEntryAdmission<'_>,
    policy: &Policy,
    ids: &mut std::collections::BTreeSet<String>,
    issues: &mut Vec<Issue>,
) {
    if !valid_id(entry.id, policy.bounds.max_id_bytes) || !ids.insert(entry.id.to_string()) {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            format!("{}.id", entry.path),
            "guidance IDs must be safe, non-empty, and globally unique",
        ));
    }
    if entry.priority > policy.bounds.max_priority {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            format!("{}.priority", entry.path),
            "guidance priority exceeds the policy bound",
        ));
    }
    if entry.authority != entry.expected_authority
        || !policy
            .allowed_authority_classes
            .iter()
            .any(|candidate| candidate == entry.authority)
    {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_AUTHORITY,
            format!("{}.authority", entry.path),
            "project declarations cannot create or impersonate policy authority",
        ));
    }
}

fn normalize_selectors(
    selectors: &mut Vec<String>,
    known: &[String],
    path: &str,
    policy: &Policy,
    issues: &mut Vec<Issue>,
) {
    selectors.sort();
    selectors.dedup();
    if selectors.len() > policy.bounds.max_selectors_per_entry {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            path,
            "selector count exceeds the policy bound",
        ));
    }
    for selector in selectors {
        if !known.contains(selector) {
            issues.push(issue(
                AGENT_GUIDANCE_ISSUE_SELECTOR,
                path,
                format!("unknown selector: {selector}"),
            ));
        }
    }
}

fn validate_inline_text(text: &str, path: &str, policy: &Policy, issues: &mut Vec<Issue>) {
    if text.is_empty() || text.len() > policy.bounds.max_inline_text_bytes || text.contains('\0') {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_DECLARATION,
            format!("{path}.text"),
            "guidance text is empty or exceeds the policy bound",
        ));
    }
}

fn validate_request(request: &Request, policy: &Policy) -> Vec<Issue> {
    let mut issues = Vec::new();
    validate_requested_selector(
        request.profile.as_deref(),
        &policy.known_profiles,
        "request.profile",
        &mut issues,
    );
    validate_requested_selector(
        request.artifact.as_deref(),
        &policy.known_artifacts,
        "request.artifact",
        &mut issues,
    );
    validate_requested_selector(
        request.operation.as_deref(),
        &policy.known_operations,
        "request.operation",
        &mut issues,
    );
    issues
}

fn validate_requested_selector(
    selector: Option<&str>,
    known: &[String],
    path: &str,
    issues: &mut Vec<Issue>,
) {
    if selector.is_some_and(|value| !known.iter().any(|candidate| candidate == value)) {
        issues.push(issue(
            AGENT_GUIDANCE_ISSUE_SELECTOR,
            path,
            format!(
                "unknown requested selector: {}",
                selector.unwrap_or_default()
            ),
        ));
    }
}

fn validate_observation(
    reference: &ContextReference,
    observation: &SourceObservation,
    issues: &mut Vec<Issue>,
) {
    let path = format!("observations.{}", reference.id);
    match observation.status {
        ObservationStatus::Present => {
            let Some(text) = observation.text.as_deref() else {
                issues.push(issue(
                    AGENT_GUIDANCE_ISSUE_OBSERVATION,
                    path,
                    "present source observation must include text",
                ));
                return;
            };
            if observation.byte_length != Some(text.len())
                || observation.content_blake3.as_deref()
                    != Some(super::evidence::blake3_hex(text.as_bytes()).as_str())
            {
                issues.push(issue(
                    AGENT_GUIDANCE_ISSUE_OBSERVATION,
                    path,
                    "source observation length or BLAKE3 identity does not match text",
                ));
            }
        }
        ObservationStatus::MissingOptional if !reference.required => {}
        ObservationStatus::MissingRequired => issues.push(issue(
            AGENT_GUIDANCE_ISSUE_SOURCE_MISSING,
            path,
            "required context source is missing",
        )),
        ObservationStatus::MissingOptional => issues.push(issue(
            AGENT_GUIDANCE_ISSUE_SOURCE_MISSING,
            path,
            "required context source was reported as optional",
        )),
        ObservationStatus::Invalid => issues.push(issue(
            AGENT_GUIDANCE_ISSUE_SOURCE,
            path,
            observation
                .diagnostic
                .as_deref()
                .unwrap_or("context source observation is invalid"),
        )),
    }
}

fn policy_constraint_entry(text: &String) -> Entry {
    Entry {
        id: format!(
            "policy-constraint-{}",
            super::evidence::blake3_hex(text.as_bytes())
        ),
        authority: AGENT_GUIDANCE_AUTHORITY_POLICY.to_string(),
        priority: POLICY_CONSTRAINT_PRIORITY,
        source_root: None,
        path: None,
        required: None,
        profiles: Vec::new(),
        artifacts: Vec::new(),
        operations: Vec::new(),
        status: ObservationStatus::Present,
        byte_length: Some(text.len()),
        content_blake3: Some(super::evidence::blake3_hex(text.as_bytes())),
        text: Some(text.clone()),
    }
}

fn context_entry(reference: &ContextReference, observation: &SourceObservation) -> Entry {
    Entry {
        id: reference.id.clone(),
        authority: reference.authority.clone(),
        priority: reference.priority,
        source_root: Some(reference.source_root.clone()),
        path: Some(reference.path.clone()),
        required: Some(reference.required),
        profiles: reference.profiles.clone(),
        artifacts: reference.artifacts.clone(),
        operations: reference.operations.clone(),
        status: observation.status.clone(),
        byte_length: observation.byte_length,
        content_blake3: observation.content_blake3.clone(),
        text: observation.text.clone(),
    }
}

fn artifact_entry(guidance: &ArtifactAdvice) -> Entry {
    inline_entry(
        &guidance.id,
        &guidance.text,
        guidance.priority,
        guidance.profiles.clone(),
        guidance.artifacts.clone(),
        Vec::new(),
    )
}

fn operation_entry(guidance: &OperationAdvice) -> Entry {
    inline_entry(
        &guidance.id,
        &guidance.text,
        guidance.priority,
        Vec::new(),
        Vec::new(),
        guidance.operations.clone(),
    )
}

fn inline_entry(
    id: &str,
    text: &str,
    priority: u32,
    profiles: Vec<String>,
    artifacts: Vec<String>,
    operations: Vec<String>,
) -> Entry {
    Entry {
        id: id.to_string(),
        authority: AGENT_GUIDANCE_AUTHORITY_ADVISORY.to_string(),
        priority,
        source_root: None,
        path: None,
        required: None,
        profiles,
        artifacts,
        operations,
        status: ObservationStatus::Present,
        byte_length: Some(text.len()),
        content_blake3: Some(super::evidence::blake3_hex(text.as_bytes())),
        text: Some(text.to_string()),
    }
}

fn portable_entry(entry: &Entry) -> PortableEntry {
    PortableEntry {
        id: entry.id.clone(),
        authority: entry.authority.clone(),
        priority: entry.priority,
        source_root: entry.source_root.clone(),
        path: entry.path.clone(),
        required: entry.required,
        profiles: entry.profiles.clone(),
        artifacts: entry.artifacts.clone(),
        operations: entry.operations.clone(),
        status: entry.status.clone(),
        byte_length: entry.byte_length,
        content_blake3: entry.content_blake3.clone(),
    }
}

fn selectors_match(selectors: &[String], requested: Option<&str>) -> bool {
    selectors.is_empty()
        || requested.is_some_and(|value| selectors.iter().any(|selector| selector == value))
}

fn valid_id(id: &str, max_bytes: usize) -> bool {
    !id.is_empty()
        && id.len() <= max_bytes
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn context_ref_order(left: &ContextReference, right: &ContextReference) -> std::cmp::Ordering {
    left.priority
        .cmp(&right.priority)
        .then(left.source_root.cmp(&right.source_root))
        .then(left.path.cmp(&right.path))
        .then(left.id.cmp(&right.id))
}

fn artifact_order(left: &ArtifactAdvice, right: &ArtifactAdvice) -> std::cmp::Ordering {
    left.priority
        .cmp(&right.priority)
        .then(left.id.cmp(&right.id))
}

fn operation_order(left: &OperationAdvice, right: &OperationAdvice) -> std::cmp::Ordering {
    left.priority
        .cmp(&right.priority)
        .then(left.id.cmp(&right.id))
}

fn entry_order(left: &Entry, right: &Entry) -> std::cmp::Ordering {
    authority_rank(&left.authority)
        .cmp(&authority_rank(&right.authority))
        .then(left.priority.cmp(&right.priority))
        .then(left.source_root.cmp(&right.source_root))
        .then(left.path.cmp(&right.path))
        .then(left.id.cmp(&right.id))
}

fn authority_rank(authority: &str) -> u8 {
    match authority {
        AGENT_GUIDANCE_AUTHORITY_POLICY => AUTHORITY_RANK_POLICY,
        AGENT_GUIDANCE_AUTHORITY_FACT => AUTHORITY_RANK_FACT,
        AGENT_GUIDANCE_AUTHORITY_ADVISORY => AUTHORITY_RANK_ADVISORY,
        _ => u8::MAX,
    }
}

fn contains_forbidden_portable_field(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(key.as_str(), "text" | "absolute_path")
                || contains_forbidden_portable_field(child)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_forbidden_portable_field),
        _ => false,
    }
}

fn unique_nonempty(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| !value.is_empty())
        && values
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == values.len()
}

fn contains_required(values: &[String], required: &[&str]) -> bool {
    required
        .iter()
        .all(|required| values.iter().any(|value| value == required))
}

fn issue(
    code: impl Into<String>,
    field_path: impl Into<String>,
    message: impl Into<String>,
) -> Issue {
    Issue {
        code: code.into(),
        field_path: field_path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_CONTEXT_REFS: usize = 8;
    const MAX_GUIDANCE: usize = 8;
    const MAX_SELECTORS: usize = 8;
    const MAX_ID_BYTES: usize = 64;
    const MAX_PATH_BYTES: usize = 256;
    const MAX_TEXT_BYTES: usize = 4096;
    const MAX_PRIORITY: u32 = 100;
    const MAX_ISSUES: usize = 32;
    const FACT_PRIORITY: u32 = 1;
    const ADVICE_PRIORITY: u32 = 2;
    const BLAKE3_HEX_LENGTH: usize = 64;
    const ADVICE_ENTRY_INDEX: usize = 2;

    fn policy() -> Policy {
        Policy {
            schema: AGENT_GUIDANCE_POLICY_SCHEMA_V1.to_string(),
            allowed_source_roots: vec![
                AGENT_GUIDANCE_SOURCE_PROJECT.to_string(),
                AGENT_GUIDANCE_SOURCE_LIFECYCLE.to_string(),
            ],
            allowed_authority_classes: vec![
                AGENT_GUIDANCE_AUTHORITY_FACT.to_string(),
                AGENT_GUIDANCE_AUTHORITY_ADVISORY.to_string(),
            ],
            known_profiles: vec!["spec-driven".to_string()],
            known_artifacts: vec!["proposal".to_string()],
            known_operations: vec![
                AGENT_GUIDANCE_OPERATION_CONTEXT.to_string(),
                AGENT_GUIDANCE_OPERATION_WORKFLOW_INSTRUCTIONS.to_string(),
            ],
            bounds: Bounds {
                max_context_refs: MAX_CONTEXT_REFS,
                max_artifact_guidance: MAX_GUIDANCE,
                max_operation_guidance: MAX_GUIDANCE,
                max_selectors_per_entry: MAX_SELECTORS,
                max_id_bytes: MAX_ID_BYTES,
                max_path_bytes: MAX_PATH_BYTES,
                max_inline_text_bytes: MAX_TEXT_BYTES,
                max_source_text_bytes: MAX_TEXT_BYTES,
                max_priority: MAX_PRIORITY,
                max_issues: MAX_ISSUES,
            },
            receipt_fields: AGENT_GUIDANCE_REQUIRED_RECEIPT_FIELDS
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
            portable_excluded_fields: vec!["text".to_string(), "absolute_path".to_string()],
            required_non_claims: vec!["guidance is not policy".to_string()],
        }
    }

    fn declaration() -> Declaration {
        Declaration {
            context_refs: vec![ContextReference {
                id: "project-facts".to_string(),
                source_root: AGENT_GUIDANCE_SOURCE_PROJECT.to_string(),
                path: "docs/facts.md".to_string(),
                required: true,
                priority: FACT_PRIORITY,
                authority: AGENT_GUIDANCE_AUTHORITY_FACT.to_string(),
                profiles: vec!["spec-driven".to_string(), "spec-driven".to_string()],
                artifacts: vec!["proposal".to_string()],
                operations: Vec::new(),
            }],
            artifact_guidance: vec![ArtifactAdvice {
                id: "proposal-style".to_string(),
                text: "Keep the proposal bounded.".to_string(),
                priority: ADVICE_PRIORITY,
                authority: AGENT_GUIDANCE_AUTHORITY_ADVISORY.to_string(),
                profiles: vec!["spec-driven".to_string()],
                artifacts: vec!["proposal".to_string()],
            }],
            operation_guidance: Vec::new(),
        }
    }

    #[test]
    fn selects_and_orders_policy_facts_and_advice() {
        // r[verify cairn.agent_guidance_context.admission.valid]
        // r[verify cairn.agent_guidance_context.selection.artifact]
        // r[verify cairn.agent_guidance_context.receipts.portable]
        // r[verify cairn.agent_guidance_context.functional_boundary.pure]
        let policy = policy();
        let admitted = admit_declaration(&declaration(), &policy).expect("admitted");
        assert_eq!(admitted.declaration.context_refs[0].profiles.len(), 1);
        let request = Request {
            profile: Some("spec-driven".to_string()),
            artifact: Some("proposal".to_string()),
            operation: Some(AGENT_GUIDANCE_OPERATION_WORKFLOW_INSTRUCTIONS.to_string()),
        };
        let plan = select(&admitted, &request, &policy).expect("selected");
        let text = "Declared project fact.";
        let result = build_result(
            &plan,
            &[SourceObservation {
                id: "project-facts".to_string(),
                status: ObservationStatus::Present,
                text: Some(text.to_string()),
                byte_length: Some(text.len()),
                content_blake3: Some(super::super::evidence::blake3_hex(text.as_bytes())),
                diagnostic: None,
            }],
            &["Policy constraint.".to_string()],
            Some("project-hash".to_string()),
            "policy-hash".to_string(),
            &policy,
        );
        assert!(result.valid);
        assert_eq!(result.entries[0].authority, AGENT_GUIDANCE_AUTHORITY_POLICY);
        assert_eq!(result.entries[1].authority, AGENT_GUIDANCE_AUTHORITY_FACT);
        assert_eq!(
            result.entries[ADVICE_ENTRY_INDEX].authority,
            AGENT_GUIDANCE_AUTHORITY_ADVISORY
        );
        let portable = serde_json::to_string(&result.portable_entries).expect("portable JSON");
        assert!(!portable.contains(text));
    }

    #[test]
    fn rejects_unsafe_unknown_duplicate_and_policy_authority_inputs() {
        // r[verify cairn.agent_guidance_context.admission.invalid]
        // r[verify cairn.agent_guidance_context.sources.invalid]
        // r[verify cairn.agent_guidance_context.authority.reject]
        // r[verify cairn.agent_guidance_context.boundary.overclaim]
        let policy = policy();
        let mut input = declaration();
        input.context_refs[0].path = "../secret".to_string();
        input.context_refs[0].profiles = vec!["unknown".to_string()];
        input.context_refs[0].authority = AGENT_GUIDANCE_AUTHORITY_POLICY.to_string();
        input.artifact_guidance[0].id = input.context_refs[0].id.clone();
        let issues = admit_declaration(&input, &policy).expect_err("rejected");
        let codes = issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(codes.contains(AGENT_GUIDANCE_ISSUE_PATH));
        assert!(codes.contains(AGENT_GUIDANCE_ISSUE_SELECTOR));
        assert!(codes.contains(AGENT_GUIDANCE_ISSUE_AUTHORITY));
        assert!(codes.contains(AGENT_GUIDANCE_ISSUE_DECLARATION));
    }

    #[test]
    fn unknown_request_and_stale_observation_fail_closed() {
        // r[verify cairn.agent_guidance_context.selection.unknown]
        // r[verify cairn.agent_guidance_context.sources.invalid]
        let policy = policy();
        let admitted = admit_declaration(&declaration(), &policy).expect("admitted");
        let unknown = select(
            &admitted,
            &Request {
                profile: Some("unknown".to_string()),
                artifact: None,
                operation: None,
            },
            &policy,
        )
        .expect_err("unknown selector");
        assert_eq!(unknown[0].code, AGENT_GUIDANCE_ISSUE_SELECTOR);

        let request = Request {
            profile: Some("spec-driven".to_string()),
            artifact: Some("proposal".to_string()),
            operation: None,
        };
        let plan = select(&admitted, &request, &policy).expect("selected");
        let result = build_result(
            &plan,
            &[SourceObservation {
                id: "project-facts".to_string(),
                status: ObservationStatus::Present,
                text: Some("changed".to_string()),
                byte_length: Some(1),
                content_blake3: Some("0".repeat(BLAKE3_HEX_LENGTH)),
                diagnostic: None,
            }],
            &[],
            None,
            "policy".to_string(),
            &policy,
        );
        assert!(!result.valid);
        assert_eq!(result.issues[0].code, AGENT_GUIDANCE_ISSUE_OBSERVATION);
    }

    #[test]
    fn optional_missing_source_remains_visible_without_invalidating_result() {
        // r[verify cairn.agent_guidance_context.sources.optional]
        let policy = policy();
        let mut declaration = declaration();
        declaration.context_refs[0].required = false;
        let admitted = admit_declaration(&declaration, &policy).expect("admitted optional");
        let plan = select(
            &admitted,
            &Request {
                profile: Some("spec-driven".to_string()),
                artifact: Some("proposal".to_string()),
                operation: None,
            },
            &policy,
        )
        .expect("selected optional");
        let result = build_result(
            &plan,
            &[SourceObservation {
                id: "project-facts".to_string(),
                status: ObservationStatus::MissingOptional,
                text: None,
                byte_length: None,
                content_blake3: None,
                diagnostic: None,
            }],
            &[],
            None,
            "policy".to_string(),
            &policy,
        );
        assert!(result.valid);
        assert_eq!(result.entries[0].status, ObservationStatus::MissingOptional);
    }

    #[test]
    fn portable_admission_rejects_raw_text_and_accepts_identity_only_values() {
        // r[verify cairn.agent_guidance_context.receipts.reject_raw]
        let policy = policy();
        let portable = serde_json::json!({
            "request": Request::default(),
            "bounds": policy.bounds,
            "entries": [],
            "portable_identity": "portable-identity",
            "project_identity": null,
            "policy_hash": "policy-hash",
            "non_claims": policy.required_non_claims,
        });
        assert!(admit_portable_result(&portable).is_ok());
        let mut raw = portable;
        raw["text"] = serde_json::Value::String("secret-like text".to_string());
        let issue = admit_portable_result(&raw).expect_err("raw text rejected");
        assert_eq!(issue.code, AGENT_GUIDANCE_ISSUE_PORTABLE_RAW);
    }
}

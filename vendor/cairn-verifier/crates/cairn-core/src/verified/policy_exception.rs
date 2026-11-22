pub const POLICY_EXCEPTION_SCHEMA_V1: &str = "cairn.policy-exception.v1";
pub const POLICY_EXCEPTION_COB_TYPE_NAME: &str = "xyz.onixresearch.policy-exception";
pub const POLICY_EXCEPTION_PORTABLE_SCHEMA_V1: &str = "cairn.policy-exception-portable.v1";
const BLAKE3_HEX_LENGTH: usize = 64;

// r[impl cairn.policy_exception_cob.scope]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyScope {
    pub rule_ids: Vec<String>,
    pub targets: Vec<String>,
}

// r[impl cairn.policy_exception_cob.approval]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExceptionRequest {
    pub policy_identity: String,
    pub scope: PolicyScope,
    pub rationale: String,
    pub compensating_controls: Vec<String>,
    pub request_root: String,
}

// r[impl cairn.policy_exception_cob.validity]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidityObservation {
    pub id: String,
    pub observed_epoch: u64,
    pub finite_valid_until: u64,
    pub source: String,
}

// r[impl cairn.policy_exception_cob.approval]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExceptionFinding {
    pub id: String,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyExceptionState {
    pub schema: String,
    pub kind: String,
    pub exception_id: String,
    pub workspace_id: String,
    pub request: ExceptionRequest,
    pub request_revision: u64,
    pub requesters: std::collections::BTreeSet<String>,
    pub reviewers: std::collections::BTreeSet<String>,
    pub approvers: std::collections::BTreeSet<String>,
    pub revokers: std::collections::BTreeSet<String>,
    pub observers: std::collections::BTreeSet<String>,
    pub findings: std::collections::BTreeMap<String, ExceptionFinding>,
    pub approvals: Vec<String>,
    pub observations: std::collections::BTreeMap<String, ValidityObservation>,
    pub lifecycle: String,
    pub predecessor: Option<String>,
    pub operations: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyExceptionInit {
    pub schema: String,
    pub kind: String,
    pub exception_id: String,
    pub workspace_id: String,
    pub request: ExceptionRequest,
    pub requesters: std::collections::BTreeSet<String>,
    pub reviewers: std::collections::BTreeSet<String>,
    pub approvers: std::collections::BTreeSet<String>,
    pub revokers: std::collections::BTreeSet<String>,
    pub observers: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyExceptionAction {
    Init { state: Box<PolicyExceptionInit> },
    Revise { request: ExceptionRequest },
    AddFinding { finding: ExceptionFinding },
    ResolveFinding { finding_id: String },
    Approve { scope_root: String },
    Reject { reason: String },
    ObserveValidity { observation: ValidityObservation },
    Revoke { reason: String },
    Expire { reason: String },
    Close { reason: String },
    Supersede { successor: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyExceptionOperation {
    pub id: String,
    pub actor: String,
    pub role: Option<ExceptionRole>,
    pub actions: Vec<PolicyExceptionAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionRole {
    Requester,
    Reviewer,
    Approver,
    Revoker,
    Observer,
}

impl ExceptionRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requester => "requester",
            Self::Reviewer => "reviewer",
            Self::Approver => "approver",
            Self::Revoker => "revoker",
            Self::Observer => "observer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyExceptionError {
    InvalidField {
        field: String,
        reason: String,
    },
    UnsupportedKind {
        kind: String,
    },
    SchemaMismatch {
        expected: String,
        actual: String,
    },
    KindMismatch {
        expected: String,
        actual: String,
    },
    EmptyOperation,
    DuplicateOperation {
        id: String,
    },
    UnauthorizedRole {
        required: &'static str,
        actor: String,
    },
    UnknownRule {
        rule: String,
    },
    WildcardScope {
        selector: String,
    },
    UnsafeSelector {
        selector: String,
    },
    PolicyMismatch {
        expected: String,
        actual: String,
    },
    StaleRequest {
        expected: u64,
        observed: u64,
    },
    StaleObservation {
        source: String,
    },
    ConflictingDecision,
    MissingObservation {
        source: String,
    },
    ExpiredBeforeRenewal,
    InvalidRootShape,
    MissingRelatedBase {
        base: String,
    },
    PortableTamper {
        field: String,
    },
    Factory(String),
}

impl std::fmt::Display for PolicyExceptionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidField { field, reason } => write!(
                formatter,
                "invalid policy-exception field {field}: {reason}"
            ),
            Self::UnsupportedKind { kind } => {
                write!(formatter, "unsupported policy-exception kind {kind}")
            }
            Self::SchemaMismatch { expected, actual } => write!(
                formatter,
                "schema mismatch; expected {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "kind mismatch; expected {expected}, got {actual}"
            ),
            Self::EmptyOperation => write!(formatter, "operation has no actions"),
            Self::DuplicateOperation { id } => write!(formatter, "duplicate operation {id}"),
            Self::UnauthorizedRole { required, actor } => {
                write!(formatter, "actor {actor} is not a required {required}")
            }
            Self::UnknownRule { rule } => write!(formatter, "unknown policy rule {rule}"),
            Self::WildcardScope { selector } => write!(
                formatter,
                "implicit wildcard scope selector {selector} is not allowed"
            ),
            Self::UnsafeSelector { selector } => {
                write!(formatter, "unsafe scope selector {selector}")
            }
            Self::PolicyMismatch { expected, actual } => write!(
                formatter,
                "policy identity mismatch; expected {expected}, got {actual}"
            ),
            Self::StaleRequest { expected, observed } => write!(
                formatter,
                "stale request revision; expected {expected}, observed {observed}"
            ),
            Self::StaleObservation { source } => {
                write!(formatter, "stale validity observation from {source}")
            }
            Self::ConflictingDecision => {
                write!(formatter, "conflicting decisions block applicability")
            }
            Self::MissingObservation { source } => {
                write!(formatter, "missing validity observation from {source}")
            }
            Self::ExpiredBeforeRenewal => write!(
                formatter,
                "expiration cannot be reversed by an older observation"
            ),
            Self::InvalidRootShape => write!(formatter, "root operation must be one Init"),
            Self::MissingRelatedBase { base } => write!(
                formatter,
                "root base object {base} is missing from related objects"
            ),
            Self::PortableTamper { field } => {
                write!(formatter, "portable envelope field {field} is tampered")
            }
            Self::Factory(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for PolicyExceptionError {}

// r[impl cairn.policy_exception_cob.scope]
const KNOWN_RULES: &[&str] = &["allow-network-egress", "allow-fs-write", "allow-privileged"];

pub fn validate_scope(scope: &PolicyScope) -> Result<(), PolicyExceptionError> {
    for rule in &scope.rule_ids {
        if !KNOWN_RULES.contains(&rule.as_str()) {
            return Err(PolicyExceptionError::UnknownRule { rule: rule.clone() });
        }
    }
    for target in &scope.targets {
        if target == "*" || target == "**" {
            return Err(PolicyExceptionError::WildcardScope {
                selector: target.clone(),
            });
        }
        if !target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':'))
        {
            return Err(PolicyExceptionError::UnsafeSelector {
                selector: target.clone(),
            });
        }
    }
    Ok(())
}

pub fn exception_request_root(request: &ExceptionRequest) -> Result<String, PolicyExceptionError> {
    Ok(super::evidence::canonical_json_hash(&serde_json::json!({
        "policy": request.policy_identity,
        "scope": request.scope,
        "rationale": request.rationale,
        "controls": request.compensating_controls,
    })))
}

// r[impl cairn.policy_exception_cob.profile]
// r[impl cairn.policy_exception_cob.profile.root]
pub fn initialize_exception(
    operation: PolicyExceptionOperation,
) -> Result<PolicyExceptionState, PolicyExceptionError> {
    if operation.actions.is_empty() {
        return Err(PolicyExceptionError::EmptyOperation);
    }
    let [PolicyExceptionAction::Init { state }] = operation.actions.as_slice() else {
        return Err(PolicyExceptionError::InvalidRootShape);
    };
    validate_exception_init(state)?;
    let root = exception_request_root(&state.request)?;
    if root != state.request.request_root {
        return Err(PolicyExceptionError::PolicyMismatch {
            expected: state.request.request_root.clone(),
            actual: root,
        });
    }
    Ok(PolicyExceptionState {
        schema: POLICY_EXCEPTION_SCHEMA_V1.to_string(),
        kind: "policy-exception".to_string(),
        exception_id: state.exception_id.clone(),
        workspace_id: state.workspace_id.clone(),
        request: state.request.clone(),
        request_revision: 0,
        requesters: state.requesters.clone(),
        reviewers: state.reviewers.clone(),
        approvers: state.approvers.clone(),
        revokers: state.revokers.clone(),
        observers: state.observers.clone(),
        findings: std::collections::BTreeMap::new(),
        approvals: Vec::new(),
        observations: std::collections::BTreeMap::new(),
        lifecycle: "requested".to_string(),
        predecessor: None,
        operations: std::collections::BTreeSet::from([operation.id]),
    })
}

pub fn validate_exception_init(init: &PolicyExceptionInit) -> Result<(), PolicyExceptionError> {
    if init.schema != POLICY_EXCEPTION_SCHEMA_V1 {
        return Err(PolicyExceptionError::SchemaMismatch {
            expected: POLICY_EXCEPTION_SCHEMA_V1.to_string(),
            actual: init.schema.clone(),
        });
    }
    if init.kind != "policy-exception" {
        return Err(PolicyExceptionError::KindMismatch {
            expected: "policy-exception".to_string(),
            actual: init.kind.clone(),
        });
    }
    if init.exception_id.is_empty()
        || init.request.policy_identity.len() != BLAKE3_HEX_LENGTH
        || init.request.rationale.is_empty()
    {
        return Err(PolicyExceptionError::InvalidField {
            field: "identity".to_string(),
            reason: "exception id, policy identity, and rationale must be present".to_string(),
        });
    }
    validate_scope(&init.request.scope)?;
    if init.requesters.is_empty()
        || init.reviewers.is_empty()
        || init.approvers.is_empty()
        || init.revokers.is_empty()
        || init.observers.is_empty()
    {
        return Err(PolicyExceptionError::InvalidField {
            field: "roles".to_string(),
            reason: "each policy-exception role set must be non-empty".to_string(),
        });
    }
    Ok(())
}

// r[impl cairn.policy_exception_cob.authority]
// r[impl cairn.policy_exception_cob.approval]
// r[impl cairn.policy_exception_cob.validity]
// r[impl cairn.policy_exception_cob.applicability]
pub fn apply_exception_operation(
    state: &mut PolicyExceptionState,
    operation: PolicyExceptionOperation,
) -> Result<(), PolicyExceptionError> {
    if operation.actions.is_empty() {
        return Err(PolicyExceptionError::EmptyOperation);
    }
    if !state.operations.insert(operation.id.clone()) {
        return Err(PolicyExceptionError::DuplicateOperation { id: operation.id });
    }
    let mut snapshot = state.clone();
    for action in &operation.actions {
        apply_exception_action(&mut snapshot, &operation, action)?;
    }
    *state = snapshot;
    Ok(())
}

fn apply_exception_action(
    state: &mut PolicyExceptionState,
    operation: &PolicyExceptionOperation,
    action: &PolicyExceptionAction,
) -> Result<(), PolicyExceptionError> {
    let required = required_role(action);
    if let Some(required) = required {
        if !exception_actor_has_role(state, &operation.actor, required.clone()) {
            return Err(PolicyExceptionError::UnauthorizedRole {
                required: required.as_str(),
                actor: operation.actor.clone(),
            });
        }
    }
    match action {
        PolicyExceptionAction::Init { .. } => Err(PolicyExceptionError::InvalidRootShape),
        PolicyExceptionAction::Revise { request } => {
            validate_scope(&request.scope)?;
            if request.policy_identity != state.request.policy_identity {
                return Err(PolicyExceptionError::PolicyMismatch {
                    expected: state.request.policy_identity.clone(),
                    actual: request.policy_identity.clone(),
                });
            }
            let root = exception_request_root(request)?;
            state.request = request.clone();
            state.request.request_root = root;
            state.request_revision += 1;
            state.lifecycle = "requested".to_string();
            Ok(())
        }
        PolicyExceptionAction::AddFinding { finding } => {
            if state.findings.contains_key(&finding.id) {
                return Err(PolicyExceptionError::DuplicateOperation {
                    id: finding.id.clone(),
                });
            }
            state.findings.insert(finding.id.clone(), finding.clone());
            Ok(())
        }
        PolicyExceptionAction::ResolveFinding { finding_id } => {
            if state.findings.remove(finding_id).is_none() {
                return Err(PolicyExceptionError::InvalidField {
                    field: "finding".to_string(),
                    reason: format!("unknown finding {finding_id}"),
                });
            }
            Ok(())
        }
        PolicyExceptionAction::Approve { scope_root } => {
            let current_root = exception_request_root(&state.request)?;
            if *scope_root != current_root {
                return Err(PolicyExceptionError::PolicyMismatch {
                    expected: current_root,
                    actual: scope_root.clone(),
                });
            }
            if state.lifecycle != "requested" && state.lifecycle != "in-review" {
                return Err(PolicyExceptionError::ConflictingDecision);
            }
            state
                .approvals
                .push(format!("{}:{}", operation.actor, scope_root));
            if state.approvals.len() >= 2 {
                if state.lifecycle != "approved" {
                    state.lifecycle = "approved".to_string();
                }
            } else {
                state.lifecycle = "in-review".to_string();
            }
            Ok(())
        }
        PolicyExceptionAction::Reject { .. } => {
            state.lifecycle = "rejected".to_string();
            Ok(())
        }
        PolicyExceptionAction::ObserveValidity { observation } => {
            if let Some(prior) = state.observations.get(&observation.source) {
                if prior.observed_epoch > observation.observed_epoch {
                    return Err(PolicyExceptionError::StaleObservation {
                        source: observation.source.clone(),
                    });
                }
            }
            state
                .observations
                .insert(observation.source.clone(), observation.clone());
            Ok(())
        }
        PolicyExceptionAction::Revoke { .. } => {
            state.lifecycle = "revoked".to_string();
            Ok(())
        }
        PolicyExceptionAction::Expire { .. } => {
            state.lifecycle = "expired".to_string();
            Ok(())
        }
        PolicyExceptionAction::Close { .. } => {
            state.lifecycle = "closed".to_string();
            Ok(())
        }
        PolicyExceptionAction::Supersede { successor } => {
            state.lifecycle = "superseded".to_string();
            state.predecessor = Some(successor.clone());
            Ok(())
        }
    }
}

fn required_role(action: &PolicyExceptionAction) -> Option<ExceptionRole> {
    match action {
        PolicyExceptionAction::Init { .. } => None,
        PolicyExceptionAction::Revise { .. } => Some(ExceptionRole::Requester),
        PolicyExceptionAction::AddFinding { .. } | PolicyExceptionAction::ResolveFinding { .. } => {
            Some(ExceptionRole::Reviewer)
        }
        PolicyExceptionAction::Approve { .. } | PolicyExceptionAction::Reject { .. } => {
            Some(ExceptionRole::Approver)
        }
        PolicyExceptionAction::ObserveValidity { .. } => Some(ExceptionRole::Observer),
        PolicyExceptionAction::Revoke { .. } => Some(ExceptionRole::Revoker),
        PolicyExceptionAction::Expire { .. } => Some(ExceptionRole::Revoker),
        PolicyExceptionAction::Close { .. } => Some(ExceptionRole::Approver),
        PolicyExceptionAction::Supersede { .. } => Some(ExceptionRole::Requester),
    }
}

pub fn exception_actor_has_role(
    state: &PolicyExceptionState,
    actor: &str,
    role: ExceptionRole,
) -> bool {
    match role {
        ExceptionRole::Requester => state.requesters.contains(actor),
        ExceptionRole::Reviewer => state.reviewers.contains(actor),
        ExceptionRole::Approver => state.approvers.contains(actor),
        ExceptionRole::Revoker => state.revokers.contains(actor),
        ExceptionRole::Observer => state.observers.contains(actor),
    }
}

// r[impl cairn.policy_exception_cob.applicability]
pub fn classify_applicability(
    state: &PolicyExceptionState,
) -> Result<String, PolicyExceptionError> {
    if state.lifecycle == "revoked" || state.lifecycle == "expired" {
        return Ok("inapplicable".to_string());
    }
    if state.lifecycle != "approved" {
        return Ok("not-approved".to_string());
    }
    // Every observer source must supply a current finite-bound observation.
    let mut latest = 0u64;
    for (source, obs) in &state.observations {
        if !state.observers.contains(source) {
            continue;
        }
        if obs.observed_epoch > latest {
            latest = obs.observed_epoch;
        }
    }
    if !state.lifecycle_set_covers_observers() {
        return Err(PolicyExceptionError::MissingObservation {
            source: "all-observers".to_string(),
        });
    }
    let current_observation = state
        .observations
        .values()
        .max_by_key(|obs| obs.observed_epoch);
    let Some(observation) = current_observation else {
        return Err(PolicyExceptionError::MissingObservation {
            source: "any".to_string(),
        });
    };
    if observation.finite_valid_until <= latest {
        return Ok("expired".to_string());
    }
    Ok("applicable".to_string())
}

impl PolicyExceptionState {
    fn lifecycle_set_covers_observers(&self) -> bool {
        // All observers must have an observation at the latest epoch.
        let latest = self
            .observations
            .values()
            .map(|obs| obs.observed_epoch)
            .max()
            .unwrap_or(0);
        self.observers
            .iter()
            .all(|source| self.observations.get(source).map(|o| o.observed_epoch) == Some(latest))
            || self.observers.is_empty()
    }
}

pub fn exception_state_hash(state: &PolicyExceptionState) -> Result<String, PolicyExceptionError> {
    Ok(super::evidence::canonical_json_hash(&serde_json::json!(
        state
    )))
}

pub fn exception_content_root(
    state: &PolicyExceptionState,
) -> Result<String, PolicyExceptionError> {
    Ok(super::evidence::canonical_json_hash(&serde_json::json!({
        "kind": state.kind,
        "exception_id": state.exception_id,
        "request": state.request,
        "request_revision": state.request_revision,
        "findings": state.findings,
        "approvals": state.approvals,
        "observations": state.observations,
        "lifecycle": state.lifecycle,
        "predecessor": state.predecessor,
    })))
}

// r[impl cairn.policy_exception_cob.backends.roundtrip]
pub fn serialize_exception_portable(
    state: &PolicyExceptionState,
) -> Result<String, PolicyExceptionError> {
    let mut value = serde_json::json!({
        "schema": POLICY_EXCEPTION_PORTABLE_SCHEMA_V1,
        "backend": "portable",
        "state": state,
    });
    value["state_identity"] = serde_json::json!(exception_state_hash(state)?);
    value["content_identity"] = serde_json::json!(exception_content_root(state)?);
    value["portable_identity"] = serde_json::json!(portable_identity(&value));
    serde_json::to_string(&value).map_err(exception_json_error)
}

pub fn parse_exception_portable(text: &str) -> Result<PolicyExceptionState, PolicyExceptionError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(exception_json_error)?;
    if value.get("schema").and_then(serde_json::Value::as_str)
        != Some(POLICY_EXCEPTION_PORTABLE_SCHEMA_V1)
    {
        return Err(PolicyExceptionError::PortableTamper {
            field: "schema".to_string(),
        });
    }
    let state: PolicyExceptionState = serde_json::from_value(
        value
            .get("state")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(exception_json_error)?;
    if value
        .get("state_identity")
        .and_then(serde_json::Value::as_str)
        != Some(&exception_state_hash(&state)?)
    {
        return Err(PolicyExceptionError::PortableTamper {
            field: "state_identity".to_string(),
        });
    }
    Ok(state)
}

fn portable_identity(value: &serde_json::Value) -> String {
    let mut material = value.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("portable_identity");
    }
    super::evidence::canonical_json_hash(&material)
}

fn exception_json_error<E: std::fmt::Display>(error: E) -> PolicyExceptionError {
    PolicyExceptionError::Factory(format!("policy-exception JSON error: {error}"))
}

#[cfg(test)]
#[path = "policy_exception_tests.rs"]
mod tests;

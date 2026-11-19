//! Opt-in finite workflow obligations shared by guidance and completion.
//!
//! A single admitted definition produces both agent guidance and a
//! machine-readable completion predicate. Presentation adapters change layout
//! and prose only: evidence requirements, permitted command classes and the
//! declared authority come from the admitted definition. Native predicates
//! (agent receipts, freshness, completion authority and verification claim
//! revisions) retain ownership of their evidence; this module composes their
//! exact revisions and never re-derives them.
//!
//! The bounded temporal extension evaluates ordering between named events in a
//! declared finite trace interval with an explicit observer identity and a
//! supplied clock epoch. Missing or partial observations remain unknown. A
//! finite trace result never implies unbounded safety or liveness.
//!
//! Policy exceptions are referenced through their native identities and
//! revisions. An exception path requires current applicability and evidence for
//! its declared compensating control. The original unsatisfied condition and
//! basis stay visible; acceptance never relabels it as proved.
//!
//! This module adds no executor, clock service or monitoring. It is read-only
//! and has no filesystem, process, network or model effects.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::generated_agent_workflow::{
    AdmittedGeneratedWorkflowCatalog, GeneratedWorkflowAdapterFormat, GeneratedWorkflowIssue,
};
use super::policy_exception::{classify_applicability, PolicyExceptionState};

mod consumer;
pub use consumer::*;

pub const WORKFLOW_OBLIGATION_POLICY_SCHEMA_V1: &str = "cairn.workflow-obligation-policy.v1";
pub const WORKFLOW_OBLIGATION_PREDICATE_SCHEMA_V1: &str = "cairn.workflow-obligation-predicate.v1";
pub const WORKFLOW_OBLIGATION_REPORT_SCHEMA_V1: &str = "cairn.workflow-obligation-report.v1";
pub const WORKFLOW_OBLIGATION_EXPORT_SCHEMA_V1: &str = "cairn.workflow-obligation-export.v1";
pub const FINITE_OBLIGATION_PROFILE_V1: &str = "cairn.workflow-obligation.finite.v1";

pub const WORKFLOW_OBLIGATION_ISSUE_POLICY: &str = "workflow_obligation.policy.invalid";
pub const WORKFLOW_OBLIGATION_ISSUE_BINDING: &str = "workflow_obligation.binding.mismatch";
pub const WORKFLOW_OBLIGATION_ISSUE_EXPRESSION: &str = "workflow_obligation.expression.invalid";

// r[impl cairn.workflow_obligations.authority]
pub const WORKFLOW_OBLIGATION_BOUNDARY: &str = "Guidance and completion share one admitted finite definition. This module does not execute actions, observe clocks, monitor work, grant approval, or replace native evidence owners, policy-exception applicability, readiness, or the Pass | Block gate. Activity counts, checked task boxes and an agent success statement are not evidence. A finite trace ordering result is not evidence of unbounded safety or liveness.";

const MAX_DEFINITIONS_CEILING: usize = 64;
const MAX_OBLIGATIONS_CEILING: usize = 512;
const MAX_ALTERNATIVES_CEILING: usize = 32;
const MAX_PREDICATES_CEILING: usize = 128;
const MAX_EXCEPTIONS_CEILING: usize = 32;
const MAX_EVENTS_CEILING: usize = 64;
const MAX_DEPTH_CEILING: usize = 32;
const MAX_TEXT_BYTES_CEILING: usize = 4096;

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePredicateKind {
    Evidence,
    Applicability,
}

impl EvidencePredicateKind {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Applicability => "applicability",
        }
    }
}

/// A reference to an existing native, validated evidence predicate. The
/// accepted revision is the exact native requirement row revision; observations
/// must present that same revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEvidencePredicate {
    pub id: String,
    pub requirement_id: String,
    pub owner: String,
    pub accepted_revision_blake3: String,
    pub kind: EvidencePredicateKind,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationModel {
    pub id: String,
    pub evidence_kinds: Vec<String>,
    pub clock: String,
    pub completeness: String,
    pub unknown_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationBounds {
    pub max_definitions: usize,
    pub max_obligations: usize,
    pub max_alternatives: usize,
    pub max_predicates: usize,
    pub max_exceptions: usize,
    pub max_temporal_events: usize,
    pub max_depth: usize,
    pub max_text_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationPolicy {
    pub schema: String,
    pub profile: String,
    pub interpretation: String,
    pub evidence_predicates: Vec<NativeEvidencePredicate>,
    pub observation_model: ObservationModel,
    pub definitions: Vec<WorkflowObligationDefinition>,
    pub bounds: WorkflowObligationBounds,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPrecondition {
    pub id: String,
    pub action: String,
    pub required_predicates: Vec<String>,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedObservation {
    pub id: String,
    pub command_class: String,
    pub predicate: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressFact {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceCompleteness {
    Complete,
    Partial,
}

impl TraceCompleteness {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObligationExpr {
    Evidence {
        id: String,
        predicate: String,
    },
    AllOf {
        id: String,
        obligations: Vec<ObligationExpr>,
    },
    AnyOf {
        id: String,
        alternatives: Vec<ObligationAlternative>,
    },
    Applicable {
        id: String,
        condition: String,
        then: Box<ObligationExpr>,
    },
    Temporal {
        id: String,
        check: String,
    },
}

impl ObligationExpr {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Evidence { id, .. }
            | Self::AllOf { id, .. }
            | Self::AnyOf { id, .. }
            | Self::Applicable { id, .. }
            | Self::Temporal { id, .. } => id,
        }
    }

    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Evidence { .. } => "evidence",
            Self::AllOf { .. } => "all_of",
            Self::AnyOf { .. } => "any_of",
            Self::Applicable { .. } => "applicable",
            Self::Temporal { .. } => "temporal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationAlternative {
    pub id: String,
    pub obligations: Vec<ObligationExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompensatingControl {
    pub id: String,
    pub description: String,
    pub evidence_predicate: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExceptionPath {
    pub id: String,
    pub node: String,
    pub exception_id: String,
    pub request_revision: u64,
    pub policy_identity: String,
    pub scope_rule_id: String,
    pub control: CompensatingControl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalExtension {
    pub id: String,
    pub trace_id: String,
    pub first_event: String,
    pub second_event: String,
    pub observer: String,
    pub clock: String,
    pub max_span_epochs: u64,
    pub completeness: TraceCompleteness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationDefinition {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version: u32,
    pub intent_blake3: String,
    pub action_preconditions: Vec<ActionPrecondition>,
    pub expected_observations: Vec<ExpectedObservation>,
    pub progress_facts: Vec<ProgressFact>,
    pub completion: ObligationExpr,
    pub exceptions: Vec<ExceptionPath>,
    pub temporal: Option<TemporalExtension>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

impl std::fmt::Display for WorkflowObligationIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.field_path, self.message
        )
    }
}

impl std::error::Error for WorkflowObligationIssue {}

/// Exact binding admitted from the generated workflow catalog and the checked
/// policy identity. Any change to the workflow descriptor, policy, definition
/// or intent changes this identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationBinding {
    pub workflow_id: String,
    pub workflow_version: u32,
    pub definition_id: String,
    pub intent_blake3: String,
    pub workflow_policy_blake3: String,
    pub catalog_identity: String,
    pub binding_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedWorkflowObligation {
    pub definition: WorkflowObligationDefinition,
    pub binding: ObligationBinding,
    pub command_classes: Vec<String>,
    pub mutation_class: super::generated_agent_workflow::GeneratedWorkflowMutationClass,
    pub evidence_predicates: BTreeMap<String, NativeEvidencePredicate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedWorkflowObligationCatalog {
    pub policy: WorkflowObligationPolicy,
    pub workflow_policy_blake3: String,
    pub catalog_identity: String,
    pub definitions: Vec<AdmittedWorkflowObligation>,
}

// ---------------------------------------------------------------------------
// Errors and identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", content = "detail", rename_all = "snake_case")]
pub enum WorkflowObligationError {
    Unsupported(String),
    ScopeMismatch(String),
    InvalidInput(String),
    BoundExhaustion(String),
    MissingAlternative(String),
}

impl WorkflowObligationError {
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::ScopeMismatch(_) => "scope_mismatch",
            Self::InvalidInput(_) => "invalid_input",
            Self::BoundExhaustion(_) => "bound_exhaustion",
            Self::MissingAlternative(_) => "missing_alternative",
        }
    }
}

impl std::fmt::Display for WorkflowObligationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.outcome())
    }
}

impl std::error::Error for WorkflowObligationError {}

fn identity<T: Serialize>(domain: &str, value: &T) -> Result<String, WorkflowObligationError> {
    let value = serde_json::to_value(value)
        .map_err(|error| WorkflowObligationError::InvalidInput(error.to_string()))?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(super::evidence::canonical_json(&value).as_bytes());
    Ok(hash.finalize().to_hex().to_string())
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bounded_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_' | '.')
        })
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

fn issue(
    code: &str,
    field_path: impl Into<String>,
    message: impl Into<String>,
) -> WorkflowObligationIssue {
    WorkflowObligationIssue {
        code: code.to_string(),
        field_path: field_path.into(),
        message: message.into(),
    }
}

/// Admit the checked obligation policy against the already admitted generated
/// workflow catalog. References are checked here; nothing is duplicated from
/// the generated policy.
// r[impl cairn.workflow_obligations.policy]
pub fn admit_workflow_obligation_policy(
    policy: &WorkflowObligationPolicy,
    catalog: &AdmittedGeneratedWorkflowCatalog,
    workflow_policy_blake3: &str,
) -> Result<AdmittedWorkflowObligationCatalog, Vec<WorkflowObligationIssue>> {
    let mut issues = Vec::new();
    validate_policy_header(policy, &mut issues);
    let predicates = validate_predicates(policy, &mut issues);
    validate_observation_model(policy, &mut issues);
    validate_definitions(policy, catalog, &predicates, &mut issues);
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    let limit = policy.bounds.max_definitions.max(1) * 32;
    issues.truncate(limit);
    if !issues.is_empty() {
        return Err(issues);
    }

    let mut definitions = Vec::with_capacity(policy.definitions.len());
    for definition in &policy.definitions {
        let descriptor = catalog
            .policy
            .descriptors
            .iter()
            .find(|descriptor| descriptor.id == definition.workflow_id)
            .ok_or_else(|| {
                vec![issue(
                    WORKFLOW_OBLIGATION_ISSUE_BINDING,
                    format!("definitions.{}.workflow_id", definition.id),
                    "workflow descriptor is absent from the admitted catalog",
                )]
            })?;
        let binding_blake3 = identity(
            "cairn.workflow-obligation-binding.v1",
            &(
                &definition.id,
                &definition.workflow_id,
                definition.workflow_version,
                &definition.intent_blake3,
                workflow_policy_blake3,
                &catalog.catalog_identity,
            ),
        )
        .map_err(|error| {
            vec![issue(
                WORKFLOW_OBLIGATION_ISSUE_BINDING,
                format!("definitions.{}", definition.id),
                error.to_string(),
            )]
        })?;
        definitions.push(AdmittedWorkflowObligation {
            definition: definition.clone(),
            binding: ObligationBinding {
                workflow_id: definition.workflow_id.clone(),
                workflow_version: definition.workflow_version,
                definition_id: definition.id.clone(),
                intent_blake3: definition.intent_blake3.clone(),
                workflow_policy_blake3: workflow_policy_blake3.to_string(),
                catalog_identity: catalog.catalog_identity.clone(),
                binding_blake3,
            },
            command_classes: descriptor.command_classes.clone(),
            mutation_class: descriptor.mutation_class.clone(),
            evidence_predicates: predicates.clone(),
        });
    }
    definitions.sort_by(|left, right| left.definition.id.cmp(&right.definition.id));
    Ok(AdmittedWorkflowObligationCatalog {
        policy: policy.clone(),
        workflow_policy_blake3: workflow_policy_blake3.to_string(),
        catalog_identity: catalog.catalog_identity.clone(),
        definitions,
    })
}

/// The primary definition for a workflow is the one whose ID equals the
/// workflow ID. Additional scoped definitions (for example a bounded temporal
/// example) never replace it implicitly.
#[must_use]
pub fn find_workflow_obligation<'a>(
    catalog: &'a AdmittedWorkflowObligationCatalog,
    workflow_id: &str,
) -> Option<&'a AdmittedWorkflowObligation> {
    catalog
        .definitions
        .iter()
        .find(|admitted| admitted.definition.id == workflow_id)
}

fn validate_policy_header(
    policy: &WorkflowObligationPolicy,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    if policy.schema != WORKFLOW_OBLIGATION_POLICY_SCHEMA_V1 {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.schema",
            "workflow obligation policy schema is unsupported",
        ));
    }
    if policy.profile != FINITE_OBLIGATION_PROFILE_V1 {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.profile",
            "workflow obligation profile is unsupported",
        ));
    }
    if !bounded_text(&policy.interpretation, MAX_TEXT_BYTES_CEILING) {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.interpretation",
            "workflow obligation interpretation must be bounded and non-empty",
        ));
    }
    let bounds = &policy.bounds;
    if bounds.max_definitions == 0
        || bounds.max_definitions > MAX_DEFINITIONS_CEILING
        || bounds.max_obligations == 0
        || bounds.max_obligations > MAX_OBLIGATIONS_CEILING
        || bounds.max_alternatives == 0
        || bounds.max_alternatives > MAX_ALTERNATIVES_CEILING
        || bounds.max_predicates == 0
        || bounds.max_predicates > MAX_PREDICATES_CEILING
        || bounds.max_exceptions == 0
        || bounds.max_exceptions > MAX_EXCEPTIONS_CEILING
        || bounds.max_temporal_events == 0
        || bounds.max_temporal_events > MAX_EVENTS_CEILING
        || bounds.max_depth == 0
        || bounds.max_depth > MAX_DEPTH_CEILING
        || bounds.max_text_bytes == 0
        || bounds.max_text_bytes > MAX_TEXT_BYTES_CEILING
    {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.bounds",
            "workflow obligation bounds are outside the supported ceilings",
        ));
    }
    if policy.evidence_predicates.len() > bounds.max_predicates
        || policy.definitions.len() > bounds.max_definitions
    {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy",
            "workflow obligation policy exceeds a collection bound",
        ));
    }
    if policy.non_claims.is_empty()
        || policy.non_claims.len() > bounds.max_obligations
        || policy
            .non_claims
            .iter()
            .any(|value| !bounded_text(value, bounds.max_text_bytes))
    {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.non_claims",
            "workflow obligation non-claims are incomplete or out of bounds",
        ));
    }
}

fn validate_predicates(
    policy: &WorkflowObligationPolicy,
    issues: &mut Vec<WorkflowObligationIssue>,
) -> BTreeMap<String, NativeEvidencePredicate> {
    let mut predicates = BTreeMap::new();
    for (index, predicate) in policy.evidence_predicates.iter().enumerate() {
        let path = format!("workflow_obligation_policy.evidence_predicates[{index}]");
        if !valid_id(&predicate.id)
            || predicates
                .insert(predicate.id.clone(), predicate.clone())
                .is_some()
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                format!("{path}.id"),
                "evidence predicate IDs must be safe and unique",
            ));
        }
        if predicate.requirement_id.trim().is_empty()
            || !predicate.requirement_id.starts_with("cairn.")
            || predicate.owner.trim().is_empty()
            || predicate.description.trim().is_empty()
            || !digest(&predicate.accepted_revision_blake3)
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                path,
                "evidence predicates must name a native requirement, an owner, a description and an exact 64-character BLAKE3 revision",
            ));
        }
    }
    if predicates.is_empty() {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.evidence_predicates",
            "at least one native evidence predicate is required",
        ));
    }
    predicates
}

fn validate_observation_model(
    policy: &WorkflowObligationPolicy,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let model = &policy.observation_model;
    if !valid_id(&model.id)
        || model.evidence_kinds.is_empty()
        || model
            .evidence_kinds
            .iter()
            .any(|kind| !bounded_text(kind, policy.bounds.max_text_bytes))
        || !bounded_text(&model.clock, policy.bounds.max_text_bytes)
        || !bounded_text(&model.completeness, policy.bounds.max_text_bytes)
        || !bounded_text(&model.unknown_rule, policy.bounds.max_text_bytes)
    {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            "workflow_obligation_policy.observation_model",
            "the observation model must declare evidence kinds, clock, completeness and the unknown rule",
        ));
    }
}

fn validate_definitions(
    policy: &WorkflowObligationPolicy,
    catalog: &AdmittedGeneratedWorkflowCatalog,
    predicates: &BTreeMap<String, NativeEvidencePredicate>,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let mut ids = BTreeSet::new();
    for (index, definition) in policy.definitions.iter().enumerate() {
        let path = format!("workflow_obligation_policy.definitions[{index}]");
        if !valid_id(&definition.id) || !ids.insert(definition.id.clone()) {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                format!("{path}.id"),
                "definition IDs must be safe and unique",
            ));
        }
        let Some(descriptor) = catalog
            .policy
            .descriptors
            .iter()
            .find(|descriptor| descriptor.id == definition.workflow_id)
        else {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_BINDING,
                format!("{path}.workflow_id"),
                "definition does not bind an admitted workflow descriptor",
            ));
            continue;
        };
        if definition.workflow_version != descriptor.version {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_BINDING,
                format!("{path}.workflow_version"),
                "definition workflow version does not match the admitted descriptor",
            ));
        }
        if !digest(&definition.intent_blake3) {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_BINDING,
                format!("{path}.intent_blake3"),
                "definition must bind an exact 64-character intent identity",
            ));
        }
        let mut node_ids = BTreeSet::new();
        let context = ExprContext {
            policy,
            predicates,
            temporal: &definition.temporal,
            path: &path,
        };
        validate_expr(&context, &definition.completion, 1, &mut node_ids, issues);
        validate_preconditions(policy, definition, descriptor, predicates, &path, issues);
        validate_expected_observations(policy, definition, descriptor, predicates, &path, issues);
        validate_progress_facts(policy, definition, &path, issues);
        validate_exceptions(policy, definition, predicates, &node_ids, &path, issues);
        validate_temporal(definition, &path, issues);
        if definition.non_claims.is_empty()
            || definition
                .non_claims
                .iter()
                .any(|value| !bounded_text(value, policy.bounds.max_text_bytes))
            || !policy
                .non_claims
                .iter()
                .all(|required| definition.non_claims.contains(required))
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                format!("{path}.non_claims"),
                "definition non-claims must be non-empty, bounded and retain the policy non-claims",
            ));
        }
    }
}

struct ExprContext<'a> {
    policy: &'a WorkflowObligationPolicy,
    predicates: &'a BTreeMap<String, NativeEvidencePredicate>,
    temporal: &'a Option<TemporalExtension>,
    path: &'a str,
}

fn validate_expr(
    context: &ExprContext<'_>,
    expression: &ObligationExpr,
    depth: usize,
    node_ids: &mut BTreeSet<String>,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let path = context.path;
    if depth > context.policy.bounds.max_depth || !node_ids.insert(expression.id().to_string()) {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
            path,
            "obligation expression repeats a node ID or exceeds the depth bound",
        ));
    }
    match expression {
        ObligationExpr::Evidence { predicate, .. } => match context.predicates.get(predicate) {
            Some(known) if known.kind == EvidencePredicateKind::Evidence => {}
            Some(_) => issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                format!("{path}.{}.predicate", expression.id()),
                "an evidence node references an applicability predicate",
            )),
            None => issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                format!("{path}.{}.predicate", expression.id()),
                "an evidence node references an unknown native predicate",
            )),
        },
        ObligationExpr::AllOf { obligations, .. } => {
            if obligations.is_empty() || obligations.len() > context.policy.bounds.max_obligations {
                issues.push(issue(
                    WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                    format!("{path}.{}.obligations", expression.id()),
                    "all_of requires between one and the admitted obligation count",
                ));
            }
            for child in obligations {
                validate_expr(context, child, depth + 1, node_ids, issues);
            }
        }
        ObligationExpr::AnyOf { alternatives, .. } => {
            if alternatives.is_empty()
                || alternatives.len() > context.policy.bounds.max_alternatives
            {
                issues.push(issue(
                    WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                    format!("{path}.{}.alternatives", expression.id()),
                    "any_of requires between one and the admitted alternative count",
                ));
            }
            let mut alternative_ids = BTreeSet::new();
            for alternative in alternatives {
                if !valid_id(&alternative.id)
                    || !alternative_ids.insert(alternative.id.clone())
                    || alternative.obligations.is_empty()
                {
                    issues.push(issue(
                        WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                        format!("{path}.{}.alternatives", expression.id()),
                        "alternatives need unique safe IDs and at least one obligation",
                    ));
                }
                for child in &alternative.obligations {
                    validate_expr(context, child, depth + 1, node_ids, issues);
                }
            }
        }
        ObligationExpr::Applicable {
            condition, then, ..
        } => {
            match context.predicates.get(condition) {
                Some(known) if known.kind == EvidencePredicateKind::Applicability => {}
                Some(_) => issues.push(issue(
                    WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                    format!("{path}.{}.condition", expression.id()),
                    "a conditional node requires an applicability predicate",
                )),
                None => issues.push(issue(
                    WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                    format!("{path}.{}.condition", expression.id()),
                    "a conditional node references an unknown predicate",
                )),
            }
            validate_expr(context, then, depth + 1, node_ids, issues);
        }
        ObligationExpr::Temporal { check, .. } => match context.temporal {
            Some(extension) if extension.id == *check => {}
            Some(_) => issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                format!("{path}.{}.check", expression.id()),
                "temporal node does not reference the definition's declared extension",
            )),
            None => issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_EXPRESSION,
                format!("{path}.{}.check", expression.id()),
                "temporal node requires a declared temporal extension",
            )),
        },
    }
}

fn validate_preconditions(
    policy: &WorkflowObligationPolicy,
    definition: &WorkflowObligationDefinition,
    descriptor: &super::generated_agent_workflow::GeneratedWorkflowDescriptor,
    predicates: &BTreeMap<String, NativeEvidencePredicate>,
    path: &str,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let mut ids = BTreeSet::new();
    for (index, precondition) in definition.action_preconditions.iter().enumerate() {
        let field = format!("{path}.action_preconditions[{index}]");
        if !valid_id(&precondition.id)
            || !ids.insert(precondition.id.clone())
            || !descriptor.command_classes.contains(&precondition.action)
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                field,
                "action preconditions need unique IDs and must reference an admitted descriptor command class",
            ));
        }
        if precondition.required_predicates.is_empty()
            || precondition
                .required_predicates
                .iter()
                .any(|predicate| !predicates.contains_key(predicate))
            || !bounded_text(&precondition.authority, policy.bounds.max_text_bytes)
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                format!("{path}.action_preconditions[{index}].required_predicates"),
                "action preconditions must reference known predicates and declare authority",
            ));
        }
    }
}

fn validate_expected_observations(
    policy: &WorkflowObligationPolicy,
    definition: &WorkflowObligationDefinition,
    descriptor: &super::generated_agent_workflow::GeneratedWorkflowDescriptor,
    predicates: &BTreeMap<String, NativeEvidencePredicate>,
    path: &str,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let mut ids = BTreeSet::new();
    for (index, expected) in definition.expected_observations.iter().enumerate() {
        if !valid_id(&expected.id)
            || !ids.insert(expected.id.clone())
            || !descriptor.command_classes.contains(&expected.command_class)
            || !predicates.contains_key(&expected.predicate)
            || !bounded_text(&expected.description, policy.bounds.max_text_bytes)
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                format!("{path}.expected_observations[{index}]"),
                "expected observations need unique IDs, an admitted command class, a known predicate and a bounded description",
            ));
        }
    }
}

fn validate_progress_facts(
    policy: &WorkflowObligationPolicy,
    definition: &WorkflowObligationDefinition,
    path: &str,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let mut ids = BTreeSet::new();
    if definition.progress_facts.is_empty() {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            format!("{path}.progress_facts"),
            "at least one progress fact must be declared separately from completion",
        ));
    }
    for (index, fact) in definition.progress_facts.iter().enumerate() {
        if !valid_id(&fact.id)
            || !ids.insert(fact.id.clone())
            || !bounded_text(&fact.description, policy.bounds.max_text_bytes)
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                format!("{path}.progress_facts[{index}]"),
                "progress facts need unique IDs and a bounded description",
            ));
        }
    }
}

fn validate_exceptions(
    policy: &WorkflowObligationPolicy,
    definition: &WorkflowObligationDefinition,
    predicates: &BTreeMap<String, NativeEvidencePredicate>,
    node_ids: &BTreeSet<String>,
    path: &str,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let mut ids = BTreeSet::new();
    if definition.exceptions.len() > policy.bounds.max_exceptions {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            format!("{path}.exceptions"),
            "exception paths exceed the admitted bound",
        ));
    }
    for (index, exception) in definition.exceptions.iter().enumerate() {
        let field = format!("{path}.exceptions[{index}]");
        if !valid_id(&exception.id)
            || !ids.insert(exception.id.clone())
            || !node_ids.contains(&exception.node)
            || exception.exception_id.trim().is_empty()
            || exception.scope_rule_id.trim().is_empty()
            || !digest(&exception.policy_identity)
            || !valid_id(&exception.control.id)
            || !bounded_text(&exception.control.description, policy.bounds.max_text_bytes)
            || !matches!(
                predicates.get(&exception.control.evidence_predicate),
                Some(predicate) if predicate.kind == EvidencePredicateKind::Evidence
            )
        {
            issues.push(issue(
                WORKFLOW_OBLIGATION_ISSUE_POLICY,
                field,
                "exception paths need unique IDs, an existing completion node, a native exception identity/revision, a scope rule and one evidence-backed compensating control",
            ));
        }
    }
}

fn validate_temporal(
    definition: &WorkflowObligationDefinition,
    path: &str,
    issues: &mut Vec<WorkflowObligationIssue>,
) {
    let Some(extension) = &definition.temporal else {
        return;
    };
    if !valid_id(&extension.id)
        || extension.trace_id.trim().is_empty()
        || extension.first_event.trim().is_empty()
        || extension.second_event.trim().is_empty()
        || extension.first_event == extension.second_event
        || extension.observer.trim().is_empty()
        || extension.clock.trim().is_empty()
        || extension.max_span_epochs == 0
    {
        issues.push(issue(
            WORKFLOW_OBLIGATION_ISSUE_POLICY,
            format!("{path}.temporal"),
            "the temporal extension must declare two distinct named events, an observer, a clock, and a finite positive span",
        ));
    }
}

// ---------------------------------------------------------------------------
// Guidance and predicate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationFormat {
    Markdown,
    AgentSkills,
    Compact,
}

/// Layout-only presentation. It has no fields for evidence, obligations,
/// command classes or authority, so it cannot weaken the predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    pub format: PresentationFormat,
    pub heading_level: u8,
    pub bullet: String,
}

impl Presentation {
    fn validate(&self) -> Result<(), WorkflowObligationError> {
        if !(1..=5).contains(&self.heading_level) || !matches!(self.bullet.as_str(), "-" | "*") {
            return Err(WorkflowObligationError::InvalidInput(
                "presentation may change only heading level and bullet character".into(),
            ));
        }
        Ok(())
    }
}

#[must_use]
pub fn presentation_for_adapter(format: &GeneratedWorkflowAdapterFormat) -> Presentation {
    match format {
        GeneratedWorkflowAdapterFormat::AgentSkills => Presentation {
            format: PresentationFormat::AgentSkills,
            heading_level: 2,
            bullet: "-".to_string(),
        },
        GeneratedWorkflowAdapterFormat::Claude => Presentation {
            format: PresentationFormat::Markdown,
            heading_level: 2,
            bullet: "-".to_string(),
        },
        GeneratedWorkflowAdapterFormat::Codex => Presentation {
            format: PresentationFormat::Compact,
            heading_level: 3,
            bullet: "*".to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidanceSection {
    pub id: String,
    pub heading: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationGuidance {
    pub workflow_id: String,
    pub workflow_version: u32,
    pub definition_id: String,
    pub binding_blake3: String,
    pub requirements_blake3: String,
    pub allowed_command_classes: Vec<String>,
    pub authority: String,
    pub selected_alternatives: BTreeMap<String, String>,
    pub sections: Vec<GuidanceSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRequirement {
    pub node: String,
    pub predicate: String,
    pub requirement_id: String,
    pub owner: String,
    pub accepted_revision_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationSummary {
    pub id: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationPredicate {
    pub schema: String,
    pub workflow_id: String,
    pub workflow_version: u32,
    pub definition_id: String,
    pub binding: ObligationBinding,
    pub interpretation: String,
    pub subject_blake3: String,
    pub context_blake3: String,
    pub selected_alternatives: BTreeMap<String, String>,
    pub expression: ObligationExpr,
    pub evidence_requirements: Vec<EvidenceRequirement>,
    pub allowed_command_classes: Vec<String>,
    pub authority: String,
    pub predicate_blake3: String,
    pub boundary: String,
}

/// Consumer export for selected-work/Kiln composition. It carries the same
/// binding, predicate and obligation IDs used by evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationExport {
    pub schema: String,
    pub workflow_id: String,
    pub workflow_version: u32,
    pub definition_id: String,
    pub binding: ObligationBinding,
    pub subject_blake3: String,
    pub context_blake3: String,
    pub predicate: ObligationPredicate,
    pub obligations: Vec<ObligationSummary>,
    pub alternatives: BTreeMap<String, Vec<String>>,
    pub non_claims: Vec<String>,
    pub boundary: String,
}

fn resolve_predicate(
    admitted: &AdmittedWorkflowObligation,
    policy: &WorkflowObligationPolicy,
    selection: &BTreeMap<String, String>,
    subject_blake3: &str,
    context_blake3: &str,
) -> Result<ObligationPredicate, WorkflowObligationError> {
    if !digest(subject_blake3) || !digest(context_blake3) {
        return Err(WorkflowObligationError::InvalidInput(
            "subject and context revision identities are required".into(),
        ));
    }
    let mut evidence_requirements = Vec::new();
    let mut seen = BTreeSet::new();
    validate_selection(&admitted.definition.completion, selection)?;
    collect_requirements(
        &admitted.definition.completion,
        &admitted.evidence_predicates,
        &mut evidence_requirements,
        &mut seen,
    )?;
    for exception in &admitted.definition.exceptions {
        if !seen.insert(exception.id.clone()) {
            continue;
        }
        let known = admitted
            .evidence_predicates
            .get(&exception.control.evidence_predicate)
            .ok_or_else(|| {
                WorkflowObligationError::Unsupported(format!(
                    "exception {} references unknown control predicate {}",
                    exception.id, exception.control.evidence_predicate
                ))
            })?;
        evidence_requirements.push(EvidenceRequirement {
            node: exception.id.clone(),
            predicate: exception.control.evidence_predicate.clone(),
            requirement_id: known.requirement_id.clone(),
            owner: known.owner.clone(),
            accepted_revision_blake3: known.accepted_revision_blake3.clone(),
        });
    }
    evidence_requirements.sort_by(|left, right| left.node.cmp(&right.node));
    let authority = format!(
        "native requirement owners retain authority: {}",
        evidence_requirements
            .iter()
            .map(|requirement| requirement.owner.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut predicate = ObligationPredicate {
        schema: WORKFLOW_OBLIGATION_PREDICATE_SCHEMA_V1.to_string(),
        workflow_id: admitted.definition.workflow_id.clone(),
        workflow_version: admitted.definition.workflow_version,
        definition_id: admitted.definition.id.clone(),
        binding: admitted.binding.clone(),
        interpretation: policy.interpretation.clone(),
        subject_blake3: subject_blake3.to_string(),
        context_blake3: context_blake3.to_string(),
        selected_alternatives: selection.clone(),
        expression: admitted.definition.completion.clone(),
        evidence_requirements,
        allowed_command_classes: admitted.command_classes.clone(),
        authority,
        predicate_blake3: String::new(),
        boundary: WORKFLOW_OBLIGATION_BOUNDARY.to_string(),
    };
    let predicate_blake3 = identity("cairn.workflow-obligation-predicate.v1", &predicate)?;
    predicate.predicate_blake3 = predicate_blake3;
    Ok(predicate)
}

// r[impl cairn.workflow_obligations.predicate]
pub fn obligation_predicate(
    admitted: &AdmittedWorkflowObligation,
    policy: &WorkflowObligationPolicy,
    selection: &BTreeMap<String, String>,
    subject_blake3: &str,
    context_blake3: &str,
) -> Result<ObligationPredicate, WorkflowObligationError> {
    resolve_predicate(admitted, policy, selection, subject_blake3, context_blake3)
}

fn validate_selection(
    expression: &ObligationExpr,
    selection: &BTreeMap<String, String>,
) -> Result<(), WorkflowObligationError> {
    let mut any_of_nodes = BTreeSet::new();
    collect_any_of_ids(expression, &mut any_of_nodes);
    if let Some(unknown) = selection
        .keys()
        .find(|node| !any_of_nodes.contains(node.as_str()))
    {
        return Err(WorkflowObligationError::InvalidInput(format!(
            "selection names unknown any_of node {unknown}"
        )));
    }
    match expression {
        ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => Ok(()),
        ObligationExpr::AllOf { obligations, .. } => obligations
            .iter()
            .try_for_each(|child| validate_declared_alternative(child, selection)),
        ObligationExpr::AnyOf { id, alternatives } => {
            if let Some(selected) = selection.get(id) {
                let declared = alternatives
                    .iter()
                    .any(|alternative| alternative.id == *selected);
                if !declared {
                    return Err(WorkflowObligationError::MissingAlternative(format!(
                        "any_of node {id} does not declare alternative {selected}"
                    )));
                }
            }
            alternatives.iter().try_for_each(|alternative| {
                alternative
                    .obligations
                    .iter()
                    .try_for_each(|child| validate_declared_alternative(child, selection))
            })
        }
        ObligationExpr::Applicable { then, .. } => validate_declared_alternative(then, selection),
    }
}

/// Check that every selected alternative is declared by its node. The node set
/// is checked once at the root, so this recursion must not re-check it against
/// a sub-expression.
fn validate_declared_alternative(
    expression: &ObligationExpr,
    selection: &BTreeMap<String, String>,
) -> Result<(), WorkflowObligationError> {
    match expression {
        ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => Ok(()),
        ObligationExpr::AllOf { obligations, .. } => obligations
            .iter()
            .try_for_each(|child| validate_declared_alternative(child, selection)),
        ObligationExpr::AnyOf { id, alternatives } => {
            if let Some(selected) = selection.get(id) {
                let declared = alternatives
                    .iter()
                    .any(|alternative| alternative.id == *selected);
                if !declared {
                    return Err(WorkflowObligationError::MissingAlternative(format!(
                        "any_of node {id} does not declare alternative {selected}"
                    )));
                }
            }
            alternatives.iter().try_for_each(|alternative| {
                alternative
                    .obligations
                    .iter()
                    .try_for_each(|child| validate_declared_alternative(child, selection))
            })
        }
        ObligationExpr::Applicable { then, .. } => validate_declared_alternative(then, selection),
    }
}

fn collect_any_of_ids(expression: &ObligationExpr, ids: &mut BTreeSet<String>) {
    match expression {
        ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => {}
        ObligationExpr::AllOf { obligations, .. } => {
            for child in obligations {
                collect_any_of_ids(child, ids);
            }
        }
        ObligationExpr::AnyOf { id, alternatives } => {
            ids.insert(id.clone());
            for alternative in alternatives {
                for child in &alternative.obligations {
                    collect_any_of_ids(child, ids);
                }
            }
        }
        ObligationExpr::Applicable { then, .. } => collect_any_of_ids(then, ids),
    }
}

fn collect_requirements(
    expression: &ObligationExpr,
    predicates: &BTreeMap<String, NativeEvidencePredicate>,
    requirements: &mut Vec<EvidenceRequirement>,
    seen: &mut BTreeSet<String>,
) -> Result<(), WorkflowObligationError> {
    match expression {
        ObligationExpr::Evidence { id, predicate } => {
            if !seen.insert(id.clone()) {
                return Ok(());
            }
            let known = predicates.get(predicate).ok_or_else(|| {
                WorkflowObligationError::Unsupported(format!(
                    "evidence node {id} references unknown predicate {predicate}"
                ))
            })?;
            requirements.push(EvidenceRequirement {
                node: id.clone(),
                predicate: predicate.clone(),
                requirement_id: known.requirement_id.clone(),
                owner: known.owner.clone(),
                accepted_revision_blake3: known.accepted_revision_blake3.clone(),
            });
        }
        ObligationExpr::AllOf { obligations, .. } => {
            for child in obligations {
                collect_requirements(child, predicates, requirements, seen)?;
            }
        }
        ObligationExpr::AnyOf { alternatives, .. } => {
            for alternative in alternatives {
                for child in &alternative.obligations {
                    collect_requirements(child, predicates, requirements, seen)?;
                }
            }
        }
        ObligationExpr::Applicable {
            condition, then, ..
        } => {
            if seen.insert(condition.clone()) {
                let known = predicates.get(condition).ok_or_else(|| {
                    WorkflowObligationError::Unsupported(format!(
                        "applicability condition {condition} is unknown"
                    ))
                })?;
                requirements.push(EvidenceRequirement {
                    node: condition.clone(),
                    predicate: condition.clone(),
                    requirement_id: known.requirement_id.clone(),
                    owner: known.owner.clone(),
                    accepted_revision_blake3: known.accepted_revision_blake3.clone(),
                });
            }
            collect_requirements(then, predicates, requirements, seen)?;
        }
        ObligationExpr::Temporal { .. } => {}
    }
    Ok(())
}

// r[impl cairn.workflow_obligations.guidance]
pub fn obligation_guidance(
    admitted: &AdmittedWorkflowObligation,
    policy: &WorkflowObligationPolicy,
    selection: &BTreeMap<String, String>,
) -> Result<ObligationGuidance, WorkflowObligationError> {
    let requirements_blake3 = identity(
        "cairn.workflow-obligation-requirements.v1",
        &(
            &admitted.definition.completion,
            &admitted.definition.exceptions,
            &admitted.definition.temporal,
            selection,
        ),
    )?;
    let mut predicate_requirements = Vec::new();
    let mut seen = BTreeSet::new();
    validate_selection(&admitted.definition.completion, selection)?;
    collect_requirements(
        &admitted.definition.completion,
        &admitted.evidence_predicates,
        &mut predicate_requirements,
        &mut seen,
    )?;
    predicate_requirements.sort_by(|left, right| left.node.cmp(&right.node));

    let mut sections = Vec::new();
    sections.push(GuidanceSection {
        id: "completion".to_string(),
        heading: "Completion obligations".to_string(),
        items: predicate_requirements
            .iter()
            .map(|requirement| {
                format!(
                    "{}: {} ({}) at revision {}; owner {}",
                    requirement.node,
                    requirement.requirement_id,
                    requirement.predicate,
                    requirement.accepted_revision_blake3,
                    requirement.owner
                )
            })
            .collect(),
    });
    let mut alternatives = BTreeMap::new();
    collect_alternatives(&admitted.definition.completion, &mut alternatives);
    let alternative_items = alternatives
        .iter()
        .map(|(node, values)| format!("{node}: choose exactly one of {}", values.join(", ")))
        .collect::<Vec<_>>();
    if !alternative_items.is_empty() {
        sections.push(GuidanceSection {
            id: "alternatives".to_string(),
            heading: "Alternative strategies".to_string(),
            items: alternative_items,
        });
        let selected = selection
            .iter()
            .map(|(node, alternative)| format!("{node}: {alternative}"))
            .collect::<Vec<_>>();
        sections.push(GuidanceSection {
            id: "selected_alternatives".to_string(),
            heading: "Selected alternative".to_string(),
            items: if selected.is_empty() {
                vec!["no alternative selected; completion is unresolved".to_string()]
            } else {
                selected
            },
        });
    }
    sections.push(GuidanceSection {
        id: "action_preconditions".to_string(),
        heading: "Action preconditions".to_string(),
        items: admitted
            .definition
            .action_preconditions
            .iter()
            .map(|precondition| {
                format!(
                    "{}: {} requires {}; {}",
                    precondition.id,
                    precondition.action,
                    precondition.required_predicates.join(", "),
                    precondition.authority
                )
            })
            .collect(),
    });
    sections.push(GuidanceSection {
        id: "expected_observations".to_string(),
        heading: "Expected observations".to_string(),
        items: admitted
            .definition
            .expected_observations
            .iter()
            .map(|expected| {
                format!(
                    "{}: {} yields {} for {}",
                    expected.id, expected.command_class, expected.predicate, expected.description
                )
            })
            .collect(),
    });
    sections.push(GuidanceSection {
        id: "progress_facts".to_string(),
        heading: "Progress facts".to_string(),
        items: admitted
            .definition
            .progress_facts
            .iter()
            .map(|fact| {
                format!(
                    "{}: {}; progress facts never discharge completion obligations",
                    fact.id, fact.description
                )
            })
            .collect(),
    });
    if let Some(extension) = &admitted.definition.temporal {
        sections.push(GuidanceSection {
            id: "temporal".to_string(),
            heading: "Bounded temporal check".to_string(),
            items: vec![format!(
                "{}: {} before {} within trace {} observed by {} on clock {}; completeness {}; finite interval of {} epochs. Missing or partial observations stay unknown, and this finite result implies no unbounded safety or liveness.",
                extension.id,
                extension.first_event,
                extension.second_event,
                extension.trace_id,
                extension.observer,
                extension.clock,
                extension.completeness.tag(),
                extension.max_span_epochs
            )],
        });
    }
    if !admitted.definition.exceptions.is_empty() {
        sections.push(GuidanceSection {
            id: "exceptions".to_string(),
            heading: "Policy exception paths".to_string(),
            items: admitted
                .definition
                .exceptions
                .iter()
                .map(|exception| {
                    format!(
                        "{}: {} under native exception {} revision {} for rule {}; requires compensating control {} ({}) and never proves the original condition",
                        exception.id,
                        exception.node,
                        exception.exception_id,
                        exception.request_revision,
                        exception.scope_rule_id,
                        exception.control.id,
                        exception.control.description
                    )
                })
                .collect(),
        });
    }
    sections.push(GuidanceSection {
        id: "authority".to_string(),
        heading: "Authority".to_string(),
        items: vec![format!(
            "Allowed command classes: {}. Interpretation: {}.",
            admitted.command_classes.join(", "),
            policy.interpretation
        )],
    });
    sections.push(GuidanceSection {
        id: "non_claims".to_string(),
        heading: "Non-claims".to_string(),
        items: admitted.definition.non_claims.clone(),
    });
    Ok(ObligationGuidance {
        workflow_id: admitted.definition.workflow_id.clone(),
        workflow_version: admitted.definition.workflow_version,
        definition_id: admitted.definition.id.clone(),
        binding_blake3: admitted.binding.binding_blake3.clone(),
        requirements_blake3,
        allowed_command_classes: admitted.command_classes.clone(),
        authority: admitted.command_classes.join(", "),
        selected_alternatives: selection.clone(),
        sections,
    })
}

fn collect_alternatives(
    expression: &ObligationExpr,
    alternatives: &mut BTreeMap<String, Vec<String>>,
) {
    match expression {
        ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => {}
        ObligationExpr::AllOf { obligations, .. } => {
            for child in obligations {
                collect_alternatives(child, alternatives);
            }
        }
        ObligationExpr::AnyOf {
            id,
            alternatives: values,
        } => {
            alternatives.insert(
                id.clone(),
                values
                    .iter()
                    .map(|alternative| alternative.id.clone())
                    .collect(),
            );
            for alternative in values {
                for child in &alternative.obligations {
                    collect_alternatives(child, alternatives);
                }
            }
        }
        ObligationExpr::Applicable { then, .. } => collect_alternatives(then, alternatives),
    }
}

/// Render guidance under a layout-only presentation. Every content line comes
/// from `ObligationGuidance`; a presentation edit cannot add evidence, remove a
/// requirement or widen command authority.
// r[impl cairn.workflow_obligations.adapter_invariance]
pub fn render_guidance(
    guidance: &ObligationGuidance,
    presentation: &Presentation,
) -> Result<String, WorkflowObligationError> {
    presentation.validate()?;
    let mut output = String::new();
    let hashes = format!(
        "workflow {} version {} definition {} binding {} requirements {}",
        guidance.workflow_id,
        guidance.workflow_version,
        guidance.definition_id,
        guidance.binding_blake3,
        guidance.requirements_blake3
    );
    match presentation.format {
        PresentationFormat::Markdown | PresentationFormat::AgentSkills => {
            let heading = "#".repeat(usize::from(presentation.heading_level));
            let section_heading = "#".repeat(usize::from(presentation.heading_level) + 1);
            output.push_str(&format!("{heading} Workflow obligations\n\n"));
            output.push_str(&format!("{hashes}\n\n"));
            for section in &guidance.sections {
                output.push_str(&format!("{section_heading} {}\n\n", section.heading));
                for item in &section.items {
                    output.push_str(&format!("{} {item}\n", presentation.bullet));
                }
                output.push('\n');
            }
        }
        PresentationFormat::Compact => {
            output.push_str("Workflow obligations: ");
            output.push_str(&hashes);
            output.push('.');
            for section in &guidance.sections {
                output.push_str(&format!(" {}:", section.heading));
                for item in &section.items {
                    output.push(' ');
                    output.push_str(item);
                    output.push('.');
                }
            }
            output.push('\n');
        }
    }
    Ok(output)
}

// r[impl cairn.workflow_obligations.export]
pub fn export_workflow_obligation(
    admitted: &AdmittedWorkflowObligation,
    policy: &WorkflowObligationPolicy,
    selection: &BTreeMap<String, String>,
    subject_blake3: &str,
    context_blake3: &str,
) -> Result<WorkflowObligationExport, WorkflowObligationError> {
    let predicate = resolve_predicate(admitted, policy, selection, subject_blake3, context_blake3)?;
    let mut obligations = Vec::new();
    collect_summaries(&admitted.definition.completion, &mut obligations);
    for exception in &admitted.definition.exceptions {
        obligations.push(ObligationSummary {
            id: exception.id.clone(),
            kind: "exception".to_string(),
            detail: format!(
                "substitutes only while native exception {} revision {} is applicable for rule {} with compensating control {}; the original condition remains unproved",
                exception.exception_id,
                exception.request_revision,
                exception.scope_rule_id,
                exception.control.id
            ),
        });
    }
    obligations.sort_by(|left, right| left.id.cmp(&right.id));
    let mut alternatives = BTreeMap::new();
    collect_alternatives(&admitted.definition.completion, &mut alternatives);
    Ok(WorkflowObligationExport {
        schema: WORKFLOW_OBLIGATION_EXPORT_SCHEMA_V1.to_string(),
        workflow_id: admitted.definition.workflow_id.clone(),
        workflow_version: admitted.definition.workflow_version,
        definition_id: admitted.definition.id.clone(),
        binding: admitted.binding.clone(),
        subject_blake3: subject_blake3.to_string(),
        context_blake3: context_blake3.to_string(),
        predicate,
        obligations,
        alternatives,
        non_claims: admitted.definition.non_claims.clone(),
        boundary: WORKFLOW_OBLIGATION_BOUNDARY.to_string(),
    })
}

fn collect_summaries(expression: &ObligationExpr, summaries: &mut Vec<ObligationSummary>) {
    summaries.push(ObligationSummary {
        id: expression.id().to_string(),
        kind: expression.kind().to_string(),
        detail: match expression {
            ObligationExpr::Evidence { predicate, .. } => format!("native predicate {predicate}"),
            ObligationExpr::AnyOf { alternatives, .. } => format!(
                "requires one complete alternative: {}",
                alternatives
                    .iter()
                    .map(|alternative| alternative.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ObligationExpr::Applicable { condition, .. } => {
                format!("applies only when native applicability {condition} is observed true")
            }
            ObligationExpr::Temporal { check, .. } => {
                format!("bounded temporal ordering check {check}")
            }
            ObligationExpr::AllOf { obligations, .. } => {
                format!("requires all {} obligations", obligations.len())
            }
        },
    });
    match expression {
        ObligationExpr::AllOf { obligations, .. } => {
            for child in obligations {
                collect_summaries(child, summaries);
            }
        }
        ObligationExpr::AnyOf { alternatives, .. } => {
            for alternative in alternatives {
                for child in &alternative.obligations {
                    collect_summaries(child, summaries);
                }
            }
        }
        ObligationExpr::Applicable { then, .. } => collect_summaries(then, summaries),
        ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => {}
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Satisfied,
    Unsatisfied,
    Unknown,
    Unsupported,
}

impl ObservationState {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Unsatisfied => "unsatisfied",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceObservation {
    pub predicate: String,
    pub requirement_id: String,
    pub revision_blake3: String,
    pub subject_blake3: String,
    pub context_blake3: String,
    pub policy_blake3: String,
    pub observation_id: String,
    pub state: ObservationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressObservation {
    pub fact: String,
    pub observed: bool,
    pub source: String,
}

/// Recorded but never consulted as evidence. Kept so an explanation can show
/// that an agent claim was seen and rejected as a substitute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentClaims {
    pub success_statement: Option<String>,
    pub checked_task_boxes: u64,
    pub activity_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExceptionSnapshot {
    pub path: String,
    pub state: PolicyExceptionState,
    pub validity_epoch: u64,
    pub control_evidence: Vec<EvidenceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedEvent {
    pub event: String,
    pub epoch: u64,
    pub observer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalObservation {
    pub trace_id: String,
    pub observer: String,
    pub clock: String,
    pub window_start_epoch: u64,
    pub horizon_epoch: u64,
    pub completeness: TraceCompleteness,
    pub events: Vec<ObservedEvent>,
    pub observation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    Active,
    Delivered,
    Blocked,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalObservation {
    pub state: TerminalState,
    pub reason: String,
    pub observation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationRequest {
    pub binding: ObligationBinding,
    pub subject_blake3: String,
    pub context_blake3: String,
    pub selected_alternatives: BTreeMap<String, String>,
    pub evidence: Vec<EvidenceObservation>,
    pub progress: Vec<ProgressObservation>,
    pub agent_claims: Option<AgentClaims>,
    pub exceptions: Vec<ExceptionSnapshot>,
    pub temporal: Option<TemporalObservation>,
    pub terminal: Option<TerminalObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Satisfied,
    NotApplicable,
    Unsatisfied,
    Unknown,
    Unsupported,
    ExceptionAccepted,
}

impl NodeState {
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::NotApplicable => "not_applicable",
            Self::Unsatisfied => "unsatisfied",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
            Self::ExceptionAccepted => "exception_accepted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalCondition {
    pub node: String,
    pub state: String,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExceptionEvaluation {
    pub path: String,
    pub exception_id: String,
    pub request_revision: u64,
    pub applicability: String,
    pub control: String,
    pub control_state: String,
    pub applied: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationNodeExplanation {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub detail: String,
    pub evidence: Vec<String>,
    pub original: Option<OriginalCondition>,
    pub exception: Option<ExceptionEvaluation>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingObligation {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub reason: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreconditionExplanation {
    pub id: String,
    pub action: String,
    pub state: String,
    pub detail: String,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedObservationExplanation {
    pub id: String,
    pub command_class: String,
    pub predicate: String,
    pub state: String,
    pub observation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressExplanation {
    pub id: String,
    pub observed: bool,
    pub source: String,
    pub effect: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalExplanation {
    pub id: String,
    pub trace_id: String,
    pub observer: String,
    pub clock: String,
    pub window_start_epoch: u64,
    pub horizon_epoch: u64,
    pub completeness: String,
    pub first_event: String,
    pub second_event: String,
    pub first_epoch: Option<u64>,
    pub second_epoch: Option<u64>,
    pub state: String,
    pub detail: String,
    pub non_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowObligationReport {
    pub schema: String,
    pub workflow_id: String,
    pub workflow_version: u32,
    pub definition_id: String,
    pub binding: ObligationBinding,
    pub subject_blake3: String,
    pub context_blake3: String,
    pub outcome: String,
    pub proved: bool,
    pub successful_delivery: bool,
    pub selected_alternatives: BTreeMap<String, String>,
    pub completion: Vec<ObligationNodeExplanation>,
    pub action_preconditions: Vec<PreconditionExplanation>,
    pub expected_observations: Vec<ExpectedObservationExplanation>,
    pub progress: Vec<ProgressExplanation>,
    pub exceptions: Vec<ExceptionEvaluation>,
    pub temporal: Option<TemporalExplanation>,
    pub satisfied: Vec<String>,
    pub remaining: Vec<RemainingObligation>,
    pub ignored_claims: Vec<String>,
    pub reevaluation_triggers: BTreeSet<String>,
    pub trust_dependencies: BTreeSet<String>,
    pub predicate_blake3: String,
    pub input_blake3: String,
    pub report_blake3: String,
    pub non_claims: Vec<String>,
    pub boundary: String,
}

struct EvidenceView<'a> {
    observations: &'a [EvidenceObservation],
    subject_blake3: &'a str,
    context_blake3: &'a str,
    policy_blake3: &'a str,
    predicates: &'a BTreeMap<String, NativeEvidencePredicate>,
}

#[derive(Clone)]
struct NodeOutcome {
    state: NodeState,
    detail: String,
    evidence: Vec<String>,
    original: Option<OriginalCondition>,
    exception: Option<ExceptionEvaluation>,
    children: Vec<String>,
}

impl NodeOutcome {
    fn new(state: NodeState, detail: impl Into<String>) -> Self {
        Self {
            state,
            detail: detail.into(),
            evidence: Vec::new(),
            original: None,
            exception: None,
            children: Vec::new(),
        }
    }
}

struct Evaluation<'a> {
    admitted: &'a AdmittedWorkflowObligation,
    request: &'a ObligationRequest,
    evidence: EvidenceView<'a>,
    exceptions: BTreeMap<&'a str, &'a ExceptionSnapshot>,
    outcomes: BTreeMap<String, NodeOutcome>,
}

// r[impl cairn.workflow_obligations.evaluation]
pub fn evaluate_workflow_obligations(
    admitted: &AdmittedWorkflowObligation,
    policy: &WorkflowObligationPolicy,
    request: &ObligationRequest,
) -> Result<WorkflowObligationReport, WorkflowObligationError> {
    if request.binding != admitted.binding {
        return Err(WorkflowObligationError::ScopeMismatch(
            "request binding does not equal the admitted definition binding".into(),
        ));
    }
    if !digest(&request.subject_blake3) || !digest(&request.context_blake3) {
        return Err(WorkflowObligationError::InvalidInput(
            "subject and context revision identities are required".into(),
        ));
    }
    validate_selection(
        &admitted.definition.completion,
        &request.selected_alternatives,
    )?;
    let mut exceptions = BTreeMap::new();
    for snapshot in &request.exceptions {
        if exceptions
            .insert(snapshot.path.as_str(), snapshot)
            .is_some()
        {
            return Err(WorkflowObligationError::InvalidInput(format!(
                "duplicate exception snapshot for path {}",
                snapshot.path
            )));
        }
    }
    let mut evaluation = Evaluation {
        admitted,
        request,
        evidence: EvidenceView {
            observations: &request.evidence,
            subject_blake3: &request.subject_blake3,
            context_blake3: &request.context_blake3,
            policy_blake3: &admitted.binding.workflow_policy_blake3,
            predicates: &admitted.evidence_predicates,
        },
        exceptions,
        outcomes: BTreeMap::new(),
    };
    let root = evaluation.evaluate_expr(&admitted.definition.completion)?;
    let mut completion = Vec::new();
    collect_explanations(
        &mut evaluation.outcomes,
        &admitted.definition.completion,
        &mut completion,
    );

    let mut remaining = Vec::new();
    let mut satisfied = Vec::new();
    for explanation in &completion {
        if matches!(
            explanation.state.as_str(),
            "satisfied" | "not_applicable" | "exception_accepted"
        ) {
            satisfied.push(explanation.id.clone());
        } else {
            remaining.push(RemainingObligation {
                id: explanation.id.clone(),
                kind: explanation.kind.clone(),
                state: explanation.state.clone(),
                reason: explanation.detail.clone(),
                evidence: explanation.evidence.clone(),
            });
        }
    }

    let (outcome, proved, successful_delivery) =
        match request.terminal.as_ref().map(|terminal| terminal.state) {
            Some(TerminalState::Blocked) => ("blocked", false, false),
            Some(TerminalState::Abandoned) => ("abandoned", false, false),
            _ => match root.state {
                NodeState::Satisfied | NodeState::NotApplicable => ("proved", true, true),
                NodeState::ExceptionAccepted => ("exception_accepted", false, true),
                NodeState::Unsatisfied => ("unsatisfied", false, false),
                NodeState::Unknown => ("unknown", false, false),
                NodeState::Unsupported => ("unsupported", false, false),
            },
        };

    let mut preconditions = Vec::new();
    for precondition in &admitted.definition.action_preconditions {
        let mut state = "ready";
        let mut details = Vec::new();
        for predicate in &precondition.required_predicates {
            let (predicate_state, detail) = evaluation.evidence_state(predicate);
            details.push(format!("{predicate}: {predicate_state} ({detail})"));
            if predicate_state != "satisfied" && state == "ready" {
                state = if predicate_state == "unknown" {
                    "unknown"
                } else {
                    "unsatisfied"
                };
            }
        }
        if state != "ready" {
            remaining.push(RemainingObligation {
                id: precondition.id.clone(),
                kind: "action_precondition".to_string(),
                state: state.to_string(),
                reason: details.join("; "),
                evidence: precondition.required_predicates.clone(),
            });
        }
        preconditions.push(PreconditionExplanation {
            id: precondition.id.clone(),
            action: precondition.action.clone(),
            state: state.to_string(),
            detail: details.join("; "),
            authority: precondition.authority.clone(),
        });
    }

    let expected_observations = admitted
        .definition
        .expected_observations
        .iter()
        .map(|expected| {
            let observation = request
                .evidence
                .iter()
                .find(|observation| observation.predicate == expected.predicate)
                .map(|observation| observation.observation_id.clone());
            ExpectedObservationExplanation {
                id: expected.id.clone(),
                command_class: expected.command_class.clone(),
                predicate: expected.predicate.clone(),
                state: if observation.is_some() {
                    "supplied".to_string()
                } else {
                    "missing".to_string()
                },
                observation,
            }
        })
        .collect::<Vec<_>>();

    let progress = admitted
        .definition
        .progress_facts
        .iter()
        .map(|fact| {
            let observed = request
                .progress
                .iter()
                .find(|observation| observation.fact == fact.id);
            ProgressExplanation {
                id: fact.id.clone(),
                observed: observed.is_some_and(|observation| observation.observed),
                source: observed.map_or_else(String::new, |observation| observation.source.clone()),
                effect: "none_on_completion".to_string(),
            }
        })
        .collect::<Vec<_>>();

    let mut implemented_exceptions = Vec::new();
    for exception in &admitted.definition.exceptions {
        if let Some(snapshot) = evaluation.exceptions.get(exception.id.as_str()).copied() {
            let (applicability, control_state, detail) =
                exception_applicability(exception, snapshot, &evaluation.evidence);
            implemented_exceptions.push(ExceptionEvaluation {
                path: exception.id.clone(),
                exception_id: exception.exception_id.clone(),
                request_revision: exception.request_revision,
                applicability,
                control: exception.control.id.clone(),
                control_state,
                applied: completion
                    .iter()
                    .any(|node| node.id == exception.node && node.state == "exception_accepted"),
                detail,
            });
        }
    }

    let temporal = admitted
        .definition
        .temporal
        .as_ref()
        .map(|extension| temporal_explanation(extension, request.temporal.as_ref()));

    let mut ignored_claims = Vec::new();
    if let Some(claims) = &request.agent_claims {
        if claims.success_statement.is_some() {
            ignored_claims.push("agent success statement is not native evidence".to_string());
        }
        if claims.checked_task_boxes > 0 {
            ignored_claims.push(
                "checked task boxes and counts are progress claims, not native evidence"
                    .to_string(),
            );
        }
        if claims.activity_count > 0 {
            ignored_claims.push("activity counts are not native evidence".to_string());
        }
    }

    let predicate = resolve_predicate(
        admitted,
        policy,
        &request.selected_alternatives,
        &request.subject_blake3,
        &request.context_blake3,
    )?;
    let mut reevaluation_triggers = BTreeSet::from([
        "subject revision change".to_string(),
        "context revision change".to_string(),
        "workflow policy or catalog change".to_string(),
        "evidence revision or observation replacement".to_string(),
        "alternative selection change".to_string(),
    ]);
    if !admitted.definition.exceptions.is_empty() {
        reevaluation_triggers
            .insert("exception revocation, expiry or request revision change".to_string());
        reevaluation_triggers.insert("compensating-control evidence change".to_string());
    }
    if admitted.definition.temporal.is_some() {
        reevaluation_triggers.insert("temporal horizon or observer change".to_string());
    }
    let trust_dependencies = BTreeSet::from([
        "native observation provenance and execution honesty".to_string(),
        "policy exception authority and approval custody".to_string(),
        "supplied clock epoch meaning".to_string(),
    ]);
    let input_blake3 = identity(
        "cairn.workflow-obligation-input.v1",
        &(&admitted.binding, request),
    )?;
    let mut report = WorkflowObligationReport {
        schema: WORKFLOW_OBLIGATION_REPORT_SCHEMA_V1.to_string(),
        workflow_id: admitted.definition.workflow_id.clone(),
        workflow_version: admitted.definition.workflow_version,
        definition_id: admitted.definition.id.clone(),
        binding: admitted.binding.clone(),
        subject_blake3: request.subject_blake3.clone(),
        context_blake3: request.context_blake3.clone(),
        outcome: outcome.to_string(),
        proved,
        successful_delivery,
        selected_alternatives: request.selected_alternatives.clone(),
        completion,
        action_preconditions: preconditions,
        expected_observations,
        progress,
        exceptions: implemented_exceptions,
        temporal,
        satisfied,
        remaining,
        ignored_claims,
        reevaluation_triggers,
        trust_dependencies,
        predicate_blake3: predicate.predicate_blake3,
        input_blake3,
        report_blake3: String::new(),
        non_claims: admitted.definition.non_claims.clone(),
        boundary: WORKFLOW_OBLIGATION_BOUNDARY.to_string(),
    };
    let report_blake3 = identity("cairn.workflow-obligation-report.v1", &report)?;
    report.report_blake3 = report_blake3;
    Ok(report)
}

fn collect_explanations(
    outcomes: &mut BTreeMap<String, NodeOutcome>,
    expression: &ObligationExpr,
    explanations: &mut Vec<ObligationNodeExplanation>,
) {
    if let Some(outcome) = outcomes.remove(expression.id()) {
        explanations.push(ObligationNodeExplanation {
            id: expression.id().to_string(),
            kind: expression.kind().to_string(),
            state: outcome.state.tag().to_string(),
            detail: outcome.detail,
            evidence: outcome.evidence,
            original: outcome.original,
            exception: outcome.exception,
            children: outcome.children,
        });
    }
    match expression {
        ObligationExpr::AllOf { obligations, .. } => {
            for child in obligations {
                collect_explanations(outcomes, child, explanations);
            }
        }
        ObligationExpr::AnyOf { alternatives, .. } => {
            for alternative in alternatives {
                for child in &alternative.obligations {
                    collect_explanations(outcomes, child, explanations);
                }
            }
        }
        ObligationExpr::Applicable { then, .. } => {
            collect_explanations(outcomes, then, explanations)
        }
        ObligationExpr::Evidence { .. } | ObligationExpr::Temporal { .. } => {}
    }
}

impl Evaluation<'_> {
    fn evaluate_expr(
        &mut self,
        expression: &ObligationExpr,
    ) -> Result<NodeOutcome, WorkflowObligationError> {
        if let Some(outcome) = self.outcomes.get(expression.id()) {
            return Ok(outcome.clone());
        }
        let mut outcome = match expression {
            ObligationExpr::Evidence { id, predicate } => {
                let (state, detail) = self.evidence_state(predicate);
                let mut outcome = NodeOutcome::new(
                    match state {
                        "satisfied" => NodeState::Satisfied,
                        "unsatisfied" => NodeState::Unsatisfied,
                        "unsupported" => NodeState::Unsupported,
                        _ => NodeState::Unknown,
                    },
                    detail,
                );
                outcome.evidence = self
                    .evidence
                    .observations
                    .iter()
                    .filter(|observation| observation.predicate == *predicate)
                    .map(|observation| observation.observation_id.clone())
                    .collect();
                if outcome.evidence.is_empty() {
                    outcome
                        .evidence
                        .push(format!("{id}: no observation supplied"));
                }
                outcome
            }
            ObligationExpr::AllOf { obligations, .. } => {
                let mut state = NodeState::Satisfied;
                let mut details = Vec::with_capacity(obligations.len());
                let mut evidence = Vec::new();
                let mut children = Vec::with_capacity(obligations.len());
                for child in obligations {
                    let child_outcome = self.evaluate_expr(child)?;
                    children.push(child.id().to_string());
                    details.push(format!("{}: {}", child.id(), child_outcome.state.tag()));
                    evidence.extend(child_outcome.evidence.iter().cloned());
                    state = combine_all(state, child_outcome.state);
                }
                let mut outcome = NodeOutcome::new(state, details.join("; "));
                outcome.evidence = evidence;
                outcome.children = children;
                outcome
            }
            ObligationExpr::AnyOf { id, alternatives } => {
                let Some(selected) = self.request.selected_alternatives.get(id) else {
                    let mut outcome = NodeOutcome::new(
                        NodeState::Unknown,
                        format!("any_of {id} has no selected alternative"),
                    );
                    outcome.children = alternatives
                        .iter()
                        .map(|alternative| alternative.id.clone())
                        .collect();
                    self.outcomes
                        .insert(expression.id().to_string(), outcome.clone());
                    return Ok(outcome);
                };
                let Some(alternative) = alternatives.iter().find(|value| value.id == *selected)
                else {
                    let mut outcome = NodeOutcome::new(
                        NodeState::Unsupported,
                        format!("any_of {id} selects unknown alternative {selected}"),
                    );
                    outcome.children = alternatives
                        .iter()
                        .map(|alternative| alternative.id.clone())
                        .collect();
                    self.outcomes
                        .insert(expression.id().to_string(), outcome.clone());
                    return Ok(outcome);
                };
                let mut state = NodeState::Satisfied;
                let mut details = Vec::with_capacity(alternative.obligations.len());
                let mut evidence = Vec::new();
                let mut children = Vec::with_capacity(alternative.obligations.len());
                for child in &alternative.obligations {
                    let child_outcome = self.evaluate_expr(child)?;
                    children.push(child.id().to_string());
                    details.push(format!("{}: {}", child.id(), child_outcome.state.tag()));
                    evidence.extend(child_outcome.evidence.iter().cloned());
                    state = combine_all(state, child_outcome.state);
                }
                let mut outcome = NodeOutcome::new(
                    state,
                    format!("selected {selected}; {}", details.join("; ")),
                );
                outcome.evidence = evidence;
                outcome.children = children;
                outcome
            }
            ObligationExpr::Applicable {
                id,
                condition,
                then,
            } => {
                let (condition_state, detail) = self.evidence_state(condition);
                match condition_state {
                    "satisfied" => {
                        let mut outcome = self.evaluate_expr(then)?;
                        outcome.detail = format!(
                            "applicability {condition} observed true; {}",
                            outcome.detail
                        );
                        outcome
                    }
                    "unsatisfied" => NodeOutcome::new(
                        NodeState::NotApplicable,
                        format!("applicability {condition} observed false: {detail}"),
                    ),
                    "unsupported" => NodeOutcome::new(
                        NodeState::Unsupported,
                        format!("applicability {condition} is unsupported: {detail}"),
                    ),
                    _ => NodeOutcome::new(
                        NodeState::Unknown,
                        format!(
                            "conditional obligation {id} has no admitted applicability observation for {condition}: {detail}"
                        ),
                    ),
                }
            }
            ObligationExpr::Temporal { id, check } => match &self.admitted.definition.temporal {
                Some(extension) if extension.id == *check => {
                    let explanation =
                        temporal_explanation(extension, self.request.temporal.as_ref());
                    let state = match explanation.state.as_str() {
                        "satisfied" => NodeState::Satisfied,
                        "unsatisfied" => NodeState::Unsatisfied,
                        "unsupported" => NodeState::Unsupported,
                        _ => NodeState::Unknown,
                    };
                    NodeOutcome::new(state, explanation.detail)
                }
                Some(_) => NodeOutcome::new(
                    NodeState::Unsupported,
                    format!("temporal node {id} references an undeclared extension {check}"),
                ),
                None => NodeOutcome::new(
                    NodeState::Unsupported,
                    format!("temporal node {id} has no declared extension"),
                ),
            },
        };
        let exception = self
            .admitted
            .definition
            .exceptions
            .iter()
            .find_map(|exception| {
                if exception.node != expression.id() {
                    return None;
                }
                self.exceptions
                    .get(exception.id.as_str())
                    .copied()
                    .map(|snapshot| (exception, snapshot))
            });
        if let (true, Some((exception, snapshot))) =
            (outcome.state != NodeState::Satisfied, exception)
        {
            let (applicability, control_state, detail) =
                exception_applicability(exception, snapshot, &self.evidence);
            let mut evaluation = ExceptionEvaluation {
                path: exception.id.clone(),
                exception_id: exception.exception_id.clone(),
                request_revision: exception.request_revision,
                applicability: applicability.clone(),
                control: exception.control.id.clone(),
                control_state: control_state.clone(),
                applied: false,
                detail: detail.clone(),
            };
            match (
                outcome.state,
                applicability.as_str(),
                control_state.as_str(),
            ) {
                (NodeState::Unsatisfied, "applicable", "satisfied") => {
                    outcome.original = Some(OriginalCondition {
                        node: expression.id().to_string(),
                        state: outcome.state.tag().to_string(),
                        basis: outcome.detail.clone(),
                    });
                    evaluation.applied = true;
                    outcome.state = NodeState::ExceptionAccepted;
                    outcome.detail = format!(
                            "exception {} accepted with control {}; original condition remains unsatisfied and unproved",
                            exception.id, exception.control.id
                        );
                }
                (NodeState::Unknown, _, _) => {
                    evaluation.detail = format!(
                        "{}; exception path is not applied to an unknown condition",
                        evaluation.detail
                    );
                }
                _ => {}
            }
            outcome.exception = Some(evaluation);
        }
        self.outcomes
            .insert(expression.id().to_string(), outcome.clone());
        Ok(outcome)
    }

    fn evidence_state(&self, predicate: &str) -> (&'static str, String) {
        let Some(known) = self.evidence.predicates.get(predicate) else {
            return (
                "unsupported",
                format!("predicate {predicate} is not admitted"),
            );
        };
        let mut matching = self
            .evidence
            .observations
            .iter()
            .filter(|observation| observation.predicate == predicate);
        let Some(observation) = matching.next() else {
            return (
                "unknown",
                format!("no observation supplied for {}", known.requirement_id),
            );
        };
        if let Some(duplicate) = matching.next() {
            return (
                "unknown",
                format!(
                    "conflicting duplicate observations {} and {} for {}",
                    observation.observation_id, duplicate.observation_id, known.requirement_id
                ),
            );
        }
        if observation.requirement_id != known.requirement_id
            || observation.revision_blake3 != known.accepted_revision_blake3
            || observation.subject_blake3 != self.evidence.subject_blake3
            || observation.context_blake3 != self.evidence.context_blake3
            || observation.policy_blake3 != self.evidence.policy_blake3
            || observation.observation_id.trim().is_empty()
        {
            return (
                "unknown",
                format!(
                    "observation {} is stale or out of scope for native requirement {}",
                    observation.observation_id, known.requirement_id
                ),
            );
        }
        match observation.state {
            ObservationState::Satisfied => (
                "satisfied",
                format!(
                    "{} at revision {}",
                    known.requirement_id, known.accepted_revision_blake3
                ),
            ),
            ObservationState::Unsatisfied => (
                "unsatisfied",
                format!("{} reports unsatisfied", known.requirement_id),
            ),
            ObservationState::Unknown => (
                "unknown",
                format!("{} remains unknown", known.requirement_id),
            ),
            ObservationState::Unsupported => (
                "unsupported",
                format!("{} is unsupported", known.requirement_id),
            ),
        }
    }
}

fn combine_all(current: NodeState, next: NodeState) -> NodeState {
    if current == NodeState::Unsatisfied || next == NodeState::Unsatisfied {
        NodeState::Unsatisfied
    } else if current == NodeState::Unsupported || next == NodeState::Unsupported {
        NodeState::Unsupported
    } else if current == NodeState::Unknown || next == NodeState::Unknown {
        NodeState::Unknown
    } else if current == NodeState::ExceptionAccepted || next == NodeState::ExceptionAccepted {
        NodeState::ExceptionAccepted
    } else {
        NodeState::Satisfied
    }
}

fn exception_applicability(
    exception: &ExceptionPath,
    snapshot: &ExceptionSnapshot,
    evidence: &EvidenceView<'_>,
) -> (String, String, String) {
    let control = evidence_for(
        evidence,
        &snapshot.control_evidence,
        &exception.control.evidence_predicate,
    );
    let control_state = control.0.to_string();
    let control_detail = control.1;
    let state = &snapshot.state;
    if state.exception_id != exception.exception_id {
        return (
            "inapplicable".to_string(),
            control_state,
            "exception identity does not match the declared path".to_string(),
        );
    }
    if state.request_revision != exception.request_revision {
        return (
            "inapplicable".to_string(),
            control_state,
            format!(
                "exception request revision {} is stale for revision {}",
                state.request_revision, exception.request_revision
            ),
        );
    }
    if state.request.policy_identity != exception.policy_identity {
        return (
            "inapplicable".to_string(),
            control_state,
            "exception policy identity is stale".to_string(),
        );
    }
    if !state
        .request
        .scope
        .rule_ids
        .iter()
        .any(|rule| rule == &exception.scope_rule_id)
    {
        return (
            "inapplicable".to_string(),
            control_state,
            "exception scope does not cover the declared rule".to_string(),
        );
    }
    if !state
        .request
        .compensating_controls
        .iter()
        .any(|control| control == &exception.control.id)
    {
        return (
            "inapplicable".to_string(),
            control_state,
            "exception request no longer declares the required compensating control".to_string(),
        );
    }
    let applicability = match classify_applicability(state) {
        Ok(value) => value,
        Err(error) => {
            return (
                "unknown".to_string(),
                control_state,
                format!("native exception applicability is unavailable: {error}"),
            );
        }
    };
    if applicability != "applicable" {
        return (
            applicability,
            control_state,
            "native exception is not currently applicable".to_string(),
        );
    }
    let finite_valid_until = state
        .observations
        .values()
        .map(|observation| observation.finite_valid_until)
        .max()
        .unwrap_or(0);
    if snapshot.validity_epoch >= finite_valid_until {
        return (
            "expired".to_string(),
            control_state,
            format!(
                "exception validity ended at epoch {finite_valid_until} before supplied epoch {}",
                snapshot.validity_epoch
            ),
        );
    }
    if control_state != "satisfied" {
        let detail = format!("compensating control evidence is {control_state}: {control_detail}");
        return ("applicable".to_string(), control_state, detail);
    }
    (
        "applicable".to_string(),
        control_state,
        "current applicability and compensating control evidence are present".to_string(),
    )
}

/// Resolve the compensating-control predicate from the exception snapshot's
/// declared observations. Control evidence is carried by the snapshot; a
/// general evidence observation cannot silently substitute for it.
fn evidence_for(
    evidence: &EvidenceView<'_>,
    declared: &[EvidenceObservation],
    predicate: &str,
) -> (&'static str, String) {
    let Some(known) = evidence.predicates.get(predicate) else {
        return (
            "unsupported",
            format!("predicate {predicate} is not admitted"),
        );
    };
    let mut matching = declared
        .iter()
        .filter(|observation| observation.predicate == predicate);
    let Some(observation) = matching.next() else {
        return (
            "unknown",
            format!("no observation supplied for {}", known.requirement_id),
        );
    };
    if matching.next().is_some() {
        return ("unknown", "conflicting duplicate observations".to_string());
    }
    if observation.requirement_id != known.requirement_id
        || observation.revision_blake3 != known.accepted_revision_blake3
        || observation.subject_blake3 != evidence.subject_blake3
        || observation.context_blake3 != evidence.context_blake3
        || observation.policy_blake3 != evidence.policy_blake3
        || observation.observation_id.trim().is_empty()
    {
        return (
            "unknown",
            "compensating-control observation is stale or out of scope".to_string(),
        );
    }
    match observation.state {
        ObservationState::Satisfied => ("satisfied", String::new()),
        ObservationState::Unsatisfied => ("unsatisfied", String::new()),
        ObservationState::Unknown => ("unknown", String::new()),
        ObservationState::Unsupported => ("unsupported", String::new()),
    }
}

fn temporal_explanation(
    extension: &TemporalExtension,
    observation: Option<&TemporalObservation>,
) -> TemporalExplanation {
    let mut explanation = TemporalExplanation {
        id: extension.id.clone(),
        trace_id: extension.trace_id.clone(),
        observer: extension.observer.clone(),
        clock: extension.clock.clone(),
        window_start_epoch: 0,
        horizon_epoch: 0,
        completeness: extension.completeness.tag().to_string(),
        first_event: extension.first_event.clone(),
        second_event: extension.second_event.clone(),
        first_epoch: None,
        second_epoch: None,
        state: "unknown".to_string(),
        detail: String::new(),
        non_claim: "a finite trace ordering result implies no unbounded safety or liveness"
            .to_string(),
    };
    let Some(observation) = observation else {
        explanation.detail =
            "no temporal observation supplied; ordering remains unknown".to_string();
        return explanation;
    };
    explanation.window_start_epoch = observation.window_start_epoch;
    explanation.horizon_epoch = observation.horizon_epoch;
    explanation.completeness = observation.completeness.tag().to_string();
    if observation.trace_id != extension.trace_id
        || observation.observer != extension.observer
        || observation.clock != extension.clock
        || observation.observation_id.trim().is_empty()
    {
        explanation.detail =
            "trace identity, observer or clock basis differs from the declared interval; ordering remains unknown"
                .to_string();
        return explanation;
    }
    if observation.completeness != extension.completeness {
        explanation.detail = format!(
            "trace completeness {} is weaker than the declared {}",
            observation.completeness.tag(),
            extension.completeness.tag()
        );
        return explanation;
    }
    if observation.horizon_epoch <= observation.window_start_epoch
        || observation.horizon_epoch - observation.window_start_epoch != extension.max_span_epochs
    {
        explanation.detail = format!(
            "supplied horizon does not match the declared finite span of {} epochs",
            extension.max_span_epochs
        );
        return explanation;
    }
    let mut first = None;
    let mut second = None;
    let mut untrusted = None;
    for event in &observation.events {
        if event.event != extension.first_event && event.event != extension.second_event {
            continue;
        }
        if event.observer != extension.observer {
            untrusted = Some(event.event.clone());
            continue;
        }
        if event.event == extension.first_event {
            first = Some(event.epoch);
        } else {
            second = Some(event.epoch);
        }
    }
    explanation.first_epoch = first;
    explanation.second_epoch = second;
    if let Some(event) = untrusted {
        explanation.detail = format!(
            "event {event} was not observed by the declared observer {}; ordering remains unknown",
            extension.observer
        );
        return explanation;
    }
    let (Some(first_epoch), Some(second_epoch)) = (first, second) else {
        if observation.completeness == TraceCompleteness::Complete {
            explanation.state = "unsatisfied".to_string();
            explanation.detail =
                "a declared complete interval lacks one of the named events".to_string();
        } else {
            explanation.detail =
                "the declared interval is incomplete; ordering remains unknown".to_string();
        }
        return explanation;
    };
    if first_epoch > observation.horizon_epoch || second_epoch > observation.horizon_epoch {
        explanation.detail = "an event falls outside the declared horizon".to_string();
        return explanation;
    }
    if first_epoch < second_epoch {
        explanation.state = "satisfied".to_string();
        explanation.detail = format!(
            "{} at epoch {first_epoch} precedes {} at epoch {second_epoch} within the declared interval",
            extension.first_event, extension.second_event
        );
    } else {
        explanation.state = "unsatisfied".to_string();
        explanation.detail = format!(
            "{} at epoch {first_epoch} does not precede {} at epoch {second_epoch}",
            extension.first_event, extension.second_event
        );
    }
    explanation
}

// ---------------------------------------------------------------------------
// Generated workflow adapter integration
// ---------------------------------------------------------------------------

/// Render the admitted catalog with obligation guidance appended for workflows
/// that have an admitted primary definition. Legacy rendering is unchanged when
/// no definition matches; descriptor identity binds the obligation binding so a
/// plan cannot mix text from a different obligation definition.
// r[impl cairn.workflow_obligations.legacy_compatibility]
pub fn render_with_workflow_obligations(
    catalog: &AdmittedGeneratedWorkflowCatalog,
    obligations: &AdmittedWorkflowObligationCatalog,
    adapter_id: &str,
) -> Result<
    Vec<super::generated_agent_workflow::RenderedGeneratedWorkflowFile>,
    Vec<GeneratedWorkflowIssue>,
> {
    let adapter =
        super::generated_agent_workflow::find_generated_workflow_adapter(catalog, adapter_id)
            .map_err(|issue| vec![issue])?;
    let presentation = presentation_for_adapter(&adapter.format);
    let mut rendered_files: BTreeMap<
        String,
        super::generated_agent_workflow::RenderedGeneratedWorkflowFile,
    > = super::generated_agent_workflow::render_generated_agent_workflows(catalog, adapter_id)?
        .into_iter()
        .map(|file| (file.workflow_id.clone(), file))
        .collect();
    let mut files = Vec::with_capacity(catalog.policy.descriptors.len());
    for descriptor in &catalog.policy.descriptors {
        let mut rendered = rendered_files.remove(&descriptor.id).ok_or_else(|| {
            vec![super::generated_agent_workflow::GeneratedWorkflowIssue {
                code: super::generated_agent_workflow::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER
                    .to_string(),
                field_path: format!("adapters.{}", adapter.id),
                message: "adapter did not render the admitted workflow".to_string(),
            }]
        })?;
        if let Some(admitted) = find_workflow_obligation(obligations, &descriptor.id) {
            if admitted.binding.catalog_identity != catalog.catalog_identity
                || admitted.binding.workflow_policy_blake3 != obligations.workflow_policy_blake3
            {
                return Err(vec![
                    super::generated_agent_workflow::GeneratedWorkflowIssue {
                        code: WORKFLOW_OBLIGATION_ISSUE_BINDING.to_string(),
                        field_path: format!("obligations.{}", descriptor.id),
                        message: "obligation binding does not match the rendering catalog"
                            .to_string(),
                    },
                ]);
            }
            let guidance = obligation_guidance(admitted, &obligations.policy, &BTreeMap::new())
                .and_then(|guidance| render_guidance(&guidance, &presentation))
                .map_err(|error| {
                    vec![super::generated_agent_workflow::GeneratedWorkflowIssue {
                        code: WORKFLOW_OBLIGATION_ISSUE_EXPRESSION.to_string(),
                        field_path: format!("obligations.{}", descriptor.id),
                        message: error.to_string(),
                    }]
                })?;
            rendered.contents.push('\n');
            rendered.contents.push_str(&guidance);
            rendered.content_blake3 = super::evidence::blake3_hex(rendered.contents.as_bytes());
            rendered.descriptor_identity =
                super::evidence::canonical_json_hash(&serde_json::json!({
                    "descriptor": descriptor,
                    "generator_version": catalog.policy.generator_version,
                    "obligation_binding": admitted.binding,
                }));
        }
        if rendered.contents.len() > catalog.policy.bounds.max_file_bytes {
            return Err(vec![
                super::generated_agent_workflow::GeneratedWorkflowIssue {
                    code: super::generated_agent_workflow::GENERATED_AGENT_WORKFLOW_ISSUE_ADAPTER
                        .to_string(),
                    field_path: format!("adapters.{}.file_bytes", adapter.id),
                    message: "adapter output with obligations exceeds the policy file bound"
                        .to_string(),
                },
            ]);
        }
        files.push(rendered);
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

#[cfg(test)]
mod tests;

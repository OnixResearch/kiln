pub const WORKSET_STATE_SCHEMA_V1: &str = "cairn.workset-state.v1";
pub const WORKSET_TRANSITION_PLAN_SCHEMA_V1: &str = "cairn.workset-transition-plan.v1";
pub const WORKSET_OPEN_PLAN_SCHEMA_V1: &str = "cairn.workset-open-plan.v1";
pub const WORKSET_LOCAL_RECEIPT_SCHEMA_V1: &str = "cairn.workset-local-receipt.v1";
pub const WORKSET_STATE_LOGICAL_PATH: &str = "cairn/worksets/state.json";
pub const WORKSET_EVIDENCE_CLASS_LOCAL_ONLY: &str = "local-only";
pub const WORKSET_NON_CLAIM_MEMBER_EFFECTS: &str =
    "workset plans do not delete, change, close, or unregister member repositories";
pub const WORKSET_NON_CLAIM_LIFECYCLE_AUTHORITY: &str =
    "worksets do not prove lifecycle authority, dependency satisfaction, readiness, or release eligibility";
pub const WORKSET_NON_CLAIM_EDITOR_STATE: &str =
    "an opener plan or process observation does not prove editor state or saved work";
pub const MAX_WORKSET_ID_BYTES: usize = 64;
pub const MAX_WORKSET_MEMBER_LABEL_BYTES: usize = 128;
pub const MAX_OPENER_PROFILE_ID_BYTES: usize = 64;
pub const MAX_WORKSET_MEMBERS: usize = 32;
pub const MAX_WORKSETS: usize = 128;
pub const MAX_OPENER_PROFILES: usize = 16;
pub const MAX_EXECUTABLE_BYTES: usize = 128;
pub const MAX_FIXED_ARGS: usize = 16;
pub const MAX_ARG_BYTES: usize = 512;
pub const MAX_DISPLAY_PATH_BYTES: usize = 4096;
pub const BLAKE3_HEX_BYTES: usize = 64;

const SHELL_SYNTAX_CHARS: &str = "$`'\";|&<>()\n\r";
const SHELL_EXECUTABLES: &[&str] = &[
    "bash",
    "cmd",
    "dash",
    "fish",
    "powershell",
    "pwsh",
    "sh",
    "zsh",
];

// r[impl cairn.lifecycle_stores.worksets]
// r[impl cairn.lifecycle_stores.receipts]

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct WorksetId(String);

impl WorksetId {
    pub fn parse(value: impl Into<String>) -> Result<Self, WorksetError> {
        let value = value.into();
        validate_identifier(&value, "workset_id", MAX_WORKSET_ID_BYTES)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct WorksetMemberLabel(String);

impl WorksetMemberLabel {
    pub fn parse(value: impl Into<String>) -> Result<Self, WorksetError> {
        let value = value.into();
        validate_identifier(&value, "member_label", MAX_WORKSET_MEMBER_LABEL_BYTES)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct WorksetOpenerProfileId(String);

impl WorksetOpenerProfileId {
    pub fn parse(value: impl Into<String>) -> Result<Self, WorksetError> {
        let value = value.into();
        validate_identifier(&value, "opener_profile_id", MAX_OPENER_PROFILE_ID_BYTES)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetMember {
    pub label: WorksetMemberLabel,
    pub display_path: String,
}

impl WorksetMember {
    pub fn new(
        label: impl Into<String>,
        display_path: impl Into<String>,
    ) -> Result<Self, WorksetError> {
        let member = Self {
            label: WorksetMemberLabel::parse(label)?,
            display_path: display_path.into(),
        };
        validate_display_path(&member.display_path, member.label.as_str())?;
        Ok(member)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Workset {
    pub id: WorksetId,
    pub members: Vec<WorksetMember>,
    pub primary_member: WorksetMemberLabel,
    pub opener_profile: WorksetOpenerProfileId,
}

impl Workset {
    pub fn new(
        id: impl Into<String>,
        members: Vec<WorksetMember>,
        primary_member: impl Into<String>,
        opener_profile: impl Into<String>,
    ) -> Result<Self, WorksetError> {
        let workset = Self {
            id: WorksetId::parse(id)?,
            members,
            primary_member: WorksetMemberLabel::parse(primary_member)?,
            opener_profile: WorksetOpenerProfileId::parse(opener_profile)?,
        };
        validate_workset_shape(&workset)?;
        Ok(workset)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetState {
    pub schema_version: String,
    pub worksets: Vec<Workset>,
}

impl WorksetState {
    pub fn empty() -> Self {
        Self {
            schema_version: WORKSET_STATE_SCHEMA_V1.to_string(),
            worksets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetOpenerProfile {
    pub id: WorksetOpenerProfileId,
    pub executable: String,
    pub fixed_args: Vec<String>,
}

impl WorksetOpenerProfile {
    pub fn new(
        id: impl Into<String>,
        executable: impl Into<String>,
        fixed_args: Vec<String>,
    ) -> Result<Self, WorksetError> {
        let profile = Self {
            id: WorksetOpenerProfileId::parse(id)?,
            executable: executable.into(),
            fixed_args,
        };
        validate_opener_profile(&profile)?;
        Ok(profile)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetOpenerPolicy {
    pub policy_hash: String,
    pub profiles: Vec<WorksetOpenerProfile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorksetExecutionMode {
    Preview,
    Execute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorksetTransitionOperation {
    Create,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorksetReceiptOperation {
    Create,
    Remove,
    Open,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorksetArtifactRole {
    LocalState,
    PrimaryMember,
    Member,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorksetArtifactEffect {
    ReplaceLocalState,
    PreserveMember,
    OpenReadOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetArtifactPlan {
    pub label: String,
    pub role: WorksetArtifactRole,
    pub effect: WorksetArtifactEffect,
    pub logical_path: Option<String>,
    pub display_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorksetPlanAction {
    ReplaceLocalState {
        logical_path: String,
        expected_prior_state_identity: String,
        next_state_identity: String,
    },
    LaunchOpener {
        profile_id: WorksetOpenerProfileId,
        argv: Vec<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetLocalMemberFact {
    pub label: WorksetMemberLabel,
    pub display_path: String,
    pub primary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetLocalReceiptFacts {
    pub schema_version: String,
    pub evidence_class: String,
    pub operation: WorksetReceiptOperation,
    pub execution: WorksetExecutionMode,
    pub workset_id: WorksetId,
    pub policy_hash: String,
    pub state_identity_before: String,
    pub state_identity_after: String,
    pub portable_state_identity_before: String,
    pub portable_state_identity_after: String,
    pub workset_identity: String,
    pub portable_workset_identity: String,
    pub opener_profile_id: WorksetOpenerProfileId,
    pub opener_profile_identity: String,
    pub ordered_members: Vec<WorksetLocalMemberFact>,
    pub actions: Vec<WorksetPlanAction>,
    pub artifacts: Vec<WorksetArtifactPlan>,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorksetCreateRequest {
    pub workset: Workset,
    pub expected_prior_state_identity: String,
    pub expected_prior_portable_state_identity: String,
    pub execution: WorksetExecutionMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorksetRemoveRequest {
    pub workset_id: WorksetId,
    pub expected_prior_state_identity: String,
    pub expected_prior_portable_state_identity: String,
    pub execution: WorksetExecutionMode,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetTransitionPlan {
    pub schema_version: String,
    pub operation: WorksetTransitionOperation,
    pub execution: WorksetExecutionMode,
    pub workset_id: WorksetId,
    pub subject_workset: Workset,
    pub expected_prior_state_identity: String,
    pub expected_prior_portable_state_identity: String,
    pub next_state_identity: String,
    pub next_portable_state_identity: String,
    pub next_state: WorksetState,
    pub actions: Vec<WorksetPlanAction>,
    pub artifacts: Vec<WorksetArtifactPlan>,
    pub local_receipt: WorksetLocalReceiptFacts,
    pub local_receipt_identity: String,
    pub portable_identity: String,
    pub plan_identity: String,
}

impl WorksetTransitionPlan {
    pub fn execution_authorized(&self) -> bool {
        self.execution == WorksetExecutionMode::Execute
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorksetOpenRequest {
    pub workset_id: WorksetId,
    pub opener_override: Option<WorksetOpenerProfileId>,
    pub expected_state_identity: String,
    pub expected_portable_state_identity: String,
    pub execution: WorksetExecutionMode,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum WorksetPortableArg {
    Literal(String),
    Member(WorksetMemberLabel),
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorksetOpenPlan {
    pub schema_version: String,
    pub execution: WorksetExecutionMode,
    pub workset_id: WorksetId,
    pub requested_opener: Option<WorksetOpenerProfileId>,
    pub selected_opener: WorksetOpenerProfileId,
    pub expected_state_identity: String,
    pub expected_portable_state_identity: String,
    pub expected_workset_identity: String,
    pub expected_portable_workset_identity: String,
    pub policy_hash: String,
    pub opener_profile_identity: String,
    pub argv: Vec<String>,
    pub portable_argv: Vec<WorksetPortableArg>,
    pub actions: Vec<WorksetPlanAction>,
    pub artifacts: Vec<WorksetArtifactPlan>,
    pub local_receipt: WorksetLocalReceiptFacts,
    pub local_receipt_identity: String,
    pub portable_identity: String,
    pub plan_identity: String,
}

impl WorksetOpenPlan {
    pub fn execution_authorized(&self) -> bool {
        self.execution == WorksetExecutionMode::Execute
    }
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum WorksetError {
    #[error("{field} is empty")]
    EmptyField { field: String },
    #[error("{field} contains shell syntax")]
    ShellSyntaxRejected { field: String },
    #[error("{field} is not a bounded identifier")]
    InvalidIdentifier { field: String },
    #[error("{field} has {actual} bytes, limit is {limit}")]
    ByteLimitExceeded {
        field: String,
        actual: usize,
        limit: usize,
    },
    #[error("workset has {actual} members, limit is {limit}")]
    MemberLimitExceeded { actual: usize, limit: usize },
    #[error("workset state has {actual} rows, limit is {limit}")]
    WorksetLimitExceeded { actual: usize, limit: usize },
    #[error("opener policy has {actual} profiles, limit is {limit}")]
    ProfileLimitExceeded { actual: usize, limit: usize },
    #[error("duplicate workset member label: {label}")]
    DuplicateMemberLabel { label: String },
    #[error("duplicate workset member path: {display_path}")]
    DuplicateMemberPath { display_path: String },
    #[error("primary member is not present exactly once: {label}")]
    PrimaryMemberMissing { label: String },
    #[error("duplicate workset id: {workset_id}")]
    DuplicateWorksetId { workset_id: String },
    #[error("workset rows are not ordered by id")]
    WorksetsNotOrdered,
    #[error("unknown opener profile: {profile_id}")]
    UnknownOpenerProfile { profile_id: String },
    #[error("duplicate opener profile: {profile_id}")]
    DuplicateOpenerProfile { profile_id: String },
    #[error("opener profile field contains an absolute path: {field}")]
    AbsoluteProfilePath { field: String },
    #[error("opener profile cannot use a shell executable: {executable}")]
    ShellExecutableRejected { executable: String },
    #[error("invalid BLAKE3 identity in {field}")]
    InvalidBlake3Identity { field: String },
    #[error("unsupported workset state schema: {actual}")]
    UnsupportedStateSchema { actual: String },
    #[error("workset already exists: {workset_id}")]
    WorksetAlreadyExists { workset_id: String },
    #[error("unknown workset: {workset_id}")]
    UnknownWorkset { workset_id: String },
    #[error("expected prior state identity is stale")]
    StalePriorStateIdentity,
    #[error("expected prior portable state identity is stale")]
    StalePriorPortableStateIdentity,
    #[error("plan identity does not match plan material")]
    PlanIdentityMismatch,
    #[error("reviewed plan does not match the current pure transition")]
    ReviewedPlanMismatch,
    #[error("preview plan does not authorize transition application")]
    PreviewDoesNotAuthorizeTransition,
    #[error("serialization failed: {reason}")]
    Serialization { reason: String },
}

pub fn workset_state_identity(state: &WorksetState) -> Result<String, WorksetError> {
    validate_workset_state_shape(state)?;
    hash_serializable(state)
}

pub fn workset_portable_state_identity(state: &WorksetState) -> Result<String, WorksetError> {
    validate_workset_state_shape(state)?;
    let material = PortableStateMaterial {
        schema_version: state.schema_version.clone(),
        worksets: state.worksets.iter().map(portable_workset).collect(),
    };
    hash_serializable(&material)
}

pub fn workset_identity(workset: &Workset) -> Result<String, WorksetError> {
    validate_workset_shape(workset)?;
    hash_serializable(workset)
}

pub fn workset_portable_identity(workset: &Workset) -> Result<String, WorksetError> {
    validate_workset_shape(workset)?;
    hash_serializable(&portable_workset(workset))
}

pub fn opener_profile_identity(profile: &WorksetOpenerProfile) -> Result<String, WorksetError> {
    validate_opener_profile(profile)?;
    hash_serializable(profile)
}

pub fn plan_workset_create(
    state: &WorksetState,
    policy: &WorksetOpenerPolicy,
    request: WorksetCreateRequest,
) -> Result<WorksetTransitionPlan, WorksetError> {
    validate_workset_state(state, policy)?;
    validate_workset_with_policy(&request.workset, policy)?;
    validate_expected_prior_identities(
        state,
        &request.expected_prior_state_identity,
        &request.expected_prior_portable_state_identity,
    )?;
    if state
        .worksets
        .iter()
        .any(|workset| workset.id == request.workset.id)
    {
        return Err(WorksetError::WorksetAlreadyExists {
            workset_id: request.workset.id.as_str().to_string(),
        });
    }

    let mut next_state = state.clone();
    next_state.worksets.push(request.workset.clone());
    next_state
        .worksets
        .sort_by(|left, right| left.id.cmp(&right.id));
    build_transition_plan(
        state,
        next_state,
        policy,
        request.workset,
        WorksetTransitionOperation::Create,
        request.execution,
    )
}

pub fn plan_workset_remove(
    state: &WorksetState,
    policy: &WorksetOpenerPolicy,
    request: WorksetRemoveRequest,
) -> Result<WorksetTransitionPlan, WorksetError> {
    validate_workset_state(state, policy)?;
    validate_expected_prior_identities(
        state,
        &request.expected_prior_state_identity,
        &request.expected_prior_portable_state_identity,
    )?;
    let Some(subject_workset) = state
        .worksets
        .iter()
        .find(|workset| workset.id == request.workset_id)
        .cloned()
    else {
        return Err(WorksetError::UnknownWorkset {
            workset_id: request.workset_id.as_str().to_string(),
        });
    };
    let mut next_state = state.clone();
    next_state
        .worksets
        .retain(|workset| workset.id != request.workset_id);
    build_transition_plan(
        state,
        next_state,
        policy,
        subject_workset,
        WorksetTransitionOperation::Remove,
        request.execution,
    )
}

pub fn apply_workset_transition(
    state: &WorksetState,
    policy: &WorksetOpenerPolicy,
    plan: &WorksetTransitionPlan,
) -> Result<WorksetState, WorksetError> {
    if workset_transition_plan_identity(plan)? != plan.plan_identity {
        return Err(WorksetError::PlanIdentityMismatch);
    }
    if !plan.execution_authorized() {
        return Err(WorksetError::PreviewDoesNotAuthorizeTransition);
    }
    let expected = match plan.operation {
        WorksetTransitionOperation::Create => plan_workset_create(
            state,
            policy,
            WorksetCreateRequest {
                workset: plan.subject_workset.clone(),
                expected_prior_state_identity: plan.expected_prior_state_identity.clone(),
                expected_prior_portable_state_identity: plan
                    .expected_prior_portable_state_identity
                    .clone(),
                execution: plan.execution,
            },
        )?,
        WorksetTransitionOperation::Remove => plan_workset_remove(
            state,
            policy,
            WorksetRemoveRequest {
                workset_id: plan.workset_id.clone(),
                expected_prior_state_identity: plan.expected_prior_state_identity.clone(),
                expected_prior_portable_state_identity: plan
                    .expected_prior_portable_state_identity
                    .clone(),
                execution: plan.execution,
            },
        )?,
    };
    if expected != *plan {
        return Err(WorksetError::ReviewedPlanMismatch);
    }
    Ok(plan.next_state.clone())
}

pub fn plan_workset_open(
    state: &WorksetState,
    policy: &WorksetOpenerPolicy,
    request: WorksetOpenRequest,
) -> Result<WorksetOpenPlan, WorksetError> {
    validate_workset_state(state, policy)?;
    validate_expected_prior_identities(
        state,
        &request.expected_state_identity,
        &request.expected_portable_state_identity,
    )?;
    let Some(workset) = state
        .worksets
        .iter()
        .find(|workset| workset.id == request.workset_id)
    else {
        return Err(WorksetError::UnknownWorkset {
            workset_id: request.workset_id.as_str().to_string(),
        });
    };
    let selected_opener = request
        .opener_override
        .clone()
        .unwrap_or_else(|| workset.opener_profile.clone());
    let profile = require_opener_profile(policy, &selected_opener)?;
    let opener_profile_identity = opener_profile_identity(profile)?;
    let (argv, portable_argv) = build_opener_argv(profile, workset)?;
    let artifacts = member_artifacts(workset, WorksetArtifactEffect::OpenReadOnly);
    let actions = vec![WorksetPlanAction::LaunchOpener {
        profile_id: selected_opener.clone(),
        argv: argv.clone(),
    }];
    let state_identity = workset_state_identity(state)?;
    let portable_state_identity = workset_portable_state_identity(state)?;
    let expected_workset_identity = workset_identity(workset)?;
    let expected_portable_workset_identity = workset_portable_identity(workset)?;
    let local_receipt = build_local_receipt(LocalReceiptInput {
        operation: WorksetReceiptOperation::Open,
        execution: request.execution,
        state_identity_before: &state_identity,
        state_identity_after: &state_identity,
        portable_state_identity_before: &portable_state_identity,
        portable_state_identity_after: &portable_state_identity,
        workset,
        policy,
        profile,
        actions: &actions,
        artifacts: &artifacts,
    })?;
    let local_receipt_identity = hash_serializable(&local_receipt)?;
    let portable_identity = hash_serializable(&PortableOpenPlanMaterial {
        schema_version: WORKSET_OPEN_PLAN_SCHEMA_V1.to_string(),
        execution: request.execution,
        workset: portable_workset(workset),
        requested_opener: request.opener_override.clone(),
        selected_opener: selected_opener.clone(),
        expected_portable_state_identity: portable_state_identity.clone(),
        expected_portable_workset_identity: expected_portable_workset_identity.clone(),
        policy_hash: policy.policy_hash.clone(),
        opener_profile_identity: opener_profile_identity.clone(),
        portable_argv: portable_argv.clone(),
        artifacts: portable_artifacts(&artifacts),
        non_claims: workset_non_claims(),
    })?;
    let mut plan = WorksetOpenPlan {
        schema_version: WORKSET_OPEN_PLAN_SCHEMA_V1.to_string(),
        execution: request.execution,
        workset_id: workset.id.clone(),
        requested_opener: request.opener_override,
        selected_opener,
        expected_state_identity: state_identity,
        expected_portable_state_identity: portable_state_identity,
        expected_workset_identity,
        expected_portable_workset_identity,
        policy_hash: policy.policy_hash.clone(),
        opener_profile_identity,
        argv,
        portable_argv,
        actions,
        artifacts,
        local_receipt,
        local_receipt_identity,
        portable_identity,
        plan_identity: String::new(),
    };
    plan.plan_identity = workset_open_plan_identity(&plan)?;
    Ok(plan)
}

pub fn recheck_workset_open_plan(
    state: &WorksetState,
    policy: &WorksetOpenerPolicy,
    plan: &WorksetOpenPlan,
) -> Result<(), WorksetError> {
    if workset_open_plan_identity(plan)? != plan.plan_identity {
        return Err(WorksetError::PlanIdentityMismatch);
    }
    let expected = plan_workset_open(
        state,
        policy,
        WorksetOpenRequest {
            workset_id: plan.workset_id.clone(),
            opener_override: plan.requested_opener.clone(),
            expected_state_identity: plan.expected_state_identity.clone(),
            expected_portable_state_identity: plan.expected_portable_state_identity.clone(),
            execution: plan.execution,
        },
    )?;
    if expected != *plan {
        return Err(WorksetError::ReviewedPlanMismatch);
    }
    Ok(())
}

pub fn workset_transition_plan_identity(
    plan: &WorksetTransitionPlan,
) -> Result<String, WorksetError> {
    let mut material = plan.clone();
    material.plan_identity.clear();
    hash_serializable(&material)
}

pub fn workset_open_plan_identity(plan: &WorksetOpenPlan) -> Result<String, WorksetError> {
    let mut material = plan.clone();
    material.plan_identity.clear();
    hash_serializable(&material)
}

fn build_transition_plan(
    state: &WorksetState,
    next_state: WorksetState,
    policy: &WorksetOpenerPolicy,
    subject_workset: Workset,
    operation: WorksetTransitionOperation,
    execution: WorksetExecutionMode,
) -> Result<WorksetTransitionPlan, WorksetError> {
    validate_workset_state(&next_state, policy)?;
    let expected_prior_state_identity = workset_state_identity(state)?;
    let expected_prior_portable_state_identity = workset_portable_state_identity(state)?;
    let next_state_identity = workset_state_identity(&next_state)?;
    let next_portable_state_identity = workset_portable_state_identity(&next_state)?;
    let action = WorksetPlanAction::ReplaceLocalState {
        logical_path: WORKSET_STATE_LOGICAL_PATH.to_string(),
        expected_prior_state_identity: expected_prior_state_identity.clone(),
        next_state_identity: next_state_identity.clone(),
    };
    let actions = vec![action];
    let artifacts = transition_artifacts(&subject_workset);
    let profile = require_opener_profile(policy, &subject_workset.opener_profile)?;
    let local_receipt = build_local_receipt(LocalReceiptInput {
        operation: match operation {
            WorksetTransitionOperation::Create => WorksetReceiptOperation::Create,
            WorksetTransitionOperation::Remove => WorksetReceiptOperation::Remove,
        },
        execution,
        state_identity_before: &expected_prior_state_identity,
        state_identity_after: &next_state_identity,
        portable_state_identity_before: &expected_prior_portable_state_identity,
        portable_state_identity_after: &next_portable_state_identity,
        workset: &subject_workset,
        policy,
        profile,
        actions: &actions,
        artifacts: &artifacts,
    })?;
    let local_receipt_identity = hash_serializable(&local_receipt)?;
    let opener_profile_identity = opener_profile_identity(profile)?;
    let portable_identity = hash_serializable(&PortableTransitionPlanMaterial {
        schema_version: WORKSET_TRANSITION_PLAN_SCHEMA_V1.to_string(),
        operation,
        execution,
        workset: portable_workset(&subject_workset),
        expected_prior_portable_state_identity: expected_prior_portable_state_identity.clone(),
        next_portable_state_identity: next_portable_state_identity.clone(),
        policy_hash: policy.policy_hash.clone(),
        opener_profile_identity,
        actions: vec![PortablePlanAction::ReplaceLocalState {
            logical_path: WORKSET_STATE_LOGICAL_PATH.to_string(),
            expected_prior_portable_state_identity: expected_prior_portable_state_identity.clone(),
            next_portable_state_identity: next_portable_state_identity.clone(),
        }],
        artifacts: portable_artifacts(&artifacts),
        non_claims: workset_non_claims(),
    })?;
    let mut plan = WorksetTransitionPlan {
        schema_version: WORKSET_TRANSITION_PLAN_SCHEMA_V1.to_string(),
        operation,
        execution,
        workset_id: subject_workset.id.clone(),
        subject_workset,
        expected_prior_state_identity,
        expected_prior_portable_state_identity,
        next_state_identity,
        next_portable_state_identity,
        next_state,
        actions,
        artifacts,
        local_receipt,
        local_receipt_identity,
        portable_identity,
        plan_identity: String::new(),
    };
    plan.plan_identity = workset_transition_plan_identity(&plan)?;
    Ok(plan)
}

struct LocalReceiptInput<'a> {
    operation: WorksetReceiptOperation,
    execution: WorksetExecutionMode,
    state_identity_before: &'a str,
    state_identity_after: &'a str,
    portable_state_identity_before: &'a str,
    portable_state_identity_after: &'a str,
    workset: &'a Workset,
    policy: &'a WorksetOpenerPolicy,
    profile: &'a WorksetOpenerProfile,
    actions: &'a [WorksetPlanAction],
    artifacts: &'a [WorksetArtifactPlan],
}

fn build_local_receipt(
    input: LocalReceiptInput<'_>,
) -> Result<WorksetLocalReceiptFacts, WorksetError> {
    Ok(WorksetLocalReceiptFacts {
        schema_version: WORKSET_LOCAL_RECEIPT_SCHEMA_V1.to_string(),
        evidence_class: WORKSET_EVIDENCE_CLASS_LOCAL_ONLY.to_string(),
        operation: input.operation,
        execution: input.execution,
        workset_id: input.workset.id.clone(),
        policy_hash: input.policy.policy_hash.clone(),
        state_identity_before: input.state_identity_before.to_string(),
        state_identity_after: input.state_identity_after.to_string(),
        portable_state_identity_before: input.portable_state_identity_before.to_string(),
        portable_state_identity_after: input.portable_state_identity_after.to_string(),
        workset_identity: workset_identity(input.workset)?,
        portable_workset_identity: workset_portable_identity(input.workset)?,
        opener_profile_id: input.profile.id.clone(),
        opener_profile_identity: opener_profile_identity(input.profile)?,
        ordered_members: input
            .workset
            .members
            .iter()
            .map(|member| WorksetLocalMemberFact {
                label: member.label.clone(),
                display_path: member.display_path.clone(),
                primary: member.label == input.workset.primary_member,
            })
            .collect(),
        actions: input.actions.to_vec(),
        artifacts: input.artifacts.to_vec(),
        non_claims: workset_non_claims(),
    })
}

fn validate_workset_state(
    state: &WorksetState,
    policy: &WorksetOpenerPolicy,
) -> Result<(), WorksetError> {
    validate_opener_policy(policy)?;
    validate_workset_state_shape(state)?;
    for workset in &state.worksets {
        validate_workset_with_policy(workset, policy)?;
    }
    Ok(())
}

fn validate_workset_state_shape(state: &WorksetState) -> Result<(), WorksetError> {
    if state.schema_version != WORKSET_STATE_SCHEMA_V1 {
        return Err(WorksetError::UnsupportedStateSchema {
            actual: state.schema_version.clone(),
        });
    }
    if state.worksets.len() > MAX_WORKSETS {
        return Err(WorksetError::WorksetLimitExceeded {
            actual: state.worksets.len(),
            limit: MAX_WORKSETS,
        });
    }
    let mut prior_id: Option<&WorksetId> = None;
    for workset in &state.worksets {
        validate_workset_shape(workset)?;
        if prior_id.is_some_and(|prior| prior > &workset.id) {
            return Err(WorksetError::WorksetsNotOrdered);
        }
        if prior_id == Some(&workset.id) {
            return Err(WorksetError::DuplicateWorksetId {
                workset_id: workset.id.as_str().to_string(),
            });
        }
        prior_id = Some(&workset.id);
    }
    Ok(())
}

fn validate_workset_with_policy(
    workset: &Workset,
    policy: &WorksetOpenerPolicy,
) -> Result<(), WorksetError> {
    validate_workset_shape(workset)?;
    require_opener_profile(policy, &workset.opener_profile)?;
    Ok(())
}

fn validate_workset_shape(workset: &Workset) -> Result<(), WorksetError> {
    validate_identifier(workset.id.as_str(), "workset_id", MAX_WORKSET_ID_BYTES)?;
    validate_identifier(
        workset.primary_member.as_str(),
        "primary_member",
        MAX_WORKSET_MEMBER_LABEL_BYTES,
    )?;
    validate_identifier(
        workset.opener_profile.as_str(),
        "opener_profile",
        MAX_OPENER_PROFILE_ID_BYTES,
    )?;
    if workset.members.is_empty() {
        return Err(WorksetError::EmptyField {
            field: "members".to_string(),
        });
    }
    if workset.members.len() > MAX_WORKSET_MEMBERS {
        return Err(WorksetError::MemberLimitExceeded {
            actual: workset.members.len(),
            limit: MAX_WORKSET_MEMBERS,
        });
    }
    let mut labels = std::collections::BTreeSet::new();
    let mut display_paths = std::collections::BTreeSet::new();
    for member in &workset.members {
        validate_identifier(
            member.label.as_str(),
            "member_label",
            MAX_WORKSET_MEMBER_LABEL_BYTES,
        )?;
        validate_display_path(&member.display_path, member.label.as_str())?;
        if !labels.insert(member.label.as_str()) {
            return Err(WorksetError::DuplicateMemberLabel {
                label: member.label.as_str().to_string(),
            });
        }
        if !display_paths.insert(member.display_path.as_str()) {
            return Err(WorksetError::DuplicateMemberPath {
                display_path: member.display_path.clone(),
            });
        }
    }
    let primary_count = workset
        .members
        .iter()
        .filter(|member| member.label == workset.primary_member)
        .count();
    if primary_count != 1 {
        return Err(WorksetError::PrimaryMemberMissing {
            label: workset.primary_member.as_str().to_string(),
        });
    }
    Ok(())
}

fn validate_opener_policy(policy: &WorksetOpenerPolicy) -> Result<(), WorksetError> {
    validate_blake3_identity(&policy.policy_hash, "policy_hash")?;
    if policy.profiles.len() > MAX_OPENER_PROFILES {
        return Err(WorksetError::ProfileLimitExceeded {
            actual: policy.profiles.len(),
            limit: MAX_OPENER_PROFILES,
        });
    }
    let mut profile_ids = std::collections::BTreeSet::new();
    for profile in &policy.profiles {
        validate_opener_profile(profile)?;
        if !profile_ids.insert(profile.id.as_str()) {
            return Err(WorksetError::DuplicateOpenerProfile {
                profile_id: profile.id.as_str().to_string(),
            });
        }
    }
    Ok(())
}

fn validate_opener_profile(profile: &WorksetOpenerProfile) -> Result<(), WorksetError> {
    validate_identifier(
        profile.id.as_str(),
        "opener_profile_id",
        MAX_OPENER_PROFILE_ID_BYTES,
    )?;
    validate_bounded_token(
        &profile.executable,
        "opener_executable",
        MAX_EXECUTABLE_BYTES,
    )?;
    let executable_is_simple = profile.executable.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '+')
    });
    if !executable_is_simple {
        return Err(WorksetError::InvalidIdentifier {
            field: "opener_executable".to_string(),
        });
    }
    if SHELL_EXECUTABLES.contains(&profile.executable.as_str()) {
        return Err(WorksetError::ShellExecutableRejected {
            executable: profile.executable.clone(),
        });
    }
    if profile.fixed_args.len() > MAX_FIXED_ARGS {
        return Err(WorksetError::ByteLimitExceeded {
            field: "opener_fixed_args".to_string(),
            actual: profile.fixed_args.len(),
            limit: MAX_FIXED_ARGS,
        });
    }
    for (index, argument) in profile.fixed_args.iter().enumerate() {
        let field = format!("opener_fixed_args[{index}]");
        validate_bounded_token(argument, &field, MAX_ARG_BYTES)?;
        if contains_absolute_path_text(argument) {
            return Err(WorksetError::AbsoluteProfilePath { field });
        }
    }
    Ok(())
}

fn validate_expected_prior_identities(
    state: &WorksetState,
    expected_local: &str,
    expected_portable: &str,
) -> Result<(), WorksetError> {
    validate_blake3_identity(expected_local, "expected_prior_state_identity")?;
    validate_blake3_identity(expected_portable, "expected_prior_portable_state_identity")?;
    if workset_state_identity(state)? != expected_local {
        return Err(WorksetError::StalePriorStateIdentity);
    }
    if workset_portable_state_identity(state)? != expected_portable {
        return Err(WorksetError::StalePriorPortableStateIdentity);
    }
    Ok(())
}

fn require_opener_profile<'a>(
    policy: &'a WorksetOpenerPolicy,
    profile_id: &WorksetOpenerProfileId,
) -> Result<&'a WorksetOpenerProfile, WorksetError> {
    validate_opener_policy(policy)?;
    policy
        .profiles
        .iter()
        .find(|profile| profile.id == *profile_id)
        .ok_or_else(|| WorksetError::UnknownOpenerProfile {
            profile_id: profile_id.as_str().to_string(),
        })
}

fn validate_identifier(value: &str, field: &str, limit: usize) -> Result<(), WorksetError> {
    if value.is_empty() {
        return Err(WorksetError::EmptyField {
            field: field.to_string(),
        });
    }
    if contains_shell_syntax(value) {
        return Err(WorksetError::ShellSyntaxRejected {
            field: field.to_string(),
        });
    }
    if value.len() > limit {
        return Err(WorksetError::ByteLimitExceeded {
            field: field.to_string(),
            actual: value.len(),
            limit,
        });
    }
    let valid = value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'));
    if !valid {
        return Err(WorksetError::InvalidIdentifier {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn validate_bounded_token(value: &str, field: &str, limit: usize) -> Result<(), WorksetError> {
    if value.is_empty() {
        return Err(WorksetError::EmptyField {
            field: field.to_string(),
        });
    }
    if contains_shell_syntax(value) {
        return Err(WorksetError::ShellSyntaxRejected {
            field: field.to_string(),
        });
    }
    if value.len() > limit {
        return Err(WorksetError::ByteLimitExceeded {
            field: field.to_string(),
            actual: value.len(),
            limit,
        });
    }
    if value.contains('\0') {
        return Err(WorksetError::InvalidIdentifier {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn validate_display_path(display_path: &str, label: &str) -> Result<(), WorksetError> {
    let field = format!("members[{label}].display_path");
    if display_path.is_empty() {
        return Err(WorksetError::EmptyField { field });
    }
    if display_path.len() > MAX_DISPLAY_PATH_BYTES {
        return Err(WorksetError::ByteLimitExceeded {
            field,
            actual: display_path.len(),
            limit: MAX_DISPLAY_PATH_BYTES,
        });
    }
    if display_path.contains('\0') {
        return Err(WorksetError::InvalidIdentifier { field });
    }
    Ok(())
}

fn validate_blake3_identity(value: &str, field: &str) -> Result<(), WorksetError> {
    let valid = value.len() == BLAKE3_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !valid {
        return Err(WorksetError::InvalidBlake3Identity {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn contains_shell_syntax(value: &str) -> bool {
    value
        .chars()
        .any(|character| SHELL_SYNTAX_CHARS.contains(character))
}

fn contains_absolute_path_text(value: &str) -> bool {
    if looks_absolute_path(value) {
        return true;
    }
    value
        .split_once('=')
        .is_some_and(|(_, suffix)| looks_absolute_path(suffix))
}

fn looks_absolute_path(value: &str) -> bool {
    if value.starts_with('/') || value.starts_with('\\') {
        return true;
    }
    let mut characters = value.chars();
    matches!(
        (characters.next(), characters.next(), characters.next()),
        (Some(drive), Some(':'), Some('/' | '\\')) if drive.is_ascii_alphabetic()
    )
}

fn build_opener_argv(
    profile: &WorksetOpenerProfile,
    workset: &Workset,
) -> Result<(Vec<String>, Vec<WorksetPortableArg>), WorksetError> {
    validate_opener_profile(profile)?;
    validate_workset_shape(workset)?;
    let mut argv = Vec::with_capacity(1 + profile.fixed_args.len() + workset.members.len());
    argv.push(profile.executable.clone());
    argv.extend(profile.fixed_args.iter().cloned());
    argv.extend(
        workset
            .members
            .iter()
            .map(|member| member.display_path.clone()),
    );

    let mut portable_argv = Vec::with_capacity(argv.len());
    portable_argv.push(WorksetPortableArg::Literal(profile.executable.clone()));
    portable_argv.extend(
        profile
            .fixed_args
            .iter()
            .cloned()
            .map(WorksetPortableArg::Literal),
    );
    portable_argv.extend(
        workset
            .members
            .iter()
            .map(|member| WorksetPortableArg::Member(member.label.clone())),
    );
    Ok((argv, portable_argv))
}

fn transition_artifacts(workset: &Workset) -> Vec<WorksetArtifactPlan> {
    let mut artifacts = vec![WorksetArtifactPlan {
        label: "workset-state".to_string(),
        role: WorksetArtifactRole::LocalState,
        effect: WorksetArtifactEffect::ReplaceLocalState,
        logical_path: Some(WORKSET_STATE_LOGICAL_PATH.to_string()),
        display_path: None,
    }];
    artifacts.extend(member_artifacts(
        workset,
        WorksetArtifactEffect::PreserveMember,
    ));
    artifacts
}

fn member_artifacts(workset: &Workset, effect: WorksetArtifactEffect) -> Vec<WorksetArtifactPlan> {
    workset
        .members
        .iter()
        .map(|member| WorksetArtifactPlan {
            label: member.label.as_str().to_string(),
            role: if member.label == workset.primary_member {
                WorksetArtifactRole::PrimaryMember
            } else {
                WorksetArtifactRole::Member
            },
            effect,
            logical_path: None,
            display_path: Some(member.display_path.clone()),
        })
        .collect()
}

fn portable_artifacts(artifacts: &[WorksetArtifactPlan]) -> Vec<PortableArtifactPlan> {
    artifacts
        .iter()
        .map(|artifact| PortableArtifactPlan {
            label: artifact.label.clone(),
            role: artifact.role,
            effect: artifact.effect,
            logical_path: artifact.logical_path.clone(),
        })
        .collect()
}

fn workset_non_claims() -> Vec<String> {
    vec![
        WORKSET_NON_CLAIM_MEMBER_EFFECTS.to_string(),
        WORKSET_NON_CLAIM_LIFECYCLE_AUTHORITY.to_string(),
        WORKSET_NON_CLAIM_EDITOR_STATE.to_string(),
    ]
}

fn hash_serializable<T: serde::Serialize>(value: &T) -> Result<String, WorksetError> {
    let value = serde_json::to_value(value).map_err(|error| WorksetError::Serialization {
        reason: error.to_string(),
    })?;
    Ok(super::evidence::canonical_json_hash(&value))
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct PortableWorkset {
    id: WorksetId,
    ordered_member_labels: Vec<WorksetMemberLabel>,
    primary_member: WorksetMemberLabel,
    opener_profile: WorksetOpenerProfileId,
}

fn portable_workset(workset: &Workset) -> PortableWorkset {
    PortableWorkset {
        id: workset.id.clone(),
        ordered_member_labels: workset
            .members
            .iter()
            .map(|member| member.label.clone())
            .collect(),
        primary_member: workset.primary_member.clone(),
        opener_profile: workset.opener_profile.clone(),
    }
}

#[derive(serde::Serialize)]
struct PortableStateMaterial {
    schema_version: String,
    worksets: Vec<PortableWorkset>,
}

#[derive(serde::Serialize)]
struct PortableArtifactPlan {
    label: String,
    role: WorksetArtifactRole,
    effect: WorksetArtifactEffect,
    logical_path: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PortablePlanAction {
    ReplaceLocalState {
        logical_path: String,
        expected_prior_portable_state_identity: String,
        next_portable_state_identity: String,
    },
}

#[derive(serde::Serialize)]
struct PortableTransitionPlanMaterial {
    schema_version: String,
    operation: WorksetTransitionOperation,
    execution: WorksetExecutionMode,
    workset: PortableWorkset,
    expected_prior_portable_state_identity: String,
    next_portable_state_identity: String,
    policy_hash: String,
    opener_profile_identity: String,
    actions: Vec<PortablePlanAction>,
    artifacts: Vec<PortableArtifactPlan>,
    non_claims: Vec<String>,
}

#[derive(serde::Serialize)]
struct PortableOpenPlanMaterial {
    schema_version: String,
    execution: WorksetExecutionMode,
    workset: PortableWorkset,
    requested_opener: Option<WorksetOpenerProfileId>,
    selected_opener: WorksetOpenerProfileId,
    expected_portable_state_identity: String,
    expected_portable_workset_identity: String,
    policy_hash: String,
    opener_profile_identity: String,
    portable_argv: Vec<WorksetPortableArg>,
    artifacts: Vec<PortableArtifactPlan>,
    non_claims: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ROOT_ONE: &str = "/home/alice/git";
    const ROOT_TWO: &str = "/srv/build/checkouts";
    const TOO_MANY_ID_BYTES: usize = MAX_WORKSET_ID_BYTES + 1;

    fn policy() -> WorksetOpenerPolicy {
        WorksetOpenerPolicy {
            policy_hash: POLICY_HASH.to_string(),
            profiles: vec![
                WorksetOpenerProfile::new("code", "code", vec!["--new-window".to_string()])
                    .unwrap(),
                WorksetOpenerProfile::new("zed", "zed", Vec::new()).unwrap(),
            ],
        }
    }

    fn workset(root: &str) -> Workset {
        Workset::new(
            "release-stack",
            vec![
                WorksetMember::new("cairn", format!("{root}/cairn")).unwrap(),
                WorksetMember::new("onixos", format!("{root}/onixos")).unwrap(),
            ],
            "cairn",
            "code",
        )
        .unwrap()
    }

    fn create_request(
        state: &WorksetState,
        root: &str,
        execution: WorksetExecutionMode,
    ) -> WorksetCreateRequest {
        WorksetCreateRequest {
            workset: workset(root),
            expected_prior_state_identity: workset_state_identity(state).unwrap(),
            expected_prior_portable_state_identity: workset_portable_state_identity(state).unwrap(),
            execution,
        }
    }

    fn populated_state(root: &str) -> WorksetState {
        let empty = WorksetState::empty();
        let plan = plan_workset_create(
            &empty,
            &policy(),
            create_request(&empty, root, WorksetExecutionMode::Execute),
        )
        .unwrap();
        apply_workset_transition(&empty, &policy(), &plan).unwrap()
    }

    fn open_request(state: &WorksetState, execution: WorksetExecutionMode) -> WorksetOpenRequest {
        WorksetOpenRequest {
            workset_id: WorksetId::parse("release-stack").unwrap(),
            opener_override: None,
            expected_state_identity: workset_state_identity(state).unwrap(),
            expected_portable_state_identity: workset_portable_state_identity(state).unwrap(),
            execution,
        }
    }

    // r[verify cairn.lifecycle_stores.worksets.open]
    #[test]
    fn valid_create_remove_and_open_plans_are_exact() {
        let empty = WorksetState::empty();
        let create = plan_workset_create(
            &empty,
            &policy(),
            create_request(&empty, ROOT_ONE, WorksetExecutionMode::Execute),
        )
        .unwrap();
        assert!(create.execution_authorized());
        assert_eq!(create.actions.len(), 1);
        let populated = apply_workset_transition(&empty, &policy(), &create).unwrap();

        let open = plan_workset_open(
            &populated,
            &policy(),
            open_request(&populated, WorksetExecutionMode::Execute),
        )
        .unwrap();
        assert_eq!(
            open.argv,
            vec![
                "code".to_string(),
                "--new-window".to_string(),
                format!("{ROOT_ONE}/cairn"),
                format!("{ROOT_ONE}/onixos"),
            ]
        );
        assert_eq!(
            open.artifacts
                .iter()
                .map(|artifact| artifact.role)
                .collect::<Vec<_>>(),
            vec![
                WorksetArtifactRole::PrimaryMember,
                WorksetArtifactRole::Member,
            ]
        );
        assert_eq!(
            open.local_receipt.evidence_class,
            WORKSET_EVIDENCE_CLASS_LOCAL_ONLY
        );
        recheck_workset_open_plan(&populated, &policy(), &open).unwrap();

        let remove = plan_workset_remove(
            &populated,
            &policy(),
            WorksetRemoveRequest {
                workset_id: WorksetId::parse("release-stack").unwrap(),
                expected_prior_state_identity: workset_state_identity(&populated).unwrap(),
                expected_prior_portable_state_identity: workset_portable_state_identity(&populated)
                    .unwrap(),
                execution: WorksetExecutionMode::Execute,
            },
        )
        .unwrap();
        let removed = apply_workset_transition(&populated, &policy(), &remove).unwrap();
        assert!(removed.worksets.is_empty());
    }

    // r[verify cairn.lifecycle_stores.worksets.opener_reject]
    #[test]
    fn empty_and_duplicate_members_are_rejected() {
        let empty_error = Workset::new("empty", Vec::new(), "primary", "code").unwrap_err();
        assert_eq!(
            empty_error,
            WorksetError::EmptyField {
                field: "members".to_string(),
            }
        );

        let duplicate_label = Workset::new(
            "duplicate-label",
            vec![
                WorksetMember::new("cairn", "/one/cairn").unwrap(),
                WorksetMember::new("cairn", "/two/cairn").unwrap(),
            ],
            "cairn",
            "code",
        )
        .unwrap_err();
        assert_eq!(
            duplicate_label,
            WorksetError::DuplicateMemberLabel {
                label: "cairn".to_string(),
            }
        );

        let duplicate_path = Workset::new(
            "duplicate-path",
            vec![
                WorksetMember::new("cairn", "/same/path").unwrap(),
                WorksetMember::new("onixos", "/same/path").unwrap(),
            ],
            "cairn",
            "code",
        )
        .unwrap_err();
        assert_eq!(
            duplicate_path,
            WorksetError::DuplicateMemberPath {
                display_path: "/same/path".to_string(),
            }
        );
    }

    #[test]
    fn bounded_ids_reject_oversized_text() {
        let error = WorksetId::parse("x".repeat(TOO_MANY_ID_BYTES)).unwrap_err();
        assert_eq!(
            error,
            WorksetError::ByteLimitExceeded {
                field: "workset_id".to_string(),
                actual: TOO_MANY_ID_BYTES,
                limit: MAX_WORKSET_ID_BYTES,
            }
        );
    }

    #[test]
    fn primary_member_must_name_exactly_one_member() {
        let error = Workset::new(
            "missing-primary",
            vec![WorksetMember::new("cairn", "/one/cairn").unwrap()],
            "onixos",
            "code",
        )
        .unwrap_err();
        assert_eq!(
            error,
            WorksetError::PrimaryMemberMissing {
                label: "onixos".to_string(),
            }
        );
    }

    // r[verify cairn.lifecycle_stores.worksets.opener_reject]
    #[test]
    fn unknown_opener_text_and_shell_interpolation_are_rejected() {
        let state = populated_state(ROOT_ONE);
        let unknown = plan_workset_open(
            &state,
            &policy(),
            WorksetOpenRequest {
                opener_override: Some(WorksetOpenerProfileId::parse("custom-command").unwrap()),
                ..open_request(&state, WorksetExecutionMode::Preview)
            },
        )
        .unwrap_err();
        assert_eq!(
            unknown,
            WorksetError::UnknownOpenerProfile {
                profile_id: "custom-command".to_string(),
            }
        );

        let interpolation = WorksetOpenerProfileId::parse("$(touch-pwned)").unwrap_err();
        assert_eq!(
            interpolation,
            WorksetError::ShellSyntaxRejected {
                field: "opener_profile_id".to_string(),
            }
        );
        let profile_interpolation =
            WorksetOpenerProfile::new("unsafe", "code", vec!["$(touch-pwned)".to_string()])
                .unwrap_err();
        assert_eq!(
            profile_interpolation,
            WorksetError::ShellSyntaxRejected {
                field: "opener_fixed_args[0]".to_string(),
            }
        );
        let shell_executable =
            WorksetOpenerProfile::new("unsafe", "sh", vec!["-c".to_string()]).unwrap_err();
        assert_eq!(
            shell_executable,
            WorksetError::ShellExecutableRejected {
                executable: "sh".to_string(),
            }
        );
    }

    // r[verify cairn.lifecycle_stores.functional_boundary.shell]
    #[test]
    fn changed_member_paths_reject_a_reviewed_open_plan() {
        let state = populated_state(ROOT_ONE);
        let plan = plan_workset_open(
            &state,
            &policy(),
            open_request(&state, WorksetExecutionMode::Execute),
        )
        .unwrap();
        let changed = populated_state(ROOT_TWO);
        let error = recheck_workset_open_plan(&changed, &policy(), &plan).unwrap_err();
        assert_eq!(error, WorksetError::StalePriorStateIdentity);
    }

    // r[verify cairn.lifecycle_stores.functional_boundary.shell]
    #[test]
    fn stale_transition_plans_fail_without_partial_state() {
        let empty = WorksetState::empty();
        let reviewed = plan_workset_create(
            &empty,
            &policy(),
            create_request(&empty, ROOT_ONE, WorksetExecutionMode::Execute),
        )
        .unwrap();
        let other_workset = Workset::new(
            "other",
            vec![WorksetMember::new("other", "/tmp/other").unwrap()],
            "other",
            "zed",
        )
        .unwrap();
        let concurrent = plan_workset_create(
            &empty,
            &policy(),
            WorksetCreateRequest {
                workset: other_workset,
                expected_prior_state_identity: workset_state_identity(&empty).unwrap(),
                expected_prior_portable_state_identity: workset_portable_state_identity(&empty)
                    .unwrap(),
                execution: WorksetExecutionMode::Execute,
            },
        )
        .unwrap();
        let changed = apply_workset_transition(&empty, &policy(), &concurrent).unwrap();
        let error = apply_workset_transition(&changed, &policy(), &reviewed).unwrap_err();
        assert_eq!(error, WorksetError::StalePriorStateIdentity);
        assert_eq!(changed.worksets.len(), 1);
        assert_eq!(changed.worksets[0].id.as_str(), "other");
    }

    // r[verify cairn.lifecycle_stores.worksets.open]
    #[test]
    fn preview_open_exposes_exact_plan_without_execution_authority() {
        let state = populated_state(ROOT_ONE);
        let preview = plan_workset_open(
            &state,
            &policy(),
            open_request(&state, WorksetExecutionMode::Preview),
        )
        .unwrap();
        assert!(!preview.execution_authorized());
        assert!(!preview.argv.is_empty());
        assert_eq!(preview.actions.len(), 1);
        assert!(preview
            .artifacts
            .iter()
            .all(|artifact| artifact.effect == WorksetArtifactEffect::OpenReadOnly));
        recheck_workset_open_plan(&state, &policy(), &preview).unwrap();
    }

    // r[verify cairn.lifecycle_stores.worksets.remove]
    #[test]
    fn removal_plan_replaces_only_local_state_and_preserves_members() {
        let state = populated_state(ROOT_ONE);
        let remove = plan_workset_remove(
            &state,
            &policy(),
            WorksetRemoveRequest {
                workset_id: WorksetId::parse("release-stack").unwrap(),
                expected_prior_state_identity: workset_state_identity(&state).unwrap(),
                expected_prior_portable_state_identity: workset_portable_state_identity(&state)
                    .unwrap(),
                execution: WorksetExecutionMode::Execute,
            },
        )
        .unwrap();
        assert_eq!(remove.actions.len(), 1);
        assert_eq!(
            remove.artifacts[0].effect,
            WorksetArtifactEffect::ReplaceLocalState
        );
        assert!(remove.artifacts[1..]
            .iter()
            .all(|artifact| artifact.effect == WorksetArtifactEffect::PreserveMember));
        assert!(remove
            .local_receipt
            .non_claims
            .iter()
            .any(|claim| claim == WORKSET_NON_CLAIM_MEMBER_EFFECTS));
    }

    // r[verify cairn.lifecycle_stores.receipts.portable]
    #[test]
    fn portable_hashes_exclude_absolute_member_paths() {
        let empty = WorksetState::empty();
        let first_create = plan_workset_create(
            &empty,
            &policy(),
            create_request(&empty, ROOT_ONE, WorksetExecutionMode::Preview),
        )
        .unwrap();
        let second_create = plan_workset_create(
            &empty,
            &policy(),
            create_request(&empty, ROOT_TWO, WorksetExecutionMode::Preview),
        )
        .unwrap();
        assert_ne!(
            first_create.local_receipt_identity,
            second_create.local_receipt_identity
        );
        assert_eq!(
            first_create.portable_identity,
            second_create.portable_identity
        );

        let first_state = populated_state(ROOT_ONE);
        let second_state = populated_state(ROOT_TWO);
        assert_ne!(
            workset_state_identity(&first_state).unwrap(),
            workset_state_identity(&second_state).unwrap()
        );
        assert_eq!(
            workset_portable_state_identity(&first_state).unwrap(),
            workset_portable_state_identity(&second_state).unwrap()
        );

        let first_open = plan_workset_open(
            &first_state,
            &policy(),
            open_request(&first_state, WorksetExecutionMode::Preview),
        )
        .unwrap();
        let second_open = plan_workset_open(
            &second_state,
            &policy(),
            open_request(&second_state, WorksetExecutionMode::Preview),
        )
        .unwrap();
        assert_ne!(
            first_open.local_receipt_identity,
            second_open.local_receipt_identity
        );
        assert_ne!(first_open.plan_identity, second_open.plan_identity);
        assert_eq!(first_open.portable_identity, second_open.portable_identity);
        assert_eq!(first_open.portable_argv, second_open.portable_argv);
        assert_ne!(first_open.argv, second_open.argv);
    }
}

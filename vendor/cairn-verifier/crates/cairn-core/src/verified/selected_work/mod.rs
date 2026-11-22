//! Pure selected-work composition. Native validators, not this view, own decisions.
use std::collections::BTreeSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::readiness::{ReadinessReport, ReadinessState, READINESS_STAGES};
use crate::canonical_json_hash;

mod obligations;
pub use obligations::{
    ObligationObservationInput, ObligationSource, SelectedObligationExport, SelectedObligations,
};

pub const SCHEMA: &str = "cairn.selected-work.v1";
const PRODUCER: &str = "cairn";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkPolicy {
    pub schema: String,
    pub max_artifacts: usize,
    pub max_units: usize,
    pub max_references: usize,
    pub max_context_entries: usize,
    pub max_observation_bytes: u64,
}
static BUNDLED: LazyLock<Result<SelectedWorkPolicy, SelectedWorkError>> = LazyLock::new(|| {
    SelectedWorkPolicy::admit_json(include_str!(
        "../../../../../cairn-policy/generated/selected-work.json"
    ))
});
impl SelectedWorkPolicy {
    pub fn admit_json(text: &str) -> Result<Self, SelectedWorkError> {
        let policy: Self =
            serde_json::from_str(text).map_err(|_| SelectedWorkError::InvalidPolicy)?;
        if policy.schema != SCHEMA
            || policy.max_artifacts == 0
            || policy.max_artifacts > 512
            || policy.max_units == 0
            || policy.max_units > 512
            || policy.max_references == 0
            || policy.max_references > 4096
            || policy.max_context_entries == 0
            || policy.max_context_entries > 512
            || policy.max_observation_bytes == 0
            || policy.max_observation_bytes > 67_108_864
        {
            return Err(SelectedWorkError::InvalidPolicy);
        }
        Ok(policy)
    }
    pub fn bundled() -> Result<Self, SelectedWorkError> {
        (*BUNDLED).clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub change: String,
    pub task: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRevisionKind {
    Exact,
    Projection,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRevision {
    pub kind: SourceRevisionKind,
    pub identity: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRoot {
    Lifecycle,
    Repository,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Present,
    Missing,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceArtifact {
    pub root: SourceRoot,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    pub status: SourceStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCohort {
    pub revision: SourceRevision,
    pub artifacts: Vec<SourceArtifact>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Ready,
    Blocked,
    Unavailable,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyFact {
    pub task: String,
    pub prerequisite: String,
    pub state: Availability,
    pub identity: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionFact {
    pub state: Availability,
    pub contract_hash: Option<String>,
    pub results_hash: Option<String>,
    pub subject_hash: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextEntry {
    pub id: String,
    pub authority: String,
    pub source_root: Option<String>,
    pub path: Option<String>,
    pub status: Availability,
    pub content_hash: Option<String>,
    pub text: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFact {
    pub selection_hash: String,
    pub local_hash: String,
    pub portable_hash: String,
    pub entries: Vec<ContextEntry>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableContextEntry {
    pub id: String,
    pub authority: String,
    pub source_root: Option<String>,
    pub path: Option<String>,
    pub status: Availability,
    pub content_hash: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableContext {
    pub selection_hash: String,
    pub portable_hash: String,
    pub entries: Vec<PortableContextEntry>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    Ready,
    Blocked,
    Complete,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Requirement,
    Acceptance,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "task", rename_all = "snake_case")]
pub enum ReferenceScope {
    Change,
    Task(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedReference {
    pub kind: ReferenceKind,
    pub id: String,
    pub scope: ReferenceScope,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationUnit {
    pub task: Option<String>,
    pub state: WorkState,
    pub references: Vec<ScopedReference>,
    pub verification_contracts: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemainingKind {
    Implementation,
    Completion,
    Sync,
    Archive,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingWork {
    pub kind: RemainingKind,
    pub state: Availability,
    pub subject: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enumeration {
    pub complete: bool,
    pub observed: u32,
    pub total: Option<u32>,
    pub budget_bytes: u64,
    pub consumed_bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkInput {
    pub producer: String,
    pub selection: Selection,
    pub source: SourceCohort,
    pub policy_hash: String,
    pub profile: String,
    pub profile_hash: String,
    pub dependencies: Vec<DependencyFact>,
    pub completion: CompletionFact,
    pub context: ContextFact,
    pub readiness: ReadinessReport,
    pub units: Vec<ImplementationUnit>,
    pub remaining: Vec<RemainingWork>,
    pub enumeration: Enumeration,
    pub local_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CohortIdentity {
    pub source: String,
    pub intent: String,
    pub context: String,
    pub verification_subject: String,
    pub dependencies: String,
    pub obligations: String,
    pub selection: String,
    pub observation: String,
}
pub type ExpectedCohort = CohortIdentity;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableStage {
    pub stage: String,
    pub state: ReadinessState,
    pub decision_hash: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableBlocker {
    pub id: String,
    pub stage: String,
    pub code: String,
    pub target: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableAction {
    pub id: String,
    pub kind: String,
    pub causes: Vec<String>,
    pub target: String,
    pub prerequisites: Vec<String>,
    pub authority: String,
    pub expected_observation: String,
    pub original_argv_hash: String,
    pub portable_argv: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableReadinessScope {
    pub change: String,
    pub policy_hash: String,
    pub source_hash: String,
    pub evidence_hash: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableReadiness {
    pub native_receipt_hash: String,
    pub native_scope: PortableReadinessScope,
    pub native_input_hash: String,
    pub native_policy_hash: String,
    pub valid: bool,
    pub classification: String,
    pub stages: Vec<PortableStage>,
    pub blockers: Vec<PortableBlocker>,
    pub actions: Vec<PortableAction>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticAvailability {
    Supported,
    NotSelected,
    Unsupported,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticExtensions {
    pub dependency_meanings: SemanticAvailability,
    pub obligation_explanations: SemanticAvailability,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalDetail {
    pub root: String,
    pub context: ContextFact,
    pub readiness: ReadinessReport,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkSnapshot {
    pub schema: String,
    pub producer: String,
    pub selection: Selection,
    pub cohort: CohortIdentity,
    pub source: SourceCohort,
    pub policy_hash: String,
    pub profile: String,
    pub profile_hash: String,
    pub dependencies: Vec<DependencyFact>,
    pub completion: CompletionFact,
    pub context: PortableContext,
    pub readiness: PortableReadiness,
    pub units: Vec<ImplementationUnit>,
    pub remaining: Vec<RemainingWork>,
    pub enumeration: Enumeration,
    pub semantic_extensions: SemanticExtensions,
    pub obligations: SelectedObligations,
    pub local: Option<LocalDetail>,
}
impl SelectedWorkSnapshot {
    pub fn portable(&self) -> Self {
        self.clone().into_portable()
    }
    pub fn into_portable(mut self) -> Self {
        self.local = None;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedWorkError {
    InvalidPolicy,
    UnsupportedSchema,
    InvalidIdentity,
    InvalidSelection,
    Duplicate,
    InvalidSource,
    Incomplete,
    BudgetExceeded,
    CrossedSubject,
    StaleCohort,
    InvalidReadiness,
    InvalidContext,
    UnsafePortable,
    InvalidObligations,
}
impl std::fmt::Display for SelectedWorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "selected-work rejected: {self:?}")
    }
}
impl std::error::Error for SelectedWorkError {}

fn digest(value: serde_json::Value) -> String {
    canonical_json_hash(&value)
}
fn hash_ok(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn locator_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 4096
        && !s.starts_with('/')
        && !s.contains('\\')
        && !s.contains(':')
        && s.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
fn selector_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn reference_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && s.split(' ').all(selector_ok)
}

/// Extract scoped native `r[...]` and `a[...]` markers from admitted text.
/// Invalid or unterminated markers refuse rather than implying no references.
// r[impl cairn.selected_work.conformance]
pub fn extract_scoped_references(
    text: &str,
    scope: ReferenceScope,
) -> Result<Vec<ScopedReference>, SelectedWorkError> {
    let mut references = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        let mut cursor = 0;
        while cursor < line.len() {
            let next = line[cursor..]
                .match_indices("r[")
                .map(|(offset, _)| (cursor + offset, ReferenceKind::Requirement))
                .chain(
                    line[cursor..]
                        .match_indices("a[")
                        .map(|(offset, _)| (cursor + offset, ReferenceKind::Acceptance)),
                )
                .min_by_key(|(index, _)| *index);
            let Some((start, kind)) = next else { break };
            if start > 0 && line.as_bytes()[start - 1].is_ascii_alphanumeric() {
                cursor = start + 2;
                continue;
            }
            let content_start = start + 2;
            let Some(end) = line[content_start..]
                .find(']')
                .map(|offset| content_start + offset)
            else {
                return Err(SelectedWorkError::InvalidSelection);
            };
            let id = line[content_start..end].trim();
            if !reference_ok(id) {
                return Err(SelectedWorkError::InvalidSelection);
            }
            if seen.insert((kind as u8, id)) {
                references.push(ScopedReference {
                    kind,
                    id: id.into(),
                    scope: scope.clone(),
                });
            }
            cursor = end + 1;
        }
    }
    Ok(references)
}
fn expected_policy(policy: &SelectedWorkPolicy) -> Result<(), SelectedWorkError> {
    match &*BUNDLED {
        Ok(bundled) if policy == bundled => Ok(()),
        Ok(_) => Err(SelectedWorkError::UnsupportedSchema),
        Err(error) => Err(*error),
    }
}
fn complete_stages<'a>(stages: impl Iterator<Item = &'a str>) -> bool {
    let mut bits = 0u8;
    for stage in stages {
        let Some(index) = READINESS_STAGES.iter().position(|name| *name == stage) else {
            return false;
        };
        let bit = 1 << index;
        if bits & bit != 0 {
            return false;
        }
        bits |= bit;
    }
    bits == 0b111_1111
}
fn validate_source(
    policy: &SelectedWorkPolicy,
    source: &SourceCohort,
) -> Result<(), SelectedWorkError> {
    if !hash_ok(&source.revision.identity)
        || source.artifacts.is_empty()
        || source.artifacts.len() > policy.max_artifacts
    {
        return Err(SelectedWorkError::InvalidSource);
    }
    if source.artifacts.windows(2).any(|pair| {
        (pair[0].root as u8, pair[0].path.as_str()) >= (pair[1].root as u8, pair[1].path.as_str())
    }) {
        return Err(SelectedWorkError::InvalidSource);
    }
    if source.revision.kind == SourceRevisionKind::Projection
        && source.revision.identity != digest(json!({"artifacts": source.artifacts}))
    {
        return Err(SelectedWorkError::InvalidIdentity);
    }
    let mut seen = BTreeSet::new();
    for artifact in &source.artifacts {
        if !locator_ok(&artifact.path)
            || !seen.insert((artifact.root as u8, &artifact.path))
            || (artifact.status == SourceStatus::Present && !hash_ok(&artifact.blake3))
            || (artifact.status != SourceStatus::Present
                && (!artifact.blake3.is_empty() || artifact.bytes != 0))
        {
            return Err(SelectedWorkError::InvalidSource);
        }
    }
    Ok(())
}
fn check_enumeration(
    policy: &SelectedWorkPolicy,
    enumeration: &Enumeration,
) -> Result<(), SelectedWorkError> {
    if enumeration.budget_bytes == 0
        || enumeration.budget_bytes > policy.max_observation_bytes
        || enumeration.consumed_bytes > enumeration.budget_bytes
    {
        return Err(SelectedWorkError::BudgetExceeded);
    }
    if !enumeration.complete || enumeration.total != Some(enumeration.observed) {
        return Err(SelectedWorkError::Incomplete);
    }
    Ok(())
}
fn validate_units(
    selection: &Selection,
    units: &[ImplementationUnit],
    dependencies: &[DependencyFact],
    remaining: &[RemainingWork],
    policy: &SelectedWorkPolicy,
) -> Result<(), SelectedWorkError> {
    if units.len() > policy.max_units {
        return Err(SelectedWorkError::InvalidSelection);
    }
    let mut remaining_kinds = [false; 4];
    for work in remaining {
        let kind = work.kind as usize;
        if remaining_kinds[kind] {
            return Err(SelectedWorkError::Duplicate);
        }
        remaining_kinds[kind] = true;
    }
    if !remaining_kinds[RemainingKind::Completion as usize]
        || !remaining_kinds[RemainingKind::Sync as usize]
        || !remaining_kinds[RemainingKind::Archive as usize]
        || (units.iter().any(|unit| unit.state != WorkState::Complete)
            && !remaining_kinds[RemainingKind::Implementation as usize])
    {
        return Err(SelectedWorkError::Incomplete);
    }
    let mut seen = BTreeSet::new();
    for unit in units {
        if unit.references.len() > policy.max_references
            || !seen.insert(unit.task.as_deref())
            || unit.task.as_deref().is_some_and(|id| !selector_ok(id))
        {
            return Err(SelectedWorkError::Duplicate);
        }
        if unit.references.iter().any(|reference| {
            !reference_ok(&reference.id)
                || match &reference.scope {
                    ReferenceScope::Change => false,
                    ReferenceScope::Task(id) => unit.task.as_ref() != Some(id),
                }
        }) || unit.verification_contracts.iter().any(|id| !locator_ok(id))
        {
            return Err(SelectedWorkError::InvalidSelection);
        }
        let mut refs = BTreeSet::new();
        if unit
            .references
            .iter()
            .any(|reference| !refs.insert((reference.kind as u8, &reference.id, &reference.scope)))
        {
            return Err(SelectedWorkError::Duplicate);
        }
    }
    if selection.task.is_some() && !seen.contains(&selection.task.as_deref()) {
        return Err(SelectedWorkError::InvalidSelection);
    }
    if selection.task.is_none()
        && units.is_empty()
        && !remaining
            .iter()
            .any(|work| work.kind == RemainingKind::Completion)
    {
        return Err(SelectedWorkError::Incomplete);
    }
    let known = units
        .iter()
        .filter_map(|unit| unit.task.as_deref())
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::new();
    for dependency in dependencies {
        if !(known.contains(dependency.task.as_str()) || dependency.task == selection.change)
            || !selector_ok(&dependency.prerequisite)
            || !hash_ok(&dependency.identity)
            || !edges.insert((&dependency.task, &dependency.prerequisite))
        {
            return Err(SelectedWorkError::InvalidSelection);
        }
    }
    // Explicit task scope is exactly its declared prerequisite closure.
    if let Some(selected) = &selection.task {
        let mut reachable = BTreeSet::from([selected.as_str()]);
        for _ in 0..units.len() {
            let before = reachable.len();
            for dependency in dependencies {
                if dependency.task != selection.change
                    && reachable.contains(dependency.task.as_str())
                    && known.contains(dependency.prerequisite.as_str())
                {
                    reachable.insert(&dependency.prerequisite);
                }
            }
            if before == reachable.len() {
                break;
            }
        }
        if known != reachable {
            return Err(SelectedWorkError::InvalidSelection);
        }
    }
    Ok(())
}
fn portable_target(target: &str, root: &str) -> Result<String, SelectedWorkError> {
    if root.is_empty() {
        return Err(SelectedWorkError::InvalidSelection);
    }
    if let Some(suffix) = target.strip_prefix(root) {
        if suffix.is_empty() || suffix.starts_with('/') {
            return Ok(format!("$ROOT{suffix}"));
        }
    }
    if let Some(suffix) = target.strip_prefix("--root=") {
        if suffix == root {
            return Ok("--root=$ROOT".into());
        }
    }
    if target.starts_with('/') {
        return Ok("$LOCAL_DETAIL_PATH".into());
    }
    if target.contains(root) {
        return Err(SelectedWorkError::UnsafePortable);
    }
    Ok(target.into())
}
fn portable_argument(
    arg: &str,
    previous: Option<&str>,
    root: &str,
) -> Result<String, SelectedWorkError> {
    if previous == Some("--policy") && arg.starts_with('/') {
        return Ok("$POLICY".into());
    }
    if let Some(path) = arg.strip_prefix("--policy=") {
        if path.starts_with('/') {
            return Ok("--policy=$POLICY".into());
        }
    }
    portable_target(arg, root)
}
// r[impl cairn.selected_work.boundary]
fn portable_readiness(
    report: &ReadinessReport,
    root: &str,
) -> Result<PortableReadiness, SelectedWorkError> {
    let stages = report
        .stages
        .iter()
        .map(|stage| PortableStage {
            stage: stage.stage.clone(),
            state: stage.state,
            decision_hash: stage.decision_hash.clone(),
        })
        .collect();
    let blockers = report
        .blockers
        .iter()
        .map(|blocker| {
            Ok(PortableBlocker {
                id: blocker.id.clone(),
                stage: blocker.stage.clone(),
                code: blocker.code.clone(),
                target: portable_target(&blocker.target, root)?,
            })
        })
        .collect::<Result<Vec<_>, SelectedWorkError>>()?;
    let actions = report
        .actions
        .iter()
        .map(|action| {
            Ok(PortableAction {
                id: action.id.clone(),
                kind: action.kind.clone(),
                causes: action.causes.clone(),
                target: portable_target(&action.target, root)?,
                prerequisites: action.prerequisites.clone(),
                authority: action.authority.clone(),
                expected_observation: action.expected_observation.clone(),
                original_argv_hash: digest(json!(action.argv)),
                portable_argv: action
                    .argv
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        portable_argument(
                            arg,
                            index
                                .checked_sub(1)
                                .map(|previous| action.argv[previous].as_str()),
                            root,
                        )
                    })
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, SelectedWorkError>>()?;
    Ok(PortableReadiness {
        native_receipt_hash: report.receipt_hash.clone(),
        native_scope: PortableReadinessScope {
            change: report.scope.change.clone(),
            policy_hash: report.scope.policy_hash.clone(),
            source_hash: report.scope.source_hash.clone(),
            evidence_hash: report.scope.evidence_hash.clone(),
        },
        native_input_hash: report.input_hash.clone(),
        native_policy_hash: report.readiness_policy_hash.clone(),
        valid: report.valid,
        classification: report.classification.clone(),
        stages,
        blockers,
        actions,
    })
}
/// Units, dependency facts and remaining work are sets keyed by task or kind;
/// identities hash them in canonical order so input ordering never matters.
fn canonical_work(
    snapshot: &SelectedWorkSnapshot,
) -> (
    Vec<&ImplementationUnit>,
    Vec<&DependencyFact>,
    Vec<&RemainingWork>,
) {
    let mut units = snapshot.units.iter().collect::<Vec<_>>();
    units.sort_by(|left, right| left.task.cmp(&right.task));
    let mut dependencies = snapshot.dependencies.iter().collect::<Vec<_>>();
    dependencies.sort_by(|left, right| {
        (&left.task, &left.prerequisite).cmp(&(&right.task, &right.prerequisite))
    });
    let mut remaining = snapshot.remaining.iter().collect::<Vec<_>>();
    remaining.sort_by_key(|work| work.kind as u8);
    (units, dependencies, remaining)
}
fn cohort(snapshot: &SelectedWorkSnapshot) -> CohortIdentity {
    let (units, dependency_facts, remaining) = canonical_work(snapshot);
    let source = digest(json!(["source", snapshot.source]));
    let intent = digest(json!([
        "intent",
        source,
        snapshot.policy_hash,
        snapshot.profile,
        snapshot.profile_hash,
        dependency_facts,
        snapshot.completion,
        units,
        remaining
    ]));
    // Exactly the declared prerequisite closure: selected IDs, prerequisite
    // edges and each prerequisite's observed state. Fact identities embed
    // task prose, so they are excluded here and remain bound by `intent`.
    let dependencies = digest(json!([
        "dependencies",
        snapshot.selection,
        units.iter().map(|unit| &unit.task).collect::<Vec<_>>(),
        dependency_facts
            .iter()
            .map(|fact| json!([fact.task, fact.prerequisite, fact.state]))
            .collect::<Vec<_>>()
    ]));
    let context = digest(json!(["context", snapshot.context]));
    let verification_subject = digest(json!([
        "verification-subject",
        snapshot.completion.subject_hash,
        snapshot.completion.contract_hash,
        snapshot.completion.results_hash,
        snapshot.readiness.native_receipt_hash,
        snapshot.readiness.native_scope,
        snapshot.readiness.native_input_hash,
        snapshot.readiness.native_policy_hash
    ]));
    let obligations = digest(json!(["obligations", snapshot.obligations]));
    let selection = digest(json!([
        "selection",
        snapshot.selection,
        snapshot.enumeration
    ]));
    let observation = digest(json!([
        "observation",
        snapshot.schema,
        snapshot.producer,
        source,
        intent,
        context,
        verification_subject,
        dependencies,
        obligations,
        selection,
        snapshot.readiness,
        snapshot.semantic_extensions
    ]));
    CohortIdentity {
        source,
        intent,
        context,
        verification_subject,
        dependencies,
        obligations,
        selection,
        observation,
    }
}

/// Compose one selected-work snapshot. When `obligations` selects an admitted
/// workflow obligation definition, its predicate, guidance, typed facts and
/// evaluation bind this snapshot's verification subject and context; without
/// a selection the section stays explicitly `not_selected`.
// r[impl cairn.selected_work.contract]
pub fn compose(
    policy: &SelectedWorkPolicy,
    input: &SelectedWorkInput,
    obligation_source: Option<&ObligationSource<'_>>,
) -> Result<SelectedWorkSnapshot, SelectedWorkError> {
    expected_policy(policy)?;
    if input.producer != PRODUCER
        || !selector_ok(&input.selection.change)
        || input
            .selection
            .task
            .as_deref()
            .is_some_and(|id| !selector_ok(id))
        || !hash_ok(&input.policy_hash)
        || !hash_ok(&input.profile_hash)
        || !selector_ok(&input.profile)
        || !input.local_root.starts_with('/')
    {
        return Err(SelectedWorkError::InvalidSelection);
    }
    validate_source(policy, &input.source)?;
    check_enumeration(policy, &input.enumeration)?;
    validate_units(
        &input.selection,
        &input.units,
        &input.dependencies,
        &input.remaining,
        policy,
    )?;
    if input.context.entries.len() > policy.max_context_entries
        || !hash_ok(&input.context.selection_hash)
        || !hash_ok(&input.context.local_hash)
        || !hash_ok(&input.context.portable_hash)
    {
        return Err(SelectedWorkError::InvalidContext);
    }
    let mut context_ids = BTreeSet::new();
    let entries = input
        .context
        .entries
        .iter()
        .map(|entry| {
            if !selector_ok(&entry.id)
                || !context_ids.insert(entry.id.as_str())
                || entry.path.as_deref().is_some_and(|path| !locator_ok(path))
                || entry
                    .content_hash
                    .as_deref()
                    .is_some_and(|hash| !hash_ok(hash))
                || entry
                    .source_root
                    .as_deref()
                    .is_some_and(|root| root != "project" && root != "lifecycle")
            {
                return Err(SelectedWorkError::InvalidContext);
            }
            Ok(PortableContextEntry {
                id: entry.id.clone(),
                authority: entry.authority.clone(),
                source_root: entry.source_root.clone(),
                path: entry.path.clone(),
                status: entry.status,
                content_hash: entry.content_hash.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if input.readiness.schema != "cairn.readiness.v1"
        || input.readiness.scope.change != input.selection.change
        || input.readiness.scope.policy_hash != input.policy_hash
        || !hash_ok(&input.readiness.scope.source_hash)
        || !hash_ok(&input.readiness.scope.evidence_hash)
        || !hash_ok(&input.readiness.input_hash)
        || !hash_ok(&input.readiness.readiness_policy_hash)
        || !hash_ok(&input.readiness.receipt_hash)
        || input.readiness.stages.len() != READINESS_STAGES.len()
        || !complete_stages(
            input
                .readiness
                .stages
                .iter()
                .map(|stage| stage.stage.as_str()),
        )
        || !input
            .readiness
            .stages
            .iter()
            .all(|stage| hash_ok(&stage.decision_hash))
    {
        return Err(SelectedWorkError::InvalidReadiness);
    }
    if input
        .completion
        .contract_hash
        .as_deref()
        .is_some_and(|hash| !hash_ok(hash))
        || input
            .completion
            .results_hash
            .as_deref()
            .is_some_and(|hash| !hash_ok(hash))
        || input
            .completion
            .subject_hash
            .as_deref()
            .is_some_and(|hash| !hash_ok(hash))
    {
        return Err(SelectedWorkError::InvalidIdentity);
    }
    let readiness = portable_readiness(&input.readiness, &input.local_root)?;
    let mut snapshot = SelectedWorkSnapshot {
        schema: SCHEMA.into(),
        producer: input.producer.clone(),
        selection: input.selection.clone(),
        cohort: CohortIdentity {
            source: String::new(),
            intent: String::new(),
            context: String::new(),
            verification_subject: String::new(),
            dependencies: String::new(),
            obligations: String::new(),
            selection: String::new(),
            observation: String::new(),
        },
        source: input.source.clone(),
        policy_hash: input.policy_hash.clone(),
        profile: input.profile.clone(),
        profile_hash: input.profile_hash.clone(),
        dependencies: input.dependencies.clone(),
        completion: input.completion.clone(),
        context: PortableContext {
            selection_hash: input.context.selection_hash.clone(),
            portable_hash: input.context.portable_hash.clone(),
            entries,
        },
        readiness,
        units: input.units.clone(),
        remaining: input.remaining.clone(),
        enumeration: input.enumeration.clone(),
        semantic_extensions: SemanticExtensions {
            dependency_meanings: SemanticAvailability::Unsupported,
            obligation_explanations: SemanticAvailability::NotSelected,
        },
        obligations: SelectedObligations::NotSelected,
        local: Some(LocalDetail {
            root: input.local_root.clone(),
            context: input.context.clone(),
            readiness: input.readiness.clone(),
        }),
    };
    if let Some(source) = obligation_source {
        // Subject and context identities never depend on the obligation section.
        let bound = cohort(&snapshot);
        snapshot.obligations = obligations::compose_obligations(
            source,
            &input.policy_hash,
            &bound.verification_subject,
            &bound.context,
        )?;
        snapshot.semantic_extensions.obligation_explanations = SemanticAvailability::Supported;
    }
    snapshot.cohort = cohort(&snapshot);
    Ok(snapshot)
}

/// Independent admission of a portable wire value; never derives execution authority.
// r[impl cairn.selected_work.contract]
// r[impl cairn.selected_work.conformance]
pub fn admit_portable(
    policy: &SelectedWorkPolicy,
    snapshot: &SelectedWorkSnapshot,
    expected: &ExpectedCohort,
) -> Result<(), SelectedWorkError> {
    expected_policy(policy)?;
    if snapshot.schema != SCHEMA || snapshot.producer != PRODUCER {
        return Err(SelectedWorkError::UnsupportedSchema);
    }
    if snapshot.local.is_some() {
        return Err(SelectedWorkError::UnsafePortable);
    }
    if !selector_ok(&snapshot.selection.change)
        || snapshot
            .selection
            .task
            .as_deref()
            .is_some_and(|id| !selector_ok(id))
        || !hash_ok(&snapshot.policy_hash)
        || !hash_ok(&snapshot.profile_hash)
        || !selector_ok(&snapshot.profile)
    {
        return Err(SelectedWorkError::InvalidSelection);
    }
    validate_source(policy, &snapshot.source)?;
    check_enumeration(policy, &snapshot.enumeration)?;
    if snapshot.semantic_extensions.dependency_meanings != SemanticAvailability::Unsupported {
        return Err(SelectedWorkError::UnsupportedSchema);
    }
    let identity = cohort(snapshot);
    obligations::admit_obligations(
        &snapshot.obligations,
        snapshot.semantic_extensions.obligation_explanations,
        &snapshot.policy_hash,
        &identity,
    )?;
    if identity.verification_subject != expected.verification_subject {
        return Err(SelectedWorkError::CrossedSubject);
    }
    if &identity != expected || identity != snapshot.cohort {
        return Err(SelectedWorkError::StaleCohort);
    }
    if snapshot.context.entries.len() > policy.max_context_entries
        || !hash_ok(&snapshot.context.selection_hash)
        || !hash_ok(&snapshot.context.portable_hash)
        || snapshot.context.entries.iter().any(|entry| {
            !selector_ok(&entry.id)
                || entry.path.as_deref().is_some_and(|path| !locator_ok(path))
                || entry
                    .content_hash
                    .as_deref()
                    .is_some_and(|hash| !hash_ok(hash))
        })
    {
        return Err(SelectedWorkError::InvalidContext);
    }
    validate_units(
        &snapshot.selection,
        &snapshot.units,
        &snapshot.dependencies,
        &snapshot.remaining,
        policy,
    )?;
    if snapshot.readiness.native_scope.change != snapshot.selection.change
        || snapshot.readiness.native_scope.policy_hash != snapshot.policy_hash
        || !hash_ok(&snapshot.readiness.native_scope.source_hash)
        || !hash_ok(&snapshot.readiness.native_scope.evidence_hash)
        || !hash_ok(&snapshot.readiness.native_input_hash)
        || !hash_ok(&snapshot.readiness.native_policy_hash)
        || !hash_ok(&snapshot.readiness.native_receipt_hash)
        || snapshot.readiness.stages.len() != READINESS_STAGES.len()
        || !complete_stages(
            snapshot
                .readiness
                .stages
                .iter()
                .map(|stage| stage.stage.as_str()),
        )
        || snapshot
            .readiness
            .stages
            .iter()
            .any(|stage| !hash_ok(&stage.decision_hash))
        || snapshot
            .readiness
            .actions
            .iter()
            .any(|action| action.authority != "advisory" || !hash_ok(&action.original_argv_hash))
    {
        return Err(SelectedWorkError::InvalidReadiness);
    }
    let wire = serde_json::to_value(snapshot).map_err(|_| SelectedWorkError::UnsafePortable)?;
    if contains_private(&wire) {
        return Err(SelectedWorkError::UnsafePortable);
    }
    Ok(())
}
fn contains_private(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => {
            text.starts_with('/')
                || text.contains("/home/")
                || text.contains("/Users/")
                || text.contains("file://")
        }
        serde_json::Value::Array(values) => values.iter().any(contains_private),
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            matches!(key.as_str(), "text" | "absolute_path" | "local_root")
                || contains_private(value)
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests;

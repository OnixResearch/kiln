//! Versioned implementation-work admission.
//!
//! This is the Kiln-side boundary between logged intent and a future coded
//! attempt. It admits and ranks work; it never claims, executes, verifies,
//! publishes or completes it. Source adapters provide exact source versions and
//! explicit acceptance obligations; free-form issue text is never interpreted
//! as execution permission.
//!
//! Identities are BLAKE3 over a canonical field encoding, and selection is a
//! pure deterministic function of the admitted ledger, so replay produces the
//! same work versions and ordering.

pub mod attempt;

pub use attempt::*;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const IMPLEMENTATION_WORK_SCHEMA: &str = "kiln.implementation-work.v1";
pub const MAX_TITLE_BYTES: usize = 512;
pub const MAX_OUTCOME_BYTES: usize = 8192;
pub const MAX_OBLIGATION_BYTES: usize = 4096;
pub const MAX_ACCEPTANCE: usize = 64;
pub const MAX_DEPENDENCIES: usize = 64;
pub const MAX_PATHS: usize = 64;
pub const MAX_EFFECTS: usize = 16;
pub const MAX_ALIASES: usize = 16;
pub const MAX_ACTOR_BYTES: usize = 128;
pub const MAX_REPOSITORY_BYTES: usize = 256;
pub const MAX_PATH_BYTES: usize = 512;
pub const MAX_ATTEMPTS_BOUND: u8 = 10;

/// Where one exact work unit was observed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkSourceKind {
    GitHubIssue,
    CairnTask,
}

impl WorkSourceKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::GitHubIssue => "github_issue",
            Self::CairnTask => "cairn_task",
        }
    }
}

/// One exact source observation. `version` is BLAKE3 over the source bytes the
/// adapter admitted; it is not a forge revision or lifecycle authority.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub kind: WorkSourceKind,
    pub source: ExternalRef,
    pub native_id: String,
    pub version: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceObligation {
    pub id: String,
    pub statement: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkBudget {
    /// Maximum implementation attempts a consumer may authorize.
    pub attempts: u8,
    /// Maximum wall-clock minutes per attempt.
    pub minutes_per_attempt: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalPolicy {
    /// A candidate must produce one verified pull request.
    VerifiedPullRequest,
    /// An independently verified already-satisfied result is also terminal.
    VerifiedOrAlreadySatisfied,
}

impl TerminalPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedPullRequest => "verified_pull_request",
            Self::VerifiedOrAlreadySatisfied => "verified_or_already_satisfied",
        }
    }
}

/// Explicit issue-to-task mapping. A linked issue and its task-level units
/// resolve to one canonical key; unrelated work never merges by title.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMapping {
    pub issue: SourceRef,
    pub tasks: Vec<SourceRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationWork {
    pub schema: String,
    /// Monotonic source sequence supplied by the adapter. A lower sequence is
    /// stale; an equal sequence with different content is ambiguous.
    pub revision: u64,
    pub actor: String,
    pub repository: String,
    pub project: String,
    pub title: String,
    pub requested_outcome: String,
    pub acceptance: Vec<AcceptanceObligation>,
    /// Canonical identities of prerequisite work units.
    pub dependencies: Vec<ContentId>,
    /// Repository-relative permitted path globs.
    pub permitted_paths: Vec<String>,
    /// Reviewed effect classes this work may request.
    pub permitted_effects: BTreeSet<String>,
    pub verification_profile: ContentId,
    pub budget: WorkBudget,
    pub terminal_policy: TerminalPolicy,
    pub priority: u8,
    pub aliases: Vec<SourceRef>,
}

impl ImplementationWork {
    /// Canonical work identity over every admitted field except the revision.
    // r[impl kiln.implementation.contract]
    pub fn identity(&self) -> CoreResult<ContentId> {
        let mut bytes = Vec::new();
        push_str(&mut bytes, IMPLEMENTATION_WORK_SCHEMA)?;
        push_str(&mut bytes, &self.repository)?;
        push_str(&mut bytes, &self.project)?;
        push_str(&mut bytes, &self.title)?;
        push_str(&mut bytes, &self.requested_outcome)?;
        let mut acceptance: Vec<&AcceptanceObligation> = self.acceptance.iter().collect();
        acceptance.sort_by(|left, right| left.id.cmp(&right.id));
        for obligation in acceptance {
            push_str(&mut bytes, &obligation.id)?;
            push_str(&mut bytes, &obligation.statement)?;
        }
        let mut dependencies: Vec<&str> = self.dependencies.iter().map(ContentId::as_str).collect();
        dependencies.sort_unstable();
        for dependency in dependencies {
            push_str(&mut bytes, dependency)?;
        }
        let mut paths: Vec<&str> = self.permitted_paths.iter().map(String::as_str).collect();
        paths.sort_unstable();
        for path in paths {
            push_str(&mut bytes, path)?;
        }
        for effect in &self.permitted_effects {
            push_str(&mut bytes, effect)?;
        }
        push_str(&mut bytes, self.verification_profile.as_str())?;
        bytes.push(self.budget.attempts);
        bytes.extend_from_slice(&self.budget.minutes_per_attempt.to_be_bytes());
        push_str(&mut bytes, self.terminal_policy.as_str())?;
        bytes.push(self.priority);
        let mut aliases: Vec<&SourceRef> = self.aliases.iter().collect();
        aliases.sort();
        for alias in aliases {
            push_str(&mut bytes, alias.kind.as_str())?;
            push_str(&mut bytes, alias.source.provider_namespace.as_str())?;
            push_str(&mut bytes, alias.source.native_kind.as_str())?;
            push_str(&mut bytes, alias.source.native_id.as_str())?;
            push_str(&mut bytes, &alias.native_id)?;
            push_str(&mut bytes, alias.version.as_str())?;
        }
        content_identity(&bytes)
    }

    /// Canonical key over the exact source alias set only. Versions of the same
    /// aliases share one key and are ordered by `revision`.
    pub fn canonical_key(&self) -> CoreResult<ContentId> {
        let mut aliases: Vec<&SourceRef> = self.aliases.iter().collect();
        aliases.sort();
        let mut bytes = Vec::new();
        push_str(&mut bytes, IMPLEMENTATION_WORK_SCHEMA)?;
        for alias in aliases {
            push_str(&mut bytes, alias.kind.as_str())?;
            push_str(&mut bytes, alias.source.provider_namespace.as_str())?;
            push_str(&mut bytes, alias.source.native_kind.as_str())?;
            push_str(&mut bytes, alias.source.native_id.as_str())?;
            push_str(&mut bytes, &alias.native_id)?;
            push_str(&mut bytes, alias.version.as_str())?;
        }
        content_identity(&bytes)
    }
}

/// Configured admission boundary. A work unit must name an enrolled repository
/// and actor; enrollment never infers permission from issue text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    pub repositories: BTreeSet<String>,
    pub actors: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum BlockReason {
    MissingDependency { identity: ContentId },
    NoAcceptanceObligations,
    DependencyCycle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Eligibility {
    Ready,
    Blocked { reasons: Vec<BlockReason> },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdmissionDecision {
    Admitted {
        identity: ContentId,
        eligibility: Eligibility,
    },
    Duplicate {
        identity: ContentId,
    },
    Superseded {
        previous: ContentId,
        identity: ContentId,
    },
    /// Delivery is older than the retained version; nothing changed.
    Stale {
        current: ContentId,
    },
}

/// The exact admitted state. `current` holds one version per canonical key;
/// superseded versions remain in history for audit and replay.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkLedger {
    current: BTreeMap<String, ImplementationWork>,
    superseded: Vec<ImplementationWork>,
}

impl WorkLedger {
    pub fn current(&self) -> impl Iterator<Item = &ImplementationWork> {
        self.current.values()
    }

    #[must_use]
    pub fn superseded(&self) -> &[ImplementationWork] {
        &self.superseded
    }

    pub fn get(&self, canonical_key: &ContentId) -> Option<&ImplementationWork> {
        self.current.get(canonical_key.as_str())
    }

    /// Admit one source observation.
    ///
    /// Duplicate and out-of-order observations never dispatch: they either
    /// return the retained identity, are marked stale, or refuse ambiguity.
    // r[impl kiln.implementation.replay] r[impl kiln.implementation.eligibility]
    pub fn admit(
        &mut self,
        work: &ImplementationWork,
        mapping: Option<&SourceMapping>,
        enrollment: &Enrollment,
    ) -> CoreResult<AdmissionDecision> {
        validate_work(work, mapping, enrollment)?;
        let identity = work.identity()?;
        let key = work.canonical_key()?;
        let key_text = key.as_str().to_string();
        let eligibility = self.eligibility(work, &identity)?;
        let Some(existing) = self.current.get(&key_text) else {
            self.current.insert(key_text, work.clone());
            return Ok(AdmissionDecision::Admitted {
                identity,
                eligibility,
            });
        };
        let existing_identity = existing.identity()?;
        match work.revision.cmp(&existing.revision) {
            core::cmp::Ordering::Less => Ok(AdmissionDecision::Stale {
                current: existing_identity,
            }),
            core::cmp::Ordering::Equal => {
                if existing_identity == identity {
                    Ok(AdmissionDecision::Duplicate { identity })
                } else {
                    Err(Diagnostic::new(
                        "implementation_revision_ambiguous",
                        "work.revision",
                        "one source sequence produced two different work versions",
                    ))
                }
            }
            core::cmp::Ordering::Greater => {
                let previous = self.current.insert(key_text, work.clone()).ok_or_else(|| {
                    Diagnostic::new(
                        "implementation_ledger",
                        "work.ledger",
                        "admitted work disappeared from the ledger",
                    )
                })?;
                self.superseded.push(previous);
                Ok(AdmissionDecision::Superseded {
                    previous: existing_identity,
                    identity,
                })
            }
        }
    }

    /// Ready work ordered by reviewed priority, then canonical identity.
    #[must_use]
    pub fn ranked(&self) -> Vec<RankedWork> {
        let mut ready: Vec<RankedWork> = self
            .current
            .values()
            .filter_map(|work| {
                let identity = work.identity().ok()?;
                let key = work.canonical_key().ok()?;
                matches!(self.eligibility(work, &identity).ok()?, Eligibility::Ready).then_some(
                    RankedWork {
                        identity,
                        canonical_key: key,
                        priority: work.priority,
                        terminal_policy: work.terminal_policy,
                    },
                )
            })
            .collect();
        ready.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.canonical_key.cmp(&right.canonical_key))
        });
        ready
    }

    /// Every retained work unit with its visible blocked reasons.
    pub fn eligibility_of(&self, canonical_key: &ContentId) -> Option<Eligibility> {
        let work = self.get(canonical_key)?;
        let identity = work.identity().ok()?;
        self.eligibility(work, &identity).ok()
    }

    fn eligibility(
        &self,
        work: &ImplementationWork,
        _identity: &ContentId,
    ) -> CoreResult<Eligibility> {
        let mut reasons = Vec::new();
        if work.acceptance.is_empty() {
            reasons.push(BlockReason::NoAcceptanceObligations);
        }
        for dependency in &work.dependencies {
            if dependency == &work.identity()? {
                reasons.push(BlockReason::DependencyCycle);
            } else if !self.current.values().any(|candidate| {
                candidate
                    .identity()
                    .is_ok_and(|identity| &identity == dependency)
            }) {
                reasons.push(BlockReason::MissingDependency {
                    identity: dependency.clone(),
                });
            }
        }
        if self.reaches_cycle(work)? {
            reasons.push(BlockReason::DependencyCycle);
        }
        reasons.sort();
        reasons.dedup();
        Ok(if reasons.is_empty() {
            Eligibility::Ready
        } else {
            Eligibility::Blocked { reasons }
        })
    }

    fn reaches_cycle(&self, work: &ImplementationWork) -> CoreResult<bool> {
        let mut pending: Vec<ContentId> = work.dependencies.clone();
        let mut seen = BTreeSet::new();
        let start = work.identity()?;
        while let Some(identity) = pending.pop() {
            if identity == start {
                return Ok(true);
            }
            if !seen.insert(identity.as_str().to_string()) {
                continue;
            }
            if let Some(dependency) = self.current.values().find(|candidate| {
                candidate
                    .identity()
                    .is_ok_and(|candidate_identity| candidate_identity == identity)
            }) {
                pending.extend(dependency.dependencies.iter().cloned());
            }
        }
        Ok(false)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedWork {
    pub identity: ContentId,
    pub canonical_key: ContentId,
    pub priority: u8,
    pub terminal_policy: TerminalPolicy,
}

fn validate_work(
    work: &ImplementationWork,
    mapping: Option<&SourceMapping>,
    enrollment: &Enrollment,
) -> CoreResult<()> {
    if work.schema != IMPLEMENTATION_WORK_SCHEMA {
        return Err(refusal(
            "implementation_schema",
            "work.schema",
            "work schema is unsupported",
        ));
    }
    if !identifier(&work.repository, MAX_REPOSITORY_BYTES)
        || !identifier(&work.project, 100)
        || !identifier(&work.actor, MAX_ACTOR_BYTES)
    {
        return Err(refusal(
            "implementation_identity",
            "work.identity",
            "repository, project or actor is empty or malformed",
        ));
    }
    if !enrollment.repositories.contains(&work.repository)
        || !enrollment.actors.contains(&work.actor)
    {
        return Err(refusal(
            "implementation_unenrolled",
            "work.enrollment",
            "repository or actor is not enrolled for implementation",
        ));
    }
    if !bounded(&work.title, MAX_TITLE_BYTES)
        || !bounded(&work.requested_outcome, MAX_OUTCOME_BYTES)
    {
        return Err(refusal(
            "implementation_text_bound",
            "work.text",
            "work title or requested outcome exceeds its bound",
        ));
    }
    if work.acceptance.len() > MAX_ACCEPTANCE || work.acceptance.is_empty() {
        return Err(refusal(
            "implementation_acceptance_bound",
            "work.acceptance",
            "acceptance obligations are missing or exceed their bound",
        ));
    }
    let mut obligation_ids = BTreeSet::new();
    for obligation in &work.acceptance {
        if !identifier(&obligation.id, 128)
            || !bounded(&obligation.statement, MAX_OBLIGATION_BYTES)
            || !obligation_ids.insert(obligation.id.as_str())
        {
            return Err(refusal(
                "implementation_acceptance",
                "work.acceptance",
                "acceptance obligation is empty, oversized or duplicated",
            ));
        }
    }
    if work.dependencies.len() > MAX_DEPENDENCIES
        || work.permitted_paths.len() > MAX_PATHS
        || work.aliases.is_empty()
        || work.aliases.len() > MAX_ALIASES
        || work.permitted_effects.len() > MAX_EFFECTS
    {
        return Err(refusal(
            "implementation_bound",
            "work.bounds",
            "work list exceeds its reviewed bound",
        ));
    }
    if work.budget.attempts == 0
        || work.budget.attempts > MAX_ATTEMPTS_BOUND
        || work.budget.minutes_per_attempt == 0
    {
        return Err(refusal(
            "implementation_budget",
            "work.budget",
            "work budget is zero or exceeds its reviewed bound",
        ));
    }
    for path in &work.permitted_paths {
        if !relative_path(path) {
            return Err(refusal(
                "implementation_path",
                "work.permitted_paths",
                "permitted path is absolute, escaping or malformed",
            ));
        }
    }
    for effect in &work.permitted_effects {
        if !identifier(effect, 64) {
            return Err(refusal(
                "implementation_effect",
                "work.permitted_effects",
                "permitted effect is empty or malformed",
            ));
        }
    }
    let mut aliases = BTreeSet::new();
    for alias in &work.aliases {
        alias.source.validate()?;
        if !identifier(&alias.native_id, 256) || !aliases.insert(alias) {
            return Err(refusal(
                "implementation_alias",
                "work.aliases",
                "source alias is empty or duplicated",
            ));
        }
    }
    if let Some(mapping) = mapping {
        validate_mapping(mapping, work)?;
    }
    Ok(())
}

fn validate_mapping(mapping: &SourceMapping, work: &ImplementationWork) -> CoreResult<()> {
    // r[impl kiln.implementation.mapping]
    mapping.issue.source.validate()?;
    if mapping.tasks.is_empty() || mapping.tasks.len() > MAX_ALIASES {
        return Err(refusal(
            "implementation_mapping_bound",
            "work.mapping",
            "task mapping is empty or exceeds its bound",
        ));
    }
    let mut expected = BTreeSet::new();
    expected.insert(&mapping.issue);
    for task in &mapping.tasks {
        task.source.validate()?;
        if !expected.insert(task) {
            return Err(refusal(
                "implementation_mapping_duplicate",
                "work.mapping",
                "task mapping repeats one source unit",
            ));
        }
    }
    let actual: BTreeSet<&SourceRef> = work.aliases.iter().collect();
    if actual != expected {
        return Err(refusal(
            "implementation_mapping_forged",
            "work.mapping",
            "source mapping does not match the admitted aliases",
        ));
    }
    Ok(())
}

fn push_str(bytes: &mut Vec<u8>, value: &str) -> CoreResult<()> {
    let length = u32::try_from(value.len()).map_err(|_| {
        refusal(
            "implementation_encoding",
            "work.identity",
            "work field exceeds the encoding bound",
        )
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn content_identity(bytes: &[u8]) -> CoreResult<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string())
}

fn identifier(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !matches!(part, "" | "." | "..") && !part.contains("**"))
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn source(kind: WorkSourceKind, native_id: &str, version: char) -> SourceRef {
        let (namespace, native_kind, id) = match kind {
            WorkSourceKind::GitHubIssue => ("github", "issue", native_id),
            WorkSourceKind::CairnTask => ("cairn", "task", native_id),
        };
        SourceRef {
            kind,
            source: ExternalRef::new(namespace, native_kind, id).unwrap_or_else(|_| unreachable!()),
            native_id: native_id.to_string(),
            version: content(version),
        }
    }

    fn work(aliases: Vec<SourceRef>) -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            title: "Admit implementation work".to_string(),
            requested_outcome: "Implement the bounded work admission".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "the exact admitted work is implemented".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates/kiln-core".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 10,
            aliases,
        }
    }

    fn enrollment() -> Enrollment {
        Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        }
    }

    // r[verify kiln.implementation.replay] r[verify kiln.implementation.replay.scenario.duplicate]
    #[test]
    fn duplicate_and_out_of_order_observations_change_nothing() {
        let mut ledger = WorkLedger::default();
        let mut first = work(vec![source(WorkSourceKind::GitHubIssue, "44", '1')]);
        assert!(matches!(
            ledger.admit(&first, None, &enrollment()),
            Ok(AdmissionDecision::Admitted { .. })
        ));
        let admitted = ledger
            .current()
            .next()
            .cloned()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            ledger.admit(&first, None, &enrollment()),
            Ok(AdmissionDecision::Duplicate {
                identity: first.identity().unwrap_or_else(|_| unreachable!())
            })
        );
        first.revision = 2;
        first.requested_outcome = "different".to_string();
        assert!(matches!(
            ledger.admit(&first, None, &enrollment()),
            Ok(AdmissionDecision::Superseded { .. })
        ));
        first.revision = 1;
        assert!(matches!(
            ledger.admit(&first, None, &enrollment()),
            Ok(AdmissionDecision::Stale { .. })
        ));
        assert_eq!(ledger.current().count(), 1);
        assert_eq!(ledger.superseded().len(), 1);
        assert_eq!(
            ledger.current().next(),
            Some(&{
                let mut newer = admitted;
                newer.revision = 2;
                newer.requested_outcome = "different".to_string();
                newer
            })
        );
    }

    // r[verify kiln.implementation.mapping] r[verify kiln.implementation.mapping.scenario.linked] r[verify kiln.implementation.mapping.scenario.forged]
    #[test]
    fn explicit_mapping_merges_linked_units_and_rejects_forgeries() {
        let issue = source(WorkSourceKind::GitHubIssue, "44", '1');
        let task = source(WorkSourceKind::CairnTask, "change/task-1", '2');
        let mut linked = work(vec![issue.clone(), task.clone()]);
        let mapping = SourceMapping {
            issue: issue.clone(),
            tasks: vec![task.clone()],
        };
        let mut ledger = WorkLedger::default();
        assert!(ledger.admit(&linked, Some(&mapping), &enrollment()).is_ok());
        assert_eq!(ledger.current().count(), 1);

        let unrelated = work(vec![source(WorkSourceKind::CairnTask, "other/task-1", '3')]);
        assert!(ledger.admit(&unrelated, None, &enrollment()).is_ok());
        assert_eq!(ledger.current().count(), 2);

        linked
            .aliases
            .push(source(WorkSourceKind::CairnTask, "forged", '4'));
        assert_eq!(
            ledger
                .admit(&linked, Some(&mapping), &enrollment())
                .map(|_| ()),
            Err(refusal(
                "implementation_mapping_forged",
                "work.mapping",
                "source mapping does not match the admitted aliases",
            ))
        );
    }

    // r[verify kiln.implementation.eligibility] r[verify kiln.implementation.eligibility.scenario.blocked]
    #[test]
    fn missing_dependencies_and_cycles_are_blocked_not_executable() {
        let mut ledger = WorkLedger::default();
        let mut dependent = work(vec![source(WorkSourceKind::GitHubIssue, "45", '1')]);
        dependent.dependencies = vec![content('a')];
        assert!(matches!(
            ledger.admit(&dependent, None, &enrollment()),
            Ok(AdmissionDecision::Admitted {
                eligibility: Eligibility::Blocked { .. },
                ..
            })
        ));
        assert!(ledger.ranked().is_empty());

        let mut cyclic = work(vec![source(WorkSourceKind::GitHubIssue, "46", '1')]);
        cyclic.dependencies = vec![cyclic.identity().unwrap_or_else(|_| unreachable!())];
        assert!(matches!(
            ledger.admit(&cyclic, None, &enrollment()),
            Ok(AdmissionDecision::Admitted {
                eligibility: Eligibility::Blocked { .. },
                ..
            })
        ));
        assert!(ledger.ranked().is_empty());
    }

    // r[verify kiln.implementation.replay] r[verify kiln.implementation.replay.scenario.edit] r[verify kiln.implementation.eligibility.scenario.rank]
    #[test]
    fn edits_supersede_the_old_snapshot_and_selection_is_replayable() {
        let mut ledger = WorkLedger::default();
        let mut edited = work(vec![source(WorkSourceKind::GitHubIssue, "44", '1')]);
        ledger
            .admit(&edited, None, &enrollment())
            .unwrap_or_else(|_| unreachable!());
        let first = edited.identity().unwrap_or_else(|_| unreachable!());
        edited.revision = 2;
        edited.acceptance.push(AcceptanceObligation {
            id: "extra".to_string(),
            statement: "an extra reviewed obligation".to_string(),
        });
        assert_eq!(
            ledger.admit(&edited, None, &enrollment()),
            Ok(AdmissionDecision::Superseded {
                previous: first,
                identity: edited.identity().unwrap_or_else(|_| unreachable!()),
            })
        );
        assert_eq!(ledger.superseded().len(), 1);
        let selection: Vec<_> = ledger
            .ranked()
            .iter()
            .map(|row| row.identity.clone())
            .collect();
        let replay: Vec<_> = ledger
            .ranked()
            .iter()
            .map(|row| row.identity.clone())
            .collect();
        assert_eq!(selection, replay);
        assert_eq!(selection.len(), 1);
    }

    // r[verify kiln.implementation.contract] r[verify kiln.implementation.contract.scenario.refused]
    #[test]
    fn unenrolled_unknown_and_oversized_inputs_are_refused() {
        let mut ledger = WorkLedger::default();
        let mut candidate = work(vec![source(WorkSourceKind::GitHubIssue, "44", '1')]);
        candidate.actor = "stranger".to_string();
        assert!(ledger.admit(&candidate, None, &enrollment()).is_err());
        candidate.actor = "operator".to_string();
        candidate.schema = "kiln.other.v1".to_string();
        assert!(ledger.admit(&candidate, None, &enrollment()).is_err());
        candidate.schema = IMPLEMENTATION_WORK_SCHEMA.to_string();
        candidate.permitted_paths = vec!["../escape".to_string()];
        assert!(ledger.admit(&candidate, None, &enrollment()).is_err());
        candidate.permitted_paths = vec!["crates".to_string()];
        candidate.acceptance.clear();
        assert!(ledger.admit(&candidate, None, &enrollment()).is_err());
        assert_eq!(ledger.current().count(), 0);
    }

    // r[verify kiln.implementation.contract] r[verify kiln.implementation.contract.scenario.admit] r[verify kiln.implementation.replay.scenario.lifecycle]
    #[test]
    fn archive_readiness_is_not_part_of_work_admission() {
        // The Cairn adapter supplies only ready task identities and versions.
        // Archive readiness is a separate lifecycle field and is deliberately
        // absent from this contract, so an implementation-ready task can be
        // admitted while archive readiness stays false.
        let cairn = work(vec![source(WorkSourceKind::CairnTask, "change/task", '1')]);
        let mut ledger = WorkLedger::default();
        assert!(matches!(
            ledger.admit(&cairn, None, &enrollment()),
            Ok(AdmissionDecision::Admitted {
                eligibility: Eligibility::Ready,
                ..
            })
        ));
    }
}

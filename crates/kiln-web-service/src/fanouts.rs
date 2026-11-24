//! Reviewed runbook fan-out DTOs, deterministic target expansion, and fleet write scopes.

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::fleet::{FleetScope, contains_scope, parse_scope};
use kiln_core::implementation::{
    FanOutRun, FanoutAdmissionState, FanoutChild, FanoutChildStatus, FanoutClaimPolicy,
    FanoutCounts, FanoutMode, FanoutPhase, MAX_FANOUT_GLOBS, MAX_FANOUT_TARGETS, glob_matches,
    valid_target_glob,
};
use kiln_core::source::SourceResolution;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile::{ProjectProfile, ServiceProfile};
use crate::runbooks::{RunbookDefinition, RunbookMode, is_hex40, match_project, parse_target};

pub const FANOUT_DISPATCH_RESULT_SCHEMA: &str = "kiln.runbook-fanout-result.v1";
pub const FANOUT_CONTROL_RESULT_SCHEMA: &str = "kiln.runbook-fanout-control-result.v1";
pub const FANOUT_LIST_SCHEMA: &str = "kiln.runbook-fanouts-view.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanoutDispatchRequest {
    pub request_id: String,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub target_globs: Vec<String>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    #[serde(default)]
    pub max_concurrency: Option<u16>,
    #[serde(default)]
    pub budget_cap: Option<u64>,
    #[serde(default)]
    pub claim_policy: Option<FanoutClaimPolicy>,
    #[serde(default)]
    pub scope_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanoutControlRequest {
    pub request_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FanoutControlsView {
    pub pause: bool,
    #[serde(rename = "continue")]
    pub can_continue: bool,
    pub cancel: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FanoutChildView {
    pub index: u16,
    pub project_id: String,
    pub repository: String,
    pub target: String,
    pub resolved_reference: String,
    pub source_revision: String,
    pub write_scope: Option<String>,
    pub status: FanoutChildStatus,
    pub run_id: Option<String>,
    pub outcome_detail: Option<String>,
    pub reason: Option<String>,
    pub admitted_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FanoutView {
    pub id: String,
    pub ordinal: u64,
    pub runbook_id: String,
    pub mode: FanoutMode,
    pub principal: String,
    pub request_id: String,
    pub target_selectors: Vec<String>,
    pub claim_policy: FanoutClaimPolicy,
    pub admission: FanoutAdmissionState,
    pub phase: FanoutPhase,
    pub stop_reason: Option<String>,
    pub max_concurrency: u16,
    pub active_children: usize,
    pub budget_cap: u64,
    pub budget_drawn: u64,
    pub budget_remaining: u64,
    pub unit_cost: u64,
    pub admitted_runs: u32,
    pub lattice_bundle_id: Option<String>,
    pub lattice_bundle_identity: String,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params_identity: String,
    pub params: BTreeMap<String, Value>,
    pub controls: FanoutControlsView,
    pub counts: FanoutCounts,
    pub children: Vec<FanoutChildView>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FanoutDispatchResponse {
    pub schema: &'static str,
    pub request_id: String,
    pub replayed: bool,
    pub fanout_id: String,
    pub fanout: FanoutView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FanoutControlResponse {
    pub schema: &'static str,
    pub request_id: String,
    pub replayed: bool,
    pub action: &'static str,
    pub fanout_id: String,
    pub fanout: FanoutView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FanoutPage {
    pub schema: &'static str,
    pub fanouts: Vec<FanoutView>,
    pub next_cursor: Option<String>,
    pub total: usize,
    pub bounded: bool,
}

#[must_use]
pub fn fanout_mode(mode: RunbookMode) -> FanoutMode {
    match mode {
        RunbookMode::ReadOnly => FanoutMode::ReadOnly,
        RunbookMode::Mutating => FanoutMode::Mutating,
    }
}

#[must_use]
pub fn fanout_view(run: &FanOutRun) -> FanoutView {
    let counts = run.counts();
    let has_open_work = counts.pending > 0 || counts.active > 0;
    let controls = FanoutControlsView {
        pause: has_open_work && run.admission == FanoutAdmissionState::Open,
        can_continue: has_open_work && run.admission == FanoutAdmissionState::Paused,
        cancel: has_open_work && run.admission != FanoutAdmissionState::Cancelled,
    };
    let children = run
        .children
        .iter()
        .map(|child| FanoutChildView {
            index: child.index,
            project_id: child.project_id.clone(),
            repository: child.repository.clone(),
            target: child.target.clone(),
            resolved_reference: child.resolved_reference.clone(),
            source_revision: child.source_revision.clone(),
            write_scope: child.write_scope.as_ref().map(FleetScope::text),
            status: child.status,
            run_id: child.run_id.clone(),
            outcome_detail: child.outcome_detail.clone(),
            reason: child.reason.clone(),
            admitted_at_ms: child.admitted_at_ms,
            completed_at_ms: child.completed_at_ms,
        })
        .collect();
    FanoutView {
        id: run.id.clone(),
        ordinal: run.ordinal,
        runbook_id: run.runbook_id.clone(),
        mode: run.mode,
        principal: run.principal_id.clone(),
        request_id: run.request_id.clone(),
        target_selectors: run.target_selectors.clone(),
        claim_policy: run.claim_policy,
        admission: run.admission,
        phase: run.phase,
        stop_reason: run.stop_reason.clone(),
        max_concurrency: run.max_concurrency,
        active_children: counts.active,
        budget_cap: run.budget.ceiling_units,
        budget_drawn: run.budget.drawn_units,
        budget_remaining: run.budget.remaining_units(),
        unit_cost: run.budget.unit_cost,
        admitted_runs: run.budget.admitted_runs,
        lattice_bundle_id: run.lattice_bundle_id.clone(),
        lattice_bundle_identity: run.lattice_bundle_identity.clone(),
        lattice_workflow_name: run.lattice_workflow_name.clone(),
        lattice_workflow_revision: run.lattice_workflow_revision.clone(),
        params_identity: run.params_identity.clone(),
        params: run.params.clone(),
        controls,
        counts,
        children,
        created_at_ms: run.created_at_ms,
        updated_at_ms: run.updated_at_ms,
        completed_at_ms: run.completed_at_ms,
    }
}

pub(crate) fn valid_fanout_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[derive(Clone, Debug)]
struct ExpandedTargetSpec<'a> {
    target_rule: &'a crate::runbooks::RunbookTarget,
    project: &'a ProjectProfile,
    requested_ref: Option<String>,
}

pub(crate) type ExpandedFanout = Result<(Vec<String>, Vec<FanoutChild>), (u16, &'static str)>;

pub(crate) struct ExpandTargetRequest<'a> {
    pub(crate) runbook: &'a RunbookDefinition,
    pub(crate) service: &'a ServiceProfile,
    pub(crate) enrolled: &'a BTreeSet<String>,
    pub(crate) targets: &'a [String],
    pub(crate) target_globs: &'a [String],
    pub(crate) scope_path: Option<&'a str>,
}

pub(crate) fn expand_fanout_children(
    req: ExpandTargetRequest<'_>,
    resolve_ref_source: impl Fn(&ProjectProfile, &str) -> Result<SourceResolution, (u16, &'static str)>,
    resolve_sha_source: impl Fn(
        &ProjectProfile,
        &crate::runbooks::RunbookTarget,
        &str,
    ) -> Result<SourceResolution, (u16, &'static str)>,
    allow_ref: impl Fn(&crate::runbooks::RunbookTarget, &str) -> Option<String>,
) -> ExpandedFanout {
    let ExpandTargetRequest {
        runbook,
        service,
        enrolled,
        targets,
        target_globs,
        scope_path,
    } = req;
    if runbook
        .fanout
        .as_ref()
        .is_some_and(|config| !config.enabled)
    {
        return Err((409, "fanout_disabled"));
    }
    if targets.is_empty() && target_globs.is_empty() {
        return Err((400, "fanout_targets_empty"));
    }
    let max_targets = runbook
        .fanout
        .as_ref()
        .map_or(MAX_FANOUT_TARGETS, |config| config.max_targets)
        .min(MAX_FANOUT_TARGETS);
    if targets.len() > max_targets {
        return Err((400, "fanout_targets_bound"));
    }
    if target_globs.len() > MAX_FANOUT_GLOBS {
        return Err((400, "fanout_glob_invalid"));
    }
    if targets.len().saturating_add(target_globs.len()) > max_targets {
        return Err((400, "fanout_targets_bound"));
    }
    if runbook.mode == RunbookMode::ReadOnly && scope_path.is_some() {
        return Err((400, "fanout_scope_invalid"));
    }

    let mut selectors = Vec::with_capacity(targets.len() + target_globs.len());
    let mut candidates: Vec<ExpandedTargetSpec<'_>> = Vec::new();

    for raw in targets {
        if raw.is_empty()
            || raw.len() > 1280
            || raw
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err((400, "runbook_target_invalid"));
        }
        let (selector, requested_ref) = if raw.contains('@') {
            let (repo, raw_ref) = parse_target(raw).map_err(|code| (400, code))?;
            (repo, Some(raw_ref.to_owned()))
        } else {
            if raw.len() > 256 {
                return Err((400, "runbook_target_invalid"));
            }
            (raw.as_str(), None)
        };
        if selector.contains('*') || selector.contains('?') {
            return Err((400, "runbook_target_invalid"));
        }
        let Some(target_rule) = runbook.allowed_targets.iter().find(|rule| {
            rule.repository.eq_ignore_ascii_case(selector) || rule.project_id == selector
        }) else {
            return Err((400, "runbook_target_not_allowed"));
        };
        let Some(project) = match_project(service, &target_rule.project_id) else {
            return Err((404, "project_not_allowlisted"));
        };
        if !enrolled.contains(&project.id) {
            return Err((409, "project_not_enrolled"));
        }
        selectors.push(raw.clone());
        candidates.push(ExpandedTargetSpec {
            target_rule,
            project,
            requested_ref,
        });
    }

    for raw_glob in target_globs {
        if raw_glob.is_empty()
            || raw_glob.len() > 1280
            || raw_glob
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err((400, "fanout_glob_invalid"));
        }
        let (pattern, requested_ref) = if let Some((pat, raw_ref)) = raw_glob.split_once('@') {
            if raw_ref.is_empty()
                || raw_ref.len() > 1024
                || raw_ref.contains('@')
                || raw_ref.starts_with('-')
                || raw_ref.contains("..")
            {
                return Err((400, "fanout_glob_invalid"));
            }
            if is_hex40(raw_ref) {
                return Err((400, "runbook_target_not_allowed"));
            }
            (pat, Some(raw_ref.to_owned()))
        } else {
            (raw_glob.as_str(), None)
        };
        if !valid_target_glob(pattern) {
            return Err((400, "fanout_glob_invalid"));
        }
        let mut matched_allowed = 0usize;
        let mut matched_enrolled = 0usize;
        for target_rule in &runbook.allowed_targets {
            if !glob_matches(pattern, &target_rule.repository, &target_rule.project_id) {
                continue;
            }
            matched_allowed += 1;
            if !enrolled.contains(&target_rule.project_id) {
                continue;
            }
            let Some(project) = match_project(service, &target_rule.project_id) else {
                return Err((404, "project_not_allowlisted"));
            };
            matched_enrolled += 1;
            candidates.push(ExpandedTargetSpec {
                target_rule,
                project,
                requested_ref: requested_ref.clone(),
            });
        }
        if matched_enrolled == 0 {
            if matched_allowed > 0 {
                return Err((409, "project_not_enrolled"));
            }
            return Err((400, "fanout_targets_empty"));
        }
        selectors.push(raw_glob.clone());
    }

    struct ResolvedEntry {
        project_id: String,
        repository: String,
        resolved_reference: String,
        target: String,
        source_revision: String,
        source: SourceResolution,
        write_scope: Option<FleetScope>,
    }

    let mut by_project: BTreeMap<String, ResolvedEntry> = BTreeMap::new();
    for spec in candidates {
        let (resolved_reference, source) = match spec.requested_ref.as_deref() {
            Some(raw_ref) if is_hex40(raw_ref) => {
                if !spec.target_rule.allow_commit_sha {
                    return Err((400, "runbook_target_not_allowed"));
                }
                let src = resolve_sha_source(spec.project, spec.target_rule, raw_ref)?;
                (raw_ref.to_owned(), src)
            }
            other => {
                let candidate_ref = other.unwrap_or(spec.project.reference.as_str());
                let Some(admitted_ref) = allow_ref(spec.target_rule, candidate_ref) else {
                    return Err((400, "runbook_target_not_allowed"));
                };
                let src = resolve_ref_source(spec.project, &admitted_ref)?;
                (admitted_ref, src)
            }
        };
        let write_scope = if runbook.mode == RunbookMode::Mutating {
            Some(derive_write_scope(
                runbook,
                spec.target_rule,
                spec.project,
                scope_path,
            )?)
        } else {
            None
        };
        let source_revision = source.exact_revision.native_id.as_str().to_owned();
        let target_label = format!("{}@{}", spec.target_rule.repository, resolved_reference);
        let entry = ResolvedEntry {
            project_id: spec.project.id.clone(),
            repository: spec.target_rule.repository.clone(),
            resolved_reference,
            target: target_label,
            source_revision,
            source,
            write_scope,
        };
        if let Some(existing) = by_project.get(&entry.project_id) {
            if existing.resolved_reference != entry.resolved_reference
                || existing.source_revision != entry.source_revision
                || existing.write_scope != entry.write_scope
            {
                return Err((400, "fanout_duplicate_target"));
            }
            continue;
        }
        by_project.insert(entry.project_id.clone(), entry);
    }

    if by_project.is_empty() {
        return Err((400, "fanout_targets_empty"));
    }
    if by_project.len() > max_targets {
        return Err((400, "fanout_targets_bound"));
    }

    let mut ordered: Vec<ResolvedEntry> = by_project.into_values().collect();
    ordered.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.repository.cmp(&right.repository))
            .then_with(|| left.resolved_reference.cmp(&right.resolved_reference))
    });

    let mut children = Vec::with_capacity(ordered.len());
    for (idx, entry) in ordered.into_iter().enumerate() {
        let index = u16::try_from(idx).map_err(|_| (400, "fanout_targets_bound"))?;
        children.push(FanoutChild {
            index,
            target: entry.target,
            repository: entry.repository,
            project_id: entry.project_id,
            resolved_reference: entry.resolved_reference,
            source_revision: entry.source_revision,
            source: entry.source,
            write_scope: entry.write_scope,
            status: FanoutChildStatus::Pending,
            run_id: None,
            outcome_detail: None,
            reason: None,
            admitted_at_ms: None,
            completed_at_ms: None,
        });
    }
    Ok((selectors, children))
}

fn derive_write_scope(
    runbook: &RunbookDefinition,
    target_rule: &crate::runbooks::RunbookTarget,
    project: &ProjectProfile,
    scope_path: Option<&str>,
) -> Result<FleetScope, (u16, &'static str)> {
    if !target_rule.repository.contains('/') {
        return Err((400, "fanout_scope_invalid"));
    }
    let project_root = project
        .scope
        .as_ref()
        .map_or(".", |scope| scope.root.as_str());
    let project_scope = parse_scope(&target_rule.repository, project_root)
        .map_err(|_| (400, "fanout_scope_invalid"))?;

    if let Some(requested) = scope_path {
        let requested_scope = parse_scope(&target_rule.repository, requested)
            .map_err(|_| (400, "fanout_scope_invalid"))?;
        if !contains_scope(&project_scope, &requested_scope) {
            return Err((400, "fanout_scope_widening"));
        }
        if let Some(fanout) = &runbook.fanout
            && !fanout.write_scopes.is_empty()
        {
            let allowed = fanout.write_scopes.iter().any(|declared| {
                parse_scope(&target_rule.repository, declared)
                    .is_ok_and(|declared_scope| contains_scope(&declared_scope, &requested_scope))
            });
            if !allowed {
                return Err((400, "fanout_scope_widening"));
            }
        }
        return Ok(requested_scope);
    }

    if let Some(fanout) = &runbook.fanout
        && !fanout.write_scopes.is_empty()
    {
        if fanout.write_scopes.len() == 1
            && let Some(single) = fanout.write_scopes.first()
        {
            let declared_scope = parse_scope(&target_rule.repository, single)
                .map_err(|_| (400, "fanout_scope_invalid"))?;
            if contains_scope(&project_scope, &declared_scope) {
                return Ok(declared_scope);
            }
            if contains_scope(&declared_scope, &project_scope) {
                return Ok(project_scope);
            }
            return Err((400, "fanout_scope_widening"));
        }
        return Err((400, "fanout_scope_required"));
    }
    Ok(project_scope)
}

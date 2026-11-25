use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;

use super::api::{error, json_response};
use super::fanouts::{FleetClaimsSnapshot, conflict_summary};
use super::model::{RequestKind, RunRecord};
use super::queue::{CandidateSource, resolve_source};
use super::{ApiResponse, Engine, Role, host_time_ms, identity};
use crate::access::Principal;
use crate::fanouts::{ExpandTargetRequest, expand_fanout_children};
use crate::profile::ProjectProfile;
use crate::runbooks::{
    ApprovalGate, DispatchRunbookRequest, RUNBOOK_DISPATCH_RESULT_SCHEMA, RUNBOOK_PREVIEW_SCHEMA,
    RUNBOOKS_VIEW_SCHEMA, RunbookDefinition, RunbookDispatchResponse, RunbookListResponse,
    RunbookMode, RunbookPreviewBudget, RunbookPreviewEdge, RunbookPreviewNode,
    RunbookPreviewRequest, RunbookPreviewResponse, RunbookPreviewTarget, RunbookRegistry,
    RunbookRunBinding, RunbookTarget, is_hex40, match_project, parse_target, resolve_params,
    resolve_target_rule, valid_runbook_id, valid_write_scope_path,
};
use kiln_core::implementation::{
    MAX_FANOUT_BUDGET_UNITS, MAX_FANOUT_CONCURRENCY, find_claim_overlap,
};

impl Engine {
    pub fn configure_runbooks(&mut self, registry: RunbookRegistry) -> Result<(), String> {
        registry.admit_for_service(&self.profile)?;
        if let Some(dir) = &self.runbooks_dir {
            crate::runbooks::bundle::verify_registry_bundles(dir, &registry)?;
        }
        self.runbook_profile = Some(registry);
        self.recover_stale_summary_notifications()?;
        Ok(())
    }

    pub fn configure_runbooks_path(&mut self, path: &Path) -> Result<(), String> {
        let registry = RunbookRegistry::load(path)?;
        registry.admit_for_service(&self.profile)?;
        if let Some(dir) = &self.runbooks_dir {
            crate::runbooks::bundle::verify_registry_bundles(dir, &registry)?;
        }
        self.runbooks_path = Some(path.to_path_buf());
        self.runbook_profile = Some(registry);
        self.recover_stale_summary_notifications()?;
        Ok(())
    }

    pub fn configure_runbooks_dir(&mut self, dir: &Path) -> Result<(), String> {
        if let Some(registry) = self.current_runbooks()? {
            crate::runbooks::bundle::verify_registry_bundles(dir, &registry)?;
        }
        self.runbooks_dir = Some(dir.to_path_buf());
        Ok(())
    }

    pub(super) fn verify_configured_runbook_bundle(
        &self,
        runbook: &RunbookDefinition,
    ) -> Result<Result<bool, ApiResponse>, String> {
        let Some(dir) = &self.runbooks_dir else {
            return Ok(Ok(false));
        };
        match crate::runbooks::bundle::verify_runbook_bundle(dir, runbook) {
            Ok(_) => Ok(Ok(true)),
            Err(err) => {
                let (status, code) = err.status_and_code();
                Ok(Err(error(status, code)?))
            }
        }
    }

    pub(super) fn current_runbooks(&self) -> Result<Option<RunbookRegistry>, String> {
        if let Some(path) = &self.runbooks_path {
            let registry = RunbookRegistry::load(path)?;
            registry.admit_for_service(&self.profile)?;
            return Ok(Some(registry));
        }
        Ok(self.runbook_profile.clone())
    }

    pub(super) fn authorize_runbook_principal(
        &self,
        id: &str,
        principal: &Principal,
    ) -> Result<Result<RunbookDefinition, ApiResponse>, String> {
        let registry = match self.current_runbooks() {
            Ok(Some(registry)) => registry,
            Ok(None) => return Ok(Err(error(404, "runbook_not_found")?)),
            Err(_) => return Ok(Err(error(503, "runbooks_unavailable")?)),
        };
        let Some(runbook) = registry.find(id).cloned() else {
            return Ok(Err(error(404, "runbook_not_found")?));
        };
        let now_ms = host_time_ms()?;
        if let Err(code) = registry.permits(principal, &runbook, now_ms) {
            return Ok(Err(error(403, code)?));
        }
        Ok(Ok(runbook))
    }

    fn validate_runbook_rerun_definition(
        &self,
        project: &ProjectProfile,
        prior: &RunRecord,
        rb: &RunbookRunBinding,
        runbook: &RunbookDefinition,
    ) -> Result<Option<ApiResponse>, String> {
        if runbook.mode != rb.mode || runbook.approval_gate != rb.approval_gate {
            return error(409, "runbook_mode_changed").map(Some);
        }
        let Ok((repo, raw_ref)) = parse_target(&rb.target) else {
            return error(409, "runbook_target_not_allowed").map(Some);
        };
        let Some(target_rule) = resolve_target_rule(runbook, repo) else {
            return error(409, "runbook_target_not_allowed").map(Some);
        };
        if target_rule.project_id != prior.project_id {
            return error(409, "runbook_target_not_allowed").map(Some);
        }
        if is_hex40(raw_ref) {
            if !target_rule.allow_commit_sha {
                return error(409, "runbook_target_not_allowed").map(Some);
            }
            if let Err((status, code)) = resolve_sha_target_source(project, target_rule, raw_ref) {
                return error(status, code).map(Some);
            }
        } else if target_allows_reference(target_rule, raw_ref).is_none() {
            return error(409, "runbook_target_not_allowed").map(Some);
        }

        if runbook.lattice.bundle_id != rb.lattice_bundle_id
            || runbook.lattice.bundle_identity != rb.lattice_bundle_identity
            || runbook.lattice.workflow_name != rb.lattice_workflow_name
            || runbook.lattice.workflow_revision != rb.lattice_workflow_revision
        {
            return error(409, "runbook_definition_changed").map(Some);
        }
        let supplied: BTreeMap<String, serde_json::Value> = rb
            .params
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        let Ok(resolved) = resolve_params(&runbook.params, &supplied) else {
            return error(409, "runbook_definition_changed").map(Some);
        };
        if resolved.identity != rb.params_identity || resolved.env_bindings != rb.env_bindings {
            return error(409, "runbook_definition_changed").map(Some);
        }
        Ok(None)
    }

    pub(super) fn authorize_runbook_rerun(
        &self,
        principal: &Principal,
        project: &ProjectProfile,
        prior: &RunRecord,
        rb: &RunbookRunBinding,
    ) -> Result<Option<ApiResponse>, String> {
        let runbook = match self.authorize_runbook_principal(&rb.runbook_id, principal)? {
            Ok(runbook) => runbook,
            Err(response) => return Ok(Some(response)),
        };
        if let Err(response) = self.verify_configured_runbook_bundle(&runbook)? {
            return Ok(Some(response));
        }
        self.validate_runbook_rerun_definition(project, prior, rb, &runbook)
    }

    pub(super) fn list_runbooks(&self, principal: &Principal) -> Result<ApiResponse, String> {
        let registry = match self.current_runbooks() {
            Ok(Some(registry)) => registry,
            Ok(None) => {
                return json_response(
                    200,
                    RunbookListResponse {
                        schema: RUNBOOKS_VIEW_SCHEMA,
                        configured: false,
                        principal: principal.id.clone(),
                        runbooks: Vec::new(),
                    },
                );
            }
            Err(_) => return error(503, "runbooks_unavailable"),
        };
        let now_ms = host_time_ms()?;
        json_response(200, registry.list_view(principal, now_ms))
    }

    pub(super) fn dispatch_runbook(
        &mut self,
        id: &str,
        body: &[u8],
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        if !valid_runbook_id(id) {
            return error(400, "runbook_id_invalid");
        }
        let runbook = match self.authorize_runbook_principal(id, principal)? {
            Ok(runbook) => runbook,
            Err(response) => return Ok(response),
        };
        if let Err(response) = self.verify_configured_runbook_bundle(&runbook)? {
            return Ok(response);
        }

        let req: DispatchRunbookRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "request_invalid"),
        };
        if !crate::profile::identifier(&req.request_id) {
            return error(400, "request_id_invalid");
        }
        let fingerprint = identity(&("kiln.runbook-dispatch.v1", id, &principal.id, &req))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }

        let (repo, raw_ref) = match parse_target(&req.target) {
            Ok(parsed) => parsed,
            Err(code) => return error(400, code),
        };
        let Some(target_rule) = resolve_target_rule(&runbook, repo) else {
            return error(400, "runbook_target_not_allowed");
        };
        let Some(project) = match_project(&self.profile, &target_rule.project_id).cloned() else {
            return error(404, "project_not_allowlisted");
        };
        if !self.state.enrolled.contains(&project.id) {
            return error(409, "project_not_enrolled");
        }

        let source = if is_hex40(raw_ref) {
            if !target_rule.allow_commit_sha {
                return error(400, "runbook_target_not_allowed");
            }
            match resolve_sha_target_source(&project, target_rule, raw_ref) {
                Ok(source) => source,
                Err((status, code)) => return error(status, code),
            }
        } else {
            let Some(resolved_ref) = target_allows_reference(target_rule, raw_ref) else {
                return error(400, "runbook_target_not_allowed");
            };
            match resolve_source(&project, &resolved_ref, None) {
                Ok(source) => source,
                Err(code) => {
                    return error(
                        if code == "source_unavailable" {
                            503
                        } else {
                            400
                        },
                        &code,
                    );
                }
            }
        };

        let resolved_params = match resolve_params(&runbook.params, &req.params) {
            Ok(resolved) => resolved,
            Err(err) => return error(400, err.code),
        };

        let mut source_env = BTreeMap::new();
        source_env.insert("LATTICE_SOURCE_PROVIDER".to_owned(), "git".to_owned());
        source_env.insert(
            "LATTICE_SOURCE_REPOSITORY".to_owned(),
            source.repository.native_id.as_str().to_owned(),
        );
        source_env.insert(
            "LATTICE_SOURCE_REVISION".to_owned(),
            source.exact_revision.native_id.as_str().to_owned(),
        );

        let binding = RunbookRunBinding {
            runbook_id: runbook.id.clone(),
            mode: runbook.mode,
            approval_gate: runbook.approval_gate,
            target: req.target.clone(),
            lattice_bundle_id: runbook.lattice.bundle_id.clone(),
            lattice_bundle_identity: runbook.lattice.bundle_identity.clone(),
            lattice_workflow_name: runbook.lattice.workflow_name.clone(),
            lattice_workflow_revision: runbook.lattice.workflow_revision.clone(),
            params_identity: resolved_params.identity,
            params: resolved_params.values,
            env_bindings: resolved_params.env_bindings,
            source_env,
        };

        let request_id = req.request_id.clone();
        let affected = crate::monorepo::manual_affected(&project)?;
        self.enqueue_inner(
            &project,
            &request_id,
            fingerprint,
            CandidateSource {
                resolution: source,
                dirty_snapshot: None,
            },
            None,
            Vec::new(),
            affected,
            Some(binding.target.clone()),
            None,
            None,
            None,
            Some(binding),
            RequestKind::RunbookDispatch,
            |run| {
                let Some(rb) = run.runbook.as_ref() else {
                    return Err("runbook binding missing".into());
                };
                serde_json::to_value(dispatch_response(&request_id, false, run, rb))
                    .map_err(|_| "runbook dispatch encoding failed".into())
            },
        )
    }

    pub(super) fn preview_runbook(
        &self,
        id: &str,
        body: &[u8],
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        if !valid_runbook_id(id) {
            return error(400, "runbook_id_invalid");
        }
        match principal.role {
            Role::Worker => return error(403, "worker_scope"),
            Role::Viewer => return error(403, "operator_required"),
            Role::Operator => {}
        }
        let registry = match self.current_runbooks() {
            Ok(Some(registry)) => registry,
            Ok(None) => return error(404, "runbook_not_found"),
            Err(_) => return error(503, "runbooks_unavailable"),
        };
        let Some(runbook) = registry.find(id).cloned() else {
            return error(404, "runbook_not_found");
        };
        let bundle_pin_verified = match self.verify_configured_runbook_bundle(&runbook)? {
            Ok(verified) => verified,
            Err(response) => return Ok(response),
        };

        let req: RunbookPreviewRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "request_invalid"),
        };
        let single_mode = req.target.is_some();
        let fanout_mode = !req.targets.is_empty() || !req.target_globs.is_empty();
        if single_mode == fanout_mode {
            return error(400, "request_invalid");
        }
        if let Some(scope) = req.scope_path.as_deref()
            && (runbook.mode == RunbookMode::ReadOnly || !valid_write_scope_path(scope))
        {
            return error(400, "fanout_scope_invalid");
        }
        let resolved_params = match resolve_params(&runbook.params, &req.params) {
            Ok(resolved) => resolved,
            Err(err) => return error(400, err.code),
        };

        let now_ms = host_time_ms()?;
        let (can_dispatch, dispatch_refusal) = match registry.permits(principal, &runbook, now_ms) {
            Ok(()) => (true, None),
            Err(code) => (false, Some(code)),
        };

        let max_runs_available = self
            .profile
            .bounds
            .max_runs
            .saturating_sub(self.state.runs.len());

        let (targets, budget) = if let Some(raw_target) = req.target.as_deref() {
            let (repo, raw_ref) = match parse_target(raw_target) {
                Ok(parsed) => parsed,
                Err(code) => return error(400, code),
            };
            let Some(target_rule) = resolve_target_rule(&runbook, repo) else {
                return error(400, "runbook_target_not_allowed");
            };
            let Some(project) = match_project(&self.profile, &target_rule.project_id).cloned()
            else {
                return error(404, "project_not_allowlisted");
            };
            if !self.state.enrolled.contains(&project.id) {
                return error(409, "project_not_enrolled");
            }
            let (resolved_reference, source) = if is_hex40(raw_ref) {
                if !target_rule.allow_commit_sha {
                    return error(400, "runbook_target_not_allowed");
                }
                match resolve_sha_target_source(&project, target_rule, raw_ref) {
                    Ok(source) => (raw_ref.to_owned(), source),
                    Err((status, code)) => return error(status, code),
                }
            } else {
                let Some(resolved_ref) = target_allows_reference(target_rule, raw_ref) else {
                    return error(400, "runbook_target_not_allowed");
                };
                match resolve_source(&project, &resolved_ref, None) {
                    Ok(source) => (resolved_ref, source),
                    Err(code) => {
                        return error(
                            if code == "source_unavailable" {
                                503
                            } else {
                                400
                            },
                            &code,
                        );
                    }
                }
            };
            let (derived_scope, write_scopes) = if runbook.mode == RunbookMode::ReadOnly {
                (None, Vec::new())
            } else if let Some(requested_scope) = req.scope_path.as_deref() {
                let scope = match crate::fanouts::derive_write_scope(
                    &runbook,
                    target_rule,
                    &project,
                    Some(requested_scope),
                ) {
                    Ok(scope) => scope,
                    Err((status, code)) => return error(status, code),
                };
                let text = scope.text();
                (Some(scope), vec![text])
            } else if let Some(cfg) = &runbook.fanout
                && !cfg.write_scopes.is_empty()
            {
                let scopes = cfg
                    .write_scopes
                    .iter()
                    .map(|p| {
                        if target_rule.repository.contains('/') {
                            format!("{} {p}", target_rule.repository)
                        } else {
                            p.clone()
                        }
                    })
                    .collect();
                (None, scopes)
            } else {
                let root = project.scope.as_ref().map_or(".", |s| s.root.as_str());
                let scopes = if target_rule.repository.contains('/') {
                    vec![format!("{} {root}", target_rule.repository)]
                } else {
                    vec![root.to_owned()]
                };
                (None, scopes)
            };
            let claim_conflict = if runbook.mode == RunbookMode::Mutating {
                match &self.fleet_claims {
                    FleetClaimsSnapshot::Unavailable => return error(503, "fleet_unavailable"),
                    FleetClaimsSnapshot::Unconfigured => None,
                    FleetClaimsSnapshot::Available(claims) => {
                        let mut conflict_found = None;
                        if let Some(scope) = derived_scope.as_ref() {
                            if let Some(conflict) = find_claim_overlap(scope, claims, now_ms) {
                                conflict_found = Some(conflict_summary(&conflict));
                            }
                        } else if target_rule.repository.contains('/') {
                            let scope_paths: Vec<&str> = if let Some(cfg) = &runbook.fanout
                                && !cfg.write_scopes.is_empty()
                            {
                                cfg.write_scopes.iter().map(String::as_str).collect()
                            } else {
                                vec![project.scope.as_ref().map_or(".", |s| s.root.as_str())]
                            };
                            for sp in scope_paths {
                                if let Ok(scope) =
                                    kiln_core::fleet::parse_scope(&target_rule.repository, sp)
                                    && let Some(conflict) =
                                        find_claim_overlap(&scope, claims, now_ms)
                                {
                                    conflict_found = Some(conflict_summary(&conflict));
                                    break;
                                }
                            }
                        }
                        conflict_found
                    }
                }
            } else {
                None
            };
            let preview_target = RunbookPreviewTarget {
                project_id: project.id,
                repository: target_rule.repository.clone(),
                target: raw_target.to_owned(),
                resolved_reference,
                source_revision: source.exact_revision.native_id.as_str().to_owned(),
                write_scopes,
                claim_conflict,
            };
            let budget = RunbookPreviewBudget {
                max_concurrency: 1,
                budget_cap: 1,
                unit_cost: 1,
                estimated_units: 1,
                within_budget: max_runs_available >= 1,
                max_runs_available,
            };
            (vec![preview_target], budget)
        } else {
            let max_concurrency = req.max_concurrency.unwrap_or_else(|| {
                runbook
                    .fanout
                    .as_ref()
                    .map_or(1, |cfg| cfg.default_max_concurrency)
            });
            let max_concurrency_limit = runbook
                .fanout
                .as_ref()
                .map_or(MAX_FANOUT_CONCURRENCY, |cfg| cfg.max_concurrency_limit);
            if max_concurrency == 0 || max_concurrency > max_concurrency_limit {
                return error(400, "fanout_concurrency_invalid");
            }
            let budget_cap = req.budget_cap.unwrap_or_else(|| {
                runbook
                    .fanout
                    .as_ref()
                    .map_or(16, |cfg| cfg.default_budget_cap)
            });
            let max_budget_cap = runbook
                .fanout
                .as_ref()
                .map_or(MAX_FANOUT_BUDGET_UNITS, |cfg| cfg.max_budget_cap);
            if budget_cap == 0 || budget_cap > max_budget_cap {
                return error(400, "fanout_budget_invalid");
            }
            let (_selectors, children) = match expand_fanout_children(
                ExpandTargetRequest {
                    runbook: &runbook,
                    service: &self.profile,
                    enrolled: &self.state.enrolled,
                    targets: &req.targets,
                    target_globs: &req.target_globs,
                    scope_path: req.scope_path.as_deref(),
                },
                |project, reference| {
                    resolve_source(project, reference, None).map_err(|code| {
                        let status = if code == "source_unavailable" {
                            503
                        } else {
                            400
                        };
                        let static_code = match code.as_str() {
                            "source_unavailable" => "source_unavailable",
                            "source_reference_not_admitted" => "source_reference_not_admitted",
                            "source_revision_invalid" => "source_revision_invalid",
                            "source_revision_unreachable" => "source_revision_unreachable",
                            _ => "source_unavailable",
                        };
                        (status, static_code)
                    })
                },
                resolve_sha_target_source,
                target_allows_reference,
            ) {
                Ok(expanded) => expanded,
                Err((status, code)) => return error(status, code),
            };
            let live_claims = if runbook.mode == RunbookMode::Mutating {
                match &self.fleet_claims {
                    FleetClaimsSnapshot::Unavailable => return error(503, "fleet_unavailable"),
                    FleetClaimsSnapshot::Unconfigured => &[][..],
                    FleetClaimsSnapshot::Available(claims) => claims.as_slice(),
                }
            } else {
                &[][..]
            };
            let preview_targets: Vec<RunbookPreviewTarget> = children
                .iter()
                .map(|child| {
                    let claim_conflict = if runbook.mode == RunbookMode::Mutating
                        && let Some(scope) = &child.write_scope
                    {
                        find_claim_overlap(scope, live_claims, now_ms)
                            .map(|conflict| conflict_summary(&conflict))
                    } else {
                        None
                    };
                    let write_scopes = child
                        .write_scope
                        .as_ref()
                        .map(|s| vec![s.text()])
                        .unwrap_or_default();
                    RunbookPreviewTarget {
                        project_id: child.project_id.clone(),
                        repository: child.repository.clone(),
                        target: child.target.clone(),
                        resolved_reference: child.resolved_reference.clone(),
                        source_revision: child.source_revision.clone(),
                        write_scopes,
                        claim_conflict,
                    }
                })
                .collect();
            let estimated_units = preview_targets.len() as u64;
            let budget = RunbookPreviewBudget {
                max_concurrency,
                budget_cap,
                unit_cost: 1,
                estimated_units,
                within_budget: estimated_units <= budget_cap,
                max_runs_available,
            };
            (preview_targets, budget)
        };

        let nodes = runbook
            .lattice
            .nodes
            .iter()
            .map(|n| RunbookPreviewNode {
                id: n.id.clone(),
                kind: n.kind.clone(),
                effect_class: format!("{:?}", n.effect_class).to_ascii_lowercase(),
                adapter_kind: n.adapter_kind.clone(),
                operation_identity: n.operation_identity.clone(),
            })
            .collect();
        let edges = runbook
            .lattice
            .edges
            .iter()
            .map(|e| RunbookPreviewEdge {
                source: e.source.clone(),
                target: e.target.clone(),
            })
            .collect();

        let mut non_claims = vec![
            "Dry-run preview performs zero state mutation, reserves no request_id, and dispatches no workflow execution.",
            "Preview grants no execution, approval, merge, or release authority (grants_authority: false).",
        ];
        if runbook.approval_gate == ApprovalGate::KilnPublication {
            non_claims.push(
                "Mutating steps run without GitHub push or publisher credentials; PR/tag publication requires explicit approval and Kiln's separately credentialed publisher.",
            );
        }

        json_response(
            200,
            RunbookPreviewResponse {
                schema: RUNBOOK_PREVIEW_SCHEMA,
                dry_run: true,
                grants_authority: false,
                runbook_id: runbook.id,
                mode: runbook.mode,
                approval_gate: runbook.approval_gate,
                can_dispatch,
                dispatch_refusal,
                bundle_pin_verified,
                bundle_id: runbook.lattice.bundle_id,
                bundle_identity: runbook.lattice.bundle_identity,
                workflow_name: runbook.lattice.workflow_name,
                workflow_revision: runbook.lattice.workflow_revision,
                nodes,
                edges,
                params_identity: resolved_params.identity,
                params: resolved_params.values,
                env_bindings: resolved_params.env_bindings,
                targets,
                budget,
                non_claims,
            },
        )
    }
}

pub(super) fn dispatch_response(
    request_id: &str,
    replayed: bool,
    run: &super::model::RunRecord,
    rb: &RunbookRunBinding,
) -> RunbookDispatchResponse {
    RunbookDispatchResponse {
        schema: RUNBOOK_DISPATCH_RESULT_SCHEMA,
        request_id: request_id.to_owned(),
        replayed,
        runbook_id: rb.runbook_id.clone(),
        mode: rb.mode,
        approval_gate: rb.approval_gate,
        target: rb.target.clone(),
        project_id: run.project_id.clone(),
        source_revision: run.source.exact_revision.native_id.as_str().to_owned(),
        lattice_bundle_id: rb.lattice_bundle_id.clone(),
        lattice_bundle_identity: rb.lattice_bundle_identity.clone(),
        lattice_workflow_name: rb.lattice_workflow_name.clone(),
        lattice_workflow_revision: rb.lattice_workflow_revision.clone(),
        params_identity: rb.params_identity.clone(),
        params: rb.params.clone(),
        env_bindings: rb.env_bindings.clone(),
        source_env: rb.source_env.clone(),
        run_id: run.id.clone(),
        run: run.view(),
    }
}

pub(super) fn replay_response_with_flag(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("replayed".into(), json!(true));
    }
    value
}

pub(super) fn target_allows_reference(
    target_rule: &RunbookTarget,
    raw_ref: &str,
) -> Option<String> {
    target_rule
        .references
        .iter()
        .find(|r| r.as_str() == raw_ref)
        .cloned()
        .or_else(|| {
            (!raw_ref.starts_with("refs/"))
                .then(|| format!("refs/heads/{raw_ref}"))
                .filter(|candidate| target_rule.references.iter().any(|r| r == candidate))
        })
}

pub(super) fn resolve_sha_target_source(
    project: &ProjectProfile,
    target_rule: &RunbookTarget,
    raw_ref: &str,
) -> Result<kiln_core::source::SourceResolution, (u16, &'static str)> {
    let mut unavailable = false;
    let mut last_error = "source_revision_unreachable";
    for candidate_ref in &target_rule.references {
        match resolve_source(project, candidate_ref, Some(raw_ref)) {
            Ok(source) => return Ok(source),
            Err(code) => {
                if code == "source_unavailable" {
                    unavailable = true;
                } else if !unavailable {
                    last_error = "source_revision_unreachable";
                }
            }
        }
    }
    if unavailable {
        Err((503, "source_unavailable"))
    } else {
        Err((400, last_error))
    }
}

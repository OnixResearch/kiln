use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;

use super::api::{error, json_response};
use super::model::{RequestKind, RunRecord};
use super::queue::{CandidateSource, resolve_source};
use super::{ApiResponse, Engine, host_time_ms, identity};
use crate::access::Principal;
use crate::profile::ProjectProfile;
use crate::runbooks::{
    DispatchRunbookRequest, RUNBOOK_DISPATCH_RESULT_SCHEMA, RUNBOOKS_VIEW_SCHEMA,
    RunbookDefinition, RunbookDispatchResponse, RunbookListResponse, RunbookRegistry,
    RunbookRunBinding, RunbookTarget, is_hex40, match_project, parse_target, resolve_params,
    resolve_target_rule, valid_runbook_id,
};

impl Engine {
    pub fn configure_runbooks(&mut self, registry: RunbookRegistry) -> Result<(), String> {
        registry.admit_for_service(&self.profile)?;
        self.runbook_profile = Some(registry);
        Ok(())
    }

    pub fn configure_runbooks_path(&mut self, path: &Path) -> Result<(), String> {
        let registry = RunbookRegistry::load(path)?;
        registry.admit_for_service(&self.profile)?;
        self.runbooks_path = Some(path.to_path_buf());
        self.runbook_profile = Some(registry);
        Ok(())
    }

    fn current_runbooks(&self) -> Result<Option<RunbookRegistry>, String> {
        if let Some(path) = &self.runbooks_path {
            let registry = RunbookRegistry::load(path)?;
            registry.admit_for_service(&self.profile)?;
            return Ok(Some(registry));
        }
        Ok(self.runbook_profile.clone())
    }

    fn authorize_runbook_principal(
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
        if runbook.mode != rb.mode {
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

fn target_allows_reference(target_rule: &RunbookTarget, raw_ref: &str) -> Option<String> {
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

fn resolve_sha_target_source(
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

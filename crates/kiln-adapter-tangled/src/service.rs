//! Spindle-compatible service: XRPC handling and the run worker.
//!
//! The service composes the pure core with injected shell ports: the replay
//! store, exact source acquisition, a Kiln `WorkflowPort`, a status publisher
//! and an issuer key resolver. Any uncertainty in a port is reported as
//! retryable or `Unknown`, never as success.

use kiln_provider_api::{PortFailure, PortResult, WorkflowPort};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{Expectation, VerifiedClaims, bearer, parse_token, verify_token};
use crate::error::{Rejection, RejectionClass, TangledResult, infrastructure};
use crate::events::{Admission, AdmittedEvent, admit_notify, admit_trigger};
use crate::identity::{valid_did, valid_record_key};
use crate::nsid;
use crate::pipeline::{
    PipelineRecord, PlanOutcome, PublicationPhase, RunPhase, approve, not_found, owned,
    pipeline_message, pipeline_view, plan_pipeline, plan_retry, refresh_publication, view_delivery,
    workflow_view,
};
use crate::profile::{AdmittedProfile, RepositoryPin};
use crate::resolver::KeyResolver;
use crate::source::{AcquireRequest, SourcePort};
use crate::status::{StatusDelivery, StatusPort};
use crate::store::{Insert, SqliteStore, TokenClaim};
use crate::workflows::review;
use crate::xrpc::{Method, XrpcRequest, XrpcResponse};

/// Longest `ref` parameter accepted by listWorkflowDefinitions (lexicon bound).
const MAXIMUM_REF_PARAMETER_BYTES: usize = 256;
const DEFAULT_REFERENCE: &str = "HEAD";

#[derive(Debug)]
pub struct TangledService<S, W, P, K> {
    profile: AdmittedProfile,
    store: SqliteStore,
    source: S,
    workflow: W,
    status: P,
    keys: K,
    cursor: i64,
}

#[derive(Deserialize)]
struct PipelineInput {
    repo: String,
    pipeline: String,
    #[serde(default)]
    workflows: Option<Vec<String>>,
}

fn port_rejection(failure: &PortFailure, code: &'static str) -> Rejection {
    Rejection::from_port(failure, code, "A Tangled provider port failed")
}

fn to_port(rejection: &Rejection) -> PortFailure {
    infrastructure(rejection.code, rejection.message)
}

impl<S: SourcePort, W: WorkflowPort, P: StatusPort, K: KeyResolver> TangledService<S, W, P, K> {
    pub fn new(
        profile: AdmittedProfile,
        store: SqliteStore,
        source: S,
        workflow: W,
        status: P,
        keys: K,
    ) -> Self {
        Self {
            profile,
            store,
            source,
            workflow,
            status,
            keys,
            cursor: 0,
        }
    }

    pub fn store(&self) -> &SqliteStore {
        &self.store
    }

    pub fn workflow(&self) -> &W {
        &self.workflow
    }

    // r[impl kiln.tangled.xrpc]
    pub fn handle(&mut self, request: &XrpcRequest, now: i64) -> XrpcResponse {
        let procedure = matches!(
            request.nsid.as_str(),
            nsid::NOTIFY_EVENT
                | nsid::TRIGGER_PIPELINE
                | nsid::APPROVE_PIPELINE
                | nsid::RETRY_PIPELINE
        );
        let query = matches!(
            request.nsid.as_str(),
            nsid::LIST_WORKFLOWS | nsid::GET_WORKFLOW | nsid::GET_PIPELINE
        );
        if !procedure && !query {
            return XrpcResponse::named(
                501,
                "MethodNotImplemented",
                "Method is not served by this Kiln spindle",
            );
        }
        if (procedure && request.method != Method::Post) || (query && request.method != Method::Get)
        {
            return XrpcResponse::named(
                405,
                "InvalidRequest",
                "HTTP method does not match the lexicon",
            );
        }
        let result = match request.nsid.as_str() {
            nsid::NOTIFY_EVENT => self.notify(request, now),
            nsid::TRIGGER_PIPELINE => self.trigger(request, now),
            nsid::APPROVE_PIPELINE => self.approve(request, now),
            nsid::RETRY_PIPELINE => self.retry(request, now),
            nsid::LIST_WORKFLOWS => self.list_workflows(request),
            nsid::GET_WORKFLOW => self.get_workflow(request),
            _ => self.get_pipeline(request),
        };
        match result {
            Ok(value) => XrpcResponse::ok(&value),
            Err(rejection) => XrpcResponse::error(&rejection),
        }
    }

    fn known_issuer(&self, did: &str) -> bool {
        self.profile.profile.repositories.iter().any(|pin| {
            pin.knot_did == did
                || pin
                    .trusted_actors
                    .iter()
                    .chain(&pin.approvers)
                    .any(|actor| actor == did)
        })
    }

    // r[impl kiln.tangled.auth] r[impl kiln.tangled.replay]
    fn authenticate(
        &mut self,
        request: &XrpcRequest,
        method: &str,
        now: i64,
    ) -> TangledResult<VerifiedClaims> {
        let token = bearer(request.authorization.as_deref())?;
        let parsed = parse_token(token)?;
        // No document is fetched for issuers the profile does not name.
        if !self.known_issuer(&parsed.issuer) {
            return Err(Rejection::forbidden(
                "tangled_auth_issuer",
                "Token issuer is not pinned by this spindle",
            ));
        }
        let key = self
            .keys
            .resolve(&parsed.issuer, now)
            .map_err(|failure| match failure {
                PortFailure::Domain(_) => Rejection::authentication(
                    "tangled_auth_key",
                    "Issuer key could not be established",
                ),
                _ => Rejection::unavailable(
                    "tangled_auth_key_unavailable",
                    "Issuer key resolution is unavailable",
                ),
            })?;
        let limits = &self.profile.profile.limits;
        let claims = verify_token(
            &parsed,
            &key,
            &Expectation {
                audience: &self.profile.profile.spindle_did,
                method,
                now_unix: now,
                maximum_lifetime_seconds: limits.maximum_token_lifetime_seconds,
                clock_skew_seconds: limits.clock_skew_seconds,
            },
        )?;
        let expires = claims
            .expires_unix
            .saturating_add(limits.clock_skew_seconds);
        match self
            .store
            .claim_token(&claims.issuer, &claims.jti, expires, now)
        {
            Ok(TokenClaim::Fresh) => Ok(claims),
            Ok(TokenClaim::Replayed) => Err(Rejection::authentication(
                "tangled_auth_replay",
                "Token was already used",
            )),
            Err(failure) => Err(port_rejection(&failure, "tangled_auth_replay_store")),
        }
    }

    fn notify(&mut self, request: &XrpcRequest, now: i64) -> TangledResult<Value> {
        let claims = self.authenticate(request, nsid::NOTIFY_EVENT, now)?;
        let admission = admit_notify(&self.profile, &claims, &request.body)?;
        self.schedule(admission, now)
    }

    fn trigger(&mut self, request: &XrpcRequest, now: i64) -> TangledResult<Value> {
        let claims = self.authenticate(request, nsid::TRIGGER_PIPELINE, now)?;
        let admission = admit_trigger(&self.profile, &claims, &request.body)?;
        self.schedule(admission, now)
    }

    fn pin(&self, repo_did: &str) -> TangledResult<RepositoryPin> {
        if !valid_did(repo_did) {
            return Err(Rejection::invalid(
                "tangled_query_repo",
                "Repository DID is invalid",
            ));
        }
        self.profile
            .repository(repo_did)
            .cloned()
            .ok_or(Rejection::new(
                RejectionClass::NotFound("RepoNotFound"),
                "tangled_repo_unknown",
                "Repository DID is not served by this Kiln spindle",
            ))
    }

    fn scheduled(pipeline_id: &str, message: String) -> Value {
        json!({ "ci": { "pipelines": [pipeline_id], "message": message } })
    }

    // r[impl kiln.tangled.events] r[impl kiln.tangled.replay] r[impl kiln.tangled.source]
    fn schedule(&mut self, admission: Admission, now: i64) -> TangledResult<Value> {
        let event: Box<AdmittedEvent> = match admission {
            Admission::Ignored(reason) => {
                return Ok(json!({ "ci": { "message": format!("kiln: {reason}") } }));
            }
            Admission::Event(event) => event,
        };
        let replayed = self
            .store
            .find_replay(event.replay_key.as_str())
            .map_err(|failure| port_rejection(&failure, "tangled_replay_lookup"))?;
        if let Some(existing) = replayed {
            let message = format!("kiln: replay of pipeline {existing}");
            return Ok(Self::scheduled(&existing, message));
        }
        let pin = self.pin(&event.repo_did)?;
        let source = self
            .source
            .acquire(&AcquireRequest {
                repo_did: &event.repo_did,
                clone_url: &pin.clone_url,
                commit: &event.commit,
                base: event.base.as_ref(),
            })
            .map_err(|failure| port_rejection(&failure, "tangled_acquisition"))?;
        let mut record = match plan_pipeline(&self.profile, &event, &source, now)? {
            PlanOutcome::NoMatch { message } => return Ok(json!({ "ci": { "message": message } })),
            PlanOutcome::Pipeline(record) => *record,
        };
        refresh_publication(&mut record)?;
        let message = pipeline_message(&record);
        match self
            .store
            .insert(&record)
            .map_err(|failure| port_rejection(&failure, "tangled_store_insert"))?
        {
            Insert::Inserted => Ok(Self::scheduled(&record.pipeline_id, message)),
            Insert::Existing(existing) => Ok(Self::scheduled(
                &existing,
                format!("kiln: replay of pipeline {existing}"),
            )),
        }
    }

    fn load_owned(&self, input: &PipelineInput) -> TangledResult<(RepositoryPin, PipelineRecord)> {
        if !valid_record_key(&input.pipeline) {
            return Err(Rejection::invalid(
                "tangled_pipeline_id",
                "Pipeline id is not a record key",
            ));
        }
        let pin = self.pin(&input.repo)?;
        let record = self
            .store
            .load(&input.pipeline)
            .map_err(|failure| port_rejection(&failure, "tangled_store_load"))?
            .ok_or_else(not_found)?;
        owned(&record, &pin.repo_did)?;
        Ok((pin, record))
    }

    fn pipeline_input(request: &XrpcRequest) -> TangledResult<PipelineInput> {
        serde_json::from_slice(&request.body)
            .map_err(|_| Rejection::invalid("tangled_body", "Request body is malformed"))
    }

    // r[impl kiln.tangled.untrusted]
    fn approve(&mut self, request: &XrpcRequest, now: i64) -> TangledResult<Value> {
        let claims = self.authenticate(request, nsid::APPROVE_PIPELINE, now)?;
        let input = Self::pipeline_input(request)?;
        if input.workflows.is_some() {
            return Err(Rejection::invalid(
                "tangled_body",
                "approvePipeline takes no workflows",
            ));
        }
        let (pin, mut record) = self.load_owned(&input)?;
        approve(&mut record, &pin, &claims.issuer, now)?;
        refresh_publication(&mut record)?;
        self.store
            .save(&record)
            .map_err(|failure| port_rejection(&failure, "tangled_store_save"))?;
        Ok(json!({}))
    }

    // r[impl kiln.tangled.xrpc]
    fn retry(&mut self, request: &XrpcRequest, now: i64) -> TangledResult<Value> {
        let claims = self.authenticate(request, nsid::RETRY_PIPELINE, now)?;
        let input = Self::pipeline_input(request)?;
        let (pin, mut original) = self.load_owned(&input)?;
        let workflows = input.workflows.clone().unwrap_or_default();
        let mut retry = plan_retry(
            &self.profile,
            &mut original,
            &pin,
            &claims.issuer,
            &workflows,
            now,
        )?;
        refresh_publication(&mut retry)?;
        match self
            .store
            .insert(&retry)
            .map_err(|failure| port_rejection(&failure, "tangled_store_insert"))?
        {
            Insert::Inserted => {
                self.store
                    .save(&original)
                    .map_err(|failure| port_rejection(&failure, "tangled_store_save"))?;
                Ok(json!({ "pipeline": retry.pipeline_id }))
            }
            Insert::Existing(existing) => Ok(json!({ "pipeline": existing })),
        }
    }

    fn definitions(
        &mut self,
        request: &XrpcRequest,
        reference: &str,
    ) -> TangledResult<(String, String, Vec<Value>)> {
        let pin = self.pin(request.required("repo")?)?;
        let commit = self
            .source
            .resolve(&pin.repo_did, &pin.clone_url, reference)
            .map_err(|failure| port_rejection(&failure, "tangled_resolve"))?
            .ok_or(Rejection::new(
                RejectionClass::NotFound("RefNotFound"),
                "tangled_ref_unknown",
                "No branch or tag of the repository has this name",
            ))?;
        let source = self
            .source
            .acquire(&AcquireRequest {
                repo_did: &pin.repo_did,
                clone_url: &pin.clone_url,
                commit: &commit,
                base: None,
            })
            .map_err(|failure| port_rejection(&failure, "tangled_acquisition"))?;
        let limits = &self.profile.profile.limits;
        let views = source
            .files
            .iter()
            .filter_map(|file| {
                review(
                    &file.path,
                    &file.bytes,
                    &self.profile.profile.engines,
                    limits.maximum_workflow_bytes,
                )
                .ok()
            })
            .map(|workflow| {
                (
                    workflow.id.clone(),
                    workflow_view(&pin.repo_did, commit.as_str(), &workflow),
                )
            })
            .map(|(_, view)| view)
            .collect();
        Ok((pin.repo_did, commit.as_str().to_string(), views))
    }

    fn list_workflows(&mut self, request: &XrpcRequest) -> TangledResult<Value> {
        let reference = request
            .param("ref")?
            .unwrap_or(DEFAULT_REFERENCE)
            .to_string();
        if reference.is_empty() || reference.len() > MAXIMUM_REF_PARAMETER_BYTES {
            return Err(Rejection::invalid(
                "tangled_query_ref",
                "ref parameter is empty or too long",
            ));
        }
        let (_, _, views) = self.definitions(request, &reference)?;
        Ok(json!({ "workflows": views }))
    }

    fn get_workflow(&mut self, request: &XrpcRequest) -> TangledResult<Value> {
        let name = request.required("workflow")?.to_string();
        let (_, _, views) = self.definitions(request, DEFAULT_REFERENCE)?;
        views
            .into_iter()
            .find(|view| view.get("id").and_then(Value::as_str) == Some(name.as_str()))
            .map(|view| json!({ "workflow": view }))
            .ok_or(Rejection::new(
                RejectionClass::NotFound("WorkflowNotFound"),
                "tangled_workflow_unknown",
                "Workflow not found",
            ))
    }

    // r[impl kiln.tangled.status]
    fn get_pipeline(&mut self, request: &XrpcRequest) -> TangledResult<Value> {
        let input = PipelineInput {
            repo: request.required("repo")?.to_string(),
            pipeline: request.required("pipeline")?.to_string(),
            workflows: None,
        };
        let (_, record) = self.load_owned(&input)?;
        Ok(pipeline_view(&record))
    }

    /// Advance one pipeline with work. `Ok(false)` when none is left this pass.
    pub fn tick(&mut self, now: i64) -> PortResult<bool> {
        let Some((sequence, pipeline_id)) = self.store.next_work(self.cursor)? else {
            self.cursor = 0;
            return Ok(false);
        };
        self.cursor = sequence;
        let Some(mut record) = self.store.load(&pipeline_id)? else {
            return Ok(true);
        };
        self.advance_runs(&mut record, now)?;
        refresh_publication(&mut record).map_err(|rejection| to_port(&rejection))?;
        self.store.save(&record)?;
        self.advance_publication(&mut record)?;
        Ok(true)
    }

    // r[impl kiln.tangled.source] r[impl kiln.tangled.untrusted]
    fn advance_runs(&mut self, record: &mut PipelineRecord, now: i64) -> PortResult<()> {
        if crate::pipeline::held(record) {
            return Ok(());
        }
        let limits = self.profile.profile.limits;
        for index in 0..record.workflows.len() {
            let Some(phase) = record.workflows.get(index).map(|run| run.phase) else {
                continue;
            };
            let result = match phase {
                RunPhase::Terminal | RunPhase::Unresolved => continue,
                RunPhase::Queued => {
                    if self.store.active_runs()? >= limits.maximum_active_runs {
                        continue;
                    }
                    if let Some(run) = record.workflows.get_mut(index) {
                        run.phase = RunPhase::DispatchReserved;
                    }
                    self.store.save(record)?;
                    let Some(run) = record.workflows.get(index) else {
                        continue;
                    };
                    self.workflow.dispatch(&run.plan)
                }
                RunPhase::DispatchReserved | RunPhase::Active | RunPhase::Unknown => {
                    let Some(run) = record.workflows.get_mut(index) else {
                        continue;
                    };
                    if run.polls >= limits.maximum_workflow_polls {
                        run.exhaust().map_err(|rejection| to_port(&rejection))?;
                        self.store.save(record)?;
                        continue;
                    }
                    run.polls += 1;
                    if phase == RunPhase::DispatchReserved
                        || (phase == RunPhase::Unknown && run.observation.is_none())
                    {
                        self.workflow.recover_dispatch(&run.plan)
                    } else if phase == RunPhase::Unknown {
                        self.workflow.reconcile(&run.plan.effect_id)
                    } else {
                        self.workflow.observe(&run.plan.effect_id)
                    }
                }
            };
            if let Some(run) = record.workflows.get_mut(index) {
                run.accept(result, now)
                    .map_err(|rejection| to_port(&rejection))?;
            }
            self.store.save(record)?;
        }
        Ok(())
    }

    // r[impl kiln.tangled.status]
    fn advance_publication(&mut self, record: &mut PipelineRecord) -> PortResult<()> {
        let maximum_attempts = self.profile.profile.status.maximum_attempts;
        let Some(index) = record.publications.iter().rposition(|publication| {
            matches!(
                publication.phase,
                PublicationPhase::Ready | PublicationPhase::Reserved | PublicationPhase::Unknown
            )
        }) else {
            return Ok(());
        };
        let (bytes, identity) = view_delivery(record).map_err(|rejection| to_port(&rejection))?;
        let Some(publication) = record.publications.get_mut(index) else {
            return Ok(());
        };
        if publication.request_identity != identity {
            publication.phase = PublicationPhase::Superseded;
            return self.store.save(record);
        }
        if publication.attempts >= maximum_attempts {
            publication.phase = PublicationPhase::Failed;
            publication.error_code = Some("tangled_status_attempts".to_string());
            return self.store.save(record);
        }
        publication.phase = PublicationPhase::Reserved;
        publication.attempts += 1;
        self.store.save(record)?;
        let delivery = StatusDelivery {
            pipeline_id: record.pipeline_id.clone(),
            request_identity: identity,
            body: bytes,
        };
        let result = self.status.publish(&delivery);
        let Some(publication) = record.publications.get_mut(index) else {
            return Ok(());
        };
        match result {
            Ok(()) => {
                publication.phase = PublicationPhase::Published;
                publication.error_code = None;
            }
            Err(failure) => {
                publication.error_code = Some(crate::error::port_code(&failure).to_string());
                publication.phase = match failure {
                    PortFailure::Domain(_) => PublicationPhase::Failed,
                    PortFailure::Unknown(_) => PublicationPhase::Unknown,
                    // Not delivered: the same request is sent again.
                    _ => PublicationPhase::Ready,
                };
            }
        }
        self.store.save(record)
    }
}

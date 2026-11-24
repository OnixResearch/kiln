//! Canonical execution observations admitted as review CI evidence.

use kiln_core::change_review::{CiCheck, CiFact};
use kiln_core::domain::Outcome;
use kiln_core::workflow::WorkflowNodeState;

use super::Engine;
impl Engine {
    /// Run one explicitly configured native Nix build as a new canonical CI
    /// run. The caller selects only the already retained source run and
    /// idempotency key; the Nix profile and admitted BuildEffectPlan come from
    /// trusted service configuration and this method persists the intent before
    /// the provider is dispatched.
    pub fn run_canonical_nix_build(
        &mut self,
        source_run_id: &str,
        request_id: &str,
    ) -> Result<String, String> {
        use kiln_core::{
            build::{BuildEffectPlan, validate_build_effect_plan},
            domain::{Command, Outcome, State},
            types::{
                Alias, AttemptId, EffectId, PipelineRevisionId, PolicyId, ProviderProfileId, RunId,
            },
            workflow::{WorkflowEffectClass, WorkflowNode},
        };
        use kiln_provider_api::BuildPort;

        if self.storage_failed || !crate::profile::identifier(request_id) {
            return Err("canonical_nix_build_unavailable_or_request_invalid".into());
        }
        let (source, source_candidate, project_id, config) = {
            let source_run = self
                .state
                .runs
                .get(source_run_id)
                .ok_or("canonical_nix_build_source_missing")?;
            if source_run.phase != "terminal"
                || source_run.outcome() != Outcome::Success
                || !source_run.selected_nodes.is_empty()
                || source_run.dirty_snapshot.is_some()
                || source_run.untrusted.is_some()
                || super::verification::validate_record_provenance(source_run).is_err()
            {
                return Err("canonical_nix_build_source_not_clean_success".into());
            }
            let observation = source_run
                .observation
                .as_ref()
                .ok_or("canonical_nix_build_source_observation_missing")?;
            let project = self
                .profile
                .projects
                .iter()
                .find(|p| p.id == source_run.project_id)
                .ok_or("canonical_nix_build_project_missing")?;
            let config = project
                .canonical_nix_build
                .as_ref()
                .ok_or("canonical_nix_build_not_opted_in")?;
            if source_run.source.exact_revision.native_id.as_str() != config.source_revision {
                return Err("canonical_nix_build_source_revision_mismatch".into());
            }
            (
                source_run.source.clone(),
                observation.candidate_id.clone(),
                project.id.clone(),
                config.clone(),
            )
        };
        let profile_identity = super::identity(&config.profile)?;
        let operation_identity =
            crate::profile::canonical_nix_build_operation_identity(&project_id, &config)?;
        let pipeline_identity = super::identity(&(
            "kiln.web.canonical-nix-build.pipeline.v1",
            &project_id,
            &config,
        ))?;
        let run_identity = super::identity(&(
            "kiln.web.canonical-nix-build.run.v1",
            &self.profile.service_id,
            &project_id,
            source_run_id,
            request_id,
            &profile_identity,
            &operation_identity,
            &source,
        ))?;
        let run_id = format!("web-nix-build-{}", run_identity.as_str());
        let build_profile = config
            .profile
            .clone()
            .into_profile()
            .map_err(|failure| format!("canonical_nix_build_profile: {failure:?}"))?;
        let mut adapter = kiln_adapter_nix::NixBuildAdapter::new(
            build_profile,
            kiln_adapter_nix::BoundedNixBuildRuntime,
        )
        .map_err(|failure| format!("canonical_nix_build_adapter: {failure:?}"))?;
        adapter
            .preflight()
            .map_err(|failure| format!("canonical_nix_build_preflight: {failure:?}"))?;
        let effect_id = EffectId::new(format!("web-effect-{}", run_identity.as_str()))
            .map_err(|e| e.message)?;
        let attempt_id = AttemptId::new(format!("web-attempt-{}", run_identity.as_str()))
            .map_err(|e| e.message)?;
        let candidate_id = source_candidate;
        let pipeline_revision_id =
            PipelineRevisionId::new(format!("web-pipeline-{}", pipeline_identity.as_str()))
                .map_err(|e| e.message)?;
        let provider_profile = ProviderProfileId::new(config.profile.provider_profile_id.clone())
            .map_err(|e| e.message)?;
        let workflow_policy_id = PolicyId::new(format!(
            "nix-build-{}",
            config.profile.policy_identity.as_str()
        ))
        .map_err(|e| e.message)?;
        let platform = Alias::new(config.profile.platform.clone()).map_err(|e| e.message)?;
        let source_context = self
            .state
            .runs
            .get(source_run_id)
            .and_then(|run| run.plan.source.clone())
            .ok_or("canonical_nix_build_source_context_missing")?;
        let bounds = self
            .state
            .runs
            .get(source_run_id)
            .ok_or("canonical_nix_build_source_missing")?
            .plan
            .bounds;
        let workflow_plan = kiln_core::workflow::WorkflowEffectPlan {
            effect_id: effect_id.clone(),
            attempt_id: attempt_id.clone(),
            candidate_id: candidate_id.clone(),
            pipeline_revision_id: pipeline_revision_id.clone(),
            workflow_policy_id: workflow_policy_id.clone(),
            provider_profile: provider_profile.clone(),
            idempotency_key: run_identity.clone(),
            platform: platform.clone(),
            source: Some(source_context),
            nodes: vec![WorkflowNode {
                id: Alias::new(config.node_id.clone()).map_err(|e| e.message)?,
                kind: Alias::new("build").map_err(|e| e.message)?,
                effect_class: WorkflowEffectClass::Build,
                adapter_kind: Alias::new("nix").map_err(|e| e.message)?,
                operation_identity: operation_identity.clone(),
            }],
            edges: vec![],
            controls: kiln_core::workflow::WorkflowControls {
                cancel: false,
                pause: false,
                resume: false,
            },
            bounds,
        };
        kiln_core::workflow::validate_workflow_plan(&workflow_plan).map_err(|e| e.message)?;
        let build_plan = BuildEffectPlan {
            effect_id,
            attempt_id: attempt_id.clone(),
            idempotency_key: run_identity.clone(),
            candidate_id,
            pipeline_revision: pipeline_revision_id,
            plan_identity: config.profile.plan_identity.clone(),
            policy_identity: config.profile.policy_identity.clone(),
            provider_profile,
            platform,
            required_products: config
                .profile
                .expected_outputs
                .iter()
                .map(|output| Alias::new(output.name.as_str()).map_err(|e| e.message))
                .collect::<Result<_, _>>()?,
            constraints: None,
        };
        validate_build_effect_plan(&build_plan, config.profile.limits.max_required_products)
            .map_err(|diagnostic| diagnostic.code.to_string())?;
        let product_paths = config
            .profile
            .expected_outputs
            .iter()
            .map(|output| {
                (
                    output.name.as_str().to_owned(),
                    output.path.to_string_lossy().into_owned(),
                )
            })
            .collect();
        if let Some(existing) = self.state.runs.get(&run_id) {
            let evidence = existing
                .preview_build
                .as_ref()
                .ok_or("canonical_nix_build_run_conflict")?;
            if existing.plan != workflow_plan
                || evidence.source_run_id != source_run_id
                || evidence.request_id != request_id
                || evidence.profile_identity != profile_identity
                || evidence.plan != build_plan
            {
                return Err("canonical_nix_build_replay_conflict".into());
            }
            if evidence.observation.is_some() {
                finalize_canonical_nix_build(&mut self.state.runs, &run_id)?;
                self.persist()?;
                let retained = self
                    .state
                    .runs
                    .get(&run_id)
                    .ok_or("canonical_nix_build_record_missing")?;
                self.validate_canonical_nix_build_record(&run_id, retained)?;
                return Ok(run_id);
            }
            let record = self
                .state
                .runs
                .get_mut(&run_id)
                .ok_or("canonical_nix_build_record_missing")?;
            if !record.outcome().is_terminal() {
                record.error_code = Some("native_nix_build_outcome_unknown".into());
                record.phase = "unknown".into();
                record.mark_unknown()?;
                self.persist()?;
            }
            return Err("canonical_nix_build_uncertain_reconcile_required".into());
        }
        if self.state.runs.len() >= self.profile.bounds.max_runs {
            return Err("canonical_run_retention_exceeded".into());
        }
        let now = super::host_time_ms()?;
        let mut record = super::model::RunRecord {
            id: run_id.clone(),
            ordinal: self
                .state
                .version
                .checked_add(1)
                .ok_or("canonical ordinal overflow")?,
            project_id,
            pipeline_identity,
            source,
            requested_reference: None,
            requested_pull_request: None,
            requested_trigger: None,
            plan: workflow_plan.clone(),
            events: vec![],
            event_recorded_at_ms: vec![],
            phase: "dispatch_reserved".into(),
            observation: None,
            error_code: None,
            superseded_by: None,
            superseded_revision: None,
            artifacts: vec![],
            logs: vec![],
            created_at_ms: Some(now),
            started_at_ms: Some(now),
            completed_at_ms: None,
            parent_run_id: Some(source_run_id.to_owned()),
            auto_retry: None,
            selected_nodes: vec![config.node_id.clone()],
            node_timing: std::collections::BTreeMap::new(),
            reports: vec![],
            step_telemetry: vec![],
            dirty_snapshot: None,
            affected: None,
            scope: None,
            untrusted: None,
            implementation_verification: None,
            runbook: None,
            preview_build: Some(super::PreviewBuildEvidence {
                source_run_id: source_run_id.to_owned(),
                request_id: request_id.to_owned(),
                profile_identity: profile_identity.clone(),
                node_id: Alias::new(config.node_id).map_err(|e| e.message)?,
                plan: build_plan.clone(),
                product_paths,
                observation: None,
            }),
            state: State::default(),
        };
        record.apply(Command::StartRun {
            run_id: RunId::new(run_id.clone()).map_err(|e| e.message)?,
            attempt_id,
            provider_profile: workflow_plan.provider_profile.clone(),
            effect_id: workflow_plan.effect_id.clone(),
        })?;
        self.state.runs.insert(run_id.clone(), record);
        self.persist()?;

        let build_observation = match adapter.dispatch(&build_plan) {
            Ok(observation) => observation,
            Err(failure) => {
                let record = self
                    .state
                    .runs
                    .get_mut(&run_id)
                    .ok_or("canonical_nix_build_record_missing")?;
                record.error_code = Some("native_nix_build_dispatch_unknown".into());
                record.phase = "unknown".into();
                record.mark_unknown()?;
                self.persist()?;
                return Err(format!("canonical_nix_build_outcome_unknown: {failure:?}"));
            }
        };
        if let Err(diagnostic) =
            kiln_core::build::admit_build_evaluation(&build_plan, &build_observation)
        {
            let record = self
                .state
                .runs
                .get_mut(&run_id)
                .ok_or("canonical_nix_build_record_missing")?;
            record.error_code = Some(diagnostic.code.to_string());
            record.phase = "unknown".into();
            record.mark_unknown()?;
            self.persist()?;
            return Err(diagnostic.code.to_string());
        }
        let observation =
            build_workflow_observation(&workflow_plan, &profile_identity, &build_observation)?;
        kiln_core::workflow::admit_workflow_observation(&workflow_plan, &observation)
            .map_err(|e| e.message)?;
        {
            let record = self
                .state
                .runs
                .get_mut(&run_id)
                .ok_or("canonical_nix_build_record_missing")?;
            record.observation = Some(observation.clone());
            record
                .preview_build
                .as_mut()
                .ok_or("canonical_nix_build_intent_missing")?
                .observation = Some(build_observation);
        }
        self.persist()?;
        finalize_canonical_nix_build(&mut self.state.runs, &run_id)?;
        self.persist()?;
        Ok(run_id)
    }
    /// Prepare a disposable preview from the exact product observation retained
    /// by the successful canonical Build node. No build is dispatched here.
    pub fn prepare_preview_server(
        &self,
        run_id: &str,
        node_id: &str,
        selection: crate::preview::PreviewSelection,
        engine: crate::preview::engine::EngineBundle,
        nix: &crate::preview::nix::NativeNix,
        scratch: &std::path::Path,
    ) -> Result<crate::preview::service::PreviewServer, String> {
        use kiln_core::workflow::WorkflowEffectClass;

        let (project, repository, revision, plan, observation, product_paths) = {
            let run = self.state.runs.get(run_id).ok_or("preview_run_missing")?;
            self.validate_canonical_nix_build_record(run_id, run)?;
            let workflow = run
                .observation
                .as_ref()
                .ok_or("preview_workflow_observation_missing")?;
            let evidence = run.preview_build.as_ref().ok_or("preview_build_missing")?;
            let node = run
                .plan
                .nodes
                .iter()
                .find(|node| node.id.as_str() == node_id)
                .ok_or("preview_build_node_missing")?;
            let observed_node = workflow
                .nodes
                .iter()
                .find(|observed| observed.node_id == node.id);
            let observation = evidence
                .observation
                .as_ref()
                .ok_or("preview_build_observation_missing")?;
            if evidence.node_id != node.id
                || node.effect_class != WorkflowEffectClass::Build
                || !observed_node.is_some_and(|observed| {
                    observed.terminal
                        && observed.successful
                        && observed.state == kiln_core::workflow::WorkflowNodeState::Completed
                })
                || evidence.plan.candidate_id != workflow.candidate_id
                || evidence.plan.attempt_id != workflow.attempt_id
                || evidence.plan.pipeline_revision.as_str()
                    != workflow.pipeline_revision_id.as_str()
            {
                return Err("preview_build_not_bound_to_successful_exact_node".into());
            }
            let (_, successful) = self
                .review_ci_outcome(run_id)
                .ok_or("preview_run_missing")?;
            if !successful {
                return Err("preview_run_not_current_successful_ci".into());
            }
            (
                run.project_id.clone(),
                run.source.repository.native_id.as_str().to_owned(),
                run.source.exact_revision.native_id.as_str().to_owned(),
                evidence.plan.clone(),
                observation.clone(),
                evidence.product_paths.clone(),
            )
        };
        let product_name = product_paths
            .iter()
            .find(|(_, path)| path.as_str() == selection.root_path)
            .map(|(name, _)| name.as_str())
            .ok_or("preview_root_not_a_retained_product")?;
        if !plan
            .required_products
            .iter()
            .any(|name| name.as_str() == product_name)
        {
            return Err("preview_product_not_required_by_build".into());
        }
        let product = observation
            .products
            .iter()
            .find(|product| product.name.as_str() == product_name)
            .ok_or("preview_product_missing")?;
        let binding = kiln_core::preview::PreviewBinding {
            project,
            repository,
            revision,
            run_id: kiln_core::RunId::new(run_id).map_err(|_| "preview_run_invalid")?,
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            build_receipt: observation
                .receipt_identity
                .clone()
                .ok_or("preview_build_receipt_missing")?,
            product: product.name.clone(),
            artifact_identity: product.artifact_identity.clone(),
            store_identity: product.store_identity.clone(),
        };
        let request = crate::preview::produce(
            crate::preview::PreviewProduction {
                plan,
                observation,
                current: binding.clone(),
                selection: selection.clone(),
            },
            &engine,
            nix,
            scratch,
            crate::preview::service::now()?,
        )?;
        crate::preview::service::PreviewServer::prepare(
            request, &binding, &selection, engine, nix, scratch,
        )
    }
    pub(crate) fn review_ci_outcome(&self, run_id: &str) -> Option<(bool, bool)> {
        self.state.runs.get(run_id).map(|run| {
            (
                run.outcome().is_terminal(),
                !self.storage_failed
                    && super::verification::validate_record_provenance(run).is_ok()
                    && run.outcome() == Outcome::Success
                    && run.untrusted.as_ref().is_none_or(|gate| {
                        gate.satisfies_required_ci(run.source.exact_revision.native_id.as_str())
                    }),
            )
        })
    }
    pub(crate) fn validate_canonical_nix_build_record(
        &self,
        run_id: &str,
        run: &super::model::RunRecord,
    ) -> Result<(), String> {
        validate_canonical_nix_build_record(&self.profile, &self.state.runs, run_id, run)
    }
    pub(crate) fn canonical_preview_builds(
        &self,
    ) -> std::collections::BTreeMap<String, super::PreviewBuildEvidence> {
        self.state
            .runs
            .iter()
            .filter_map(|(run_id, run)| {
                self.validate_canonical_nix_build_record(run_id, run).ok()?;
                Some((run_id.clone(), run.preview_build.as_ref()?.clone()))
            })
            .collect()
    }

    /// Cancel a superseded speculative review candidate run that has not been
    /// dispatched yet. Dispatched runs are left alone: provider cancellation is
    /// an ambiguous external effect, so the review queue ignores their results.
    /// Returns whether the run was cancelled; repeated calls are no-ops.
    // r[impl kiln.review.queue.speculative.failure]
    pub(crate) fn cancel_review_candidate(&mut self, run_id: &str) -> Result<bool, String> {
        if self.storage_failed {
            return Err("durable_state_unavailable".into());
        }
        let Some(run) = self.state.runs.get_mut(run_id) else {
            return Ok(false);
        };
        if run.phase != "queued" {
            return Ok(false);
        }
        run.complete(Outcome::Cancelled, None)?;
        run.phase = "terminal".into();
        run.completed_at_ms = Some(super::host_time_ms()?);
        self.persist()?;
        Ok(true)
    }

    /// Export retained canonical run facts, not browser assertions or receipt votes.
    ///
    /// Keep failed/non-terminal observations: the newest run for an exact source
    /// supersedes an earlier success. Dirty snapshots and partial-node reruns can
    /// be inspected but never satisfy a full clean revision requirement.
    pub(crate) fn code_review_ci(&self) -> Vec<CiFact> {
        code_review_ci_from_state(&self.state, self.storage_failed)
    }
}
pub(super) fn validate_canonical_nix_build_record(
    profile: &crate::profile::ServiceProfile,
    runs: &std::collections::BTreeMap<String, super::model::RunRecord>,
    run_id: &str,
    run: &super::model::RunRecord,
) -> Result<(), String> {
    let evidence = run
        .preview_build
        .as_ref()
        .ok_or("canonical_nix_build_evidence_missing")?;
    let source = runs
        .get(&evidence.source_run_id)
        .ok_or("canonical_nix_build_source_missing")?;
    let project = profile
        .projects
        .iter()
        .find(|project| project.id == run.project_id)
        .ok_or("canonical_nix_build_project_missing")?;
    let config = project
        .canonical_nix_build
        .as_ref()
        .ok_or("canonical_nix_build_not_opted_in")?;
    if run.id != run_id
        || (run.phase != "terminal" && run.phase != "unknown")
        || source.phase != "terminal"
        || source.outcome() != Outcome::Success
        || !source.selected_nodes.is_empty()
        || source.dirty_snapshot.is_some()
        || source.untrusted.is_some()
        || super::verification::validate_record_provenance(source).is_err()
        || source.source != run.source
        || source.source.exact_revision.native_id.as_str() != config.source_revision
        || source.project_id != run.project_id
        || run.parent_run_id.as_deref() != Some(evidence.source_run_id.as_str())
        || !crate::profile::identifier(&evidence.request_id)
    {
        return Err("canonical_nix_build_source_binding_invalid".into());
    }
    let source_workflow = source
        .observation
        .as_ref()
        .ok_or("canonical_nix_build_source_observation_missing")?;
    let profile_identity = super::identity(&config.profile)?;
    let operation_identity =
        crate::profile::canonical_nix_build_operation_identity(&project.id, config)?;
    let pipeline_identity = super::identity(&(
        "kiln.web.canonical-nix-build.pipeline.v1",
        &project.id,
        config,
    ))?;
    let run_identity = super::identity(&(
        "kiln.web.canonical-nix-build.run.v1",
        &profile.service_id,
        &project.id,
        evidence.source_run_id.as_str(),
        evidence.request_id.as_str(),
        &profile_identity,
        &operation_identity,
        &run.source,
    ))?;
    let expected_run_id = format!("web-nix-build-{}", run_identity.as_str());
    let node_id = kiln_core::Alias::new(config.node_id.clone()).map_err(|e| e.message)?;
    let workflow_node = run
        .plan
        .nodes
        .first()
        .ok_or("canonical_nix_build_node_missing")?;
    let expected_pipeline = kiln_core::types::PipelineRevisionId::new(format!(
        "web-pipeline-{}",
        pipeline_identity.as_str()
    ))
    .map_err(|e| e.message)?;
    let expected_attempt =
        kiln_core::types::AttemptId::new(format!("web-attempt-{}", run_identity.as_str()))
            .map_err(|e| e.message)?;
    let expected_effect =
        kiln_core::types::EffectId::new(format!("web-effect-{}", run_identity.as_str()))
            .map_err(|e| e.message)?;
    let expected_provider =
        kiln_core::types::ProviderProfileId::new(config.profile.provider_profile_id.clone())
            .map_err(|e| e.message)?;
    let expected_policy = kiln_core::types::PolicyId::new(format!(
        "nix-build-{}",
        config.profile.policy_identity.as_str()
    ))
    .map_err(|e| e.message)?;
    let expected_platform =
        kiln_core::Alias::new(config.profile.platform.clone()).map_err(|e| e.message)?;
    let expected_products = config
        .profile
        .expected_outputs
        .iter()
        .map(|output| kiln_core::Alias::new(output.name.as_str()).map_err(|e| e.message))
        .collect::<Result<Vec<_>, _>>()?;
    let expected_paths: std::collections::BTreeMap<_, _> = config
        .profile
        .expected_outputs
        .iter()
        .map(|output| {
            (
                output.name.as_str().to_owned(),
                output.path.to_string_lossy().into_owned(),
            )
        })
        .collect();
    if run.pipeline_identity != pipeline_identity || run.scope.is_some() {
        return Err("canonical_nix_build_pipeline_binding_invalid".into());
    }
    if run.id != expected_run_id
        || evidence.profile_identity != profile_identity
        || evidence.node_id != node_id
        || evidence.plan
            != (kiln_core::build::BuildEffectPlan {
                effect_id: expected_effect.clone(),
                attempt_id: expected_attempt.clone(),
                idempotency_key: run_identity.clone(),
                candidate_id: source_workflow.candidate_id.clone(),
                pipeline_revision: expected_pipeline.clone(),
                plan_identity: config.profile.plan_identity.clone(),
                policy_identity: config.profile.policy_identity.clone(),
                provider_profile: expected_provider.clone(),
                platform: expected_platform.clone(),
                required_products: expected_products,
                constraints: None,
            })
        || evidence.product_paths != expected_paths
        || run.plan.effect_id != expected_effect
        || run.plan.attempt_id != expected_attempt
        || run.plan.candidate_id != source_workflow.candidate_id
        || run.plan.pipeline_revision_id != expected_pipeline
        || run.plan.workflow_policy_id != expected_policy
        || run.plan.provider_profile != expected_provider
        || run.plan.idempotency_key != run_identity
        || run.plan.platform != expected_platform
        || run.plan.source != source.plan.source
        || run.plan.bounds != source.plan.bounds
        || run.selected_nodes != vec![config.node_id.clone()]
        || run.plan.nodes.len() != 1
        || workflow_node.id != node_id
        || workflow_node.kind.as_str() != "build"
        || workflow_node.effect_class != kiln_core::workflow::WorkflowEffectClass::Build
        || workflow_node.adapter_kind.as_str() != "nix"
        || workflow_node.operation_identity != operation_identity
        || !run.plan.edges.is_empty()
        || evidence.plan.effect_id != run.plan.effect_id
        || evidence.plan.attempt_id != run.plan.attempt_id
        || evidence.plan.pipeline_revision.as_str() != run.plan.pipeline_revision_id.as_str()
    {
        return Err("canonical_nix_build_plan_binding_invalid".into());
    }
    let Some(build) = evidence.observation.as_ref() else {
        if run.phase == "unknown" && run.outcome() == Outcome::Unknown && run.observation.is_none()
        {
            return Ok(());
        }
        return Err("canonical_nix_build_observation_missing".into());
    };
    kiln_core::build::admit_build_evaluation(&evidence.plan, build).map_err(|e| e.message)?;
    let expected_workflow = build_workflow_observation(&run.plan, &profile_identity, build)?;
    let expected_outcome = build_domain_outcome(build.outcome);
    if run.observation.as_ref() != Some(&expected_workflow)
        || run.outcome() != expected_outcome
        || (expected_outcome == Outcome::Unknown && run.phase != "unknown")
        || (expected_outcome != Outcome::Unknown && run.phase != "terminal")
    {
        return Err("canonical_nix_build_workflow_observation_invalid".into());
    }
    Ok(())
}

pub(crate) fn read_only_code_review_ci(
    profile: &crate::profile::ServiceProfile,
) -> Result<Vec<CiFact>, String> {
    let (_store, state) = super::storage::Store::open_read_only(profile)?;
    Ok(code_review_ci_from_state(&state, false))
}
pub(crate) fn code_review_ci_from_state(
    state: &super::model::DurableState,
    storage_failed: bool,
) -> Vec<CiFact> {
    state
        .runs
        .values()
        .filter(|run| !run.is_canonical_nix_build())
        .map(|run| {
            let trusted_provenance = super::verification::validate_record_provenance(run).is_ok();
            let checks = run
                .plan
                .nodes
                .iter()
                .map(|node| {
                    let outcome = run
                        .observation
                        .as_ref()
                        .and_then(|observation| {
                            observation
                                .nodes
                                .iter()
                                .find(|observed| observed.node_id == node.id)
                        })
                        .map(|observed| match observed.state {
                            WorkflowNodeState::Completed => "pass",
                            WorkflowNodeState::Failed => "fail",
                            WorkflowNodeState::Cancelled => "cancelled",
                            WorkflowNodeState::Skipped => "skipped",
                            WorkflowNodeState::Pending
                            | WorkflowNodeState::Running
                            | WorkflowNodeState::Paused
                            | WorkflowNodeState::Unknown => "unknown",
                        })
                        .unwrap_or("unknown")
                        .to_owned();
                    let artifact_digest = run
                        .reports
                        .iter()
                        .find(|report| report.node_id == node.id.as_str())
                        .and_then(|report| {
                            report
                                .report
                                .as_ref()
                                .map(|parsed| parsed.source_blake3.clone())
                                .or_else(|| {
                                    report
                                        .artifact
                                        .as_ref()
                                        .map(|artifact| artifact.blake3.clone())
                                })
                        });
                    CiCheck {
                        name: node.id.as_str().to_owned(),
                        kind: node.kind.as_str().to_owned(),
                        outcome,
                        artifact_digest,
                    }
                })
                .collect();
            CiFact {
                project: run.project_id.clone(),
                repository: run
                    .implementation_verification
                    .as_ref()
                    .filter(|_| run.is_independent_verification() && trusted_provenance)
                    .map(|provenance| provenance.repository_alias.clone())
                    .unwrap_or_else(|| run.source.repository.native_id.as_str().to_owned()),
                revision: run.source.exact_revision.native_id.as_str().to_owned(),
                run_id: run.id.clone(),
                ordinal: run.ordinal,
                success: !storage_failed && run.outcome() == Outcome::Success && trusted_provenance,
                full_run: run.selected_nodes.is_empty(),
                clean: run.dirty_snapshot.is_none(),
                scope_identity: run.scope.as_ref().map(|scope| scope.identity.clone()),
                terminal: run.outcome().is_terminal(),
                untrusted: run
                    .untrusted
                    .as_ref()
                    .map(kiln_core::untrusted::UntrustedGate::summary),
                checks,
            }
        })
        .collect()
}

fn build_workflow_observation(
    plan: &kiln_core::workflow::WorkflowEffectPlan,
    profile_identity: &kiln_core::ContentId,
    build: &kiln_core::build::BuildEvaluationObservation,
) -> Result<kiln_core::workflow::WorkflowObservation, String> {
    use kiln_core::workflow::{WorkflowNodeObservation, WorkflowNodeState, WorkflowState};
    let (state, node_state, outcome, successful) = match build.outcome {
        kiln_core::build::BuildEvaluationOutcome::Success => (
            WorkflowState::Completed,
            WorkflowNodeState::Completed,
            Outcome::Success,
            true,
        ),
        kiln_core::build::BuildEvaluationOutcome::Cancelled => (
            WorkflowState::Cancelled,
            WorkflowNodeState::Cancelled,
            Outcome::Cancelled,
            false,
        ),
        kiln_core::build::BuildEvaluationOutcome::Unknown => (
            WorkflowState::Unknown,
            WorkflowNodeState::Unknown,
            Outcome::Unknown,
            false,
        ),
        kiln_core::build::BuildEvaluationOutcome::Failed
        | kiln_core::build::BuildEvaluationOutcome::TimedOut
        | kiln_core::build::BuildEvaluationOutcome::ProviderError => (
            WorkflowState::Failed,
            WorkflowNodeState::Failed,
            Outcome::DomainFailure,
            false,
        ),
    };
    let receipt = build
        .receipt_identity
        .as_ref()
        .map(|identity| {
            kiln_core::types::ExternalRef::new(
                "kiln",
                "native-nix-build-receipt",
                identity.as_str(),
            )
            .map_err(|e| e.message)
        })
        .transpose()?;
    let node = plan
        .nodes
        .first()
        .ok_or("canonical_nix_build_node_missing")?;
    let execution = kiln_core::types::ExternalRef::new(
        "kiln",
        "native-nix-build-effect",
        plan.effect_id.as_str(),
    )
    .map_err(|e| e.message)?;
    let workflow = kiln_core::types::ExternalRef::new(
        "kiln",
        "native-nix-build-profile",
        profile_identity.as_str(),
    )
    .map_err(|e| e.message)?;
    let adapter = kiln_core::types::ExternalRef::new(
        "kiln",
        "native-nix-build-provider",
        plan.provider_profile.as_str(),
    )
    .map_err(|e| e.message)?;
    Ok(kiln_core::workflow::WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow,
        execution,
        sequence: 1,
        state,
        terminal: state.is_terminal(),
        successful,
        outcome,
        nodes: vec![WorkflowNodeObservation {
            node_id: node.id.clone(),
            state: node_state,
            terminal: node_state.is_terminal(),
            successful: node_state.is_successful(),
            adapter,
            receipt: receipt.clone(),
        }],
        logs: vec![],
        receipt,
    })
}

fn build_domain_outcome(outcome: kiln_core::build::BuildEvaluationOutcome) -> Outcome {
    match outcome {
        kiln_core::build::BuildEvaluationOutcome::Success => Outcome::Success,
        kiln_core::build::BuildEvaluationOutcome::Cancelled => Outcome::Cancelled,
        kiln_core::build::BuildEvaluationOutcome::Unknown => Outcome::Unknown,
        kiln_core::build::BuildEvaluationOutcome::Failed
        | kiln_core::build::BuildEvaluationOutcome::TimedOut
        | kiln_core::build::BuildEvaluationOutcome::ProviderError => Outcome::DomainFailure,
    }
}

fn finalize_canonical_nix_build(
    runs: &mut std::collections::BTreeMap<String, super::model::RunRecord>,
    run_id: &str,
) -> Result<(), String> {
    let record = runs
        .get_mut(run_id)
        .ok_or("canonical_nix_build_record_missing")?;
    let build = record
        .preview_build
        .as_ref()
        .and_then(|evidence| evidence.observation.as_ref())
        .ok_or("canonical_nix_build_observation_missing")?;
    if record.outcome().is_terminal() {
        return Ok(());
    }
    let workflow = record
        .observation
        .as_ref()
        .ok_or("canonical_nix_build_workflow_observation_missing")?;
    let outcome = build_domain_outcome(build.outcome);
    if outcome == Outcome::Unknown {
        record.phase = "unknown".into();
        record.mark_unknown()?;
    } else {
        if outcome != Outcome::Success {
            record.error_code = Some("native_nix_build_failed".into());
        }
        record.phase = "terminal".into();
        record.completed_at_ms = Some(super::host_time_ms()?);
        record.complete(outcome, Some(workflow.execution.clone()))?;
    }
    Ok(())
}

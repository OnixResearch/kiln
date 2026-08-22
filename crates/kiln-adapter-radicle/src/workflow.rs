use kiln_codec::encode_and_identify;
use kiln_core::types::{
    Alias, AttemptId, CandidateId, EffectId, PipelineRevisionId, PolicyId, RunId,
};
use kiln_core::workflow::{
    WorkflowEffectPlan, WorkflowObservation, WorkflowSourceContext, WorkflowState,
    validate_workflow_plan,
};
use kiln_provider_api::{
    PortFailure, PortResult, TriggerIntakePort, TriggerLifecyclePolicy, TriggerRequest, TriggerRun,
    TriggerTerminal, WorkflowPort,
};

use crate::profile::{RadicleShellConfig, workflow_edges, workflow_nodes};
use crate::replay::{ReplayRecord, ReplayState, ReplayStore, ReserveResult, reserved_record};

const RUN_ID_PREFIX: &str = "radicle-run-";
const EFFECT_ID_PREFIX: &str = "radicle-effect-";
const ATTEMPT_ID_PREFIX: &str = "radicle-attempt-";
const CANDIDATE_ID_PREFIX: &str = "radicle-candidate-";
const PIPELINE_ID_PREFIX: &str = "radicle-revision-";

#[derive(Debug)]
pub struct RadicleWorkflowService<W, R> {
    workflow: W,
    replay: R,
    config: RadicleShellConfig,
}

impl<W, R> RadicleWorkflowService<W, R> {
    pub const fn new(workflow: W, replay: R, config: RadicleShellConfig) -> Self {
        Self {
            workflow,
            replay,
            config,
        }
    }

    #[must_use]
    pub const fn workflow(&self) -> &W {
        &self.workflow
    }

    #[must_use]
    pub const fn replay(&self) -> &R {
        &self.replay
    }
}

impl<W: WorkflowPort, R: ReplayStore> TriggerIntakePort for RadicleWorkflowService<W, R> {
    // r[impl kiln.radicle.trigger]
    // r[impl kiln.radicle.lifecycle]
    // r[impl kiln.radicle.shell_trigger]
    fn accept(&mut self, trigger: TriggerRequest) -> PortResult<TriggerRun> {
        let record = plan_trigger(&self.config, trigger)?;
        match self.replay.reserve(record)? {
            ReserveResult::Inserted(record) => {
                let observation = match self.workflow.dispatch(&record.plan) {
                    Ok(observation) => observation,
                    Err(PortFailure::Unknown(_)) => return self.persist_uncertain(&record),
                    Err(error) => return Err(error),
                };
                self.persist_observation(&record, observation)
            }
            ReserveResult::Existing(record) => {
                if record.state == ReplayState::Terminal {
                    return Ok(record.run);
                }
                let observation = match self.workflow.recover_dispatch(&record.plan) {
                    Ok(observation) => observation,
                    Err(PortFailure::Unknown(_)) => return self.persist_uncertain(&record),
                    Err(error) => return Err(error),
                };
                self.persist_observation(&record, observation)
            }
        }
    }

    fn observe(
        &mut self,
        accepted: &TriggerRun,
        policy: TriggerLifecyclePolicy,
    ) -> PortResult<TriggerRun> {
        let mut record = self.replay.load_by_run(&accepted.run_id)?.ok_or_else(|| {
            domain(
                "radicle_replay_missing",
                "radicle.run",
                "run has no durable replay record",
            )
        })?;
        if record.state == ReplayState::Terminal {
            return Ok(record.run);
        }
        let maximum_polls = policy.max_polls.min(self.config.maximum_polls);
        if maximum_polls == 0 {
            return Err(domain(
                "radicle_poll_bound",
                "radicle.polls",
                "effective Radicle poll bound must be positive",
            ));
        }
        for _poll in 0..maximum_polls {
            let result = if record.state == ReplayState::Uncertain {
                self.workflow.reconcile(&record.plan.effect_id)
            } else {
                self.workflow.observe(&record.plan.effect_id)
            };
            let observation = match result {
                Ok(observation) => observation,
                Err(PortFailure::Unknown(_)) => return self.persist_uncertain(&record),
                Err(PortFailure::TimedOut(_)) => return self.persist_timeout(&record),
                Err(error) => return Err(error),
            };
            let run = self.persist_observation(&record, observation)?;
            record = self.replay.load_by_run(&run.run_id)?.ok_or_else(|| {
                domain(
                    "radicle_replay_missing",
                    "radicle.run",
                    "updated run disappeared",
                )
            })?;
            if record.state == ReplayState::Terminal || record.state == ReplayState::Uncertain {
                return Ok(record.run);
            }
        }
        self.persist_timeout(&record)
    }
}

impl<W: WorkflowPort, R: ReplayStore> RadicleWorkflowService<W, R> {
    fn persist_observation(
        &mut self,
        current: &ReplayRecord,
        observation: WorkflowObservation,
    ) -> PortResult<TriggerRun> {
        let mut next = current.clone();
        next.run = trigger_run(&observation, current.run.run_id.clone());
        next.state = match observation.state {
            WorkflowState::Completed | WorkflowState::Failed | WorkflowState::Cancelled => {
                ReplayState::Terminal
            }
            WorkflowState::Unknown => ReplayState::Uncertain,
            WorkflowState::Accepted
            | WorkflowState::Queued
            | WorkflowState::Running
            | WorkflowState::Paused => ReplayState::Accepted,
        };
        next.observation = Some(observation);
        self.replay.replace(current, next.clone())?;
        Ok(next.run)
    }

    fn persist_uncertain(&mut self, current: &ReplayRecord) -> PortResult<TriggerRun> {
        let mut next = current.clone();
        next.state = ReplayState::Uncertain;
        next.run.terminal = TriggerTerminal::Unknown;
        self.replay.replace(current, next.clone())?;
        Ok(next.run)
    }

    fn persist_timeout(&mut self, current: &ReplayRecord) -> PortResult<TriggerRun> {
        let mut next = current.clone();
        next.state = ReplayState::Uncertain;
        next.run.terminal = TriggerTerminal::TimedOut;
        self.replay.replace(current, next.clone())?;
        Ok(next.run)
    }
}

// r[impl kiln.radicle.trigger]
pub fn plan_trigger(
    config: &RadicleShellConfig,
    trigger: TriggerRequest,
) -> PortResult<ReplayRecord> {
    trigger.repository.validate()?;
    trigger.actor.validate()?;
    trigger.after.validate()?;
    if trigger.repository.provider_namespace.as_str() != "radicle"
        || trigger.repository.native_kind.as_str() != "repository"
        || trigger.after.provider_namespace.as_str() != "radicle"
        || trigger.after.native_kind.as_str() != "revision"
    {
        return Err(domain(
            "radicle_trigger_source",
            "radicle.trigger",
            "trigger does not carry exact Radicle repository and revision facts",
        ));
    }
    let (_bytes, event_identity) =
        encode_and_identify(&trigger, config.adapter_limits.max_request_bytes).map_err(|_| {
            domain(
                "radicle_trigger_identity",
                "radicle.trigger",
                "trigger cannot be encoded within its canonical identity bound",
            )
        })?;
    let identity = event_identity.as_str().to_string();
    let run_id = RunId::new(format!("{RUN_ID_PREFIX}{identity}"))?;
    let plan = WorkflowEffectPlan {
        effect_id: EffectId::new(format!("{EFFECT_ID_PREFIX}{identity}"))?,
        attempt_id: AttemptId::new(format!("{ATTEMPT_ID_PREFIX}{identity}"))?,
        candidate_id: CandidateId::new(format!("{CANDIDATE_ID_PREFIX}{identity}"))?,
        pipeline_revision_id: PipelineRevisionId::new(format!("{PIPELINE_ID_PREFIX}{identity}"))?,
        workflow_policy_id: PolicyId::new(config.workflow.policy_id.clone())?,
        provider_profile: kiln_core::types::ProviderProfileId::new(
            config.workflow.provider_profile_id.clone(),
        )?,
        idempotency_key: event_identity,
        platform: Alias::new(config.workflow.platform.clone())?,
        source: Some(WorkflowSourceContext {
            provider: Alias::new("radicle")?,
            repository: trigger.repository.clone(),
            revision: trigger.after.clone(),
        }),
        nodes: workflow_nodes(&config.workflow)?,
        edges: workflow_edges(&config.workflow)?,
        controls: config.workflow.controls,
        bounds: config.workflow.bounds,
    };
    validate_workflow_plan(&plan)?;
    Ok(reserved_record(
        identity,
        trigger,
        plan,
        TriggerRun {
            run_id,
            terminal: TriggerTerminal::Unknown,
            log_url: None,
        },
    ))
}

fn trigger_run(observation: &WorkflowObservation, run_id: RunId) -> TriggerRun {
    let terminal = match observation.state {
        WorkflowState::Completed => TriggerTerminal::Success,
        WorkflowState::Failed => TriggerTerminal::Failure,
        WorkflowState::Cancelled => TriggerTerminal::Cancelled,
        WorkflowState::Unknown => TriggerTerminal::Unknown,
        WorkflowState::Accepted
        | WorkflowState::Queued
        | WorkflowState::Running
        | WorkflowState::Paused => TriggerTerminal::Unknown,
    };
    TriggerRun {
        run_id,
        terminal,
        log_url: None,
    }
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(kiln_core::types::Diagnostic::new(code, field, message))
}

#[cfg(test)]
pub(crate) fn fixture_record(character: char) -> ReplayRecord {
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_core::workflow::{WorkflowBounds, WorkflowControls};
    use kiln_provider_api::TriggerEventKind;

    let hex = character
        .to_string()
        .repeat(kiln_core::types::BLAKE3_HEX_BYTES);
    let trigger = TriggerRequest {
        repository: ExternalRef::new("radicle", "repository", "rad:zrepo")
            .unwrap_or_else(|_| unreachable!()),
        actor: ExternalRef::new("radicle", "actor", "broker").unwrap_or_else(|_| unreachable!()),
        before: None,
        after: ExternalRef::new("radicle", "revision", "abc").unwrap_or_else(|_| unreachable!()),
        branch: None,
        patch: None,
        event: TriggerEventKind::Push,
    };
    let plan = WorkflowEffectPlan {
        effect_id: EffectId::new(format!("effect-{hex}")).unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new(format!("attempt-{hex}")).unwrap_or_else(|_| unreachable!()),
        candidate_id: CandidateId::new(format!("candidate-{hex}"))
            .unwrap_or_else(|_| unreachable!()),
        pipeline_revision_id: PipelineRevisionId::new(format!("pipeline-{hex}"))
            .unwrap_or_else(|_| unreachable!()),
        workflow_policy_id: PolicyId::new("policy").unwrap_or_else(|_| unreachable!()),
        provider_profile: ProviderProfileId::new("lattice-v1").unwrap_or_else(|_| unreachable!()),
        idempotency_key: ContentId::from_blake3_hex(hex.clone()).unwrap_or_else(|_| unreachable!()),
        platform: Alias::new("linux-x86_64").unwrap_or_else(|_| unreachable!()),
        source: Some(WorkflowSourceContext {
            provider: Alias::new("radicle").unwrap_or_else(|_| unreachable!()),
            repository: trigger.repository.clone(),
            revision: trigger.after.clone(),
        }),
        nodes: Vec::new(),
        edges: Vec::new(),
        controls: WorkflowControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        bounds: WorkflowBounds {
            max_nodes: 1,
            max_edges: 1,
            max_log_references: 1,
            max_log_read_bytes: 1,
            max_total_log_bytes: 1,
        },
    };
    reserved_record(
        hex,
        trigger,
        plan,
        TriggerRun {
            run_id: RunId::new("run").unwrap_or_else(|_| unreachable!()),
            terminal: TriggerTerminal::Unknown,
            log_url: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use kiln_adapter_lattice::{LatticeAdapterLimits, LatticeProfile};
    use kiln_core::domain::Outcome;
    use kiln_core::provider::CapabilityProfile;
    use kiln_core::types::{ExternalRef, ProviderProfileId};
    use kiln_core::workflow::{
        WorkflowBounds, WorkflowControlRequest, WorkflowControls, WorkflowLogChunk,
        WorkflowLogReadRequest, WorkflowNodeObservation, WorkflowNodeState,
    };
    use kiln_provider_api::{InfrastructureFailure, TriggerEventKind};
    use lattice_workflow_transport::TransportLimits;
    use tempfile::tempdir;

    use super::*;
    use crate::profile::{
        AdapterLimits, CapabilitySelection, ConfigEffectClass, ConfigNode,
        PINNED_RADICLE_BROKER_REVISION, PINNED_RADICLE_LICENSE, PINNED_RADICLE_RUNTIME_REVISION,
        RADICLE_SHELL_SCHEMA, REQUIRED_NON_CLAIMS, RadicleShellConfig, RuntimeMode,
        WorkflowTemplate, admit_config,
    };
    use crate::replay::SqliteReplayStore;

    const MAXIMUM_REPLAY_RECORDS: usize = 8;
    const MAXIMUM_POLLS: usize = 4;
    const MAXIMUM_NODES: usize = 4;
    const MAXIMUM_EDGES: usize = 4;
    const MAXIMUM_LOG_REFERENCES: usize = 4;
    const MAXIMUM_LOG_READ_BYTES: usize = 4_096;
    const MAXIMUM_TOTAL_LOG_BYTES: usize = 16_384;
    const MAXIMUM_REQUEST_BYTES: usize = 65_536;
    const MAXIMUM_RESPONSE_BYTES: usize = 65_536;
    const MAXIMUM_IN_FLIGHT: usize = 4;
    const MAXIMUM_FRAME_BYTES: u32 = 1_048_576;
    const MAXIMUM_LOG_BYTES: u32 = 65_536;
    const MAXIMUM_CONNECTIONS: u32 = 8;
    const DEADLINE_MILLISECONDS: u64 = 5_000;
    const USER_ID: u32 = 1_000;
    const OPERATION_IDENTITY: &str =
        "1111111111111111111111111111111111111111111111111111111111111111";

    #[derive(Debug)]
    struct FixtureWorkflow {
        profile: CapabilityProfile,
        dispatch_unknown: bool,
        dispatch_calls: usize,
        recovery_calls: usize,
        observe_calls: usize,
        active_plan: Option<WorkflowEffectPlan>,
    }

    impl FixtureWorkflow {
        fn new(dispatch_unknown: bool) -> Self {
            let limits = LatticeAdapterLimits {
                workflow: bounds(),
                max_request_bytes: MAXIMUM_REQUEST_BYTES,
                max_response_bytes: MAXIMUM_RESPONSE_BYTES,
                max_in_flight: MAXIMUM_IN_FLIGHT,
            };
            let profile = LatticeProfile::pinned(
                ProviderProfileId::new("lattice-v1").unwrap_or_else(|_| unreachable!()),
                limits,
            )
            .unwrap_or_else(|_| unreachable!())
            .provider;
            Self {
                profile,
                dispatch_unknown,
                dispatch_calls: 0,
                recovery_calls: 0,
                observe_calls: 0,
                active_plan: None,
            }
        }
    }

    impl WorkflowPort for FixtureWorkflow {
        fn profile(&self) -> &CapabilityProfile {
            &self.profile
        }

        fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
            self.dispatch_calls = self.dispatch_calls.saturating_add(1);
            self.active_plan = Some(plan.clone());
            if self.dispatch_unknown {
                return Err(PortFailure::Unknown(failure("fixture_dispatch_unknown")));
            }
            Ok(accepted_observation(plan))
        }

        fn recover_dispatch(
            &mut self,
            plan: &WorkflowEffectPlan,
        ) -> PortResult<WorkflowObservation> {
            self.recovery_calls = self.recovery_calls.saturating_add(1);
            self.active_plan = Some(plan.clone());
            Ok(accepted_observation(plan))
        }

        fn observe(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
            self.observe_calls = self.observe_calls.saturating_add(1);
            let plan = self
                .active_plan
                .as_ref()
                .ok_or_else(|| PortFailure::Infrastructure(failure("fixture_plan_missing")))?;
            if &plan.effect_id != effect_id {
                return Err(PortFailure::Infrastructure(failure(
                    "fixture_effect_mismatch",
                )));
            }
            Ok(completed_observation(plan))
        }

        fn control(
            &mut self,
            _request: &WorkflowControlRequest,
        ) -> PortResult<WorkflowObservation> {
            Err(PortFailure::Infrastructure(failure("fixture_control")))
        }

        fn read_log(&mut self, _request: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
            Err(PortFailure::Infrastructure(failure("fixture_log")))
        }

        fn reconcile(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
            self.observe(effect_id)
        }
    }

    // r[verify kiln.radicle.protocol]
    // r[verify kiln.radicle.native_runner.scenario.shell]
    // r[verify kiln.radicle.shell_profile.scenario.fixture]
    #[test]
    fn profile_admits_trigger_only_and_rejects_fixture_or_collaboration() {
        let directory = tempdir().unwrap_or_else(|_| unreachable!());
        let profile = config(directory.path());
        assert!(admit_config(profile.clone()).is_ok());

        let mut fixture = profile.clone();
        fixture.mode = RuntimeMode::Fixture;
        assert!(admit_config(fixture).is_err());

        let mut collaboration = profile;
        collaboration.capabilities.collaboration = true;
        assert!(admit_config(collaboration).is_err());
    }

    // r[verify kiln.radicle.result.scenario.unknown]
    // r[verify kiln.radicle.fixtures]
    // r[verify kiln.radicle.shell_trigger.scenario.valid]
    // r[verify kiln.radicle.shell_trigger.scenario.replay]
    #[test]
    fn uncertain_dispatch_recovers_without_redispatch_after_restart() {
        let directory = tempdir().unwrap_or_else(|_| unreachable!());
        let profile = config(directory.path());
        let database = profile.replay_database_path.clone();
        let trigger = trigger();
        {
            let replay = SqliteReplayStore::open(&database, MAXIMUM_REPLAY_RECORDS)
                .unwrap_or_else(|_| unreachable!());
            let mut service =
                RadicleWorkflowService::new(FixtureWorkflow::new(true), replay, profile.clone());
            let accepted = service
                .accept(trigger.clone())
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(accepted.terminal, TriggerTerminal::Unknown);
            assert_eq!(service.workflow().dispatch_calls, 1);
        }

        let replay = SqliteReplayStore::open(&database, MAXIMUM_REPLAY_RECORDS)
            .unwrap_or_else(|_| unreachable!());
        let mut service = RadicleWorkflowService::new(FixtureWorkflow::new(false), replay, profile);
        let accepted = service.accept(trigger).unwrap_or_else(|_| unreachable!());
        assert_eq!(service.workflow().dispatch_calls, 0);
        assert_eq!(service.workflow().recovery_calls, 1);
        let completed = service
            .observe(
                &accepted,
                TriggerLifecyclePolicy {
                    mode: kiln_provider_api::TriggerWaitMode::Wait,
                    max_polls: MAXIMUM_POLLS,
                },
            )
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(completed.terminal, TriggerTerminal::Success);
    }

    fn config(root: &std::path::Path) -> RadicleShellConfig {
        RadicleShellConfig {
            schema: RADICLE_SHELL_SCHEMA.to_string(),
            mode: RuntimeMode::Production,
            radicle_runtime_revision: PINNED_RADICLE_RUNTIME_REVISION.to_string(),
            radicle_broker_revision: PINNED_RADICLE_BROKER_REVISION.to_string(),
            radicle_license: PINNED_RADICLE_LICENSE.to_string(),
            lattice_source: kiln_adapter_lattice::PINNED_LATTICE_SOURCE.to_string(),
            lattice_revision: kiln_adapter_lattice::PINNED_LATTICE_REVISION.to_string(),
            lattice_license: kiln_adapter_lattice::PINNED_LATTICE_CONTRACT_LICENSE.to_string(),
            capabilities: CapabilitySelection {
                source: false,
                trigger: true,
                status: false,
                collaboration: false,
            },
            socket_path: root.join("lattice.sock"),
            replay_database_path: root.join("replay.sqlite"),
            maximum_replay_records: MAXIMUM_REPLAY_RECORDS,
            maximum_polls: MAXIMUM_POLLS,
            allowed_user_ids: vec![USER_ID],
            transport_limits: TransportLimits {
                maximum_frame_bytes: MAXIMUM_FRAME_BYTES,
                maximum_log_bytes: MAXIMUM_LOG_BYTES,
                maximum_connections: MAXIMUM_CONNECTIONS,
                read_deadline_milliseconds: DEADLINE_MILLISECONDS,
                write_deadline_milliseconds: DEADLINE_MILLISECONDS,
            },
            adapter_limits: AdapterLimits {
                max_request_bytes: MAXIMUM_REQUEST_BYTES,
                max_response_bytes: MAXIMUM_RESPONSE_BYTES,
                max_in_flight: MAXIMUM_IN_FLIGHT,
            },
            workflow: WorkflowTemplate {
                policy_id: "radicle-policy".to_string(),
                provider_profile_id: "lattice-v1".to_string(),
                platform: "linux-x86_64".to_string(),
                nodes: vec![ConfigNode {
                    id: "build".to_string(),
                    kind: "build".to_string(),
                    effect_class: ConfigEffectClass::Build,
                    adapter_kind: "local".to_string(),
                    operation_identity: OPERATION_IDENTITY.to_string(),
                }],
                edges: Vec::new(),
                controls: WorkflowControls {
                    cancel: true,
                    pause: false,
                    resume: false,
                },
                bounds: bounds(),
            },
            non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
        }
    }

    const fn bounds() -> WorkflowBounds {
        WorkflowBounds {
            max_nodes: MAXIMUM_NODES,
            max_edges: MAXIMUM_EDGES,
            max_log_references: MAXIMUM_LOG_REFERENCES,
            max_log_read_bytes: MAXIMUM_LOG_READ_BYTES,
            max_total_log_bytes: MAXIMUM_TOTAL_LOG_BYTES,
        }
    }

    fn trigger() -> TriggerRequest {
        TriggerRequest {
            repository: ExternalRef::new("radicle", "repository", "rad:zrepo")
                .unwrap_or_else(|_| unreachable!()),
            actor: ExternalRef::new("radicle", "actor", "broker")
                .unwrap_or_else(|_| unreachable!()),
            before: None,
            after: ExternalRef::new(
                "radicle",
                "revision",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_or_else(|_| unreachable!()),
            branch: Some(Alias::new("main").unwrap_or_else(|_| unreachable!())),
            patch: None,
            event: TriggerEventKind::Push,
        }
    }

    fn accepted_observation(plan: &WorkflowEffectPlan) -> WorkflowObservation {
        WorkflowObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            workflow_policy_id: plan.workflow_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            workflow: ExternalRef::new("lattice", "workflow", "accepted")
                .unwrap_or_else(|_| unreachable!()),
            execution: ExternalRef::new("lattice", "execution", "accepted")
                .unwrap_or_else(|_| unreachable!()),
            sequence: 0,
            state: WorkflowState::Accepted,
            terminal: false,
            successful: false,
            outcome: Outcome::Pending,
            nodes: Vec::new(),
            logs: Vec::new(),
            receipt: None,
        }
    }

    fn completed_observation(plan: &WorkflowEffectPlan) -> WorkflowObservation {
        WorkflowObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            workflow_policy_id: plan.workflow_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            workflow: ExternalRef::new("lattice", "workflow", "completed")
                .unwrap_or_else(|_| unreachable!()),
            execution: ExternalRef::new("lattice", "execution", "completed")
                .unwrap_or_else(|_| unreachable!()),
            sequence: 1,
            state: WorkflowState::Completed,
            terminal: true,
            successful: true,
            outcome: Outcome::Success,
            nodes: vec![WorkflowNodeObservation {
                node_id: Alias::new("build").unwrap_or_else(|_| unreachable!()),
                state: WorkflowNodeState::Completed,
                terminal: true,
                successful: true,
                adapter: ExternalRef::new("lattice", "adapter", "build")
                    .unwrap_or_else(|_| unreachable!()),
                receipt: Some(
                    ExternalRef::new("lattice", "receipt", "build")
                        .unwrap_or_else(|_| unreachable!()),
                ),
            }],
            logs: Vec::new(),
            receipt: Some(
                ExternalRef::new("lattice", "receipt", "completed")
                    .unwrap_or_else(|_| unreachable!()),
            ),
        }
    }

    fn failure(code: &str) -> InfrastructureFailure {
        InfrastructureFailure {
            code: code.to_string(),
            message: "fixture workflow failure".to_string(),
        }
    }
}

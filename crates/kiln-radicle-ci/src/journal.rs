use kiln_adapter_aspen::{
    AdmittedAspenRuntimeProfile, AspenServiceDecision, AspenServiceIngress, IngressSource,
    SERVICE_ACK_SCHEMA, ServiceTransportOutcome, build_ingress,
};
use kiln_adapter_radicle::status::{
    StatusDispatch, StatusPlan, StatusWriteObservation, classify_status_outcome,
};
use kiln_core::domain::{Command, CommandEnvelope, Outcome, State, decide, evolve};
use kiln_core::operations::{JournalRecord, replay};
use kiln_core::types::{CONTRACT_VERSION_V1, CommandId, ExternalRef, Limits};
use kiln_onixos_pilot::identity;
use kiln_provider_api::TriggerRequest;
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::coordinator::CiProfile;
use crate::project::{
    ProjectSourceIntent, ProjectWorkerProfile, ProjectWorkerResult, SourceContext, admit_result,
    admit_source_intent,
};

const JOURNAL_SCHEMA: &str = "kiln.radicle-ci.journal.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiJournal {
    pub schema: String,
    pub profile_identity: kiln_core::ContentId,
    pub source: SourceContext,
    pub source_intent: ProjectSourceIntent,
    pub trigger: TriggerRequest,
    pub ingress: AspenServiceIngress,
    pub records: Vec<JournalRecord>,
    pub native_observations: Vec<ServiceTransportOutcome>,
    pub worker_result: Option<ProjectWorkerResult>,
    pub status_intent: Option<StatusPlan>,
    pub status_observation: Option<StatusDispatch>,
    pub identity: kiln_core::ContentId,
}

impl CiJournal {
    fn content_identity(&self) -> Result<kiln_core::ContentId> {
        identity(&(
            &self.schema,
            &self.profile_identity,
            &self.source,
            &self.source_intent,
            &self.trigger,
            &self.ingress,
            &self.records,
            &self.native_observations,
            &self.worker_result,
            &self.status_intent,
            &self.status_observation,
        ))
    }

    pub(crate) fn seal(&mut self) -> Result<()> {
        self.identity = self.content_identity()?;
        Ok(())
    }

    pub(crate) fn fresh(
        profile: &CiProfile,
        worker: &ProjectWorkerProfile,
        runtime: &AdmittedAspenRuntimeProfile,
        trigger: TriggerRequest,
        source_intent: ProjectSourceIntent,
    ) -> Result<Self> {
        admit_source_intent(worker, &profile.worker_profile_identity, &source_intent)?;
        if worker.trigger(&source_intent.source)? != trigger {
            return Err("broker trigger differs from the admitted exact project source".into());
        }
        let ingress =
            build_ingress(runtime, IngressSource::Radicle, trigger.clone()).map_err(text)?;
        let mut journal = Self {
            schema: JOURNAL_SCHEMA.into(),
            profile_identity: identity(profile)?,
            source: source_intent.source.clone(),
            source_intent,
            trigger,
            ingress,
            records: Vec::new(),
            native_observations: Vec::new(),
            worker_result: None,
            status_intent: None,
            status_observation: None,
            identity: identity(&JOURNAL_SCHEMA)?,
        };
        let command = start_command(profile, worker, &journal.source)?;
        journal.append(profile, command, "start")?;
        journal.seal()?;
        Ok(journal)
    }

    pub(crate) fn append(
        &mut self,
        profile: &CiProfile,
        command: Command,
        label: &str,
    ) -> Result<()> {
        append_records(profile, &self.ingress, &mut self.records, command, label)
    }

    // r[impl kiln.radicle_ci.journal] r[impl kiln.radicle_ci.recovery]
    pub(crate) fn validate(
        &self,
        profile: &CiProfile,
        worker: &ProjectWorkerProfile,
        runtime: &AdmittedAspenRuntimeProfile,
    ) -> Result<State> {
        if self.schema != JOURNAL_SCHEMA
            || self.profile_identity != identity(profile)?
            || self.identity != self.content_identity()?
            || self.records.len() > profile.max_journal_records
            || self.native_observations.len() > profile.max_journal_records
            || self.source != self.source_intent.source
            || worker.trigger(&self.source)? != self.trigger
        {
            return Err("CI journal schema, profile, source, identity or bound mismatch".into());
        }
        admit_source_intent(
            worker,
            &profile.worker_profile_identity,
            &self.source_intent,
        )?;
        let expected_ingress =
            build_ingress(runtime, IngressSource::Radicle, self.trigger.clone()).map_err(text)?;
        if self.ingress != expected_ingress {
            return Err("CI journal ingress substitution".into());
        }
        let bindings = worker.bindings(&profile.worker_profile_identity, &self.source)?;
        for native in &self.native_observations {
            admit_native(&self.ingress, &bindings.run_id, native)?;
        }
        let mut expected = Vec::new();
        append_records(
            profile,
            &self.ingress,
            &mut expected,
            start_command(profile, worker, &self.source)?,
            "start",
        )?;
        let state = replay(&State::default(), &self.records).map_err(text)?;
        if terminal_outcome(&state, &bindings.attempt_id)?.is_some() {
            let command = self
                .completion(profile, worker)?
                .ok_or("terminal CI journal lacks exact source/worker and native completion")?;
            append_records(profile, &self.ingress, &mut expected, command, "complete")?;
        } else if let Some(result) = &self.worker_result {
            admit_result(
                worker,
                &profile.worker_profile_identity,
                &self.trigger,
                result,
            )?;
            if result.source_intent != self.source_intent {
                return Err("worker changed the reserved signed source".into());
            }
        }
        if self.records != expected {
            return Err("CI journal contains noncanonical or substituted events".into());
        }
        self.validate_status(profile, worker, &state)?;
        Ok(state)
    }

    pub(crate) fn completion(
        &self,
        profile: &CiProfile,
        worker: &ProjectWorkerProfile,
    ) -> Result<Option<Command>> {
        let Some(result) = &self.worker_result else {
            return Ok(None);
        };
        if result.source_intent != self.source_intent {
            return Err("worker changed the reserved signed source".into());
        }
        let (outcome, products) = admit_result(
            worker,
            &profile.worker_profile_identity,
            &self.trigger,
            result,
        )?;
        if matches!(outcome, Outcome::Pending | Outcome::Unknown) {
            return Ok(None);
        }
        let mut receipt = None;
        for native in &self.native_observations {
            if let ServiceTransportOutcome::Acknowledged(ack)
            | ServiceTransportOutcome::Rejected(ack) = native
            {
                if let Some(host_outcome) = ack_outcome(ack.decision) {
                    if host_outcome != outcome
                        && !(host_outcome == Outcome::DomainFailure
                            && matches!(
                                outcome,
                                Outcome::ProviderError | Outcome::TimedOut | Outcome::Cancelled
                            ))
                    {
                        return Err(
                            "native host terminal conflicts with actual source/worker outcome"
                                .into(),
                        );
                    }
                    receipt = Some(ack.receipt_ref.clone());
                }
            }
        }
        let Some(receipt) = receipt else {
            return Ok(None);
        };
        let bindings = worker.bindings(&profile.worker_profile_identity, &self.source)?;
        Ok(Some(Command::CompleteAttempt {
            attempt_id: bindings.attempt_id,
            outcome,
            external_reference: Some(receipt),
            products,
            metrics: Vec::new(),
        }))
    }

    pub(crate) fn status_plan(
        &self,
        profile: &CiProfile,
        worker: &ProjectWorkerProfile,
        state: &State,
    ) -> Result<Option<StatusPlan>> {
        let bindings = worker.bindings(&profile.worker_profile_identity, &self.source)?;
        let Some(outcome) = terminal_outcome(state, &bindings.attempt_id)? else {
            return Ok(None);
        };
        Ok(Some(StatusPlan {
            repository: self.trigger.repository.clone(),
            revision: self.trigger.after.clone(),
            object_id: None,
            expected_head: None,
            run_id: bindings.run_id,
            idempotency_key: ExternalRef::new("kiln", "idempotency-key", bindings.key.as_str())
                .map_err(text)?,
            outcome: classify_status_outcome(outcome).map_err(text)?,
            max_payload_bytes: profile.status_profile.max_payload_bytes,
        }))
    }

    fn validate_status(
        &self,
        profile: &CiProfile,
        worker: &ProjectWorkerProfile,
        state: &State,
    ) -> Result<()> {
        if let Some(plan) = &self.status_intent {
            if self.status_plan(profile, worker, state)?.as_ref() != Some(plan) {
                return Err("status intent differs from canonical exact-revision outcome".into());
            }
            if let Some(observation) = &self.status_observation {
                admit_status_observation(profile, plan, observation)?;
            }
        } else if self.status_observation.is_some() {
            return Err("status observation exists without durable publication intent".into());
        }
        Ok(())
    }
}

fn start_command(
    profile: &CiProfile,
    worker: &ProjectWorkerProfile,
    source: &SourceContext,
) -> Result<Command> {
    let bindings = worker.bindings(&profile.worker_profile_identity, source)?;
    Ok(Command::StartRun {
        run_id: bindings.run_id,
        attempt_id: bindings.attempt_id,
        provider_profile: kiln_core::ProviderProfileId::new(
            &worker.build_profile.provider_profile_id,
        )
        .map_err(text)?,
        effect_id: bindings.build_intent.effect_id,
    })
}

fn append_records(
    profile: &CiProfile,
    ingress: &AspenServiceIngress,
    records: &mut Vec<JournalRecord>,
    command: Command,
    label: &str,
) -> Result<()> {
    let mut state = replay(&State::default(), records).map_err(text)?;
    let command_identity = identity(&("kiln.radicle-ci.command.v1", label, &ingress.operation_id))?;
    let envelope = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: CommandId::new(format!("radicle-ci-{label}-{}", command_identity.as_str()))
            .map_err(text)?,
        expected_revision: state.revision,
        command,
    };
    let decision = decide(&state, &envelope, Limits::default()).map_err(text)?;
    if records
        .len()
        .checked_add(decision.events.len())
        .ok_or("journal record count overflow")?
        > profile.max_journal_records
    {
        return Err("CI journal record bound reached".into());
    }
    for event in decision.events {
        let (_, content) =
            kiln_codec::encode_and_identify(&event, profile.max_journal_bytes).map_err(text)?;
        let event_identity =
            ExternalRef::new("kiln", "ci-event-blake3", content.as_str()).map_err(text)?;
        let expected_revision = state.revision;
        state = evolve(&state, &event).map_err(text)?;
        records.push(JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream: ingress.operation_id.clone(),
            expected_revision,
            event_identity,
            event,
        });
    }
    Ok(())
}

pub(crate) fn terminal_outcome(
    state: &State,
    attempt: &kiln_core::AttemptId,
) -> Result<Option<Outcome>> {
    let outcome = state
        .attempts
        .get(attempt)
        .ok_or("canonical CI attempt is absent")?
        .outcome;
    Ok((!matches!(outcome, Outcome::Pending | Outcome::Unknown)).then_some(outcome))
}

pub(crate) fn admit_native(
    ingress: &AspenServiceIngress,
    run: &kiln_core::RunId,
    native: &ServiceTransportOutcome,
) -> Result<()> {
    match native {
        ServiceTransportOutcome::Acknowledged(ack) | ServiceTransportOutcome::Rejected(ack) => {
            if ack.schema != SERVICE_ACK_SCHEMA
                || ack.operation_id != ingress.operation_id
                || ack.generation != ingress.generation
                || ack.trigger_identity != ingress.trigger_identity
                || ack.run_id.as_ref() != Some(run)
            {
                return Err("native acknowledgement differs from reserved operation/run".into());
            }
            ack.receipt_ref.validate().map_err(text)?;
        }
        ServiceTransportOutcome::DisconnectedAfterAcceptance { operation_id }
            if operation_id != &ingress.operation_id =>
        {
            return Err("disconnected native operation identity differs".into());
        }
        _ => {}
    }
    Ok(())
}

fn ack_outcome(decision: AspenServiceDecision) -> Option<Outcome> {
    match decision {
        AspenServiceDecision::Completed => Some(Outcome::Success),
        AspenServiceDecision::Failed => Some(Outcome::DomainFailure),
        AspenServiceDecision::Denied | AspenServiceDecision::Rejected => Some(Outcome::Denied),
        AspenServiceDecision::Cancelled => Some(Outcome::Cancelled),
        AspenServiceDecision::TimedOut => Some(Outcome::TimedOut),
        _ => None,
    }
}

pub(crate) fn admit_status_observation(
    profile: &CiProfile,
    plan: &StatusPlan,
    observation: &StatusDispatch,
) -> Result<()> {
    match observation {
        StatusDispatch::Written(written) => admit_written(profile, plan, written),
        StatusDispatch::Unknown(unknown) => {
            if unknown.repository != plan.repository
                || unknown.revision != plan.revision
                || unknown.run_id != plan.run_id.as_str()
                || unknown.idempotency_key != plan.idempotency_key.native_id.as_str()
            {
                return Err("unknown status observation differs from durable intent".into());
            }
            Ok(())
        }
    }
}

fn admit_written(
    profile: &CiProfile,
    plan: &StatusPlan,
    written: &StatusWriteObservation,
) -> Result<()> {
    if written.repository != plan.repository
        || written.revision != plan.revision
        || written.run_id != plan.run_id.as_str()
        || written.idempotency_key != plan.idempotency_key.native_id.as_str()
        || written.outcome != plan.outcome
        || written.signer_identity != profile.status_profile.signer_identity
    {
        return Err(
            "status write observation differs from exact revision/run/outcome/signer intent".into(),
        );
    }
    written.object_id.validate().map_err(text)?;
    written.head.validate().map_err(text)?;
    Ok(())
}

fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
#[path = "journal_tests.rs"]
pub(crate) mod tests;

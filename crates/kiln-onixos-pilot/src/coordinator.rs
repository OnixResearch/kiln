use crate::storage::JournalLock;
use crate::{
    PilotProfile, Result, SignedRequest, WORKER_RESULT_SCHEMA, WorkerResult, authenticate,
    identity, load_json, now_unix,
};
use kiln_adapter_aspen::{
    AspenServiceAck, AspenServiceDecision, AspenServiceIngress, AspenServiceTransport,
    IngressSource, SERVICE_ACK_SCHEMA, ServiceTransportOutcome, UnixAspenServiceTransport,
    build_ingress,
};
use kiln_core::build::{BuildEvaluationOutcome, admit_build_evaluation};
use kiln_core::discovery::{DiscoveryTerminal, admit_discovery_observation};
use kiln_core::domain::{Command, CommandEnvelope, Outcome, Product, State, decide, evolve};
use kiln_core::operations::{JournalRecord, replay as replay_journal};
use kiln_core::types::{CONTRACT_VERSION_V1, CommandId, ExternalRef, Limits};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const JOURNAL_SCHEMA: &str = "kiln.onixos-pilot.journal.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PilotJournal {
    pub schema: String,
    pub request: SignedRequest,
    pub ingress: AspenServiceIngress,
    pub records: Vec<JournalRecord>,
    pub native_observations: Vec<ServiceTransportOutcome>,
    pub worker_result: Option<WorkerResult>,
    pub identity: kiln_core::ContentId,
}

impl PilotJournal {
    fn content_identity(&self) -> Result<kiln_core::ContentId> {
        identity(&(
            &self.schema,
            &self.request,
            &self.ingress,
            &self.records,
            &self.native_observations,
            &self.worker_result,
        ))
    }
    fn seal(&mut self) -> Result<()> {
        self.identity = self.content_identity()?;
        Ok(())
    }
    fn append(&mut self, profile: &PilotProfile, command: Command, label: &str) -> Result<()> {
        if self.records.len() >= profile.max_journal_records {
            return Err("journal record bound reached".into());
        }
        let mut state = replay_journal(&State::default(), &self.records)
            .map_err(|error| format!("journal replay: {error:?}"))?;
        let envelope = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: CommandId::new(format!(
                "pilot-{label}-{}",
                identity(&self.request.request)?.as_str()
            ))
            .map_err(|error| format!("command identity: {error:?}"))?,
            expected_revision: state.revision,
            command,
        };
        let decision = decide(&state, &envelope, Limits::default())
            .map_err(|error| format!("canonical decision: {error:?}"))?;
        for event in decision.events {
            let (_, content) = kiln_codec::encode_and_identify(&event, profile.max_journal_bytes)
                .map_err(|error| format!("canonical event encoding: {error:?}"))?;
            let event_identity = ExternalRef::new("kiln", "pilot-event-blake3", content.as_str())
                .map_err(|error| format!("event identity: {error:?}"))?;
            let expected_revision = state.revision;
            state =
                evolve(&state, &event).map_err(|error| format!("canonical evolve: {error:?}"))?;
            self.records.push(JournalRecord {
                schema_version: CONTRACT_VERSION_V1,
                stream: self.ingress.operation_id.clone(),
                expected_revision,
                event_identity,
                event,
            });
        }
        Ok(())
    }
    fn fresh(profile: &PilotProfile, request: SignedRequest) -> Result<Self> {
        let runtime = profile.admit()?;
        let ingress = build_ingress(
            &runtime,
            IngressSource::Operator,
            request.request.trigger.clone(),
        )
        .map_err(|error| format!("operator ingress: {error:?}"))?;
        let mut journal = Self {
            schema: JOURNAL_SCHEMA.into(),
            identity: identity(&request)?,
            request,
            ingress,
            records: Vec::new(),
            native_observations: Vec::new(),
            worker_result: None,
        };
        journal.append(
            profile,
            Command::StartRun {
                run_id: journal.request.request.run_id.clone(),
                attempt_id: journal.request.request.attempt_id.clone(),
                provider_profile: profile.build_plan.provider_profile.clone(),
                effect_id: profile.build_plan.effect_id.clone(),
            },
            "start",
        )?;
        journal.seal()?;
        Ok(journal)
    }
    fn validate(&self, profile: &PilotProfile) -> Result<State> {
        // Freshness is checked at reservation, not during recovery of an already signed intent.
        authenticate(profile, &self.request, self.request.request.issued_at_unix)?;
        if self.schema != JOURNAL_SCHEMA
            || self.content_identity()? != self.identity
            || self.records.len() > profile.max_journal_records
            || self.native_observations.len() > profile.max_journal_records
        {
            return Err("journal schema, identity or bound mismatch".into());
        }
        let mut expected = Self::fresh(profile, self.request.clone())?;
        if expected.ingress != self.ingress {
            return Err("journal ingress identity drift".into());
        }
        for observation in &self.native_observations {
            validate_native(&self.ingress, &self.request, observation)?;
        }
        if let Some(worker) = &self.worker_result {
            admitted_worker(profile, worker)?;
        }
        let state = replay_journal(&State::default(), &self.records)
            .map_err(|error| format!("journal replay: {error:?}"))?;
        let terminal = state
            .attempts
            .get(&self.request.request.attempt_id)
            .ok_or("journal missing requested attempt")?
            .outcome
            .is_terminal();
        if terminal {
            let command = completion(profile, self)?
                .ok_or("terminal journal lacks matching native and worker evidence")?;
            expected.append(profile, command, "complete")?;
        }
        if expected.records != self.records {
            return Err("journal contains noncanonical or substituted records".into());
        }
        Ok(state)
    }
}

fn validate_native(
    ingress: &AspenServiceIngress,
    request: &SignedRequest,
    observation: &ServiceTransportOutcome,
) -> Result<()> {
    match observation {
        ServiceTransportOutcome::Acknowledged(ack) | ServiceTransportOutcome::Rejected(ack) => {
            if ack.schema != SERVICE_ACK_SCHEMA
                || ack.operation_id != ingress.operation_id
                || ack.generation != ingress.generation
                || ack.trigger_identity != ingress.trigger_identity
                || ack.run_id.as_ref() != Some(&request.request.run_id)
            {
                return Err("native acknowledgement identity mismatch".into());
            }
            ack.receipt_ref
                .validate()
                .map_err(|error| format!("native receipt: {error:?}"))?;
        }
        ServiceTransportOutcome::DisconnectedAfterAcceptance { operation_id }
            if operation_id != &ingress.operation_id =>
        {
            return Err("disconnected operation identity mismatch".into());
        }
        _ => {}
    }
    Ok(())
}

fn admitted_worker(
    profile: &PilotProfile,
    worker: &WorkerResult,
) -> Result<(Outcome, Vec<Product>)> {
    if worker.schema != WORKER_RESULT_SCHEMA
        || worker.job_identity != profile.job_identity
        || worker.source_uri != profile.source_uri
        || worker.discovery_plan != profile.discovery_plan
    {
        return Err("worker result job/source/discovery plan substitution".into());
    }
    let tasks = admit_discovery_observation(&profile.discovery_plan, &worker.discovery_observation)
        .map_err(|error| format!("discovery admission: {error:?}"))?;
    if worker.discovery_observation.terminal != DiscoveryTerminal::Complete {
        if worker.build_plan.is_some() || worker.build_observation.is_some() {
            return Err("failed discovery must not carry build evidence".into());
        }
        let outcome = match worker.discovery_observation.terminal {
            DiscoveryTerminal::Failed => Outcome::DomainFailure,
            DiscoveryTerminal::Cancelled => Outcome::Cancelled,
            DiscoveryTerminal::TimedOut => Outcome::TimedOut,
            DiscoveryTerminal::ProviderError => Outcome::ProviderError,
            DiscoveryTerminal::Unknown => Outcome::Unknown,
            DiscoveryTerminal::Complete => return Err("invalid discovery branch".into()),
        };
        return Ok((outcome, Vec::new()));
    }
    let task = tasks
        .iter()
        .find(|task| task.native_attribute.native_id.as_str() == profile.selected_task.as_str())
        .ok_or("selected real discovery task is absent")?;
    let plan = worker
        .build_plan
        .as_ref()
        .ok_or("successful discovery lacks actual build plan")?;
    let observation = worker
        .build_observation
        .as_ref()
        .ok_or("successful discovery lacks actual build observation")?;
    let mut outputs = task
        .outputs
        .iter()
        .map(|output| output.name.clone())
        .collect::<Vec<_>>();
    outputs.sort();
    if plan != &profile.build_plan
        || task.build_provider != plan.provider_profile
        || outputs != plan.required_products
    {
        return Err("build plan differs from exact profile or admitted discovery task".into());
    }
    admit_build_evaluation(plan, observation)
        .map_err(|error| format!("build admission: {error:?}"))?;
    let outcome = match observation.outcome {
        BuildEvaluationOutcome::Success => Outcome::Success,
        BuildEvaluationOutcome::Failed => Outcome::DomainFailure,
        BuildEvaluationOutcome::Cancelled => Outcome::Cancelled,
        BuildEvaluationOutcome::TimedOut => Outcome::TimedOut,
        BuildEvaluationOutcome::ProviderError => Outcome::ProviderError,
        BuildEvaluationOutcome::Unknown => Outcome::Unknown,
    };
    let products = observation
        .products
        .iter()
        .map(|product| Product {
            identity: product.artifact_identity.clone(),
            producing_attempt: observation.attempt_id.clone(),
            provider_reference: None,
        })
        .collect();
    Ok((outcome, products))
}

fn ack_outcome(ack: &AspenServiceAck) -> Option<Outcome> {
    match ack.decision {
        AspenServiceDecision::Completed => Some(Outcome::Success),
        AspenServiceDecision::Failed => Some(Outcome::DomainFailure),
        AspenServiceDecision::Denied | AspenServiceDecision::Rejected => Some(Outcome::Denied),
        AspenServiceDecision::Cancelled => Some(Outcome::Cancelled),
        AspenServiceDecision::TimedOut => Some(Outcome::TimedOut),
        _ => None,
    }
}
fn completion(profile: &PilotProfile, journal: &PilotJournal) -> Result<Option<Command>> {
    let Some(worker) = &journal.worker_result else {
        return Ok(None);
    };
    let (outcome, products) = admitted_worker(profile, worker)?;
    if !outcome.is_terminal() {
        return Ok(None);
    }
    let mut terminal = None;
    for native in &journal.native_observations {
        if let ServiceTransportOutcome::Acknowledged(ack) | ServiceTransportOutcome::Rejected(ack) =
            native
        {
            if let Some(native_outcome) = ack_outcome(ack) {
                let compatible = native_outcome == outcome
                    || (native_outcome == Outcome::DomainFailure
                        && matches!(
                            outcome,
                            Outcome::ProviderError | Outcome::TimedOut | Outcome::Cancelled
                        ));
                if !compatible {
                    return Err("native host phase conflicts with actual worker outcome".into());
                }
                terminal = Some(ack);
            }
        }
    }
    let Some(ack) = terminal else {
        return Ok(None);
    };
    Ok(Some(Command::CompleteAttempt {
        attempt_id: journal.request.request.attempt_id.clone(),
        outcome,
        external_reference: Some(ack.receipt_ref.clone()),
        products,
        metrics: Vec::new(),
    }))
}

fn transport(profile: &PilotProfile) -> Result<UnixAspenServiceTransport> {
    UnixAspenServiceTransport::new(
        profile.socket_path.clone(),
        profile.maximum_frame_bytes,
        Duration::from_secs(profile.transport_timeout_seconds),
    )
    .map_err(str::to_owned)
}
fn observe(
    profile: &PilotProfile,
    journal: &mut PilotJournal,
    native: ServiceTransportOutcome,
) -> Result<()> {
    validate_native(&journal.ingress, &journal.request, &native)?;
    if journal.native_observations.len() >= profile.max_journal_records {
        return Err("native observation bound reached".into());
    }
    journal.native_observations.push(native);
    if let Some(command) = completion(profile, journal)? {
        journal.append(profile, command, "complete")?;
    }
    journal.seal()
}

// r[impl kiln.onixos_pilot.trigger]
// r[impl kiln.onixos_pilot.journal]
pub fn start(profile: &PilotProfile, signed: SignedRequest, path: &Path) -> Result<PilotJournal> {
    authenticate(profile, &signed, now_unix()?)?;
    let lock = JournalLock::acquire(path)?;
    if path.try_exists().map_err(|error| error.to_string())? {
        let journal: PilotJournal = load_json(path, profile.max_journal_bytes)?;
        journal.validate(profile)?;
        if journal.request.request != signed.request {
            return Err("journal already reserves another logical request".into());
        }
        return Ok(journal); // Redelivery never submits or reconciles implicitly.
    }
    let mut journal = PilotJournal::fresh(profile, signed)?;
    let mut client = transport(profile)?;
    lock.store(&journal, profile.max_journal_bytes)?; // Durable StartRun intent precedes any host effect.
    let runtime = profile.admit()?;
    let outcome = client.submit(&runtime, &journal.ingress);
    observe(profile, &mut journal, outcome)?;
    lock.store(&journal, profile.max_journal_bytes)?;
    Ok(journal)
}

// r[impl kiln.onixos_pilot.recovery]
// r[impl kiln.onixos_pilot.failure]
pub fn reconcile(
    profile: &PilotProfile,
    path: &Path,
    result: Option<&Path>,
) -> Result<PilotJournal> {
    let lock = JournalLock::acquire(path)?;
    let mut journal: PilotJournal = load_json(path, profile.max_journal_bytes)?;
    let state = journal.validate(profile)?;
    if let Some(path) = result {
        let worker: WorkerResult = load_json(path, profile.max_journal_bytes)?;
        admitted_worker(profile, &worker)?;
        if journal
            .worker_result
            .as_ref()
            .is_some_and(|old| old != &worker)
        {
            return Err("immutable worker result changed".into());
        }
        journal.worker_result = Some(worker);
    }
    if state
        .attempts
        .get(&journal.request.request.attempt_id)
        .ok_or("missing attempt")?
        .outcome
        .is_terminal()
    {
        return Ok(journal);
    }
    let outcome = transport(profile)?.reconcile(&profile.admit()?, &journal.ingress.operation_id);
    // Retain observations even when completion admission rejects conflicting truth.
    let admission = observe(profile, &mut journal, outcome);
    journal.seal()?;
    lock.store(&journal, profile.max_journal_bytes)?;
    admission?;
    Ok(journal)
}

// r[impl kiln.onixos_pilot.journal]
pub fn replay(profile: &PilotProfile, path: &Path) -> Result<State> {
    load_replayed(profile, path).map(|(_, state)| state)
}

pub(crate) fn load_replayed(profile: &PilotProfile, path: &Path) -> Result<(PilotJournal, State)> {
    let _lock = JournalLock::acquire(path)?;
    let journal: PilotJournal = load_json(path, profile.max_journal_bytes)?;
    let state = journal.validate(profile)?;
    Ok((journal, state))
}

#[cfg(test)]
mod tests;

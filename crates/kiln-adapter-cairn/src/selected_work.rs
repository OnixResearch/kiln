//! Admission of Cairn's `cairn.selected-work.v1` snapshot into Kiln types.
//!
//! The producer's own portable admission and obligation node-fact verification
//! decide producer validity. This adapter only reads the record through the
//! bounded Cairn runtime, refuses anything the producer refuses, and maps the
//! admitted record into provider-neutral `kiln-core` facts. Guidance prose is
//! never parsed and transport success grants no authority.
use std::{ffi::OsString, path::PathBuf};

use bounded_exec::ExecutionLimits;
use cairn_core::verified::selected_work::{
    self as producer, SelectedObligationExport, SelectedObligations, SelectedWorkError,
    SelectedWorkPolicy, SelectedWorkSnapshot,
};
use cairn_core::{
    EvidencePredicateKind, ObligationNodeKind as ProducerNodeKind, ObligationSelectionState,
    verify_obligation_node_facts, verify_predicate_identity,
};
use kiln_core::implementation::selected_work::{
    AdmittedSelectedWork, ArtifactStatus, CompletionInputs, ContextEntryFact, ContextFacts,
    DependencyFact, EnumerationFacts, EvidenceKind, EvidenceObservationState, EvidenceRequirement,
    FactState, IntakeProfile, NodeSelection, ObligationEvaluation, ObligationEvidenceObservation,
    ObligationNode, ObligationNodeKind, ObligationOutcome, ObligationPlan, ObligationSection,
    ReadinessFacts, ReferenceKind, ReferenceScope, RemainingCondition, RemainingKind,
    RemainingWorkFact, SelectedWorkCohort, SourceArtifactFact, SourceFacts, SourceRevisionKind,
    SourceRootKind, StableBlocker, StageReadiness, StageState, UnitState, WorkReference,
    WorkSelection, WorkUnitFact,
};
use kiln_core::types::ContentId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::campaign::CairnCampaignProfile;
use crate::{CairnProcessOutput, CairnRuntime, ProcessInvocation};

/// Producer schema this adapter admits.
pub const SELECTED_WORK_SCHEMA: &str = producer::SCHEMA;
/// Largest admitted snapshot; the producer's own budgets bound it far below.
pub const MAX_SELECTED_WORK_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 256 * 1024 * 1024;
const TIMEOUT_MS: u64 = 30_000;
const STDERR_BYTES: usize = 65_536;
const POLL_INTERVAL_MS: u64 = 25;
const TEARDOWN_TIMEOUT_MS: u64 = 1_000;
const REFUSAL_PREFIX: &str = "selected-work rejected: ";

/// Typed refusal. Producer variants keep the producer's names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectedWorkRefusal {
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
    /// Bytes did not decode as the exact versioned wire record.
    Malformed,
    /// The read failed, timed out, was cancelled or has an unknown outcome.
    Unavailable,
}

impl SelectedWorkRefusal {
    #[must_use]
    pub const fn from_producer(error: SelectedWorkError) -> Self {
        match error {
            SelectedWorkError::InvalidPolicy => Self::InvalidPolicy,
            SelectedWorkError::UnsupportedSchema => Self::UnsupportedSchema,
            SelectedWorkError::InvalidIdentity => Self::InvalidIdentity,
            SelectedWorkError::InvalidSelection => Self::InvalidSelection,
            SelectedWorkError::Duplicate => Self::Duplicate,
            SelectedWorkError::InvalidSource => Self::InvalidSource,
            SelectedWorkError::Incomplete => Self::Incomplete,
            SelectedWorkError::BudgetExceeded => Self::BudgetExceeded,
            SelectedWorkError::CrossedSubject => Self::CrossedSubject,
            SelectedWorkError::StaleCohort => Self::StaleCohort,
            SelectedWorkError::InvalidReadiness => Self::InvalidReadiness,
            SelectedWorkError::InvalidContext => Self::InvalidContext,
            SelectedWorkError::UnsafePortable => Self::UnsafePortable,
            SelectedWorkError::InvalidObligations => Self::InvalidObligations,
        }
    }

    /// Parse the producer CLI's documented refusal line.
    #[must_use]
    pub fn from_stderr(stderr: &[u8]) -> Self {
        let text = String::from_utf8_lossy(stderr);
        let Some(variant) = text
            .lines()
            .find_map(|line| line.split_once(REFUSAL_PREFIX).map(|(_, rest)| rest.trim()))
        else {
            return Self::Unavailable;
        };
        serde_json::from_value(Value::String(snake(variant))).unwrap_or(Self::Unavailable)
    }
}

impl std::fmt::Display for SelectedWorkRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{REFUSAL_PREFIX}{self:?}")
    }
}

impl std::error::Error for SelectedWorkRefusal {}

fn snake(variant: &str) -> String {
    let mut out = String::with_capacity(variant.len().saturating_add(4));
    for (index, character) in variant.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

/// One selected-work read.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkRequest {
    pub change: String,
    pub task: Option<String>,
    /// Admitted obligation definition, such as `verify`.
    pub obligation_definition: Option<String>,
    /// Observation file relative to the repository root.
    pub obligation_observations: Option<PathBuf>,
    /// Cohort retained from an earlier admitted read.
    pub expected: Option<SelectedWorkCohort>,
}

/// Bounded read-only `cairn selected-work` reader.
#[derive(Debug)]
pub struct CairnSelectedWorkReader<R> {
    profile: CairnCampaignProfile,
    runtime: R,
}

impl<R: CairnRuntime> CairnSelectedWorkReader<R> {
    pub fn new(profile: CairnCampaignProfile, runtime: R) -> Result<Self, String> {
        if !profile.executable.is_absolute()
            || !profile.executable.starts_with("/nix/store")
            || !profile.repository_root.is_absolute()
            || !profile.policy_path.is_absolute()
            || profile.executable_identity.validate().is_err()
            || profile.policy_identity.validate().is_err()
        {
            return Err(
                "Cairn selected-work requires an immutable executable and exact policy identity"
                    .into(),
            );
        }
        Ok(Self { profile, runtime })
    }

    /// Read and admit one snapshot. Every failure is a typed refusal; an
    /// unavailable read is never an empty snapshot.
    // r[impl kiln.selected_work.admission]
    pub fn read(
        &mut self,
        request: &SelectedWorkRequest,
    ) -> Result<AdmittedSelectedWork, SelectedWorkRefusal> {
        let invocation = self.invocation(request)?;
        let observed = self
            .runtime
            .executable_identity(&self.profile.executable, MAX_EXECUTABLE_BYTES)
            .map_err(|_| SelectedWorkRefusal::Unavailable)?;
        if observed != self.profile.executable_identity {
            return Err(SelectedWorkRefusal::Unavailable);
        }
        match self
            .runtime
            .run(&invocation)
            .map_err(|_| SelectedWorkRefusal::Unavailable)?
        {
            CairnProcessOutput::Exited {
                process_status: Some(0),
                stdout,
                ..
            } => {
                let admitted = admit_selected_work(&stdout, request.expected.as_ref())?;
                if admitted.policy.as_str() != self.profile.policy_identity.as_str()
                    || admitted.selection.change != request.change
                    || admitted.selection.task != request.task
                {
                    return Err(SelectedWorkRefusal::CrossedSubject);
                }
                Ok(admitted)
            }
            CairnProcessOutput::Exited {
                process_status: Some(1),
                stderr,
                ..
            } => Err(SelectedWorkRefusal::from_stderr(&stderr)),
            _ => Err(SelectedWorkRefusal::Unavailable),
        }
    }

    fn invocation(
        &self,
        request: &SelectedWorkRequest,
    ) -> Result<ProcessInvocation, SelectedWorkRefusal> {
        let selector = |value: &str| {
            !value.is_empty()
                && value.len() <= 256
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        };
        if !selector(&request.change)
            || request.task.as_deref().is_some_and(|task| !selector(task))
            || request
                .obligation_definition
                .as_deref()
                .is_some_and(|definition| !selector(definition))
            || (request.obligation_observations.is_some()
                && request.obligation_definition.is_none())
        {
            return Err(SelectedWorkRefusal::InvalidSelection);
        }
        let text = |path: &std::path::Path| {
            path.to_str()
                .filter(|s| !s.is_empty() && !s.contains('\0'))
                .map(OsString::from)
                .ok_or(SelectedWorkRefusal::InvalidSelection)
        };
        let mut args: Vec<OsString> = vec![
            "selected-work".into(),
            request.change.clone().into(),
            "--root".into(),
            text(&self.profile.repository_root)?,
            "--policy".into(),
            text(&self.profile.policy_path)?,
            "--disclosure".into(),
            "portable".into(),
        ];
        if let Some(task) = &request.task {
            args.extend(["--task".into(), task.clone().into()]);
        }
        if let Some(definition) = &request.obligation_definition {
            args.extend(["--obligation-definition".into(), definition.clone().into()]);
        }
        if let Some(path) = &request.obligation_observations {
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err(SelectedWorkRefusal::InvalidSelection);
            }
            args.extend(["--obligation-observations".into(), text(path)?]);
        }
        if let Some(expected) = &request.expected {
            let cohort = serde_json::to_string(&producer_cohort(expected))
                .map_err(|_| SelectedWorkRefusal::InvalidIdentity)?;
            args.extend(["--expected-cohort".into(), cohort.into()]);
        }
        Ok(ProcessInvocation {
            program: self.profile.executable.clone(),
            args,
            current_dir: self.profile.repository_root.clone(),
            environment: Vec::new(),
            limits: ExecutionLimits {
                timeout_ms: TIMEOUT_MS,
                stdin_max_bytes: 0,
                stdout_max_bytes: MAX_SELECTED_WORK_BYTES,
                stderr_max_bytes: STDERR_BYTES,
                poll_interval_ms: POLL_INTERVAL_MS,
                teardown_timeout_ms: TEARDOWN_TIMEOUT_MS,
            },
        })
    }
}

/// Admit exact producer wire bytes against an expected cohort (the record's
/// own cohort on a first read) and map them into Kiln types.
// r[impl kiln.selected_work.admission.scenario.refused]
pub fn admit_selected_work(
    wire: &[u8],
    expected: Option<&SelectedWorkCohort>,
) -> Result<AdmittedSelectedWork, SelectedWorkRefusal> {
    if wire.is_empty() || wire.len() > MAX_SELECTED_WORK_BYTES {
        return Err(SelectedWorkRefusal::BudgetExceeded);
    }
    let value: Value = serde_json::from_slice(wire).map_err(|_| SelectedWorkRefusal::Malformed)?;
    if value.get("schema").and_then(Value::as_str) != Some(SELECTED_WORK_SCHEMA) {
        return Err(SelectedWorkRefusal::UnsupportedSchema);
    }
    let snapshot: SelectedWorkSnapshot =
        serde_json::from_value(value).map_err(|_| SelectedWorkRefusal::Malformed)?;
    let expected = match expected {
        Some(cohort) => producer_cohort(cohort),
        None => snapshot.cohort.clone(),
    };
    let policy = SelectedWorkPolicy::bundled().map_err(SelectedWorkRefusal::from_producer)?;
    producer::admit_portable(&policy, &snapshot, &expected)
        .map_err(SelectedWorkRefusal::from_producer)?;
    if let SelectedObligations::Exported(export) = &snapshot.obligations
        && (!verify_predicate_identity(&export.predicate)
            || verify_obligation_node_facts(&export.predicate, &export.guidance, &export.nodes)
                .is_err())
    {
        return Err(SelectedWorkRefusal::InvalidObligations);
    }
    let admitted = map_snapshot(&snapshot, blake3::hash(wire).to_hex().as_str())?;
    admitted
        .validate()
        .map_err(|_| SelectedWorkRefusal::CrossedSubject)?;
    Ok(admitted)
}

fn producer_cohort(cohort: &SelectedWorkCohort) -> producer::CohortIdentity {
    producer::CohortIdentity {
        source: cohort.source.as_str().into(),
        intent: cohort.intent.as_str().into(),
        context: cohort.context.as_str().into(),
        verification_subject: cohort.verification_subject.as_str().into(),
        dependencies: cohort.dependencies.as_str().into(),
        obligations: cohort.obligations.as_str().into(),
        selection: cohort.selection.as_str().into(),
        observation: cohort.observation.as_str().into(),
    }
}

fn cid(value: &str) -> Result<ContentId, SelectedWorkRefusal> {
    ContentId::from_blake3_hex(value).map_err(|_| SelectedWorkRefusal::InvalidIdentity)
}

fn cid_opt(value: Option<&String>) -> Result<Option<ContentId>, SelectedWorkRefusal> {
    value.map(|value| cid(value)).transpose()
}

fn fact_state(state: producer::Availability) -> FactState {
    match state {
        producer::Availability::Ready => FactState::Ready,
        producer::Availability::Blocked => FactState::Blocked,
        producer::Availability::Unavailable => FactState::Unavailable,
        producer::Availability::Unsupported => FactState::Unsupported,
    }
}

/// Typed requirement/acceptance references keep their kind and scope.
// r[impl kiln.selected_work.references]
fn map_snapshot(
    snapshot: &SelectedWorkSnapshot,
    wire_identity: &str,
) -> Result<AdmittedSelectedWork, SelectedWorkRefusal> {
    let cohort = &snapshot.cohort;
    Ok(AdmittedSelectedWork {
        snapshot: cid(wire_identity)?,
        intake: IntakeProfile::TypedSnapshotV1,
        producer_schema: snapshot.schema.clone(),
        selection: WorkSelection {
            change: snapshot.selection.change.clone(),
            task: snapshot.selection.task.clone(),
        },
        cohort: SelectedWorkCohort {
            source: cid(&cohort.source)?,
            intent: cid(&cohort.intent)?,
            context: cid(&cohort.context)?,
            verification_subject: cid(&cohort.verification_subject)?,
            dependencies: cid(&cohort.dependencies)?,
            obligations: cid(&cohort.obligations)?,
            selection: cid(&cohort.selection)?,
            observation: cid(&cohort.observation)?,
        },
        source: SourceFacts {
            revision_kind: match snapshot.source.revision.kind {
                producer::SourceRevisionKind::Exact => SourceRevisionKind::Exact,
                producer::SourceRevisionKind::Projection => SourceRevisionKind::Projection,
            },
            revision: snapshot.source.revision.identity.clone(),
            artifacts: snapshot
                .source
                .artifacts
                .iter()
                .map(|artifact| {
                    Ok(SourceArtifactFact {
                        root: match artifact.root {
                            producer::SourceRoot::Lifecycle => SourceRootKind::Lifecycle,
                            producer::SourceRoot::Repository => SourceRootKind::Repository,
                        },
                        path: artifact.path.clone(),
                        bytes: artifact.bytes,
                        content: cid(&artifact.blake3)?,
                        status: match artifact.status {
                            producer::SourceStatus::Present => ArtifactStatus::Present,
                            producer::SourceStatus::Missing => ArtifactStatus::Missing,
                            producer::SourceStatus::Unavailable => ArtifactStatus::Unavailable,
                        },
                    })
                })
                .collect::<Result<_, _>>()?,
        },
        policy: cid(&snapshot.policy_hash)?,
        profile: snapshot.profile.clone(),
        profile_identity: cid(&snapshot.profile_hash)?,
        completion: CompletionInputs {
            state: fact_state(snapshot.completion.state),
            contract: cid_opt(snapshot.completion.contract_hash.as_ref())?,
            results: cid_opt(snapshot.completion.results_hash.as_ref())?,
            subject: cid_opt(snapshot.completion.subject_hash.as_ref())?,
        },
        context: ContextFacts {
            selection: cid(&snapshot.context.selection_hash)?,
            portable: cid(&snapshot.context.portable_hash)?,
            entries: snapshot
                .context
                .entries
                .iter()
                .map(|entry| {
                    Ok(ContextEntryFact {
                        id: entry.id.clone(),
                        authority: entry.authority.clone(),
                        source_root: entry.source_root.clone(),
                        path: entry.path.clone(),
                        status: fact_state(entry.status),
                        content: cid_opt(entry.content_hash.as_ref())?,
                    })
                })
                .collect::<Result<_, _>>()?,
        },
        readiness: map_readiness(&snapshot.readiness)?,
        units: snapshot
            .units
            .iter()
            .map(|unit| WorkUnitFact {
                task: unit.task.clone(),
                state: match unit.state {
                    producer::WorkState::Ready => UnitState::Ready,
                    producer::WorkState::Blocked => UnitState::Blocked,
                    producer::WorkState::Complete => UnitState::Complete,
                    producer::WorkState::Unavailable => UnitState::Unavailable,
                },
                references: unit
                    .references
                    .iter()
                    .map(|reference| WorkReference {
                        kind: match reference.kind {
                            producer::ReferenceKind::Requirement => ReferenceKind::Requirement,
                            producer::ReferenceKind::Acceptance => ReferenceKind::Acceptance,
                        },
                        id: reference.id.clone(),
                        scope: match &reference.scope {
                            producer::ReferenceScope::Change => ReferenceScope::Change,
                            producer::ReferenceScope::Task(task) => {
                                ReferenceScope::Task(task.clone())
                            }
                        },
                    })
                    .collect(),
                verification_contracts: unit.verification_contracts.clone(),
            })
            .collect(),
        remaining: snapshot
            .remaining
            .iter()
            .map(|item| RemainingWorkFact {
                kind: match item.kind {
                    producer::RemainingKind::Implementation => RemainingKind::Implementation,
                    producer::RemainingKind::Completion => RemainingKind::Completion,
                    producer::RemainingKind::Sync => RemainingKind::Sync,
                    producer::RemainingKind::Archive => RemainingKind::Archive,
                },
                state: fact_state(item.state),
                subject: item.subject.clone(),
            })
            .collect(),
        dependencies: snapshot
            .dependencies
            .iter()
            .map(|fact| {
                Ok(DependencyFact {
                    task: fact.task.clone(),
                    prerequisite: fact.prerequisite.clone(),
                    state: fact_state(fact.state),
                    identity: cid(&fact.identity)?,
                })
            })
            .collect::<Result<_, _>>()?,
        enumeration: EnumerationFacts {
            complete: snapshot.enumeration.complete,
            observed: snapshot.enumeration.observed,
            total: snapshot.enumeration.total,
        },
        obligations: match &snapshot.obligations {
            SelectedObligations::NotSelected => ObligationSection::NotSelected,
            SelectedObligations::Exported(export) => {
                ObligationSection::Exported(Box::new(map_plan(export)?))
            }
        },
    })
}

fn map_readiness(
    readiness: &producer::PortableReadiness,
) -> Result<ReadinessFacts, SelectedWorkRefusal> {
    Ok(ReadinessFacts {
        receipt: cid(&readiness.native_receipt_hash)?,
        input: cid(&readiness.native_input_hash)?,
        policy: cid(&readiness.native_policy_hash)?,
        source: cid(&readiness.native_scope.source_hash)?,
        evidence: cid(&readiness.native_scope.evidence_hash)?,
        valid: readiness.valid,
        classification: readiness.classification.clone(),
        stages: readiness
            .stages
            .iter()
            .map(|stage| {
                Ok(StageReadiness {
                    stage: stage.stage.clone(),
                    state: match stage.state {
                        cairn_core::verified::readiness::ReadinessState::Ready => StageState::Ready,
                        cairn_core::verified::readiness::ReadinessState::Blocked => {
                            StageState::Blocked
                        }
                        cairn_core::verified::readiness::ReadinessState::Unavailable => {
                            StageState::Unavailable
                        }
                        cairn_core::verified::readiness::ReadinessState::NotApplicable => {
                            StageState::NotApplicable
                        }
                    },
                    decision: cid(&stage.decision_hash)?,
                })
            })
            .collect::<Result<_, _>>()?,
        blockers: readiness
            .blockers
            .iter()
            .map(|blocker| StableBlocker {
                id: blocker.id.clone(),
                stage: blocker.stage.clone(),
                code: blocker.code.clone(),
                target: blocker.target.clone(),
            })
            .collect(),
        advisory_actions: readiness
            .actions
            .iter()
            .map(|action| action.id.clone())
            .collect(),
    })
}

fn map_plan(export: &SelectedObligationExport) -> Result<ObligationPlan, SelectedWorkRefusal> {
    let evaluation = &export.evaluation;
    Ok(ObligationPlan {
        workflow_id: export.binding.workflow_id.clone(),
        workflow_version: export.binding.workflow_version,
        definition_id: export.binding.definition_id.clone(),
        binding: cid(&export.binding.binding_blake3)?,
        intent: cid(&export.binding.intent_blake3)?,
        workflow_policy: cid(&export.binding.workflow_policy_blake3)?,
        predicate: cid(&export.predicate.predicate_blake3)?,
        subject: cid(&export.predicate.subject_blake3)?,
        context: cid(&export.predicate.context_blake3)?,
        selected_alternatives: export.predicate.selected_alternatives.clone(),
        nodes: export
            .nodes
            .iter()
            .map(|node| {
                Ok(ObligationNode {
                    id: node.id.clone(),
                    kind: match node.kind {
                        ProducerNodeKind::Evidence => ObligationNodeKind::Evidence,
                        ProducerNodeKind::AllOf => ObligationNodeKind::AllOf,
                        ProducerNodeKind::AnyOf => ObligationNodeKind::AnyOf,
                        ProducerNodeKind::Applicable => ObligationNodeKind::Applicable,
                        ProducerNodeKind::Temporal => ObligationNodeKind::Temporal,
                        ProducerNodeKind::Exception => ObligationNodeKind::Exception,
                    },
                    parent: node.parent.clone(),
                    alternative: node.alternative.clone(),
                    children: node.children.clone(),
                    selection: match node.selection {
                        ObligationSelectionState::Required => NodeSelection::Required,
                        ObligationSelectionState::SelectedAlternative => {
                            NodeSelection::SelectedAlternative
                        }
                        ObligationSelectionState::Conditional => NodeSelection::Conditional,
                        ObligationSelectionState::SelectionMissing => {
                            NodeSelection::SelectionMissing
                        }
                        ObligationSelectionState::UnselectedAlternative => {
                            NodeSelection::UnselectedAlternative
                        }
                    },
                    conditions: node.conditions.clone(),
                    evidence: node
                        .evidence
                        .iter()
                        .map(|reference| {
                            Ok(EvidenceRequirement {
                                predicate: reference.predicate.clone(),
                                kind: match reference.kind {
                                    EvidencePredicateKind::Evidence => EvidenceKind::Evidence,
                                    EvidencePredicateKind::Applicability => {
                                        EvidenceKind::Applicability
                                    }
                                },
                                requirement_id: reference.requirement_id.clone(),
                                owner: reference.owner.clone(),
                                accepted_revision: cid(&reference.accepted_revision_blake3)?,
                                reference: cid(&reference.reference_blake3)?,
                            })
                        })
                        .collect::<Result<_, _>>()?,
                    obligation: cid(&node.obligation_blake3)?,
                    guidance: cid(&node.guidance_blake3)?,
                    condition: cid(&node.condition_blake3)?,
                })
            })
            .collect::<Result<_, _>>()?,
        observations_supplied: export.observations_supplied,
        evaluation: ObligationEvaluation {
            outcome: match evaluation.outcome.as_str() {
                "proved" => ObligationOutcome::Proved,
                "exception_accepted" => ObligationOutcome::ExceptionAccepted,
                "unsatisfied" => ObligationOutcome::Unsatisfied,
                "blocked" => ObligationOutcome::Blocked,
                "abandoned" => ObligationOutcome::Abandoned,
                "unknown" => ObligationOutcome::Unknown,
                "unsupported" => ObligationOutcome::Unsupported,
                _ => return Err(SelectedWorkRefusal::InvalidObligations),
            },
            proved: evaluation.proved,
            successful_delivery: evaluation.successful_delivery,
            satisfied: evaluation.satisfied.clone(),
            remaining: evaluation
                .remaining
                .iter()
                .map(|item| RemainingCondition {
                    id: item.id.clone(),
                    kind: item.kind.clone(),
                    state: item.state.clone(),
                    reason: item.reason.clone(),
                })
                .collect(),
            report: cid(&evaluation.report_blake3)?,
        },
    })
}

/// Encode Kiln-derived evidence as the producer's `--obligation-observations`
/// file. Selected alternatives come from the bound producer plan, never from
/// worker output.
#[must_use]
pub fn producer_observation_file(
    plan: &ObligationPlan,
    evidence: &[ObligationEvidenceObservation],
) -> Value {
    json!({
        "selected_alternatives": plan.selected_alternatives,
        "evidence": evidence.iter().map(|item| json!({
            "predicate": item.predicate,
            "requirement_id": item.requirement_id,
            "revision_blake3": item.revision.as_str(),
            "subject_blake3": item.subject.as_str(),
            "context_blake3": item.context.as_str(),
            "policy_blake3": item.policy.as_str(),
            "observation_id": format!("kiln-check:{}", item.observation.as_str()),
            "state": match item.state {
                EvidenceObservationState::Satisfied => "satisfied",
                EvidenceObservationState::Unsatisfied => "unsatisfied",
                EvidenceObservationState::Unknown => "unknown",
            },
        })).collect::<Vec<_>>(),
        "progress": [],
        "agent_claims": null,
        "exceptions": [],
        "temporal": null,
        "terminal": null,
    })
}

#[cfg(test)]
#[path = "selected_work_tests.rs"]
mod tests;

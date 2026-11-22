//! Agent revisions, capability and wait reasons of the Cairn workspace (#84),
//! for Lattice #24 and #25.
//!
//! A pure read of Kiln's admitted resource inventory projection: the
//! definitions' revision slots, task-family qualification references with
//! their provenance, declared capability, the workers' revision pins, the
//! owner-attributed wait reasons and, apart from them, the workflow revision
//! slots. It never reads producer documents, storage, a clock or the
//! environment, and it copies an explicit allow-list of fields: no session,
//! workspace, execution or grant reference, model-service binding, full
//! qualification record, preparation failure or other work's attempt, and no
//! guidance, context or source identity of a workflow.
use std::collections::BTreeSet;

use kiln_core::inventory::{
    AgentDefinition, CompiledWorkflow, EvidenceProvenance, FactRef, InventoryKind,
    InventoryProjection, ObservationState, Permission, PinStatus, RevisionManagement, RevisionPin,
    SelectionKind, SourceStatus, WaitReason, WaitSubject, WorkerInstance, WorkerLifecycle,
    WorkflowRevision, WorkflowRevisionSlots,
};
use serde::Serialize;

/// Wait reason a producer attaches to each revision fact it leaves unknown.
const REVISION_FACT_UNKNOWN: &str = "revision_fact_unknown";
/// Unknown-field name of the whole revision state, for producers that do not
/// split it into slots.
const REVISIONS_FIELD: &str = "revisions";
/// Unknown-field name of a worker's revision pin, and of a workflow
/// definition's pinned revisions.
const PINNED_FIELD: &str = "pinned";

/// The resource inventory as the detail route observed it. Its failure is
/// independent of the canonical Cairn facts.
#[derive(Clone, Debug)]
pub enum InventoryObservation {
    /// No `--inventory` input is configured.
    NotConfigured,
    /// The input was refused; the code names why.
    Unavailable(String),
    /// The admitted projection for the requesting principal.
    Admitted(Box<InventoryProjection>),
}

/// One revision fact. `state` is `recorded`, `not_recorded`, `unknown`,
/// `unmanaged` or `unpublished`; `owner` names who can resolve an unknown or
/// unpublished fact when someone does.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Slot {
    pub state: &'static str,
    pub value: Option<String>,
    pub owner: Option<String>,
}

impl Slot {
    fn new(state: &'static str, value: Option<String>, owner: Option<String>) -> Self {
        Self {
            state,
            value,
            owner,
        }
    }
}

/// The worker revision a workflow revision pairs with, and how it relates to
/// the worker definition the inventory projects.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkerPairingView {
    pub definition: FactRef,
    pub revision: String,
    pub bundle: String,
    /// `profile_revision` when it is the definition's projected revision,
    /// `other_revision`, or `definition_not_observed`.
    pub observed: &'static str,
}

/// One workflow revision: its compiled workflow identities and its worker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowRevisionView {
    pub revision: String,
    pub workflow: CompiledWorkflow,
    pub worker: WorkerPairingView,
}

/// One workflow revision slot. `state` is `recorded`, `not_recorded` or
/// `unknown`; `revision` names an entry of the definition's `revisions`. No
/// producer names an owner for an unknown workflow slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowSlotView {
    pub state: &'static str,
    pub revision: Option<String>,
}

/// Sessions pinned to one workflow revision in one pin state, counted, never
/// named. `revision` names an entry of the definition's `revisions`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowPinView {
    pub revision: String,
    pub status: PinStatus,
    pub blocked: bool,
    pub sessions: u64,
    /// `matches_selected`, `differs_from_selected` or `no_selection`.
    pub selection: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowDefinitionView {
    pub reference: FactRef,
    pub observation: ObservationState,
    /// Each revision the slots and pins name, once, ascending by revision.
    pub revisions: Vec<WorkflowRevisionView>,
    pub desired: WorkflowSlotView,
    pub last_usable: WorkflowSlotView,
    pub selected_for_new_sessions: WorkflowSlotView,
    /// Rollout generation, kind and qualification reference of a recorded selection.
    pub rollout_generation: Option<u64>,
    pub selection_kind: Option<SelectionKind>,
    pub qualification: Option<String>,
    /// `recorded` when sessions are pinned, `not_recorded` when none are, or `unknown`.
    pub pinned_state: &'static str,
    pub pinned: Vec<WorkflowPinView>,
}

/// Workflow revisions, kept apart from the definitions' and workers'
/// revisions, with their own failure boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowRevisions {
    /// `admitted`, `not_configured` or `unavailable`.
    pub source: &'static str,
    /// The refusal code of unavailable workflow revisions.
    pub reason: Option<String>,
    pub sources: Vec<SourceView>,
    pub definitions: Vec<WorkflowDefinitionView>,
}

impl WorkflowRevisions {
    fn unobserved(source: &'static str, reason: Option<String>) -> Self {
        Self {
            source,
            reason,
            sources: Vec::new(),
            definitions: Vec::new(),
        }
    }
}

/// A producer wait reason: its code, `fact:<field>` or `route:<name>`, and
/// owner. No free text.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct WaitView {
    pub code: String,
    pub subject: String,
    pub owner: String,
}

/// A declared capability list: `declared`, `none` (known to be empty) or
/// `unknown`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Capability {
    pub state: &'static str,
    pub names: Vec<String>,
}

/// A task-family qualification reference. It is evidence, never authority,
/// and only `live` provenance counts as live evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QualificationView {
    pub reference: String,
    pub evidence_class: String,
    pub provenance: EvidenceProvenance,
    pub counts_as_live: bool,
}

/// One task family: declared by the definition and/or qualified for its
/// projected revision. `state` is `live`, `fixture` or `unqualified`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TaskFamilyView {
    pub task_family: String,
    pub declared: bool,
    pub state: &'static str,
    pub qualification: Option<QualificationView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DefinitionView {
    pub reference: FactRef,
    pub observation: ObservationState,
    pub profile_revision: String,
    /// `managed`, `unmanaged` or `unpublished`.
    pub management: &'static str,
    pub desired: Slot,
    pub prepared: Slot,
    pub qualified: Slot,
    pub selected_for_new_sessions: Slot,
    /// Rollout generation of a recorded selection.
    pub rollout_generation: Option<u64>,
    /// Declared task-family support: `declared`, `none` or `unknown`.
    pub task_families_state: &'static str,
    pub task_families: Vec<TaskFamilyView>,
    pub tools: Capability,
    pub harnesses: Capability,
    pub output_contracts: Capability,
    pub models: Capability,
    pub authority: Permission,
    pub wait_reasons: Vec<WaitView>,
    /// Workers in this section joined to the definition.
    pub workers: usize,
}

/// A worker's revision pin. A recorded pin carries its facts; `blocked` means
/// a further effect or export would be refused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PinView {
    /// `recorded`, `unmanaged`, `unknown` or `unpublished`.
    pub state: &'static str,
    pub owner: Option<String>,
    pub qualification: Option<String>,
    pub rollout_generation: Option<u64>,
    pub identity: Option<String>,
    pub status: Option<PinStatus>,
    pub blocked: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkerView {
    pub reference: FactRef,
    pub observation: ObservationState,
    pub definition: FactRef,
    pub lifecycle: WorkerLifecycle,
    pub pinned_revision: String,
    /// `matches_selected`, `differs_from_selected`, `no_selection` or
    /// `definition_not_observed`.
    pub selection: &'static str,
    pub pin: PinView,
    /// `this_change`, `other_work` or `no_attempt`.
    pub work: &'static str,
    /// The attempt and fence, only when they are this change's.
    pub attempt: Option<String>,
    pub fence: Option<u64>,
    pub wait_reasons: Vec<WaitView>,
}

/// Status of one agents or workers source of the inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceView {
    pub source: String,
    pub producer: String,
    pub kind: InventoryKind,
    pub state: ObservationState,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentRevisions {
    /// `admitted`, `not_configured` or `unavailable`.
    pub source: &'static str,
    /// The refusal code of an unavailable inventory.
    pub reason: Option<String>,
    pub inventory_identity: Option<String>,
    pub evaluated_at_ms: Option<u64>,
    pub synthetic: Option<bool>,
    pub sources: Vec<SourceView>,
    pub workflow_revisions: WorkflowRevisions,
    pub definitions: Vec<DefinitionView>,
    pub workers: Vec<WorkerView>,
}

fn waits(reasons: &[WaitReason]) -> Vec<WaitView> {
    let mut views: Vec<WaitView> = reasons
        .iter()
        .map(|reason| WaitView {
            code: reason.code.clone(),
            subject: reason.subject.constraint(),
            owner: reason.owner.clone(),
        })
        .collect();
    views.sort();
    views.dedup();
    views
}

/// The owner a producer named for one unknown revision fact.
fn unknown_owner(reasons: &[WaitReason], field: &str) -> Option<String> {
    reasons
        .iter()
        .find(|reason| {
            reason.code == REVISION_FACT_UNKNOWN
                && matches!(&reason.subject, WaitSubject::Fact { field: named } if named == field)
        })
        .map(|reason| reason.owner.clone())
}

// r[impl kiln.cairn.workspace.agents.scenario.slots]
/// One definition revision slot. An unknown fact wins: the producer said it
/// does not know. Then a recorded value, then the lifecycle's own state.
fn slot(definition: &AgentDefinition, name: &str, value: Option<&String>) -> Slot {
    let field = format!("{REVISIONS_FIELD}.{name}");
    let unknown = &definition.meta.unknown_fields;
    if unknown
        .iter()
        .any(|listed| listed == &field || listed == REVISIONS_FIELD)
    {
        let owner = unknown_owner(&definition.wait_reasons, &field)
            .or_else(|| unknown_owner(&definition.wait_reasons, REVISIONS_FIELD));
        return Slot::new("unknown", None, owner);
    }
    if let Some(value) = value {
        return Slot::new("recorded", Some(value.clone()), None);
    }
    match definition.revisions.management {
        Some(RevisionManagement::Unmanaged) => Slot::new("unmanaged", None, None),
        Some(RevisionManagement::Managed) => Slot::new("not_recorded", None, None),
        None => Slot::new("unpublished", None, None),
    }
}

fn management(definition: &AgentDefinition) -> &'static str {
    match definition.revisions.management {
        Some(RevisionManagement::Managed) => "managed",
        Some(RevisionManagement::Unmanaged) => "unmanaged",
        None => "unpublished",
    }
}

fn capability_state(definition: &AgentDefinition, field: &str, names: &[String]) -> &'static str {
    if definition.meta.unknown_fields.iter().any(|f| f == field) {
        "unknown"
    } else if names.is_empty() {
        "none"
    } else {
        "declared"
    }
}

fn capability(definition: &AgentDefinition, field: &str, names: &[String]) -> Capability {
    Capability {
        state: capability_state(definition, field, names),
        names: names.to_vec(),
    }
}

// r[impl kiln.cairn.workspace.agents.scenario.provenance]
/// Declared families first in declaration order, then qualified families the
/// definition does not declare. Only `live` provenance counts as live.
fn task_families(definition: &AgentDefinition) -> Vec<TaskFamilyView> {
    let qualification = |family: &str| {
        definition
            .task_family_qualifications
            .iter()
            .find(|q| q.task_family == family)
            .map(|q| QualificationView {
                reference: q.reference.clone(),
                evidence_class: q.evidence_class.clone(),
                provenance: q.provenance,
                counts_as_live: q.provenance == EvidenceProvenance::Live,
            })
    };
    let view = |family: &str, declared: bool| {
        let qualification = qualification(family);
        TaskFamilyView {
            task_family: family.to_owned(),
            declared,
            state: match qualification.as_ref().map(|q| q.provenance) {
                Some(EvidenceProvenance::Live) => "live",
                Some(EvidenceProvenance::Fixture) => "fixture",
                None => "unqualified",
            },
            qualification,
        }
    };
    let declared: BTreeSet<&str> = definition
        .task_families
        .iter()
        .map(String::as_str)
        .collect();
    definition
        .task_families
        .iter()
        .map(|family| view(family, true))
        .chain(
            definition
                .task_family_qualifications
                .iter()
                .filter(|q| !declared.contains(q.task_family.as_str()))
                .map(|q| view(&q.task_family, false)),
        )
        .collect()
}

fn fact_state(projection: &InventoryProjection, reference: &FactRef) -> ObservationState {
    projection
        .facts
        .iter()
        .find(|fact| &fact.reference == reference)
        .map_or(ObservationState::Unknown, |fact| fact.state)
}

fn definition_view(
    projection: &InventoryProjection,
    definition: &AgentDefinition,
) -> DefinitionView {
    let revisions = &definition.revisions;
    let selected = slot(
        definition,
        "selected_for_new_sessions",
        revisions.selected_for_new_sessions.as_ref(),
    );
    DefinitionView {
        reference: definition.meta.reference.clone(),
        observation: fact_state(projection, &definition.meta.reference),
        profile_revision: definition.profile_revision.clone(),
        management: management(definition),
        desired: slot(definition, "desired", revisions.desired.as_ref()),
        prepared: slot(definition, "prepared", revisions.prepared.as_ref()),
        qualified: slot(definition, "qualified", revisions.qualified.as_ref()),
        rollout_generation: (selected.state == "recorded").then_some(revisions.generation),
        selected_for_new_sessions: selected,
        task_families_state: capability_state(
            definition,
            "task_families",
            &definition.task_families,
        ),
        task_families: task_families(definition),
        tools: capability(definition, "tools", &definition.tools),
        harnesses: capability(definition, "harnesses", &definition.harnesses),
        output_contracts: capability(definition, "output_contracts", &definition.output_contracts),
        models: capability(definition, "permitted_models", &definition.permitted_models),
        authority: definition.authority,
        wait_reasons: waits(&definition.wait_reasons),
        workers: projection
            .snapshot
            .workers
            .iter()
            .filter(|worker| worker.definition == definition.meta.reference)
            .count(),
    }
}

fn pin_view(worker: &WorkerInstance) -> PinView {
    let empty = |state, owner| PinView {
        state,
        owner,
        qualification: None,
        rollout_generation: None,
        identity: None,
        status: None,
        blocked: None,
    };
    if worker
        .meta
        .unknown_fields
        .iter()
        .any(|field| field == PINNED_FIELD)
    {
        return empty("unknown", unknown_owner(&worker.wait_reasons, PINNED_FIELD));
    }
    match &worker.pin {
        None => empty("unpublished", None),
        Some(RevisionPin::Unmanaged {}) => empty("unmanaged", None),
        Some(RevisionPin::Recorded {
            qualification,
            rollout_generation,
            identity,
            status,
            blocked,
        }) => PinView {
            state: "recorded",
            owner: None,
            qualification: Some(qualification.clone()),
            rollout_generation: Some(*rollout_generation),
            identity: Some(identity.clone()),
            status: Some(*status),
            blocked: Some(*blocked),
        },
    }
}

// r[impl kiln.cairn.workspace.agents.scenario.selection]
fn selection(definition: Option<&AgentDefinition>, worker: &WorkerInstance) -> &'static str {
    let Some(definition) = definition else {
        return "definition_not_observed";
    };
    match &definition.revisions.selected_for_new_sessions {
        None => "no_selection",
        Some(selected) if selected == &worker.pinned_revision => "matches_selected",
        Some(_) => "differs_from_selected",
    }
}

// r[impl kiln.cairn.workspace.agents.scenario.redaction]
fn worker_view(
    projection: &InventoryProjection,
    worker: &WorkerInstance,
    attempts: &BTreeSet<(&str, u64)>,
) -> WorkerView {
    let definition = projection
        .snapshot
        .agents
        .iter()
        .find(|definition| definition.meta.reference == worker.definition);
    let this_change = match (worker.attempt.as_deref(), worker.fence) {
        (Some(attempt), Some(fence)) => attempts.contains(&(attempt, fence)),
        _ => false,
    };
    WorkerView {
        reference: worker.meta.reference.clone(),
        observation: fact_state(projection, &worker.meta.reference),
        definition: worker.definition.clone(),
        lifecycle: worker.lifecycle,
        pinned_revision: worker.pinned_revision.clone(),
        selection: selection(definition, worker),
        pin: pin_view(worker),
        work: if this_change {
            "this_change"
        } else if worker.attempt.is_some() {
            "other_work"
        } else {
            "no_attempt"
        },
        attempt: worker.attempt.clone().filter(|_| this_change),
        fence: worker.fence.filter(|_| this_change),
        wait_reasons: waits(&worker.wait_reasons),
    }
}

fn unobserved(source: &'static str, reason: Option<String>) -> AgentRevisions {
    AgentRevisions {
        source,
        reason: reason.clone(),
        inventory_identity: None,
        evaluated_at_ms: None,
        synthetic: None,
        sources: Vec::new(),
        workflow_revisions: WorkflowRevisions::unobserved(source, reason),
        definitions: Vec::new(),
        workers: Vec::new(),
    }
}

fn source_view(source: &SourceStatus) -> SourceView {
    SourceView {
        source: source.source.clone(),
        producer: source.producer.clone(),
        kind: source.kind,
        state: source.state,
        reason: source.reason.clone(),
    }
}

// r[impl kiln.cairn.workspace.agents.scenario.workflow]
/// The paired worker revision against the worker definition the inventory
/// projects.
fn workflow_revision(
    projection: &InventoryProjection,
    revision: &WorkflowRevision,
) -> WorkflowRevisionView {
    let worker = &revision.worker;
    let observed = projection
        .snapshot
        .agents
        .iter()
        .find(|definition| definition.meta.reference == worker.definition)
        .map_or("definition_not_observed", |definition| {
            if definition.profile_revision == worker.revision {
                "profile_revision"
            } else {
                "other_revision"
            }
        });
    WorkflowRevisionView {
        revision: revision.revision.clone(),
        workflow: revision.workflow.clone(),
        worker: WorkerPairingView {
            definition: worker.definition.clone(),
            revision: worker.revision.clone(),
            bundle: worker.bundle.clone(),
            observed,
        },
    }
}

/// One workflow slot. An unknown slot wins, then a recorded revision; a
/// workflow lifecycle is always managed, so an empty slot is `not_recorded`.
fn workflow_slot(
    slots: &WorkflowRevisionSlots,
    field: &str,
    value: Option<&str>,
) -> WorkflowSlotView {
    if slots
        .meta
        .unknown_fields
        .iter()
        .any(|listed| listed == field)
    {
        return WorkflowSlotView {
            state: "unknown",
            revision: None,
        };
    }
    match value.filter(|value| slots.revision(value).is_some()) {
        Some(revision) => WorkflowSlotView {
            state: "recorded",
            revision: Some(revision.to_owned()),
        },
        None => WorkflowSlotView {
            state: "not_recorded",
            revision: None,
        },
    }
}

fn workflow_definition(
    projection: &InventoryProjection,
    slots: &WorkflowRevisionSlots,
) -> WorkflowDefinitionView {
    let selection = slots.selected_for_new_sessions.as_ref();
    let selected = workflow_slot(
        slots,
        "selected_for_new_sessions",
        selection.map(|selection| selection.revision.as_str()),
    );
    let recorded = selected.state == "recorded";
    let pinned_unknown = slots.meta.unknown_fields.iter().any(|f| f == PINNED_FIELD);
    let pinned: Vec<WorkflowPinView> = slots
        .pinned
        .iter()
        .filter(|pin| slots.revision(&pin.revision).is_some())
        .map(|pin| WorkflowPinView {
            revision: pin.revision.clone(),
            status: pin.status,
            blocked: pin.blocked,
            sessions: pin.sessions,
            selection: match selection {
                None => "no_selection",
                Some(selection) if selection.revision == pin.revision => "matches_selected",
                Some(_) => "differs_from_selected",
            },
        })
        .collect();
    WorkflowDefinitionView {
        reference: slots.meta.reference.clone(),
        observation: fact_state(projection, &slots.meta.reference),
        revisions: slots
            .revisions
            .iter()
            .map(|revision| workflow_revision(projection, revision))
            .collect(),
        desired: workflow_slot(slots, "desired", slots.desired.as_deref()),
        last_usable: workflow_slot(slots, "last_usable", slots.last_usable.as_deref()),
        selected_for_new_sessions: selected,
        rollout_generation: selection
            .filter(|_| recorded)
            .map(|selection| selection.rollout_generation),
        selection_kind: selection
            .filter(|_| recorded)
            .map(|selection| selection.kind),
        qualification: selection
            .filter(|_| recorded)
            .map(|selection| selection.qualification.clone()),
        pinned_state: if pinned_unknown {
            "unknown"
        } else if pinned.is_empty() {
            "not_recorded"
        } else {
            "recorded"
        },
        pinned: if pinned_unknown { Vec::new() } else { pinned },
    }
}

/// The reason no configured workflow source was observed.
fn workflow_unavailable_reason(sources: &[&SourceStatus]) -> String {
    sources
        .iter()
        .find_map(|source| source.reason.clone())
        .unwrap_or_else(|| {
            let state = sources.first().map(|source| source.state);
            match state {
                Some(ObservationState::Missing) => "workflow_revisions_source_missing",
                Some(ObservationState::OutOfOrder) => "workflow_revisions_source_out_of_order",
                _ => "workflow_revisions_source_unavailable",
            }
            .to_owned()
        })
}

// r[impl kiln.cairn.workspace.agents.scenario.workflow_source]
/// Workflow revisions from the admitted inventory. No configured workflow
/// source is `not_configured`; configured sources none of which was observed
/// are `unavailable` with the first refusal code. Either leaves the rest of
/// the section as it is.
fn workflow_revisions(projection: &InventoryProjection) -> WorkflowRevisions {
    let sources: Vec<&SourceStatus> = projection
        .sources
        .iter()
        .filter(|source| source.kind == InventoryKind::WorkflowRevisions)
        .collect();
    if sources.is_empty() {
        return WorkflowRevisions::unobserved("not_configured", None);
    }
    let observed = sources.iter().any(|source| {
        !matches!(
            source.state,
            ObservationState::Missing
                | ObservationState::OutOfOrder
                | ObservationState::Unavailable
        )
    });
    if !observed {
        let mut unavailable = WorkflowRevisions::unobserved(
            "unavailable",
            Some(workflow_unavailable_reason(&sources)),
        );
        unavailable.sources = sources.into_iter().map(source_view).collect();
        return unavailable;
    }
    WorkflowRevisions {
        source: "admitted",
        reason: None,
        sources: sources.into_iter().map(source_view).collect(),
        definitions: projection
            .snapshot
            .workflow_revisions
            .iter()
            .map(|slots| workflow_definition(projection, slots))
            .collect(),
    }
}

// r[impl kiln.cairn.workspace.agents]
// r[impl kiln.cairn.workspace.agents.scenario.source]
/// The section for one change, whose attempts are `(identity, fence)` pairs.
/// Workers bound to this change come first, then by reference.
#[must_use]
pub fn project(
    observation: &InventoryObservation,
    attempts: &BTreeSet<(&str, u64)>,
) -> AgentRevisions {
    let projection = match observation {
        InventoryObservation::NotConfigured => return unobserved("not_configured", None),
        InventoryObservation::Unavailable(code) => {
            return unobserved("unavailable", Some(code.clone()));
        }
        InventoryObservation::Admitted(projection) => projection,
    };
    let mut definitions: Vec<DefinitionView> = projection
        .snapshot
        .agents
        .iter()
        .map(|definition| definition_view(projection, definition))
        .collect();
    definitions.sort_by(|a, b| a.reference.cmp(&b.reference));
    let mut workers: Vec<WorkerView> = projection
        .snapshot
        .workers
        .iter()
        .map(|worker| worker_view(projection, worker, attempts))
        .collect();
    workers.sort_by(|a, b| {
        (a.work != "this_change", &a.reference).cmp(&(b.work != "this_change", &b.reference))
    });
    AgentRevisions {
        source: "admitted",
        reason: None,
        inventory_identity: Some(projection.identity.clone()),
        evaluated_at_ms: Some(projection.evaluated_at_ms),
        synthetic: Some(projection.snapshot.synthetic),
        sources: projection
            .sources
            .iter()
            .filter(|source| matches!(source.kind, InventoryKind::Agents | InventoryKind::Workers))
            .map(source_view)
            .collect(),
        workflow_revisions: workflow_revisions(projection),
        definitions,
        workers,
    }
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "agents_workflow_tests.rs"]
mod workflow_tests;

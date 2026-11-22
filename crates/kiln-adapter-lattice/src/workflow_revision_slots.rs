//! Lattice `lattice.workflow-agent-revision-slots.v1` documents (the
//! `outcome.workflow_slots` member of `lattice agent-revision inspect`,
//! `docs/agent-revisions.md` in Lattice) mapped into inventory v2 workflow
//! revision slots.
//!
//! The DTOs mirror Lattice's wire record field for field and deny unknown
//! fields, so a guidance, context, source or obligation identity is refused,
//! never copied. A workflow-agent definition is named by its pairing name and
//! its worker definition, so every revision of one document pairs with one
//! worker definition; a workflow revision is a content identity, so one
//! revision carries one binding wherever it appears. Session identities and
//! names are validated but not projected: pins are counted per revision,
//! status and blocked state.
// r[impl kiln.inventory.workflow]
// r[impl kiln.inventory.workflow.scenario.mapped]
// r[impl kiln.inventory.workflow.scenario.refused]
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use kiln_core::inventory::{
    CompiledWorkflow, FactMeta, FactRef, InventoryFragment, InventoryJoins, InventoryKind,
    MAX_EXACT_NUMBER, MAX_INVENTORY_ITEMS, MAX_WORKFLOW_PINNED_SESSIONS, MAX_WORKFLOW_REVISIONS,
    PinStatus, SelectionKind, SourceObservation, WorkerPairing, WorkflowPin, WorkflowRevision,
    WorkflowRevisionSlots, WorkflowSelection, admit_workflow_revisions, public_token,
};
use serde::Deserialize;

pub const WORKFLOW_REVISION_SLOTS_SCHEMA: &str = "lattice.workflow-agent-revision-slots.v1";
/// Upper bound of one slots document. Every pin repeats its revision's full
/// entry (about 1.1 KB), so 1024 pins need about 1.1 MB; 2 MiB leaves margin
/// for the bounded revision list. The collection shell reads slots documents
/// up to this bound, apart from the general producer-document bound.
pub const MAX_WORKFLOW_REVISION_SLOTS_BYTES: usize = 2_097_152;
/// Lattice bounds one token to 256 printable bytes (`MAX_TOKEN_BYTES`).
const MAX_TOKEN_BYTES: usize = 256;
/// Lattice's `blake3:` references and its `b3:` plan and cohort identities.
const REFERENCE_PREFIX: &str = "blake3:";
const PLAN_PREFIX: &str = "b3:";
const DIGEST_HEX: usize = 64;

#[derive(Deserialize)]
struct SchemaProbe {
    schema: String,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct CompiledDto {
    compiler_revision: String,
    compiler_artifact_identity: String,
    plan_identity: String,
    graph_identity: String,
    selection_identity: String,
    obligations_identity: String,
    policy_identity: String,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct WorkerDto {
    definition: String,
    definition_name: String,
    revision: String,
    bundle_identity: String,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct EntryDto {
    revision: String,
    workflow: CompiledDto,
    worker: WorkerDto,
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum KindDto {
    Promotion,
    Rollback,
}

#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SelectedDto {
    bundle: EntryDto,
    rollout_generation: u64,
    kind: KindDto,
    qualification_ref: String,
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum StatusDto {
    Live,
    Closed,
}

#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PinnedDto {
    session: String,
    bundle: EntryDto,
    status: StatusDto,
    is_blocked: bool,
}

#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SlotsDto {
    schema: String,
    definition: String,
    name: String,
    desired: Option<EntryDto>,
    last_usable: Option<EntryDto>,
    selected_for_new_sessions: Option<SelectedDto>,
    pinned: Vec<PinnedDto>,
}

/// 64 lowercase hex characters.
fn is_digest(value: &str) -> bool {
    value.len() == DIGEST_HEX
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_reference(value: &str) -> bool {
    value.strip_prefix(REFERENCE_PREFIX).is_some_and(is_digest)
}

fn is_plan_identity(value: &str) -> bool {
    value.strip_prefix(PLAN_PREFIX).is_some_and(is_digest)
}

/// Lattice's token: 1 to 256 printable bytes. A projected token must also be
/// a Kiln public token.
fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TOKEN_BYTES
        && value.bytes().all(|b| b.is_ascii_graphic())
}

fn validate_entry(entry: &EntryDto) -> Result<(), &'static str> {
    let workflow = &entry.workflow;
    let compiled = [
        public_token(&workflow.compiler_revision),
        public_token(&workflow.compiler_artifact_identity),
        is_plan_identity(&workflow.plan_identity),
        is_reference(&workflow.graph_identity),
        is_plan_identity(&workflow.selection_identity),
        is_plan_identity(&workflow.obligations_identity),
        is_plan_identity(&workflow.policy_identity),
    ];
    let worker = [
        is_reference(&entry.worker.definition),
        is_token(&entry.worker.definition_name),
        is_digest(&entry.worker.revision),
        is_reference(&entry.worker.bundle_identity),
    ];
    if !is_digest(&entry.revision) {
        return Err("lattice_workflow_revision_invalid");
    }
    if !compiled.iter().all(|check| *check) {
        return Err("lattice_workflow_identity_invalid");
    }
    if !worker.iter().all(|check| *check) {
        return Err("lattice_workflow_worker_invalid");
    }
    Ok(())
}

fn entries(slots: &SlotsDto) -> impl Iterator<Item = &EntryDto> {
    slots
        .desired
        .iter()
        .chain(slots.last_usable.iter())
        .chain(
            slots
                .selected_for_new_sessions
                .iter()
                .map(|selected| &selected.bundle),
        )
        .chain(slots.pinned.iter().map(|pin| &pin.bundle))
}

fn validate_pins(pins: &[PinnedDto]) -> Result<(), &'static str> {
    // Kiln's bound: Lattice limits live sessions at admission, and closed
    // sessions stay pinned, so a lifecycle may hold more pins than this.
    if u64::try_from(pins.len()).map_or(true, |count| count > MAX_WORKFLOW_PINNED_SESSIONS) {
        return Err("lattice_workflow_pin_bound");
    }
    let sorted = pins.windows(2).all(|pair| match pair {
        [first, second] => first.session < second.session,
        _ => false,
    });
    if !sorted || pins.iter().any(|pin| !is_token(&pin.session)) {
        return Err("lattice_workflow_pin_invalid");
    }
    // A closed session may perform no further effect under its pin.
    if pins
        .iter()
        .any(|pin| pin.status == StatusDto::Closed && !pin.is_blocked)
    {
        return Err("lattice_workflow_pin_inconsistent");
    }
    Ok(())
}

/// Every revision pairs with the definition's one worker definition, one
/// revision carries one binding wherever it appears, and at most
/// `MAX_WORKFLOW_REVISIONS` distinct revisions are named.
fn validate_bindings(slots: &SlotsDto) -> Result<(), &'static str> {
    let mut worker: Option<&str> = None;
    let mut bindings: BTreeMap<&str, &EntryDto> = BTreeMap::new();
    for entry in entries(slots) {
        validate_entry(entry)?;
        match worker {
            Some(definition) if definition != entry.worker.definition => {
                return Err("lattice_workflow_worker_binding_mismatch");
            }
            _ => worker = Some(&entry.worker.definition),
        }
        match bindings.entry(&entry.revision) {
            Entry::Vacant(slot) => {
                slot.insert(entry);
            }
            Entry::Occupied(slot) if *slot.get() == entry => {}
            Entry::Occupied(_) => return Err("lattice_workflow_revision_conflict"),
        }
        if bindings.len() > MAX_WORKFLOW_REVISIONS {
            return Err("lattice_workflow_revision_bound");
        }
    }
    Ok(())
}

fn validate(slots: &SlotsDto) -> Result<(), &'static str> {
    if !is_reference(&slots.definition) {
        return Err("lattice_workflow_definition_invalid");
    }
    if !is_token(&slots.name) {
        return Err("lattice_workflow_name_invalid");
    }
    if let Some(selected) = &slots.selected_for_new_sessions {
        let generation = selected.rollout_generation;
        if generation == 0
            || generation > MAX_EXACT_NUMBER
            || !is_reference(&selected.qualification_ref)
        {
            return Err("lattice_workflow_selection_invalid");
        }
    }
    validate_pins(&slots.pinned)?;
    validate_bindings(slots)
}

fn decode(document: &[u8]) -> Result<SlotsDto, &'static str> {
    if document.len() > MAX_WORKFLOW_REVISION_SLOTS_BYTES {
        return Err("lattice_workflow_slots_byte_bound");
    }
    let probe: SchemaProbe =
        serde_json::from_slice(document).map_err(|_| "lattice_workflow_slots_malformed")?;
    if probe.schema != WORKFLOW_REVISION_SLOTS_SCHEMA {
        return Err("lattice_workflow_slots_unsupported_schema");
    }
    let slots: SlotsDto =
        serde_json::from_slice(document).map_err(|_| "lattice_workflow_slots_malformed")?;
    debug_assert_eq!(slots.schema, WORKFLOW_REVISION_SLOTS_SCHEMA);
    validate(&slots)?;
    Ok(slots)
}

fn revision(entry: &EntryDto, joins: &InventoryJoins) -> WorkflowRevision {
    let workflow = &entry.workflow;
    WorkflowRevision {
        revision: entry.revision.clone(),
        workflow: CompiledWorkflow {
            compiler_revision: workflow.compiler_revision.clone(),
            compiler_artifact: workflow.compiler_artifact_identity.clone(),
            plan: workflow.plan_identity.clone(),
            graph: workflow.graph_identity.clone(),
            selection: workflow.selection_identity.clone(),
            obligations: workflow.obligations_identity.clone(),
            policy: workflow.policy_identity.clone(),
        },
        worker: WorkerPairing {
            definition: FactRef {
                source: joins.agents.clone(),
                id: entry.worker.definition.clone(),
            },
            revision: entry.worker.revision.clone(),
            bundle: entry.worker.bundle_identity.clone(),
        },
    }
}

/// Pins counted per revision, status and blocked state, in that order.
fn pins(pinned: &[PinnedDto]) -> Vec<WorkflowPin> {
    let mut groups: BTreeMap<(&str, u8, bool), u64> = BTreeMap::new();
    for pin in pinned {
        let status = match pin.status {
            StatusDto::Live => 0,
            StatusDto::Closed => 1,
        };
        *groups
            .entry((pin.bundle.revision.as_str(), status, pin.is_blocked))
            .or_default() += 1;
    }
    groups
        .into_iter()
        .map(|((revision, status, blocked), sessions)| WorkflowPin {
            revision: revision.to_owned(),
            status: if status == 0 {
                PinStatus::Live
            } else {
                PinStatus::Closed
            },
            blocked,
            sessions,
        })
        .collect()
}

fn slots_fact(
    slots: &SlotsDto,
    source: &SourceObservation,
    joins: &InventoryJoins,
) -> WorkflowRevisionSlots {
    // One revision view per distinct revision; pins repeat their entries.
    let mut revisions: BTreeMap<&str, WorkflowRevision> = BTreeMap::new();
    for entry in entries(slots) {
        if let Entry::Vacant(slot) = revisions.entry(entry.revision.as_str()) {
            slot.insert(revision(entry, joins));
        }
    }
    WorkflowRevisionSlots {
        meta: FactMeta {
            reference: FactRef {
                source: source.source.clone(),
                id: slots.definition.clone(),
            },
            observed_at_ms: source.observed_at_ms,
            valid_until_ms: source.valid_until_ms,
            unknown_fields: Vec::new(),
        },
        revisions: revisions.into_values().collect(),
        desired: slots.desired.as_ref().map(|entry| entry.revision.clone()),
        last_usable: slots
            .last_usable
            .as_ref()
            .map(|entry| entry.revision.clone()),
        selected_for_new_sessions: slots.selected_for_new_sessions.as_ref().map(|selected| {
            WorkflowSelection {
                revision: selected.bundle.revision.clone(),
                rollout_generation: selected.rollout_generation,
                kind: match selected.kind {
                    KindDto::Promotion => SelectionKind::Promotion,
                    KindDto::Rollback => SelectionKind::Rollback,
                },
                qualification: selected.qualification_ref.clone(),
            }
        }),
        pinned: pins(&slots.pinned),
    }
}

/// Map one source's slots documents: one fact per workflow-agent definition.
/// Repeats with equal decoded documents collapse; two
/// documents for one definition that differ in any field, sessions and names
/// included, are refused. Each fact must pass core admission, so a document
/// this reader admits can never fail admission of the whole snapshot.
pub fn map_workflow_revision_slots(
    source: &SourceObservation,
    joins: &InventoryJoins,
    documents: &[&[u8]],
) -> Result<InventoryFragment, &'static str> {
    joins.admit()?;
    if documents.len() > MAX_INVENTORY_ITEMS {
        return Err("lattice_workflow_slots_document_bound");
    }
    let mut decoded: BTreeMap<String, SlotsDto> = BTreeMap::new();
    for document in documents {
        let slots = decode(document)?;
        match decoded.entry(slots.definition.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(slots);
            }
            Entry::Occupied(slot) if *slot.get() == slots => {}
            Entry::Occupied(_) => return Err("lattice_workflow_definition_conflict"),
        }
    }
    let mut fragment = InventoryFragment::new(InventoryKind::WorkflowRevisions, source.clone());
    for slots in decoded.values() {
        let fact = slots_fact(slots, source, joins);
        admit_workflow_revisions(&fact)?;
        fragment.workflow_revisions.push(fact);
    }
    Ok(fragment)
}

#[cfg(test)]
#[path = "workflow_revision_slots_tests.rs"]
mod tests;

//! Explicit producer collection for inventory v2.
//!
//! A `kiln.inventory.collection-set.v1` manifest names, per source, the Kiln
//! collection window and the producer documents the collecting shell retained,
//! by paths relative to the manifest. This shell reads exactly those regular
//! files under named byte bounds, maps them through the producer adapters and
//! composes one snapshot by exact producer-scoped joins. It performs no
//! discovery, network access or write; a live host shell that collects
//! authorized observations writes the same manifest and documents.
//!
//! Workflow revision slots are display-only: no readiness decision reads
//! them. A slots document that cannot be read or is refused therefore makes
//! only its source unavailable, with the refusal code as the source's reason,
//! and never fails the rest of the collection.
// r[impl kiln.inventory.collection]
// r[impl kiln.inventory.collection.scenario.refused]
// r[impl kiln.inventory.workflow.scenario.isolated]
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use kiln_core::inventory::{
    InventoryFragment, InventoryJoins, InventoryKind, InventoryProfile, InventorySnapshot,
    MAX_INVENTORY_ITEMS, MAX_INVENTORY_SOURCES, SourceObservation, admit_snapshot,
    compose_snapshot, isolate_workflow_revisions,
};
use kiln_provider_api::inventory::{InventoryObservationPort, InventoryReadFailure};
use serde::{Deserialize, Serialize};

use crate::inventory::InventoryInput;

pub const COLLECTION_SET_SCHEMA: &str = "kiln.inventory.collection-set.v1";
/// Upper bound of one manifest document.
pub const MAX_COLLECTION_MANIFEST_BYTES: u64 = 262_144;
/// Upper bound of one producer document; each adapter applies its own tighter
/// bound. Workflow revision slots documents have their own, larger bound.
pub const MAX_PRODUCER_DOCUMENT_BYTES: u64 = 262_144;
/// Upper bound of one inventory v2 profile document.
pub const MAX_PROFILE_BYTES: u64 = 65_536;
/// Upper bound of one relative document path.
pub const MAX_DOCUMENT_PATH_BYTES: usize = 512;

/// Producer contracts this shell maps, each into exactly the kinds listed.
pub const LATTICE_WORKER_INVENTORY: &str =
    kiln_adapter_lattice::worker_inventory::WORKER_INVENTORY_SCHEMA;
pub const COMPUTE_ALLOCATION_OBSERVATION: &str =
    kiln_adapter_lattice::compute_inventory::COMPUTE_OBSERVATION_SCHEMA;
pub const ANIMUS_SERVING_SNAPSHOT: &str = kiln_adapter_animus::SNAPSHOT_SCHEMA;
pub const LATTICE_WORKFLOW_REVISION_SLOTS: &str =
    kiln_adapter_lattice::workflow_revision_slots::WORKFLOW_REVISION_SLOTS_SCHEMA;

/// One source's collection: the Kiln-side window and the retained documents.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCollection {
    pub source: String,
    pub producer: String,
    pub scope: String,
    pub kind: InventoryKind,
    pub contract: String,
    pub generation: u64,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub complete: bool,
    pub next_page: Option<String>,
    pub unavailable_reason: Option<String>,
    /// The producer's logical clock as the collecting shell read it, if any.
    pub provider_logical_now: Option<u64>,
    /// Producer documents, relative to the manifest's directory.
    pub documents: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionSet {
    pub schema: String,
    pub principal: String,
    pub synthetic: bool,
    pub joins: InventoryJoins,
    pub collections: Vec<SourceCollection>,
}

fn read_bounded(path: &Path, limit: u64, unavailable: &'static str) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| unavailable)?;
    if !metadata.is_file() {
        return Err("inventory_collection_document_not_regular".into());
    }
    let file = File::open(path).map_err(|_| unavailable)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err("inventory_collection_byte_bound".into());
    }
    Ok(bytes)
}

/// Only plain relative paths below the manifest directory are admitted.
fn document_path(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    let is_plain = !relative.is_empty()
        && relative.len() <= MAX_DOCUMENT_PATH_BYTES
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !is_plain {
        return Err("inventory_collection_path_invalid".into());
    }
    Ok(base.join(path))
}

fn source_observation(collection: &SourceCollection) -> SourceObservation {
    SourceObservation {
        source: collection.source.clone(),
        producer: collection.producer.clone(),
        scope: collection.scope.clone(),
        generation: collection.generation,
        observed_at_ms: collection.observed_at_ms,
        valid_until_ms: collection.valid_until_ms,
        complete: collection.complete,
        next_page: collection.next_page.clone(),
        unavailable_reason: collection.unavailable_reason.clone(),
    }
}

/// A display-only source whose refusal leaves the rest of the collection intact.
fn is_isolated(collection: &SourceCollection) -> bool {
    collection.kind == InventoryKind::WorkflowRevisions
        && collection.contract == LATTICE_WORKFLOW_REVISION_SLOTS
}

/// The source unavailable with `reason`, carrying no facts.
fn unavailable(collection: &SourceCollection, reason: String) -> InventoryFragment {
    let mut source = source_observation(collection);
    source.unavailable_reason = Some(reason);
    InventoryFragment::new(collection.kind, source)
}

/// The read bound of one document of `collection`.
fn document_bound(collection: &SourceCollection) -> u64 {
    if is_isolated(collection) {
        u64::try_from(
            kiln_adapter_lattice::workflow_revision_slots::MAX_WORKFLOW_REVISION_SLOTS_BYTES,
        )
        .unwrap_or(MAX_PRODUCER_DOCUMENT_BYTES)
    } else {
        MAX_PRODUCER_DOCUMENT_BYTES
    }
}

fn fragment(
    collection: &SourceCollection,
    joins: &InventoryJoins,
    documents: &[&[u8]],
) -> Result<InventoryFragment, String> {
    let source = source_observation(collection);
    let contract = collection.contract.as_str();
    let mapped = match collection.kind {
        InventoryKind::Agents | InventoryKind::Workers if contract == LATTICE_WORKER_INVENTORY => {
            kiln_adapter_lattice::worker_inventory::map_worker_inventory(
                collection.kind,
                &source,
                joins,
                documents,
            )
        }
        InventoryKind::Compute if contract == COMPUTE_ALLOCATION_OBSERVATION => {
            kiln_adapter_lattice::compute_inventory::map_compute_observations(
                &source,
                collection.provider_logical_now,
                documents,
            )
        }
        InventoryKind::Serving if contract == ANIMUS_SERVING_SNAPSHOT => {
            kiln_adapter_animus::map_serving_snapshots(
                &source,
                joins,
                collection.provider_logical_now,
                documents,
            )
        }
        InventoryKind::WorkflowRevisions if contract == LATTICE_WORKFLOW_REVISION_SLOTS => {
            kiln_adapter_lattice::workflow_revision_slots::map_workflow_revision_slots(
                &source, joins, documents,
            )
        }
        _ => return Err("inventory_collection_contract_unsupported".into()),
    };
    mapped.map_err(str::to_owned)
}

impl CollectionSet {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = read_bounded(
            path,
            MAX_COLLECTION_MANIFEST_BYTES,
            "inventory_collection_unavailable",
        )?;
        let set: Self =
            serde_json::from_slice(&bytes).map_err(|_| "inventory_collection_invalid")?;
        if set.schema != COLLECTION_SET_SCHEMA {
            return Err("inventory_collection_unsupported".into());
        }
        set.joins.admit()?;
        if set.collections.len() > MAX_INVENTORY_SOURCES {
            return Err("inventory_source_bound".into());
        }
        let documents: usize = set.collections.iter().map(|c| c.documents.len()).sum();
        if documents > MAX_INVENTORY_ITEMS {
            return Err("inventory_collection_document_bound".into());
        }
        Ok(set)
    }

    /// Read each named document and compose one snapshot. An unavailable
    /// source carries its reason and no facts.
    pub fn collect(&self, base: &Path) -> Result<InventorySnapshot, String> {
        self.collect_with_profile_opt(base, None)
    }

    fn collect_with_profile_opt(
        &self,
        base: &Path,
        profile: Option<&InventoryProfile>,
    ) -> Result<InventorySnapshot, String> {
        let mut fragments = Vec::with_capacity(self.collections.len());
        for collection in &self.collections {
            if collection.unavailable_reason.is_some() && !collection.documents.is_empty() {
                return Err("inventory_collection_unavailable_with_documents".into());
            }
            let paths = collection
                .documents
                .iter()
                .map(|relative| document_path(base, relative))
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(profile) = profile
                && let Some(policy) = profile
                    .sources
                    .iter()
                    .find(|p| p.source == collection.source)
                && (is_isolated(collection) || policy.kind == InventoryKind::WorkflowRevisions)
                && policy.kind != collection.kind
            {
                fragments.push(unavailable(
                    collection,
                    "inventory_source_kind_mismatch".to_string(),
                ));
                continue;
            }
            let mapped = if is_isolated(collection) {
                self.collect_workflow_slots_within(
                    collection,
                    &paths,
                    kiln_adapter_lattice::workflow_revision_slots::MAX_WORKFLOW_SLOTS_AGGREGATE_BYTES,
                )
            } else {
                paths
                    .iter()
                    .map(|path| {
                        read_bounded(
                            path,
                            document_bound(collection),
                            "inventory_collection_document_unavailable",
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(|bytes| {
                        let documents: Vec<&[u8]> = bytes.iter().map(Vec::as_slice).collect();
                        fragment(collection, &self.joins, &documents)
                    })
            };
            fragments.push(match mapped {
                Err(reason) if is_isolated(collection) => unavailable(collection, reason),
                other => other?,
            });
        }
        compose_snapshot(&self.principal, self.synthetic, fragments).map_err(str::to_owned)
    }

    fn collect_workflow_slots_within(
        &self,
        collection: &SourceCollection,
        paths: &[PathBuf],
        max_aggregate_bytes: u64,
    ) -> Result<InventoryFragment, String> {
        let mut accumulator =
            kiln_adapter_lattice::workflow_revision_slots::WorkflowRevisionSlotsAccumulator::new(
                source_observation(collection),
                self.joins.clone(),
            )
            .map_err(str::to_owned)?;
        let bound = document_bound(collection);
        for path in paths {
            let bytes = read_bounded(path, bound, "inventory_collection_document_unavailable")?;
            accumulator
                .push_document_within(&bytes, max_aggregate_bytes)
                .map_err(str::to_owned)?;
        }
        Ok(accumulator.finish())
    }
}

fn manifest_directory(manifest: &Path) -> &Path {
    manifest.parent().unwrap_or_else(|| Path::new("."))
}

/// Collect the snapshot a manifest names.
pub fn collect_snapshot(manifest: &Path) -> Result<InventorySnapshot, String> {
    CollectionSet::load(manifest)?.collect(manifest_directory(manifest))
}

/// Collect and admit against an inventory v2 profile, yielding the input the
/// existing CLI, API and offline HTML consumers read.
pub fn collect_input(manifest: &Path, profile: &Path) -> Result<InventoryInput, String> {
    let profile: InventoryProfile = serde_json::from_slice(&read_bounded(
        profile,
        MAX_PROFILE_BYTES,
        "inventory_profile_unavailable",
    )?)
    .map_err(|_| "inventory_profile_invalid")?;
    let set = CollectionSet::load(manifest)?;
    let mut snapshot =
        set.collect_with_profile_opt(manifest_directory(manifest), Some(&profile))?;
    // A workflow revision fact admission would refuse, or one past the
    // profile's item bound, makes only its own source unavailable.
    isolate_workflow_revisions(&profile, &mut snapshot);
    admit_snapshot(&profile, &snapshot)?;
    Ok(InventoryInput { profile, snapshot })
}

/// `inventory-collect MANIFEST PROFILE`: prints the admitted `{profile, snapshot}` input.
pub fn run_collect_cli_args(args: &[String]) -> Result<String, String> {
    let [manifest, profile] = args else {
        return Err("usage: inventory-collect MANIFEST PROFILE".into());
    };
    let input = collect_input(Path::new(manifest), Path::new(profile))?;
    serde_json::to_string_pretty(&input).map_err(|_| "inventory_input_encoding_invalid".into())
}

/// Observation port over one explicit manifest; it never discovers providers.
#[derive(Clone, Debug)]
pub struct ExplicitCollectionPort {
    manifest: PathBuf,
}

impl ExplicitCollectionPort {
    #[must_use]
    pub fn new(manifest: PathBuf) -> Self {
        Self { manifest }
    }
}

impl InventoryObservationPort for ExplicitCollectionPort {
    fn observe_inventory(
        &mut self,
        principal: &str,
    ) -> Result<InventorySnapshot, InventoryReadFailure> {
        let set = CollectionSet::load(&self.manifest).map_err(|error| {
            if error == "inventory_collection_unavailable" {
                InventoryReadFailure::Unavailable
            } else {
                InventoryReadFailure::MalformedObservation
            }
        })?;
        if set.principal != principal {
            return Err(InventoryReadFailure::PrincipalDenied);
        }
        set.collect(manifest_directory(&self.manifest))
            .map_err(|_| InventoryReadFailure::MalformedObservation)
    }
}

#[cfg(test)]
#[path = "inventory_collection_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "inventory_collection_workflow_tests.rs"]
mod workflow_tests;

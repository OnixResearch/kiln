//! Pure composition of per-source producer fragments into one inventory v2
//! snapshot. Joins use exact producer-scoped references only; nothing here
//! reads a clock, a file or a provider.
// r[impl kiln.inventory.collection]
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::{
    AgentDefinition, ComputeDevice, FactRef, INVENTORY_PROFILE, InventoryKind, InventorySnapshot,
    MAX_INVENTORY_SOURCES, ModelService, ResourcePool, SourceObservation, WorkerInstance, token,
};

/// Source identities that scope references between inventories. A producer
/// adapter resolves a cross-inventory reference only into these sources and
/// only by an exact producer identity, never by host name.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryJoins {
    pub agents: String,
    pub compute: String,
    pub serving: String,
}

impl InventoryJoins {
    pub fn admit(&self) -> Result<(), &'static str> {
        token(&self.agents)?;
        token(&self.compute)?;
        token(&self.serving)
    }
}

/// Facts one adapter mapped from one source's producer documents.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryFragment {
    pub kind: InventoryKind,
    pub source: SourceObservation,
    pub agents: Vec<AgentDefinition>,
    pub workers: Vec<WorkerInstance>,
    pub compute: Vec<ComputeDevice>,
    pub pools: Vec<ResourcePool>,
    pub models: Vec<ModelService>,
}

impl InventoryFragment {
    /// An empty fragment for one source; adapters push the facts they mapped.
    #[must_use]
    pub fn new(kind: InventoryKind, source: SourceObservation) -> Self {
        Self {
            kind,
            source,
            agents: Vec::new(),
            workers: Vec::new(),
            compute: Vec::new(),
            pools: Vec::new(),
            models: Vec::new(),
        }
    }

    fn references(&self) -> impl Iterator<Item = &FactRef> {
        self.agents
            .iter()
            .map(|v| &v.meta.reference)
            .chain(self.workers.iter().map(|v| &v.meta.reference))
            .chain(self.compute.iter().map(|v| &v.meta.reference))
            .chain(self.pools.iter().map(|v| &v.meta.reference))
            .chain(self.models.iter().map(|v| &v.meta.reference))
    }

    fn holds_only_its_kind(&self) -> bool {
        let agents = self.agents.is_empty();
        let workers = self.workers.is_empty();
        let compute = self.compute.is_empty() && self.pools.is_empty();
        let models = self.models.is_empty();
        match self.kind {
            InventoryKind::Agents => workers && compute && models,
            InventoryKind::Workers => agents && compute && models,
            InventoryKind::Compute => agents && workers && models,
            InventoryKind::Serving => agents && workers && compute,
            InventoryKind::Policy => false,
        }
    }
}

/// Compose producer fragments into one snapshot. A serving fact's accounting
/// pools become the union of the pools of its exact compute allocations, so a
/// shared deployment or shared pool is never a second capacity.
pub fn compose_snapshot(
    principal: &str,
    synthetic: bool,
    fragments: Vec<InventoryFragment>,
) -> Result<InventorySnapshot, &'static str> {
    token(principal)?;
    if fragments.len() > MAX_INVENTORY_SOURCES {
        return Err("inventory_source_bound");
    }
    let mut sources = BTreeSet::new();
    let mut references = BTreeSet::new();
    for fragment in &fragments {
        if !fragment.holds_only_its_kind() {
            return Err("inventory_fragment_kind_mismatch");
        }
        if !sources.insert(fragment.source.source.as_str()) {
            return Err("inventory_source_ambiguous");
        }
        for reference in fragment.references() {
            if reference.source != fragment.source.source {
                return Err("inventory_fragment_source_mismatch");
            }
            if !references.insert(reference.clone()) {
                return Err("inventory_fact_ambiguous");
            }
        }
    }
    let mut snapshot = InventorySnapshot {
        schema: INVENTORY_PROFILE.into(),
        principal: principal.into(),
        synthetic,
        sources: Vec::new(),
        agents: Vec::new(),
        workers: Vec::new(),
        compute: Vec::new(),
        pools: Vec::new(),
        models: Vec::new(),
        admissions: Vec::new(),
        policies: Vec::new(),
        attempts: Vec::new(),
    };
    for fragment in fragments {
        snapshot.sources.push(fragment.source);
        snapshot.agents.extend(fragment.agents);
        snapshot.workers.extend(fragment.workers);
        snapshot.compute.extend(fragment.compute);
        snapshot.pools.extend(fragment.pools);
        snapshot.models.extend(fragment.models);
    }
    for model in &mut snapshot.models {
        let mut pools: BTreeSet<FactRef> = model.accounting_pools.iter().cloned().collect();
        for device in &model.compute_devices {
            if let Some(compute) = snapshot
                .compute
                .iter()
                .find(|c| &c.meta.reference == device)
            {
                pools.extend(compute.accounting_pools.iter().cloned());
            }
        }
        model.accounting_pools = pools.into_iter().collect();
    }
    snapshot.sources.sort_by(|a, b| a.source.cmp(&b.source));
    snapshot
        .agents
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .workers
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .compute
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .pools
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    snapshot
        .models
        .sort_by(|a, b| a.meta.reference.cmp(&b.meta.reference));
    debug_assert!(snapshot.sources.len() <= MAX_INVENTORY_SOURCES);
    Ok(snapshot)
}

#[cfg(test)]
#[path = "inventory_compose_tests.rs"]
mod tests;

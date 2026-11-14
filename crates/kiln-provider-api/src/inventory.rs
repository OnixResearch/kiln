//! Narrow read-only observation ports. Providers retain reservation and scheduling authority.
use kiln_core::inventory::{InventorySnapshot, WorkNeedsAttachment};
use kiln_core::types::ContentId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryReadFailure {
    Unavailable,
    UnsupportedProfile,
    PrincipalDenied,
    MalformedObservation,
    StaleWork,
}

/// Authorized host shells collect bounded snapshots. Offline renderers never call this port.
pub trait InventoryObservationPort {
    fn observe_inventory(
        &mut self,
        principal: &str,
    ) -> Result<InventorySnapshot, InventoryReadFailure>;
}

/// Implementations must validate attachment against current canonical work, revision and policy.
/// Missing requirements remain unknown; titles/configured slots are not requirements evidence.
pub trait InventoryWorkPort {
    fn current_requirements(
        &self,
        work: &ContentId,
    ) -> Result<Option<WorkNeedsAttachment>, InventoryReadFailure>;
}

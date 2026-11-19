//! Pure, deterministic core types for cairn.
//!
//! Filesystem access, environment reads, process control, and presentation live
//! in shell crates. Octet checks modules under `verified/` with its
//! `verified_purity` lint.
//! r[impl cairn.supersede.boundary.pure_core]
//! r[impl cairn.supersede.migrate_openspec.boundary]
//! r[impl cairn.supersede.tracey_coverage.boundary]
//! r[impl cairn.supersede.traceability_ownership.authoritative_checker]
//! r[impl cairn.supersede.traceability_ownership.configured_profiles]
//! r[impl cairn.supersede.gate_policy.references]
//! r[impl cairn.supersede.gate_policy.task_links]
//! r[impl cairn.supersede.gate_policy.exemptions]
//! r[impl cairn.supersede.doctor]
//! r[impl cairn.supersede.doctor.boundary]

pub mod verified;
pub use verified::assurance_targets;
pub use verified::assurance_targets::*;
pub use verified::concurrent_change_footprints;
pub use verified::concurrent_change_footprints::*;
pub use verified::convergence;
pub use verified::convergence::*;
pub use verified::refinement_typed_dependencies;
pub use verified::refinement_typed_dependencies::*;
pub use verified::requirement_consistency;
pub use verified::requirement_consistency::*;
pub use verified::revision_classification;
pub use verified::revision_classification::*;
pub use verified::scoped_claim_derivations;
pub use verified::scoped_claim_derivations::*;
pub use verified::source_review;
pub use verified::support_maintenance;
pub use verified::support_maintenance::*;
pub use verified::typed_requirement_contracts;
pub use verified::typed_requirement_contracts::*;
pub use verified::workflow_obligations;
pub use verified::workflow_obligations::*;

mod api;
pub use api::*;

//! Versioned workflow request and observation contract for Lattice consumers.
//!
//! This crate owns bounded wire values, deterministic BLAKE3 identities, and
//! pure admission decisions. It performs no workflow execution, persistence,
//! transport, credential access, clock access, filesystem access, or provider
//! calls. Lattice remains the workflow runtime owner. Consumers retain their
//! domain state, authority, outcome policy, and evidence claims.

#![no_std]
#![forbid(unsafe_code)]
#![allow(
    unknown_lints,
    reason = "the transport deny-all runner compiles this dependency without loading its Dylint driver"
)]

extern crate alloc;

// r[impl lattice.workflow_interchange.contract]
mod admission;
mod identity;
mod model;
mod observation;

pub use admission::validate_accepted;
pub use admission::validate_control_request;
pub use admission::validate_log_read_request;
pub use admission::validate_profile;
pub use admission::validate_request;
pub use identity::graph_identity;
pub use identity::observation_identity;
pub use identity::profile_identity;
pub use identity::request_identity;
pub use model::*;
pub use observation::validate_observation;

#[cfg(test)]
mod tests;

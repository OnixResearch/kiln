//! Versioned gate request and result contract for Cairn consumers.
//!
//! This crate owns bounded wire values, deterministic BLAKE3 identities, and
//! pure admission decisions. It performs no lifecycle evaluation, filesystem
//! access, process execution, persistence, transport, clock access, credential
//! access, or provider calls. Cairn retains native gate meaning. Consumers
//! retain candidate truth, process policy, reconciliation, and effect authority.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

// r[impl cairn.gate_interchange.contract]
// r[verify cairn.gate_interchange.contract.scenario.pinned]
// r[impl cairn.gate_interchange.boundary]
// r[verify cairn.gate_interchange.boundary.scenario.scope]
mod admission;
mod identity;
mod model;

pub use admission::{build_result, validate_diagnostic, validate_request, validate_result};
pub use identity::{request_identity, result_identity};
pub use model::*;

#[cfg(test)]
mod tests;

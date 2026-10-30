//! Semantic merge checker producer port.
//!
//! Language-owned checkers (for example the Octet Rust producer or the Noble checker) implement
//! this port in their own shells. This repository admits only a fixture producer; no production
//! producer, receipt profile, or required production gate is claimed.

use core::sync::atomic::AtomicBool;

use kiln_core::semantic_merge::{
    CheckerIdentity, CheckerObservation, CheckerRequest, ProducerFailure,
};
use kiln_core::types::Diagnostic;

use crate::{PortFailure, PortResult};

// r[impl kiln.semantic_merge.dispatch]
pub trait SemanticCheckerPort {
    /// Exact producer, checker and toolchain identity this port runs.
    fn producer(&self) -> &CheckerIdentity;

    /// Runs one bounded, cancellable check for exactly one attempt.
    fn check(
        &mut self,
        request: &CheckerRequest,
        cancellation: &AtomicBool,
    ) -> PortResult<CheckerObservation>;
}

/// Dispatches one attempt after refusing a producer substitution, before any external effect.
// r[impl kiln.semantic_merge.dispatch]
pub fn dispatch_semantic_check<P: SemanticCheckerPort>(
    port: &mut P,
    request: &CheckerRequest,
    cancellation: &AtomicBool,
) -> PortResult<CheckerObservation> {
    if port.producer() != &request.request.request().checker {
        return Err(PortFailure::Domain(Diagnostic::new(
            "semantic_checker_substitution",
            "semantic_merge.binding.checker",
            "checker port runs a producer other than the admitted checker",
        )));
    }
    port.check(request, cancellation)
}

/// Classifies a producer-port failure into the outcome Kiln records for the attempt.
#[must_use]
pub const fn producer_failure(failure: &PortFailure) -> ProducerFailure {
    match failure {
        PortFailure::TimedOut(_) => ProducerFailure::TimedOut,
        PortFailure::Cancelled(_) => ProducerFailure::Cancelled,
        PortFailure::Domain(_)
        | PortFailure::Infrastructure(_)
        | PortFailure::Retryable(_)
        | PortFailure::Unknown(_) => ProducerFailure::ToolError,
    }
}

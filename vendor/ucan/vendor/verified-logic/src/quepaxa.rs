//! Finite QuePaxa consensus-admission primitives.
//!
//! This module models deterministic crash-stop QuePaxa safety/admission facts:
//! proposal ordering, interval summary register (ISR) transitions, abstract
//! propagation summaries, abstract decision guards, concrete phase/catch-up
//! guards, fast-path admission, and hedging metadata. It intentionally does not
//! prove real network liveness, probabilistic termination, random-number quality,
//! content-oblivious adversary enforcement, TLS secrecy, storage durability,
//! batching, benchmark results, adaptive multi-armed-bandit optimality,
//! production SMR behavior, or Byzantine fault tolerance.

pub mod isr;
pub mod model;
pub mod order;
pub mod round;

pub use isr::*;
pub use model::*;
pub use order::*;
pub use round::*;

#[cfg(test)]
mod tests;

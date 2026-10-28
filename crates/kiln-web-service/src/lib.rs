//! Authenticated single-tenant CI application shell; provider mechanisms stay behind ports.

pub mod access;
pub mod automation;
pub mod cobs;
pub mod engine;
pub mod profile;
pub mod reports;
pub mod sources;
pub mod worker;

mod auth;
pub mod server;

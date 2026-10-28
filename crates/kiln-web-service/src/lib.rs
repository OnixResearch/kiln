//! Authenticated single-tenant CI application shell; provider mechanisms stay behind ports.

pub mod access;
pub mod automation;
pub mod cache;
pub mod cobs;
pub mod code_review;
pub mod developer;
pub mod engine;
pub mod execution;
pub mod profile;
pub mod reports;
pub mod reviews;
pub mod snapshot;
pub mod sources;
pub mod telemetry;
pub mod work;
pub mod worker;

mod auth;
pub mod server;

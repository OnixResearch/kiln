//! Authenticated single-tenant CI application shell; provider mechanisms stay behind ports.

pub mod access;
pub mod automation;
pub mod cache;
pub mod cairn_workspace;
pub mod campaign_controls;
pub mod cobs;
pub mod code_review;
pub mod developer;
pub mod engine;
pub mod execution;
pub mod feeds;
pub mod fleet;
pub mod implementation_campaign;
pub mod inventory;
pub mod inventory_collection;
pub mod monorepo;
pub mod preview;
pub mod profile;
pub mod reports;
pub mod reviews;
pub mod runbooks;
pub mod schedule;
pub mod snapshot;
pub mod sources;
pub mod swarm_view;
pub mod telemetry;
pub mod work;
pub mod worker;

mod auth;
pub mod server;

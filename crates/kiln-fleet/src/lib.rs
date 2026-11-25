//! Kiln fleet service: the claim authority and dispatcher for interactive
//! coding agents across machines.
//!
//! One process serves the bounded fleet API (claims, work, attempts,
//! handoffs, decisions) to authenticated machines and runs one dispatch
//! thread that follows herdr sessions from claimed work to a captured
//! candidate handoff. Every decision is made by the pure
//! `kiln_core::fleet` rules over the durable store; this crate performs
//! effects and reports observations. Agent and swarm text is data.

pub mod api;
pub mod capture;
pub mod cli;
pub mod client;
pub mod config;
mod core_observer;
pub mod dispatch;
pub mod exec;
pub mod http;
pub mod identity;
pub mod log;
pub mod mcp;
pub mod server;
pub mod store;
pub mod swarm;

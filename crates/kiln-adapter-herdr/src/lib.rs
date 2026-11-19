//! herdr execution adapter for Kiln's interactive agent fleet.
//!
//! Kiln owns which attempt runs next; this adapter owns where it runs. It
//! reads an operator inventory of machines and agent slots, chooses a
//! placement with the pure [`place`], and drives the pinned `herdr` CLI as
//! bounded child processes with a cleared environment: worktree creation,
//! agent start, prompt, status, transcript reads, interrupt and workspace
//! close. Every failure is classified as unavailable, rejected or unknown;
//! an unknown mutation must be reconciled by listing before any retry.
//! Agent output is untrusted data.

pub mod client;
pub mod inventory;
pub mod placement;

pub use client::{
    AgentObservation, AgentStatus, CreatedWorktree, FailureClass, HerdrAgent, HerdrClient,
    HerdrFailure, StartedAgent, WorktreeEntry,
};
pub use inventory::{
    AgentKind, AgentSlots, HerdrEnvironment, HerdrInventory, HerdrMachine, INVENTORY_SCHEMA,
};
pub use placement::{
    AGENT_NAME_PREFIX, KindCounts, MachineLoad, Occupancy, Placement, agent_name, attempt_short,
    count_occupancy, kiln_agent_name, place, valid_agent_name,
};

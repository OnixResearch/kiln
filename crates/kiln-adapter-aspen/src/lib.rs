//! Kiln-owned adapter contracts for the Aspen runtime backend.
//!
//! Pure profile, identity, callback, state, durability, content, and effect
//! mappings stay separate from transport, storage, and callback process shells.
//! Kiln retains CI meaning. Aspen supplies only the exact admitted runtime
//! mechanisms represented by these ports.

#![forbid(unsafe_code)]

pub mod composition;
pub mod content;
pub mod durability;
pub mod effects;
pub mod identity;
pub mod profile;
pub mod protocol;
pub mod provider;
pub mod service;
pub mod transport;

#[cfg(test)]
mod tests;

pub use composition::*;
pub use content::*;
pub use durability::*;
pub use effects::*;
pub use identity::*;
pub use profile::*;
pub use protocol::*;
pub use provider::*;
pub use service::*;
pub use transport::*;

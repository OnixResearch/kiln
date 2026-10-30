//! Minimal shim for the upstream `radicle::identity` pieces canonical rules
//! resolve against: the DID wrapper and valid delegates/thresholds.

mod did;
pub mod doc;

pub use did::{Did, DidError};

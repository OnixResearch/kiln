//! Bounded, advisory cache index over `iroh-docs`.
//!
//! The index is a mutable, eventually consistent document. It is not Kiln
//! canonical state, a cache reservation, or an authority grant. Consumers must
//! still fetch the referenced blob, verify its BLAKE3 identity, and apply the
//! existing cache policy.

pub mod index;
pub mod profile;

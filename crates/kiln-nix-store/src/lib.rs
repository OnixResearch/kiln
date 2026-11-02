//! Bounded, permissive Nix store facts for Kiln.
//!
//! This crate adapts the data semantics of Snix (`nix-compat`, castore,
//! PathInfo, reference scanning) without linking the GPL-3.0 Snix crates. It
//! uses the Apache-2.0 `nix-derivation`, `nix-archive` and `nix-narinfo`
//! crates and performs no ambient I/O.

#![forbid(unsafe_code)]

pub mod cache;
pub mod derivation;
pub mod nar;

use kiln_core::types::{ContentId, Diagnostic};

/// Frame a length-prefixed domain-separated identity input.
pub(crate) fn identity(domain: &str, fields: &[&[u8]]) -> ContentId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(domain.len() as u64).to_le_bytes());
    hasher.update(domain.as_bytes());
    for field in fields {
        hasher.update(&(field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
        .unwrap_or_else(|_| unreachable!("BLAKE3 hex is a valid canonical identity"))
}

pub(crate) fn diagnostic(
    code: &'static str,
    field: &'static str,
    message: &'static str,
) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod boundary_tests {
    // r[verify kiln.nix_store.facts] r[verify kiln.nix_store.facts.scenario.license]
    #[test]
    fn manifest_and_lock_have_no_snix_crate() {
        let manifest = include_str!("../Cargo.toml");
        let lock = include_str!("../../../Cargo.lock");
        assert!(!manifest.contains("snix"));
        assert!(!lock.contains("name = \"snix-"));
    }
}

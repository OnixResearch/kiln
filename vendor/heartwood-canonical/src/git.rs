//! Minimal shim for the upstream `radicle::git` paths used by the vendored
//! canonical module. The algorithm files under `git/canonical` are upstream.

pub mod canonical;
pub mod raw;

pub mod fmt {
    //! Re-exports of `radicle_git_ref_format` plus this crate's validating
    //! `refname!`, so canonical code keeps its upstream `crate::git::fmt` paths.
    pub use crate::refname;
    pub use radicle_git_ref_format::*;
}

pub use radicle_oid::Oid;

pub mod refs {
    //! Minimal copy of the upstream branch-reference helper used by
    //! `canonical::protect`'s companion implementations.
    pub use radicle_git_ref_format::{Qualified, RefStr, lit};

    /// Create a qualified branch reference: `refs/heads/<branch>`.
    pub fn branch<'a>(branch: &RefStr) -> Qualified<'a> {
        Qualified::from(lit::refs_heads(branch))
    }
}

//! Minimal slice of upstream `radicle::git::raw`: the error re-export and the
//! `not found` extension trait canonical computation relies on.

pub use git2::{
    Blob, Commit, Error, ErrorCode, FileMode, Object, ObjectType, Oid, Reference, Repository,
    Signature, Time, Tree,
};

/// Upstream `radicle::git::raw::ErrorExt`, verbatim.
pub trait ErrorExt {
    /// Returns `true` if the error is [`git2::ErrorCode::NotFound`].
    fn is_not_found(&self) -> bool;
}

impl ErrorExt for git2::Error {
    fn is_not_found(&self) -> bool {
        self.code() == git2::ErrorCode::NotFound
    }
}

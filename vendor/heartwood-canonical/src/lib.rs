//! Vendored Radicle canonical-reference computation.
//!
//! This crate carries the unmodified `radicle::git::canonical` source from
//! heartwood revision `71a91042d96bb994c4cd4311d85f5cd1a6311b4e` (crate
//! `radicle` 0.25.1), plus minimal shims that reproduce the module paths the
//! canonical code uses. The upstream crate cannot be linked here directly: it
//! depends on the `sqlite` crate, whose native `sqlite3` link collides with the
//! workspace's rusqlite/libsqlite3-sys.
//!
//! Provenance, digests and the exact upstream paths are recorded in
//! `vendor/heartwood-canonical.source.json`. Only test modules that require
//! upstream storage fixtures were removed; the algorithm files are otherwise
//! byte-identical.
#![forbid(unsafe_code)]
#![allow(clippy::doc_overindented_list_items)]

pub use radicle_crypto as crypto;

pub mod git;
pub mod identity;

pub mod prelude {
    pub use crate::identity::Did;
}

/// Always-validating `refname!` (upstream's macro skips validation outside
/// debug assertions; the retained tests rely on the validated form).
#[macro_export]
macro_rules! refname {
    ($arg:literal) => {
        $crate::git::fmt::RefString::try_from($arg).expect(concat!(
            "literal `",
            $arg,
            "` must be a valid reference name"
        ))
    };
}

/// Copied from `radicle::assert_matches` at the same revision; used by the
/// retained upstream unit tests.
#[macro_export]
macro_rules! assert_matches {
    ( $e:expr_2021 , $($pat:pat_param)|+ ) => {
        match $e {
            $($pat)|+ => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+))
        }
    };
    ( $e:expr_2021 , $($pat:pat_param)|+ if $cond:expr_2021 ) => {
        match $e {
            $($pat)|+ if $cond => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+ if $cond))
        }
    };
    ( $e:expr_2021 , $($pat:pat_param)|+ => $arm:expr_2021 ) => {
        match $e {
            $($pat)|+ => $arm,
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+))
        }
    };
    ( $e:expr_2021 , $($pat:pat_param)|+ if $cond:expr_2021 => $arm:expr_2021 ) => {
        match $e {
            $($pat)|+ if $cond => $arm,
            ref e => panic!("assertion failed: `{:?}` does not match `{}`",
                e, stringify!($($pat)|+ if $cond))
        }
    };
    ( $e:expr_2021 , $($pat:pat_param)|+ , $($arg:tt)* ) => {
        match $e {
            $($pat)|+ => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`: {}",
                e, stringify!($($pat)|+), format_args!($($arg)*))
        }
    };
    ( $e:expr_2021 , $($pat:pat_param)|+ if $cond:expr_2021 , $($arg:tt)* ) => {
        match $e {
            $($pat)|+ if $cond => (),
            ref e => panic!("assertion failed: `{:?}` does not match `{}`: {}",
                e, stringify!($($pat)|+ if $cond), format_args!($($arg)*))
        }
    };
}

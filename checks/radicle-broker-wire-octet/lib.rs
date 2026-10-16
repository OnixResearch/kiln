#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the focused wire shim preserves production imports while checking parser behavior"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Radicle Defelo compatibility names retain their protocol prefix"
)]

pub const MAX_LOG_URL_BYTES: usize = 2048;
pub const DEFAULT_MAX_POLLS: usize = 64;

pub mod defelo;

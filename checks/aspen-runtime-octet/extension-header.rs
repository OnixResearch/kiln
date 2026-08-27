#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the focused shim preserves production imports while checking all callback behavior"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln and Aspen callback names retain their owner prefixes"
)]
#![allow(
    tigerstyle::usize_in_public_api,
    reason = "callback byte bounds match the existing host execution contract"
)]

#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the focused shim preserves production imports while checking all behavioral lints"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln and Aspen compatibility names retain their owner prefixes"
)]
#![allow(
    tigerstyle::usize_in_public_api,
    reason = "byte and collection bounds match existing Kiln port contracts on the host"
)]

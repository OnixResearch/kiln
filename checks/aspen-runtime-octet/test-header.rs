#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the focused test shim preserves readable fixture imports while checking all negative paths"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln and Aspen fixture names retain their owner prefixes"
)]
#![allow(
    tigerstyle::usize_in_public_api,
    reason = "fixture byte bounds match the existing host contracts"
)]
#![allow(
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert constructor and fixture invariants"
)]

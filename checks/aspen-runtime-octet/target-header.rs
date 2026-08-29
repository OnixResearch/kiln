#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the focused shell shim preserves production imports while checking effect behavior"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln and Aspen shell names retain their owner prefixes"
)]
#![allow(
    tigerstyle::usize_in_public_api,
    reason = "callback bounds match the existing host execution contract"
)]
#![allow(
    tigerstyle::ambient_env,
    reason = "the extension shell captures its explicit runtime-profile arguments once"
)]

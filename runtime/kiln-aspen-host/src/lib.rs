#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the composition shell keeps concrete Aspen, Kiln, and Lattice owners visible"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen host names preserve their protocol ownership"
)]

pub mod cohort;
pub mod durability;
pub mod effect;
pub mod error;
pub mod runtime;
pub mod server;

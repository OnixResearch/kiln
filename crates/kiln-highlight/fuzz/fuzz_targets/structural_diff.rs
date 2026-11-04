#![no_main]

use kiln_highlight::{Language, StructuralBounds, structural_diff};
use libfuzzer_sys::fuzz_target;

const SMALL: StructuralBounds = StructuralBounds {
    max_bytes: 4_096,
    max_nodes: 512,
    max_steps: 32_768,
};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let language = Language::ALL[usize::from(selector) % Language::ALL.len()];
    let bounds = if selector & 0x80 != 0 {
        SMALL
    } else {
        StructuralBounds::DEFAULT
    };
    let split = rest.len() / 2;
    let old = &rest[..split];
    let new = &rest[split..];
    let _ = structural_diff(Some(language), Some(old), Some(new), &bounds, &|| false);
});

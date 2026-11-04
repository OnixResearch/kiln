#![no_main]

use kiln_highlight::{Bounds, highlight_diff};
use libfuzzer_sys::fuzz_target;

const SMALL: Bounds = Bounds {
    max_bytes: 4_096,
    max_lines: 64,
    max_tokens_per_line: 32,
    max_tokens: 512,
    max_diff_files: 4,
};

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let bounds = if selector & 0x80 != 0 {
        SMALL
    } else {
        Bounds::DEFAULT
    };
    let diff = String::from_utf8_lossy(rest);
    let highlighted = highlight_diff(&diff, &bounds);
    assert!(highlighted.lines().len() <= bounds.max_lines);
});

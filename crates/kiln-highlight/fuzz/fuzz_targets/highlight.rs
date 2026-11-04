#![no_main]

use kiln_highlight::{Bounds, Language, highlight};
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
    let language = Language::ALL[usize::from(selector) % Language::ALL.len()];
    let bounds = if selector & 0x80 != 0 {
        SMALL
    } else {
        Bounds::DEFAULT
    };
    let text = rest.to_vec();
    let highlighted = highlight(Some(language), &text, &bounds);
    assert!(highlighted.lines().len() <= bounds.max_lines);
    for line in highlighted.lines() {
        assert!(line.len() <= bounds.max_tokens_per_line);
    }
    let _ = kiln_highlight::language_for_path(&String::from_utf8_lossy(&text));
});

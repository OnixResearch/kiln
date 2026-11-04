#![no_main]

use kiln_web_service::feeds::{AtomItem, MAX_TITLE_BYTES, parse, render_atom};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let item = AtomItem {
        id: format!("urn:kiln:fuzz:{text}"),
        title: text.to_string(),
        link: "https://example.invalid/fuzz".into(),
        updated_ms: 1_700_000_000_000,
        summary: text.to_string(),
    };
    let xml = render_atom(
        "urn:kiln:fuzz:feed",
        &text,
        "https://example.invalid/feed",
        1_700_000_000_000,
        &[item],
    );
    let parsed = parse(xml.as_bytes(), 1).unwrap_or_else(|error| {
        panic!("render_atom produced unparseable XML: {error}");
    });
    assert_eq!(parsed.entries.len(), 1);
    assert!(parsed.entries[0].title.len() <= MAX_TITLE_BYTES + 4);
});

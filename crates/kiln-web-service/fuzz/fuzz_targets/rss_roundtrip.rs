#![no_main]

use kiln_web_service::feeds::{AtomItem, parse, render_rss};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let xml = render_rss(
        &text,
        "https://example.invalid/rss",
        &text,
        &[AtomItem {
            id: "urn:kiln:fuzz:item".into(),
            title: text.to_string(),
            link: "https://example.invalid/item".into(),
            updated_ms: 1_700_000_000_000,
            summary: text.to_string(),
        }],
    );
    let parsed = parse(xml.as_bytes(), 1).unwrap_or_else(|error| {
        panic!("render_rss produced unparseable XML: {error}");
    });
    assert_eq!(parsed.entries.len(), 1);
});

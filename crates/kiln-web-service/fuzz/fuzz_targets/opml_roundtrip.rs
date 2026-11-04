#![no_main]

use kiln_web_service::feeds::{parse_opml, render_opml};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let xml = render_opml(&text, &[(&text, "https://example.invalid/feed")]);
    let parsed = parse_opml(xml.as_bytes(), 1).unwrap_or_else(|error| {
        panic!("render_opml produced unparseable XML: {error}");
    });
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].0, "https://example.invalid/feed");
});

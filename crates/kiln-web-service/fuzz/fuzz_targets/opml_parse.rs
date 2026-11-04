#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    let maximum = usize::from(selector % 9);
    let body = body.to_vec();
    if let Ok(outlines) = kiln_web_service::feeds::parse_opml(&body, maximum) {
        assert!(outlines.len() <= maximum);
        for (url, _) in outlines {
            assert!(url.starts_with("http://") || url.starts_with("https://"));
        }
    }
});

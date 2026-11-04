#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, body)) = data.split_first() else {
        return;
    };
    let maximum = usize::from(selector % 5);
    let body = body.to_vec();
    if let Ok(feed) = kiln_web_service::feeds::parse(&body, maximum) {
        assert!(feed.entries.len() <= maximum);
        for entry in feed.entries {
            assert!(entry.title.len() <= kiln_web_service::feeds::MAX_TITLE_BYTES + 4);
            if let Some(summary) = entry.summary {
                assert!(summary.len() <= kiln_web_service::feeds::MAX_SUMMARY_BYTES + 4);
            }
            if let Some(link) = entry.link {
                assert!(link.starts_with("http://") || link.starts_with("https://"));
                assert!(link.len() <= kiln_web_service::feeds::MAX_URL_BYTES);
            }
        }
    }
});

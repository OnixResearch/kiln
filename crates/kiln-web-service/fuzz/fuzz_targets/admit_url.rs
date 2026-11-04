#![no_main]

use kiln_web_service::feeds::{admit_url, globally_routable};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let url = String::from_utf8_lossy(data);
    if let Ok(parsed) = admit_url(&url, false) {
        assert!(parsed.scheme() == "http" || parsed.scheme() == "https");
        if let Some(host) = parsed.host() {
            match host {
                url::Host::Ipv4(address) => assert!(globally_routable(address.into())),
                url::Host::Ipv6(address) => assert!(globally_routable(address.into())),
                url::Host::Domain(_) => {}
            }
        }
    }
});

//! `kiln-hosted` builds GitHub status clients for its connections, so it must
//! serve the DNS helper entry those clients re-run it with.

#[path = "../../kiln-adapter-github/tests/support/dns_entry.rs"]
mod dns_entry;

// r[verify kiln.github.dns_helper_entry.scenario.served]
#[test]
fn hosted_binary_serves_the_dns_helper_entry() {
    dns_entry::assert_served(env!("CARGO_BIN_EXE_kiln-hosted"));
    dns_entry::assert_malformed_entry_refused(env!("CARGO_BIN_EXE_kiln-hosted"));
}

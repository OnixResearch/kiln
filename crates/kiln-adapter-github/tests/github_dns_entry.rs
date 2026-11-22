//! Every binary that can construct a GitHub HTTPS client serves the reserved
//! DNS helper entry it re-runs itself with.

#[path = "support/dns_entry.rs"]
mod dns_entry;

// r[verify kiln.github.dns_helper_entry] r[verify kiln.github.dns_helper_entry.scenario.served]
#[test]
fn adapter_binary_serves_the_dns_helper_entry() {
    dns_entry::assert_served(env!("CARGO_BIN_EXE_kiln-adapter-github"));
    dns_entry::assert_malformed_entry_refused(env!("CARGO_BIN_EXE_kiln-adapter-github"));
}

// Regression: `kiln-implementer` rejected the entry as an unexpected argument,
// so every `poll` and `publish` failed at the first GitHub call.
// r[verify kiln.github.dns_helper_entry.scenario.served]
#[test]
fn implementer_binary_serves_the_dns_helper_entry() {
    dns_entry::assert_served(env!("CARGO_BIN_EXE_kiln-implementer"));
    dns_entry::assert_malformed_entry_refused(env!("CARGO_BIN_EXE_kiln-implementer"));
}

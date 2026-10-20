//! Test-only assertion rail for the dependency-advisory posture.
//!
//! This integration test file has no effect on runtime crate code. It asserts
//! that the committed cargo-deny policy and lockfile keep the two pinned
//! advisories and that the fixture rail rejects a missing pin. The
//! production checker in `tools/check-dependency-posture.rs` owns exact
//! parsing; this rail asserts the reviewable markers.

const DENY_TOML: &str = include_str!("../deny.toml");
const POSITIVE_FIXTURE: &str = include_str!("../cargo-deny/fixtures/positive-locked.deny.toml");
const NEGATIVE_FIXTURE: &str = include_str!("../cargo-deny/fixtures/negative-missing-pin.deny.toml");
const CARGO_LOCK: &str = include_str!("../Cargo.lock");

const ADVISORY_PASTE: &str = "RUSTSEC-2024-0436";
const ADVISORY_ANYHOW: &str = "RUSTSEC-2026-0190";

fn has_pin_with_reason(text: &str, id: &str) -> bool {
    let marker = format!("id = \"{id}\"");
    text.find(&marker).is_some_and(|index| text[index + marker.len()..].contains("reason = \""))
}

#[test]
fn locked_policy_pins_both_applicable_advisories_with_rationale() {
    assert!(has_pin_with_reason(DENY_TOML, ADVISORY_PASTE));
    assert!(has_pin_with_reason(DENY_TOML, ADVISORY_ANYHOW));
    assert!(DENY_TOML.contains("MPL-2.0"));
    assert!(DENY_TOML.contains("AGPL-3.0-or-later"));
}

#[test]
fn workspace_lockfile_still_binds_pinned_crates() {
    assert!(CARGO_LOCK.contains("name = \"paste\""));
    assert!(CARGO_LOCK.contains("name = \"anyhow\""));
}

#[test]
fn fixture_rail_rejects_a_missing_pin() {
    assert!(has_pin_with_reason(POSITIVE_FIXTURE, ADVISORY_PASTE));
    assert!(has_pin_with_reason(POSITIVE_FIXTURE, ADVISORY_ANYHOW));
    // The negative fixture drops the paste pin; the rail must reject it.
    assert!(has_pin_with_reason(NEGATIVE_FIXTURE, ADVISORY_ANYHOW));
    assert!(!has_pin_with_reason(NEGATIVE_FIXTURE, ADVISORY_PASTE));
}

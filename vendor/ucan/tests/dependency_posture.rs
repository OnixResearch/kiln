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
const UCAN_MANIFEST: &str = include_str!("../Cargo.toml");
const UCAN_CORE_MANIFEST: &str = include_str!("../crates/ucan-core/Cargo.toml");
const UCAN_NOSTR_MANIFEST: &str = include_str!("../crates/ucan-nostr/Cargo.toml");
const UCAN_PEER_AUTH_SOURCE: &str = include_str!("../crates/ucan-core/src/peer_auth.rs");
const UCAN_NOSTR_CONTEXT_SOURCE: &str = include_str!("../crates/ucan-nostr/src/context.rs");

const REVIEWED_BLAKE3: &str = "=1.8.5";
const DRIFTED_BLAKE3: &str = "=1.8.4";
const REVIEWED_BLAKE3_IDENTITY: &str = "blake3@1.8.5";
const DRIFTED_BLAKE3_IDENTITY: &str = "blake3@1.8.4";
const REVIEWED_ZEROIZE: &str = "=1.9.0";
const REVIEWED_ZEROIZE_IDENTITY: &str = "zeroize@1.9.0";
const DRIFTED_ZEROIZE_IDENTITY: &str = "zeroize@1.8.2";
const ADVISORY_PASTE: &str = "RUSTSEC-2024-0436";
const ADVISORY_ANYHOW: &str = "RUSTSEC-2026-0190";

fn has_pin_with_reason(text: &str, id: &str) -> bool {
    let marker = format!("id = \"{id}\"");
    text.find(&marker).is_some_and(|index| text[index + marker.len()..].contains("reason = \""))
}

fn blake3_cohort_align(core: &str, nostr: &str, peer_auth: &str, nostr_context: &str) -> bool {
    core.contains(REVIEWED_BLAKE3)
        && nostr.contains(REVIEWED_BLAKE3)
        && peer_auth.contains(REVIEWED_BLAKE3_IDENTITY)
        && nostr_context.contains(REVIEWED_BLAKE3_IDENTITY)
}

fn zeroize_cohort_align(root: &str, core: &str, peer_auth: &str) -> bool {
    root.contains(REVIEWED_ZEROIZE) && core.contains(REVIEWED_ZEROIZE) && peer_auth.contains(REVIEWED_ZEROIZE_IDENTITY)
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
fn exact_blake3_pins_identities_and_lock_use_one_reviewed_cohort() {
    assert!(blake3_cohort_align(
        UCAN_CORE_MANIFEST,
        UCAN_NOSTR_MANIFEST,
        UCAN_PEER_AUTH_SOURCE,
        UCAN_NOSTR_CONTEXT_SOURCE,
    ));
    assert_eq!(CARGO_LOCK.matches("name = \"blake3\"").count(), 1);
    assert!(CARGO_LOCK.contains("version = \"1.8.5\""));
}

#[test]
fn exact_blake3_manifest_and_identity_drift_are_rejected() {
    let drifted_manifest = UCAN_NOSTR_MANIFEST.replace(REVIEWED_BLAKE3, DRIFTED_BLAKE3);
    assert!(!blake3_cohort_align(
        UCAN_CORE_MANIFEST,
        &drifted_manifest,
        UCAN_PEER_AUTH_SOURCE,
        UCAN_NOSTR_CONTEXT_SOURCE,
    ));

    let drifted_identity = UCAN_PEER_AUTH_SOURCE.replace(REVIEWED_BLAKE3_IDENTITY, DRIFTED_BLAKE3_IDENTITY);
    assert!(!blake3_cohort_align(
        UCAN_CORE_MANIFEST,
        UCAN_NOSTR_MANIFEST,
        &drifted_identity,
        UCAN_NOSTR_CONTEXT_SOURCE,
    ));
}

#[test]
fn exact_zeroize_pins_identity_and_lock_use_one_reviewed_cohort() {
    assert!(zeroize_cohort_align(UCAN_MANIFEST, UCAN_CORE_MANIFEST, UCAN_PEER_AUTH_SOURCE,));
    assert_eq!(CARGO_LOCK.matches("name = \"zeroize\"").count(), 1);
    assert!(CARGO_LOCK.contains("version = \"1.9.0\""));
}

#[test]
fn stale_zeroize_identity_is_rejected() {
    let drifted_identity = UCAN_PEER_AUTH_SOURCE.replace(REVIEWED_ZEROIZE_IDENTITY, DRIFTED_ZEROIZE_IDENTITY);
    assert!(!zeroize_cohort_align(UCAN_MANIFEST, UCAN_CORE_MANIFEST, &drifted_identity,));
}

#[test]
fn fixture_rail_rejects_a_missing_pin() {
    assert!(has_pin_with_reason(POSITIVE_FIXTURE, ADVISORY_PASTE));
    assert!(has_pin_with_reason(POSITIVE_FIXTURE, ADVISORY_ANYHOW));
    // The negative fixture drops the paste pin; the rail must reject it.
    assert!(has_pin_with_reason(NEGATIVE_FIXTURE, ADVISORY_ANYHOW));
    assert!(!has_pin_with_reason(NEGATIVE_FIXTURE, ADVISORY_PASTE));
}

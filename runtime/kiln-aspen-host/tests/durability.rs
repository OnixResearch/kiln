#![cfg(feature = "test-fixture")]
#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the recovery test exercises concrete durable host adapters"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen host names preserve their owner prefixes"
)]
#![allow(
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert closed durability fixtures"
)]

use std::io::Write;

use kiln_adapter_aspen::profile::{AdmittedAspenRuntimeProfile, admit_profile, profile_fixture};
use kiln_aspen_host::cohort::{HostCohort, HostPaths};
use kiln_aspen_host::durability::DurableHostStateMarker;

const TRUNCATED_LEDGER_BYTES: u64 = 8;
const CORRUPT_VALUE_LEAF: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn fixture() -> (tempfile::TempDir, AdmittedAspenRuntimeProfile, HostPaths) {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let profile =
        admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("Aspen profile"));
    let paths = HostPaths {
        extension_executable: std::path::PathBuf::from(env!(
            "CARGO_BIN_EXE_kiln-aspen-host-extension-fixture"
        )),
        runtime_profile: directory.path().join("runtime-profile.json"),
        workspace: directory.path().to_path_buf(),
    };
    let cohort = HostCohort::build(profile.clone(), &paths)
        .unwrap_or_else(|error| unreachable!("fresh durable cohort: {error}"));
    drop(cohort);
    (directory, profile, paths)
}

#[test]
fn exact_durable_state_reopens_for_the_same_cohort() {
    let (_directory, profile, paths) = fixture();
    let recovered = HostCohort::build(profile, &paths)
        .unwrap_or_else(|error| unreachable!("recovered durable cohort: {error}"));
    assert_eq!(
        recovered.state_mode,
        kiln_aspen_host::durability::HostStateMode::Recovered
    );
}

#[test]
fn missing_native_journal_fails_closed() {
    let (_directory, profile, paths) = fixture();
    std::fs::remove_file(
        paths
            .workspace
            .join("native-journal")
            .join("fabric-durability.redb"),
    )
    .unwrap_or_else(|_| unreachable!("remove native journal"));
    assert!(HostCohort::build(profile, &paths).is_err());
}

#[test]
fn changed_state_marker_fails_closed() {
    let (_directory, profile, paths) = fixture();
    let marker_path = paths.workspace.join("host-state.json");
    let bytes = std::fs::read(&marker_path).unwrap_or_else(|_| unreachable!("read state marker"));
    let mut marker: DurableHostStateMarker =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!("decode state marker"));
    marker.cohort_identity = "blake3:changed".to_string();
    std::fs::write(
        marker_path,
        serde_json::to_vec_pretty(&marker)
            .unwrap_or_else(|_| unreachable!("encode changed state marker")),
    )
    .unwrap_or_else(|_| unreachable!("write changed state marker"));
    assert!(HostCohort::build(profile, &paths).is_err());
}

// r[verify kiln.aspen_host.production_durability.rejected]
#[test]
fn truncated_operation_ledger_fails_closed() {
    let (_directory, profile, paths) = fixture();
    let ledger_path = paths
        .workspace
        .join("operation-ledger")
        .join("operations.sqlite");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(ledger_path)
        .unwrap_or_else(|_| unreachable!("open operation ledger"));
    file.set_len(TRUNCATED_LEDGER_BYTES)
        .unwrap_or_else(|_| unreachable!("truncate operation ledger"));
    assert!(HostCohort::build(profile, &paths).is_err());
}

#[test]
fn corrupt_callback_value_fails_closed() {
    let (_directory, profile, paths) = fixture();
    let value_path = paths
        .workspace
        .join("callback-values")
        .join(CORRUPT_VALUE_LEAF);
    let mut file = std::fs::File::create(value_path)
        .unwrap_or_else(|_| unreachable!("create corrupt callback value"));
    file.write_all(b"corrupt callback value")
        .and_then(|()| file.sync_all())
        .unwrap_or_else(|_| unreachable!("write corrupt callback value"));
    assert!(HostCohort::build(profile, &paths).is_err());
}

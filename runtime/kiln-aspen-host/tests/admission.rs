#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the admission test keeps the concrete Aspen profile and host boundary visible"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen host names preserve their protocol ownership"
)]
#![allow(
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert closed admission fixtures"
)]

use std::time::Duration;

use kiln_adapter_aspen::profile::{admit_profile, profile_fixture};
use kiln_aspen_host::cohort::{
    EXTENSION_RUNTIME_PROFILE_ARGUMENT, EXTENSION_RUNTIME_PROFILE_IDENTITY_ARGUMENT, HostCohort,
    HostPaths,
};
use kiln_aspen_host::server::ServerConfig;

const TEST_MAXIMUM_FRAME_BYTES: usize = 1_048_576;
const TEST_MAXIMUM_REQUESTS: usize = 8;
const TEST_TIMEOUT_MILLISECONDS: u64 = 5_000;

// r[verify kiln.aspen_host.composition.rejected]
#[test]
fn missing_extension_artifact_fails_before_host_installation() {
    let profile =
        admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("profile admits"));
    let workspace = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary workspace"));
    let paths = HostPaths {
        extension_executable: workspace.path().join("missing-extension"),
        runtime_profile: workspace.path().join("runtime-profile.json"),
        workspace: workspace.path().to_path_buf(),
    };
    assert!(HostCohort::build(profile, &paths).is_err());
}

// r[verify kiln.aspen_host.extension_profile_binding.accepted]
#[test]
fn execution_template_binds_exact_runtime_profile_path_and_identity() {
    let profile =
        admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("profile admits"));
    let profile_identity = profile.profile_identity.as_str().to_string();
    let workspace = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary workspace"));
    let runtime_profile = workspace.path().join("runtime-profile.json");
    let executable =
        std::env::current_exe().unwrap_or_else(|_| unreachable!("test executable path must exist"));
    let paths = HostPaths {
        extension_executable: executable,
        runtime_profile: runtime_profile.clone(),
        workspace: workspace.path().to_path_buf(),
    };
    let cohort = HostCohort::build(profile, &paths)
        .unwrap_or_else(|error| unreachable!("host cohort must admit: {error}"));
    assert_eq!(
        cohort.template.request.arguments,
        vec![
            EXTENSION_RUNTIME_PROFILE_ARGUMENT.to_string(),
            runtime_profile.to_string_lossy().into_owned(),
            EXTENSION_RUNTIME_PROFILE_IDENTITY_ARGUMENT.to_string(),
            profile_identity,
        ],
    );
}

// r[verify kiln.aspen_host.transport.rejected]
#[test]
fn unsafe_or_unbounded_server_configuration_is_rejected() {
    let valid = ServerConfig {
        socket_path: std::path::PathBuf::from("/run/kiln-aspen-host/test.sock"),
        maximum_frame_bytes: TEST_MAXIMUM_FRAME_BYTES,
        maximum_requests: TEST_MAXIMUM_REQUESTS,
        timeout: Duration::from_millis(TEST_TIMEOUT_MILLISECONDS),
    };
    assert!(valid.validate().is_ok());

    let mut relative = valid.clone();
    relative.socket_path = std::path::PathBuf::from("run/test.sock");
    assert!(relative.validate().is_err());

    let mut unbounded = valid;
    unbounded.maximum_requests = 0;
    assert!(unbounded.validate().is_err());
}

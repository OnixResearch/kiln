#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

use kiln_core::discovery::{DiscoveryEffectPlan, validate_discovery_plan};
use kiln_core::types::Diagnostic;
use kiln_provider_api::{PortFailure, PortResult};

pub const ONIX_CANDIDATE_PROFILE: &str = "onix-nix-eval-v1";
pub const ONIX_EVALUATOR_KIND: &str = "onix-nix";
pub const ONIX_HOST_ABI_SOURCE_SYMBOL: &str = "nix_wasm_init_v1";
pub const ONIX_FORK_REVISION: &str = "f93f28ad9dce007baae13aa461c16abf656d1171";
pub const ONIX_FORK_NAR_HASH: &str = "sha256-sxgxa7OyG4WPBehA05IfYrgs1REwnIS3m+SW6utVX+A=";
pub const ONIX_EVALUATOR_REVISION: &str = "9f1553afabd95f9f59951f5c946e86005af7196a";
pub const ONIX_EVALUATOR_NAR_HASH: &str = "sha256-t1FbcjvTWQ1WO3hJNKN+viNXuz4BX9Pte5K4F+IJLDk=";
pub const ONIX_API_PATCH_BLAKE3: &str =
    "3dda2b2961149c553dcacc9de4a765bf0346c2baae39874f2850dc75dd03b15c";
pub const ONIX_DERIVATIONS_PATCH_BLAKE3: &str =
    "9600554082a4b7e1ec8dc53f26b30ac4919bde1cdd5af7d825c1cadf3c8793a0";
pub const ONIX_EXECUTABLE_PATH: &str =
    "/nix/store/69k3iijnj5s8417anq7ncgc9f006gc1n-nix-eval-jobs-2.36.4/bin/nix-eval-jobs";
pub const ONIX_EXECUTABLE_BLAKE3: &str =
    "4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361";
pub const ONIX_PLUGIN_PATH: &str =
    "/nix/store/di3jrnpmrc4lff4581vxmprmcsgswd0r-nickel-plugin-0.1.0/nickel_plugin.wasm";
pub const ONIX_PLUGIN_BLAKE3: &str =
    "6bbf0180617b4d87746416eb8d58d09dea1076312935038b4d92706e5c7626f6";
pub const ONIX_REQUIRED_FEATURES: &[&str] = &["flakes", "nix-command", "wasm-builtin"];
/// Exact ldd-observed evaluator closure, not the separately built OnixOS Nix CLI.
pub const ONIX_LINKED_COMPONENTS: &[(&str, &str)] = &[
    (
        "/nix/store/xs9x9imziz5fz9d089vs64xxahz5v7iy-nix-store-2.36.0/lib/libnixstore.so.2.36.0",
        "83efc75ea9a8bddf172cdd4f2827d2b85ee986bc872e407c6f509e5cc24bff9d",
    ),
    (
        "/nix/store/sifz76vs2qn3ng7d8bc9ll96zb58x4zv-nix-util-2.36.0/lib/libnixutil.so.2.36.0",
        "58d1aa3a8a8a28fc264ab3604d6d327f6b5b9756fec516a5b218120566fc3cd4",
    ),
    (
        "/nix/store/43v5jy3q8lql3snvdzqwj0drvsrmihxg-nix-fetchers-2.36.0/lib/libnixfetchers.so.2.36.0",
        "8afea53fa2c1fdfe215fe86fc3c35f473a4a2f90304d2d7a7a4c7c2603629f90",
    ),
    (
        "/nix/store/77sdbia58r9r11xy9xn9q45sgy5gvcvl-nix-expr-2.36.0/lib/libnixexpr.so.2.36.0",
        "15ab72e4ab76edcbcaa836d47a4fae5da119ead8dca70f8d1e6d2ef3ee12a1e6",
    ),
    (
        "/nix/store/wskhfcg99q278r1fps88rk22jncndmrf-nix-flake-2.36.0/lib/libnixflake.so.2.36.0",
        "a42f60463440478c599502decf74e82e2f7310316335c01c156af65a74e50c8b",
    ),
    (
        "/nix/store/5g13fg62n6fqzyzgvv2i0g1f8a1f4q28-nix-main-2.36.0/lib/libnixmain.so.2.36.0",
        "8cbb440566851976a87fd0c0798eec619c1d43829c8313b2fd1c22d34824b309",
    ),
    (
        "/nix/store/l3gh3m8hzzpfk5yx73nxfpq50nr354jp-nix-cmd-2.36.0/lib/libnixcmd.so.2.36.0",
        "871233de0fe2784ad54fb4804be3a936907aee77f580c634974b47da73a0fd8f",
    ),
];

const MAX_EVALUATOR_BYTES: u64 = 536_870_912;
const MAX_PLUGIN_BYTES: u64 = 67_108_864;
const MAX_LINKED_COMPONENT_BYTES: u64 = 536_870_912;

fn denied(code: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, "nix_eval.onix_candidate", message))
}

/// Check measured evaluator facts and fail closed until the source and manifest
/// have published identities. Never grants authority to evaluate a jobset.
pub fn admit_onix_candidate(plan: &DiscoveryEffectPlan) -> PortResult<()> {
    validate_discovery_plan(plan).map_err(PortFailure::Domain)?;
    let cohort = plan.evaluator_cohort.as_ref().ok_or_else(|| {
        denied(
            "onix_cohort_missing",
            "explicit Onix evaluator facts are required",
        )
    })?;
    if plan.provider_profile.as_str() != ONIX_CANDIDATE_PROFILE
        || cohort.kind.as_str() != ONIX_EVALUATOR_KIND
    {
        return Err(denied(
            "onix_profile_mismatch",
            "the Onix candidate profile must be explicitly selected",
        ));
    }
    if cohort.executable_identity.as_str() != ONIX_EXECUTABLE_BLAKE3 {
        return Err(denied(
            "onix_executable_drift",
            "the measured evaluator bytes differ",
        ));
    }
    if cohort.host_abi.as_str() != ONIX_HOST_ABI_SOURCE_SYMBOL {
        return Err(denied(
            "onix_host_abi_drift",
            "the source-defined Wasm host handshake differs",
        ));
    }
    if cohort.features.len() != ONIX_REQUIRED_FEATURES.len()
        || !cohort
            .features
            .iter()
            .zip(ONIX_REQUIRED_FEATURES.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
    {
        return Err(denied(
            "onix_feature_drift",
            "the admitted experimental feature set differs",
        ));
    }
    if cohort.plugin_identity.as_str() != ONIX_PLUGIN_BLAKE3
        || cohort.plugin.provider_namespace.as_str() != "nix"
        || cohort.plugin.native_kind.as_str() != "store-path"
        || cohort.plugin.native_id.as_str() != ONIX_PLUGIN_PATH
    {
        return Err(denied(
            "onix_plugin_drift",
            "the source-bound Nickel plugin bytes or path differ",
        ));
    }
    if cohort.components.len() != ONIX_LINKED_COMPONENTS.len()
        || !cohort
            .components
            .iter()
            .zip(ONIX_LINKED_COMPONENTS.iter())
            .all(|(actual, (path, identity))| {
                actual.library.provider_namespace.as_str() == "nix"
                    && actual.library.native_kind.as_str() == "shared-library"
                    && actual.library.native_id.as_str() == *path
                    && actual.identity.as_str() == *identity
            })
    {
        return Err(denied(
            "onix_component_drift",
            "the evaluator's linked Nix library closure differs",
        ));
    }
    Err(denied(
        "onix_source_unpublished",
        "no published immutable source and manifest identities are available for this cohort",
    ))
}

/// Hash the already measured evaluator closure without following final path
/// symlinks, without spawning any program, and without a fallback file source.
#[cfg(target_os = "linux")]
pub fn verify_onix_candidate_files() -> PortResult<()> {
    verify_bounded_file(
        ONIX_EXECUTABLE_PATH,
        ONIX_EXECUTABLE_BLAKE3,
        MAX_EVALUATOR_BYTES,
        "onix_executable_drift",
    )?;
    verify_bounded_file(
        ONIX_PLUGIN_PATH,
        ONIX_PLUGIN_BLAKE3,
        MAX_PLUGIN_BYTES,
        "onix_plugin_drift",
    )?;
    for (path, identity) in ONIX_LINKED_COMPONENTS {
        verify_bounded_file(
            path,
            identity,
            MAX_LINKED_COMPONENT_BYTES,
            "onix_component_drift",
        )?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn verify_onix_candidate_files() -> PortResult<()> {
    Err(denied(
        "onix_platform_unsupported",
        "the measured evaluator cohort requires Linux",
    ))
}

#[cfg(target_os = "linux")]
fn verify_bounded_file(
    path: &str,
    identity: &str,
    max_bytes: u64,
    drift_code: &str,
) -> PortResult<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| {
            denied(
                "onix_file_unavailable",
                "a measured file cannot be opened without following symlinks",
            )
        })?;
    let metadata = file.metadata().map_err(|_| {
        denied(
            "onix_file_unavailable",
            "a measured file has no readable metadata",
        )
    })?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(denied(
            "onix_file_bound",
            "a measured file is not a bounded regular file",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| denied("onix_file_unavailable", "a measured file cannot be read"))?;
        if count == 0 {
            break;
        }
        let count_u64 = u64::try_from(count)
            .map_err(|_| denied("onix_file_bound", "a measured file exceeds its byte bound"))?;
        observed = observed
            .checked_add(count_u64)
            .ok_or_else(|| denied("onix_file_bound", "a measured file exceeds its byte bound"))?;
        if observed > max_bytes {
            return Err(denied(
                "onix_file_bound",
                "a measured file exceeds its byte bound",
            ));
        }
        let bytes = buffer
            .get(..count)
            .ok_or_else(|| denied("onix_file_bound", "a measured file exceeds its byte bound"))?;
        hasher.update(bytes);
    }
    if hasher.finalize().to_hex().as_str() != identity {
        return Err(denied(
            drift_code,
            "measured file bytes differ from their exact pin",
        ));
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn measured_file_preflight_binds_bytes_bounds_and_no_follow() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let regular = directory.path().join("measured");
        let link = directory.path().join("link");
        std::fs::write(&regular, b"measured bytes").unwrap_or_else(|_| unreachable!());
        symlink(&regular, &link).unwrap_or_else(|_| unreachable!());
        let identity = blake3::hash(b"measured bytes").to_hex().to_string();
        assert_eq!(
            verify_bounded_file(
                regular.to_str().unwrap_or_else(|| unreachable!()),
                &identity,
                64,
                "onix_test_drift",
            ),
            Ok(())
        );
        assert!(matches!(
            verify_bounded_file(
                link.to_str().unwrap_or_else(|| unreachable!()),
                &identity,
                64,
                "onix_test_drift",
            ),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_file_unavailable"
        ));
        assert!(matches!(
            verify_bounded_file(
                regular.to_str().unwrap_or_else(|| unreachable!()),
                &identity,
                12,
                "onix_test_drift",
            ),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_file_bound"
        ));
        assert!(matches!(
            verify_bounded_file(
                regular.to_str().unwrap_or_else(|| unreachable!()),
                blake3::hash(b"other bytes").to_hex().as_str(),
                64,
                "onix_test_drift",
            ),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_test_drift"
        ));
    }
}

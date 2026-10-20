use std::path::Path;

use kiln_core::discovery::{
    DiscoveryEffectPlan, DiscoveryEvaluatorCohort, DiscoveryEvaluatorComponent, DiscoverySourceMode,
};
use kiln_core::types::{Alias, BLAKE3_HEX_BYTES, ContentId, Diagnostic, ExternalRef};
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};

use crate::mapping::selector_is_supported;
use crate::onix_pins::{
    ONIX_FIXTURE_REVISION, ONIX_MANIFEST_BLAKE3, ONIX_SOURCE_BLAKE3, ONIX_SOURCE_PATH,
};
use crate::profile::PINNED_PLATFORM;
use crate::runtime::NixEvalRuntime;

pub const ONIX_PROFILE: &str = "onix-nix-eval-v1";
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

pub const ONIX_ROOT_SELECTOR: &str = "jobs.x86_64-linux";
pub const ONIX_EVALUATOR_SOURCE: &str = "https://github.com/NixOS/nix-eval-jobs";
pub const ONIX_FORK_SOURCE: &str = "https://github.com/onixcomputer/nix";
pub const ONIX_PLUGIN_SOURCE: &str = "https://github.com/onixcomputer/onix-wasm";
pub const ONIX_PLUGIN_REVISION: &str = "54bf09574c42a21f9f38dc7e01bc44440e6fbc07";
pub const ONIX_ADAPTER_VERSION: &str = "2.36.4";
pub const ONIX_NIXPKGS_REVISION: &str = "34ca302a9572963c02e385c056be37c85ff51b77";
pub const ONIX_SOURCE_VERSION: &str = "0.1.6";
pub const ONIX_NIX_VERSION: &str = "2.36.0";
pub const ONIX_EVALUATOR_LICENSE: &str = "GPL-3.0";
pub const ONIX_FORK_LICENSE: &str = "LGPL-2.1-or-later";
pub const ONIX_PLUGIN_LICENSE: &str = "NOASSERTION (plugin wrapper); vendored Nickel MIT";
pub const ONIX_PACKAGE_SOURCE: &str = "https://github.com/OnixResearch/onixpkgs";
pub const ONIX_PACKAGE_REVISION: &str = "3bdf51c00947ffd6b8bd3355ff83509a03752042";
pub const ONIX_PACKAGE_LICENSE: &str = "NOASSERTION (package wrapper); evaluator GPL-3.0";
pub const ONIX_FIXTURE_UPSTREAM: &str = "https://github.com/onixcomputer/onix-modules";
pub const ONIX_FIXTURE_LICENSE: &str = "AGPL-3.0-or-later";

const MAX_PLUGIN_BYTES: u64 = 67_108_864;
const MAX_LINKED_COMPONENT_BYTES: u64 = 536_870_912;

/// Source provenance excludes the manifest's own identity to avoid a circular hash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnixProvenance {
    pub evaluator_kind: String,
    pub evaluator_source: String,
    pub evaluator_license: String,
    pub evaluator_nar_hash: String,
    pub fork_source: String,
    pub fork_license: String,
    pub fork_nar_hash: String,
    pub api_patch_blake3: String,
    pub derivations_patch_blake3: String,
    pub executable_identity: ContentId,
    pub host_abi: Alias,
    pub features: Vec<Alias>,
    pub plugin: ExternalRef,
    pub plugin_identity: ContentId,
    pub plugin_source: String,
    pub plugin_revision: String,
    pub components: Vec<DiscoveryEvaluatorComponent>,
    pub plugin_license: String,
    pub package_source: String,
    pub package_revision: String,
    pub package_license: String,
    pub fixture_upstream: String,
    pub fixture_license: String,
    pub fixture_source: ExternalRef,
    pub fixture_source_identity: ContentId,
    pub fixture_revision: String,
}

/// One explicitly admitted consumer, independent of evaluator/plugin provenance.
// r[impl kiln.onixos_pilot.source]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnixSourceBinding {
    pub source: ExternalRef,
    /// BLAKE3 of the exact source root's `flake.nix` bytes (not a NAR hash).
    pub source_identity: ContentId,
    pub root_selector: Alias,
    pub system: Alias,
}

// r[impl kiln.onixos_pilot.source]
pub(crate) fn admit_onix_source_binding(binding: &OnixSourceBinding) -> PortResult<()> {
    binding.source.validate().map_err(PortFailure::Domain)?;
    if binding.source.provider_namespace.as_str() != "kiln-source"
        || binding.source.native_kind.as_str() != "nix-store-path"
        || !direct_store_root(binding.source.native_id.as_str())
        || binding.source_identity.as_str().len() != BLAKE3_HEX_BYTES
        || !binding
            .source_identity
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || !selector_is_supported(binding.root_selector.as_str())
        || binding.system.as_str() != PINNED_PLATFORM
    {
        return Err(denied(
            "onix_source_binding",
            "Onix requires a direct immutable store root, supported selector, and x86_64-linux",
        ));
    }
    Ok(())
}

fn direct_store_root(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    let Some((hash, name)) = name.split_once('-') else {
        return false;
    };
    const STORE_HASH_LENGTH: usize = 32;
    const NIX_BASE32: &[u8] = b"0123456789abcdfghijklmnpqrsvwxyz";
    hash.len() == STORE_HASH_LENGTH
        && hash.bytes().all(|byte| NIX_BASE32.contains(&byte))
        && !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.' | b'_'))
}

pub fn onix_cohort() -> PortResult<DiscoveryEvaluatorCohort> {
    Ok(DiscoveryEvaluatorCohort {
        kind: Alias::new(ONIX_EVALUATOR_KIND)?,
        executable_identity: ContentId::from_blake3_hex(ONIX_EXECUTABLE_BLAKE3)?,
        manifest_identity: ContentId::from_blake3_hex(ONIX_MANIFEST_BLAKE3)?,
        host_abi: Alias::new(ONIX_HOST_ABI_SOURCE_SYMBOL)?,
        features: ONIX_REQUIRED_FEATURES
            .iter()
            .map(|feature| Alias::new(*feature))
            .collect::<Result<_, _>>()?,
        plugin: ExternalRef::new("nix", "store-path", ONIX_PLUGIN_PATH)?,
        plugin_identity: ContentId::from_blake3_hex(ONIX_PLUGIN_BLAKE3)?,
        components: ONIX_LINKED_COMPONENTS
            .iter()
            .map(|(path, identity)| {
                Ok(DiscoveryEvaluatorComponent {
                    library: ExternalRef::new("nix", "shared-library", path)?,
                    identity: ContentId::from_blake3_hex(*identity)?,
                })
            })
            .collect::<PortResult<_>>()?,
    })
}

// r[impl kiln.nix_eval_jobs_onix.cohort]
pub fn onix_provenance() -> PortResult<OnixProvenance> {
    let cohort = onix_cohort()?;
    Ok(OnixProvenance {
        evaluator_kind: ONIX_EVALUATOR_KIND.to_string(),
        evaluator_license: ONIX_EVALUATOR_LICENSE.to_string(),
        evaluator_source: ONIX_EVALUATOR_SOURCE.to_string(),
        evaluator_nar_hash: ONIX_EVALUATOR_NAR_HASH.to_string(),
        fork_license: ONIX_FORK_LICENSE.to_string(),
        fork_source: ONIX_FORK_SOURCE.to_string(),
        fork_nar_hash: ONIX_FORK_NAR_HASH.to_string(),
        api_patch_blake3: ONIX_API_PATCH_BLAKE3.to_string(),
        derivations_patch_blake3: ONIX_DERIVATIONS_PATCH_BLAKE3.to_string(),
        executable_identity: cohort.executable_identity,
        host_abi: cohort.host_abi,
        features: cohort.features,
        plugin: cohort.plugin,
        plugin_identity: cohort.plugin_identity,
        plugin_source: ONIX_PLUGIN_SOURCE.to_string(),
        plugin_license: ONIX_PLUGIN_LICENSE.to_string(),
        package_source: ONIX_PACKAGE_SOURCE.to_string(),
        package_revision: ONIX_PACKAGE_REVISION.to_string(),
        package_license: ONIX_PACKAGE_LICENSE.to_string(),
        fixture_upstream: ONIX_FIXTURE_UPSTREAM.to_string(),
        fixture_license: ONIX_FIXTURE_LICENSE.to_string(),
        plugin_revision: ONIX_PLUGIN_REVISION.to_string(),
        components: cohort.components,
        fixture_source: ExternalRef::new("kiln-source", "nix-store-path", ONIX_SOURCE_PATH)?,
        fixture_source_identity: ContentId::from_blake3_hex(ONIX_SOURCE_BLAKE3)?,
        fixture_revision: ONIX_FIXTURE_REVISION.to_string(),
    })
}

// r[impl kiln.nix_eval_jobs_onix.admission]
// r[impl kiln.nix_eval_jobs_onix.fallback]
pub(crate) fn admit_onix_cohort(cohort: &DiscoveryEvaluatorCohort) -> PortResult<()> {
    let drift = if cohort.kind.as_str() != ONIX_EVALUATOR_KIND {
        Some((
            "onix_profile_mismatch",
            "the explicit Onix evaluator kind differs",
        ))
    } else if cohort.executable_identity.as_str() != ONIX_EXECUTABLE_BLAKE3 {
        Some((
            "onix_executable_drift",
            "the measured evaluator identity differs",
        ))
    } else if cohort.manifest_identity.as_str() != ONIX_MANIFEST_BLAKE3 {
        Some((
            "onix_manifest_drift",
            "the published manifest identity differs",
        ))
    } else if cohort.host_abi.as_str() != ONIX_HOST_ABI_SOURCE_SYMBOL {
        Some((
            "onix_host_abi_drift",
            "the source-defined Wasm host handshake differs",
        ))
    } else if cohort.features.len() != ONIX_REQUIRED_FEATURES.len()
        || !cohort
            .features
            .iter()
            .zip(ONIX_REQUIRED_FEATURES)
            .all(|(actual, expected)| actual.as_str() == *expected)
    {
        Some((
            "onix_feature_drift",
            "the exact experimental feature set differs",
        ))
    } else if cohort.plugin_identity.as_str() != ONIX_PLUGIN_BLAKE3
        || cohort.plugin.provider_namespace.as_str() != "nix"
        || cohort.plugin.native_kind.as_str() != "store-path"
        || cohort.plugin.native_id.as_str() != ONIX_PLUGIN_PATH
    {
        Some((
            "onix_plugin_drift",
            "the source-bound Nickel plugin differs",
        ))
    } else if cohort.components.len() != ONIX_LINKED_COMPONENTS.len()
        || !cohort.components.iter().zip(ONIX_LINKED_COMPONENTS).all(
            |(actual, (path, identity))| {
                actual.library.provider_namespace.as_str() == "nix"
                    && actual.library.native_kind.as_str() == "shared-library"
                    && actual.library.native_id.as_str() == *path
                    && actual.identity.as_str() == *identity
            },
        )
    {
        Some((
            "onix_component_drift",
            "the evaluator linked Nix library closure differs",
        ))
    } else {
        None
    };
    match drift {
        Some((code, message)) => Err(denied(code, message)),
        None => Ok(()),
    }
}

// r[impl kiln.nix_eval_jobs_onix.functional_core]
// r[impl kiln.onixos_pilot.source]
pub(crate) fn admit_onix_source(
    binding: &OnixSourceBinding,
    plan: &DiscoveryEffectPlan,
) -> PortResult<()> {
    admit_onix_source_binding(binding)?;
    if plan.source != binding.source
        || plan.source_identity != binding.source_identity
        || plan.root_selector != binding.root_selector
        || plan.system != binding.system
        || plan.source_mode != DiscoverySourceMode::Flake
    {
        return Err(denied(
            "onix_source_drift",
            "source, flake identity, selector, platform, or mode differs from the explicit binding",
        ));
    }
    Ok(())
}

pub(crate) fn verify_onix_files<R: NixEvalRuntime>(runtime: &R) -> PortResult<()> {
    verify_file(
        runtime,
        Path::new(ONIX_PLUGIN_PATH),
        ONIX_PLUGIN_BLAKE3,
        MAX_PLUGIN_BYTES,
        "onix_plugin_drift",
    )?;
    for (path, identity) in ONIX_LINKED_COMPONENTS {
        verify_file(
            runtime,
            Path::new(path),
            identity,
            MAX_LINKED_COMPONENT_BYTES,
            "onix_component_drift",
        )?;
    }
    Ok(())
}

pub(crate) fn verify_file<R: NixEvalRuntime>(
    runtime: &R,
    path: &Path,
    expected: &str,
    max_bytes: u64,
    code: &str,
) -> PortResult<()> {
    let actual = runtime
        .file_identity_no_follow(path, max_bytes)
        .map_err(|_| {
            denied(
                code,
                "pinned file cannot be observed within its no-follow bound",
            )
        })?;
    if actual.as_str() != expected {
        return Err(denied(
            code,
            "pinned file bytes differ from the exact cohort",
        ));
    }
    Ok(())
}

pub(crate) fn denied(code: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, "nix_eval.onix", message))
}

#![allow(dead_code)]

use std::path::PathBuf;

use kiln_adapter_nix::{NixBuildLimits, NixBuildProfile, NixBuildProfileConfig, NixOutputBinding};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
};
use kiln_nix_build::NixBuildIntent;

pub const IDENTITY_HEX_BYTES: usize = 64;
pub const STORE_PATH: &str = "/nix/store";
pub const BASE_DRV: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-base.drv";
pub const BASE_OUT: &str = "/nix/store/dddddddddddddddddddddddddddddddd-base";
pub const GC_ROOT_BASE: &str = "/tmp/kiln-nix-composition-roots";
pub const HOME_PATH: &str = "/tmp/kiln-nix-composition-home";

pub fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

pub fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

pub fn limits() -> NixBuildLimits {
    NixBuildLimits {
        max_required_products: 16,
        max_products: 64,
        max_logs: 1,
        max_gc_roots: 256,
        max_request_bytes: 1_048_576,
        max_result_bytes: 1_048_576,
        max_stderr_bytes: 65_536,
        max_log_bytes: 65_536,
        max_executable_bytes: 268_435_456,
        max_derivation_bytes: 1_048_576,
        max_in_flight: 4,
        stdin_max_bytes: 1,
        timeout_milliseconds: 30_000,
        poll_interval_milliseconds: 5,
        teardown_timeout_milliseconds: 1_000,
    }
}

pub fn base() -> NixBuildProfile {
    NixBuildProfile::pinned(
        ProviderProfileId::new("nix-build-v1").unwrap_or_else(|_| unreachable!()),
        kiln_adapter_nix::NixRuntimeBinding {
            executable_path: PathBuf::from(kiln_adapter_nix::PINNED_NIX_EXECUTABLE_PATH),
            executable_identity: ContentId::from_blake3_hex(
                kiln_adapter_nix::PINNED_NIX_EXECUTABLE_BLAKE3,
            )
            .unwrap_or_else(|_| unreachable!()),
            nix_store_executable_path: PathBuf::from(
                kiln_adapter_nix::PINNED_NIX_STORE_EXECUTABLE_PATH,
            ),
            nix_store_executable_identity: ContentId::from_blake3_hex(
                kiln_adapter_nix::PINNED_NIX_STORE_EXECUTABLE_BLAKE3,
            )
            .unwrap_or_else(|_| unreachable!()),
            derivation_path: PathBuf::from(BASE_DRV),
            derivation_identity: content('2'),
            plan_identity: content('3'),
            policy_identity: content('4'),
            store_path: PathBuf::from(STORE_PATH),
            gc_root_base: PathBuf::from(GC_ROOT_BASE),
            home_base: PathBuf::from(HOME_PATH),
            worker_identity: content('5'),
            store_identity: content('6'),
            expected_outputs: vec![NixOutputBinding {
                name: alias("out"),
                path: PathBuf::from(BASE_OUT),
            }],
        },
        limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

pub fn intent() -> NixBuildIntent {
    NixBuildIntent {
        effect_id: EffectId::new("build-effect-a").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('1'),
        candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!()),
        plan_identity: content('3'),
        policy_identity: content('4'),
        platform: alias("x86_64-linux"),
    }
}

pub fn config() -> NixBuildProfileConfig {
    let profile = base();
    NixBuildProfileConfig {
        nixpkgs_revision: profile.nixpkgs_revision.clone(),
        nix_version: profile.nix_version.clone(),
        bounded_exec_source: profile.bounded_exec_source.clone(),
        bounded_exec_revision: profile.bounded_exec_revision.clone(),
        bounded_exec_license: profile.bounded_exec_license.clone(),
        request_schema: profile.request_schema.clone(),
        observation_schema: profile.observation_schema.clone(),
        receipt_schema: profile.receipt_schema.clone(),
        native_report_schema: profile.native_report_schema.clone(),
        command: profile.command.clone(),
        platform: profile.platform.clone(),
        provider_profile_id: profile.provider.profile_id.as_str().to_string(),
        capabilities: vec![
            "cancel".to_string(),
            "dispatch".to_string(),
            "observe".to_string(),
            "read_logs".to_string(),
            "reconcile".to_string(),
        ],
        executable_path: profile.runtime.executable_path.clone(),
        executable_identity: profile.runtime.executable_identity.clone(),
        nix_store_executable_path: profile.runtime.nix_store_executable_path.clone(),
        nix_store_executable_identity: profile.runtime.nix_store_executable_identity.clone(),
        derivation_path: profile.runtime.derivation_path.clone(),
        derivation_identity: profile.runtime.derivation_identity.clone(),
        plan_identity: profile.runtime.plan_identity.clone(),
        policy_identity: profile.runtime.policy_identity.clone(),
        store_path: profile.runtime.store_path.clone(),
        gc_root_base: profile.runtime.gc_root_base.clone(),
        home_base: profile.runtime.home_base.clone(),
        worker_identity: profile.runtime.worker_identity.clone(),
        store_identity: profile.runtime.store_identity.clone(),
        expected_outputs: profile.runtime.expected_outputs.clone(),
        limits: profile.limits,
        required_non_claims: profile.required_non_claims.clone(),
    }
}

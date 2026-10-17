#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the standalone composition root keeps concrete bounded-execution and publication adapters visible"
)]

mod admission;
mod execution;
mod model;
mod publication;

pub use admission::load_hosted_provider_profile;
pub use model::*;

use std::sync::atomic::AtomicBool;

use admission::observe_and_admit;
use execution::{execute_process, render_log};
use publication::publish_exact;

const SOURCE_URI_PREFIX: &str = "git+file://";
const SOURCE_URI_REVISION_SEPARATOR: &str = "?rev=";

// r[impl kiln.radicle.hosted_nix_provider]
pub fn execute_hosted_nix_provider(
    profile: &HostedNixProviderProfile,
    request: &HostedProviderRequest,
    cancellation: &AtomicBool,
) -> HostedProviderResult<HostedProviderReceipt> {
    observe_and_admit(profile, request)?;
    let source_uri = format!(
        "{SOURCE_URI_PREFIX}{}{SOURCE_URI_REVISION_SEPARATOR}{}",
        profile.source_root.display(),
        request.revision,
    );
    let run_identity = run_identity(profile, request)?;
    let observation = execute_process(profile, &source_uri, cancellation)?;
    let log_bytes = render_log(profile, request, &source_uri, &observation)?;
    let log_blake3 = blake3::hash(&log_bytes).to_hex().to_string();
    let report = HostedProviderReport {
        schema: HOSTED_NIX_PROVIDER_SCHEMA,
        run_identity: &run_identity,
        repository: &request.repository,
        revision: &request.revision,
        wrapper_blake3: &profile.wrapper_blake3,
        outcome: observation.outcome,
        exit_code: observation.exit_code,
        log_blake3: &log_blake3,
        non_claims: &profile.non_claims,
    };
    let report_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|_| HostedProviderError::Infrastructure("encode provider report".to_string()))?;
    let (log_leaf, report_blake3) =
        publish_report(profile, &run_identity, &log_bytes, &report_bytes)?;
    Ok(HostedProviderReceipt {
        schema: HOSTED_NIX_PROVIDER_SCHEMA.to_string(),
        run_identity,
        repository: request.repository.clone(),
        revision: request.revision.clone(),
        wrapper_blake3: profile.wrapper_blake3.clone(),
        outcome: observation.outcome,
        exit_code: observation.exit_code,
        log_blake3,
        report_blake3,
        log_url: format!(
            "{}/{}/{}",
            profile.report_base_url.trim_end_matches('/'),
            profile.report_namespace,
            log_leaf,
        ),
        non_claims: profile.non_claims.clone(),
    })
}

fn publish_report(
    profile: &HostedNixProviderProfile,
    run_identity: &str,
    log_bytes: &[u8],
    report_bytes: &[u8],
) -> HostedProviderResult<(String, String)> {
    let report_maximum = usize::try_from(profile.limits.report_max_bytes).map_err(|_| {
        HostedProviderError::Admission("report bound does not fit this host".to_string())
    })?;
    if report_bytes.len() > report_maximum {
        return Err(HostedProviderError::Admission(
            "provider report exceeds its named byte bound".to_string(),
        ));
    }
    let run_leaf = run_identity.strip_prefix("b3:").ok_or_else(|| {
        HostedProviderError::Admission("run identity prefix is invalid".to_string())
    })?;
    let log_leaf = format!("{run_leaf}.log");
    let report_leaf = format!("{run_leaf}.json");
    let report_directory = profile.report_root.join(&profile.report_namespace);
    for (leaf, bytes) in [
        (log_leaf.as_str(), log_bytes),
        (report_leaf.as_str(), report_bytes),
    ] {
        publish_exact(
            &report_directory,
            leaf,
            bytes,
            report_maximum,
            profile.limits.stage_collision_attempts,
        )?;
    }
    Ok((log_leaf, blake3::hash(report_bytes).to_hex().to_string()))
}

fn run_identity(
    profile: &HostedNixProviderProfile,
    request: &HostedProviderRequest,
) -> HostedProviderResult<String> {
    let mut hasher = blake3::Hasher::new();
    for part in [
        HOSTED_NIX_PROVIDER_SCHEMA.as_bytes(),
        profile.repository.as_bytes(),
        request.revision.as_bytes(),
        profile.wrapper_blake3.as_bytes(),
        profile.report_namespace.as_bytes(),
    ] {
        let part_length = u64::try_from(part.len()).map_err(|_| {
            HostedProviderError::Admission("run identity input does not fit u64".to_string())
        })?;
        hasher.update(&part_length.to_le_bytes());
        hasher.update(part);
    }
    Ok(format!("b3:{}", hasher.finalize().to_hex()))
}

#[cfg(test)]
mod tests;

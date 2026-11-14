//! Opt-in exact artifact previews. No function in this module changes CI evidence.
//! The local consumer uses an existing, content-pinned TryNix engine, never its
//! unrestricted package picker or build path.
//! r[impl kiln.preview.runtime]
pub mod cohort;
pub mod engine;
pub mod nix;
pub mod retention;
pub mod service;

use kiln_core::{
    build::{BuildEffectPlan, BuildEvaluationObservation},
    preview::{PreviewBinding, PreviewDescriptor, admit_preview},
};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::Path};

pub const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;

/// Trusted shell input, not a browser request. The build records must come from
/// the canonical admitted build journal; importing JSON does not establish that
/// provenance. The real review consumer additionally matches canonical CI facts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    pub plan: BuildEffectPlan,
    pub observation: BuildEvaluationObservation,
    pub descriptor: PreviewDescriptor,
}

/// Machine export of typed preview policy plus one exact product selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewSelection {
    pub root_path: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub trust_identity: kiln_core::ContentId,
    pub public_keys: Vec<String>,
    pub limits: kiln_core::preview::PreviewLimits,
}

pub fn enforce_policy(
    descriptor: &PreviewDescriptor,
    policy: &PreviewSelection,
) -> Result<(), String> {
    if descriptor.root_path != policy.root_path
        || descriptor.executable != policy.executable
        || descriptor.arguments != policy.arguments
        || descriptor.trust_identity != policy.trust_identity
        || descriptor.public_keys != policy.public_keys
        || descriptor.limits != policy.limits
    {
        return Err("preview_policy_mismatch: descriptor differs from independently selected operator trust/runtime policy".into());
    }
    Ok(())
}

/// Frozen canonical build facts and the independently selected runtime policy.
#[derive(Debug)]
pub struct PreviewProduction {
    pub plan: BuildEffectPlan,
    pub observation: BuildEvaluationObservation,
    pub current: PreviewBinding,
    pub selection: PreviewSelection,
}

/// Real producer for already-built exact Nix products. It observes only the
/// runtime closure, checks canonical build bindings and signatures, then exports
/// verified NARs transiently to prove the descriptor is locally executable.
pub fn produce(
    input: PreviewProduction,
    engine: &engine::EngineBundle,
    nix: &nix::NativeNix,
    scratch: &Path,
    now: u64,
) -> Result<PreviewRequest, String> {
    let PreviewProduction {
        plan,
        observation,
        current,
        selection,
    } = input;
    let closure = nix.runtime_members(&selection.root_path, scratch)?;
    let descriptor = PreviewDescriptor {
        version: kiln_core::preview::PREVIEW_VERSION,
        binding: current.clone(),
        platform: plan.platform.clone(),
        root_path: selection.root_path,
        executable: selection.executable,
        arguments: selection.arguments,
        closure,
        closure_complete: true,
        engine: engine.capabilities()?,
        trust_identity: selection.trust_identity,
        public_keys: selection.public_keys,
        issued_at: now,
        limits: selection.limits,
    };
    let request = PreviewRequest {
        plan,
        observation,
        descriptor,
    };
    admit(&request, &current, engine, now)?;
    let verified = retention::create(
        scratch,
        "verification",
        request.descriptor.limits.expires_at,
    )?;
    nix.export(&request.descriptor, verified.path())?;
    kiln_core::preview::validate_descriptor(&request.descriptor, service::now()?)
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    Ok(request)
}

pub fn read_request(path: &Path) -> Result<PreviewRequest, String> {
    read_json(path, MAX_REQUEST_BYTES)
}

pub fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    maximum_bytes: u64,
) -> Result<T, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > maximum_bytes {
        return Err("preview_input_exhausted".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > maximum_bytes {
        return Err("preview_input_exhausted".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn admit(
    request: &PreviewRequest,
    current: &PreviewBinding,
    engine: &engine::EngineBundle,
    now: u64,
) -> Result<String, String> {
    let selected = engine.capabilities()?;
    let identity = admit_preview(
        &request.plan,
        &request.observation,
        current,
        &request.descriptor,
        &selected,
        now,
    )
    .map_err(|e| format!("{}: {}", e.code, e.message))?;
    nix::verify_product_path(&request.descriptor)?;
    Ok(identity.as_str().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operator_policy_is_independent_of_descriptor_supplied_keys()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = "/nix/store/00000000000000000000000000000000-cli";
        let id = "a".repeat(64);
        let mut d: PreviewDescriptor = serde_json::from_value(serde_json::json!({
            "version":1,"binding":{"project":"p","repository":"r","revision":"c".repeat(40),"run_id":"run","attempt_id":"attempt","candidate_id":"candidate","build_receipt":id,"product":"out","artifact_identity":id,"store_identity":id},
            "platform":"x86_64-linux","root_path":root,"executable":format!("{root}/bin/cli"),"arguments":[],
            "closure":[{"path":root,"nar_sha256":id,"nar_size":16,"references":[],"signatures":["cache-1:signature"]}],"closure_complete":true,
            "engine":{"protocol":"trynix-startvm-v1","identity":id,"platform":"x86_64-linux","guest_memory_bytes":536870912,"network_access":false,"supports_exact_closure":true,"supports_disposable_session":true},
            "trust_identity":id,"public_keys":["operator-key"],"issued_at":100,
            "limits":{"runtime_bytes":1024,"run_seconds":60,"output_bytes":4096,"expires_at":200,"retain_after_session":false}
        }))?;
        let policy = PreviewSelection {
            root_path: d.root_path.clone(),
            executable: d.executable.clone(),
            arguments: d.arguments.clone(),
            trust_identity: d.trust_identity.clone(),
            public_keys: d.public_keys.clone(),
            limits: d.limits.clone(),
        };
        assert!(enforce_policy(&d, &policy).is_ok());
        d.public_keys = vec!["attacker-key".into()];
        assert!(enforce_policy(&d, &policy).is_err());
        d.public_keys = policy.public_keys.clone();
        d.limits.output_bytes += 1;
        assert!(enforce_policy(&d, &policy).is_err());
        Ok(())
    }
}

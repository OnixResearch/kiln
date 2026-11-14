//! Exact, opt-in runtime previews. Admission has no CI state mutation or provider effects.
use alloc::{collections::BTreeSet, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    build::{
        BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
        admit_build_evaluation, validate_build_effect_plan,
    },
    types::{Alias, AttemptId, CandidateId, ContentId, CoreResult, Diagnostic, RunId},
};

pub const PREVIEW_VERSION: u16 = 1;
pub const MAX_PREVIEW_MEMBERS: usize = 128;
pub const MAX_PREVIEW_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_PREVIEW_LIFETIME_SECONDS: u64 = 3600;
pub const MAX_PREVIEW_RUN_SECONDS: u64 = 180;
pub const MAX_PREVIEW_TEXT: usize = 4096;
pub const MAX_PREVIEW_ARGS: usize = 16;
pub const MAX_PREVIEW_KEYS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewBinding {
    pub project: String,
    pub repository: String,
    /// Exact source revision, never a branch or flake reference.
    pub revision: String,
    pub run_id: RunId,
    pub attempt_id: AttemptId,
    pub candidate_id: CandidateId,
    pub build_receipt: ContentId,
    pub product: Alias,
    pub artifact_identity: ContentId,
    pub store_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeMember {
    pub path: String,
    /// Native NAR SHA-256, lowercase hex, not a Kiln identity.
    pub nar_sha256: String,
    pub nar_size: u64,
    /// Sorted unique direct runtime references, with complete membership below.
    pub references: Vec<String>,
    pub signatures: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewEngine {
    pub protocol: Alias,
    pub identity: ContentId,
    pub platform: Alias,
    pub guest_memory_bytes: u64,
    pub network_access: bool,
    pub supports_exact_closure: bool,
    pub supports_disposable_session: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewLimits {
    pub runtime_bytes: u64,
    pub run_seconds: u64,
    pub output_bytes: u64,
    pub expires_at: u64,
    /// Export is ephemeral; no browser or server persistence is permitted.
    pub retain_after_session: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewDescriptor {
    pub version: u16,
    pub binding: PreviewBinding,
    pub platform: Alias,
    pub root_path: String,
    /// An exact executable under root_path; no shell fragment or PATH lookup.
    pub executable: String,
    pub arguments: Vec<String>,
    pub closure: Vec<RuntimeMember>,
    pub closure_complete: bool,
    pub engine: PreviewEngine,
    /// Identity of operator-admitted cache/tenant/source-trust policy.
    pub trust_identity: ContentId,
    pub public_keys: Vec<String>,
    pub issued_at: u64,
    pub limits: PreviewLimits,
}

fn reject(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, "preview", message)
}
fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_PREVIEW_TEXT && !value.chars().any(char::is_control)
}
#[must_use]
pub fn store_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    name.len() > 33
        && name.len() <= 256
        && name.as_bytes().get(32) == Some(&b'-')
        && name.as_bytes().get(..32).is_some_and(|hash| {
            hash.iter()
                .all(|b| b"0123456789abcdfghijklmnpqrsvwxyz".contains(b))
        })
        && name.as_bytes().get(33..).is_some_and(|suffix| {
            suffix
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b"+-._?=".contains(b))
        })
        && !name.ends_with(".drv")
}

// r[impl kiln.preview.exact]
pub fn validate_descriptor(descriptor: &PreviewDescriptor, now: u64) -> CoreResult<()> {
    let d = descriptor;
    if d.version != PREVIEW_VERSION || !d.closure_complete {
        return Err(reject(
            "preview_partial_closure",
            "unsupported descriptor or incomplete runtime closure",
        ));
    }
    let b = &d.binding;
    if ![&b.project, &b.repository, &b.revision]
        .into_iter()
        .all(|s| text(s))
        || ![
            b.run_id.as_str(),
            b.attempt_id.as_str(),
            b.candidate_id.as_str(),
            b.product.as_str(),
        ]
        .into_iter()
        .all(text)
        || !matches!(b.revision.len(), 40 | 64)
        || !b.revision.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(reject(
            "preview_binding",
            "preview needs an exact bounded source and run binding",
        ));
    }
    for id in [
        &b.build_receipt,
        &b.artifact_identity,
        &b.store_identity,
        &d.engine.identity,
        &d.trust_identity,
    ] {
        id.validate()?;
    }
    if d.platform != d.engine.platform
        || !d.platform.as_str().ends_with("-linux")
        || d.engine.network_access
        || !d.engine.supports_exact_closure
        || !d.engine.supports_disposable_session
        || d.engine.guest_memory_bytes == 0
        || d.engine.guest_memory_bytes > 1024 * 1024 * 1024
        || !text(d.engine.protocol.as_str())
    {
        return Err(reject(
            "preview_engine",
            "runner lacks the exact isolated Linux runtime capabilities",
        ));
    }
    if d.issued_at > now
        || now >= d.limits.expires_at
        || d.limits.expires_at.saturating_sub(d.issued_at) > MAX_PREVIEW_LIFETIME_SECONDS
        || d.limits.runtime_bytes == 0
        || d.limits.runtime_bytes > MAX_PREVIEW_BYTES
        || d.limits.run_seconds == 0
        || d.limits.run_seconds > MAX_PREVIEW_RUN_SECONDS
        || d.limits.output_bytes == 0
        || d.limits.output_bytes > 65536
        || d.limits.retain_after_session
    {
        return Err(reject(
            "preview_expired_or_exhausted",
            "preview expiry, retention or resource bounds are inadmissible",
        ));
    }
    if !store_path(&d.root_path)
        || !d.executable.starts_with(&d.root_path)
        || !d.executable[d.root_path.len()..].starts_with("/bin/")
        || !d.executable[d.root_path.len() + 5..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+-._".contains(&b))
        || d.executable.len() <= d.root_path.len() + 5
        || d.arguments.len() > MAX_PREVIEW_ARGS
        || d.arguments
            .iter()
            .any(|a| a.len() > 256 || a.chars().any(char::is_control))
    {
        return Err(reject(
            "preview_entrypoint",
            "entrypoint must be one exact root executable with bounded literal arguments",
        ));
    }
    if d.public_keys.is_empty()
        || d.public_keys.len() > MAX_PREVIEW_KEYS
        || d.public_keys
            .windows(2)
            .any(|p| matches!(p, [first, second] if first >= second))
        || d.public_keys.iter().any(|k| !text(k))
    {
        return Err(reject(
            "preview_trust",
            "explicit sorted unique trusted public keys are required",
        ));
    }
    validate_runtime_closure(&d.root_path, &d.closure, d.limits.runtime_bytes)
}

/// Shared by admitted CI previews and explicitly non-CI engine qualification.
/// Only exact transitive runtime members may cross the renderer boundary.
pub fn validate_runtime_closure(
    root_path: &str,
    closure: &[RuntimeMember],
    maximum_bytes: u64,
) -> CoreResult<()> {
    if !store_path(root_path)
        || closure.is_empty()
        || closure.len() > MAX_PREVIEW_MEMBERS
        || maximum_bytes == 0
        || maximum_bytes > MAX_PREVIEW_BYTES
        || closure
            .windows(2)
            .any(|p| matches!(p, [first, second] if first.path >= second.path))
    {
        return Err(reject(
            "preview_closure",
            "runtime members must be bounded sorted and unique",
        ));
    }
    let mut bytes = 0u64;
    for m in closure {
        if !store_path(&m.path)
            || m.nar_sha256.len() != 64
            || !m
                .nar_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || m.nar_size == 0
            || m.references.len() > MAX_PREVIEW_MEMBERS
            || m.references
                .windows(2)
                .any(|p| matches!(p, [first, second] if first >= second))
            || m.signatures.is_empty()
            || m.signatures.len() > MAX_PREVIEW_KEYS
            || m.signatures.iter().any(|s| !text(s))
        {
            return Err(reject(
                "preview_member",
                "runtime member has invalid native hash, references or required signatures",
            ));
        }
        bytes = bytes
            .checked_add(m.nar_size)
            .ok_or_else(|| reject("preview_exhausted", "runtime size overflow"))?;
        for reference in &m.references {
            if closure.binary_search_by(|m| m.path.cmp(reference)).is_err() {
                return Err(reject(
                    "preview_missing_member",
                    "runtime closure omits a referenced member",
                ));
            }
        }
    }
    if bytes > maximum_bytes {
        return Err(reject(
            "preview_exhausted",
            "runtime exceeds the admitted payload budget",
        ));
    }
    let mut reached = BTreeSet::new();
    let mut pending = alloc::vec![root_path];
    while let Some(path) = pending.pop() {
        if !reached.insert(path) {
            continue;
        }
        let member = closure
            .binary_search_by(|m| m.path.as_str().cmp(path))
            .ok()
            .and_then(|index| closure.get(index))
            .ok_or_else(|| reject("preview_missing_member", "runtime root is unavailable"))?;
        pending.extend(
            member
                .references
                .iter()
                .map(String::as_str)
                .filter(|p| !reached.contains(p)),
        );
    }
    if reached.len() != closure.len() {
        return Err(reject(
            "preview_build_only_input",
            "closure includes an unreachable build-only or unrelated member",
        ));
    }
    Ok(())
}

/// The caller supplies its authoritative current review/run binding, not values
/// relabelled from the descriptor. NAR/signature truth remains the provider's job.
// r[impl kiln.preview.exact]
pub fn admit_preview(
    plan: &BuildEffectPlan,
    observation: &BuildEvaluationObservation,
    current: &PreviewBinding,
    descriptor: &PreviewDescriptor,
    selected_engine: &PreviewEngine,
    now: u64,
) -> CoreResult<ContentId> {
    validate_build_effect_plan(plan, MAX_PREVIEW_MEMBERS)?;
    admit_build_evaluation(plan, observation)?;
    validate_descriptor(descriptor, now)?;
    let b = &descriptor.binding;
    if b != current
        || &descriptor.engine != selected_engine
        || b.candidate_id != plan.candidate_id
        || b.attempt_id != plan.attempt_id
        || observation.receipt_identity.as_ref() != Some(&b.build_receipt)
        || observation.outcome != BuildEvaluationOutcome::Success
        || descriptor.platform != plan.platform
    {
        return Err(reject(
            "preview_stale_or_mismatch",
            "preview is not the current exact completed build and selected runner",
        ));
    }
    let product = observation
        .products
        .iter()
        .find(|p| p.name == b.product)
        .ok_or_else(|| {
            reject(
                "preview_product",
                "required selected build product is missing",
            )
        })?;
    if product.artifact_identity != b.artifact_identity
        || product.store_identity != b.store_identity
    {
        return Err(reject(
            "preview_product",
            "selected artifact differs from admitted build evidence",
        ));
    }
    let root = descriptor
        .closure
        .iter()
        .find(|m| m.path == descriptor.root_path)
        .ok_or_else(|| reject("preview_missing_member", "runtime root is absent"))?;
    let store = product.store.as_ref().ok_or_else(|| {
        reject(
            "preview_store_facts",
            "completed product lacks exact native NAR facts",
        )
    })?;
    if !store.references_complete
        || store.nar_sha256 != root.nar_sha256
        || store.nar_size != root.nar_size
        || store.references != root.references
        || store
            .signatures
            .iter()
            .any(|s| !root.signatures.contains(s))
    {
        return Err(reject(
            "preview_store_facts",
            "runtime root facts do not match admitted complete product facts",
        ));
    }
    descriptor_identity(descriptor)
}

/// BLAKE3 over a versioned, length-framed canonical field sequence. No serialized
/// copy is allocated; collection lengths distinguish nested/empty collections.
pub fn descriptor_identity(d: &PreviewDescriptor) -> CoreResult<ContentId> {
    fn put(h: &mut blake3::Hasher, value: &[u8]) {
        h.update(&(value.len() as u64).to_le_bytes());
        h.update(value);
    }
    fn number(h: &mut blake3::Hasher, value: u64) {
        put(h, &value.to_le_bytes());
    }
    let mut h = blake3::Hasher::new();
    put(&mut h, b"kiln.preview.descriptor.v1");
    number(&mut h, u64::from(d.version));
    let b = &d.binding;
    for value in [
        b.project.as_str(),
        b.repository.as_str(),
        b.revision.as_str(),
        b.run_id.as_str(),
        b.attempt_id.as_str(),
        b.candidate_id.as_str(),
        b.build_receipt.as_str(),
        b.product.as_str(),
        b.artifact_identity.as_str(),
        b.store_identity.as_str(),
        d.platform.as_str(),
        d.root_path.as_str(),
        d.executable.as_str(),
    ] {
        put(&mut h, value.as_bytes());
    }
    number(&mut h, d.arguments.len() as u64);
    for value in &d.arguments {
        put(&mut h, value.as_bytes());
    }
    number(&mut h, d.closure.len() as u64);
    for m in &d.closure {
        put(&mut h, m.path.as_bytes());
        put(&mut h, m.nar_sha256.as_bytes());
        number(&mut h, m.nar_size);
        number(&mut h, m.references.len() as u64);
        for value in &m.references {
            put(&mut h, value.as_bytes());
        }
        number(&mut h, m.signatures.len() as u64);
        for value in &m.signatures {
            put(&mut h, value.as_bytes());
        }
    }
    number(&mut h, u64::from(d.closure_complete));
    for value in [
        d.engine.protocol.as_str(),
        d.engine.identity.as_str(),
        d.engine.platform.as_str(),
    ] {
        put(&mut h, value.as_bytes());
    }
    for value in [
        d.engine.guest_memory_bytes,
        u64::from(d.engine.network_access),
        u64::from(d.engine.supports_exact_closure),
        u64::from(d.engine.supports_disposable_session),
    ] {
        number(&mut h, value);
    }
    put(&mut h, d.trust_identity.as_str().as_bytes());
    number(&mut h, d.public_keys.len() as u64);
    for value in &d.public_keys {
        put(&mut h, value.as_bytes());
    }
    for value in [
        d.issued_at,
        d.limits.runtime_bytes,
        d.limits.run_seconds,
        d.limits.output_bytes,
        d.limits.expires_at,
        u64::from(d.limits.retain_after_session),
    ] {
        number(&mut h, value);
    }
    ContentId::from_blake3_hex(h.finalize().to_hex().as_str())
}

#[cfg(test)]
mod tests;

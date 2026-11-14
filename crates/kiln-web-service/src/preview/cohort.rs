//! Real engine qualification with no invented CI provenance. This payload cannot
//! be exported to the review catalog and has no run/attempt/candidate/receipt.
use super::{PreviewSelection, engine::EngineBundle, nix::NativeNix};
use kiln_core::{
    Alias, ContentId,
    preview::{
        MAX_PREVIEW_ARGS, MAX_PREVIEW_KEYS, MAX_PREVIEW_LIFETIME_SECONDS, MAX_PREVIEW_RUN_SECONDS,
        PreviewEngine, PreviewLimits, RuntimeMember, validate_runtime_closure,
    },
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct EngineCohort {
    pub version: u16,
    pub qualification: &'static str,
    pub platform: Alias,
    pub root_path: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub closure: Vec<RuntimeMember>,
    pub closure_complete: bool,
    pub engine: PreviewEngine,
    pub trust_identity: ContentId,
    pub public_keys: Vec<String>,
    pub issued_at: u64,
    pub limits: PreviewLimits,
}
impl EngineCohort {
    pub fn observe(
        selection: PreviewSelection,
        engine: &EngineBundle,
        nix: &NativeNix,
        scratch: &Path,
        now: u64,
    ) -> Result<Self, String> {
        let limits = &selection.limits;
        if limits.expires_at <= now
            || limits.expires_at - now > MAX_PREVIEW_LIFETIME_SECONDS
            || limits.run_seconds == 0
            || limits.run_seconds > MAX_PREVIEW_RUN_SECONDS
            || limits.output_bytes == 0
            || limits.output_bytes > 65536
            || limits.retain_after_session
        {
            return Err("preview_cohort_expired_or_exhausted".into());
        }
        if !kiln_core::preview::store_path(&selection.root_path)
            || !selection
                .executable
                .starts_with(&format!("{}/bin/", selection.root_path))
            || selection.executable.len() <= selection.root_path.len() + 5
            || !selection.executable[selection.root_path.len() + 5..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-._".contains(&b))
            || selection.arguments.len() > MAX_PREVIEW_ARGS
            || selection
                .arguments
                .iter()
                .any(|a| a.len() > 256 || a.chars().any(char::is_control))
            || selection.public_keys.is_empty()
            || selection.public_keys.len() > MAX_PREVIEW_KEYS
            || selection
                .public_keys
                .windows(2)
                .any(|p| matches!(p, [first, second] if first >= second))
        {
            return Err("preview_cohort_selection".into());
        }
        selection.trust_identity.validate().map_err(|e| e.message)?;
        let closure = nix.runtime_members(&selection.root_path, scratch)?;
        validate_runtime_closure(&selection.root_path, &closure, limits.runtime_bytes)
            .map_err(|e| format!("{}: {}", e.code, e.message))?;
        Ok(Self {
            version: 1,
            qualification: "engine-only; no canonical CI binding",
            platform: Alias::new("x86_64-linux").map_err(|e| e.message)?,
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
        })
    }
    pub fn identity(&self) -> Result<String, String> {
        struct HashWriter(blake3::Hasher);
        impl std::io::Write for HashWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = HashWriter(blake3::Hasher::new());
        writer.0.update(b"kiln.preview.engine-qualification.v1\0");
        serde_json::to_writer(&mut writer, self).map_err(|e| e.to_string())?;
        Ok(writer.0.finalize().to_hex().to_string())
    }
}

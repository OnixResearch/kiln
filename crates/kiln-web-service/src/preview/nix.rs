//! Read-only native Nix verification. Never evaluates, realises, builds, signs,
//! copies or roots a store path. Hash and Ed25519 checks fail closed.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use kiln_core::preview::{PreviewDescriptor, RuntimeMember};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::io::Write;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const NIX32: &[u8; 32] = b"0123456789abcdfghijklmnpqrsvwxyz";
const QUERY_BYTES: u64 = 128 * 1024;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Exact Nix base32 encoding (most significant five-bit group first, bits
/// within bytes little endian), not RFC4648 base32.
pub fn nix32(bytes: &[u8]) -> Result<String, String> {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    for n in (0..(bytes.len() * 8).div_ceil(5)).rev() {
        let bit = n * 5;
        let byte = bit / 8;
        let shift = bit % 8;
        let mut value = u16::from(*bytes.get(byte).ok_or("preview_native_hash")?) >> shift;
        if let Some(next) = bytes.get(byte + 1) {
            value |= u16::from(*next) << (8 - shift);
        }
        let symbol = NIX32
            .get((value & 31) as usize)
            .ok_or("preview_native_hash")?;
        out.push(char::from(*symbol));
    }
    Ok(out)
}
fn unhex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("preview_native_hash".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(|| "preview_native_hash".into())
        })
        .collect()
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
fn hash_hex(value: &str) -> Result<String, String> {
    if let Some(b64) = value.strip_prefix("sha256-") {
        return STANDARD
            .decode(b64)
            .map(hex)
            .map_err(|_| "preview_native_hash".into());
    }
    if let Some(hash) = value.strip_prefix("sha256:") {
        if hash.len() == 64 {
            return Ok(hash.into());
        }
        // Decode Nix32 by reversing the bit groups.
        if hash.len() != 52 {
            return Err("preview_native_hash".into());
        }
        let mut bytes = [0u8; 32];
        for (n, ch) in hash.bytes().rev().enumerate() {
            let v = NIX32
                .iter()
                .position(|b| *b == ch)
                .ok_or("preview_native_hash")? as u16;
            let bit = n * 5;
            let byte = bit / 8;
            let shift = bit % 8;
            if byte >= 32 || (byte == 31 && (v << shift) > 255) {
                return Err("preview_native_hash".into());
            }
            *bytes.get_mut(byte).ok_or("preview_native_hash")? |= (v << shift) as u8;
            if let Some(next) = bytes.get_mut(byte + 1) {
                *next |= (v >> (8 - shift)) as u8;
            }
        }
        return Ok(hex(bytes));
    }
    Err("preview_native_hash".into())
}

pub fn verify_signature(member: &RuntimeMember, keys: &[String]) -> Result<(), String> {
    let hash = nix32(&unhex(&member.nar_sha256)?)?;
    let fingerprint = format!(
        "1;{};sha256:{};{};{}",
        member.path,
        hash,
        member.nar_size,
        member.references.join(",")
    );
    for key in keys {
        let (name, encoded) = key.split_once(':').ok_or("preview_public_key")?;
        let bytes: [u8; 32] = STANDARD
            .decode(encoded)
            .map_err(|_| "preview_public_key")?
            .try_into()
            .map_err(|_| "preview_public_key")?;
        let verifying = VerifyingKey::from_bytes(&bytes).map_err(|_| "preview_public_key")?;
        for sig in &member.signatures {
            let Some((signer, encoded)) = sig.split_once(':') else {
                continue;
            };
            if signer != name {
                continue;
            }
            let Ok(bytes) = STANDARD.decode(encoded) else {
                continue;
            };
            let Ok(signature) = Signature::from_slice(&bytes) else {
                continue;
            };
            if verifying
                .verify_strict(fingerprint.as_bytes(), &signature)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
    Err(format!("preview_signature_mismatch: {}", member.path))
}

/// Adapter-specific identity scheme already used by kiln-adapter-nix. This is
/// deliberately outside core; a different build provider must implement its own
/// exact path association instead of treating an artifact digest as permission.
pub fn verify_product_path(d: &PreviewDescriptor) -> Result<(), String> {
    fn identity(domain: &str, fields: &[&[u8]]) -> String {
        let mut h = blake3::Hasher::new();
        for field in std::iter::once(domain.as_bytes()).chain(fields.iter().copied()) {
            h.update(&(field.len() as u64).to_le_bytes());
            h.update(field);
        }
        h.finalize().to_hex().to_string()
    }
    let path = d.root_path.as_bytes();
    if identity(
        "kiln.nix_build.artifact-reference.v1",
        &[d.binding.product.as_str().as_bytes(), path],
    ) != d.binding.artifact_identity.as_str()
        || identity("kiln.nix_build.store-object-reference.v1", &[path])
            != d.binding.store_identity.as_str()
    {
        return Err("preview_product_path_mismatch: native Nix product identity does not bind this exact store path".into());
    }
    Ok(())
}

#[derive(Debug)]
pub struct NativeNix {
    pub nix: PathBuf,
    pub nix_store: PathBuf,
    store_uri: String,
    physical_store: PathBuf,
}
impl NativeNix {
    pub fn new(nix: PathBuf, nix_store: PathBuf) -> Result<Self, String> {
        if !nix.is_absolute() || !nix_store.is_absolute() || !nix.is_file() || !nix_store.is_file()
        {
            return Err("preview_nix_unavailable".into());
        }
        Ok(Self {
            nix,
            nix_store,
            store_uri: "local?read-only=true".into(),
            physical_store: PathBuf::from("/nix/store"),
        })
    }
    /// Read an operator-owned store populated earlier by an exact signed copy.
    /// This adapter never initializes, copies into or mutates that store.
    pub fn with_store_root(mut self, root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|_| "preview_store_unavailable")?;
        let text = root.to_str().ok_or("preview_store_path")?;
        if !root.is_absolute()
            || text.contains(['?', '&', '#'])
            || text.chars().any(char::is_control)
        {
            return Err("preview_store_path".into());
        }
        self.store_uri = format!("local?root={text}&read-only=true");
        self.physical_store = root.join("nix/store");
        Ok(self)
    }
    fn run(&self, program: &Path, args: &[&str], output: &Path, limit: u64) -> Result<(), String> {
        let file = File::create(output).map_err(|e| e.to_string())?;
        let mut child=Command::new(program).args(args).env_clear().env("NIX_CONFIG","experimental-features = nix-command read-only-local-store\nsubstituters =\nextra-substituters =\nmin-free = 0\n")
            .stdin(Stdio::null()).stdout(Stdio::from(file)).stderr(Stdio::null()).spawn().map_err(|_| "preview_nix_unavailable")?;
        let start = Instant::now();
        loop {
            let too_big = fs::metadata(output)
                .map(|m| m.len() > limit)
                .unwrap_or(true);
            if too_big || start.elapsed() > PROCESS_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if too_big {
                    "preview_exhausted"
                } else {
                    "preview_verification_timeout"
                }
                .into());
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    return if status.success() {
                        Ok(())
                    } else {
                        Err("preview_cache_member_unavailable_or_invalid".into())
                    };
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("preview_nix_observation_failed".into());
                }
            }
        }
    }
    pub fn runtime_members(
        &self,
        root: &str,
        scratch: &Path,
    ) -> Result<Vec<RuntimeMember>, String> {
        if !kiln_core::preview::store_path(root) {
            return Err("preview_root_path".into());
        }
        let directory =
            super::retention::create(scratch, "runtime-discovery", super::service::now()? + 300)?;
        let query = directory.path().join("closure.json");
        self.run(
            &self.nix,
            &[
                "--extra-experimental-features",
                "nix-command read-only-local-store",
                "path-info",
                "--recursive",
                "--json",
                "--json-format",
                "1",
                "--offline",
                "--store",
                &self.store_uri,
                root,
            ],
            &query,
            super::MAX_REQUEST_BYTES,
        )?;
        let data = fs::read(query).map_err(|e| e.to_string())?;
        if data.len() as u64 > super::MAX_REQUEST_BYTES {
            return Err("preview_exhausted".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&data).map_err(|_| "preview_native_metadata")?;
        let infos: Vec<(&str, &serde_json::Value)> = if let Some(object) = value.as_object() {
            object.iter().map(|(p, v)| (p.as_str(), v)).collect()
        } else if let Some(array) = value.as_array() {
            array
                .iter()
                .map(|v| {
                    v.get("path")
                        .and_then(|p| p.as_str())
                        .map(|p| (p, v))
                        .ok_or("preview_native_metadata")
                })
                .collect::<Result<_, _>>()?
        } else {
            return Err("preview_native_metadata".into());
        };
        if infos.len() > kiln_core::preview::MAX_PREVIEW_MEMBERS {
            return Err("preview_exhausted".into());
        }
        let mut members = Vec::with_capacity(infos.len());
        for (path, info) in infos {
            let strings = |field: &str| -> Result<Vec<String>, String> {
                info.get(field)
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| format!("preview_native_{field}_missing"))?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(String::from)
                            .ok_or_else(|| "preview_native_metadata".into())
                    })
                    .collect()
            };
            members.push(RuntimeMember {
                path: path.into(),
                nar_sha256: hash_hex(
                    info.get("narHash")
                        .and_then(|v| v.as_str())
                        .ok_or("preview_native_metadata")?,
                )?,
                nar_size: info
                    .get("narSize")
                    .and_then(|v| v.as_u64())
                    .ok_or("preview_native_metadata")?,
                references: strings("references")?,
                signatures: strings("signatures")?,
            });
        }
        members.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(members)
    }
    pub fn export(&self, d: &PreviewDescriptor, directory: &Path) -> Result<Vec<PathBuf>, String> {
        self.export_members(&d.closure, &d.public_keys, directory, d.limits.expires_at)
    }
    pub fn export_members(
        &self,
        members: &[RuntimeMember],
        public_keys: &[String],
        directory: &Path,
        expires_at: u64,
    ) -> Result<Vec<PathBuf>, String> {
        let mut out = Vec::with_capacity(members.len());
        let query = directory.join("native-query.json");
        for (i, member) in members.iter().enumerate() {
            if super::service::now()? >= expires_at {
                return Err("preview_export_expired".into());
            }
            verify_signature(member, public_keys)?;
            // --offline and read-only store: missing members cannot be fetched,
            // built or replaced. A renderer sees only verified NARs below.
            self.run(
                &self.nix,
                &[
                    "--extra-experimental-features",
                    "nix-command read-only-local-store",
                    "path-info",
                    "--json",
                    "--json-format",
                    "1",
                    "--offline",
                    "--store",
                    &self.store_uri,
                    &member.path,
                ],
                &query,
                QUERY_BYTES,
            )?;
            let bytes = fs::read(&query).map_err(|e| e.to_string())?;
            if bytes.len() as u64 > QUERY_BYTES {
                return Err("preview_exhausted".into());
            }
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| "preview_native_metadata")?;
            let info = value
                .get(&member.path)
                .or_else(|| value.as_array().and_then(|a| a.first()))
                .ok_or("preview_native_metadata")?;
            let hash = info
                .get("narHash")
                .and_then(|v| v.as_str())
                .ok_or("preview_native_metadata")?;
            let refs: Vec<&str> = info
                .get("references")
                .and_then(|v| v.as_array())
                .ok_or("preview_native_metadata")?
                .iter()
                .map(|v| v.as_str().ok_or("preview_native_metadata"))
                .collect::<Result<_, _>>()?;
            if hash_hex(hash)? != member.nar_sha256
                || info.get("narSize").and_then(|v| v.as_u64()) != Some(member.nar_size)
                || refs
                    != member
                        .references
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
            {
                return Err(format!("preview_native_metadata_mismatch: {}", member.path));
            }
            let target = directory.join(format!("{i}.nar"));
            let physical = self.physical_store.join(
                member
                    .path
                    .strip_prefix("/nix/store/")
                    .ok_or("preview_store_path")?,
            );
            self.run(
                &self.nix_store,
                &["--dump", physical.to_str().ok_or("preview_store_path")?],
                &target,
                member.nar_size,
            )?;
            verify_nar(&target, member)?;
            out.push(target);
        }
        fs::remove_file(query).map_err(|e| e.to_string())?;
        Ok(out)
    }
}
pub fn verify_nar(path: &Path, member: &RuntimeMember) -> Result<(), String> {
    let file = File::open(path).map_err(|_| "preview_cache_member_unavailable")?;
    let mut reader = file.take(member.nar_size.saturating_add(1));
    let mut sha = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = reader.read(&mut buffer).map_err(|_| "preview_nar_read")?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > member.nar_size {
            return Err("preview_exhausted".into());
        }
        sha.update(buffer.get(..n).ok_or("preview_nar_read")?);
    }
    if count != member.nar_size || hex(sha.finalize()) != member.nar_sha256 {
        return Err("preview_nar_hash_mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn nix_signature_checks_native_fingerprint_and_rejects_changed_payload() {
        let signing = SigningKey::from_bytes(&[42; 32]);
        let key = format!(
            "preview-1:{}",
            STANDARD.encode(signing.verifying_key().as_bytes())
        );
        let mut m = RuntimeMember {
            path: "/nix/store/00000000000000000000000000000000-hello".into(),
            nar_sha256: "00".repeat(32),
            nar_size: 16,
            references: vec![],
            signatures: vec![],
        };
        assert_eq!(nix32(&[0; 32]), Ok("0".repeat(52)));
        let fingerprint = format!("1;{};sha256:{};16;", m.path, "0".repeat(52));
        m.signatures.push(format!(
            "preview-1:{}",
            STANDARD.encode(signing.sign(fingerprint.as_bytes()).to_bytes())
        ));
        assert!(verify_signature(&m, std::slice::from_ref(&key)).is_ok());
        m.nar_size += 1;
        assert!(verify_signature(&m, std::slice::from_ref(&key)).is_err());
        assert!(verify_signature(&m, &[]).is_err());
    }
    #[test]
    fn nar_hash_and_size_are_both_required() -> Result<(), Box<dyn std::error::Error>> {
        let mut f = tempfile::NamedTempFile::new()?;
        f.write_all(b"payload")?;
        let mut m = RuntimeMember {
            path: "unused".into(),
            nar_sha256: hex(Sha256::digest(b"payload")),
            nar_size: 7,
            references: vec![],
            signatures: vec![],
        };
        assert!(verify_nar(f.path(), &m).is_ok());
        m.nar_sha256 = "00".repeat(32);
        assert!(verify_nar(f.path(), &m).is_err());
        m.nar_size = 6;
        assert!(verify_nar(f.path(), &m).is_err());
        Ok(())
    }
}

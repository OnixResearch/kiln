//! Content-pinning adapter for the already-built TryNix Linux browser engine.
use kiln_core::{Alias, ContentId, preview::PreviewEngine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub const TRYNIX_SOURCE: &str = "7e7e793594cb58efb48279b17b4e15bbc6fbe91b";
pub const ENGINE_PROTOCOL: &str = "trynix-startvm-v1";
pub const ENGINE_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ENGINE_BYTES: u64 = 256 * 1024 * 1024;
const GUEST_FILES: &[&str] = &[
    "bzImage",
    "initramfs.cpio.gz",
    "bios-256k.bin",
    "vgabios-stdvga.bin",
    "kvmvapic.bin",
    "linuxboot_dma.bin",
];
#[path = "engine_pins.rs"]
mod pins;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineManifest {
    pub protocol: String,
    pub source_revision: String,
    pub module_dir: String,
    pub files: Vec<EngineFile>,
}

#[derive(Debug)]
pub struct EngineBundle {
    root: PathBuf,
    pub manifest: EngineManifest,
    pub identity: String,
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
fn relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}
fn digest_file(path: &Path, limit: u64) -> Result<(u64, String), String> {
    let file = File::open(path).map_err(|_| "preview_engine_unavailable")?;
    let mut sha = Sha256::new();
    let mut reader = file.take(limit.saturating_add(1));
    let mut buffer = [0u8; 65536];
    let mut size = 0u64;
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|_| "preview_engine_read")?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > limit {
            return Err("preview_engine_exhausted".into());
        }
        sha.update(buffer.get(..n).ok_or("preview_engine_read")?);
    }
    Ok((size, hex(sha.finalize())))
}

impl EngineBundle {
    pub fn load(root: &Path, manifest: EngineManifest) -> Result<Self, String> {
        if manifest.protocol != ENGINE_PROTOCOL
            || manifest.source_revision != TRYNIX_SOURCE
            || !relative(&manifest.module_dir)
            || manifest.module_dir.contains('/')
            || !(manifest.module_dir == "js" || manifest.module_dir.starts_with("js."))
            || manifest.files.len() != pins::FILES.len()
            || manifest
                .files
                .windows(2)
                .any(|w| matches!(w, [first, second] if first.path >= second.path))
        {
            return Err("preview_engine_unsupported".into());
        }
        let mut bytes = 0u64;
        for (file, (name, sha)) in manifest.files.iter().zip(pins::FILES) {
            let path_matches = if let Some(tail) = name.strip_prefix("js/") {
                file.path
                    .strip_prefix(&manifest.module_dir)
                    .and_then(|s| s.strip_prefix('/'))
                    == Some(tail)
            } else {
                file.path == *name
            };
            if !path_matches || file.sha256 != *sha || !relative(&file.path) || file.bytes == 0 {
                return Err("preview_engine_manifest".into());
            }
            bytes = bytes
                .checked_add(file.bytes)
                .ok_or("preview_engine_exhausted")?;
            if bytes > MAX_ENGINE_BYTES {
                return Err("preview_engine_exhausted".into());
            }
        }
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
        writer.0.update(b"kiln.preview.trynix-engine.v1\0");
        serde_json::to_writer(&mut writer, &manifest).map_err(|e| e.to_string())?;
        let hash = writer.0;
        let bundle = Self {
            root: root
                .canonicalize()
                .map_err(|_| "preview_engine_unavailable")?,
            identity: hash.finalize().to_hex().to_string(),
            manifest,
        };
        for file in &bundle.manifest.files {
            bundle.verify_file(file)?;
        }
        let machine: serde_json::Value =
            serde_json::from_slice(&bundle.read("guest/machine.json")?)
                .map_err(|_| "preview_engine_machine")?;
        // This source cohort is qualified only for this exact existing machine.
        // No -nic user, disk/network backend or changed RAM can be silently added.
        let expected = vec![
            "-nographic",
            "-cpu",
            "Haswell-v4,+rdrand",
            "-device",
            "virtio-rng-pci",
            "-m",
            "{ram}",
            "-accel",
            "tcg,tb-size=500",
            "-L",
            "{pack}/",
            "-nic",
            "none",
            "-kernel",
            "{pack}/bzImage",
            "-initrd",
            "{pack}/initramfs.cpio.gz",
            "-fsdev",
            "local,id=store0,path={share},security_model=none",
            "-device",
            "virtio-9p-pci,fsdev=store0,mount_tag=store0,ioeventfd=off",
            "-append",
            "console=ttyS0 rdinit=/init loglevel=4 mitigations=off",
        ];
        if machine.get("ram").and_then(|v| v.as_str()) != Some("512M")
            || machine.get("args") != Some(&serde_json::json!(expected))
        {
            return Err("preview_engine_machine_mismatch".into());
        }
        Ok(bundle)
    }
    fn file(&self, name: &str) -> Result<&EngineFile, String> {
        self.manifest
            .files
            .binary_search_by(|f| f.path.as_str().cmp(name))
            .ok()
            .and_then(|index| self.manifest.files.get(index))
            .ok_or_else(|| "preview_engine_member_unavailable".into())
    }
    fn verify_file(&self, file: &EngineFile) -> Result<PathBuf, String> {
        let path = self
            .root
            .join(&file.path)
            .canonicalize()
            .map_err(|_| "preview_engine_unavailable")?;
        if !path.starts_with(&self.root) {
            return Err("preview_engine_path_escape".into());
        }
        let (size, sha) = digest_file(&path, file.bytes)?;
        if size != file.bytes || sha != file.sha256 {
            return Err("preview_engine_hash_mismatch".into());
        }
        Ok(path)
    }
    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let file = self.file(name)?;
        let path = self.verify_file(file)?;
        let mut bytes = Vec::with_capacity(file.bytes as usize);
        File::open(path)
            .map_err(|_| "preview_engine_unavailable")?
            .take(file.bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "preview_engine_read")?;
        if bytes.len() as u64 != file.bytes || hex(Sha256::digest(&bytes)) != file.sha256 {
            return Err("preview_engine_hash_mismatch".into());
        }
        Ok(bytes)
    }
    pub fn capabilities(&self) -> Result<PreviewEngine, String> {
        Ok(PreviewEngine {
            protocol: Alias::new(ENGINE_PROTOCOL).map_err(|e| e.message)?,
            identity: ContentId::from_blake3_hex(&self.identity).map_err(|e| e.message)?,
            platform: Alias::new("x86_64-linux").map_err(|e| e.message)?,
            guest_memory_bytes: ENGINE_MEMORY_BYTES,
            network_access: false,
            supports_exact_closure: true,
            supports_disposable_session: true,
        })
    }
    pub fn guest_files(&self) -> BTreeMap<String, String> {
        GUEST_FILES
            .iter()
            .map(|s| ((*s).into(), format!("guest/{s}")))
            .collect()
    }
}

/// Pin an already-built tree. This never builds or downloads. The operator must
/// select the immutable upstream source cohort and retain the emitted manifest.
pub fn pin(root: &Path, module_dir: &str) -> Result<EngineManifest, String> {
    if !relative(module_dir) || module_dir.contains('/') {
        return Err("preview_engine_module_dir".into());
    }
    let mut files = Vec::with_capacity(pins::FILES.len());
    let mut total = 0;
    for (name, expected) in pins::FILES {
        let path = if let Some(tail) = name.strip_prefix("js/") {
            format!("{module_dir}/{tail}")
        } else {
            (*name).into()
        };
        let (bytes, sha256) = digest_file(&root.join(&path), MAX_ENGINE_BYTES - total)?;
        if sha256 != *expected {
            return Err(format!("preview_engine_hash_mismatch: {path}"));
        }
        total += bytes;
        files.push(EngineFile {
            path,
            bytes,
            sha256,
        });
    }
    let manifest = EngineManifest {
        protocol: ENGINE_PROTOCOL.into(),
        source_revision: TRYNIX_SOURCE.into(),
        module_dir: module_dir.into(),
        files,
    };
    EngineBundle::load(root, manifest.clone())?;
    Ok(manifest)
}

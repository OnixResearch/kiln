//! Read-only Nix binary-cache projection.
//!
//! Rendering is an explicit shell operation over supplied paths. It does not
//! serve HTTP, sign with operator keys, substitute or mutate a Nix store.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use kiln_core::build::{BuildEvaluationObservation, BuildEvaluationOutcome};
use kiln_core::types::Diagnostic;
use nix_derivation::{CAHash, HashAlgorithm, NixHash, StoreDir};
use nix_narinfo::{Compression, NarInfo, NarInfoBuilder, NixSignature};
use serde::{Deserialize, Serialize};

use crate::diagnostic;
use crate::nar::write_nar;

pub const CACHE_PROJECTION_SCHEMA: &str = "kiln.nix-cache-projection.v1";
pub const CACHE_INFO_FILE: &str = "nix-cache-info";
pub const MAX_CACHE_PRODUCTS: usize = 1024;
pub const MAX_CACHE_NAR_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const MAX_CACHE_TEXT_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheProjection {
    pub schema: String,
    pub store_dir: String,
    pub products: Vec<CacheProduct>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheProduct {
    pub name: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nar_path: Option<PathBuf>,
    pub nar_sha256: String,
    pub nar_size: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    /// A cache projection requires a complete reference closure.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub references_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deriver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_address: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<String>,
}

impl CacheProjection {
    /// Build a projection from one admitted observation and exact output paths.
    /// Products without complete store facts are rejected rather than
    /// projecting an incomplete cache record.
    pub fn from_observation(
        observation: &BuildEvaluationObservation,
        store_dir: impl Into<String>,
        paths: &std::collections::BTreeMap<String, PathBuf>,
    ) -> Result<Self, Diagnostic> {
        if observation.outcome != BuildEvaluationOutcome::Success {
            return Err(diagnostic(
                "nix_cache_observation",
                "nix.cache.observation",
                "only a successful build observation can project a cache",
            ));
        }
        let mut products = Vec::with_capacity(observation.products.len());
        for product in &observation.products {
            let Some(store) = &product.store else {
                return Err(diagnostic(
                    "nix_cache_store",
                    "nix.cache.observation.products.store",
                    "cache projection requires product store facts",
                ));
            };
            if !store.references_complete {
                return Err(diagnostic(
                    "nix_cache_references",
                    "nix.cache.observation.products.store.references",
                    "cache projection requires a complete reference set",
                ));
            }
            let Some(path) = paths.get(product.name.as_str()) else {
                return Err(diagnostic(
                    "nix_cache_path",
                    "nix.cache.observation.products.path",
                    "cache projection requires an exact path for every product",
                ));
            };
            products.push(CacheProduct {
                name: product.name.as_str().to_owned(),
                path: path.clone(),
                nar_path: None,
                nar_sha256: store.nar_sha256.clone(),
                nar_size: store.nar_size,
                references: store.references.clone(),
                references_complete: true,
                deriver: store.deriver.clone(),
                content_address: store.content_address.clone(),
                signatures: store.signatures.clone(),
            });
        }
        let projection = Self {
            schema: CACHE_PROJECTION_SCHEMA.to_owned(),
            store_dir: store_dir.into(),
            products,
        };
        admit(&projection)?;
        Ok(projection)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheReceipt {
    pub schema: &'static str,
    pub products: usize,
    pub nar_bytes: u64,
    pub narinfo_bytes: u64,
}

pub fn admit(document: &CacheProjection) -> Result<(), Diagnostic> {
    if document.schema != CACHE_PROJECTION_SCHEMA
        || document.store_dir.is_empty()
        || !document.store_dir.starts_with('/')
        || document.store_dir.len() > MAX_CACHE_TEXT_BYTES
        || document.products.is_empty()
        || document.products.len() > MAX_CACHE_PRODUCTS
    {
        return Err(diagnostic(
            "nix_cache_header",
            "nix.cache",
            "cache projection schema, store directory or product count is invalid",
        ));
    }
    let store_dir = StoreDir::new(document.store_dir.clone()).map_err(|_| {
        diagnostic(
            "nix_cache_store",
            "nix.cache.store_dir",
            "cache store directory is not an absolute canonical path",
        )
    })?;
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for product in &document.products {
        if product.name.is_empty()
            || product.name.len() > MAX_CACHE_TEXT_BYTES
            || !names.insert(&product.name)
            || !safe_absolute(&product.path)
            || !paths.insert(&product.path)
            || !sha256(&product.nar_sha256)
            || product.nar_size == 0
            || product.nar_size > MAX_CACHE_NAR_BYTES
            || !product.references_complete
        {
            return Err(diagnostic(
                "nix_cache_product",
                "nix.cache.products",
                "cache product name, path, NAR fact or uniqueness is invalid",
            ));
        }
        let mut previous: Option<&str> = None;
        for reference in &product.references {
            if !safe_absolute(Path::new(reference))
                || previous.is_some_and(|previous| previous >= reference.as_str())
            {
                return Err(diagnostic(
                    "nix_cache_reference",
                    "nix.cache.products.references",
                    "cache references must be safe absolute paths in sorted unique order",
                ));
            }
            previous = Some(reference);
        }
        for text in product
            .deriver
            .iter()
            .chain(product.content_address.iter())
            .chain(product.signatures.iter())
        {
            if text.is_empty() || text.len() > MAX_CACHE_TEXT_BYTES {
                return Err(diagnostic(
                    "nix_cache_text",
                    "nix.cache.products",
                    "cache text fact is empty or exceeds its named bound",
                ));
            }
        }
        store_dir
            .parse_path(product.path.to_string_lossy().as_bytes())
            .map_err(|_| {
                diagnostic(
                    "nix_cache_path",
                    "nix.cache.products.path",
                    "cache product path is not an absolute Nix store path",
                )
            })?;
    }
    Ok(())
}

pub fn render(document: &CacheProjection, output: &Path) -> Result<CacheReceipt, Diagnostic> {
    admit(document)?;
    if !output.is_absolute() {
        return Err(diagnostic(
            "nix_cache_output",
            "nix.cache.output",
            "cache output directory must be absolute",
        ));
    }
    fs::create_dir_all(output.join("nar")).map_err(|_| {
        diagnostic(
            "nix_cache_output",
            "nix.cache.output",
            "cache output directory could not be created",
        )
    })?;
    let store_dir = StoreDir::new(document.store_dir.clone()).map_err(|_| {
        diagnostic(
            "nix_cache_store",
            "nix.cache.store_dir",
            "cache store directory is not an absolute canonical path",
        )
    })?;
    let mut total_nar = 0_u64;
    let mut total_narinfo = 0_u64;
    for product in &document.products {
        let store_path = store_dir
            .parse_path(product.path.to_string_lossy().as_bytes())
            .map_err(|_| {
                diagnostic(
                    "nix_cache_path",
                    "nix.cache.products.path",
                    "cache product path is not an absolute Nix store path",
                )
            })?;
        let hash_part = nix_derivation::nixbase32::encode(store_path.digest());
        let nar_relative = format!("nar/{hash_part}.nar");
        let nar_output = output.join(&nar_relative);
        let (nar_size, nar_sha256) = match &product.nar_path {
            Some(source) => {
                if !safe_absolute(source) {
                    return Err(diagnostic(
                        "nix_cache_nar_path",
                        "nix.cache.products.nar_path",
                        "supplied NAR path is not absolute",
                    ));
                }
                copy_bounded(source, &nar_output, product.nar_size)?
            }
            None => encode_bounded(&product.path, &nar_output, product.nar_size)?,
        };
        if nar_size != product.nar_size || nar_sha256 != product.nar_sha256 {
            return Err(diagnostic(
                "nix_cache_nar_mismatch",
                "nix.cache.products.nar_sha256",
                "rendered NAR differs from the recorded size or SHA-256",
            ));
        }
        let references = product
            .references
            .iter()
            .map(|reference| {
                store_dir.parse_path(reference.as_bytes()).map_err(|_| {
                    diagnostic(
                        "nix_cache_reference",
                        "nix.cache.products.references",
                        "cache reference is not an absolute Nix store path",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut builder = NarInfoBuilder::new_in(
            store_dir.clone(),
            store_path,
            nar_relative,
            NixHash::from_algorithm_and_digest(
                HashAlgorithm::Sha256,
                &hex_bytes(&product.nar_sha256).ok_or_else(|| {
                    diagnostic(
                        "nix_cache_nar_hash",
                        "nix.cache.products.nar_sha256",
                        "cache NAR SHA-256 is not 64 lowercase hexadecimal characters",
                    )
                })?,
            )
            .map_err(|_| {
                diagnostic(
                    "nix_cache_nar_hash",
                    "nix.cache.products.nar_sha256",
                    "cache NAR SHA-256 is invalid",
                )
            })?,
            product.nar_size,
        )
        .compression(Compression::None)
        .references(references);
        if let Some(deriver) = &product.deriver {
            builder = builder.deriver(Some(store_dir.parse_path(deriver.as_bytes()).map_err(
                |_| {
                    diagnostic(
                        "nix_cache_deriver",
                        "nix.cache.products.deriver",
                        "cache deriver is not an absolute Nix store path",
                    )
                },
            )?));
        }
        if let Some(content_address) = &product.content_address {
            builder =
                builder.content_address(Some(CAHash::parse(content_address).map_err(|_| {
                    diagnostic(
                        "nix_cache_content_address",
                        "nix.cache.products.content_address",
                        "cache content address is malformed",
                    )
                })?));
        }
        let signatures = product
            .signatures
            .iter()
            .map(|signature| {
                signature
                    .split_once(':')
                    .filter(|(name, encoded)| !name.is_empty() && !encoded.is_empty())
                    .map(|(name, encoded)| NixSignature {
                        key_name: name.to_owned(),
                        encoded: encoded.to_owned(),
                    })
                    .ok_or_else(|| {
                        diagnostic(
                            "nix_cache_signature",
                            "nix.cache.products.signatures",
                            "cache signature is not NAME:BASE64",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let info: NarInfo = builder.signatures(signatures).build().map_err(|_| {
            diagnostic(
                "nix_cache_narinfo",
                "nix.cache.products",
                "narinfo record could not be constructed",
            )
        })?;
        let bytes = info.to_canonical_bytes();
        if bytes.len() > nix_narinfo::MAX_NARINFO_SIZE {
            return Err(diagnostic(
                "nix_cache_narinfo_bound",
                "nix.cache.narinfo",
                "narinfo record exceeds its named bound",
            ));
        }
        fs::write(output.join(format!("{hash_part}.narinfo")), &bytes).map_err(|_| {
            diagnostic(
                "nix_cache_write",
                "nix.cache.output",
                "narinfo record could not be written",
            )
        })?;
        total_nar = total_nar.saturating_add(nar_size);
        total_narinfo =
            total_narinfo.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    }
    fs::write(
        output.join(CACHE_INFO_FILE),
        format!(
            "StoreDir: {}\nWantMassQuery: 1\nPriority: 30\n",
            document.store_dir
        ),
    )
    .map_err(|_| {
        diagnostic(
            "nix_cache_write",
            "nix.cache.output",
            "cache info could not be written",
        )
    })?;
    Ok(CacheReceipt {
        schema: "kiln.nix-cache-receipt.v1",
        products: document.products.len(),
        nar_bytes: total_nar,
        narinfo_bytes: total_narinfo,
    })
}

fn copy_bounded(source: &Path, output: &Path, maximum: u64) -> Result<(u64, String), Diagnostic> {
    let mut source_file = File::open(source).map_err(|_| {
        diagnostic(
            "nix_cache_nar_open",
            "nix.cache.products.nar_path",
            "supplied NAR file is unavailable",
        )
    })?;
    let mut target = File::create(output).map_err(|_| {
        diagnostic(
            "nix_cache_nar_write",
            "nix.cache.output",
            "projected NAR file could not be created",
        )
    })?;
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest as _;
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = source_file.read(&mut buffer).map_err(|_| {
            diagnostic(
                "nix_cache_nar_read",
                "nix.cache.products.nar_path",
                "supplied NAR file could not be read",
            )
        })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > maximum {
            return Err(diagnostic(
                "nix_cache_nar_bound",
                "nix.cache.products.nar_size",
                "supplied NAR exceeds the recorded size",
            ));
        }
        let Some(chunk) = buffer.get(..read) else {
            return Err(diagnostic(
                "nix_cache_nar_read",
                "nix.cache.products.nar_path",
                "supplied NAR read exceeded its buffer",
            ));
        };
        hasher.update(chunk);
        target.write_all(chunk).map_err(|_| {
            diagnostic(
                "nix_cache_nar_write",
                "nix.cache.output",
                "projected NAR file could not be written",
            )
        })?;
    }
    Ok((total, hex(&hasher.finalize().into())))
}

fn encode_bounded(path: &Path, output: &Path, maximum: u64) -> Result<(u64, String), Diagnostic> {
    let file = File::create(output).map_err(|_| {
        diagnostic(
            "nix_cache_nar_write",
            "nix.cache.output",
            "projected NAR file could not be created",
        )
    })?;
    let mut hasher = HashingWriter {
        inner: file,
        hasher: sha2::Sha256::new(),
        total: 0,
    };
    let size = write_nar(&mut hasher, path, maximum)?;
    use sha2::Digest as _;
    Ok((size, hex(&hasher.hasher.finalize().into())))
}

struct HashingWriter<W> {
    inner: W,
    hasher: sha2::Sha256,
    total: u64,
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest as _;
        let written = self.inner.write(bytes)?;
        if let Some(chunk) = bytes.get(..written) {
            self.hasher.update(chunk);
        }
        self.total = self
            .total
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

const fn default_true() -> bool {
    true
}

const fn is_true(value: &bool) -> bool {
    *value
}

fn safe_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| match component {
            Component::RootDir => true,
            Component::Normal(part) => !part.is_empty(),
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => false,
        })
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_bytes(value: &str) -> Option<[u8; 32]> {
    if !sha256(value) {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        *bytes.get_mut(index)? = u8::from_str_radix(text, 16).ok()?;
    }
    Some(bytes)
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix_derivation::StorePath;

    // r[verify kiln.nix_store.cache] r[verify kiln.nix_store.cache.scenario.roundtrip]
    #[test]
    fn renders_and_reparses_a_read_only_cache_projection() {
        let output = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let store = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let store_dir = store.path().to_string_lossy().into_owned();
        let basename = StorePath::from_parts([7_u8; 20], "product")
            .unwrap_or_else(|_| unreachable!())
            .to_basename();
        let product_path = store.path().join(&basename);
        fs::create_dir(&product_path).unwrap_or_else(|_| unreachable!());
        fs::write(product_path.join("file"), b"hello").unwrap_or_else(|_| unreachable!());
        let facts = crate::nar::input_facts(&product_path, crate::nar::NarBounds::DEFAULT)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let document = CacheProjection {
            schema: CACHE_PROJECTION_SCHEMA.into(),
            store_dir: store_dir.clone(),
            products: vec![CacheProduct {
                name: "out".into(),
                path: product_path.clone(),
                nar_path: None,
                nar_sha256: facts.nar_sha256.clone(),
                nar_size: facts.nar_size,
                references: Vec::new(),
                references_complete: true,
                deriver: None,
                content_address: None,
                signatures: Vec::new(),
            }],
        };
        admit(&document).unwrap_or_else(|error| unreachable!("{error:?}"));
        let receipt =
            render(&document, output.path()).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(receipt.products, 1);
        let parsed_store = StoreDir::new(store_dir).unwrap_or_else(|_| unreachable!());
        let hash_part =
            StorePath::from_parts([7_u8; 20], "product").unwrap_or_else(|_| unreachable!());
        let encoded = nix_derivation::nixbase32::encode(hash_part.digest());
        let bytes = fs::read(output.path().join(format!("{encoded}.narinfo")))
            .unwrap_or_else(|_| unreachable!());
        let info = nix_narinfo::NarInfo::parse_in(&parsed_store, &bytes)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(info.store_path().name(), "product");
        assert_eq!(info.nar_size(), facts.nar_size);
        assert_eq!(info.url(), format!("nar/{encoded}.nar"));
        assert!(output.path().join(CACHE_INFO_FILE).is_file());
        assert!(output.path().join(format!("nar/{encoded}.nar")).is_file());
    }

    #[test]
    fn rejects_duplicate_products_and_hash_mismatches() {
        let store = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let store_dir = store.path().to_string_lossy().into_owned();
        let basename = StorePath::from_parts([8_u8; 20], "product")
            .unwrap_or_else(|_| unreachable!())
            .to_basename();
        let path = store.path().join(basename);
        fs::create_dir(&path).unwrap_or_else(|_| unreachable!());
        let document = CacheProjection {
            schema: CACHE_PROJECTION_SCHEMA.into(),
            store_dir,
            products: vec![
                CacheProduct {
                    name: "out".into(),
                    path: path.clone(),
                    nar_path: None,
                    nar_sha256: "0".repeat(64),
                    nar_size: 1,
                    references: Vec::new(),
                    references_complete: true,
                    deriver: None,
                    content_address: None,
                    signatures: Vec::new(),
                },
                CacheProduct {
                    name: "out".into(),
                    path: path.clone(),
                    nar_path: None,
                    nar_sha256: "0".repeat(64),
                    nar_size: 1,
                    references: Vec::new(),
                    references_complete: true,
                    deriver: None,
                    content_address: None,
                    signatures: Vec::new(),
                },
            ],
        };
        assert!(admit(&document).is_err());
        let mut repaired = document;
        repaired.products.truncate(1);
        let output = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        assert!(render(&repaired, output.path()).is_err());
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;
    use crate::nar::{NarBounds, input_facts};
    use kiln_core::build::{
        BuildCacheKind, BuildEvaluationObservation, BuildEvaluationOutcome,
        BuildProductObservation, BuildStoreObjectObservation,
    };
    use kiln_core::types::{
        Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
    };
    use nix_derivation::StorePath;
    use std::collections::BTreeMap;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn observation_store_facts_project_to_a_cache_document() {
        let store = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let store_dir = store.path().to_string_lossy().into_owned();
        let basename = StorePath::from_parts([9_u8; 20], "product")
            .unwrap_or_else(|_| unreachable!())
            .to_basename();
        let path = store.path().join(basename);
        fs::create_dir(&path).unwrap_or_else(|_| unreachable!());
        fs::write(path.join("file"), b"facts").unwrap_or_else(|_| unreachable!());
        let facts = input_facts(&path, NarBounds::DEFAULT)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let observation = BuildEvaluationObservation {
            effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
            idempotency_key: content('1'),
            candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
            pipeline_revision: PipelineRevisionId::new("pipeline")
                .unwrap_or_else(|_| unreachable!()),
            plan_identity: content('2'),
            policy_identity: content('3'),
            provider_profile: ProviderProfileId::new("nix-build-v1")
                .unwrap_or_else(|_| unreachable!()),
            platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
            outcome: BuildEvaluationOutcome::Success,
            products: vec![BuildProductObservation {
                name: Alias::new("out").unwrap_or_else(|_| unreachable!()),
                artifact_identity: content('4'),
                store_identity: content('5'),
                store: Some(BuildStoreObjectObservation {
                    nar_sha256: facts.nar_sha256,
                    nar_size: facts.nar_size,
                    references: Vec::new(),
                    references_complete: true,
                    deriver: None,
                    content_address: None,
                    signatures: Vec::new(),
                }),
            }],
            constraints: None,
            inputs: Vec::new(),
            input_closure_identity: None,
            input_closure_complete: true,
            derivation_identity: None,
            builder_identity: content('6'),
            worker_identity: content('7'),
            store_identity: content('8'),
            cache_kind: BuildCacheKind::None,
            cache_source_identity: None,
            log_identities: Vec::new(),
            metrics: Vec::new(),
            rebuild: Vec::new(),
            receipt_identity: Some(content('9')),
        };
        let paths = BTreeMap::from([("out".to_owned(), path)]);
        let projection = CacheProjection::from_observation(&observation, store_dir, &paths)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(projection.products.len(), 1);
        let product = projection
            .products
            .first()
            .unwrap_or_else(|| unreachable!("product absent"));
        assert_eq!(product.name, "out");
        assert!(product.references_complete);
    }
}

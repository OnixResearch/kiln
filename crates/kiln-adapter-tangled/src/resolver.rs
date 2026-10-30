//! Issuer key resolution (shell).
//!
//! `did:web` documents come from `https://<host>/.well-known/did.json` and
//! `did:plc` documents from the configured PLC directory. Parsing and key
//! selection are pure (`auth::atproto_key`). Transport failures are retryable,
//! so a knot redelivers instead of dropping the event.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use kiln_provider_api::PortResult;

use crate::auth::{DocumentError, IssuerKey, atproto_key};
use crate::error::{domain, retryable};
use crate::identity::{valid_plc, web_authority};

/// Cached keys kept at most; the oldest entry is evicted first.
pub const MAXIMUM_CACHED_KEYS: usize = 1024;
const USER_AGENT: &str = "kiln-adapter-tangled";

/// Key resolution port. `now_unix` lets callers bound cache age.
pub trait KeyResolver {
    fn resolve(&mut self, did: &str, now_unix: i64) -> PortResult<IssuerKey>;
}

pub struct HttpDidResolver {
    plc_directory_url: String,
    maximum_document_bytes: usize,
    cache_seconds: i64,
    cache: BTreeMap<String, (IssuerKey, i64)>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for HttpDidResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpDidResolver")
            .field("plc_directory_url", &self.plc_directory_url)
            .field("cached", &self.cache.len())
            .finish_non_exhaustive()
    }
}

impl HttpDidResolver {
    pub fn new(
        plc_directory_url: &str,
        timeout_ms: u64,
        maximum_document_bytes: usize,
        cache_seconds: i64,
    ) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_millis(timeout_ms))
            .redirects(0)
            .user_agent(USER_AGENT)
            .build();
        Self {
            plc_directory_url: plc_directory_url.trim_end_matches('/').to_string(),
            maximum_document_bytes,
            cache_seconds,
            cache: BTreeMap::new(),
            agent,
        }
    }

    fn document_url(&self, did: &str) -> PortResult<String> {
        if let Some(authority) = web_authority(did) {
            return Ok(format!("https://{authority}/.well-known/did.json"));
        }
        if valid_plc(did) {
            return Ok(format!("{}/{did}", self.plc_directory_url));
        }
        Err(domain(
            "tangled_did_method",
            "iss",
            "Issuer DID method cannot be resolved",
        ))
    }

    fn fetch(&self, url: &str) -> PortResult<Vec<u8>> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|_| retryable("tangled_did_fetch", "DID document could not be fetched"))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(self.maximum_document_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| retryable("tangled_did_fetch", "DID document could not be read"))?;
        Ok(bytes)
    }
}

impl KeyResolver for HttpDidResolver {
    // r[impl kiln.tangled.auth]
    fn resolve(&mut self, did: &str, now_unix: i64) -> PortResult<IssuerKey> {
        if let Some((key, fetched)) = self.cache.get(did) {
            if now_unix.saturating_sub(*fetched) < self.cache_seconds {
                return Ok(key.clone());
            }
        }
        let url = self.document_url(did)?;
        let document = self.fetch(&url)?;
        let key =
            atproto_key(did, &document, self.maximum_document_bytes).map_err(
                |error| match error {
                    DocumentError::Malformed | DocumentError::IdentityMismatch => domain(
                        "tangled_did_document",
                        "iss",
                        "DID document is malformed or names another DID",
                    ),
                    DocumentError::MissingKey | DocumentError::UnsupportedKey => domain(
                        "tangled_did_key",
                        "iss",
                        "DID document has no supported #atproto Multikey",
                    ),
                },
            )?;
        if self.cache.len() >= MAXIMUM_CACHED_KEYS {
            if let Some(oldest) = self
                .cache
                .iter()
                .min_by_key(|(_, (_, fetched))| *fetched)
                .map(|(did, _)| did.clone())
            {
                self.cache.remove(&oldest);
            }
        }
        self.cache.insert(did.to_string(), (key.clone(), now_unix));
        Ok(key)
    }
}

/// Fixed keys, for fixtures and operators that pin issuer keys.
#[derive(Clone, Debug, Default)]
pub struct StaticKeyResolver {
    keys: BTreeMap<String, IssuerKey>,
}

impl StaticKeyResolver {
    #[must_use]
    pub fn new(keys: BTreeMap<String, IssuerKey>) -> Self {
        Self { keys }
    }
}

impl KeyResolver for StaticKeyResolver {
    fn resolve(&mut self, did: &str, _now_unix: i64) -> PortResult<IssuerKey> {
        self.keys.get(did).cloned().ok_or_else(|| {
            domain(
                "tangled_did_unknown",
                "iss",
                "No key is known for the issuer",
            )
        })
    }
}

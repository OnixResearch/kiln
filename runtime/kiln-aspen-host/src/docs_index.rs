//! Bounded, advisory cache index over `iroh-docs`.
//!
//! The index is a mutable, eventually consistent document. It is not Kiln
//! canonical state, a cache reservation, or an authority grant. Consumers must
//! still fetch the referenced blob, verify its BLAKE3 identity, and apply the
//! existing cache policy.

use std::path::PathBuf;

use iroh::Endpoint;
use iroh_blobs::api::Store as BlobsStore;
use iroh_docs::api::Doc;
use iroh_docs::api::protocol::{AddrInfoOptions, ShareMode};
use iroh_docs::protocol::Docs;
use iroh_docs::store::DownloadPolicy;
use iroh_docs::{AuthorId, DocTicket, NamespaceId};
use iroh_gossip::Gossip;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheIndexProfile {
    pub store_root: PathBuf,
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
}

impl CacheIndexProfile {
    pub fn admit(&self) -> Result<(), CacheIndexError> {
        if !self.store_root.is_absolute() || self.max_key_bytes == 0 || self.max_value_bytes == 0 {
            return Err(CacheIndexError::Invalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheIndexEntry {
    pub key: Vec<u8>,
    pub author: String,
    pub hash: String,
    pub length: u64,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheIndexError {
    Invalid,
    Bound,
    Missing,
    Transport(String),
}

impl std::fmt::Display for CacheIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "cache index profile is invalid",
            Self::Bound => "cache index entry exceeds its bound",
            Self::Missing => "cache index document is unavailable",
            Self::Transport(message) => message,
        })
    }
}

impl std::error::Error for CacheIndexError {}

/// Advisory cache index backed by one `iroh-docs` document.
pub struct CacheIndex {
    docs: Docs,
    doc: Doc,
    author: AuthorId,
    namespace_id: NamespaceId,
    blobs: BlobsStore,
    max_key_bytes: usize,
    max_value_bytes: usize,
}

impl std::fmt::Debug for CacheIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CacheIndex")
            .field("namespace", &self.namespace_id)
            .field("author", &self.author)
            .finish_non_exhaustive()
    }
}

impl CacheIndex {
    pub async fn open_persistent(
        profile: CacheIndexProfile,
        endpoint: Endpoint,
        blobs: BlobsStore,
        gossip: Gossip,
    ) -> Result<Self, CacheIndexError> {
        profile.admit()?;
        std::fs::create_dir_all(&profile.store_root)
            .map_err(|error| transport("cache index root", error))?;
        let docs = Docs::persistent(profile.store_root.clone())
            .spawn(endpoint, blobs.clone(), gossip)
            .await
            .map_err(|error| transport("cache index open", error))?;
        Self::from_docs(docs, blobs, profile).await
    }

    pub async fn open_memory(
        profile: CacheIndexProfile,
        endpoint: Endpoint,
        blobs: BlobsStore,
        gossip: Gossip,
    ) -> Result<Self, CacheIndexError> {
        profile.admit()?;
        let docs = Docs::memory()
            .spawn(endpoint, blobs.clone(), gossip)
            .await
            .map_err(|error| transport("cache index open", error))?;
        Self::from_docs(docs, blobs, profile).await
    }

    pub async fn open_existing(
        profile: CacheIndexProfile,
        endpoint: Endpoint,
        blobs: BlobsStore,
        gossip: Gossip,
        namespace_id: NamespaceId,
    ) -> Result<Self, CacheIndexError> {
        profile.admit()?;
        let docs = Docs::persistent(profile.store_root.clone())
            .spawn(endpoint, blobs.clone(), gossip)
            .await
            .map_err(|error| transport("cache index open", error))?;
        let author = docs
            .api()
            .author_default()
            .await
            .map_err(|error| transport("cache index author", error))?;
        let doc = docs
            .api()
            .open(namespace_id)
            .await
            .map_err(|error| transport("cache index document", error))?
            .ok_or(CacheIndexError::Missing)?;
        doc.set_download_policy(DownloadPolicy::NothingExcept(Vec::new()))
            .await
            .map_err(|error| transport("cache index download policy", error))?;
        Ok(Self {
            docs,
            doc,
            author,
            namespace_id,
            blobs,
            max_key_bytes: profile.max_key_bytes,
            max_value_bytes: profile.max_value_bytes,
        })
    }

    async fn from_docs(
        docs: Docs,
        blobs: BlobsStore,
        profile: CacheIndexProfile,
    ) -> Result<Self, CacheIndexError> {
        let author = docs
            .api()
            .author_default()
            .await
            .map_err(|error| transport("cache index author", error))?;
        let doc = docs
            .api()
            .create()
            .await
            .map_err(|error| transport("cache index document", error))?;
        doc.set_download_policy(DownloadPolicy::NothingExcept(Vec::new()))
            .await
            .map_err(|error| transport("cache index download policy", error))?;
        let namespace_id = doc.id();
        Ok(Self {
            docs,
            doc,
            author,
            namespace_id,
            blobs,
            max_key_bytes: profile.max_key_bytes,
            max_value_bytes: profile.max_value_bytes,
        })
    }

    pub async fn put(
        &self,
        key: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<CacheIndexEntry, CacheIndexError> {
        let key = key.as_ref();
        let value = value.as_ref();
        if key.is_empty() || key.len() > self.max_key_bytes {
            return Err(CacheIndexError::Bound);
        }
        if value.len() > self.max_value_bytes {
            return Err(CacheIndexError::Bound);
        }
        let hash = self
            .doc
            .set_bytes(self.author, key.to_vec(), value.to_vec())
            .await
            .map_err(|error| transport("cache index put", error))?;
        let entry = self
            .doc
            .get_exact(self.author, key, false)
            .await
            .map_err(|error| transport("cache index readback", error))?
            .ok_or(CacheIndexError::Missing)?;
        Ok(CacheIndexEntry {
            key: entry.key().to_vec(),
            author: entry.author().to_string(),
            hash: hash.to_hex(),
            length: entry.content_len(),
            timestamp: entry.timestamp(),
        })
    }

    pub async fn get(
        &self,
        key: impl AsRef<[u8]>,
    ) -> Result<Option<CacheIndexEntry>, CacheIndexError> {
        let entry = self
            .doc
            .get_exact(self.author, key.as_ref(), false)
            .await
            .map_err(|error| transport("cache index get", error))?;
        Ok(entry.map(|entry| CacheIndexEntry {
            key: entry.key().to_vec(),
            author: entry.author().to_string(),
            hash: entry.content_hash().to_hex(),
            length: entry.content_len(),
            timestamp: entry.timestamp(),
        }))
    }

    /// Fetches the indexed value from `iroh-blobs` and verifies its identity.
    pub async fn get_verified_blob(
        &self,
        key: impl AsRef<[u8]>,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, CacheIndexError> {
        if maximum_bytes == 0 {
            return Err(CacheIndexError::Bound);
        }
        let entry = self.get(key).await?.ok_or(CacheIndexError::Missing)?;
        if entry.length > maximum_bytes as u64 {
            return Err(CacheIndexError::Bound);
        }
        let hash = entry
            .hash
            .parse::<iroh_blobs::Hash>()
            .map_err(|_| CacheIndexError::Invalid)?;
        let bytes = self
            .blobs
            .blobs()
            .get_bytes(hash)
            .await
            .map_err(|error| transport("cache index blob", error))?;
        if u64::try_from(bytes.len()).ok() != Some(entry.length)
            || iroh_blobs::Hash::new(bytes.as_ref()) != hash
        {
            return Err(CacheIndexError::Invalid);
        }
        Ok(bytes.to_vec())
    }

    /// Stops the docs engine and releases its store before reopening.
    pub async fn shutdown(self) {
        iroh::protocol::ProtocolHandler::shutdown(&self.docs).await;
    }

    pub async fn share(&self, write: bool) -> Result<DocTicket, CacheIndexError> {
        let mode = if write {
            ShareMode::Write
        } else {
            ShareMode::Read
        };
        self.doc
            .share(mode, AddrInfoOptions::Addresses)
            .await
            .map_err(|error| transport("cache index share", error))
    }

    #[must_use]
    pub const fn namespace_id(&self) -> NamespaceId {
        self.namespace_id
    }

    #[must_use]
    pub const fn author_id(&self) -> AuthorId {
        self.author
    }

    pub fn docs(&self) -> &Docs {
        &self.docs
    }
}

fn transport(context: &str, error: impl std::fmt::Display) -> CacheIndexError {
    CacheIndexError::Transport(format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::{RelayMode, endpoint::presets};
    use iroh_blobs::store::mem::MemStore;

    async fn endpoint() -> Endpoint {
        let Ok(builder) = Endpoint::builder(presets::Minimal)
            .relay_mode(RelayMode::Disabled)
            .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))
        else {
            unreachable!("cache index endpoint builder");
        };
        let Ok(endpoint) = builder.bind().await else {
            unreachable!("cache index endpoint bind");
        };
        endpoint
    }

    async fn index(directory: &std::path::Path) -> CacheIndex {
        let endpoint = endpoint().await;
        let blobs = MemStore::new();
        let gossip = Gossip::builder().spawn(endpoint.clone());
        let profile = CacheIndexProfile {
            store_root: directory.join("docs"),
            max_key_bytes: 256,
            max_value_bytes: 4096,
        };
        let Ok(index) =
            CacheIndex::open_persistent(profile, endpoint, (*blobs).clone(), gossip).await
        else {
            unreachable!("cache index opens");
        };
        index
    }

    #[tokio::test]
    async fn cache_index_round_trip_and_ticket_are_bounded() {
        let Ok(directory) = tempfile::tempdir() else {
            unreachable!("cache index directory");
        };
        let index = index(directory.path()).await;
        let value = br#"{"blake3":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","length":128}"#;
        let entry = index.put(b"tenant/project/cache-key", value).await;
        let Ok(entry) = entry else {
            unreachable!("cache index put");
        };
        assert_eq!(entry.length, value.len() as u64);
        let loaded = index.get(b"tenant/project/cache-key").await;
        let Ok(Some(loaded)) = loaded else {
            unreachable!("cache index get");
        };
        assert_eq!(loaded, entry);
        let verified = index
            .get_verified_blob(b"tenant/project/cache-key", 4096)
            .await;
        let Ok(verified) = verified else {
            unreachable!("cache index verified blob");
        };
        assert_eq!(verified, value.to_vec());
        let ticket = index.share(true).await;
        let Ok(ticket) = ticket else {
            unreachable!("cache index share");
        };
        assert!(ticket.to_string().starts_with("doc"));
        assert!(!index.namespace_id().to_string().is_empty());
        assert!(!index.author_id().to_string().is_empty());
        assert!(matches!(
            index.put(b"", value).await,
            Err(CacheIndexError::Bound)
        ));
    }

    #[tokio::test]
    async fn cache_index_reopens_persistent_document() {
        let Ok(directory) = tempfile::tempdir() else {
            unreachable!("cache index directory");
        };
        let index = index(directory.path()).await;
        let value = b"persistent cache entry";
        let entry = index.put(b"tenant/project/persist", value).await;
        let Ok(entry) = entry else {
            unreachable!("cache index put");
        };
        let namespace = index.namespace_id();
        index.shutdown().await;

        let endpoint = endpoint().await;
        let blobs = MemStore::new();
        let gossip = Gossip::builder().spawn(endpoint.clone());
        let profile = CacheIndexProfile {
            store_root: directory.path().join("docs"),
            max_key_bytes: 256,
            max_value_bytes: 4096,
        };
        let reopened =
            CacheIndex::open_existing(profile, endpoint, (*blobs).clone(), gossip, namespace).await;
        let Ok(reopened) = reopened else {
            unreachable!("cache index reopens");
        };
        let loaded = reopened.get(b"tenant/project/persist").await;
        let Ok(Some(loaded)) = loaded else {
            unreachable!("cache index reload");
        };
        assert_eq!(loaded, entry);
    }
}

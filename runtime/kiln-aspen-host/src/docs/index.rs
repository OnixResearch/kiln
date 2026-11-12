use iroh::Endpoint;
use iroh_blobs::api::Store;
use iroh_docs::api::Doc;
use iroh_docs::api::protocol::{AddrInfoOptions, ShareMode};
use iroh_docs::protocol::Docs;
use iroh_docs::store::DownloadPolicy;
use iroh_docs::{AuthorId, DocTicket, NamespaceId};
use iroh_gossip::Gossip;

use super::profile::{CacheIndexEntry, CacheIndexError, CacheIndexProfile};

/// Advisory cache index backed by one `iroh-docs` document.
pub struct CacheIndex {
    docs: Docs,
    doc: Doc,
    author: AuthorId,
    namespace_id: NamespaceId,
    blobs: Store,
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
        blobs: Store,
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
        blobs: Store,
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
        blobs: Store,
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
        blobs: Store,
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
        maximum_bytes: u64,
    ) -> Result<Vec<u8>, CacheIndexError> {
        if maximum_bytes == 0 {
            return Err(CacheIndexError::Bound);
        }
        let entry = self.get(key).await?.ok_or(CacheIndexError::Missing)?;
        if entry.length > maximum_bytes {
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

    pub async fn share(&self, is_writable: bool) -> Result<DocTicket, CacheIndexError> {
        let mode = if is_writable {
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
#[path = "index/tests.rs"]
mod tests;

use iroh::{RelayMode, endpoint::presets};
use iroh_blobs::store::mem::MemStore;

use super::*;

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
    let Ok(index) = CacheIndex::open_persistent(profile, endpoint, (*blobs).clone(), gossip).await
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

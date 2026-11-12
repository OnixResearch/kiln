use std::time::Duration;

use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, RelayMode};
use iroh_blobs::BlobsProtocol;
use iroh_blobs::store::mem::MemStore;
use iroh_docs::api::protocol::{AddrInfoOptions, ShareMode};
use iroh_docs::protocol::Docs;
use iroh_docs::store::Query;
use iroh_gossip::Gossip;

async fn endpoint(lookup: &MemoryLookup) -> Endpoint {
    let Ok(builder) = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::Disabled)
        .address_lookup(lookup.clone())
        .alpns(vec![
            iroh_blobs::ALPN.to_vec(),
            iroh_gossip::ALPN.to_vec(),
            iroh_docs::ALPN.to_vec(),
        ])
        .clear_ip_transports()
        .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))
    else {
        unreachable!("docs sync endpoint builder");
    };
    let Ok(endpoint) = builder.bind().await else {
        unreachable!("docs sync endpoint bind");
    };
    endpoint
}

#[tokio::test]
async fn docs_ticket_syncs_an_advisory_entry_between_real_peers() {
    let lookup = MemoryLookup::new();
    let endpoint_a = endpoint(&lookup).await;
    let endpoint_b = endpoint(&lookup).await;
    lookup.add_endpoint_info(endpoint_a.addr());
    lookup.add_endpoint_info(endpoint_b.addr());

    let blobs_a = MemStore::new();
    let blobs_b = MemStore::new();
    let gossip_a = Gossip::builder().spawn(endpoint_a.clone());
    let gossip_b = Gossip::builder().spawn(endpoint_b.clone());

    let Ok(docs_a) = Docs::memory()
        .spawn(endpoint_a.clone(), (*blobs_a).clone(), gossip_a.clone())
        .await
    else {
        unreachable!("docs A");
    };
    let Ok(docs_b) = Docs::memory()
        .spawn(endpoint_b.clone(), (*blobs_b).clone(), gossip_b.clone())
        .await
    else {
        unreachable!("docs B");
    };

    let router_a = Router::builder(endpoint_a.clone())
        .accept(iroh_blobs::ALPN, BlobsProtocol::new(&blobs_a, None))
        .accept(iroh_gossip::ALPN, gossip_a.clone())
        .accept(iroh_docs::ALPN, docs_a.clone())
        .spawn();
    let router_b = Router::builder(endpoint_b.clone())
        .accept(iroh_blobs::ALPN, BlobsProtocol::new(&blobs_b, None))
        .accept(iroh_gossip::ALPN, gossip_b.clone())
        .accept(iroh_docs::ALPN, docs_b.clone())
        .spawn();

    let Ok(author) = docs_a.api().author_default().await else {
        unreachable!("docs A author");
    };
    let Ok(doc_a) = docs_a.api().create().await else {
        unreachable!("docs A create");
    };
    let key = b"tenant/project/docs-sync";
    let value = b"advisory cache entry";
    let Ok(_) = doc_a.set_bytes(author, key.to_vec(), value.to_vec()).await else {
        unreachable!("docs A set");
    };
    let Ok(ticket) = doc_a
        .share(ShareMode::Write, AddrInfoOptions::Addresses)
        .await
    else {
        unreachable!("docs A share");
    };
    let Ok(doc_b) = docs_b.api().import(ticket).await else {
        unreachable!("docs B import");
    };
    let Ok(()) = doc_b.start_sync(vec![endpoint_a.addr()]).await else {
        unreachable!("docs B sync");
    };
    let Ok(()) = doc_a.start_sync(vec![endpoint_b.addr()]).await else {
        unreachable!("docs A sync");
    };

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let found = doc_b.get_one(Query::key_exact(key)).await;
        if let Ok(Some(entry)) = found {
            assert_eq!(entry.content_hash(), iroh_blobs::Hash::new(value));
            assert_eq!(entry.content_len(), value.len() as u64);
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            unreachable!("docs sync did not deliver the entry");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(router_a.shutdown().await.is_ok());
    assert!(router_b.shutdown().await.is_ok());
}

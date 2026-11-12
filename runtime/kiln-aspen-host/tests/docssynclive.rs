#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the live docs sync test keeps concrete iroh owners visible"
)]
#![allow(
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert closed live docs fixtures"
)]

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

const SYNC_ATTEMPTS: usize = 100;
const SYNC_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// One docs peer. The blob store stays alive for as long as the peer does.
struct Peer {
    endpoint: Endpoint,
    _blobs: MemStore,
    docs: Docs,
    router: Router,
}

fn block_on<F: Future>(future: F) -> F::Output {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        unreachable!("docs sync test runtime");
    };
    runtime.block_on(future)
}

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

async fn peer(endpoint: Endpoint) -> Peer {
    let blobs = MemStore::new();
    let gossip = Gossip::builder().spawn(endpoint.clone());
    let Ok(docs) = Docs::memory()
        .spawn(endpoint.clone(), (*blobs).clone(), gossip.clone())
        .await
    else {
        unreachable!("docs engine");
    };
    let router = Router::builder(endpoint.clone())
        .accept(iroh_blobs::ALPN, BlobsProtocol::new(&blobs, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs.clone())
        .spawn();
    Peer {
        endpoint,
        _blobs: blobs,
        docs,
        router,
    }
}

#[test]
fn docs_ticket_syncs_an_advisory_entry_between_real_peers() {
    block_on(async {
        let lookup = MemoryLookup::new();
        let endpoint_a = endpoint(&lookup).await;
        let endpoint_b = endpoint(&lookup).await;
        lookup.add_endpoint_info(endpoint_a.addr());
        lookup.add_endpoint_info(endpoint_b.addr());
        let peer_a = peer(endpoint_a).await;
        let peer_b = peer(endpoint_b).await;

        let Ok(author) = peer_a.docs.api().author_default().await else {
            unreachable!("docs A author");
        };
        let Ok(doc_a) = peer_a.docs.api().create().await else {
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
        let Ok(doc_b) = peer_b.docs.api().import(ticket).await else {
            unreachable!("docs B import");
        };
        let Ok(()) = doc_b.start_sync(vec![peer_a.endpoint.addr()]).await else {
            unreachable!("docs B sync");
        };
        let Ok(()) = doc_a.start_sync(vec![peer_b.endpoint.addr()]).await else {
            unreachable!("docs A sync");
        };

        let mut synced = None;
        for _ in 0..SYNC_ATTEMPTS {
            if let Ok(Some(entry)) = doc_b.get_one(Query::key_exact(key)).await {
                synced = Some(entry);
                break;
            }
            tokio::time::sleep(SYNC_POLL_INTERVAL).await;
        }
        let Some(entry) = synced else {
            unreachable!("docs sync did not deliver the entry");
        };
        assert_eq!(entry.content_hash(), iroh_blobs::Hash::new(value));
        assert_eq!(entry.content_len(), value.len() as u64);

        assert!(peer_a.router.shutdown().await.is_ok());
        assert!(peer_b.router.shutdown().await.is_ok());
    });
}

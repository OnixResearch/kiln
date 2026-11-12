#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the live gossip test keeps concrete iroh and host owners visible"
)]
#![allow(
    tigerstyle::no_panic,
    reason = "test-only unreachable branches assert closed live gossip fixtures"
)]

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, EndpointId, RelayMode};
use iroh_gossip::api::Event;
use iroh_gossip::{Gossip, TopicId};
use kiln_aspen_host::gossip::{
    ContentReference, GossipHint, GossipListener, GossipProfile, HINT_SCHEMA, HintBounds, HintKind,
    now_ms, topic_id,
};

const DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);
const DUPLICATE_WINDOW: Duration = Duration::from_millis(250);
const HINT_LIFETIME_MS: u64 = 5_000;
const SINK_CAPACITY: usize = 16;

fn block_on<F: Future>(future: F) -> F::Output {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        unreachable!("gossip test runtime");
    };
    runtime.block_on(future)
}

async fn live_endpoint(lookup: &MemoryLookup) -> Endpoint {
    let Ok(builder) = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::Disabled)
        .address_lookup(lookup.clone())
        .alpns(vec![iroh_gossip::ALPN.to_vec()])
        .clear_ip_transports()
        .bind_addr((Ipv4Addr::LOCALHOST, 0))
    else {
        unreachable!("gossip endpoint builder");
    };
    let Ok(endpoint) = builder.bind().await else {
        unreachable!("gossip endpoint bind");
    };
    endpoint
}

fn spawn_gossip(endpoint: &Endpoint) -> (Gossip, Router) {
    let gossip = Gossip::builder().spawn(endpoint.clone());
    let router = Router::builder(endpoint.clone())
        .accept(iroh_gossip::ALPN, gossip.clone())
        .spawn();
    (gossip, router)
}

fn bounds() -> HintBounds {
    HintBounds {
        max_message_bytes: 4096,
        max_content_bytes: 1024,
        max_lifetime_ms: 60_000,
    }
}

fn hint(
    kind: HintKind,
    scope: &str,
    sender: EndpointId,
    epoch: u64,
    content: ContentReference,
) -> GossipHint {
    let Ok(now) = now_ms() else {
        unreachable!("host clock read");
    };
    GossipHint {
        schema: HINT_SCHEMA.to_string(),
        kind,
        scope: scope.to_string(),
        sender: sender.to_string(),
        epoch,
        expires_at_ms: now + HINT_LIFETIME_MS,
        content: Some(content),
    }
}

fn listener_profile(scope: &str, bootstrap_peer: EndpointId) -> GossipProfile {
    GossipProfile {
        topic_scope: scope.to_string(),
        bootstrap_peers: vec![bootstrap_peer],
        bind_address: (Ipv4Addr::LOCALHOST, 0).into(),
        relay_urls: Vec::new(),
        max_message_bytes: 4096,
        max_content_bytes: 1024,
        max_lifetime_ms: 60_000,
        max_events: 16,
        max_recent_hints: 16,
        join_timeout_ms: 5_000,
    }
}

async fn first_message<S, E>(events: &mut S) -> Option<Vec<u8>>
where
    S: Stream<Item = Result<Event, E>> + Unpin,
{
    while let Some(event) = events.next().await {
        if let Ok(Event::Received(message)) = event {
            return Some(message.content.to_vec());
        }
    }
    None
}

// r[verify kiln.aspen_runtime.gossip.scenario.live]
#[test]
fn live_iroh_gossip_delivers_a_bounded_hint() {
    block_on(async {
        let lookup = MemoryLookup::new();
        let receiver_endpoint = live_endpoint(&lookup).await;
        let sender_endpoint = live_endpoint(&lookup).await;
        lookup.add_endpoint_info(receiver_endpoint.addr());
        lookup.add_endpoint_info(sender_endpoint.addr());
        let receiver_id = receiver_endpoint.id();
        let sender_id = sender_endpoint.id();
        let (receiver_gossip, receiver_router) = spawn_gossip(&receiver_endpoint);
        let (sender_gossip, sender_router) = spawn_gossip(&sender_endpoint);

        let scope = "kiln.gossip.hint.test";
        let topic = TopicId::from_bytes(topic_id(scope));
        let Ok(receiver_topic) = receiver_gossip.subscribe(topic, vec![sender_id]).await else {
            unreachable!("gossip receiver subscribe");
        };
        let Ok(sender_topic) = sender_gossip
            .subscribe_and_join(topic, vec![receiver_id])
            .await
        else {
            unreachable!("gossip sender join");
        };
        let (sender, _) = sender_topic.split();
        let (_, mut receiver) = receiver_topic.split();
        let Ok(()) = receiver.joined().await else {
            unreachable!("gossip receiver joined");
        };

        let content = ContentReference {
            blake3: "a".repeat(64),
            length: 128,
        };
        let hint = hint(HintKind::CacheAvailable, scope, sender_id, 1, content);
        let Ok(encoded) = hint.encode(&bounds()) else {
            unreachable!("gossip hint encode");
        };
        let Ok(()) = sender.broadcast(encoded.into()).await else {
            unreachable!("gossip broadcast");
        };

        let received = tokio::time::timeout(DELIVERY_TIMEOUT, first_message(&mut receiver)).await;
        let Ok(Some(content)) = received else {
            unreachable!("gossip delivery");
        };
        let Ok(decoded) = GossipHint::decode(&content, &bounds()) else {
            unreachable!("gossip hint decode");
        };
        assert_eq!(decoded, hint);

        assert!(sender_router.shutdown().await.is_ok());
        assert!(receiver_router.shutdown().await.is_ok());
    });
}

#[test]
fn gossip_listener_decodes_and_dedupes_hints() {
    block_on(async {
        let lookup = MemoryLookup::new();
        let sender_endpoint = live_endpoint(&lookup).await;
        lookup.add_endpoint_info(sender_endpoint.addr());
        let sender_id = sender_endpoint.id();
        let scope = "kiln.gossip.listener.test";
        let (sink, mut received) = tokio::sync::mpsc::channel(SINK_CAPACITY);
        let dropped = Arc::new(AtomicUsize::new(0));
        let sink_dropped = Arc::clone(&dropped);
        let Ok(listener) = GossipListener::start(
            listener_profile(scope, sender_id),
            iroh::SecretKey::from_bytes(&[7_u8; 32]),
            lookup.clone(),
            move |hint| {
                if sink.try_send(hint).is_err() {
                    sink_dropped.fetch_add(1, Ordering::Relaxed);
                }
            },
        )
        .await
        else {
            unreachable!("gossip listener start");
        };
        lookup.add_endpoint_info(listener.addr());

        let (sender_gossip, sender_router) = spawn_gossip(&sender_endpoint);
        let topic = TopicId::from_bytes(topic_id(scope));
        let Ok(sender_topic) = sender_gossip
            .subscribe_and_join(topic, vec![listener.id()])
            .await
        else {
            unreachable!("sender joins listener");
        };
        let (sender, _) = sender_topic.split();

        let content = ContentReference {
            blake3: "b".repeat(64),
            length: 64,
        };
        let hint = hint(HintKind::StateChanged, scope, sender_id, 2, content);
        let Ok(encoded) = hint.encode(&bounds()) else {
            unreachable!("listener hint encode");
        };
        assert!(sender.broadcast(encoded.clone().into()).await.is_ok());
        assert!(sender.broadcast(encoded.into()).await.is_ok());

        let first = tokio::time::timeout(DELIVERY_TIMEOUT, received.recv()).await;
        let Ok(Some(decoded)) = first else {
            unreachable!("listener hint delivery");
        };
        assert_eq!(decoded, hint);
        let duplicate = tokio::time::timeout(DUPLICATE_WINDOW, received.recv()).await;
        assert!(!matches!(duplicate, Ok(Some(_))));
        assert_eq!(dropped.load(Ordering::Relaxed), 0);

        assert!(listener.stop().await.is_ok());
        assert!(sender_router.shutdown().await.is_ok());
    });
}

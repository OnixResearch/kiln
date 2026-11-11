use std::net::Ipv4Addr;
use std::time::Duration;

use futures_util::StreamExt;
use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, RelayMode};
use iroh_gossip::api::Event;
use iroh_gossip::{Gossip, TopicId};
use kiln_aspen_host::gossip::{
    ContentReference, GossipHint, GossipListener, GossipProfile, HINT_SCHEMA, HintBounds, HintKind,
    topic_id,
};

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

// r[verify kiln.aspen_runtime.gossip.scenario.live]
#[tokio::test]
async fn live_iroh_gossip_delivers_a_bounded_hint() {
    let lookup = MemoryLookup::new();
    let receiver_endpoint = live_endpoint(&lookup).await;
    let sender_endpoint = live_endpoint(&lookup).await;
    lookup.add_endpoint_info(receiver_endpoint.addr());
    lookup.add_endpoint_info(sender_endpoint.addr());
    let receiver_id = receiver_endpoint.id();
    let sender_id = sender_endpoint.id();

    let receiver_gossip = Gossip::builder().spawn(receiver_endpoint.clone());
    let sender_gossip = Gossip::builder().spawn(sender_endpoint.clone());
    let receiver_router = Router::builder(receiver_endpoint.clone())
        .accept(iroh_gossip::ALPN, receiver_gossip.clone())
        .spawn();
    let sender_router = Router::builder(sender_endpoint.clone())
        .accept(iroh_gossip::ALPN, sender_gossip.clone())
        .spawn();

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

    let bounds = HintBounds {
        max_message_bytes: 4096,
        max_content_bytes: 1024,
        max_lifetime_ms: 60_000,
    };
    let now = kiln_aspen_host::gossip::now_ms();
    let hint = GossipHint {
        schema: HINT_SCHEMA.to_string(),
        kind: HintKind::CacheAvailable,
        scope: scope.to_string(),
        sender: sender_id.to_string(),
        epoch: 1,
        expires_at_ms: now + 5_000,
        content: Some(kiln_aspen_host::gossip::ContentReference {
            blake3: "a".repeat(64),
            length: 128,
        }),
    };
    let Ok(encoded) = hint.encode(&bounds) else {
        unreachable!("gossip hint encode");
    };
    let Ok(()) = sender.broadcast(encoded.into()).await else {
        unreachable!("gossip broadcast");
    };

    let received = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = receiver.next().await {
            if let Ok(Event::Received(message)) = event {
                return Some(message.content);
            }
        }
        None
    })
    .await;
    let Ok(Some(content)) = received else {
        unreachable!("gossip delivery");
    };
    let Ok(decoded) = GossipHint::decode(&content, &bounds) else {
        unreachable!("gossip hint decode");
    };
    assert_eq!(decoded, hint);

    assert!(sender_router.shutdown().await.is_ok());
    assert!(receiver_router.shutdown().await.is_ok());
}

#[tokio::test]
async fn gossip_listener_decodes_and_dedupes_hints() {
    let lookup = MemoryLookup::new();
    let sender_endpoint = live_endpoint(&lookup).await;
    lookup.add_endpoint_info(sender_endpoint.addr());
    let sender_id = sender_endpoint.id();
    let scope = "kiln.gossip.listener.test";
    let profile = GossipProfile {
        topic_scope: scope.to_string(),
        bootstrap_peers: vec![sender_id],
        bind_address: (Ipv4Addr::LOCALHOST, 0).into(),
        relay_urls: Vec::new(),
        max_message_bytes: 4096,
        max_content_bytes: 1024,
        max_lifetime_ms: 60_000,
        max_events: 16,
        max_recent_hints: 16,
        join_timeout_ms: 5_000,
    };
    let (sink, mut received) = tokio::sync::mpsc::unbounded_channel();
    let Ok(listener) = GossipListener::start(
        profile,
        iroh::SecretKey::from_bytes(&[7_u8; 32]),
        lookup.clone(),
        move |hint| {
            let _ = sink.send(hint);
        },
    )
    .await
    else {
        unreachable!("gossip listener start");
    };
    lookup.add_endpoint_info(listener.addr());

    let sender_gossip = Gossip::builder().spawn(sender_endpoint.clone());
    let sender_router = Router::builder(sender_endpoint.clone())
        .accept(iroh_gossip::ALPN, sender_gossip.clone())
        .spawn();
    let topic = TopicId::from_bytes(topic_id(scope));
    let Ok(sender_topic) = sender_gossip
        .subscribe_and_join(topic, vec![listener.id()])
        .await
    else {
        unreachable!("sender joins listener");
    };
    let (sender, _) = sender_topic.split();

    let bounds = HintBounds {
        max_message_bytes: 4096,
        max_content_bytes: 1024,
        max_lifetime_ms: 60_000,
    };
    let now = kiln_aspen_host::gossip::now_ms();
    let hint = GossipHint {
        schema: HINT_SCHEMA.to_string(),
        kind: HintKind::StateChanged,
        scope: scope.to_string(),
        sender: sender_id.to_string(),
        epoch: 2,
        expires_at_ms: now + 5_000,
        content: Some(ContentReference {
            blake3: "b".repeat(64),
            length: 64,
        }),
    };
    let Ok(encoded) = hint.encode(&bounds) else {
        unreachable!("listener hint encode");
    };
    assert!(sender.broadcast(encoded.clone().into()).await.is_ok());
    assert!(sender.broadcast(encoded.into()).await.is_ok());

    let first = tokio::time::timeout(Duration::from_secs(10), received.recv()).await;
    let Ok(Some(decoded)) = first else {
        unreachable!("listener hint delivery");
    };
    assert_eq!(decoded, hint);
    let duplicate = tokio::time::timeout(Duration::from_millis(250), received.recv()).await;
    assert!(duplicate.is_err() || matches!(duplicate, Ok(None)));

    assert!(listener.stop().await.is_ok());
    assert!(sender_router.shutdown().await.is_ok());
}

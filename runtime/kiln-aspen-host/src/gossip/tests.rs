use super::*;

fn bounds() -> HintBounds {
    HintBounds {
        max_message_bytes: 4096,
        max_content_bytes: 1024,
        max_lifetime_ms: 60_000,
    }
}

fn hint(now_ms: u64) -> GossipHint {
    GossipHint {
        schema: HINT_SCHEMA.to_string(),
        kind: HintKind::CacheAvailable,
        scope: "org:acme/project:kiln".to_string(),
        sender: "iroh:peer".to_string(),
        epoch: 7,
        expires_at_ms: now_ms + 5_000,
        content: Some(ContentReference {
            blake3: "a".repeat(64),
            length: 128,
        }),
    }
}

#[test]
fn hint_round_trip_preserves_fields_and_dedupe() {
    let hint = hint(1_000);
    let bytes = hint.encode_at(&bounds(), 1_000);
    assert!(matches!(&bytes, Ok(value) if value.len() <= 4096));
    let Ok(bytes) = bytes else {
        unreachable!("hint encodes");
    };
    let decoded = GossipHint::decode_at(&bytes, &bounds(), 1_000);
    assert!(matches!(&decoded, Ok(value) if value == &hint));
    let Ok(decoded) = decoded else {
        unreachable!("hint decodes");
    };
    assert_eq!(decoded.dedupe_key(), hint.dedupe_key());
}

#[test]
fn expired_and_overlong_hints_are_rejected() {
    let expired = hint(1_000);
    assert!(matches!(
        expired.validate(&bounds(), 20_000),
        Err(GossipError::Expired)
    ));
    let mut overlong = hint(1_000);
    overlong.expires_at_ms = 1_000 + 60_001;
    assert!(matches!(
        overlong.validate(&bounds(), 1_000),
        Err(GossipError::Lifetime)
    ));
}

// r[verify kiln.aspen_runtime.gossip] r[verify kiln.aspen_runtime.gossip.scenario.bounds]
#[test]
fn oversized_message_and_content_are_rejected() {
    let hint = hint(1_000);
    let tight = HintBounds {
        max_message_bytes: 8,
        ..bounds()
    };
    assert!(matches!(
        hint.encode_at(&tight, 1_000),
        Err(GossipError::Oversized)
    ));
    let mut oversized = hint;
    if let Some(content) = &mut oversized.content {
        content.length = 1025;
    }
    assert!(matches!(
        oversized.validate(&bounds(), 1_000),
        Err(GossipError::ContentBound)
    ));
}

#[test]
fn malformed_schema_and_fields_are_rejected() {
    let mut wrong_schema = hint(1_000);
    wrong_schema.schema = "kiln.gossip.hint.v0".to_string();
    assert!(matches!(
        wrong_schema.validate(&bounds(), 1_000),
        Err(GossipError::InvalidField)
    ));
    let mut empty_scope = hint(1_000);
    empty_scope.scope.clear();
    assert!(matches!(
        empty_scope.validate(&bounds(), 1_000),
        Err(GossipError::InvalidField)
    ));
    let mut zero_epoch = hint(1_000);
    zero_epoch.epoch = 0;
    assert!(matches!(
        zero_epoch.validate(&bounds(), 1_000),
        Err(GossipError::InvalidField)
    ));
    assert!(matches!(
        GossipHint::decode_at(b"not json", &bounds(), 1_000),
        Err(GossipError::Malformed)
    ));
}

#[test]
fn topics_are_stable_and_scope_separated() {
    assert_eq!(topic_id("scope-a"), topic_id("scope-a"));
    assert_ne!(topic_id("scope-a"), topic_id("scope-b"));
}

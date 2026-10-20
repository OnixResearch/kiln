//! Positive and negative wire-profile shell tests, including pinned fixture
//! metadata replay and derived mutation fixtures.

use super::*;
use crate::profile::v1::Cbor;
use crate::profile::v1::WireError;

const DELEGATION_FIXTURE_BLAKE3: &str = "23ddd201dd206cebacfce06b181916519f5635aa645ab3c71db08703d8ca9db4";
const INVOCATION_FIXTURE_BLAKE3: &str = "01aeb9aa8f82530fd5bc177acd4401aeacd8d72d3301e3df34b78b54efb8c380";
const PINNED_DELEGATION_CID: &str = "bafyreigyftnzjf4rcu7glp5kfop53vqlopc3zcldauoqdxqlz7t4343gr4";

const DELEGATION_JSON: &str = include_str!("../../../fixtures/1.0.0/delegation.json");
const INVOCATION_JSON: &str = include_str!("../../../fixtures/1.0.0/invocation.json");

fn delegation_fields() -> Vec<(String, Cbor)> {
    vec![
        (core::FIELD_TAG_ANNOTATION.to_owned(), Cbor::Text(core::DELEGATION_TAG.to_owned())),
        (core::FIELD_VERSION.to_owned(), Cbor::Text(core::WIRE_VERSION.to_owned())),
        (core::FIELD_ISSUER.to_owned(), Cbor::Text("did:key:z6Mkissuer".to_owned())),
        (core::FIELD_AUDIENCE.to_owned(), Cbor::Text("did:key:z6Mkaudience".to_owned())),
        (core::FIELD_SUBJECT.to_owned(), Cbor::Text("did:key:z6Mksubject".to_owned())),
        (core::FIELD_COMMAND.to_owned(), Cbor::Text("/account".to_owned())),
        (core::FIELD_EXPIRES.to_owned(), Cbor::UInt(1_700_000_000)),
        (core::FIELD_NONCE.to_owned(), Cbor::Bytes(vec![7; 8])),
    ]
}

fn invocation_fields() -> Vec<(String, Cbor)> {
    vec![
        (core::FIELD_TAG_ANNOTATION.to_owned(), Cbor::Text(core::INVOCATION_TAG.to_owned())),
        (core::FIELD_VERSION.to_owned(), Cbor::Text(core::WIRE_VERSION.to_owned())),
        (core::FIELD_ISSUER.to_owned(), Cbor::Text("did:key:z6Mkissuer".to_owned())),
        (core::FIELD_AUDIENCE.to_owned(), Cbor::Text("did:key:z6Mkaudience".to_owned())),
        (core::FIELD_COMMAND.to_owned(), Cbor::Text("/msg/send".to_owned())),
        (core::FIELD_ARGS.to_owned(), Cbor::Map(vec![("to".to_owned(), Cbor::Text("alice".to_owned()))])),
        (core::FIELD_PROOFS.to_owned(), Cbor::Array(vec![Cbor::Bytes(vec![0x00, 0x71, 0x12, 0x20; 4])])),
    ]
}

fn signing_key(byte: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[byte; 32])
}

// ---- codec positives and negatives ----

#[test]
fn canonical_payload_round_trip_is_deterministic() {
    let fields = delegation_fields();
    let first = canonical_payload_bytes(&fields);
    let second = canonical_payload_bytes(&fields);
    assert_eq!(first, second);
    let decoded = decode_payload_bytes_to_values(&first).expect("round trip should decode");
    assert_eq!(decoded.len(), fields.len());
    for (key, value) in &fields {
        assert!(decoded.iter().any(|(k, v)| k == key && v == value));
    }
}

#[test]
fn unsorted_input_map_is_rejected() {
    let unsorted = vec![("z".to_owned(), Cbor::UInt(1)), ("a".to_owned(), Cbor::UInt(2))];
    let bytes = super::StrictEncoderTestHelper::encode_unsorted_map(unsorted);
    assert!(decode_payload_bytes_to_values(&bytes).is_err());
}

#[test]
fn duplicate_map_key_is_rejected() {
    // Hand-built array(0) then map with a duplicate key using raw bytes.
    let mut bytes = vec![0xA1, 0x62, b'a', b'b', 0x01, 0x62, b'a', b'b', 0x02];
    bytes[0] = 0xA2;
    assert!(decode_payload_bytes_to_values(&bytes).is_err());
}

#[test]
fn indefinite_form_is_rejected() {
    let indefinite_array = vec![0x9F, 0x01, 0x02, 0xFF];
    assert!(decode_payload_bytes_to_values(&indefinite_array).is_err());
}

#[test]
fn non_minimal_integer_is_rejected() {
    // 0x18 0x05 encodes 5 with a non-minimal 24-form.
    let non_minimal = vec![0x18, 0x05];
    assert!(decode_payload_bytes_to_values(&non_minimal).is_err());
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = canonical_payload_bytes(delegation_fields());
    bytes.push(0x00);
    assert!(decode_payload_bytes_to_values(&bytes).is_err());
}

#[test]
fn unsupported_float64_is_rejected() {
    let bytes = vec![0xFB, 0x3F, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    assert!(decode_payload_bytes_to_values(&bytes).is_err());
}

#[test]
fn unsupported_tag_value_is_rejected() {
    // CBOR tag 10 wrapping a text string.
    let bytes = vec![0xDA, 0x00, 0x00, 0x00, 0x0A, 0x63, b'f', b'o', b'o'];
    assert!(decode_payload_bytes_to_values(&bytes).is_err());
}

#[test]
fn cross_kind_and_tag_substitution_fail() {
    let delegation_bytes = canonical_payload_bytes(delegation_fields());
    let envelope = {
        let sig = [0_u8; 64];
        encode_value(&Cbor::Array(vec![Cbor::Bytes(sig.to_vec()), Cbor::Bytes(delegation_bytes)]))
    };
    let result = parse_envelope(&envelope, SpecKind::Invocation);
    assert!(matches!(
        result,
        Err(WireError::WrongKind {
            expected: SpecKind::Invocation,
            actual: SpecKind::Delegation
        })
    ));
    assert!(parse_envelope(&envelope, SpecKind::Delegation).is_ok());
}

#[test]
fn wrong_field_value_type_fails() {
    let mut fields = delegation_fields();
    for (key, value) in &mut fields {
        if key == core::FIELD_ISSUER {
            *value = Cbor::Bytes(vec![1, 2, 3]);
        }
    }
    let bytes = canonical_payload_bytes(fields);
    let envelope = encode_value(&Cbor::Array(vec![Cbor::Bytes(vec![0_u8; 64]), Cbor::Bytes(bytes)]));
    assert!(
        matches!(parse_envelope(&envelope, SpecKind::Delegation), Err(WireError::WrongType { field }) if field == core::FIELD_ISSUER)
    );
}

// ---- CID and multiformats ----

#[test]
fn sha256_matches_the_protocol_constant_vector() {
    let digest = sha256(b"");
    assert_eq!(digest, [
        0xE3, 0xB0, 0xC4, 0x42, 0x98, 0xFC, 0x1C, 0x14, 0x9A, 0xFB, 0xF4, 0xC8, 0x99, 0x6F, 0xB9, 0x24, 0x27, 0xAE,
        0x41, 0xE4, 0x64, 0x9B, 0x93, 0x4C, 0xA4, 0x95, 0x99, 0x1B, 0x78, 0x52, 0xB8, 0x55
    ]);
}

#[test]
fn base58btc_round_trips_and_uses_z_prefix() {
    let payload = b"hello";
    let encoded = base58btc_encode(payload);
    assert!(encoded.starts_with('z'));
    assert_eq!(base58btc_decode(&encoded).expect("decode"), payload);
    assert_eq!(base58btc_decode("z").expect("empty"), b"");
}

#[test]
fn pinned_fixture_cid_validates_as_cidv1_dagcbor_sha256() {
    let bytes = cid_validate_string(PINNED_DELEGATION_CID).expect("pinned cid must validate");
    assert_eq!(bytes[0], 0x01);
    assert_eq!(bytes[1], 0x71);
    assert_eq!(bytes[2], 0x12);
    assert_eq!(bytes[3], 0x20);
}

#[test]
fn invalid_cids_fail_closed() {
    for (cid, expected) in [
        ("bafyreigyftnzjf4rcu7glp5kfop53vqlopc3zcldauoqdxqlz7t4343g", "cid-wrong-digest-len"),
        ("QmYwAPJzv5CZsnAzt8auVZRnVc3Ld5fWvWz7yMvYcwm", "cid-wrong-version"),
        ("bafykbzacedX", "cid-wrong-codec"),
        ("bafy#!zay", "invalid-base58"),
    ] {
        assert!(
            matches!(cid_validate_string(cid), Err(WireError::UnsupportedTag { tag }) if tag == expected),
            "cid {cid} must fail with {expected}"
        );
    }
    let mut structurally_wrong = cid_v1_of_payload(b"x");
    structurally_wrong[2] = 0x13;
    assert!(matches!(
        cid_validate_string(&base58btc_encode(&structurally_wrong)),
        Err(WireError::UnsupportedTag { tag }) if tag == "cid-wrong-multihash"
    ));
}

#[test]
fn cid_is_deterministic_and_payload_bound() {
    let first = cid_v1_of_payload(b"payload");
    let second = cid_v1_of_payload(b"payload");
    let other = cid_v1_of_payload(b"other");
    assert_eq!(first, second);
    assert_ne!(first, other);
}

// ---- crypto, varsig subset, ports ----

#[test]
fn sign_then_verify_round_trips_delegation() {
    let key = signing_key(9);
    let fields = delegation_fields();
    let envelope = sign_payload(&fields, &key);
    let kind = verify_envelope(&envelope, key.verifying_key().as_bytes()).expect("signature must verify");
    assert_eq!(kind, SpecKind::Delegation);
}

#[test]
fn signature_mutation_and_wrong_key_fail() {
    let key = signing_key(9);
    let envelope = sign_payload(&delegation_fields(), &key);

    let mut mutated = envelope.clone();
    let last = mutated.len() - 1;
    mutated[last] ^= 0x01;
    assert!(matches!(
        verify_envelope(&mutated, key.verifying_key().as_bytes()),
        Err(WireError::UnsupportedTag { tag }) if tag == "signature-invalid"
    ));

    let other = signing_key(11);
    assert!(matches!(
        verify_envelope(&envelope, other.verifying_key().as_bytes()),
        Err(WireError::UnsupportedTag { tag }) if tag == "signature-invalid"
    ));
}

#[test]
fn public_key_length_and_issuer_mismatch_ports_fail_closed() {
    let key = signing_key(9);
    let envelope = sign_payload(&delegation_fields(), &key);
    assert!(matches!(
        verify_envelope(&envelope, &[0_u8; 31]),
        Err(WireError::UnsupportedTag { tag }) if tag == "public-key-length"
    ));

    let portable = StaticDidKeyResolver::new(std::collections::BTreeMap::new());
    assert_eq!(
        portable.resolve_did_key("did:key:z6Mkmissing"),
        Err("unresolved DID did:key:z6Mkmissing".to_owned())
    );

    let mut keys = std::collections::BTreeMap::new();
    keys.insert("did:key:z6Mkissuer".to_owned(), key.verifying_key().to_bytes());
    let resolver = StaticDidKeyResolver::new(keys);
    assert_eq!(resolver.resolve_did_key("did:key:z6Mkissuer"), Ok(key.verifying_key().to_bytes()));

    let signer = StaticEd25519Signer(key);
    let signed = signer.ed25519_sign(b"message").expect("sign");
    assert_eq!(signed.len(), 64);
}

// ---- pinned fixture metadata replay and derived mutations ----

#[test]
fn pinned_delegation_fixture_replays_through_profile_api() {
    let fixture: serde_json::Value = serde_json::from_str(DELEGATION_JSON).expect("fixture json");
    assert_eq!(blake3::hash(DELEGATION_JSON.as_bytes()).to_hex().to_string(), DELEGATION_FIXTURE_BLAKE3);
    let envelope = fixture.pointer("/valid/0/envelope").expect("envelope metadata");
    let payload = envelope.get("payload").expect("payload metadata");
    let fields = payload_from_json_metadata(SpecKind::Delegation, payload);
    let bytes = canonical_payload_bytes(&fields);
    let _cid = cid_v1_of_payload(&bytes);
    cid_validate_string(PINNED_DELEGATION_CID).expect("pinned cid structure");

    let rendered = payload_to_json(&fields);
    let rendered_object = rendered.as_object().expect("map");
    assert_eq!(
        rendered_object.get("iss").and_then(serde_json::Value::as_str),
        payload.get("iss").and_then(serde_json::Value::as_str)
    );
    assert_eq!(
        rendered_object.get("aud").and_then(serde_json::Value::as_str),
        payload.get("aud").and_then(serde_json::Value::as_str)
    );
    assert_eq!(
        rendered_object.get("cmd").and_then(serde_json::Value::as_str),
        payload.get("cmd").and_then(serde_json::Value::as_str)
    );
}

#[test]
fn pinned_invocation_fixture_identity_is_stable() {
    assert_eq!(blake3::hash(INVOCATION_JSON.as_bytes()).to_hex().to_string(), INVOCATION_FIXTURE_BLAKE3);
    let fields = invocation_fields();
    let key = signing_key(13);
    let envelope = sign_payload(&fields, &key);
    assert_eq!(verify_envelope(&envelope, key.verifying_key().as_bytes()).expect("verify"), SpecKind::Invocation);
}

#[derive(Clone, Copy, Debug)]
enum MutationFixtureKind {
    Missing(&'static str),
    TagSubstitution,
}

#[derive(Clone, Copy, Debug)]
struct MutationFixture {
    id: &'static str,
    source_blake3: &'static str,
    kind: MutationFixtureKind,
}

const MUTATION_FIXTURES: &[MutationFixture] = &[
    MutationFixture {
        id: "delegation-missing-issuer",
        source_blake3: DELEGATION_FIXTURE_BLAKE3,
        kind: MutationFixtureKind::Missing(core::FIELD_ISSUER),
    },
    MutationFixture {
        id: "delegation-missing-audience",
        source_blake3: DELEGATION_FIXTURE_BLAKE3,
        kind: MutationFixtureKind::Missing(core::FIELD_AUDIENCE),
    },
    MutationFixture {
        id: "delegation-tag-substitution",
        source_blake3: DELEGATION_FIXTURE_BLAKE3,
        kind: MutationFixtureKind::TagSubstitution,
    },
];

#[test]
fn derived_mutation_fixtures_fail_with_stable_reasons() {
    for mutation in MUTATION_FIXTURES {
        let base: Vec<(String, Cbor)> = match mutation.kind {
            MutationFixtureKind::Missing(field) => {
                delegation_fields().into_iter().filter(|(key, _)| key != field).collect()
            }
            MutationFixtureKind::TagSubstitution => delegation_fields()
                .into_iter()
                .map(|(key, value)| {
                    if key == core::FIELD_TAG_ANNOTATION {
                        (key, Cbor::Text(core::INVOCATION_TAG.to_owned()))
                    } else {
                        (key, value)
                    }
                })
                .collect(),
        };
        let bytes = canonical_payload_bytes(base);
        let envelope = encode_value(&Cbor::Array(vec![Cbor::Bytes(vec![0_u8; 64]), Cbor::Bytes(bytes)]));
        let result = parse_envelope(&envelope, SpecKind::Delegation);
        assert!(result.is_err(), "mutation fixture {} must fail", mutation.id);
        match result {
            Err(WireError::MissingField { field }) => match mutation.kind {
                MutationFixtureKind::Missing(expected) => assert_eq!(field, expected, "mutation {}", mutation.id),
                MutationFixtureKind::TagSubstitution => {
                    panic!("mutation {} must fail with WrongKind, got MissingField", mutation.id)
                }
            },
            Err(WireError::WrongKind { expected, actual }) => {
                assert!(matches!(mutation.kind, MutationFixtureKind::TagSubstitution), "mutation {}", mutation.id);
                assert_eq!(expected, SpecKind::Delegation, "mutation {}", mutation.id);
                assert_eq!(actual, SpecKind::Invocation, "mutation {}", mutation.id);
            }
            other => panic!("mutation {} failed with unexpected error {other:?}", mutation.id),
        }
        assert_eq!(mutation.source_blake3, DELEGATION_FIXTURE_BLAKE3, "mutation {}", mutation.id);
    }
}

/// Test helper exposing the strict encoder for non-canonical inputs.
pub struct StrictEncoderTestHelper;

impl StrictEncoderTestHelper {
    pub fn encode_unsorted_map(items: Vec<(String, Cbor)>) -> Vec<u8> {
        let mut output = super::encode_head(5, items.len());
        for (key, value) in items {
            output.extend(super::encode_text(&key));
            output.extend(super::encode_value(&value));
        }
        output
    }
}

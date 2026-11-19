use k256::schnorr::Signature;
use k256::schnorr::SigningKey;
use serde_json::json;

use crate::model::HEX_CHARS_PER_BYTE;
use crate::model::PUBLIC_KEY_BYTES;
use crate::model::decode_lower_hex;
use crate::model::encode_lower_hex;
use crate::*;

const BIP340_VECTOR_PUBLIC_KEY: &str = "F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9";
const BIP340_VECTOR_SIGNATURE: &str = "E907831F80848D1069A5371B402410364BDF1C5F8307B0084C55F1CE2DCA821525F66A4A85EA8B71E482A74F382D2CE5EBEEE8FDB2172F477DF4900D310536C0";
const BIP340_INVALID_PUBLIC_KEY: &str = "EEFDEA4CDB677750A420FEE807EACF21EB9898AE79B9768766E4FAA04A2D4A34";
const BIP340_VECTOR_MESSAGE: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const TEST_SECRET_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000003";
const EVENT_TIME: u64 = 1_700_000_000;
const MAX_AGE_SECONDS: u64 = 600;
const MAX_FUTURE_SKEW_SECONDS: u64 = 60;
const STALE_DELTA_SECONDS: u64 = MAX_AGE_SECONDS + 1;
const NIP42_RELAY: &str = "wss://relay.example.com/";
const NIP42_CHALLENGE: &str = "challenge:reviewed";
const NIP98_URL: &str = "https://api.snort.social/api/v1/n5sp/list";
const NIP98_METHOD: &str = "GET";
const APP_DOMAIN: &str = "onix.example.identity";
const APP_AUDIENCE: &str = "service:operator";
const APP_PURPOSE: &str = "authenticate-control-plane";
const APP_CHALLENGE: &str = "challenge:application";
const APP_REQUEST: &str = "request:42";
const APP_EVENT_KIND: u32 = 31_337;

fn tag(name: &str, value: &str) -> Vec<String> {
    vec![name.to_owned(), value.to_owned()]
}

fn signed_event(kind: u32, tags: Vec<Vec<String>>) -> NostrEvent {
    let secret = decode_lower_hex::<PUBLIC_KEY_BYTES>(TEST_SECRET_KEY).expect("test secret must decode");
    let signing_key = SigningKey::from_bytes(&secret).expect("test secret must be valid");
    let public_key = encode_lower_hex(&signing_key.verifying_key().to_bytes());
    let mut event = NostrEvent {
        id: String::new(),
        pubkey: public_key,
        created_at: EVENT_TIME,
        kind,
        tags,
        content: String::new(),
        sig: String::new(),
    };
    let event_id = compute_event_id(&event).expect("test event must serialize");
    let auxiliary_random = [0_u8; PUBLIC_KEY_BYTES];
    let signature: Signature =
        signing_key.sign_raw(event_id.as_bytes(), &auxiliary_random).expect("test event must sign");
    event.id = event_id.to_hex();
    event.sig = encode_lower_hex(signature.to_bytes().as_slice());
    event
}

fn freshness(observed_now: u64) -> FreshnessExpectation {
    FreshnessExpectation {
        observed_now,
        max_age_seconds: MAX_AGE_SECONDS,
        max_future_skew_seconds: MAX_FUTURE_SKEW_SECONDS,
    }
}

fn request<'a>(
    event: &NostrEvent,
    authentication: AuthenticationExpectation<'a>,
    observed_now: u64,
) -> VerificationRequest<'a> {
    let event_id = NostrEventId::parse(&event.id).expect("signed fixture event ID must parse");
    VerificationRequest {
        authentication,
        freshness: freshness(observed_now),
        replay: ReplayObservation::Fresh { event_id },
        boundary: BoundaryClaims::default(),
    }
}

fn nip42_expectation<'a>(audience: &'a str, purpose: &'a str) -> AuthenticationExpectation<'a> {
    AuthenticationExpectation::Nip42Relay {
        relay_url: NIP42_RELAY,
        challenge: NIP42_CHALLENGE,
        expected_audience: audience,
        expected_purpose: purpose,
    }
}

fn application_expectation<'a>(audience: &'a str, purpose: &'a str) -> AuthenticationExpectation<'a> {
    AuthenticationExpectation::ApplicationV1 {
        domain: APP_DOMAIN,
        audience,
        purpose,
        challenge: APP_CHALLENGE,
        request_identity: APP_REQUEST,
        event_kind: APP_EVENT_KIND,
    }
}

// r[verify ucan.nostr_identity_adapter.protocol_verification]
// r[verify ucan.nostr_identity_adapter.final_validation]
#[test]
fn official_bip340_positive_and_negative_vectors_match() {
    let message = decode_lower_hex::<PUBLIC_KEY_BYTES>(BIP340_VECTOR_MESSAGE).expect("vector message must decode");

    assert_eq!(
        verify_bip340_message(
            &BIP340_VECTOR_PUBLIC_KEY.to_ascii_lowercase(),
            &message,
            &BIP340_VECTOR_SIGNATURE.to_ascii_lowercase(),
        ),
        Ok(())
    );
    assert_eq!(
        verify_bip340_message(
            &BIP340_INVALID_PUBLIC_KEY.to_ascii_lowercase(),
            &message,
            &BIP340_VECTOR_SIGNATURE.to_ascii_lowercase(),
        ),
        Err(VerificationIssue::InvalidPublicKeyPoint)
    );
}

#[test]
fn nip01_canonical_identity_and_signature_detect_tampering() {
    let event = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let mut wrong_id = event.clone();
    wrong_id.id = "0".repeat(crate::model::EVENT_ID_BYTES * HEX_CHARS_PER_BYTE);
    let mut bad_signature = event.clone();
    let replacement = if bad_signature.sig.ends_with('0') { '1' } else { '0' };
    bad_signature.sig.pop();
    bad_signature.sig.push(replacement);
    let mut malformed_key = event.clone();
    malformed_key.pubkey = "npub1presentation-only".to_owned();
    let mut uppercase_key = event.clone();
    uppercase_key.pubkey = event.pubkey.to_ascii_uppercase();

    assert!(verify_event(&event).is_ok());
    assert_eq!(verify_event(&wrong_id), Err(VerificationIssue::EventIdMismatch));
    assert_eq!(verify_event(&bad_signature), Err(VerificationIssue::SignatureInvalid));
    assert_eq!(verify_event(&malformed_key), Err(VerificationIssue::InvalidPublicKeyEncoding));
    assert_eq!(verify_event(&uppercase_key), Err(VerificationIssue::InvalidPublicKeyEncoding));
}

// r[verify ucan.nostr_identity_adapter.authentication_context]
#[test]
fn all_closed_authentication_profiles_bind_exact_context() {
    let nip42 = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let nip42_credential =
        verify_credential(&nip42, request(&nip42, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), EVENT_TIME))
            .expect("valid NIP-42 event must verify");

    let nip98 = signed_event(NIP98_EVENT_KIND, vec![tag("u", NIP98_URL), tag("method", NIP98_METHOD)]);
    let nip98_credential = verify_credential(
        &nip98,
        request(
            &nip98,
            AuthenticationExpectation::Nip98Http {
                absolute_url: NIP98_URL,
                method: NIP98_METHOD,
                payload_sha256: None,
                expected_audience: NIP98_URL,
                expected_purpose: NIP98_PURPOSE,
            },
            EVENT_TIME,
        ),
    )
    .expect("valid NIP-98 event must verify");

    let application = signed_event(APP_EVENT_KIND, vec![
        tag("domain", APP_DOMAIN),
        tag("audience", APP_AUDIENCE),
        tag("purpose", APP_PURPOSE),
        tag("challenge", APP_CHALLENGE),
        tag("request", APP_REQUEST),
    ]);
    let application_credential = verify_credential(
        &application,
        request(&application, application_expectation(APP_AUDIENCE, APP_PURPOSE), EVENT_TIME),
    )
    .expect("valid application event must verify");

    assert_eq!(nip42_credential.profile(), AuthenticationProfile::Nip42Relay);
    assert_eq!(nip42_credential.audience(), NIP42_RELAY);
    assert_eq!(nip42_credential.purpose(), NIP42_PURPOSE);
    assert_eq!(nip98_credential.profile(), AuthenticationProfile::Nip98Http);
    assert_eq!(application_credential.profile(), AuthenticationProfile::ApplicationV1);
    assert_eq!(application_credential.audience(), APP_AUDIENCE);
}

#[test]
fn wrong_audience_purpose_staleness_and_replay_fail_closed() {
    let event = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let wrong_audience = request(&event, nip42_expectation("wss://other.example/", NIP42_PURPOSE), EVENT_TIME);
    let wrong_purpose = request(&event, nip42_expectation(NIP42_RELAY, "authorize-admin"), EVENT_TIME);
    let malformed_url = request(
        &event,
        AuthenticationExpectation::Nip42Relay {
            relay_url: "relay.example.com",
            challenge: NIP42_CHALLENGE,
            expected_audience: "relay.example.com",
            expected_purpose: NIP42_PURPOSE,
        },
        EVENT_TIME,
    );
    let stale_now = EVENT_TIME + STALE_DELTA_SECONDS;
    let mut replay = request(&event, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), EVENT_TIME);
    replay.replay = ReplayObservation::Replayed {
        event_id: NostrEventId::parse(&event.id).expect("fixture event ID must parse"),
    };
    let mut mismatched_replay = request(&event, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), EVENT_TIME);
    let unrelated_event_id = "0".repeat(crate::model::EVENT_ID_BYTES * HEX_CHARS_PER_BYTE);
    mismatched_replay.replay = ReplayObservation::Fresh {
        event_id: NostrEventId::parse(&unrelated_event_id).expect("unrelated fixture event ID must parse"),
    };

    assert_eq!(verify_credential(&event, wrong_audience), Err(VerificationIssue::AudienceMismatch));
    assert_eq!(verify_credential(&event, wrong_purpose), Err(VerificationIssue::PurposeMismatch));
    assert_eq!(verify_credential(&event, malformed_url), Err(VerificationIssue::InvalidAuthenticationContext));
    assert_eq!(
        verify_credential(&event, request(&event, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), stale_now,),),
        Err(VerificationIssue::EventStale)
    );
    assert_eq!(verify_credential(&event, replay), Err(VerificationIssue::ReplayDenied));
    assert_eq!(verify_credential(&event, mismatched_replay), Err(VerificationIssue::ReplayDenied));
}

#[test]
fn signed_application_fields_cannot_be_widened_by_expectations() {
    let event = signed_event(APP_EVENT_KIND, vec![
        tag("domain", APP_DOMAIN),
        tag("audience", APP_AUDIENCE),
        tag("purpose", APP_PURPOSE),
        tag("challenge", APP_CHALLENGE),
        tag("request", APP_REQUEST),
    ]);

    assert_eq!(
        verify_credential(&event, request(&event, application_expectation("service:other", APP_PURPOSE), EVENT_TIME,),),
        Err(VerificationIssue::TagMismatch)
    );
    assert_eq!(
        verify_credential(&event, request(&event, application_expectation(APP_AUDIENCE, "authorize-all"), EVENT_TIME,),),
        Err(VerificationIssue::TagMismatch)
    );
}

#[test]
fn application_profile_cannot_reuse_standard_event_kinds() {
    let event = signed_event(NIP42_EVENT_KIND, vec![
        tag("relay", NIP42_RELAY),
        tag("challenge", NIP42_CHALLENGE),
        tag("domain", APP_DOMAIN),
        tag("audience", APP_AUDIENCE),
        tag("purpose", APP_PURPOSE),
        tag("request", APP_REQUEST),
    ]);
    let expectation = AuthenticationExpectation::ApplicationV1 {
        domain: APP_DOMAIN,
        audience: APP_AUDIENCE,
        purpose: APP_PURPOSE,
        challenge: APP_CHALLENGE,
        request_identity: APP_REQUEST,
        event_kind: NIP42_EVENT_KIND,
    };

    assert_eq!(
        verify_credential(&event, request(&event, expectation, EVENT_TIME)),
        Err(VerificationIssue::ProfileKindMismatch)
    );
}

// r[verify ucan.nostr_identity_adapter.credential_boundary]
#[test]
fn credential_is_public_bounded_stable_and_authority_free() {
    let event = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let verification_request = request(&event, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), EVENT_TIME);
    let first = verify_credential(&event, verification_request).expect("valid credential must verify");
    let second =
        verify_credential(&event, verification_request).expect("repeat verification must remain deterministic");
    let event_debug = format!("{event:?}");
    let request_debug = format!("{verification_request:?}");
    let credential_debug = format!("{first:?}");

    assert!(!event_debug.contains(NIP42_CHALLENGE));
    assert!(!event_debug.contains(&event.sig));
    assert!(!request_debug.contains(NIP42_RELAY));
    assert!(!request_debug.contains(NIP42_CHALLENGE));
    assert!(!credential_debug.contains(NIP42_RELAY));
    assert!(!credential_debug.contains(NIP42_PURPOSE));
    assert_eq!(first, second);
    assert_eq!(first.public_key().to_hex(), event.pubkey);
    assert_eq!(first.event_id().to_hex(), event.id);
    assert_eq!(first.freshness(), FreshnessDisposition::Fresh);
    assert_eq!(first.replay(), ReplayDisposition::Fresh);
    assert_ne!(first.request_reference().to_hex(), first.event_id().to_hex());
    assert!(first.non_claims().iter().any(|claim| claim.contains("does not grant")));
}

#[test]
fn metadata_secret_and_authority_promotion_never_construct_credentials() {
    let event = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let boundaries = [
        BoundaryClaims::PromotesNip05Identity,
        BoundaryClaims::PromotesNpubKey,
        BoundaryClaims::ContainsSecretMaterial,
        BoundaryClaims::RequestsSubjectResolution,
        BoundaryClaims::RequestsAuthorization,
    ];
    for boundary in boundaries {
        let mut verification_request = request(&event, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), EVENT_TIME);
        verification_request.boundary = boundary;
        assert_eq!(verify_credential(&event, verification_request), Err(VerificationIssue::BoundaryPromotion));
    }
}

#[test]
fn digest_roles_cannot_be_substituted() {
    let event = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let credential =
        verify_credential(&event, request(&event, nip42_expectation(NIP42_RELAY, NIP42_PURPOSE), EVENT_TIME))
            .expect("valid credential must verify");
    let blake3_as_event = WireDigest {
        role: DigestRole::AdapterBlake3,
        hex: credential.request_reference().to_hex(),
    };
    let sha256_as_adapter = WireDigest {
        role: DigestRole::NostrEventSha256,
        hex: credential.event_id().to_hex(),
    };

    assert_eq!(
        validate_wire_digest(&blake3_as_event, DigestRole::NostrEventSha256),
        Err(VerificationIssue::DigestRoleMismatch)
    );
    assert_eq!(
        validate_wire_digest(&sha256_as_adapter, DigestRole::AdapterBlake3),
        Err(VerificationIssue::DigestRoleMismatch)
    );
}

#[test]
fn closed_json_and_bounds_reject_secret_fields_and_oversized_input() {
    let event = signed_event(NIP42_EVENT_KIND, vec![tag("relay", NIP42_RELAY), tag("challenge", NIP42_CHALLENGE)]);
    let mut value = serde_json::to_value(event).expect("event must encode");
    value
        .as_object_mut()
        .expect("event JSON must be an object")
        .insert("private_key".to_owned(), json!("secret"));
    let secret_bearing = serde_json::to_vec(&value).expect("fixture must encode");
    let oversized = vec![0_u8; MAX_EVENT_INPUT_BYTES + 1];

    assert_eq!(parse_event_json(&secret_bearing), Err(VerificationIssue::MalformedEventJson));
    assert_eq!(parse_event_json(&oversized), Err(VerificationIssue::EventInputTooLarge));
}

// r[verify ucan.nostr_identity_adapter.package_boundary]
#[test]
fn adapter_and_ucan_token_manifests_remain_dependency_isolated() {
    const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");
    const ADAPTER_MANIFEST: &str = include_str!("../Cargo.toml");
    let root_dependencies = manifest_section(ROOT_MANIFEST, "[dependencies]");
    let adapter_dependencies = manifest_section(ADAPTER_MANIFEST, "[dependencies]");

    assert!(!root_dependencies.contains("ucan-nostr"));
    assert!(!adapter_dependencies.contains("ucan ="));
    assert!(!adapter_dependencies.contains("ucan-core"));
}

#[test]
fn issue_codes_are_stable_and_non_secret() {
    assert_eq!(VerificationIssue::SignatureInvalid.code(), "nostr-signature-invalid");
    assert!(!VerificationIssue::BoundaryPromotion.code().contains("secret"));
}

fn manifest_section<'a>(manifest: &'a str, heading: &str) -> &'a str {
    manifest
        .split_once(heading)
        .map(|(_before, after)| after)
        .and_then(|after| after.split("\n[").next())
        .unwrap_or("")
}

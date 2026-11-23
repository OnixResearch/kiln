use k256::schnorr::Signature;
use k256::schnorr::SigningKey;
use serde_json::json;

use crate::model::HEX_CHARS_PER_BYTE;
use crate::model::PUBLIC_KEY_BYTES;
use crate::model::decode_lower_hex;
use crate::model::encode_lower_hex;
use crate::*;

/// BIP-340 verification vectors, copied verbatim from the pinned BIP revision.
const BIP340_VECTORS_SOURCE: &str =
    "https://raw.githubusercontent.com/bitcoin/bips/c38071c8c45a1fc50cecaac0d82d99e3bbd56911/bip-0340/test-vectors.csv";
/// SHA-256 of the source `test-vectors.csv` file.
const BIP340_VECTORS_SHA256: &str = "34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d";

/// Expected adapter result for one published vector.
#[derive(Clone, Copy, Debug)]
enum Bip340Expectation {
    /// The vector verifies.
    Accept,
    /// The vector fails with exactly this typed issue.
    Reject(VerificationIssue),
    /// The vector fails as a signature, but the failing stage depends on the curve library: a
    /// degenerate `r` (zero, one, or not an x coordinate) may be rejected while parsing the
    /// signature or while checking the verification equation.
    RejectSignature,
}

struct Bip340Vector {
    index: usize,
    public_key: &'static str,
    message: &'static str,
    signature: &'static str,
    published: &'static str,
    expected: Bip340Expectation,
    comment: &'static str,
}

/// Every verification row of `BIP340_VECTORS_SOURCE`, in file order. The secret key and
/// auxiliary randomness columns are omitted because verification does not use them.
const BIP340_VECTORS: &[Bip340Vector] = &[
    Bip340Vector {
        index: 0,
        public_key: "F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
        message: "0000000000000000000000000000000000000000000000000000000000000000",
        signature: "E907831F80848D1069A5371B402410364BDF1C5F8307B0084C55F1CE2DCA821525F66A4A85EA8B71E482A74F382D2CE5EBEEE8FDB2172F477DF4900D310536C0",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "",
    },
    Bip340Vector {
        index: 1,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "6896BD60EEAE296DB48A229FF71DFE071BDE413E6D43F917DC8DCF8C78DE33418906D11AC976ABCCB20B091292BFF4EA897EFCB639EA871CFA95F6DE339E4B0A",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "",
    },
    Bip340Vector {
        index: 2,
        public_key: "DD308AFEC5777E13121FA72B9CC1B7CC0139715309B086C960E18FD969774EB8",
        message: "7E2D58D8B3BCDF1ABADEC7829054F90DDA9805AAB56C77333024B9D0A508B75C",
        signature: "5831AAEED7B44BB74E5EAB94BA9D4294C49BCF2A60728D8B4C200F50DD313C1BAB745879A5AD954A72C45A91C3A51D3C7ADEA98D82F8481E0E1E03674A6F3FB7",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "",
    },
    Bip340Vector {
        index: 3,
        public_key: "25D1DFF95105F5253C4022F628A996AD3A0D95FBF21D468A1B33F8C160D8F517",
        message: "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        signature: "7EB0509757E246F19449885651611CB965ECC1A187DD51B64FDA1EDC9637D5EC97582B9CB13DB3933705B32BA982AF5AF25FD78881EBB32771FC5922EFC66EA3",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "test fails if msg is reduced modulo p or n",
    },
    Bip340Vector {
        index: 4,
        public_key: "D69C3509BB99E412E68B0FE8544E72837DFA30746D8BE2AA65975F29D22DC7B9",
        message: "4DF3C3F68FCC83B27E9D42C90431A72499F17875C81A599B566C9889B9696703",
        signature: "00000000000000000000003B78CE563F89A0ED9414F5AA28AD0D96D6795F9C6376AFB1548AF603B3EB45C9F8207DEE1060CB71C04E80F593060B07D28308D7F4",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "",
    },
    Bip340Vector {
        index: 5,
        public_key: "EEFDEA4CDB677750A420FEE807EACF21EB9898AE79B9768766E4FAA04A2D4A34",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "6CFF5C3BA86C69EA4B7376F31A9BCB4F74C1976089B2D9963DA2E5543E17776969E89B4C5564D00349106B8497785DD7D1D713A8AE82B32FA79D5F7FC407D39B",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::InvalidPublicKeyPoint),
        comment: "public key not on the curve",
    },
    Bip340Vector {
        index: 6,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "FFF97BD5755EEEA420453A14355235D382F6472F8568A18B2F057A14602975563CC27944640AC607CD107AE10923D9EF7A73C643E166BE5EBEAFA34B1AC553E2",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::SignatureInvalid),
        comment: "has_even_y(R) is false",
    },
    Bip340Vector {
        index: 7,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "1FA62E331EDBC21C394792D2AB1100A7B432B013DF3F6FF4F99FCB33E0E1515F28890B3EDB6E7189B630448B515CE4F8622A954CFE545735AAEA5134FCCDB2BD",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::SignatureInvalid),
        comment: "negated message",
    },
    Bip340Vector {
        index: 8,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "6CFF5C3BA86C69EA4B7376F31A9BCB4F74C1976089B2D9963DA2E5543E177769961764B3AA9B2FFCB6EF947B6887A226E8D7C93E00C5ED0C1834FF0D0C2E6DA6",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::SignatureInvalid),
        comment: "negated s value",
    },
    Bip340Vector {
        index: 9,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "0000000000000000000000000000000000000000000000000000000000000000123DDA8328AF9C23A94C1FEECFD123BA4FB73476F0D594DCB65C6425BD186051",
        published: "FALSE",
        expected: Bip340Expectation::RejectSignature,
        comment: "sG - eP is infinite. Test fails in single verification if has_even_y(inf) is defined as true and x(inf) as 0",
    },
    Bip340Vector {
        index: 10,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "00000000000000000000000000000000000000000000000000000000000000017615FBAF5AE28864013C099742DEADB4DBA87F11AC6754F93780D5A1837CF197",
        published: "FALSE",
        expected: Bip340Expectation::RejectSignature,
        comment: "sG - eP is infinite. Test fails in single verification if has_even_y(inf) is defined as true and x(inf) as 1",
    },
    Bip340Vector {
        index: 11,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "4A298DACAE57395A15D0795DDBFD1DCB564DA82B0F269BC70A74F8220429BA1D69E89B4C5564D00349106B8497785DD7D1D713A8AE82B32FA79D5F7FC407D39B",
        published: "FALSE",
        expected: Bip340Expectation::RejectSignature,
        comment: "sig[0:32] is not an X coordinate on the curve",
    },
    Bip340Vector {
        index: 12,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F69E89B4C5564D00349106B8497785DD7D1D713A8AE82B32FA79D5F7FC407D39B",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::InvalidSignatureEncoding),
        comment: "sig[0:32] is equal to field size",
    },
    Bip340Vector {
        index: 13,
        public_key: "DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "6CFF5C3BA86C69EA4B7376F31A9BCB4F74C1976089B2D9963DA2E5543E177769FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::InvalidSignatureEncoding),
        comment: "sig[32:64] is equal to curve order",
    },
    Bip340Vector {
        index: 14,
        public_key: "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC30",
        message: "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89",
        signature: "6CFF5C3BA86C69EA4B7376F31A9BCB4F74C1976089B2D9963DA2E5543E17776969E89B4C5564D00349106B8497785DD7D1D713A8AE82B32FA79D5F7FC407D39B",
        published: "FALSE",
        expected: Bip340Expectation::Reject(VerificationIssue::InvalidPublicKeyPoint),
        comment: "public key is not a valid X coordinate because it exceeds the field size",
    },
    Bip340Vector {
        index: 15,
        public_key: "778CAA53B4393AC467774D09497A87224BF9FAB6F6E68B23086497324D6FD117",
        message: "",
        signature: "71535DB165ECD9FBBC046E5FFAEA61186BB6AD436732FCCC25291A55895464CF6069CE26BF03466228F19A3A62DB8A649F2D560FAC652827D1AF0574E427AB63",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "message of size 0 (added 2022-12)",
    },
    Bip340Vector {
        index: 16,
        public_key: "778CAA53B4393AC467774D09497A87224BF9FAB6F6E68B23086497324D6FD117",
        message: "11",
        signature: "08A20A0AFEF64124649232E0693C583AB1B9934AE63B4C3511F3AE1134C6A303EA3173BFEA6683BD101FA5AA5DBC1996FE7CACFC5A577D33EC14564CEC2BACBF",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "message of size 1 (added 2022-12)",
    },
    Bip340Vector {
        index: 17,
        public_key: "778CAA53B4393AC467774D09497A87224BF9FAB6F6E68B23086497324D6FD117",
        message: "0102030405060708090A0B0C0D0E0F1011",
        signature: "5130F39A4059B43BC7CAC09A19ECE52B5D8699D1A71E3C52DA9AFDB6B50AC370C4A482B77BF960F8681540E25B6771ECE1E5A37FD80E5A51897C5566A97EA5A5",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "message of size 17 (added 2022-12)",
    },
    Bip340Vector {
        index: 18,
        public_key: "778CAA53B4393AC467774D09497A87224BF9FAB6F6E68B23086497324D6FD117",
        message: "99999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999",
        signature: "403B12B0D8555A344175EA7EC746566303321E5DBFA8BE6F091635163ECA79A8585ED3E3170807E7C03B720FC54C7B23897FCBA0E9D0B4A06894CFD249F22367",
        published: "TRUE",
        expected: Bip340Expectation::Accept,
        comment: "message of size 100 (added 2022-12)",
    },
];
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
    let secret = k256::FieldBytes::from(secret);
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

fn decode_vector_hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks(HEX_CHARS_PER_BYTE)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).expect("vector hex must be ASCII");
            u8::from_str_radix(digits, 16).expect("vector hex must decode")
        })
        .collect()
}

// r[verify ucan.nostr_identity_adapter.protocol_verification]
// r[verify ucan.nostr_identity_adapter.final_validation]
#[test]
fn every_published_bip340_verification_vector_matches() {
    assert!(BIP340_VECTORS_SOURCE.ends_with("/bip-0340/test-vectors.csv"));
    assert_eq!(BIP340_VECTORS_SHA256.len(), 64);
    assert_eq!(BIP340_VECTORS.len(), 19);

    for (position, vector) in BIP340_VECTORS.iter().enumerate() {
        assert_eq!(vector.index, position, "vectors must stay in published order");
        let published_accept = match vector.published {
            "TRUE" => true,
            "FALSE" => false,
            other => panic!("vector {}: unknown published result {other}", vector.index),
        };
        assert_eq!(
            matches!(vector.expected, Bip340Expectation::Accept),
            published_accept,
            "vector {}: expectation must match the published result",
            vector.index
        );

        let result = verify_bip340_message(
            &vector.public_key.to_ascii_lowercase(),
            &decode_vector_hex(vector.message),
            &vector.signature.to_ascii_lowercase(),
        );
        let matches_expectation = match vector.expected {
            Bip340Expectation::Accept => result.is_ok(),
            Bip340Expectation::Reject(issue) => result == Err(issue),
            Bip340Expectation::RejectSignature => {
                matches!(result, Err(VerificationIssue::InvalidSignatureEncoding | VerificationIssue::SignatureInvalid))
            }
        };
        assert!(
            matches_expectation,
            "vector {} ({}): expected {:?}, got {result:?}",
            vector.index, vector.comment, vector.expected
        );
    }
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

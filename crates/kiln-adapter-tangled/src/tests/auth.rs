use serde_json::json;

use super::*;
use crate::auth::*;
use crate::error::RejectionClass;
use crate::identity::*;
use crate::nsid;

const NOW: i64 = 1_800_000_000;

fn expectation(method: &str) -> Expectation<'_> {
    Expectation {
        audience: SPINDLE,
        method,
        now_unix: NOW,
        maximum_lifetime_seconds: 300,
        clock_skew_seconds: 60,
    }
}

fn knot_issuer_key() -> IssuerKey {
    IssuerKey::Secp256k1(*knot_key().verifying_key())
}

fn verify(token: &str, key: &IssuerKey) -> Result<VerifiedClaims, crate::error::Rejection> {
    verify_token(&parse_token(token)?, key, &expectation(nsid::NOTIFY_EVENT))
}

// r[verify kiln.tangled.auth]
#[test]
fn knot_es256k_and_actor_es256_tokens_verify() {
    let token = sign_token(
        &Signer::K256(knot_key()),
        &claims(KNOT, nsid::NOTIFY_EVENT, NOW, "nonce-1"),
    );
    let verified = ok(verify(&token, &knot_issuer_key()));
    assert_eq!(verified.issuer, KNOT);
    assert_eq!(verified.jti, "nonce-1");
    let token = sign_token(
        &Signer::P256(actor_key()),
        &claims(APPROVER, nsid::NOTIFY_EVENT, NOW, "nonce-2"),
    );
    let key = IssuerKey::P256(*actor_key().verifying_key());
    assert_eq!(ok(verify(&token, &key)).issuer, APPROVER);
}

// r[verify kiln.tangled.auth.scenario.bad-signature]
#[test]
fn signature_and_claim_failures_are_refused() {
    let key = knot_issuer_key();
    let rogue = sign_token(
        &Signer::K256(rogue_key()),
        &claims(KNOT, nsid::NOTIFY_EVENT, NOW, "n"),
    );
    assert_eq!(err(verify(&rogue, &key)).code, "tangled_auth_signature");

    // Claims changed after signing.
    let token = sign_token(
        &Signer::K256(knot_key()),
        &claims(KNOT, nsid::NOTIFY_EVENT, NOW, "n"),
    );
    let mut parts: Vec<&str> = token.split('.').collect();
    let forged = URL_SAFE_NO_PAD.encode(ok(serde_json::to_vec(&claims(
        KNOT,
        nsid::NOTIFY_EVENT,
        NOW,
        "other",
    ))));
    if let Some(part) = parts.get_mut(1) {
        *part = &forged;
    }
    assert_eq!(
        err(verify(&parts.join("."), &key)).code,
        "tangled_auth_signature"
    );

    // High-S twin of a valid signature.
    let signature = ok(URL_SAFE_NO_PAD.decode(some(token.split('.').nth(2))));
    let parsed = ok(k256::ecdsa::Signature::from_slice(&signature));
    let (r, s) = parsed.split_scalars();
    let high = ok(k256::ecdsa::Signature::from_scalars(r, -*s));
    let input = token
        .rsplit_once('.')
        .map(|(input, _)| input)
        .unwrap_or_default();
    let high_token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(high.to_bytes()));
    assert_eq!(
        err(verify(&high_token, &key)).code,
        "tangled_auth_signature"
    );

    let cases = [
        (
            json!({ "iss": KNOT, "aud": "did:web:other.example.com", "lxm": nsid::NOTIFY_EVENT, "exp": NOW + 60, "jti": "n" }),
            "tangled_auth_audience",
        ),
        (
            json!({ "iss": KNOT, "aud": SPINDLE, "lxm": nsid::APPROVE_PIPELINE, "exp": NOW + 60, "jti": "n" }),
            "tangled_auth_method",
        ),
        (
            json!({ "iss": KNOT, "aud": SPINDLE, "exp": NOW + 60, "jti": "n" }),
            "tangled_auth_method",
        ),
        (
            json!({ "iss": KNOT, "aud": SPINDLE, "lxm": nsid::NOTIFY_EVENT, "exp": NOW - 61, "jti": "n" }),
            "tangled_auth_expired",
        ),
        (
            json!({ "iss": KNOT, "aud": SPINDLE, "lxm": nsid::NOTIFY_EVENT, "exp": NOW + 301, "jti": "n" }),
            "tangled_auth_lifetime",
        ),
        (
            json!({ "iss": KNOT, "aud": SPINDLE, "lxm": nsid::NOTIFY_EVENT, "iat": NOW + 120, "exp": NOW + 180, "jti": "n" }),
            "tangled_auth_lifetime",
        ),
        (
            json!({ "iss": KNOT, "aud": SPINDLE, "lxm": nsid::NOTIFY_EVENT, "exp": NOW + 60 }),
            "tangled_auth_jti",
        ),
    ];
    for (claims, code) in cases {
        let token = sign_token(&Signer::K256(knot_key()), &claims);
        assert_eq!(err(verify(&token, &key)).code, code, "{claims}");
    }
    // An ES256 token against a secp256k1 key.
    let mismatched = sign_token(
        &Signer::P256(actor_key()),
        &super::claims(KNOT, nsid::NOTIFY_EVENT, NOW, "n"),
    );
    assert_eq!(
        err(verify(&mismatched, &key)).code,
        "tangled_auth_key_algorithm"
    );
}

#[test]
fn malformed_tokens_and_algorithms_are_refused() {
    let none = format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#),
        URL_SAFE_NO_PAD.encode(br#"{"iss":"did:web:knot.example.com","aud":"x","exp":1}"#),
        URL_SAFE_NO_PAD.encode([0_u8; 64])
    );
    assert_eq!(err(parse_token(&none)).code, "tangled_auth_algorithm");
    for token in [
        "",
        "a.b",
        "a.b.c.d",
        "!!.!!.!!",
        &"a".repeat(MAXIMUM_TOKEN_BYTES + 1),
    ] {
        assert_eq!(
            err(parse_token(token)).class,
            RejectionClass::Authentication
        );
    }
    assert_eq!(err(bearer(None)).code, "tangled_auth_missing");
    assert_eq!(err(bearer(Some("Basic abc"))).code, "tangled_auth_missing");
    assert_eq!(ok(bearer(Some("Bearer abc"))), "abc");
}

#[test]
fn did_documents_yield_multikeys() {
    let knot = knot_issuer_key();
    let document = json!({
        "id": KNOT,
        "verificationMethod": [{ "id": format!("{KNOT}#atproto"), "type": "Multikey", "controller": KNOT, "publicKeyMultibase": multikey(&knot) }],
        "service": [{ "id": "#tangled_knot", "type": "TangledKnot", "serviceEndpoint": "https://knot.example.com" }]
    });
    let bytes = ok(serde_json::to_vec(&document));
    assert_eq!(ok(atproto_key(KNOT, &bytes, 65_536)), knot);
    let actor = IssuerKey::P256(*actor_key().verifying_key());
    let relative = json!({ "id": APPROVER, "verificationMethod": [{ "id": "#atproto", "type": "Multikey", "publicKeyMultibase": multikey(&actor) }] });
    assert_eq!(
        ok(atproto_key(
            APPROVER,
            &ok(serde_json::to_vec(&relative)),
            65_536
        )),
        actor
    );

    assert_eq!(
        err(atproto_key(TRUSTED, &bytes, 65_536)),
        DocumentError::IdentityMismatch
    );
    assert_eq!(err(atproto_key(KNOT, &bytes, 16)), DocumentError::Malformed);
    let legacy = json!({ "id": KNOT, "verificationMethod": [{ "id": "#atproto", "type": "EcdsaSecp256k1VerificationKey2019", "publicKeyMultibase": "zabc" }] });
    assert_eq!(
        err(atproto_key(KNOT, &ok(serde_json::to_vec(&legacy)), 65_536)),
        DocumentError::UnsupportedKey
    );
    let missing = json!({ "id": KNOT, "verificationMethod": [] });
    assert_eq!(
        err(atproto_key(KNOT, &ok(serde_json::to_vec(&missing)), 65_536)),
        DocumentError::MissingKey
    );
    let ed25519 = json!({ "id": KNOT, "verificationMethod": [{ "id": "#atproto", "type": "Multikey", "publicKeyMultibase": format!("z{}", bs58::encode([0xed_u8, 0x01].iter().chain([1_u8; 32].iter()).copied().collect::<Vec<u8>>()).into_string()) }] });
    assert_eq!(
        err(atproto_key(KNOT, &ok(serde_json::to_vec(&ed25519)), 65_536)),
        DocumentError::UnsupportedKey
    );
}

// r[verify kiln.tangled.auth.scenario.bad-signature]
#[test]
fn webhook_signatures_verify_in_constant_time_form() {
    let body = br#"{"id":"kiln-1"}"#;
    let header = some(webhook_signature(body, b"secret"));
    ok(verify_webhook_signature(&header, body, b"secret"));
    assert_eq!(
        err(verify_webhook_signature(&header, b"{}", b"secret")).code,
        "tangled_webhook_signature"
    );
    assert_eq!(
        err(verify_webhook_signature(&header, body, b"other")).code,
        "tangled_webhook_signature"
    );
    assert!(verify_webhook_signature(&header.to_uppercase(), body, b"secret").is_err());
    assert!(verify_webhook_signature("sha1=00", body, b"secret").is_err());
    assert!(verify_webhook_signature(&header, body, b"").is_err());
}

#[test]
fn tangled_identifiers_are_validated() {
    assert!(valid_plc(REPO) && valid_did(REPO));
    assert!(!valid_plc("did:plc:short") && !valid_plc("did:plc:ABCDEFGHIJKLMNOPQRSTUVWX"));
    assert!(!valid_did("did:web:") && !valid_did("plc:abc") && !valid_did("did:Web:x"));
    assert_eq!(
        web_authority("did:web:knot.example.com").as_deref(),
        Some("knot.example.com")
    );
    assert_eq!(
        web_authority("did:web:localhost%3A5555").as_deref(),
        Some("localhost:5555")
    );
    for bad in [
        "did:web:Knot.example.com",
        "did:web:a..b",
        "did:web:host%3A0",
        "did:web:host:path",
        "did:plc:abc",
    ] {
        assert!(web_authority(bad).is_none(), "{bad}");
    }
    assert_eq!(object('a', 40).format(), ObjectFormat::Sha1);
    assert_eq!(object('b', 64).format(), ObjectFormat::Sha256);
    assert!(object('0', 40).is_zero());
    for bad in [
        "A".repeat(40),
        "a".repeat(41),
        "g".repeat(40),
        String::new(),
    ] {
        assert!(ObjectId::parse(&bad).is_none());
    }
    assert!(
        valid_reference("refs/heads/feature/x")
            && !valid_reference("refs/heads/../x")
            && !valid_reference("refs/heads/a b")
    );
    assert!(valid_at_uri(&format!(
        "at://{REPO}/org.tangled.track.ticket/3lzq4w5qk2c2a"
    )));
    assert!(!valid_at_uri("at://did:plc:x") && !valid_at_uri("https://x/y/z"));
}

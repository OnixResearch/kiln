//! Positive and negative cross-consumer conformance corpus.

#![allow(clippy::expect_used, clippy::panic)]

use artifact_auth_core::{
    ALGORITHM_BLAKE3, ALGORITHM_ED25519, ArtifactRef, ArtifactStatement, AuthenticationPolicy,
    AuthenticationScope, CryptographicObservation, ED25519_PUBLIC_KEY_PROFILE_V1, KeyCurrentness,
    POLICY_SCHEMA_V1, STATEMENT_SCHEMA_V1, SignatureEvidence, TrustedKeyObservation,
    evaluate_authentication, statement_identity,
};
use serde::Deserialize;

const GENERATION_ONE: u64 = 1;
const GENERATION_TWO: u64 = 2;
const THRESHOLD_TWO: u16 = 2;
const CAIRN_BASELINE_BLAKE3: &str =
    "65268c8bf653065b42f916ef79212513fbee3cc690f18c80a3b975021d7367ef";
const MANTLE_BASELINE_BLAKE3: &str =
    "d6adc008cce03ea5b39560b0a5586dc6523479e70cb1f7e93d57ec6ee230fbb4";
const CAIRN_LEGACY_PUBLIC_KEY_HEX: &str =
    "456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123";
const CAIRN_LEGACY_PUBLIC_KEY_IDENTITY: &str =
    "fd5a0f16f9af5f2739494a1bce29a4571d3845ea8a61c47e3e2e57b82975fe91";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: String,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    mutation: String,
    expected_passed: bool,
    expected_issue: Option<String>,
    expected_statement_blake3: Option<String>,
}

fn reference(profile: &str, seed: &str) -> ArtifactRef {
    ArtifactRef {
        profile: profile.to_string(),
        algorithm: ALGORITHM_BLAKE3.to_string(),
        digest_hex: blake3::hash(seed.as_bytes()).to_hex().to_string(),
    }
}

fn key(seed: &str) -> ArtifactRef {
    reference(ED25519_PUBLIC_KEY_PROFILE_V1, seed)
}

fn scope() -> AuthenticationScope {
    AuthenticationScope {
        domain: "onix.release-evidence.v1".to_string(),
        purpose: "release-evidence".to_string(),
        profile_id: "onix-artifact-auth.v1".to_string(),
        subject: reference("cairn-receipt.v1", "subject"),
        parents: vec![
            reference("policy.v1", "policy"),
            reference("graph.v1", "graph"),
        ],
        verifier_context: reference("verifier-context.v1", "context"),
    }
}

fn statement(scope: &AuthenticationScope, key_identity: ArtifactRef) -> ArtifactStatement {
    ArtifactStatement {
        schema: STATEMENT_SCHEMA_V1.to_string(),
        scope: scope.clone(),
        producer_id: "shared-producer".to_string(),
        key_id: "shared-label".to_string(),
        key_identity,
    }
}

fn trusted(key_identity: ArtifactRef, seed: &str) -> TrustedKeyObservation {
    TrustedKeyObservation {
        producer_id: "shared-producer".to_string(),
        key_id: "shared-label".to_string(),
        key_identity,
        allowed_purposes: vec!["release-evidence".to_string()],
        generation: GENERATION_ONE,
        currentness: KeyCurrentness::Current,
        currentness_ref: reference("currentness.v1", seed),
    }
}

fn evidence(statement: ArtifactStatement) -> SignatureEvidence {
    SignatureEvidence {
        generation: GENERATION_ONE,
        cryptographic: CryptographicObservation {
            algorithm: ALGORITHM_ED25519.to_string(),
            key_identity: statement.key_identity.clone(),
            verified: true,
            failure_code: None,
        },
        statement,
    }
}

fn fixture() -> (
    AuthenticationPolicy,
    AuthenticationScope,
    Vec<SignatureEvidence>,
) {
    let scope = scope();
    let key_a = key("key-a");
    let key_b = key("key-b");
    let policy = AuthenticationPolicy {
        schema: POLICY_SCHEMA_V1.to_string(),
        profile_id: scope.profile_id.clone(),
        threshold: THRESHOLD_TWO,
        trusted_keys: vec![
            trusted(key_a.clone(), "current-a"),
            trusted(key_b.clone(), "current-b"),
        ],
    };
    let evidence = vec![
        evidence(statement(&scope, key_a)),
        evidence(statement(&scope, key_b)),
    ];
    (policy, scope, evidence)
}

fn apply_mutation(
    case: &Case,
    policy: &mut AuthenticationPolicy,
    evidence: &mut Vec<SignatureEvidence>,
) {
    match case.mutation.as_str() {
        "none" => {}
        "reverse_parents" => {
            for item in evidence {
                item.statement.scope.parents.reverse();
            }
        }
        "wrong_domain" => evidence[0].statement.scope.domain = "other-domain".to_string(),
        "wrong_profile" => policy.profile_id = "other-profile".to_string(),
        "revoked_key" => policy.trusted_keys[0].currentness = KeyCurrentness::Revoked,
        "stale_generation" => evidence[0].generation = GENERATION_TWO,
        "wrong_purpose" => {
            policy.trusted_keys[0].allowed_purposes = vec!["other-purpose".to_string()]
        }
        "duplicate_key" => evidence[1] = evidence[0].clone(),
        "unknown_digest" => evidence[0].statement.scope.subject.algorithm = "unknown".to_string(),
        "crypto_failure" => {
            evidence[0].cryptographic.verified = false;
            evidence[0].cryptographic.failure_code = Some("ed25519.signature_invalid".to_string());
        }
        "unknown_currentness" => policy.trusted_keys[0].currentness = KeyCurrentness::Unknown,
        other => panic!("unknown fixture mutation: {other}"),
    }
}

fn assert_canonical_json_fixture(bytes: &[u8], expected_blake3: &str) {
    let value: serde_json::Value = serde_json::from_slice(bytes).expect("valid baseline JSON");
    let canonical = serde_json::to_vec(&value).expect("canonical baseline JSON");
    assert_eq!(canonical, bytes);
    assert_eq!(blake3::hash(bytes).to_hex().as_str(), expected_blake3);
}

// r[verify artifact_auth.conformance.consumers]
#[test]
fn legacy_statement_bytes_and_identities_are_frozen() {
    assert_canonical_json_fixture(
        include_bytes!("../../../fixtures/baselines/cairn-signature-message.json"),
        CAIRN_BASELINE_BLAKE3,
    );
    assert_canonical_json_fixture(
        include_bytes!("../../../fixtures/baselines/mantle-trust-statement.json"),
        MANTLE_BASELINE_BLAKE3,
    );
    assert_eq!(
        blake3::hash(CAIRN_LEGACY_PUBLIC_KEY_HEX.as_bytes())
            .to_hex()
            .as_str(),
        CAIRN_LEGACY_PUBLIC_KEY_IDENTITY
    );
}

// r[verify artifact_auth.conformance.consumers]
#[test]
fn positive_and_negative_conformance_corpus_is_stable() {
    let bytes = include_bytes!("../../../fixtures/conformance/cases.json");
    let corpus: Corpus = serde_json::from_slice(bytes).expect("valid conformance JSON");
    assert_eq!(corpus.schema, "artifact-auth.conformance-corpus.v1");
    assert!(!corpus.cases.is_empty());
    assert!(corpus.cases.iter().any(|case| case.expected_passed));
    assert!(corpus.cases.iter().any(|case| !case.expected_passed));

    for case in corpus.cases {
        let (mut policy, scope, mut evidence) = fixture();
        apply_mutation(&case, &mut policy, &mut evidence);
        let decision = evaluate_authentication(&policy, &scope, &evidence);
        assert_eq!(
            decision.passed, case.expected_passed,
            "{}: {:?}",
            case.id, decision.issues
        );
        if let Some(expected) = case.expected_issue {
            assert!(
                decision.issues.iter().any(|issue| issue.code == expected),
                "{}: {:?}",
                case.id,
                decision.issues
            );
        }
        if let Some(expected) = case.expected_statement_blake3 {
            let actual =
                statement_identity(&evidence[0].statement).expect("canonical fixture statement");
            assert_eq!(actual, expected, "{}", case.id);
        }
    }
}

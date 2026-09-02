use serde_json::json;

use super::*;

const DIGEST_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const OTHER_DIGEST_HEX: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const NOMINAL_DIGEST_BASELINE: &str = include_str!(
    "../../../../cairn/archive/1970-01-01-encode-digest-roles-in-types/evidence/baseline/shared-digest-and-stack-provenance.json"
);
const STACK_PROVENANCE_CORPUS: &str =
    include_str!("../../../../contracts/stack-provenance/preserves-corpus/corpus.json");

fn required_non_claims() -> Vec<String> {
    vec![SHARED_IDENTITY_REQUIRED_NON_CLAIM.to_owned()]
}

#[test]
fn supported_digest_spellings_normalize_by_profile() {
    // r[verify valence.evidence_ir.shared_identity.fixtures.positive]
    let candidate = SharedDigestCandidate {
        role: DigestRole::ArtifactIdentity,
        expected_role: Some(DigestRole::ArtifactIdentity),
        digest: format!("{B3_PREFIX}{DIGEST_HEX}"),
        profile: STRICT_B3_PROFILE,
        non_claims: required_non_claims(),
    };

    let report = validate_shared_digest(&candidate);

    assert!(report.valid);
    let digest = report.digest.expect("valid digest should be present");
    assert_eq!(digest.normalized_hex, DIGEST_HEX);
    assert_eq!(digest.role, DigestRole::ArtifactIdentity);
    assert!(report.supported_claim.contains("canonical content identity"));
}

#[test]
fn malformed_or_unsupported_digest_fails_closed() {
    // r[verify valence.evidence_ir.shared_identity.fixtures.negative]
    let candidate = SharedDigestCandidate {
        role: DigestRole::ArtifactIdentity,
        expected_role: None,
        digest: OTHER_DIGEST_HEX.to_owned(),
        profile: STRICT_B3_PROFILE,
        non_claims: required_non_claims(),
    };

    let report = validate_shared_digest(&candidate);

    assert!(!report.valid);
    assert!(report.issues.iter().any(|issue| issue.code == "unsupported_digest_spelling"));
}

#[test]
fn digest_roles_are_not_interchangeable() {
    // r[verify valence.evidence_ir.shared_identity.digest_roles]
    let candidate = SharedDigestCandidate {
        role: DigestRole::BinaryBytes,
        expected_role: Some(DigestRole::SidecarBytes),
        digest: format!("{B3_PREFIX}{DIGEST_HEX}"),
        profile: STRICT_B3_PROFILE,
        non_claims: required_non_claims(),
    };

    let report = validate_shared_digest(&candidate);

    assert!(!report.valid);
    assert!(report.issues.iter().any(|issue| issue.code == "role_mismatch"));
}

#[test]
fn canonical_json_hash_is_order_stable() {
    // r[verify valence.evidence_ir.shared_identity.fixtures.positive]
    let left = json!({"b": [2, 1], "a": {"z": true, "m": false}});
    let right = json!({"a": {"m": false, "z": true}, "b": [2, 1]});

    let left_hash = canonical_json_hash(&left).expect("JSON Value serialization should succeed");
    let right_hash = canonical_json_hash(&right).expect("JSON Value serialization should succeed");

    assert_eq!(left_hash, right_hash);
}

#[test]
fn receipt_hash_exclusion_removes_self_reference_only() {
    // r[verify valence.evidence_ir.shared_identity.self_hash_exclusion.receipt]
    let with_self_hash = json!({
        "receipt_hash": "stale",
        "preserves_content_hash": "stale",
        "payload": {"stable": true}
    });
    let without_self_hash = json!({"payload": {"stable": true}});

    let actual = canonical_json_hash_with_exclusion(&with_self_hash, Some(RECEIPT_SELF_HASH_EXCLUSION))
        .expect("known exclusion profile should hash");
    let expected = canonical_json_hash(&without_self_hash).expect("JSON Value serialization should succeed");

    assert_eq!(actual, expected);
}

#[test]
fn missing_exclusion_profile_fails_closed() {
    // r[verify valence.evidence_ir.shared_identity.self_hash_exclusion.unknown]
    let value = json!({"receipt_hash": "stale"});

    let issue = canonical_json_hash_with_exclusion(&value, None).expect_err("missing profile should fail");

    assert_eq!(issue.field, FIELD_SELF_HASH_EXCLUSION);
    assert_eq!(issue.code, "missing_self_hash_exclusion_profile");
}

#[test]
fn non_claim_boundary_rejects_overclaim() {
    // r[verify valence.evidence_ir.shared_identity.final_validation]
    let candidate = SharedDigestCandidate {
        role: DigestRole::ReceiptIdentity,
        expected_role: None,
        digest: DIGEST_HEX.to_owned(),
        profile: RAW_BLAKE3_PROFILE,
        non_claims: vec!["proves behavioral correctness".to_owned()],
    };

    let report = validate_shared_digest(&candidate);

    assert!(!report.valid);
    assert!(report.issues.iter().any(|issue| issue.code == "non_claims_weakened"));
    assert!(report.issues.iter().any(|issue| issue.code == "overclaim_detected"));
}

#[test]
fn canonical_profile_names_are_visible() {
    assert_eq!(CanonicalHashProfile::JsonValue.as_str(), "json_value");
    assert_eq!(CanonicalHashProfile::PreservesText.as_str(), "preserves_text");
}

#[test]
fn every_owned_role_uses_the_shared_checked_core() {
    fn assert_digest<Role: DigestRoleMarker>(expected_role: DigestRole) {
        let digest = Digest::<Role>::parse(DIGEST_HEX, RAW_BLAKE3_PROFILE, FIELD_DIGEST).expect("valid role digest");
        assert_eq!(Role::ROLE, expected_role);
        assert_eq!(digest.as_hex(), DIGEST_HEX);
        assert_eq!(digest.as_ref().to_owned_digest(), digest);
    }

    assert_digest::<BinaryRole>(DigestRole::BinaryBytes);
    assert_digest::<SidecarRole>(DigestRole::SidecarBytes);
    assert_digest::<ArtifactIdentityRole>(DigestRole::ArtifactIdentity);
    assert_digest::<ReceiptRole>(DigestRole::ReceiptIdentity);
    assert_digest::<PolicyRole>(DigestRole::PolicyIdentity);
    assert_digest::<GraphRole>(DigestRole::GraphIdentity);
    assert_digest::<SourceRole>(DigestRole::SourceIdentity);
    assert_digest::<PreservesMetadataRole>(DigestRole::PreservesMetadata);
    assert_digest::<CompatibilityReferenceRole>(DigestRole::CompatibilityReference);
}

#[test]
fn typed_parser_accepts_supported_spellings_and_borrowed_views() {
    let raw = ArtifactIdentityDigest::parse(DIGEST_HEX, RAW_BLAKE3_PROFILE, FIELD_DIGEST).expect("raw digest");
    let b3 = ArtifactIdentityDigest::parse(&format!("{B3_PREFIX}{DIGEST_HEX}"), STRICT_B3_PROFILE, FIELD_DIGEST)
        .expect("b3 digest");
    let blake3_text = format!("{BLAKE3_PREFIX}{DIGEST_HEX}");
    let borrowed = ArtifactIdentityDigestRef::parse(&blake3_text, MIXED_BLAKE3_PROFILE, FIELD_DIGEST)
        .expect("borrowed blake3 digest");

    assert_eq!(raw, b3);
    assert_eq!(borrowed.as_hex(), DIGEST_HEX);
    assert!(raw.as_ref().matches(borrowed));
}

#[test]
fn typed_parser_rejects_unsupported_malformed_and_non_hex_inputs() {
    let unsupported = BinaryDigest::parse(DIGEST_HEX, STRICT_B3_PROFILE, FIELD_DIGEST)
        .expect_err("raw spelling must be rejected by strict profile");
    let unsupported_prefix = BinaryDigest::parse(&format!("sha256:{DIGEST_HEX}"), STRICT_B3_PROFILE, FIELD_DIGEST)
        .expect_err("unsupported prefix must fail before core use");
    let short = BinaryDigest::parse("b3:abcd", STRICT_B3_PROFILE, FIELD_DIGEST).expect_err("short digest must fail");
    let non_hex = BinaryDigest::parse(
        "b3:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        STRICT_B3_PROFILE,
        FIELD_DIGEST,
    )
    .expect_err("non-hex digest must fail");
    let uppercase = BinaryDigest::parse(
        "b3:FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        STRICT_B3_PROFILE,
        FIELD_DIGEST,
    )
    .expect_err("uppercase is outside the shared lowercase profile");

    assert_eq!(unsupported.code, "unsupported_digest_spelling");
    assert_eq!(unsupported_prefix.code, "unsupported_digest_spelling");
    assert_eq!(short.code, "malformed_blake3");
    assert_eq!(non_hex.code, "malformed_blake3");
    assert_eq!(uppercase.code, "malformed_blake3");
}

#[test]
fn named_compatibility_parser_normalizes_existing_uppercase_fields() {
    let uppercase = "B3:FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF";
    let unsupported_prefix = BinaryDigest::parse_compatible(uppercase, MIXED_BLAKE3_PROFILE, FIELD_DIGEST)
        .expect_err("prefix matching remains exact");
    let uppercase_hex = "b3:FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF";
    let owned = BinaryDigest::parse_compatible(uppercase_hex, MIXED_BLAKE3_PROFILE, FIELD_DIGEST)
        .expect("existing uppercase hex is accepted only through compatibility parsing");
    let borrowed = BinaryDigestRef::parse_compatible(uppercase_hex, MIXED_BLAKE3_PROFILE, FIELD_DIGEST)
        .expect("borrowed compatibility digest");

    assert_eq!(unsupported_prefix.code, "malformed_blake3");
    assert_eq!(owned.as_hex(), OTHER_DIGEST_HEX);
    assert!(owned.as_ref().matches(borrowed));
}

#[test]
fn frozen_baseline_binds_shared_wire_and_stack_provenance_identities() {
    let baseline: serde_json::Value =
        serde_json::from_str(NOMINAL_DIGEST_BASELINE).expect("nominal digest baseline JSON");
    let corpus: serde_json::Value =
        serde_json::from_str(STACK_PROVENANCE_CORPUS).expect("stack provenance corpus JSON");
    let rows = corpus["entries"].as_array().expect("stack provenance corpus entries");

    assert_eq!(
        baseline["shared_digest"]["canonical_json_blake3"],
        "a371886a0c90ab6ff83db5755ad02038adaca4531ba80f5d2fefaec79d733666"
    );
    for (surface, expected_hash) in [
        ("stack-provenance-sidecar", "4fd7566d1ef466ecfb77a817101e31d5180ec04ea10d9018a2d81146912d6892"),
        ("mantle-release-manifest-view", "1756ff52c45ba23cf06956c2be1ac4a882357726a0def7bcdc9957e32528d8e5"),
        ("built-binary-thread-input", "2963805bd462f251122eac7d71a673e8d5c4a2bb77b426ff4cf26b1a155eeba1"),
    ] {
        let row = rows.iter().find(|row| row["surface"] == surface).expect("baseline stack provenance surface");
        assert_eq!(row["preserves_content_hash"], expected_hash);
        assert_eq!(baseline["stack_provenance"]["preserves_content_hashes"][surface], expected_hash);
    }
}

#[test]
fn heterogeneous_wire_round_trip_preserves_public_json_and_identity() {
    let wire = SharedDigest {
        role: DigestRole::ArtifactIdentity,
        original: format!("{B3_PREFIX}{DIGEST_HEX}"),
        normalized_hex: DIGEST_HEX.to_owned(),
        spelling: DigestSpelling::B3Prefix,
    };
    let expected_json = format!(
        "{{\"role\":\"artifact_identity\",\"original\":\"b3:{DIGEST_HEX}\",\"normalized_hex\":\"{DIGEST_HEX}\",\"spelling\":\"b3_prefix\"}}"
    );
    let before_json = serde_json::to_string(&wire).expect("serialize compatibility wire digest");
    let before_value = serde_json::to_value(&wire).expect("wire value");
    let before_identity = canonical_json_hash(&before_value).expect("wire canonical identity");

    let admitted = AnyDigest::try_from(wire).expect("admit supported wire role");
    assert_eq!(admitted.role(), DigestRole::ArtifactIdentity);
    assert_eq!(admitted.normalized_hex(), DIGEST_HEX);
    let typed = admitted.clone().try_into_role::<ArtifactIdentityRole>().expect("same-role typed extraction");
    assert_eq!(typed.as_hex(), DIGEST_HEX);
    let projected: SharedDigest = admitted.into();
    let after_json = serde_json::to_string(&projected).expect("project compatibility wire digest");
    let after_value = serde_json::to_value(&projected).expect("projected wire value");
    let after_identity = canonical_json_hash(&after_value).expect("projected canonical identity");

    assert_eq!(before_json, expected_json);
    assert_eq!(after_json, expected_json);
    assert_eq!(before_identity, "a371886a0c90ab6ff83db5755ad02038adaca4531ba80f5d2fefaec79d733666");
    assert_eq!(after_identity, before_identity);
}

#[test]
fn wire_admission_rejects_stale_normalization_and_cross_role_extraction() {
    let stale = SharedDigest {
        role: DigestRole::BinaryBytes,
        original: format!("{B3_PREFIX}{DIGEST_HEX}"),
        normalized_hex: OTHER_DIGEST_HEX.to_owned(),
        spelling: DigestSpelling::B3Prefix,
    };
    let stale_issue = AnyDigest::try_from(stale).expect_err("stale normalized digest must fail");
    assert_eq!(stale_issue.code, "normalized_digest_mismatch");

    let valid = SharedDigest {
        role: DigestRole::BinaryBytes,
        original: DIGEST_HEX.to_owned(),
        normalized_hex: DIGEST_HEX.to_owned(),
        spelling: DigestSpelling::RawHex,
    };
    let admitted = AnyDigest::try_from(valid).expect("valid binary wire digest");
    let role_issue = admitted.try_into_role::<ReceiptRole>().expect_err("binary digest cannot enter a receipt core");
    assert_eq!(role_issue.field, FIELD_ROLE);
    assert_eq!(role_issue.code, "role_mismatch");
}

#[test]
fn closed_wire_dto_rejects_unknown_duplicate_and_unsupported_roles() {
    let unknown_field = format!(
        "{{\"role\":\"binary_bytes\",\"original\":\"{DIGEST_HEX}\",\"normalized_hex\":\"{DIGEST_HEX}\",\"spelling\":\"raw_hex\",\"extra\":true}}"
    );
    let duplicate_role = format!(
        "{{\"role\":\"binary_bytes\",\"role\":\"receipt_identity\",\"original\":\"{DIGEST_HEX}\",\"normalized_hex\":\"{DIGEST_HEX}\",\"spelling\":\"raw_hex\"}}"
    );
    let unsupported_role = format!(
        "{{\"role\":\"authority_identity\",\"original\":\"{DIGEST_HEX}\",\"normalized_hex\":\"{DIGEST_HEX}\",\"spelling\":\"raw_hex\"}}"
    );

    let unknown_error = serde_json::from_str::<SharedDigest>(&unknown_field)
        .expect_err("unknown wire field must fail")
        .to_string();
    let duplicate_error = serde_json::from_str::<SharedDigest>(&duplicate_role)
        .expect_err("duplicate role must fail")
        .to_string();
    let role_error = serde_json::from_str::<SharedDigest>(&unsupported_role)
        .expect_err("unsupported role must fail")
        .to_string();

    assert!(unknown_error.contains("unknown field `extra`"));
    assert!(duplicate_error.contains("duplicate field `role`"));
    assert!(role_error.contains("unknown variant `authority_identity`"));
}

#[test]
fn typed_validation_has_no_runtime_expected_role() {
    let candidate =
        TypedDigestCandidate::<PolicyRole>::new(DIGEST_HEX.to_owned(), RAW_BLAKE3_PROFILE, required_non_claims());
    let report = validate_typed_digest(&candidate);

    assert!(report.valid);
    assert_eq!(report.digest.expect("typed policy digest").as_hex(), DIGEST_HEX);
}

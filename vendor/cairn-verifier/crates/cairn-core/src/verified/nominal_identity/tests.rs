use super::*;

const HASH_LOWER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_UPPER: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const GIT_OID: &str = "0123456789abcdef0123456789abcdef01234567";
const OVERSIZED_ID_BYTES: usize = 129;
const SERIALIZED_WIRE_FIELD_COUNT: usize = 2;

fn generated_policy() -> NominalIdentityPolicy {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/cairn-policy.json"
    ))
    .expect("generated policy JSON must parse");
    serde_json::from_value(value["nominal_identity_policy"].clone())
        .expect("generated nominal identity policy must parse")
}

#[test]
fn same_role_domain_and_digest_values_are_accepted() {
    // r[verify cairn.nominal_evidence_identities.types.same_role]
    // r[verify cairn.nominal_evidence_identities.compile_time]
    // r[verify cairn.nominal_evidence_identities.validation]
    let change = ChangeId::parse("type-lifecycle-evidence-identities", "change_id")
        .expect("canonical change id");
    let policy = PolicyDigest::parse(HASH_LOWER, RAW_BLAKE3_PROFILE, "policy_hash")
        .expect("canonical policy digest");

    let same_change: ChangeId = change;
    let same_policy: PolicyDigest = policy;
    assert_eq!(same_change.as_str(), "type-lifecycle-evidence-identities");
    assert_eq!(same_policy.as_hex(), HASH_LOWER);
}

fn assert_digest_role<Role: DigestRoleMarker>() {
    let value = Blake3Digest::<Role>::parse(HASH_LOWER, RAW_BLAKE3_PROFILE, "digest")
        .expect("same-role digest");
    let same_role: Blake3Digest<Role> = value;
    assert_eq!(same_role.as_hex(), HASH_LOWER);
}

#[test]
fn every_maintained_family_accepts_same_role_values() {
    // r[verify cairn.nominal_evidence_identities.compile_time]
    assert!(ChangeId::parse("example-change", "change_id").is_ok());
    assert!(WorkspaceId::parse("workspace:example", "workspace_id").is_ok());
    assert!(ActorId::parse("actor:example", "actor_id").is_ok());
    assert!(OperationId::parse("operation:example", "operation_id").is_ok());
    assert!(ArchiveId::parse("archive-example", "archive_id").is_ok());
    assert!(PolicyKind::parse("release-policy", "policy_kind").is_ok());
    assert!(EvidenceKind::parse("function-address", "evidence_kind").is_ok());
    assert!(ProjectionRole::parse("compatibility-json", "projection_role").is_ok());

    assert_digest_role::<PolicyDigestRole>();
    assert_digest_role::<InputDigestRole>();
    assert_digest_role::<PlanDigestRole>();
    assert_digest_role::<ReceiptDigestRole>();
    assert_digest_role::<ChainDigestRole>();
    assert_digest_role::<StateDigestRole>();
    assert_digest_role::<ContentRootDigestRole>();
    assert_digest_role::<ArtifactDigestRole>();
    assert_digest_role::<ValenceDigestRole>();
    assert_digest_role::<MantleDigestRole>();
    assert_digest_role::<GraphDigestRole>();
    assert_digest_role::<SourceDigestRole>();
    assert_digest_role::<BinaryDigestRole>();
    assert_digest_role::<SidecarDigestRole>();
    assert_digest_role::<ArchiveDigestRole>();
    assert_digest_role::<PackDigestRole>();
    assert_digest_role::<LedgerDigestRole>();

    let sha256 = Sha256Digest::parse(HASH_LOWER, "sha256").expect("same-role SHA-256");
    let same_sha256: Sha256Digest = sha256;
    assert_eq!(same_sha256.as_hex(), HASH_LOWER);
    let git = GitOidSha1Digest::parse(GIT_OID, "git_oid").expect("same-role Git OID");
    let same_git: GitOidSha1Digest = git;
    assert_eq!(same_git.as_hex(), GIT_OID);
}

#[test]
fn constructors_reject_empty_oversized_malformed_prefix_case_and_algorithm() {
    // r[verify cairn.nominal_evidence_identities.validation]
    let oversized = "a".repeat(OVERSIZED_ID_BYTES);
    let cases = [
        ChangeId::parse("", "change_id").expect_err("empty id must fail"),
        ChangeId::parse(&oversized, "change_id").expect_err("oversized id must fail"),
        ChangeId::parse("change:wrong", "change_id").expect_err("unsupported prefix must fail"),
        ChangeId::parse("Uppercase", "change_id").expect_err("uppercase must fail"),
        PolicyDigest::parse("b3:short", MIXED_BLAKE3_PROFILE, "policy_hash")
            .expect_err("short digest must fail"),
        PolicyDigest::parse(HASH_UPPER, RAW_BLAKE3_PROFILE, "policy_hash")
            .expect_err("uppercase digest must fail"),
        PolicyDigest::parse(
            &format!("sha256:{HASH_LOWER}"),
            MIXED_BLAKE3_PROFILE,
            "policy_hash",
        )
        .expect_err("wrong algorithm prefix must fail"),
    ];

    assert_eq!(cases[0].code, "identity_empty");
    assert_eq!(cases[1].code, "identity_oversized");
    assert_eq!(cases[2].code, "identity_prefix_unsupported");
    assert_eq!(cases[3].code, "identity_spelling_noncanonical");
    assert_eq!(cases[4].code, "digest_length_invalid");
    assert_eq!(cases[5].code, "digest_spelling_noncanonical");
    assert_eq!(cases[6].code, "digest_length_invalid");
}

#[test]
fn compatibility_digest_normalizes_case_without_weakening_role() {
    // r[impl cairn.nominal_evidence_identities.compatibility]
    // r[verify cairn.nominal_evidence_identities.compatibility]
    // r[verify cairn.nominal_evidence_identities.compatibility.read]
    let policy = PolicyDigest::parse_compatible(HASH_UPPER, RAW_BLAKE3_PROFILE, "policy_hash")
        .expect("legacy uppercase compatibility value");
    assert_eq!(policy.as_hex(), HASH_LOWER);
}

#[test]
fn imported_algorithms_remain_outside_blake3_roles() {
    // r[verify cairn.nominal_evidence_identities.validation]
    let sha256 = Sha256Digest::parse(HASH_LOWER, "sha256").expect("SHA-256 digest");
    let git_oid = GitOidSha1Digest::parse(GIT_OID, "git_oid").expect("Git OID");

    assert_eq!(sha256.algorithm(), ImportedDigestAlgorithm::Sha256);
    assert_eq!(git_oid.algorithm(), ImportedDigestAlgorithm::GitOidSha1);
    assert_eq!(git_oid.as_hex(), GIT_OID);
    assert!(GitOidSha1Digest::parse(HASH_LOWER, "git_oid").is_err());
}

#[test]
fn closed_wire_adapter_preserves_shape_and_rejects_wrong_role_projection() {
    // r[verify cairn.nominal_evidence_identities.wire_boundary.valid]
    // r[verify cairn.nominal_evidence_identities.wire_boundary.invalid]
    let wire = IdentityWire {
        role: IdentityRole::ReceiptDigest,
        value: HASH_LOWER.to_owned(),
    };
    let serialized = serde_json::to_value(&wire).expect("serialize wire identity");
    let object = serialized.as_object().expect("wire identity object");
    assert_eq!(object.len(), SERIALIZED_WIRE_FIELD_COUNT);
    assert_eq!(
        serialized,
        serde_json::json!({"role": "receipt-digest", "value": HASH_LOWER})
    );

    let admitted = AnyIdentity::try_from(wire).expect("admit receipt identity");
    assert_eq!(admitted.role(), IdentityRole::ReceiptDigest);
    let issue = admitted
        .try_into_policy_digest()
        .expect_err("receipt cannot become policy digest");
    assert_eq!(issue.code, "identity_role_mismatch");
}

#[test]
fn generated_catalog_is_valid_sorted_and_deterministic() {
    // r[verify cairn.nominal_evidence_identities.policy.deterministic]
    // r[verify cairn.nominal_evidence_identities.octet.guard]
    let policy = generated_policy();
    let diagnostics = nominal_identity_policy_diagnostics(&policy);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(policy
        .non_claims
        .iter()
        .any(|value| value == super::catalog::NOMINAL_IDENTITY_REQUIRED_NON_CLAIM));
    assert_eq!(policy.octet.default_mode, "deny");
    assert!(policy.octet.domains.iter().all(|domain| domain
        .core_scopes
        .contains(&"crates/cairn-core/src/verified".to_owned())));

    let first_runtime = nominal_runtime_projection_hash(&policy);
    let second_runtime = nominal_runtime_projection_hash(&policy);
    let first_octet = nominal_octet_projection_hash(&policy);
    let second_octet = nominal_octet_projection_hash(&policy);
    assert_eq!(first_runtime, second_runtime);
    assert_eq!(first_octet, second_octet);
    assert_ne!(first_runtime, first_octet);
}

#[test]
fn catalog_rejects_projection_drift_duplicates_and_weakened_nonclaims() {
    // r[verify cairn.nominal_evidence_identities.policy.deterministic]
    let mut policy = generated_policy();
    policy.octet.domains[0].symbols = vec!["RawString".to_owned()];
    policy.runtime.domains[1].domain_id = policy.runtime.domains[0].domain_id.clone();
    policy.non_claims.clear();
    let diagnostics = nominal_identity_policy_diagnostics(&policy);
    let codes = diagnostics
        .iter()
        .map(|issue| issue.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&"projection_domain_mismatch"));
    assert!(codes.contains(&"runtime_domain_duplicate"));
    assert!(codes.contains(&"runtime_domains_not_sorted"));
    assert!(codes.contains(&"non_claim_missing"));
}

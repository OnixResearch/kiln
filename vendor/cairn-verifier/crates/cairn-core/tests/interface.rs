const POLICY_SOURCE: &str = include_str!("../../../cairn-policy/generated/cairn-policy.json");

#[test]
fn accepts_compatible_public_exports() {
    // r[verify cairn.octet_integration.evidence]
    assert!(cairn_core::parse_policy_json(POLICY_SOURCE).is_ok());
    assert_eq!(
        cairn_core::BASE_REVISION_KIND_GIT_OID,
        cairn_core::verified::collaboration_change::BASE_REVISION_KIND_GIT_OID
    );
}

#[test]
fn rejects_invalid_policy_through_the_public_alias() {
    // r[verify cairn.octet_integration.evidence]
    assert!(cairn_core::parse_policy_json("").is_err());
    assert!(cairn_core::parse_policy_json("{").is_err());
}

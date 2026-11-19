use serde_json::Value;

use super::*;

const DIGEST_HEX_LENGTH: usize = 64;
const POLICY_GENERATION: u64 = 1;
const SOURCE_LIMIT: u64 = 10;
const DESTINATION_LIMIT: u64 = 5;
const RULE_PRIORITY: u64 = 100;

fn digest(byte: char) -> String {
    format!("blake3:{}", byte.to_string().repeat(DIGEST_HEX_LENGTH))
}

fn scope(resource: &str, abilities: &[&str], limit: Option<u64>) -> CapabilityScope {
    CapabilityScope::new(
        resource,
        abilities.iter().map(|ability| (*ability).to_string()),
        limit,
    )
    .expect("valid scope")
}

fn request(
    capability_class: CapabilityClass,
    action: BranchAction,
    source_scope: CapabilityScope,
    destination_scope: CapabilityScope,
) -> BranchAuthorityRequest {
    BranchAuthorityRequest::new(
        capability_class,
        action,
        digest('1'),
        digest('2'),
        digest('3'),
        source_scope,
        destination_scope,
        POLICY_GENERATION,
        true,
    )
    .expect("valid request")
}

fn current(ucan_composed: bool) -> AuthorityCurrentness {
    AuthorityCurrentness {
        observation_ref: digest('4'),
        policy: CurrentFact::Current,
        capability: CurrentFact::Current,
        revocation: CurrentFact::Current,
        replay: CurrentFact::Current,
        scope: CurrentFact::Current,
        ucan: if ucan_composed {
            UcanComposition::Verified
        } else {
            UcanComposition::Missing
        },
    }
}

fn policy() -> BranchAuthorityPolicy {
    parse_branch_authority_policy(DEFAULT_WORLD_BRANCH_AUTHORITY_POLICY_JSON)
        .expect("checked policy")
}

#[test]
fn every_closed_mode_produces_its_exact_obligations() {
    let cases = [
        (
            CapabilityClass::PublicArtifact,
            BranchAction::Create,
            scope("cap://artifact", &["read"], None),
            scope("cap://artifact", &["read"], None),
            Some(BranchMode::Copyable),
            vec![BranchObligation::IndependentDestinationGrant],
            true,
        ),
        (
            CapabilityClass::ScopedService,
            BranchAction::Activate,
            scope("cap://service/", &["read", "write"], Some(SOURCE_LIMIT)),
            scope("cap://service/tenant", &["read"], Some(DESTINATION_LIMIT)),
            Some(BranchMode::Attenuated),
            vec![BranchObligation::NarrowerDerivation],
            true,
        ),
        (
            CapabilityClass::ExclusiveLease,
            BranchAction::Transfer,
            scope("cap://lease", &["use"], None),
            scope("cap://lease", &["use"], None),
            Some(BranchMode::Linear),
            vec![
                BranchObligation::GenerationFencedTransfer,
                BranchObligation::SourceDeactivation,
            ],
            true,
        ),
        (
            CapabilityClass::ExternalEffect,
            BranchAction::Simulate,
            scope("cap://external", &["invoke"], None),
            scope("cap://external", &["invoke"], None),
            Some(BranchMode::SimulationOnly),
            vec![BranchObligation::ExactSimulationAdapter],
            true,
        ),
        (
            CapabilityClass::DeferredEffect,
            BranchAction::Promote,
            scope("cap://deferred", &["dispatch"], None),
            scope("cap://deferred", &["dispatch"], None),
            Some(BranchMode::PromotionGated),
            vec![
                BranchObligation::PromotionRecheck,
                BranchObligation::ReleaseReservation,
            ],
            true,
        ),
        (
            CapabilityClass::HostSecret,
            BranchAction::Activate,
            scope("cap://host-key", &["sign"], None),
            scope("cap://host-key", &["sign"], None),
            Some(BranchMode::ReplaceBeforeActivation),
            vec![BranchObligation::FreshDestinationGrant],
            true,
        ),
        (
            CapabilityClass::BearerCredential,
            BranchAction::Create,
            scope("cap://credential", &["use"], None),
            scope("cap://credential", &["use"], None),
            Some(BranchMode::NonBranchable),
            vec![BranchObligation::DenyActivation],
            false,
        ),
    ];
    let policy = policy();
    for (class, action, source, destination, mode, obligations, allowed) in cases {
        let decision = evaluate_branch_authority(
            &policy,
            &request(class, action, source, destination),
            &current(true),
        );
        assert_eq!(decision.is_allowed(), allowed);
        assert_eq!(decision.mode(), mode);
        assert_eq!(decision.obligations(), obligations);
    }
}

#[test]
fn widening_missing_ucan_stale_currentness_and_lossy_mapping_deny() {
    let policy = policy();
    let source = scope("cap://service/tenant", &["read"], Some(DESTINATION_LIMIT));
    let wider = scope("cap://service/", &["read", "write"], Some(SOURCE_LIMIT));
    let widening = request(
        CapabilityClass::ScopedService,
        BranchAction::Activate,
        source,
        wider,
    );
    assert_eq!(
        evaluate_branch_authority(&policy, &widening, &current(true)).diagnostic(),
        BranchAuthorityDiagnostic::ScopeWidened
    );

    let copyable = request(
        CapabilityClass::PublicArtifact,
        BranchAction::Create,
        scope("cap://artifact", &["read"], None),
        scope("cap://artifact", &["read"], None),
    );
    assert_eq!(
        evaluate_branch_authority(&policy, &copyable, &current(false)).diagnostic(),
        BranchAuthorityDiagnostic::UcanCompositionMissing
    );

    let mut stale = current(true);
    stale.revocation = CurrentFact::Stale;
    assert_eq!(
        evaluate_branch_authority(&policy, &copyable, &stale).diagnostic(),
        BranchAuthorityDiagnostic::CurrentnessDenied
    );

    let mut lossy = copyable;
    lossy.mapping_lossless = false;
    assert_eq!(
        evaluate_branch_authority(&policy, &lossy, &current(true)).diagnostic(),
        BranchAuthorityDiagnostic::MappingLossy
    );
}

#[test]
fn ambiguity_and_non_narrow_attenuation_fail_closed() {
    let mut value: Value =
        serde_json::from_str(DEFAULT_WORLD_BRANCH_AUTHORITY_POLICY_JSON).expect("policy JSON");
    let mut duplicate = value["rules"][0].clone();
    duplicate["rule_id"] = Value::String("public-artifact-copy-second".to_string());
    duplicate["priority"] = Value::from(RULE_PRIORITY);
    value["rules"]
        .as_array_mut()
        .expect("rules")
        .push(duplicate);
    let ambiguous_policy =
        parse_branch_authority_policy(&serde_json::to_string(&value).expect("mutated policy"))
            .expect("structurally valid ambiguity");
    let copy_request = request(
        CapabilityClass::PublicArtifact,
        BranchAction::Create,
        scope("cap://artifact", &["read"], None),
        scope("cap://artifact", &["read"], None),
    );
    assert_eq!(
        evaluate_branch_authority(&ambiguous_policy, &copy_request, &current(true)).diagnostic(),
        BranchAuthorityDiagnostic::AmbiguousRules
    );

    let equal_attenuation = request(
        CapabilityClass::ScopedService,
        BranchAction::Activate,
        scope("cap://service", &["read"], None),
        scope("cap://service", &["read"], None),
    );
    assert_eq!(
        evaluate_branch_authority(&policy(), &equal_attenuation, &current(true)).diagnostic(),
        BranchAuthorityDiagnostic::AttenuationNotNarrower
    );
}

#[test]
fn malformed_policy_and_bearer_inputs_are_rejected() {
    let mut value: Value =
        serde_json::from_str(DEFAULT_WORLD_BRANCH_AUTHORITY_POLICY_JSON).expect("policy JSON");
    value["rules"][0]["mode"] = Value::String("ambient-copy".to_string());
    assert_eq!(
        parse_branch_authority_policy(&serde_json::to_string(&value).expect("policy")),
        Err(BranchAuthorityPolicyError::Malformed)
    );
    assert_eq!(
        BranchAuthorityRequest::new(
            CapabilityClass::BearerCredential,
            BranchAction::Create,
            digest('1'),
            digest('2'),
            "bearer-token=private",
            scope("cap://credential", &["use"], None),
            scope("cap://credential", &["use"], None),
            POLICY_GENERATION,
            true,
        ),
        Err(BranchAuthorityInputError::InvalidReference)
    );
}

#[test]
fn linear_copy_promotion_bypass_and_replayed_currentness_are_denied() {
    let policy = policy();
    let linear_copy = request(
        CapabilityClass::ExclusiveLease,
        BranchAction::Create,
        scope("cap://lease", &["use"], None),
        scope("cap://lease", &["use"], None),
    );
    assert_eq!(
        evaluate_branch_authority(&policy, &linear_copy, &current(true)).diagnostic(),
        BranchAuthorityDiagnostic::ActionModeMismatch
    );

    let promotion_bypass = request(
        CapabilityClass::DeferredEffect,
        BranchAction::Activate,
        scope("cap://deferred", &["dispatch"], None),
        scope("cap://deferred", &["dispatch"], None),
    );
    assert_eq!(
        evaluate_branch_authority(&policy, &promotion_bypass, &current(true)).diagnostic(),
        BranchAuthorityDiagnostic::ActionModeMismatch
    );

    let copyable = request(
        CapabilityClass::PublicArtifact,
        BranchAction::Create,
        scope("cap://artifact", &["read"], None),
        scope("cap://artifact", &["read"], None),
    );
    let mut replayed = current(true);
    replayed.replay = CurrentFact::Stale;
    assert_eq!(
        evaluate_branch_authority(&policy, &copyable, &replayed).diagnostic(),
        BranchAuthorityDiagnostic::CurrentnessDenied
    );
}

#[test]
fn receipt_is_metadata_only_and_domain_bound() {
    let policy = policy();
    let request = request(
        CapabilityClass::ScopedService,
        BranchAction::Activate,
        scope("cap://service/", &["read", "write"], Some(SOURCE_LIMIT)),
        scope("cap://service/tenant", &["read"], Some(DESTINATION_LIMIT)),
    );
    let decision = evaluate_branch_authority(&policy, &request, &current(true));
    let receipt = branch_authority_receipt(&decision).expect("receipt");
    let json = serde_json::to_string(&receipt).expect("receipt JSON");
    assert!(receipt.allowed);
    assert!(!json.contains("cap://service"));
    assert!(!json.contains("bearer-token="));
    assert!(!json.contains("secret="));
    assert_eq!(receipt.non_claims.len(), POLICY_NON_CLAIMS.len());
    assert!(receipt.decision_ref.starts_with("blake3:"));
}

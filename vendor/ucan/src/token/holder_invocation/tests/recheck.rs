fn verifier<'a>(
    keys: &'a crate::KeyResolutionContext,
    proofs: &'a crate::ProofCollection,
    profiles: &'a dyn crate::OperationProfileAdmission,
    replay: &'a dyn crate::HolderInvocationReplayAdmission,
    caveats: &'a dyn crate::CaveatPolicySet,
) -> super::TestVerifier<'a> {
    super::TestVerifier {
        time: crate::VerificationTime::from_unix_seconds(super::VERIFY_AT_SECONDS),
        keys,
        proofs,
        revocations: &crate::NoRevocations,
        profiles,
        replay,
        caveats,
        token_limits: crate::VerificationLimits::default(),
        holder_limits: crate::HolderInvocationLimits::default(),
    }
}

impl super::TestVerifier<'_> {
    fn recheck<'live>(
        &self,
        authorized: &'live crate::HolderInvocationAuthorized,
        token: &str,
    ) -> Result<crate::HolderAuthorizationObservation<'live>, crate::HolderRecheckError> {
        crate::recheck_holder_authorization(
            authorized,
            token,
            &crate::HolderTokenVerificationContext::new(
                self.time,
                self.keys,
                self.proofs,
                self.revocations,
                self.token_limits,
            ),
            &crate::HolderRecheckPolicies {
                caveats: self.caveats,
                profiles: self.profiles,
            },
        )
    }
}

fn admit_direct(
    fixture: &super::DirectFixture,
    verifier: &super::TestVerifier<'_>,
) -> Result<crate::HolderInvocationAuthorized, crate::HolderInvocationError> {
    let signature = super::sign_request(
        &fixture.token,
        &fixture.audience,
        super::RESOURCE,
        super::ABILITY,
        super::operation(),
        super::nonce(),
        &fixture.holder,
    );
    verifier.verify(&crate::HolderInvocationRequest::new(
        fixture.token.as_str(),
        super::RESOURCE,
        super::ABILITY,
        super::operation(),
        super::nonce(),
        &signature,
    ))
}

#[test]
fn repeated_observations_never_readmit_the_nonce() {
    let fixture = super::DirectFixture::new();
    let proofs = crate::ProofCollection::empty();
    let profiles = super::KnownProfile::new(super::profile());
    let replay = super::RecordingReplay::default();
    let mut verifier = verifier(&fixture.keys, &proofs, &profiles, &replay, &crate::token::DenyUnknownCaveats);
    let authorized = admit_direct(&fixture, &verifier).expect("initial holder admission");
    verifier.token_limits = verifier.token_limits.with_clock_skew_tolerance_seconds(0);
    for seconds in [1500, 1501, 1502, 2000] {
        verifier.time = crate::VerificationTime::from_unix_seconds(seconds);
        let observed = verifier.recheck(&authorized, fixture.token.as_str()).expect("current observation");
        assert!(core::ptr::eq(observed.authorization(), &raw const authorized));
        assert_eq!(observed.observed_at().as_unix_seconds(), seconds);
        assert_eq!(observed.authorization().effective_time_bounds(), super::test_time_bounds());
        assert_eq!(replay.calls.get(), 1);
    }
    verifier.time = crate::VerificationTime::from_unix_seconds(2001);
    assert!(verifier.recheck(&authorized, fixture.token.as_str()).is_err());
    assert_eq!(profiles.calls.get(), 5);
    assert_eq!(replay.calls.get(), 1);
    verifier.time = crate::VerificationTime::from_unix_seconds(2000);
    assert!(matches!(admit_direct(&fixture, &verifier), Err(crate::HolderInvocationError::Replay { .. })));
    assert_eq!(replay.calls.get(), 2);
}

#[test]
fn missing_current_key_denies_before_profile_and_replay() {
    let fixture = super::DirectFixture::new();
    let proofs = crate::ProofCollection::empty();
    let profiles = super::KnownProfile::new(super::profile());
    let replay = super::RecordingReplay::default();
    let mut verifier = verifier(&fixture.keys, &proofs, &profiles, &replay, &crate::token::DenyUnknownCaveats);
    let authorized = admit_direct(&fixture, &verifier).expect("initial holder admission");
    let missing = crate::KeyResolutionContext::new(Vec::new());
    verifier.keys = &missing;
    assert!(matches!(
        verifier.recheck(&authorized, fixture.token.as_str()),
        Err(crate::HolderRecheckError::Verification(crate::HolderInvocationError::Token { .. }))
    ));
    assert_eq!(profiles.calls.get(), 1);
    assert_eq!(replay.calls.get(), 1);
}

struct ForbiddenServices;

impl crate::ProofStore for ForbiddenServices {
    fn resolve_proof<'a>(
        &'a self,
        _: &crate::ProofReference,
    ) -> Result<crate::token::ProofToken<'a>, crate::ProofStoreError> {
        panic!("preflight denial must precede proof lookup")
    }
}

impl crate::KeyResolver for ForbiddenServices {
    fn resolve_key(&self, _: &crate::IssuerDid) -> Result<crate::Ed25519PublicKey, crate::KeyResolverError> {
        panic!("preflight denial must precede key lookup")
    }
}

impl crate::RevocationChecker for ForbiddenServices {
    fn is_revoked(&self, _: &crate::ProofReference) -> Result<bool, crate::RevocationError> {
        panic!("preflight denial must precede revocation lookup")
    }
}

#[test]
fn substitution_regression_and_size_fail_before_external_services() {
    let fixture = super::DirectFixture::new();
    let other = super::DirectFixture::with_capability_and_expiration(super::direct_capability(), 2100);
    let proofs = crate::ProofCollection::empty();
    let profiles = super::KnownProfile::new(super::profile());
    let replay = super::RecordingReplay::default();
    let verifier = verifier(&fixture.keys, &proofs, &profiles, &replay, &crate::token::DenyUnknownCaveats);
    let authorized = admit_direct(&fixture, &verifier).expect("initial holder admission");
    let limits = crate::VerificationLimits::default();
    let oversized = "x".repeat(limits.max_compact_token_bytes() + 1);
    for (token, time) in [
        (other.token.as_str(), 1501),
        (fixture.token.as_str(), 1499),
        (&oversized, 1501),
    ] {
        let result = crate::recheck_holder_authorization(
            &authorized,
            token,
            &crate::HolderTokenVerificationContext::new(
                crate::VerificationTime::from_unix_seconds(time),
                &ForbiddenServices,
                &ForbiddenServices,
                &ForbiddenServices,
                limits,
            ),
            &crate::HolderRecheckPolicies {
                caveats: &crate::token::DenyUnknownCaveats,
                profiles: &profiles,
            },
        );
        assert!(result.is_err());
        assert_eq!(profiles.calls.get(), 1);
        assert_eq!(replay.calls.get(), 1);
    }
}

struct Revocations {
    revoked: Option<crate::ProofReference>,
    is_unavailable: bool,
    calls: std::cell::RefCell<Vec<crate::ProofReference>>,
}

impl crate::RevocationChecker for Revocations {
    fn is_revoked(&self, reference: &crate::ProofReference) -> Result<bool, crate::RevocationError> {
        self.calls.borrow_mut().push(reference.clone());
        if self.is_unavailable {
            return Err(crate::RevocationError::Backend {
                reference: reference.clone(),
                message: "test outage".to_owned(),
            });
        }
        Ok(self.revoked.as_ref() == Some(reference))
    }
}

#[test]
fn expiry_leaf_revocation_and_outage_deny_without_replay() {
    let fixture = super::DirectFixture::new();
    let proofs = crate::ProofCollection::empty();
    let profiles = super::KnownProfile::new(super::profile());
    let replay = super::RecordingReplay::default();
    let mut verifier = verifier(&fixture.keys, &proofs, &profiles, &replay, &crate::token::DenyUnknownCaveats);
    let authorized = admit_direct(&fixture, &verifier).expect("initial holder admission");
    verifier.time = crate::VerificationTime::from_unix_seconds(super::EXPIRED_VERIFY_AT_SECONDS);
    assert!(matches!(
        verifier.recheck(&authorized, fixture.token.as_str()),
        Err(crate::HolderRecheckError::Verification(crate::HolderInvocationError::Token { .. }))
    ));
    for is_unavailable in [false, true] {
        let revocations = Revocations {
            revoked: Some(crate::proof_reference(&fixture.token)),
            is_unavailable,
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let mut checked = self::verifier(&fixture.keys, &proofs, &profiles, &replay, &crate::token::DenyUnknownCaveats);
        checked.time = crate::VerificationTime::from_unix_seconds(1501);
        checked.revocations = &revocations;
        assert!(matches!(
            checked.recheck(&authorized, fixture.token.as_str()),
            Err(crate::HolderRecheckError::Verification(crate::HolderInvocationError::Token { .. }))
        ));
        assert!(revocations.calls.borrow().contains(&crate::proof_reference(&fixture.token)));
        assert_eq!(replay.calls.get(), 1);
    }
}

#[test]
fn delegated_recheck_visits_parent_and_denies_revoked_or_missing_proof() {
    let fixture = super::DelegatedFixture::new();
    let profiles = super::KnownProfile::new(super::profile());
    let replay = super::RecordingReplay::default();
    let mut verifier = verifier(&fixture.keys, &fixture.proofs, &profiles, &replay, &crate::token::DenyUnknownCaveats);
    let signature = super::sign_request(
        &fixture.token,
        &fixture.audience,
        super::RESOURCE,
        super::ABILITY,
        super::operation(),
        super::nonce(),
        &fixture.holder,
    );
    let request = crate::HolderInvocationRequest::new(
        fixture.token.as_str(),
        super::RESOURCE,
        super::ABILITY,
        super::operation(),
        super::nonce(),
        &signature,
    );
    let authorized = verifier.verify(&request).expect("delegated initial admission");
    let parent = crate::proof_reference(&fixture.proofs.as_slice()[0]);
    let revocations = Revocations {
        revoked: None,
        is_unavailable: false,
        calls: std::cell::RefCell::new(Vec::new()),
    };
    verifier.revocations = &revocations;
    assert!(verifier.recheck(&authorized, fixture.token.as_str()).is_ok());
    assert!(revocations.calls.borrow().contains(&parent));
    assert!(revocations.calls.borrow().contains(&crate::proof_reference(&fixture.token)));
    let revoked = Revocations {
        revoked: Some(parent),
        is_unavailable: false,
        calls: std::cell::RefCell::new(Vec::new()),
    };
    verifier.revocations = &revoked;
    assert!(verifier.recheck(&authorized, fixture.token.as_str()).is_err());
    let missing = crate::ProofCollection::empty();
    verifier.revocations = &crate::NoRevocations;
    verifier.proofs = &missing;
    assert!(verifier.recheck(&authorized, fixture.token.as_str()).is_err());
    assert_eq!(replay.calls.get(), 1);
}

#[test]
fn changed_profile_or_caveat_denies_original_operation() {
    let fixture = super::DirectFixture::with_capability_and_expiration(super::caveated_capability(), 2000);
    let proofs = crate::ProofCollection::empty();
    let events = std::cell::RefCell::new(Vec::new());
    let policies = super::OrderedCaveatSet {
        policy: super::OrderedCaveatPolicy {
            events: &events,
            satisfy: true,
        },
    };
    let profiles = super::KnownProfile::new(super::profile());
    let replay = super::RecordingReplay::default();
    let mut verifier = verifier(&fixture.keys, &proofs, &profiles, &replay, &policies);
    let authorized = admit_direct(&fixture, &verifier).expect("initial caveated admission");
    assert!(verifier.recheck(&authorized, fixture.token.as_str()).is_ok());
    let denied = super::OrderedCaveatSet {
        policy: super::OrderedCaveatPolicy {
            events: &events,
            satisfy: false,
        },
    };
    verifier.caveats = &denied;
    assert!(matches!(
        verifier.recheck(&authorized, fixture.token.as_str()),
        Err(crate::HolderRecheckError::Verification(crate::HolderInvocationError::Authorization { .. }))
    ));
    let unknown = super::KnownProfile::new(super::alternate_operation().profile());
    verifier.caveats = &policies;
    verifier.profiles = &unknown;
    let calls = events.borrow().len();
    assert!(matches!(
        verifier.recheck(&authorized, fixture.token.as_str()),
        Err(crate::HolderRecheckError::Verification(crate::HolderInvocationError::OperationProfile { .. }))
    ));
    assert_eq!(events.borrow().len(), calls);
    assert_eq!(replay.calls.get(), 1);
}

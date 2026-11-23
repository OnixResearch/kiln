//! Signed-token positives and negatives for every protected operation over
//! the packaged Basalt policy and the vendored UCAN cohort.

use kiln_adapter_basalt::test_support::{
    self, EXPIRED_AT_S, Fixture, GENERATION, UNTRUSTED_ROOT_SEED, VERIFY_AT_S,
};
use kiln_adapter_basalt::{BasaltAuthority, HolderKey, RoleSigner, VerifiedAuthorityConfig};
use kiln_core::implementation::authority::{
    AUTHORITY_DIGEST_BYTES, AttemptCurrentness, AuthorityDenialReason, AuthorityProfile,
    AuthorityProvenance, AuthorityRequest, HolderInvocationSigner, ProtectedAuthority,
    ProtectedOperation,
};
use kiln_core::implementation::grant::GrantRole;

fn request(operation: ProtectedOperation) -> AuthorityRequest {
    AuthorityRequest {
        operation,
        project: "OnixResearch/kiln".into(),
        work: "kiln.implementation.work".into(),
        work_version: 3,
        attempt: "attempt-a".into(),
        fence: 5,
        request_digest: [41; AUTHORITY_DIGEST_BYTES],
        target: "kiln://implementation/v1/target".into(),
        candidate: if operation.requires_candidate() {
            [43; AUTHORITY_DIGEST_BYTES]
        } else {
            [0; AUTHORITY_DIGEST_BYTES]
        },
    }
}

fn signed(signer: &dyn HolderInvocationSigner, request: &AuthorityRequest) -> AuthorityProfile {
    AuthorityProfile::VerifiedUcan(
        signer
            .sign(request)
            .unwrap_or_else(|error| unreachable!("{error:?}")),
    )
}

fn denial(
    authority: &mut BasaltAuthority,
    request: &AuthorityRequest,
    profile: &AuthorityProfile,
    now: u64,
) -> AuthorityDenialReason {
    authority
        .admit(request, profile, AttemptCurrentness::Current, now)
        .err()
        .unwrap_or_else(|| unreachable!("{:?} must deny", request.operation))
        .reason
}

#[test]
// r[verify kiln.implementation.authority.binding]
// r[verify kiln.implementation.authority.selection]
fn every_operation_admits_its_role_signed_request_direct_and_delegated() {
    for fixture in [Fixture::direct(), Fixture::delegated()] {
        // One signer per process: its nonces are single-use.
        let signer = fixture.signer();
        for operation in ProtectedOperation::ALL {
            let mut authority = fixture.authority();
            let request = request(operation);
            let decision = authority
                .admit(
                    &request,
                    &signed(&signer, &request),
                    AttemptCurrentness::Current,
                    VERIFY_AT_S,
                )
                .unwrap_or_else(|denial| unreachable!("{operation:?}: {denial:?}"));
            assert!(decision.allowed);
            assert_eq!(decision.provenance, AuthorityProvenance::VerifiedUcan);
            assert_eq!(
                Some(decision.principal.as_str()),
                signer.holder_did(GrantRole::of(operation))
            );
            assert_eq!(decision.generation, GENERATION);
            let session = decision
                .session
                .unwrap_or_else(|| unreachable!("allowed decision requires a session"));
            authority
                .revalidate(session, &request, AttemptCurrentness::Current, VERIFY_AT_S)
                .unwrap_or_else(|denial| unreachable!("{operation:?}: {denial:?}"));
        }
        // One consuming replay per admission; revalidation consumes none.
        assert_eq!(fixture.replay.admitted(), ProtectedOperation::ALL.len());
    }
}

#[test]
// r[verify kiln.implementation.authority.binding.scenario.substitution]
fn every_bound_field_substitution_denies_before_replay() {
    let fixture = Fixture::direct();
    for operation in ProtectedOperation::ALL {
        let signer = fixture.signer();
        let original = request(operation);
        for field in 0..8 {
            let mut altered = original.clone();
            match field {
                0 => altered.project.push('x'),
                1 => altered.work.push('x'),
                2 => altered.work_version += 1,
                3 => altered.attempt.push('x'),
                4 => altered.fence += 1,
                5 => altered.request_digest[0] ^= 1,
                6 => altered.target.push('x'),
                _ => {
                    if !operation.requires_candidate() {
                        continue;
                    }
                    altered.candidate[0] ^= 1;
                }
            }
            let profile = signed(&signer, &original);
            let mut authority = fixture.authority();
            assert_eq!(
                denial(&mut authority, &altered, &profile, VERIFY_AT_S),
                AuthorityDenialReason::CredentialDenied,
                "{operation:?} field {field}"
            );
        }
        // Deployment audience and generation are the adapter's, never the
        // caller's: a credential signed for another deployment denies.
        for config in [
            VerifiedAuthorityConfig::from_packaged_policy(
                GENERATION,
                "kiln-other-audience",
                fixture.roots.clone(),
            ),
            VerifiedAuthorityConfig::from_packaged_policy(
                GENERATION + 1,
                test_support::AUDIENCE,
                fixture.roots.clone(),
            ),
        ] {
            let config = config.unwrap_or_else(|error| unreachable!("{error}"));
            let mut authority = BasaltAuthority::verified(config, fixture.services())
                .unwrap_or_else(|error| unreachable!("{error}"));
            assert_eq!(
                denial(
                    &mut authority,
                    &original,
                    &signed(&signer, &original),
                    VERIFY_AT_S
                ),
                AuthorityDenialReason::CredentialDenied,
                "{operation:?} deployment"
            );
        }
    }
    assert_eq!(fixture.replay.admitted(), 0);
}

#[test]
// r[verify kiln.implementation.authority.separation.scenario.widen]
// r[verify kiln.implementation.authority.selection.scenario.roles]
fn a_role_key_cannot_sign_another_roles_operation() {
    let fixture = Fixture::direct();
    for operation in ProtectedOperation::ALL {
        let owner = GrantRole::of(operation);
        for impostor in [
            GrantRole::Coding,
            GrantRole::Verification,
            GrantRole::Publication,
        ] {
            if impostor == owner {
                continue;
            }
            // The impostor's key and grant presented for the owner's operation.
            let key = HolderKey::new(
                owner,
                test_support::seed(test_support::role_seed(impostor)),
                fixture.grant(impostor).to_string(),
            )
            .unwrap_or_else(|error| unreachable!("{error}"));
            let signer = RoleSigner::new(
                &fixture.config(),
                vec![key],
                test_support::counting_nonces(),
            )
            .unwrap_or_else(|error| unreachable!("{error}"));
            let request = request(operation);
            let mut authority = fixture.authority();
            assert_eq!(
                denial(
                    &mut authority,
                    &request,
                    &signed(&signer, &request),
                    VERIFY_AT_S
                ),
                AuthorityDenialReason::CredentialDenied,
                "{impostor:?} signing {operation:?}"
            );
        }
    }
    assert_eq!(fixture.replay.admitted(), 0);
}

#[test]
// r[verify kiln.implementation.authority.currentness.scenario.blocked]
// r[verify kiln.implementation.authority.currentness.scenario.unavailable]
fn expired_revoked_stale_untrusted_replayed_and_unpolicied_requests_deny() {
    for operation in ProtectedOperation::ALL {
        let request = request(operation);

        let fixture = Fixture::direct();
        let mut authority = fixture.authority();
        let profile = signed(&fixture.signer(), &request);
        assert_eq!(
            denial(&mut authority, &request, &profile, EXPIRED_AT_S),
            AuthorityDenialReason::CredentialDenied,
            "{operation:?} expired"
        );

        let fixture = Fixture::delegated();
        let parent = fixture
            .proofs
            .first()
            .cloned()
            .unwrap_or_else(|| unreachable!("delegated parent"));
        fixture.revocations.revoke(&parent);
        let mut authority = fixture.authority();
        assert_eq!(
            denial(
                &mut authority,
                &request,
                &signed(&fixture.signer(), &request),
                VERIFY_AT_S
            ),
            AuthorityDenialReason::CredentialDenied,
            "{operation:?} revoked parent"
        );

        let fixture = Fixture::direct();
        fixture.revocations.set_stale(true);
        let mut authority = fixture.authority();
        assert_eq!(
            denial(
                &mut authority,
                &request,
                &signed(&fixture.signer(), &request),
                VERIFY_AT_S
            ),
            AuthorityDenialReason::CredentialDenied,
            "{operation:?} stale revocation view"
        );

        let untrusted = Fixture::direct_from(UNTRUSTED_ROOT_SEED, test_support::EXPIRES_S);
        let trusted = Fixture::direct();
        let mut authority = trusted.authority();
        assert_eq!(
            denial(
                &mut authority,
                &request,
                &signed(&untrusted.signer(), &request),
                VERIFY_AT_S
            ),
            AuthorityDenialReason::CredentialDenied,
            "{operation:?} untrusted root"
        );

        // The same signed invocation twice: the second consuming step denies,
        // even through a fresh adapter that shares the durable ledger.
        let fixture = Fixture::direct();
        let signer = fixture.signer();
        let (capturing, captured) = test_support::CapturingSigner::new(Box::new(signer));
        let mut first = fixture.authority();
        first
            .admit(
                &request,
                &signed(&capturing, &request),
                AttemptCurrentness::Current,
                VERIFY_AT_S,
            )
            .unwrap_or_else(|denial| unreachable!("{denial:?}"));
        let replayed = captured
            .borrow()
            .last()
            .cloned()
            .unwrap_or_else(|| unreachable!("captured"));
        let mut second = fixture.authority();
        assert_eq!(
            denial(
                &mut second,
                &request,
                &AuthorityProfile::VerifiedUcan(replayed.credential()),
                VERIFY_AT_S
            ),
            AuthorityDenialReason::CredentialDenied,
            "{operation:?} replayed nonce"
        );
        assert_eq!(fixture.replay.admitted(), 1);
        assert_eq!(fixture.replay.calls(), 2);

        // A policy without this operation's contract denies before replay.
        let fixture = Fixture::direct();
        let mut document: serde_json::Value = serde_json::from_str(basalt::DEFAULT_POLICY_JSON)
            .unwrap_or_else(|error| unreachable!("{error}"));
        document
            .get_mut("contracts")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap_or_else(|| unreachable!("policy contracts"))
            .remove(&format!("kiln-{}", operation.basalt_operation()));
        let text = serde_json::to_string(&document).unwrap_or_else(|error| unreachable!("{error}"));
        let config = VerifiedAuthorityConfig::from_policy_json(
            &text,
            blake3::hash(text.as_bytes()).to_hex().as_ref(),
            GENERATION,
            test_support::AUDIENCE,
            fixture.roots.clone(),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        let mut authority = BasaltAuthority::verified(config, fixture.services())
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            denial(
                &mut authority,
                &request,
                &signed(&fixture.signer(), &request),
                VERIFY_AT_S
            ),
            AuthorityDenialReason::CredentialDenied,
            "{operation:?} policy without contract"
        );
        assert_eq!(fixture.replay.admitted(), 0);
    }
}

#[test]
fn reviewed_policy_file_must_match_its_digest() {
    assert!(
        VerifiedAuthorityConfig::from_policy_json(
            basalt::DEFAULT_POLICY_JSON,
            &"0".repeat(64),
            GENERATION,
            test_support::AUDIENCE,
            Vec::new(),
        )
        .is_err()
    );
}

//! Web-of-trust UCAN authorizer facade.
//!
//! Validity boundary: authorization accepts only when the UCAN envelope is
//! valid, the requested ability is contained by the granted ability, caveats
//! are both contained and satisfied, DID/CID decode linkage holds at the
//! spec/proof layer, and rooted web-of-trust resolution plus threshold policy
//! accept at the evaluation time. DID/CID decode stays spec-only here; the
//! runtime facade consumes a caller-provided linkage fact and only uses public
//! exec surfaces already exposed by the UCAN and web-of-trust modules.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebOfTrustUcanAuthorization {
    Authorized,
    Denied,
}

verus! {

// r[impl composite-primitives.web-of-trust-ucan-authorizer]
pub open spec fn ucan_did_cid_linkage_spec(
    issuer_did: Seq<u8>,
    issuer_public_key: Seq<u8>,
    did_codec: u64,
    proof_cid_bytes: Seq<u8>,
) -> bool {
    &&& match crate::ucan_did::decode_did_key_spec(issuer_did) {
        Some((decoded_key, decoded_codec)) => decoded_key == issuer_public_key && decoded_codec == did_codec,
        None => false,
    }
    &&& exists|cid: crate::ucan_cid::CidSpec| crate::ucan_cid::cid_bytes_spec(cid) == proof_cid_bytes
    &&& match crate::ucan_cid::cid_decode_spec(proof_cid_bytes) {
        Some(decoded_cid_bytes) => match crate::ucan_cid::multihash_decode_spec(decoded_cid_bytes) {
            Some((_mh, _used)) => true,
            None => false,
        },
        None => false,
    }
}

// r[impl composite-primitives.web-of-trust-ucan-authorizer]
pub open spec fn web_of_trust_ucan_authorizer_spec(
    payload: crate::ucan_token::UcanPayload,
    sig_valid: bool,
    now: u64,
    tolerance: u64,
    granted_ability: crate::ucan_ability::Ability,
    requested_ability: crate::ucan_ability::Ability,
    parent_caveats: Seq<crate::ucan_caveat::Caveat>,
    delegated_caveats: Seq<crate::ucan_caveat::Caveat>,
    active_caveats: Seq<crate::ucan_caveat::Caveat>,
    caveat_ctx: crate::ucan_caveat::CaveatContext,
    issuer_public_key: Seq<u8>,
    did_codec: u64,
    proof_cid_bytes: Seq<u8>,
    roots: Seq<crate::web_of_trust_principal::PrincipalId>,
    attestations: Seq<crate::web_of_trust_attestation::Attestation>,
    revocations: Seq<crate::web_of_trust_revocation::Revocation>,
    subject: crate::web_of_trust_principal::PrincipalId,
    threshold: nat,
    depth: nat,
    trust_time: u64,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    crate::ucan_token::envelope_valid_spec(payload, sig_valid, now, tolerance)
    && crate::ucan_token::temporal_valid_spec(payload.exp, payload.nbf, now, tolerance)
    && crate::ucan_ability::ability_contains_spec2(granted_ability, requested_ability)
    && crate::ucan_caveat::caveat_set_contains_spec(parent_caveats, delegated_caveats)
    && crate::ucan_caveat::caveat_set_satisfied_spec(active_caveats, caveat_ctx)
    && ucan_did_cid_linkage_spec(payload.iss@, issuer_public_key, did_codec, proof_cid_bytes)
    && crate::web_of_trust_resolution::resolves_trusted_at_spec(
        roots,
        attestations,
        revocations,
        subject,
        depth,
        trust_time,
    )
    && crate::web_of_trust_policy::threshold_accepts_spec(
        roots,
        attestations,
        revocations,
        subject,
        threshold,
        depth,
        trust_time,
    )
}

// r[verify composite-primitives.web-of-trust-ucan-authorizer]
pub proof fn web_of_trust_ucan_authorizer_refines_sources(
    payload: crate::ucan_token::UcanPayload,
    sig_valid: bool,
    now: u64,
    tolerance: u64,
    granted_ability: crate::ucan_ability::Ability,
    requested_ability: crate::ucan_ability::Ability,
    parent_caveats: Seq<crate::ucan_caveat::Caveat>,
    delegated_caveats: Seq<crate::ucan_caveat::Caveat>,
    active_caveats: Seq<crate::ucan_caveat::Caveat>,
    caveat_ctx: crate::ucan_caveat::CaveatContext,
    issuer_public_key: Seq<u8>,
    did_codec: u64,
    proof_cid_bytes: Seq<u8>,
    roots: Seq<crate::web_of_trust_principal::PrincipalId>,
    attestations: Seq<crate::web_of_trust_attestation::Attestation>,
    revocations: Seq<crate::web_of_trust_revocation::Revocation>,
    subject: crate::web_of_trust_principal::PrincipalId,
    threshold: nat,
    depth: nat,
    trust_time: u64,
)
    requires
        attestations.len() <= usize::MAX as nat,
        web_of_trust_ucan_authorizer_spec(
            payload,
            sig_valid,
            now,
            tolerance,
            granted_ability,
            requested_ability,
            parent_caveats,
            delegated_caveats,
            active_caveats,
            caveat_ctx,
            issuer_public_key,
            did_codec,
            proof_cid_bytes,
            roots,
            attestations,
            revocations,
            subject,
            threshold,
            depth,
            trust_time,
        ),
    ensures
        crate::ucan_token::envelope_valid_spec(payload, sig_valid, now, tolerance),
        crate::ucan_ability::ability_contains_spec2(granted_ability, requested_ability),
        crate::ucan_caveat::caveat_set_contains_spec(parent_caveats, delegated_caveats),
        crate::ucan_caveat::caveat_set_satisfied_spec(active_caveats, caveat_ctx),
        ucan_did_cid_linkage_spec(payload.iss@, issuer_public_key, did_codec, proof_cid_bytes),
        crate::web_of_trust_resolution::resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, trust_time),
        crate::web_of_trust_policy::threshold_accepts_spec(roots, attestations, revocations, subject, threshold, depth, trust_time),
{
}

} // verus!

// r[impl composite-primitives.web-of-trust-ucan-authorizer]
pub fn web_of_trust_ucan_authorizer(
    payload: &crate::ucan_token::UcanPayload,
    sig_valid: bool,
    now: u64,
    tolerance: u64,
    ability_allowed: bool,
    caveats_contained: bool,
    active_caveats: &Vec<crate::ucan_caveat::Caveat>,
    caveat_ctx: &crate::ucan_caveat::CaveatContext,
    did_cid_linked: bool,
    roots: &Vec<crate::web_of_trust_principal::PrincipalId>,
    attestations: &Vec<crate::web_of_trust_attestation::Attestation>,
    revocations: &Vec<crate::web_of_trust_revocation::Revocation>,
    subject: &crate::web_of_trust_principal::PrincipalId,
    threshold: usize,
    depth: usize,
    trust_time: u64,
) -> WebOfTrustUcanAuthorization {
    if crate::ucan_token::validate_envelope(payload, sig_valid, now, tolerance).is_err() {
        return WebOfTrustUcanAuthorization::Denied;
    }
    if !ability_allowed || !caveats_contained || !did_cid_linked {
        return WebOfTrustUcanAuthorization::Denied;
    }
    if !crate::ucan_caveat::caveat_set_satisfied(active_caveats, caveat_ctx) {
        return WebOfTrustUcanAuthorization::Denied;
    }
    if !crate::web_of_trust_resolution::resolves_trusted_at(
        roots,
        attestations,
        revocations,
        subject,
        depth,
        trust_time,
    ) {
        return WebOfTrustUcanAuthorization::Denied;
    }
    if !crate::web_of_trust_policy::threshold_accepts(
        roots,
        attestations,
        revocations,
        subject,
        threshold,
        depth,
        trust_time,
    ) {
        return WebOfTrustUcanAuthorization::Denied;
    }
    WebOfTrustUcanAuthorization::Authorized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ucan_caveat::Caveat;
    use crate::ucan_caveat::CaveatContext;
    use crate::ucan_token::UcanCapability;
    use crate::ucan_token::UcanPayload;
    use crate::web_of_trust_attestation::Attestation;
    use crate::web_of_trust_attestation::TRUST_ROLE_TAG;
    use crate::web_of_trust_attestation::attestation_payload_bytes;
    use crate::web_of_trust_attestation::attestation_signature_bytes_runtime;
    use crate::web_of_trust_principal::principal_id_from_bytes;
    use crate::web_of_trust_revocation::FAMILY_TARGET_KIND_TAG;
    use crate::web_of_trust_revocation::Revocation;
    use crate::web_of_trust_revocation::revocation_payload_bytes;
    use crate::web_of_trust_revocation::revocation_signature_bytes_runtime;

    fn signed_attestation(
        issuer_key_bytes: Vec<u8>,
        subject_key_bytes: Vec<u8>,
        role_tag: u8,
        not_before: u64,
        not_after: u64,
        sequence_no: u64,
    ) -> Attestation {
        let issuer = principal_id_from_bytes(&issuer_key_bytes);
        let subject = principal_id_from_bytes(&subject_key_bytes);
        let unsigned = Attestation {
            issuer_key_bytes,
            issuer,
            subject_key_bytes,
            subject,
            role_tag,
            not_before,
            not_after,
            sequence_no,
            signature: Vec::new(),
        };
        let payload = attestation_payload_bytes(&unsigned).unwrap();
        let signature = attestation_signature_bytes_runtime(unsigned.issuer_key_bytes.as_slice(), payload.as_slice());
        Attestation { signature, ..unsigned }
    }

    fn signed_family_revocation(
        issuer_key_bytes: Vec<u8>,
        subject_key_bytes: Vec<u8>,
        role_tag: u8,
        effective_time: u64,
    ) -> Revocation {
        let issuer = principal_id_from_bytes(&issuer_key_bytes);
        let family_subject = principal_id_from_bytes(&subject_key_bytes);
        let unsigned = Revocation {
            issuer_key_bytes,
            issuer,
            target_kind_tag: FAMILY_TARGET_KIND_TAG,
            exact_payload_target: Vec::new(),
            family_subject_key_bytes: subject_key_bytes,
            family_subject,
            family_role_tag: role_tag,
            effective_time,
            signature: Vec::new(),
        };
        let payload = revocation_payload_bytes(&unsigned).unwrap();
        let signature = revocation_signature_bytes_runtime(unsigned.issuer_key_bytes.as_slice(), payload.as_slice());
        Revocation { signature, ..unsigned }
    }

    fn valid_payload() -> UcanPayload {
        UcanPayload {
            iss: b"did:key:zissuer".to_vec(),
            aud: b"did:key:zaudience".to_vec(),
            att: vec![UcanCapability {
                resource: b"store://docs".to_vec(),
                ability: b"crud/read".to_vec(),
            }],
            prf: vec![b"bafy-proof".to_vec()],
            exp: 100,
            nbf: 0,
        }
    }

    #[test]
    fn rooted_threshold_token_authorizes() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![7, 8, 9];
        let att = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        let caveats = vec![Caveat::TimeBound { not_after: 50 }];
        let ctx = CaveatContext {
            now: 5,
            count_in_window: 0,
        };

        let outcome = web_of_trust_ucan_authorizer(
            &valid_payload(),
            true,
            5,
            0,
            true,
            true,
            &caveats,
            &ctx,
            true,
            &roots,
            &vec![att],
            &Vec::new(),
            &subject,
            1,
            1,
            5,
        );
        assert_eq!(outcome, WebOfTrustUcanAuthorization::Authorized);
    }

    #[test]
    fn revoked_or_under_threshold_token_denies() {
        let root_a_key = vec![1, 2, 3];
        let root_b_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att_a = signed_attestation(root_a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att_a_again = signed_attestation(root_a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att_b = signed_attestation(root_b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let rev = signed_family_revocation(root_b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 5);
        let roots = vec![
            principal_id_from_bytes(&root_a_key),
            principal_id_from_bytes(&root_b_key),
        ];
        let subject = principal_id_from_bytes(&subject_key);
        let caveats = vec![Caveat::TimeBound { not_after: 50 }];
        let ctx = CaveatContext {
            now: 5,
            count_in_window: 0,
        };

        let revoked = web_of_trust_ucan_authorizer(
            &valid_payload(),
            true,
            5,
            0,
            true,
            true,
            &caveats,
            &ctx,
            true,
            &roots,
            &vec![att_a_again, att_b],
            &vec![rev],
            &subject,
            2,
            1,
            5,
        );
        assert_eq!(revoked, WebOfTrustUcanAuthorization::Denied);

        let under_threshold = web_of_trust_ucan_authorizer(
            &valid_payload(),
            true,
            5,
            0,
            true,
            true,
            &caveats,
            &ctx,
            true,
            &roots,
            &vec![att_a],
            &Vec::new(),
            &subject,
            2,
            1,
            5,
        );
        assert_eq!(under_threshold, WebOfTrustUcanAuthorization::Denied);
    }
}

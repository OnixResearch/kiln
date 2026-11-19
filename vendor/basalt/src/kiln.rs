//! Reviewed Kiln holder-invocation boundary. Admission facts cannot mint sessions.
//!
//! Every request field is length-framed into a canonical
//! `kiln://implementation/v1/` resource. The resource and distinct ability are covered
//! by both the UCAN grant and the holder signature. Callers own keys, proof loading,
//! revocation currentness, monotonic time and durable replay. Kiln owns current
//! work/fence/policy facts, claims, budgets, verification, publication and effects.
use crate::authority_core::BasaltPolicyRef;
use crate::ucan_enforcement::{
    EnforcementAction, UcanEnforcementBoundaries, UcanEnforcementRequest,
    verify_ucan_enforcement_request,
};
use crate::ucan_profile::BasaltUcanProtocolProfile;
use crate::{EnforcementRequest, Policy};
use std::fmt::Write as _;
use ucan::{
    HolderInvocationNonce, SignerSignature, verify_compact_token_str_with_resolvers_and_revocations,
};

const RESOURCE_PREFIX: &str = "kiln://implementation/v1/";
const MAX_FIELD_BYTES: usize = 512;
const MAX_RESOURCE_BYTES: usize = 4_096;
const DIGEST_BYTES: usize = 32;

/// Closed Kiln implementation operations. No operation implies another grant.
// r[impl basalt.kiln_authority.actions]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    ClaimAcquire,
    ClaimRenew,
    Dispatch,
    CandidateSubmit,
    VerificationAdmit,
    AttemptCancel,
    PublicationPublish,
    IntegrationLand,
    LifecycleComplete,
}
impl Operation {
    /// Return the distinct reviewed action; no action implies another grant.
    #[must_use]
    pub const fn action(self) -> EnforcementAction {
        match self {
            Self::ClaimAcquire => EnforcementAction::KilnClaimAcquire,
            Self::ClaimRenew => EnforcementAction::KilnClaimRenew,
            Self::Dispatch => EnforcementAction::KilnDispatch,
            Self::CandidateSubmit => EnforcementAction::KilnCandidateSubmit,
            Self::VerificationAdmit => EnforcementAction::KilnVerificationAdmit,
            Self::AttemptCancel => EnforcementAction::KilnAttemptCancel,
            Self::PublicationPublish => EnforcementAction::KilnPublicationPublish,
            Self::IntegrationLand => EnforcementAction::KilnIntegrationLand,
            Self::LifecycleComplete => EnforcementAction::KilnLifecycleComplete,
        }
    }

    /// Return whether this operation binds a candidate identity.
    ///
    /// Claim, dispatch and cancellation act before a candidate exists; every later
    /// operation MUST name the exact candidate it admits or mutates.
    // r[impl basalt.kiln_authority.binding.scenario.candidate]
    #[must_use]
    pub const fn requires_candidate(self) -> bool {
        match self {
            Self::ClaimAcquire | Self::ClaimRenew | Self::Dispatch | Self::AttemptCancel => false,
            Self::CandidateSubmit
            | Self::VerificationAdmit
            | Self::PublicationPublish
            | Self::IntegrationLand
            | Self::LifecycleComplete => true,
        }
    }
}

/// Untrusted request, not a grant. `subject` is the signing UCAN holder DID;
/// deployment `audience` is independent and also bound into the signed resource.
#[derive(Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub operation: Operation,
    pub subject: String,
    pub audience: String,
    pub project: String,
    pub work: String,
    pub work_version: u64,
    pub attempt: String,
    pub fence: u64,
    pub generation: u64,
    pub request_digest: [u8; DIGEST_BYTES],
    pub target: String,
    pub candidate: [u8; DIGEST_BYTES],
}

/// Credential-bearing input deliberately has no Debug/serde surface.
pub struct Invocation<'a> {
    pub compact_token: &'a str,
    pub nonce: HolderInvocationNonce,
    pub holder_signature: &'a SignerSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Denied;
impl std::fmt::Display for Denied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("kiln-authority-denied")
    }
}
impl std::error::Error for Denied {}

/// Non-deserializable, non-cloneable authority minted only after final replay.
/// Do not persist this value or treat its redacted projections as live authority.
// r[impl basalt.kiln_authority.ingress.scenario.opaque]
pub struct Session {
    request: Request,
    token: ucan::CompactToken,
    policy_ref: String,
    expires_unix_s: u64,
    effective: ucan::CapabilitySet,
    decision_id: String,
}
impl Session {
    /// Return the exact signed request observation, not a reusable grant DTO.
    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }
    /// Signed effective caveats for non-consuming receiver policy construction.
    /// These observations are not transferable authority; do not log payloads.
    #[must_use]
    pub const fn effective_capabilities(&self) -> &ucan::CapabilitySet {
        &self.effective
    }
    /// Return the effective signed deadline.
    #[must_use]
    pub const fn expires_unix_s(&self) -> u64 {
        self.expires_unix_s
    }
    /// Return the reviewed policy artifact reference.
    #[must_use]
    pub fn policy_identity(&self) -> &str {
        &self.policy_ref
    }
    /// Return the real successful Basalt admission receipt reference.
    #[must_use]
    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }
    /// Check the exact operation, target and normalized digest before any effect.
    ///
    /// # Errors
    /// Returns `Denied` for any operation, target or digest mismatch.
    pub fn require(
        &self,
        operation: Operation,
        target: &str,
        digest: [u8; DIGEST_BYTES],
    ) -> Result<(), Denied> {
        if self.request.operation == operation
            && self.request.target == target
            && self.request.request_digest == digest
        {
            Ok(())
        } else {
            Err(Denied)
        }
    }
    /// Check the exact admitted candidate before any candidate-bound effect.
    ///
    /// # Errors
    /// Returns `Denied` for a different candidate, or for an operation that binds none.
    pub fn require_candidate(&self, candidate: [u8; DIGEST_BYTES]) -> Result<(), Denied> {
        if self.request.operation.requires_candidate() && self.request.candidate == candidate {
            Ok(())
        } else {
            Err(Denied)
        }
    }
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KilnSession").finish_non_exhaustive()
    }
}

/// Operator-loaded policy/configuration, never caller-supplied admission facts.
/// The artifact reference must name these exact reviewed policy bytes.
pub struct Verifier<'a> {
    pub policy: &'a Policy,
    pub policy_ref: &'a BasaltPolicyRef,
    pub generation: u64,
    pub audience: &'a str,
    /// Operator-admitted root issuers; holder keys are NOT issuer authority.
    pub trusted_roots: &'a [String],
}
impl Verifier<'_> {
    /// Public canonicalization shared by the issuer and holder signer.
    ///
    /// # Errors
    /// Returns `Denied` for stale deployment bindings, a misused candidate field or
    /// malformed/bounded request fields.
    // r[impl basalt.kiln_authority.binding]
    pub fn enforcement(&self, request: &Request) -> Result<EnforcementRequest, Denied> {
        if request.generation != self.generation
            || self.generation == 0
            || request.audience != self.audience
        {
            return Err(Denied);
        }
        let candidate = request.candidate != [0; DIGEST_BYTES];
        if candidate != request.operation.requires_candidate() {
            return Err(Denied);
        }
        let action = request.operation.action();
        let Some(contract) = self.policy.contract(action.contract_id()) else {
            return Err(Denied);
        };
        if !contract.permits_resource(RESOURCE_PREFIX)
            || !contract.permits_ability(action.ability())
        {
            return Err(Denied);
        }
        let mut resource = String::from(RESOURCE_PREFIX);
        for field in [
            request.subject.as_str(),
            request.audience.as_str(),
            request.project.as_str(),
            request.work.as_str(),
            request.attempt.as_str(),
            request.target.as_str(),
        ] {
            if field.is_empty()
                || field.len() > MAX_FIELD_BYTES
                || field.trim() != field
                || !field.is_ascii()
                || field.bytes().any(|b| b.is_ascii_control())
            {
                return Err(Denied);
            }
            // Hex encoding prevents delimiter/Unicode normalization ambiguity.
            write!(&mut resource, "{}/", field.len()).map_err(|_| Denied)?;
            for byte in field.bytes() {
                write!(&mut resource, "{byte:02x}").map_err(|_| Denied)?;
            }
            resource.push('/');
        }
        for value in [request.work_version, request.fence, request.generation] {
            write!(&mut resource, "{value}/").map_err(|_| Denied)?;
        }
        for byte in request.request_digest {
            write!(&mut resource, "{byte:02x}").map_err(|_| Denied)?;
        }
        resource.push('/');
        for byte in request.candidate {
            write!(&mut resource, "{byte:02x}").map_err(|_| Denied)?;
        }
        if resource.len() > MAX_RESOURCE_BYTES {
            return Err(Denied);
        }
        Ok(EnforcementRequest::new(
            action.contract_id(),
            resource,
            action.ability(),
        ))
    }

    fn selected_policy(&self, operation: Operation) -> BasaltPolicyRef {
        BasaltPolicyRef::new(
            &self.policy_ref.artifact_ref,
            &self.policy_ref.schema_version,
            operation.action().contract_id(),
        )
    }

    /// Verify a signed capability and holder request before consuming replay.
    ///
    /// # Errors
    /// Returns only `Denied` when any cryptographic, policy or live boundary denies.
    // r[impl basalt.kiln_authority.ingress]
    pub fn verify(
        &self,
        request: Request,
        invocation: &Invocation<'_>,
        boundaries: &UcanEnforcementBoundaries<'_>,
    ) -> Result<Session, Denied> {
        let enforcement = self.enforcement(&request)?;
        let token = ucan::CompactToken::new(invocation.compact_token).map_err(|_| Denied)?;
        // Bind holder identity BEFORE the consuming replay boundary. A credential
        // valid for a different holder must never burn a nonce for this request.
        let verified = verify_compact_token_str_with_resolvers_and_revocations(
            invocation.compact_token,
            boundaries.verification_time,
            boundaries.key_resolver,
            boundaries.proof_store,
            boundaries.revocations,
            boundaries.verification_limits,
        )
        .map_err(|_| Denied)?;
        if verified.audience().as_str() != request.subject {
            return Err(Denied);
        }
        self.check_roots(
            &verified,
            boundaries,
            &mut boundaries.verification_limits.max_proof_count(),
        )?;
        let selected = self.selected_policy(request.operation);
        let raw = UcanEnforcementRequest::new(
            &enforcement,
            invocation.compact_token,
            BasaltUcanProtocolProfile::OnixCompactEd25519V1,
            request.operation.action(),
            invocation.nonce,
            invocation.holder_signature,
        );
        let live = verify_ucan_enforcement_request(self.policy, &selected, &raw, boundaries)
            .map_err(|_| Denied)?;
        let expires_unix_s = live
            .holder_authorization()
            .effective_time_bounds()
            .expires_at_seconds();
        let decision_id = live.receipt().receipt_blake3.clone();
        let effective = verified.effective_delegation().capabilities().clone();
        Ok(Session {
            request,
            token,
            policy_ref: selected.artifact_ref,
            expires_unix_s,
            effective,
            decision_id,
        })
    }

    /// Revalidate signatures, full proof attenuation, time, current revocations,
    /// caveats and loaded policy on EVERY dispatch. This does not re-spend replay:
    /// possession of Session proves its invocation already passed final admission.
    ///
    /// # Errors
    /// Returns `Denied` when any current authority binding no longer holds.
    // r[impl basalt.kiln_authority.revalidation]
    pub fn revalidate(
        &self,
        session: &Session,
        boundaries: &UcanEnforcementBoundaries<'_>,
    ) -> Result<(), Denied> {
        if session.policy_ref != self.policy_ref.artifact_ref {
            return Err(Denied);
        }
        let enforcement = self.enforcement(&session.request)?;
        let verified = verify_compact_token_str_with_resolvers_and_revocations(
            session.token.as_str(),
            boundaries.verification_time,
            boundaries.key_resolver,
            boundaries.proof_store,
            boundaries.revocations,
            boundaries.verification_limits,
        )
        .map_err(|_| Denied)?;
        self.check_roots(
            &verified,
            boundaries,
            &mut boundaries.verification_limits.max_proof_count(),
        )?;
        if verified.audience().as_str() != session.request.subject
            || !verified
                .authorize_with_policies(
                    enforcement.resource(),
                    enforcement.ability(),
                    boundaries.caveat_policies,
                )
                .is_allowed()
        {
            return Err(Denied);
        }
        // No capability DTO supplied by the caller reaches policy enforcement.
        let local = EnforcementRequest::new(
            enforcement.contract(),
            enforcement.resource(),
            enforcement.ability(),
        )
        .with_capability(crate::CapabilityGrant::new(
            enforcement.resource(),
            enforcement.ability(),
        ));
        if !crate::enforce(self.policy, &local)
            .map_err(|_| Denied)?
            .is_allowed()
        {
            return Err(Denied);
        }
        Ok(())
    }

    // r[impl basalt.kiln_authority.delegation]
    fn check_roots(
        &self,
        token: &ucan::VerifiedToken,
        boundaries: &UcanEnforcementBoundaries<'_>,
        remaining: &mut usize,
    ) -> Result<(), Denied> {
        if token.proofs().as_slice().is_empty() {
            return if self
                .trusted_roots
                .iter()
                .any(|root| root == token.issuer().as_str())
            {
                Ok(())
            } else {
                Err(Denied)
            };
        }
        for reference in token.proofs().as_slice() {
            *remaining = remaining.checked_sub(1).ok_or(Denied)?;
            let proof = boundaries
                .proof_store
                .resolve_proof(reference)
                .map_err(|_| Denied)?;
            let proof_token = match &proof {
                ucan::ProofToken::Borrowed(token) => *token,
                ucan::ProofToken::Owned(token) => token,
            };
            if ucan::proof_reference(proof_token) != *reference {
                return Err(Denied);
            }
            let raw = match &proof {
                ucan::ProofToken::Borrowed(token) => token.as_str(),
                ucan::ProofToken::Owned(token) => token.as_str(),
            };
            let parent = verify_compact_token_str_with_resolvers_and_revocations(
                raw,
                boundaries.verification_time,
                boundaries.key_resolver,
                boundaries.proof_store,
                boundaries.revocations,
                boundaries.verification_limits,
            )
            .map_err(|_| Denied)?;
            self.check_roots(&parent, boundaries, remaining)?;
        }
        Ok(())
    }
}

//! Reviewed Mantle holder-invocation boundary. Evidence DTOs cannot mint sessions.
//!
//! Every request field is length-framed into a canonical resource. The resource
//! and distinct ability are covered by both the UCAN grant and holder signature.
//! Callers own keys, proof loading, revocation currentness and durable replay.
use crate::authority_core::BasaltPolicyRef;
use crate::ucan_enforcement::core::ExactOperationProfile;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    BuildSubmit,
    InputUpload,
    StoreRead,
    ProjectDeclare,
    StatusRead,
    LogRead,
    EventRead,
    ResultRead,
    UsageRead,
    OwnedAttemptCancel,
    Publish,
    Admin,
}
impl Operation {
    /// Return the distinct reviewed action; no action implies another grant.
    #[must_use]
    pub const fn action(self) -> EnforcementAction {
        match self {
            Self::BuildSubmit => EnforcementAction::MantleBuildSubmit,
            Self::InputUpload => EnforcementAction::MantleInputUpload,
            Self::StoreRead => EnforcementAction::MantleStoreRead,
            Self::ProjectDeclare => EnforcementAction::MantleProjectDeclare,
            Self::StatusRead => EnforcementAction::MantleStatusRead,
            Self::LogRead => EnforcementAction::MantleLogRead,
            Self::EventRead => EnforcementAction::MantleEventRead,
            Self::ResultRead => EnforcementAction::MantleResultRead,
            Self::UsageRead => EnforcementAction::MantleUsageRead,
            Self::OwnedAttemptCancel => EnforcementAction::MantleOwnedAttemptCancel,
            Self::Publish => EnforcementAction::MantlePublish,
            Self::Admin => EnforcementAction::MantleAdmin,
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
    pub account: String,
    pub project: String,
    pub endpoint: String,
    pub audience: String,
    pub generation: u64,
    pub request_digest: [u8; 32],
    pub target: String,
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
        f.write_str("gateway-authority-denied")
    }
}
impl std::error::Error for Denied {}

/// Non-deserializable, non-cloneable authority minted only after final replay.
/// Do not persist this value or treat its redacted projections as live authority.
pub struct Session {
    request: Request,
    token: ucan::CompactToken,
    holder_authorization: ucan::HolderInvocationAuthorized,
    verified: ucan::VerifiedToken,
    policy_ref: String,
    expires_unix_s: u64,
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
        self.verified.effective_delegation().capabilities()
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
    /// Check the exact host operation before it reaches any effect.
    ///
    /// # Errors
    /// Returns `Denied` for any operation, target or normalized digest mismatch.
    pub fn require(
        &self,
        operation: Operation,
        target: &str,
        digest: [u8; 32],
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
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MantleSession").finish_non_exhaustive()
    }
}

/// Operator-loaded policy/configuration, never caller-supplied admission facts.
/// The artifact reference must name these exact reviewed policy bytes.
pub struct Verifier<'a> {
    pub policy: &'a Policy,
    pub policy_ref: &'a BasaltPolicyRef,
    pub generation: u64,
    pub endpoint: &'a str,
    pub audience: &'a str,
    /// Operator-admitted root issuers; holder keys are NOT issuer authority.
    pub trusted_roots: &'a [String],
}
impl Verifier<'_> {
    /// Public canonicalization shared by the issuer and holder signer.
    ///
    /// # Errors
    /// Returns `Denied` for stale deployment bindings or malformed/bounded fields.
    pub fn enforcement(&self, request: &Request) -> Result<EnforcementRequest, Denied> {
        if request.generation != self.generation
            || self.generation == 0
            || request.endpoint != self.endpoint
            || request.audience != self.audience
        {
            return Err(Denied);
        }
        let action = request.operation.action();
        let mut resource = String::from("mantle://gateway/v1/");
        for field in [
            request.subject.as_str(),
            request.account.as_str(),
            request.project.as_str(),
            request.endpoint.as_str(),
            request.audience.as_str(),
            self.policy_ref.artifact_ref.as_str(),
            request.target.as_str(),
        ] {
            if field.is_empty()
                || field.len() > 512
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
        write!(&mut resource, "{}/", request.generation).map_err(|_| Denied)?;
        for byte in request.request_digest {
            write!(&mut resource, "{byte:02x}").map_err(|_| Denied)?;
        }
        if resource.len() > 4096 {
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
        let (holder_authorization, receipt) = live.into_parts();
        let expires_unix_s = holder_authorization
            .effective_time_bounds()
            .expires_at_seconds();
        Ok(Session {
            request,
            token,
            holder_authorization,
            verified,
            policy_ref: selected.artifact_ref,
            expires_unix_s,
            decision_id: receipt.receipt_blake3,
        })
    }

    /// Revalidate signatures, full proof attenuation, time, current revocations,
    /// caveats and loaded policy on EVERY dispatch. This does not re-spend replay:
    /// possession of Session proves its invocation already passed final admission.
    ///
    /// # Errors
    /// Returns `Denied` when any current authority binding no longer holds.
    pub fn revalidate(
        &self,
        session: &Session,
        boundaries: &UcanEnforcementBoundaries<'_>,
    ) -> Result<(), Denied> {
        if session.policy_ref != self.policy_ref.artifact_ref {
            return Err(Denied);
        }
        let enforcement = self.enforcement(&session.request)?;
        let operation = crate::ucan_enforcement::bind_enforcement_action(
            session.request.operation.action(),
            &enforcement,
        )
        .map_err(|_| Denied)?;
        let profile = ExactOperationProfile {
            expected: operation.profile(),
        };
        let verification = ucan::HolderTokenVerificationContext::new(
            boundaries.verification_time,
            boundaries.key_resolver,
            boundaries.proof_store,
            boundaries.revocations,
            boundaries.verification_limits,
        );
        ucan::recheck_holder_authorization(
            &session.holder_authorization,
            session.token.as_str(),
            &verification,
            &ucan::HolderRecheckPolicies {
                caveats: boundaries.caveat_policies,
                profiles: &profile,
            },
        )
        .map_err(|_| Denied)?;
        self.check_roots(
            &session.verified,
            boundaries,
            &mut boundaries.verification_limits.max_proof_count(),
        )?;
        if session.verified.audience().as_str() != session.request.subject {
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

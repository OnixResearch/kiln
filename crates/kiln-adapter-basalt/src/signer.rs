//! Role-separated holder signing for the verified implementation profile.
//!
//! The coordinator, verifier and publisher each hold their own Ed25519 key and
//! their own grant. A request is signed only by the holder of the role that
//! owns its operation ([`GrantRole::of`]), so a compromised coordinator key
//! cannot sign verification, publication, integration or lifecycle requests:
//! those grants name a different holder DID.

use basalt::authority_core::BasaltPolicyRef;
use basalt::kiln::{Request, Verifier};
use basalt::ucan_enforcement::bind_enforcement_action;
use kiln_core::implementation::authority::{
    AUTHORITY_NONCE_BYTES, AUTHORITY_SIGNATURE_BYTES, AuthorityCredential, AuthorityRequest,
    HolderInvocationSigner,
};
use kiln_core::implementation::grant::GrantRole;
use kiln_core::types::{CoreResult, Diagnostic};
use ucan::{
    AudienceDid, ED25519_SECRET_KEY_BYTES, Ed25519InMemorySigner, HolderInvocationLimits,
    HolderInvocationNonce, HolderInvocationSigningRequest, TokenSigner, VerificationLimits,
    sign_holder_invocation,
};

use crate::{VerifiedAuthorityConfig, basalt_operation};

/// Source of fresh single-use holder nonces.
pub type NonceSource = Box<dyn Fn() -> Option<[u8; AUTHORITY_NONCE_BYTES]>>;

/// One role's signing key and the compact grant issued to its DID.
pub struct HolderKey {
    role: GrantRole,
    signer: Ed25519InMemorySigner,
    did: String,
    token: String,
}

impl std::fmt::Debug for HolderKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HolderKey")
            .field("role", &self.role)
            .field("did", &self.did)
            .field("token_bytes", &self.token.len())
            .finish_non_exhaustive()
    }
}

impl HolderKey {
    /// Bind one role to its Ed25519 seed and compact grant.
    ///
    /// # Errors
    /// Returns a message when the DID cannot be derived or the grant is empty.
    pub fn new(
        role: GrantRole,
        seed: [u8; ED25519_SECRET_KEY_BYTES],
        token: String,
    ) -> Result<Self, String> {
        let signer = Ed25519InMemorySigner::from_seed_bytes(seed);
        let did = signer
            .issuer()
            .map_err(|_| "holder DID cannot be derived".to_string())?
            .as_str()
            .to_string();
        if token.is_empty() || !token.is_ascii() {
            return Err("holder grant must be a non-empty compact token".into());
        }
        Ok(Self {
            role,
            signer,
            did,
            token,
        })
    }

    /// DID of the role holder.
    #[must_use]
    pub fn did(&self) -> &str {
        &self.did
    }

    /// Role this key signs for.
    #[must_use]
    pub const fn role(&self) -> GrantRole {
        self.role
    }
}

/// Fresh nonces from the operating system.
#[must_use]
pub fn os_nonce() -> Option<[u8; AUTHORITY_NONCE_BYTES]> {
    let mut nonce = [0; AUTHORITY_NONCE_BYTES];
    getrandom::fill(&mut nonce).ok()?;
    Some(nonce)
}

// r[impl kiln.implementation.authority.selection]
/// Signs each exact request as the holder of the request's role.
pub struct RoleSigner {
    policy: basalt::Policy,
    policy_reference: BasaltPolicyRef,
    generation: u64,
    audience: String,
    holders: Vec<HolderKey>,
    nonce: NonceSource,
}

impl std::fmt::Debug for RoleSigner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RoleSigner")
            .field("generation", &self.generation)
            .field("audience", &self.audience)
            .field("holders", &self.holders)
            .finish_non_exhaustive()
    }
}

impl RoleSigner {
    /// Build the signer over the verifier's exact policy and deployment binding.
    ///
    /// # Errors
    /// Returns a message when two keys claim one role or one DID.
    pub fn new(
        config: &VerifiedAuthorityConfig,
        holders: Vec<HolderKey>,
        nonce: NonceSource,
    ) -> Result<Self, String> {
        for (index, holder) in holders.iter().enumerate() {
            if holders
                .iter()
                .skip(index + 1)
                .any(|other| other.role == holder.role || other.did == holder.did)
            {
                return Err("each role needs its own distinct holder key".into());
            }
        }
        Ok(Self {
            policy: config.policy.clone(),
            policy_reference: config.policy_reference.clone(),
            generation: config.generation,
            audience: config.audience.clone(),
            holders,
            nonce,
        })
    }

    /// DID of the holder that signs for one role.
    #[must_use]
    pub fn holder_did(&self, role: GrantRole) -> Option<&str> {
        self.holders
            .iter()
            .find(|holder| holder.role == role)
            .map(HolderKey::did)
    }
}

impl HolderInvocationSigner for RoleSigner {
    fn sign(&self, request: &AuthorityRequest) -> CoreResult<AuthorityCredential> {
        let denied = |message: &str| Diagnostic::new("authority_holder_signer", "holder", message);
        let role = GrantRole::of(request.operation);
        let holder = self
            .holders
            .iter()
            .find(|holder| holder.role == role)
            .ok_or_else(|| denied("no holder key is configured for this role"))?;
        let exact = Request {
            operation: basalt_operation(request.operation),
            subject: holder.did.clone(),
            audience: self.audience.clone(),
            project: request.project.clone(),
            work: request.work.clone(),
            work_version: request.work_version,
            attempt: request.attempt.clone(),
            fence: request.fence,
            generation: self.generation,
            request_digest: request.request_digest,
            target: request.target.clone(),
            candidate: request.candidate,
        };
        let operation = exact.operation;
        let enforcement = Verifier {
            policy: &self.policy,
            policy_ref: &self.policy_reference,
            generation: self.generation,
            audience: &self.audience,
            trusted_roots: &[],
        }
        .enforcement(&exact)
        .map_err(|_| denied("request has no canonical resource under the policy"))?;
        let binding = bind_enforcement_action(operation.action(), &enforcement)
            .map_err(|_| denied("operation binding refused"))?;
        let nonce = (self.nonce)().ok_or_else(|| denied("no fresh nonce is available"))?;
        let audience = AudienceDid::from(
            holder
                .signer
                .issuer()
                .map_err(|_| denied("holder DID cannot be derived"))?,
        );
        let signature = sign_holder_invocation(
            &HolderInvocationSigningRequest::new(
                &holder.token,
                &audience,
                enforcement.resource(),
                enforcement.ability(),
                binding,
                HolderInvocationNonce::from_bytes(nonce),
            ),
            &holder.signer,
            VerificationLimits::default(),
            HolderInvocationLimits::default(),
        )
        .map_err(|_| denied("holder invocation cannot be signed"))?;
        let signature: [u8; AUTHORITY_SIGNATURE_BYTES] = signature
            .as_bytes()
            .try_into()
            .map_err(|_| denied("holder signature width"))?;
        AuthorityCredential::new(holder.did.clone(), holder.token.clone(), nonce, signature)
    }

    fn holds(&self, role: GrantRole) -> bool {
        self.holder_did(role).is_some()
    }
}

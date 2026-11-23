//! Positive signed-token fixtures for the verified implementation profile.
//!
//! Only compiled with the `test-support` feature. It issues roots,
//! delegations and role grants in-process with the vendored
//! `issue_token_with_signer`, and signs holder invocations through the
//! production [`RoleSigner`], mirroring Basalt's own Kiln fixtures. Keys are
//! fixed test seeds; nothing here is production key custody.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::rc::Rc;

use kiln_core::implementation::authority::{
    AUTHORITY_NONCE_BYTES, AuthorityCredential, AuthorityRequest, HolderInvocationSigner,
    ProtectedOperation,
};
use kiln_core::implementation::grant::GrantRole;
use kiln_core::types::CoreResult;
use ucan::token::DenyUnknownCaveats;
use ucan::{
    AudienceDid, CapabilityDocument, CapabilitySet, ED25519_SECRET_KEY_BYTES,
    Ed25519InMemorySigner, HolderInvocationLimits, HolderInvocationReplayError,
    KeyResolutionContext, ProofCollection, ProofReference, ProofReferences, RevocationError,
    TokenSigner, TokenTimeBounds, VerificationLimits, issue_token_with_signer, proof_reference,
};

use crate::signer::NonceSource;
use crate::{
    AuthorityServices, BasaltAuthority, HolderKey, REPLAY_DIGEST_BYTES, ReplayLedger, ReplayRecord,
    RevocationView, RoleSigner, VerifiedAuthorityConfig, basalt_operation,
};

/// Root issuer seed byte.
pub const ROOT_SEED: u8 = 11;
/// Intermediate delegate seed byte.
pub const DELEGATE_SEED: u8 = 13;
/// Coordinator holder seed byte.
pub const COORDINATOR_SEED: u8 = 17;
/// Verifier holder seed byte.
pub const VERIFIER_SEED: u8 = 19;
/// Publisher holder seed byte.
pub const PUBLISHER_SEED: u8 = 23;
/// Issuer that is not an admitted root.
pub const UNTRUSTED_ROOT_SEED: u8 = 31;
/// Deployment audience.
pub const AUDIENCE: &str = "kiln-test-audience";
/// Deployment authority generation.
pub const GENERATION: u64 = 7;
/// Grant validity start, Unix seconds.
pub const NOT_BEFORE_S: u64 = 1_000;
/// Grant expiry, Unix seconds.
pub const EXPIRES_S: u64 = 4_000;
/// A verification time inside the grant window, Unix seconds.
pub const VERIFY_AT_S: u64 = 1_500;
/// A verification time after the grant window, Unix seconds.
pub const EXPIRED_AT_S: u64 = 5_000;
/// Replay retention used by the fixtures.
pub const REPLAY_RETENTION_MS: u64 = 60_000;
/// Every operation in the verified profile.
pub const KILN_RESOURCE_ROOT: &str = "kiln://implementation/v1/";

/// Fixed test seed.
#[must_use]
pub const fn seed(byte: u8) -> [u8; ED25519_SECRET_KEY_BYTES] {
    [byte; ED25519_SECRET_KEY_BYTES]
}

/// Signer for one test seed.
#[must_use]
pub fn signer(byte: u8) -> Ed25519InMemorySigner {
    Ed25519InMemorySigner::from_seed_bytes(seed(byte))
}

/// DID of one test seed.
///
/// # Panics
/// Never for the fixed seeds.
#[must_use]
pub fn did(byte: u8) -> String {
    signer(byte)
        .issuer()
        .unwrap_or_else(|_| unreachable!("fixed test seed derives a DID"))
        .as_str()
        .to_string()
}

/// Public key of one test seed.
///
/// # Panics
/// Never for the fixed seeds.
#[must_use]
pub fn public_key(byte: u8) -> [u8; 32] {
    ucan::verified::derive_ed25519_public_key(&seed(byte))
        .unwrap_or_else(|_| unreachable!("fixed test seed derives a key"))
}

/// Holder seed of one role.
#[must_use]
pub const fn role_seed(role: GrantRole) -> u8 {
    match role {
        GrantRole::Coding => COORDINATOR_SEED,
        GrantRole::Verification => VERIFIER_SEED,
        GrantRole::Publication => PUBLISHER_SEED,
    }
}

/// Every operation one role owns.
#[must_use]
pub fn role_operations(role: GrantRole) -> Vec<ProtectedOperation> {
    ProtectedOperation::ALL
        .into_iter()
        .filter(|operation| GrantRole::of(*operation) == role)
        .collect()
}

/// Basalt resource prefix that scopes a grant to one holder and audience.
#[must_use]
pub fn holder_prefix(holder_did: &str, audience: &str) -> String {
    let mut resource = String::from(KILN_RESOURCE_ROOT);
    for field in [holder_did, audience] {
        let _ = write!(resource, "{}/", field.len());
        for byte in field.bytes() {
            let _ = write!(resource, "{byte:02x}");
        }
        resource.push('/');
    }
    resource
}

fn capabilities(resource: &str, operations: &[ProtectedOperation]) -> CapabilitySet {
    CapabilitySet::new(
        operations
            .iter()
            .map(|operation| {
                CapabilityDocument::new(
                    resource.to_string(),
                    basalt_operation(*operation).action().ability().to_string(),
                )
                .unwrap_or_else(|_| unreachable!("fixture capability"))
            })
            .collect(),
    )
    .unwrap_or_else(|_| unreachable!("fixture capability set"))
}

/// Issue one compact token from `issuer` to `audience_seed`'s DID.
#[must_use]
pub fn issue(
    issuer: u8,
    audience_seed: u8,
    resource: &str,
    operations: &[ProtectedOperation],
    proofs: &[&str],
    expires_s: u64,
) -> String {
    let audience = AudienceDid::from(
        signer(audience_seed)
            .issuer()
            .unwrap_or_else(|_| unreachable!("fixed audience DID")),
    );
    let references = proofs
        .iter()
        .map(|token| {
            proof_reference(
                &ucan::CompactToken::new(token).unwrap_or_else(|_| unreachable!("proof token")),
            )
        })
        .collect();
    issue_token_with_signer(
        &signer(issuer),
        &audience,
        &capabilities(resource, operations),
        &ProofReferences::from_references(references),
        TokenTimeBounds::new(NOT_BEFORE_S, expires_s)
            .unwrap_or_else(|_| unreachable!("fixture time bounds")),
    )
    .unwrap_or_else(|_| unreachable!("fixture token issues"))
    .as_str()
    .to_string()
}

/// BLAKE3 proof reference of one compact token.
#[must_use]
pub fn token_ref(token: &str) -> [u8; REPLAY_DIGEST_BYTES] {
    *blake3::hash(token.as_bytes()).as_bytes()
}

/// In-memory insert-or-deny replay ledger with an observable call count.
#[derive(Clone, Debug, Default)]
pub struct MemoryReplay {
    seen: Rc<RefCell<BTreeSet<(String, u64, [u8; 32], [u8; 32])>>>,
    calls: Rc<Cell<u32>>,
}

impl MemoryReplay {
    /// Consuming calls observed so far, admitted or denied.
    #[must_use]
    pub fn calls(&self) -> u32 {
        self.calls.get()
    }

    /// Admitted records.
    #[must_use]
    pub fn admitted(&self) -> usize {
        self.seen.borrow().len()
    }
}

impl ReplayLedger for MemoryReplay {
    fn consume(
        &self,
        record: &ReplayRecord<'_>,
        _now_ms: u64,
    ) -> Result<(), HolderInvocationReplayError> {
        self.calls.set(self.calls.get().saturating_add(1));
        let key = (
            record.audience.to_string(),
            record.generation,
            record.token_ref,
            record.nonce,
        );
        if self.seen.borrow_mut().insert(key) {
            Ok(())
        } else {
            Err(HolderInvocationReplayError::Duplicate)
        }
    }
}

/// In-memory revocation view; `stale` makes every check fail closed.
#[derive(Clone, Debug, Default)]
pub struct MemoryRevocations {
    revoked: Rc<RefCell<BTreeSet<Vec<u8>>>>,
    stale: Rc<Cell<bool>>,
}

impl MemoryRevocations {
    /// Revoke one compact token.
    pub fn revoke(&self, token: &str) {
        self.revoked.borrow_mut().insert(token_ref(token).to_vec());
    }

    /// Mark the view stale or fresh.
    pub fn set_stale(&self, stale: bool) {
        self.stale.set(stale);
    }
}

impl RevocationView for MemoryRevocations {
    fn is_revoked(
        &self,
        reference: &ProofReference,
        _now_ms: u64,
    ) -> Result<bool, RevocationError> {
        if self.stale.get() {
            return Err(RevocationError::Stale {
                reference: reference.clone(),
                message: "fixture revocation view is stale".into(),
            });
        }
        Ok(self.revoked.borrow().contains(reference.as_bytes()))
    }
}

/// Deterministic distinct nonces: `[n; 32]` for n = 1, 2, ...
#[must_use]
pub fn counting_nonces() -> NonceSource {
    counting_nonces_from(1)
}

/// Deterministic distinct nonces `[n; 32]` starting at `start`, so a
/// restarted fixture process never reuses an earlier process's nonce.
#[must_use]
pub fn counting_nonces_from(start: u8) -> NonceSource {
    let next = Cell::new(start.saturating_sub(1));
    Box::new(move || {
        let value = next.get().checked_add(1)?;
        next.set(value);
        Some([value; AUTHORITY_NONCE_BYTES])
    })
}

/// A complete verified-profile fixture: config, grants, shared replay and
/// revocation state.
#[derive(Debug)]
pub struct Fixture {
    /// Admitted root issuers.
    pub roots: Vec<String>,
    /// Grant per role.
    pub grants: Vec<(GrantRole, String)>,
    /// Proof tokens served by the proof store.
    pub proofs: Vec<String>,
    /// Shared replay ledger.
    pub replay: MemoryReplay,
    /// Shared revocation view.
    pub revocations: MemoryRevocations,
}

impl Fixture {
    /// Root-issued grants, one per role, scoped to the role holder and audience.
    #[must_use]
    pub fn direct() -> Self {
        Self::direct_from(ROOT_SEED, EXPIRES_S)
    }

    /// Root-issued grants from `issuer`, expiring at `expires_s`.
    #[must_use]
    pub fn direct_from(issuer: u8, expires_s: u64) -> Self {
        let grants = [
            GrantRole::Coding,
            GrantRole::Verification,
            GrantRole::Publication,
        ]
        .into_iter()
        .map(|role| {
            let holder = role_seed(role);
            (
                role,
                issue(
                    issuer,
                    holder,
                    &holder_prefix(&did(holder), AUDIENCE),
                    &role_operations(role),
                    &[],
                    expires_s,
                ),
            )
        })
        .collect();
        Self {
            roots: vec![did(ROOT_SEED)],
            grants,
            proofs: Vec::new(),
            replay: MemoryReplay::default(),
            revocations: MemoryRevocations::default(),
        }
    }

    /// Root grants the delegate every operation over the whole Kiln resource
    /// root; the delegate attenuates each role to its holder prefix.
    #[must_use]
    pub fn delegated() -> Self {
        let parent = issue(
            ROOT_SEED,
            DELEGATE_SEED,
            KILN_RESOURCE_ROOT,
            &ProtectedOperation::ALL,
            &[],
            EXPIRES_S,
        );
        let grants = [
            GrantRole::Coding,
            GrantRole::Verification,
            GrantRole::Publication,
        ]
        .into_iter()
        .map(|role| {
            let holder = role_seed(role);
            (
                role,
                issue(
                    DELEGATE_SEED,
                    holder,
                    &holder_prefix(&did(holder), AUDIENCE),
                    &role_operations(role),
                    &[&parent],
                    EXPIRES_S,
                ),
            )
        })
        .collect();
        Self {
            roots: vec![did(ROOT_SEED)],
            grants,
            proofs: vec![parent],
            replay: MemoryReplay::default(),
            revocations: MemoryRevocations::default(),
        }
    }

    /// Grant of one role.
    #[must_use]
    pub fn grant(&self, role: GrantRole) -> &str {
        self.grants
            .iter()
            .find(|(candidate, _)| *candidate == role)
            .map_or("", |(_, token)| token.as_str())
    }

    /// Verified configuration over the packaged Basalt policy.
    ///
    /// # Panics
    /// Never for the fixed fixture values.
    #[must_use]
    pub fn config(&self) -> VerifiedAuthorityConfig {
        VerifiedAuthorityConfig::from_packaged_policy(GENERATION, AUDIENCE, self.roots.clone())
            .unwrap_or_else(|error| unreachable!("{error}"))
    }

    /// Static keys for every fixture participant.
    #[must_use]
    pub fn keys(&self) -> KeyResolutionContext {
        crate::compose::static_keys(
            &[
                ROOT_SEED,
                DELEGATE_SEED,
                COORDINATOR_SEED,
                VERIFIER_SEED,
                PUBLISHER_SEED,
                UNTRUSTED_ROOT_SEED,
            ]
            .map(|byte| (did(byte), public_key(byte))),
        )
        .unwrap_or_else(|error| unreachable!("{error}"))
    }

    /// Services sharing this fixture's replay and revocation state.
    #[must_use]
    pub fn services(&self) -> AuthorityServices {
        AuthorityServices {
            key_resolver: Box::new(self.keys()),
            proof_store: Box::new(
                crate::compose::proof_store(&self.proofs)
                    .unwrap_or_else(|error| unreachable!("{error}")),
            ),
            revocations: Box::new(self.revocations.clone()),
            caveat_policies: Box::new(DenyUnknownCaveats),
            replay: Box::new(self.replay.clone()),
            replay_retention_ms: REPLAY_RETENTION_MS,
            verification_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        }
    }

    /// Verified adapter over this fixture.
    ///
    /// # Panics
    /// Never for the fixed fixture values.
    #[must_use]
    pub fn authority(&self) -> BasaltAuthority {
        BasaltAuthority::verified(self.config(), self.services())
            .unwrap_or_else(|error| unreachable!("{error}"))
    }

    /// Role signer over this fixture's grants with deterministic nonces.
    ///
    /// # Panics
    /// Never for the fixed fixture values.
    #[must_use]
    pub fn signer(&self) -> RoleSigner {
        self.signer_from(1)
    }

    /// Role signer whose deterministic nonces start at `start`.
    ///
    /// # Panics
    /// Never for the fixed fixture values.
    #[must_use]
    pub fn signer_from(&self, start: u8) -> RoleSigner {
        let holders = self
            .grants
            .iter()
            .map(|(role, token)| {
                HolderKey::new(*role, seed(role_seed(*role)), token.clone())
                    .unwrap_or_else(|error| unreachable!("{error}"))
            })
            .collect();
        RoleSigner::new(&self.config(), holders, counting_nonces_from(start))
            .unwrap_or_else(|error| unreachable!("{error}"))
    }

    /// Proof collection for this fixture, for callers that build their own
    /// services.
    #[must_use]
    pub fn proof_collection(&self) -> ProofCollection {
        crate::compose::proof_store(&self.proofs).unwrap_or_else(|error| unreachable!("{error}"))
    }
}

/// One captured holder invocation, replayable byte for byte.
#[derive(Clone, Debug)]
pub struct Captured {
    /// Holder DID.
    pub subject: String,
    /// Compact grant.
    pub token: String,
    /// Nonce.
    pub nonce: [u8; AUTHORITY_NONCE_BYTES],
    /// Holder signature.
    pub signature: Vec<u8>,
    /// Operation of the signed request.
    pub operation: ProtectedOperation,
    /// Payload digest of the signed request.
    pub request_digest: [u8; 32],
}

impl Captured {
    /// Rebuild the exact credential.
    ///
    /// # Panics
    /// Never for a captured credential.
    #[must_use]
    pub fn credential(&self) -> AuthorityCredential {
        let signature: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("captured signature width"));
        AuthorityCredential::new(
            self.subject.clone(),
            self.token.clone(),
            self.nonce,
            signature,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"))
    }
}

/// Signer wrapper that records every credential it issues.
#[allow(missing_debug_implementations)]
pub struct CapturingSigner {
    inner: Box<dyn HolderInvocationSigner>,
    captured: Rc<RefCell<Vec<Captured>>>,
}

impl CapturingSigner {
    /// Wrap one signer; the returned handle observes every credential.
    #[must_use]
    pub fn new(inner: Box<dyn HolderInvocationSigner>) -> (Self, Rc<RefCell<Vec<Captured>>>) {
        let captured = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                inner,
                captured: Rc::clone(&captured),
            },
            captured,
        )
    }
}

impl HolderInvocationSigner for CapturingSigner {
    fn sign(&self, request: &AuthorityRequest) -> CoreResult<AuthorityCredential> {
        let credential = self.inner.sign(request)?;
        self.captured.borrow_mut().push(Captured {
            subject: credential.subject().to_string(),
            token: credential.compact_token().to_string(),
            nonce: *credential.nonce(),
            signature: credential.holder_signature().to_vec(),
            operation: request.operation,
            request_digest: request.request_digest,
        });
        Ok(credential)
    }
}

/// Signer that always presents one fixed captured credential.
#[derive(Debug)]
pub struct ReplayingSigner(pub Captured);

impl HolderInvocationSigner for ReplayingSigner {
    fn sign(&self, _request: &AuthorityRequest) -> CoreResult<AuthorityCredential> {
        Ok(self.0.credential())
    }
}

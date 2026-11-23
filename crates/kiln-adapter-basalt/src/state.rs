//! Durable authority-state ports owned by the composing shell.
//!
//! UCAN consumes replay as `HolderInvocationReplayAdmission` and revocations
//! as `RevocationChecker`, neither of which sees the token reference, the
//! deployment generation or the shell clock. These ports carry those facts so
//! a shell can keep replay and revocation in its own durable store; the
//! adapter binds them to UCAN's traits for one exact verification.

use ucan::{
    HolderInvocationReplayAdmission, HolderInvocationReplayCandidate, HolderInvocationReplayError,
    ProofReference, RevocationChecker, RevocationError,
};

/// Width of the token reference, nonce and canonical request digest.
pub const REPLAY_DIGEST_BYTES: usize = 32;

/// One holder invocation offered to the final consuming replay step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayRecord<'a> {
    /// UCAN holder audience, the holder DID.
    pub audience: &'a str,
    /// Deployment authority generation.
    pub generation: u64,
    /// BLAKE3 proof reference of the compact token.
    pub token_ref: [u8; REPLAY_DIGEST_BYTES],
    /// Holder-invocation nonce.
    pub nonce: [u8; REPLAY_DIGEST_BYTES],
    /// Canonical holder-request digest over the exact resource and ability.
    pub resource_digest: [u8; REPLAY_DIGEST_BYTES],
    /// Unix milliseconds after which the record may be pruned: never before
    /// the token itself expires.
    pub expires_ms: u64,
}

/// Durable insert-or-deny replay ledger.
pub trait ReplayLedger {
    /// Insert one record as the final consuming step, or deny a duplicate.
    ///
    /// # Errors
    /// Returns `Duplicate` for a recorded `(audience, generation, token_ref,
    /// nonce)` and `Backend` when the ledger cannot commit.
    fn consume(
        &self,
        record: &ReplayRecord<'_>,
        now_ms: u64,
    ) -> Result<(), HolderInvocationReplayError>;
}

/// Current revocation view that fails closed when it is stale.
pub trait RevocationView {
    /// Whether one proof reference is revoked at `now_ms`.
    ///
    /// `now_ms` is the last millisecond of the decision's whole-second
    /// verification time, so an age bound checked against it holds at every
    /// instant that second can stand for.
    ///
    /// # Errors
    /// Returns `Stale` when the view is older than its configured age and
    /// `Backend` when it cannot be read.
    fn is_revoked(&self, reference: &ProofReference, now_ms: u64) -> Result<bool, RevocationError>;
}

/// Replay port bound to one verification's token, generation and clock.
pub(crate) struct BoundReplay<'a> {
    pub(crate) ledger: &'a dyn ReplayLedger,
    pub(crate) generation: u64,
    pub(crate) token_ref: [u8; REPLAY_DIGEST_BYTES],
    pub(crate) expires_ms: u64,
    pub(crate) now_ms: u64,
}

impl HolderInvocationReplayAdmission for BoundReplay<'_> {
    fn admit_holder_invocation(
        &self,
        candidate: &HolderInvocationReplayCandidate<'_>,
    ) -> Result<(), HolderInvocationReplayError> {
        self.ledger.consume(
            &ReplayRecord {
                audience: candidate.audience().as_str(),
                generation: self.generation,
                token_ref: self.token_ref,
                nonce: *candidate.nonce().as_bytes(),
                resource_digest: *candidate.request_digest().as_bytes(),
                expires_ms: self.expires_ms,
            },
            self.now_ms,
        )
    }
}

/// Replay port for non-consuming revalidation: any consumption denies.
pub(crate) struct NoReplay;

impl HolderInvocationReplayAdmission for NoReplay {
    fn admit_holder_invocation(
        &self,
        _candidate: &HolderInvocationReplayCandidate<'_>,
    ) -> Result<(), HolderInvocationReplayError> {
        Err(HolderInvocationReplayError::Denied)
    }
}

/// Revocation port bound to one verification's clock.
pub(crate) struct BoundRevocations<'a> {
    pub(crate) view: &'a dyn RevocationView,
    pub(crate) now_ms: u64,
}

impl RevocationChecker for BoundRevocations<'_> {
    fn is_revoked(&self, reference: &ProofReference) -> Result<bool, RevocationError> {
        self.view.is_revoked(reference, self.now_ms)
    }
}

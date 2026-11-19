//! Durable replay and revocation adapters for caller-owned authority state.
//!
//! The adapters translate UCAN shell contracts into the separately published
//! replay-ledger and revocation-view cores. They do not own storage, clocks,
//! recovery, authenticity, authorization, or protected effects.
// r[impl ucan.durable_authority.source]
// r[impl ucan.durable_authority.source.exact]
// r[verify ucan.durable_authority.source.reject]
// r[impl ucan.durable_authority.replay_adapter]
// r[verify ucan.durable_authority.replay_adapter.first]
// r[verify ucan.durable_authority.replay_adapter.deny]
// r[impl ucan.durable_authority.revocation_adapter]
// r[verify ucan.durable_authority.revocation_adapter.current]
// r[verify ucan.durable_authority.revocation_adapter.unknown]
// r[impl ucan.durable_authority.ordering]
// r[verify ucan.durable_authority.ordering.success]
// r[verify ucan.durable_authority.ordering.denied]
// r[impl ucan.durable_authority.claims]
// r[verify ucan.durable_authority.claims.bounded]
// r[verify ucan.durable_authority.claims.overclaim]
// r[impl ucan.durable_authority.validation]
// r[verify ucan.durable_authority.validation.pass]
// r[verify ucan.durable_authority.validation.fail]
// r[impl ucan.durable_authority.lifecycle]
// r[verify ucan.durable_authority.lifecycle.complete]
// r[verify ucan.durable_authority.lifecycle.incomplete]

use std::fmt;

use replay_ledger_core::ActorDigest;
use replay_ledger_core::AttemptIdentity;
use replay_ledger_core::AuthorityGeneration;
use replay_ledger_core::CommitObservation as ReplayCommitObservation;
use replay_ledger_core::ConsumptionDecision;
use replay_ledger_core::DurabilityObservation as ReplayDurabilityObservation;
use replay_ledger_core::LedgerNamespaceDigest;
use replay_ledger_core::NonceDigest;
use replay_ledger_core::OperationProfileDigest;
use replay_ledger_core::ReplayLedgerPort;
use replay_ledger_core::ReplayScope;
use replay_ledger_core::ReplayScopeInput;
use replay_ledger_core::RequestDigest;
use replay_ledger_core::TenantDigest;
use replay_ledger_core::plan_consumption;
use revocation_view_core::CurrentnessDecision;
use revocation_view_core::CurrentnessRequirement;
use revocation_view_core::LookupDecision;
use revocation_view_core::RevocationEntryDigest;
use revocation_view_core::RevocationViewPort;
use revocation_view_core::SnapshotObservation;
use revocation_view_core::SourceDigest;
use revocation_view_core::SourceGeneration;
use revocation_view_core::UnixTime;
use revocation_view_core::lookup;

use crate::AudienceDid;
use crate::HolderInvocationReplayAdmission;
use crate::HolderInvocationReplayCandidate;
use crate::HolderInvocationReplayError;
use crate::ProofReference;
use crate::RevocationChecker;
use crate::RevocationError;

const ACTOR_DIGEST_DOMAIN: &[u8] = b"ucan/durable-authority/actor/v1";
const NONCE_DIGEST_DOMAIN: &[u8] = b"ucan/durable-authority/nonce/v1";
const REVOCATION_ENTRY_DOMAIN: &[u8] = b"ucan/durable-authority/revocation-entry/v1";

/// Role that failed durable-authority context admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableAuthorityField {
    /// Consumer-defined tenant digest.
    Tenant,
    /// Consumer-defined operation-profile digest.
    OperationProfile,
    /// Consumer-defined ledger namespace digest.
    LedgerNamespace,
    /// Protected-effect attempt identity.
    Attempt,
    /// Replay authority generation.
    AuthorityGeneration,
    /// Revocation source digest.
    RevocationSource,
    /// Minimum revocation source generation.
    SourceGeneration,
}

/// Invalid explicit context supplied to a durable-authority adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableAuthorityConfigError {
    /// A required digest used the all-zero missing sentinel.
    MissingDigest {
        /// Rejected role.
        field: DurableAuthorityField,
    },
    /// A required generation was zero.
    MissingGeneration {
        /// Rejected role.
        field: DurableAuthorityField,
    },
}

impl fmt::Display for DurableAuthorityConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDigest { field } => write!(formatter, "missing durable-authority digest for {field:?}"),
            Self::MissingGeneration { field } => {
                write!(formatter, "missing durable-authority generation for {field:?}")
            }
        }
    }
}

impl std::error::Error for DurableAuthorityConfigError {}

/// Minimum replay commit observation accepted by one consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableReplayDurability {
    /// Accept a logical commit with or without synchronization evidence.
    CommitObserved,
    /// Require the adapter to observe synchronization.
    Synchronized,
}

/// Explicit replay context before admission into shared role types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurableReplayContextInput {
    /// Consumer-defined tenant digest.
    pub tenant: [u8; replay_ledger_core::DIGEST_BYTES],
    /// Consumer-defined operation-profile digest.
    pub operation_profile: [u8; replay_ledger_core::DIGEST_BYTES],
    /// Positive authority generation.
    pub authority_generation: u64,
    /// Consumer-defined ledger namespace digest.
    pub ledger_namespace: [u8; replay_ledger_core::DIGEST_BYTES],
    /// Protected-effect attempt identity.
    pub attempt: [u8; replay_ledger_core::DIGEST_BYTES],
    /// Minimum accepted commit observation.
    pub durability: DurableReplayDurability,
}

/// Admitted caller-owned replay context for one holder invocation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurableReplayContext {
    tenant: TenantDigest,
    operation_profile: OperationProfileDigest,
    authority_generation: AuthorityGeneration,
    ledger_namespace: LedgerNamespaceDigest,
    attempt: AttemptIdentity,
    durability: DurableReplayDurability,
}

impl DurableReplayContext {
    /// Admit explicit consumer-owned replay roles.
    ///
    /// # Errors
    /// Returns [`DurableAuthorityConfigError`] when a digest or generation is missing.
    pub fn new(input: DurableReplayContextInput) -> Result<Self, DurableAuthorityConfigError> {
        Ok(Self {
            tenant: TenantDigest::new(input.tenant).map_err(|_| missing_digest(DurableAuthorityField::Tenant))?,
            operation_profile: OperationProfileDigest::new(input.operation_profile)
                .map_err(|_| missing_digest(DurableAuthorityField::OperationProfile))?,
            authority_generation: AuthorityGeneration::new(input.authority_generation)
                .map_err(|_| missing_generation(DurableAuthorityField::AuthorityGeneration))?,
            ledger_namespace: LedgerNamespaceDigest::new(input.ledger_namespace)
                .map_err(|_| missing_digest(DurableAuthorityField::LedgerNamespace))?,
            attempt: AttemptIdentity::new(input.attempt).map_err(|_| missing_digest(DurableAuthorityField::Attempt))?,
            durability: input.durability,
        })
    }
}

/// UCAN replay adapter over one caller-owned shared replay port.
pub struct DurableHolderReplayAdmission<'a, Port> {
    port: &'a Port,
    context: DurableReplayContext,
}

impl<'a, Port> DurableHolderReplayAdmission<'a, Port> {
    /// Bind a caller-owned port and already admitted context.
    #[must_use]
    pub const fn new(port: &'a Port, context: DurableReplayContext) -> Self {
        Self { port, context }
    }
}

impl<Port> HolderInvocationReplayAdmission for DurableHolderReplayAdmission<'_, Port>
where Port: ReplayLedgerPort
{
    fn admit_holder_invocation(
        &self,
        candidate: &HolderInvocationReplayCandidate<'_>,
    ) -> Result<(), HolderInvocationReplayError> {
        self.admit_parts(candidate.audience(), candidate.nonce().as_bytes(), candidate.request_digest().as_bytes())
    }
}

impl<Port> DurableHolderReplayAdmission<'_, Port>
where Port: ReplayLedgerPort
{
    fn admit_parts(
        &self,
        audience: &AudienceDid,
        nonce: &[u8],
        request_digest: &[u8; replay_ledger_core::DIGEST_BYTES],
    ) -> Result<(), HolderInvocationReplayError> {
        let actor = ActorDigest::new(domain_digest(ACTOR_DIGEST_DOMAIN, audience.as_str().as_bytes()))
            .map_err(|_| HolderInvocationReplayError::Malformed)?;
        let nonce = NonceDigest::new(domain_digest(NONCE_DIGEST_DOMAIN, nonce))
            .map_err(|_| HolderInvocationReplayError::Malformed)?;
        let request = RequestDigest::new(*request_digest).map_err(|_| HolderInvocationReplayError::Malformed)?;
        let scope = ReplayScope::new(ReplayScopeInput {
            tenant: self.context.tenant,
            actor,
            operation_profile: self.context.operation_profile,
            request,
            nonce,
            generation: self.context.authority_generation,
            namespace: self.context.ledger_namespace,
        });
        let observation = self.port.load(scope.slot_identity()).map_err(|_| HolderInvocationReplayError::Backend)?;
        match plan_consumption(scope, self.context.attempt, observation) {
            ConsumptionDecision::First { command } => {
                let committed =
                    self.port.compare_and_swap(command).map_err(|_| HolderInvocationReplayError::Backend)?;
                classify_replay_commit(committed, self.context.durability)
            }
            ConsumptionDecision::ExactDuplicate { .. } | ConsumptionDecision::Reserved { .. } => {
                Err(HolderInvocationReplayError::Duplicate)
            }
            ConsumptionDecision::Cancelled => Err(HolderInvocationReplayError::Stale),
            ConsumptionDecision::Uncertain | ConsumptionDecision::StateUnavailable => {
                Err(HolderInvocationReplayError::Backend)
            }
            ConsumptionDecision::Conflict => Err(HolderInvocationReplayError::Denied),
        }
    }
}

/// Explicit revocation-view requirements before shared role admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurableRevocationContextInput {
    /// Consumer-selected revocation source digest.
    pub source: [u8; revocation_view_core::DIGEST_BYTES],
    /// Oldest accepted source generation.
    pub minimum_generation: u64,
    /// Explicit caller-observed current Unix time.
    pub current_time_seconds: u64,
}

/// Admitted currentness requirements for one UCAN revocation adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurableRevocationContext {
    source: SourceDigest,
    minimum_generation: SourceGeneration,
    current_time: UnixTime,
}

impl DurableRevocationContext {
    /// Admit explicit revocation source and currentness requirements.
    ///
    /// # Errors
    /// Returns [`DurableAuthorityConfigError`] when source or generation is missing.
    pub fn new(input: DurableRevocationContextInput) -> Result<Self, DurableAuthorityConfigError> {
        Ok(Self {
            source: SourceDigest::new(input.source)
                .map_err(|_| missing_digest(DurableAuthorityField::RevocationSource))?,
            minimum_generation: SourceGeneration::new(input.minimum_generation)
                .map_err(|_| missing_generation(DurableAuthorityField::SourceGeneration))?,
            current_time: UnixTime::from_seconds(input.current_time_seconds),
        })
    }
}

/// UCAN revocation adapter over one caller-owned shared view port.
pub struct DurableRevocationChecker<'a, Port> {
    port: &'a Port,
    context: DurableRevocationContext,
}

impl<'a, Port> DurableRevocationChecker<'a, Port> {
    /// Bind a caller-owned port and admitted currentness context.
    #[must_use]
    pub const fn new(port: &'a Port, context: DurableRevocationContext) -> Self {
        Self { port, context }
    }
}

impl<Port> RevocationChecker for DurableRevocationChecker<'_, Port>
where Port: RevocationViewPort
{
    fn is_revoked(&self, reference: &ProofReference) -> Result<bool, RevocationError> {
        let observation = self
            .port
            .load(self.context.source)
            .map_err(|_| revocation_backend(reference, "revocation view load failed"))?;
        let stored = match observation {
            SnapshotObservation::Present(stored) => stored,
            SnapshotObservation::Empty | SnapshotObservation::Unavailable => {
                return Err(revocation_backend(reference, "revocation view is unavailable"));
            }
        };
        let entry = RevocationEntryDigest::new(durable_revocation_entry_digest(reference))
            .map_err(|_| revocation_malformed(reference, "revocation entry digest is missing"))?;
        let requirement = CurrentnessRequirement {
            source: self.context.source,
            minimum_generation: self.context.minimum_generation,
            current_time: self.context.current_time,
        };
        match lookup(stored.snapshot(), requirement, entry) {
            LookupDecision::Revoked => Ok(true),
            LookupDecision::NotRevoked => Ok(false),
            LookupDecision::Unknown(decision) => Err(map_currentness_error(reference, decision)),
        }
    }
}

/// Derive the exact shared revocation-entry bytes for one UCAN proof reference.
#[must_use]
pub fn durable_revocation_entry_digest(reference: &ProofReference) -> [u8; revocation_view_core::DIGEST_BYTES] {
    domain_digest(REVOCATION_ENTRY_DOMAIN, reference.as_bytes())
}

const fn missing_digest(field: DurableAuthorityField) -> DurableAuthorityConfigError {
    DurableAuthorityConfigError::MissingDigest { field }
}

const fn missing_generation(field: DurableAuthorityField) -> DurableAuthorityConfigError {
    DurableAuthorityConfigError::MissingGeneration { field }
}

fn classify_replay_commit(
    observation: ReplayCommitObservation,
    required: DurableReplayDurability,
) -> Result<(), HolderInvocationReplayError> {
    match observation {
        ReplayCommitObservation::Applied {
            durability: ReplayDurabilityObservation::Synchronized,
            ..
        }
        | ReplayCommitObservation::Applied {
            durability: ReplayDurabilityObservation::CommitOnly,
            ..
        } if required == DurableReplayDurability::CommitObserved => Ok(()),
        ReplayCommitObservation::Applied {
            durability: ReplayDurabilityObservation::Synchronized,
            ..
        } if required == DurableReplayDurability::Synchronized => Ok(()),
        ReplayCommitObservation::Applied { .. }
        | ReplayCommitObservation::Stale { .. }
        | ReplayCommitObservation::OutcomeUnknown => Err(HolderInvocationReplayError::Backend),
    }
}

fn map_currentness_error(reference: &ProofReference, decision: CurrentnessDecision) -> RevocationError {
    match decision {
        CurrentnessDecision::GenerationTooOld | CurrentnessDecision::NotYetEffective | CurrentnessDecision::Expired => {
            RevocationError::Stale {
                reference: reference.clone(),
                message: format!("revocation view is not current: {decision:?}"),
            }
        }
        CurrentnessDecision::WrongSource | CurrentnessDecision::Incomplete | CurrentnessDecision::Unauthenticated => {
            revocation_malformed(reference, format!("revocation view is not admissible: {decision:?}"))
        }
        CurrentnessDecision::Admitted => {
            revocation_backend(reference, "admitted currentness produced an unknown lookup")
        }
    }
}

fn revocation_malformed(reference: &ProofReference, message: impl Into<String>) -> RevocationError {
    RevocationError::Malformed {
        reference: reference.clone(),
        message: message.into(),
    }
}

fn revocation_backend(reference: &ProofReference, message: impl Into<String>) -> RevocationError {
    RevocationError::Backend {
        reference: reference.clone(),
        message: message.into(),
    }
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; blake3::OUT_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests;

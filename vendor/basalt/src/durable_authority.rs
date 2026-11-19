//! Basalt composition root for published durable replay and revocation adapters.
//!
//! The module owns no state transition, storage engine, clock, transaction,
//! authenticity policy, outage policy, or protected effect.

use replay_ledger_core::ReplayLedgerPort;
use revocation_view_core::RevocationViewPort;
use ucan::{
    CaveatPolicySet, DurableHolderReplayAdmission, DurableReplayContext, DurableRevocationChecker,
    DurableRevocationContext, HolderInvocationLimits, HolderInvocationReplayAdmission, KeyResolver,
    ProofStore, RevocationChecker, VerificationLimits, VerificationTime,
};

use crate::ucan_enforcement::UcanEnforcementBoundaries;

/// Explicit non-state UCAN inputs supplied by the Basalt application shell.
#[derive(Clone, Copy)]
pub struct DurableAuthorityVerificationInputs<'a> {
    /// Explicit verification time.
    pub verification_time: VerificationTime,
    /// Caller-owned key resolver.
    pub key_resolver: &'a dyn KeyResolver,
    /// Caller-owned proof store.
    pub proof_store: &'a dyn ProofStore,
    /// Non-consuming caveat policy observations.
    pub caveat_policies: &'a dyn CaveatPolicySet,
    /// Compact-token and proof limits.
    pub verification_limits: VerificationLimits,
    /// Holder canonical request limits.
    pub holder_limits: HolderInvocationLimits,
}

/// Visible composition root over the two separate published state adapters.
pub struct BasaltDurableAuthority<'a, ReplayPort, RevocationPort> {
    replay: DurableHolderReplayAdmission<'a, ReplayPort>,
    revocations: DurableRevocationChecker<'a, RevocationPort>,
}

impl<'a, ReplayPort, RevocationPort> BasaltDurableAuthority<'a, ReplayPort, RevocationPort> {
    /// Select caller-owned ports and admitted durable-authority contexts.
    #[must_use]
    pub const fn new(
        replay_port: &'a ReplayPort,
        replay_context: DurableReplayContext,
        revocation_port: &'a RevocationPort,
        revocation_context: DurableRevocationContext,
    ) -> Self {
        Self {
            replay: DurableHolderReplayAdmission::new(replay_port, replay_context),
            revocations: DurableRevocationChecker::new(revocation_port, revocation_context),
        }
    }
}

impl<ReplayPort, RevocationPort> BasaltDurableAuthority<'_, ReplayPort, RevocationPort>
where
    ReplayPort: ReplayLedgerPort,
    RevocationPort: RevocationViewPort,
{
    /// Return the final state-consuming UCAN replay boundary.
    #[must_use]
    pub const fn replay(&self) -> &dyn HolderInvocationReplayAdmission {
        &self.replay
    }

    /// Return the non-consuming UCAN revocation observation boundary.
    #[must_use]
    pub const fn revocations(&self) -> &dyn RevocationChecker {
        &self.revocations
    }

    /// Build Basalt's existing UCAN boundary bundle without moving authority.
    #[must_use]
    pub fn boundaries<'a>(
        &'a self,
        inputs: DurableAuthorityVerificationInputs<'a>,
    ) -> UcanEnforcementBoundaries<'a> {
        UcanEnforcementBoundaries {
            verification_time: inputs.verification_time,
            key_resolver: inputs.key_resolver,
            proof_store: inputs.proof_store,
            revocations: self.revocations(),
            caveat_policies: inputs.caveat_policies,
            replay: self.replay(),
            verification_limits: inputs.verification_limits,
            holder_limits: inputs.holder_limits,
        }
    }
}

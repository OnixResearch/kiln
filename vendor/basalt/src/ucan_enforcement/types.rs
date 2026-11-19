use super::core::operation_profile_digest;
use crate::EnforcementRequest;
use crate::ucan_profile::BasaltUcanProtocolProfile;
use serde::{Deserialize, Serialize};
use ucan::{
    CaveatPolicySet, HolderInvocationLimits, HolderInvocationNonce,
    HolderInvocationReplayAdmission, KeyResolver, OperationProfileId, ProofStore,
    RevocationChecker, SignerSignature, VerificationLimits, VerificationTime,
};

const DOCUMENT_READ_CONTRACT: &str = "document-read";
const DOCUMENT_READ_ABILITY: &str = "document/read";
const DOCUMENT_WRITE_CONTRACT: &str = "document-write";
const DOCUMENT_WRITE_ABILITY: &str = "document/write";
const PROCESS_EXECUTE_CONTRACT: &str = "process-execute";
const PROCESS_EXECUTE_ABILITY: &str = "process/execute";
const PROCESS_EXECUTION_RESOURCE_PREFIX: &str = "onix://process-execution/";

const TARGET_CANDIDATE_ACTIVATION_CONTRACT: &str = "onixos-target-candidate-activation";
const TARGET_CANDIDATE_ACTIVATION_ABILITY: &str = "onix/target-candidate-activation";
const TARGET_GENERATION_PERSISTENCE_CONTRACT: &str = "onixos-target-generation-persistence";
const TARGET_GENERATION_PERSISTENCE_ABILITY: &str = "onix/target-generation-persistence";
const TARGET_ROLLBACK_CONTRACT: &str = "onixos-target-rollback";
const TARGET_ROLLBACK_ABILITY: &str = "onix/target-rollback";
const STATE_SYNC_PUBLISH_CONTRACT: &str = "onixos-state-sync-publish";
const STATE_SYNC_PUBLISH_ABILITY: &str = "state_sync.publish";
const STATE_SYNC_DELETE_CONTRACT: &str = "onixos-state-sync-delete";
const STATE_SYNC_DELETE_ABILITY: &str = "state_sync.delete";
const STATE_SYNC_ADMIN_CONTRACT: &str = "onixos-state-sync-admin";
const STATE_SYNC_ADMIN_ABILITY: &str = "state_sync.admin";
const ONIXOS_TARGET_RESOURCE_PREFIX: &str = "did:key:z6Mkonix/onixos/";
const STATE_SYNC_RESOURCE_PREFIX: &str = "onix://state-sync/";
const KILN_RESOURCE_PREFIX: &str = "kiln://implementation/v1/";

/// Closed downstream actions with reviewed Basalt contract and UCAN ability mappings.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EnforcementAction {
    /// Read one document resource.
    DocumentRead,
    /// Write one document resource.
    DocumentWrite,
    /// Execute one independently admitted, content-bound process context.
    /// Requires explicit policy; the packaged default policy does not allow it.
    ProcessExecute,
    /// Candidate activation for one `OnixOS` target.
    OnixosTargetCandidateActivation,
    /// Generation persistence for one `OnixOS` target.
    OnixosTargetGenerationPersistence,
    /// Rollback for one `OnixOS` target.
    OnixosTargetRollback,
    /// State-sync publish for one `OnixOS` target.
    OnixosStateSyncPublish,
    /// State-sync delete for one `OnixOS` target.
    OnixosStateSyncDelete,
    /// State-sync administration for one `OnixOS` target.
    OnixosStateSyncAdmin,
    MantleBuildSubmit,
    MantleInputUpload,
    MantleStoreRead,
    MantleProjectDeclare,
    MantleStatusRead,
    MantleLogRead,
    MantleEventRead,
    MantleResultRead,
    MantleUsageRead,
    MantleOwnedAttemptCancel,
    MantlePublish,
    MantleAdmin,
    /// Acquire one fenced Kiln implementation claim.
    KilnClaimAcquire,
    /// Renew one fenced Kiln implementation claim.
    KilnClaimRenew,
    /// Dispatch one bound Kiln implementation attempt.
    KilnDispatch,
    /// Submit one sealed candidate for an exact Kiln attempt.
    KilnCandidateSubmit,
    /// Admit verification evidence for one exact Kiln candidate.
    KilnVerificationAdmit,
    /// Cancel one owned fenced Kiln attempt.
    KilnAttemptCancel,
    /// Publish one separately verified Kiln candidate.
    KilnPublicationPublish,
    /// Integrate one exact Kiln candidate through the selected route.
    KilnIntegrationLand,
    /// Complete native lifecycle for one integrated Kiln change.
    KilnLifecycleComplete,
}

impl EnforcementAction {
    /// Return the stable action identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentRead => "document-read",
            Self::DocumentWrite => "document-write",
            Self::ProcessExecute => "process-execute",
            Self::OnixosTargetCandidateActivation => "onixos-target-candidate-activation",
            Self::OnixosTargetGenerationPersistence => "onixos-target-generation-persistence",
            Self::OnixosTargetRollback => "onixos-target-rollback",
            Self::OnixosStateSyncPublish => "onixos-state-sync-publish",
            Self::OnixosStateSyncDelete => "onixos-state-sync-delete",
            Self::OnixosStateSyncAdmin => "onixos-state-sync-admin",
            Self::MantleBuildSubmit => "mantle-build-submit",
            Self::MantleInputUpload => "mantle-input-upload",
            Self::MantleStoreRead => "mantle-store-read",
            Self::MantleProjectDeclare => "mantle-project-declare",
            Self::MantleStatusRead => "mantle-status-read",
            Self::MantleLogRead => "mantle-log-read",
            Self::MantleEventRead => "mantle-event-read",
            Self::MantleResultRead => "mantle-result-read",
            Self::MantleUsageRead => "mantle-usage-read",
            Self::MantleOwnedAttemptCancel => "mantle-owned-attempt-cancel",
            Self::MantlePublish => "mantle-publish",
            Self::MantleAdmin => "mantle-admin",
            Self::KilnClaimAcquire => "kiln-claim-acquire",
            Self::KilnClaimRenew => "kiln-claim-renew",
            Self::KilnDispatch => "kiln-dispatch",
            Self::KilnCandidateSubmit => "kiln-candidate-submit",
            Self::KilnVerificationAdmit => "kiln-verification-admit",
            Self::KilnAttemptCancel => "kiln-attempt-cancel",
            Self::KilnPublicationPublish => "kiln-publication-publish",
            Self::KilnIntegrationLand => "kiln-integration-land",
            Self::KilnLifecycleComplete => "kiln-lifecycle-complete",
        }
    }

    /// Return the reviewed Basalt contract identifier.
    #[must_use]
    pub const fn contract_id(self) -> &'static str {
        match self {
            Self::DocumentRead => DOCUMENT_READ_CONTRACT,
            Self::DocumentWrite => DOCUMENT_WRITE_CONTRACT,
            Self::ProcessExecute => PROCESS_EXECUTE_CONTRACT,
            Self::OnixosTargetCandidateActivation => TARGET_CANDIDATE_ACTIVATION_CONTRACT,
            Self::OnixosTargetGenerationPersistence => TARGET_GENERATION_PERSISTENCE_CONTRACT,
            Self::OnixosTargetRollback => TARGET_ROLLBACK_CONTRACT,
            Self::OnixosStateSyncPublish => STATE_SYNC_PUBLISH_CONTRACT,
            Self::OnixosStateSyncDelete => STATE_SYNC_DELETE_CONTRACT,
            Self::OnixosStateSyncAdmin => STATE_SYNC_ADMIN_CONTRACT,
            Self::MantleBuildSubmit
            | Self::MantleInputUpload
            | Self::MantleStoreRead
            | Self::MantleProjectDeclare
            | Self::MantleStatusRead
            | Self::MantleLogRead
            | Self::MantleEventRead
            | Self::MantleResultRead
            | Self::MantleUsageRead
            | Self::MantleOwnedAttemptCancel
            | Self::MantlePublish
            | Self::MantleAdmin
            | Self::KilnClaimAcquire
            | Self::KilnClaimRenew
            | Self::KilnDispatch
            | Self::KilnCandidateSubmit
            | Self::KilnVerificationAdmit
            | Self::KilnAttemptCancel
            | Self::KilnPublicationPublish
            | Self::KilnIntegrationLand
            | Self::KilnLifecycleComplete => self.as_str(),
        }
    }

    /// Return the reviewed UCAN ability.
    #[must_use]
    pub const fn ability(self) -> &'static str {
        match self {
            Self::DocumentRead => DOCUMENT_READ_ABILITY,
            Self::DocumentWrite => DOCUMENT_WRITE_ABILITY,
            Self::ProcessExecute => PROCESS_EXECUTE_ABILITY,
            Self::OnixosTargetCandidateActivation => TARGET_CANDIDATE_ACTIVATION_ABILITY,
            Self::OnixosTargetGenerationPersistence => TARGET_GENERATION_PERSISTENCE_ABILITY,
            Self::OnixosTargetRollback => TARGET_ROLLBACK_ABILITY,
            Self::OnixosStateSyncPublish => STATE_SYNC_PUBLISH_ABILITY,
            Self::OnixosStateSyncDelete => STATE_SYNC_DELETE_ABILITY,
            Self::OnixosStateSyncAdmin => STATE_SYNC_ADMIN_ABILITY,
            Self::MantleBuildSubmit => "mantle/build/submit",
            Self::MantleInputUpload => "mantle/input/upload",
            Self::MantleStoreRead => "mantle/store/read",
            Self::MantleProjectDeclare => "mantle/project/declare",
            Self::MantleStatusRead => "mantle/status/read",
            Self::MantleLogRead => "mantle/log/read",
            Self::MantleEventRead => "mantle/event/read",
            Self::MantleResultRead => "mantle/result/read",
            Self::MantleUsageRead => "mantle/usage/read",
            Self::MantleOwnedAttemptCancel => "mantle/attempt/cancel-owned",
            Self::MantlePublish => "mantle/publish",
            Self::MantleAdmin => "mantle/admin",
            Self::KilnClaimAcquire => "kiln/claim/acquire",
            Self::KilnClaimRenew => "kiln/claim/renew",
            Self::KilnDispatch => "kiln/dispatch",
            Self::KilnCandidateSubmit => "kiln/candidate/submit",
            Self::KilnVerificationAdmit => "kiln/verification/admit",
            Self::KilnAttemptCancel => "kiln/attempt/cancel",
            Self::KilnPublicationPublish => "kiln/publication/publish",
            Self::KilnIntegrationLand => "kiln/integration/land",
            Self::KilnLifecycleComplete => "kiln/lifecycle/complete",
        }
    }

    /// Return the reviewed resource prefix for this action.
    #[must_use]
    pub const fn resource_prefix(self) -> &'static str {
        match self {
            Self::DocumentRead | Self::DocumentWrite => "",
            Self::ProcessExecute => PROCESS_EXECUTION_RESOURCE_PREFIX,
            Self::OnixosTargetCandidateActivation
            | Self::OnixosTargetGenerationPersistence
            | Self::OnixosTargetRollback => ONIXOS_TARGET_RESOURCE_PREFIX,
            Self::OnixosStateSyncPublish
            | Self::OnixosStateSyncDelete
            | Self::OnixosStateSyncAdmin => STATE_SYNC_RESOURCE_PREFIX,
            Self::MantleBuildSubmit
            | Self::MantleInputUpload
            | Self::MantleStoreRead
            | Self::MantleProjectDeclare
            | Self::MantleStatusRead
            | Self::MantleLogRead
            | Self::MantleEventRead
            | Self::MantleResultRead
            | Self::MantleUsageRead
            | Self::MantleOwnedAttemptCancel
            | Self::MantlePublish
            | Self::MantleAdmin => "mantle://gateway/v1/",
            Self::KilnClaimAcquire
            | Self::KilnClaimRenew
            | Self::KilnDispatch
            | Self::KilnCandidateSubmit
            | Self::KilnVerificationAdmit
            | Self::KilnAttemptCancel
            | Self::KilnPublicationPublish
            | Self::KilnIntegrationLand
            | Self::KilnLifecycleComplete => KILN_RESOURCE_PREFIX,
        }
    }

    /// Return the fixed BLAKE3 operation-profile identity for this mapping.
    #[must_use]
    pub fn operation_profile_id(self) -> OperationProfileId {
        OperationProfileId::from_bytes(operation_profile_digest(self))
    }
}

/// Borrowed raw holder invocation bound to one existing Basalt request.
pub struct UcanEnforcementRequest<'request, 'credential> {
    pub(super) enforcement: &'request EnforcementRequest,
    pub(super) compact_token: &'credential str,
    pub(super) profile: BasaltUcanProtocolProfile,
    pub(super) action: EnforcementAction,
    pub(super) nonce: HolderInvocationNonce,
    pub(super) holder_signature: &'credential SignerSignature,
}

impl<'request, 'credential> UcanEnforcementRequest<'request, 'credential> {
    /// Construct one borrowed raw UCAN enforcement request.
    #[must_use]
    pub const fn new(
        enforcement: &'request EnforcementRequest,
        compact_token: &'credential str,
        profile: BasaltUcanProtocolProfile,
        action: EnforcementAction,
        nonce: HolderInvocationNonce,
        holder_signature: &'credential SignerSignature,
    ) -> Self {
        Self {
            enforcement,
            compact_token,
            profile,
            action,
            nonce,
            holder_signature,
        }
    }

    /// Return the exact borrowed Basalt request.
    #[must_use]
    pub const fn enforcement(&self) -> &EnforcementRequest {
        self.enforcement
    }

    /// Return the explicitly selected UCAN protocol profile.
    #[must_use]
    pub const fn protocol_profile(&self) -> BasaltUcanProtocolProfile {
        self.profile
    }

    /// Return the selected typed downstream action.
    #[must_use]
    pub const fn action(&self) -> EnforcementAction {
        self.action
    }
}

/// Explicit caller-owned services and limits for UCAN verification.
pub struct UcanEnforcementBoundaries<'a> {
    /// Explicit verification time.
    pub verification_time: VerificationTime,
    /// Caller-owned issuer and holder key resolver.
    pub key_resolver: &'a dyn KeyResolver,
    /// Caller-owned proof store.
    pub proof_store: &'a dyn ProofStore,
    /// Caller-owned revocation checker.
    pub revocations: &'a dyn RevocationChecker,
    /// Non-consuming caveat policy observations.
    pub caveat_policies: &'a dyn CaveatPolicySet,
    /// Final state-consuming holder replay admission.
    pub replay: &'a dyn HolderInvocationReplayAdmission,
    /// Compact-token and proof limits.
    pub verification_limits: VerificationLimits,
    /// Holder canonical request limits.
    pub holder_limits: HolderInvocationLimits,
}

/// Stable typed adapter denial reason.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UcanEnforcementReason {
    OversizedCredential,
    MalformedCredential,
    SignatureDenied,
    KeyResolutionDenied,
    ProofDenied,
    TimeDenied,
    CaveatDenied,
    ScopeDenied,
    Revoked,
    HolderDenied,
    ReplayDenied,
    LimitExceeded,
    ProtocolProfileDenied,
    OperationMismatch,
    UnverifiedAuthorityFields,
    PolicyDenied,
    BackendUnavailable,
    InternalAuthorityDenied,
    Allowed,
}

impl UcanEnforcementReason {
    /// Return the stable serialized reason identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OversizedCredential => "oversized-credential",
            Self::MalformedCredential => "malformed-credential",
            Self::SignatureDenied => "signature-denied",
            Self::KeyResolutionDenied => "key-resolution-denied",
            Self::ProofDenied => "proof-denied",
            Self::TimeDenied => "time-denied",
            Self::CaveatDenied => "caveat-denied",
            Self::ScopeDenied => "scope-denied",
            Self::Revoked => "revoked",
            Self::HolderDenied => "holder-denied",
            Self::ReplayDenied => "replay-denied",
            Self::LimitExceeded => "limit-exceeded",
            Self::ProtocolProfileDenied => "protocol-profile-denied",
            Self::OperationMismatch => "operation-mismatch",
            Self::UnverifiedAuthorityFields => "unverified-authority-fields",
            Self::PolicyDenied => "policy-denied",
            Self::BackendUnavailable => "backend-unavailable",
            Self::InternalAuthorityDenied => "internal-authority-denied",
            Self::Allowed => "allowed",
        }
    }
}

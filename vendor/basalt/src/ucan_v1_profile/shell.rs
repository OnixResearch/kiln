use super::mapping::FinalProfileMapping;
use super::receipt::{FinalProfileReason, FinalProfileReceipt, ReceiptContext, build_receipt};
use crate::authority_core::{AuthorityDecision, BasaltPolicyRef};
use crate::ucan_enforcement::{
    bind_enforcement_action, build_final_authority_decision, decode_blake3_reference,
    operation_profile_digest,
};
use crate::ucan_profile::{
    BasaltUcanProtocolProfile, VerifiedUcanAuthorityInput, admit_final_profile,
};
use crate::{EnforcementRequest, Policy};
use std::cell::RefCell;
use std::fmt;
use ucan::core::profile::v1::FIELD_PROOFS;
use ucan::profile::v1::{Cbor, DidKeyResolver, SpecKind, parse_envelope};
use ucan::profile::v1_authorization::{
    AuthOutcome, AuthorizationReceipt, LiveAuthorization, V1DelegationStore, V1InvocationReplay,
    V1ReplayError, V1RevocationObservation, authorize_invocation,
};

/// Caller-owned external resource-owner observation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOwnerError {
    Denied,
    Unavailable,
    MalformedEvidence,
}

/// Port that verifies the exact UCAN subject and Basalt resource binding.
pub trait ResourceOwnerVerifier {
    /// Return a public BLAKE3 evidence identity for the exact supplied pair.
    ///
    /// # Errors
    ///
    /// Returns a closed failure class when ownership is denied or unavailable.
    fn verify(
        &self,
        subject: &str,
        resource: &str,
    ) -> Result<[u8; blake3::OUT_LEN], ResourceOwnerError>;
}

/// Borrowed final-profile Invocation bound to one exact Basalt request.
pub struct FinalProfileEnforcementRequest<'request, 'invocation> {
    enforcement: &'request EnforcementRequest,
    invocation_envelope: &'invocation [u8],
    profile: BasaltUcanProtocolProfile,
    expected_executor: &'request str,
}

impl<'request, 'invocation> FinalProfileEnforcementRequest<'request, 'invocation> {
    #[must_use]
    pub const fn new(
        enforcement: &'request EnforcementRequest,
        invocation_envelope: &'invocation [u8],
        profile: BasaltUcanProtocolProfile,
        expected_executor: &'request str,
    ) -> Self {
        Self {
            enforcement,
            invocation_envelope,
            profile,
            expected_executor,
        }
    }

    #[must_use]
    pub const fn enforcement(&self) -> &'request EnforcementRequest {
        self.enforcement
    }

    #[must_use]
    pub const fn protocol_profile(&self) -> BasaltUcanProtocolProfile {
        self.profile
    }
}

/// Live exact-request Basalt authority paired with UCAN's final live value.
///
/// This value has private construction and no clone or serde implementation.
pub struct LiveFinalProfileEnforcement<'request, 'invocation> {
    request: &'request EnforcementRequest,
    authorization: LiveAuthorization<'invocation>,
    ucan_receipt: AuthorizationReceipt,
    receipt: FinalProfileReceipt,
    _authority_decision: AuthorityDecision,
}

impl fmt::Debug for LiveFinalProfileEnforcement<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveFinalProfileEnforcement")
            .field("allowed", &true)
            .field("receipt_blake3", &self.receipt.receipt_blake3)
            .finish_non_exhaustive()
    }
}

impl<'request, 'invocation> LiveFinalProfileEnforcement<'request, 'invocation> {
    #[must_use]
    pub const fn request(&self) -> &'request EnforcementRequest {
        self.request
    }

    #[must_use]
    pub const fn authorization(&self) -> &LiveAuthorization<'invocation> {
        &self.authorization
    }

    #[must_use]
    pub const fn ucan_receipt(&self) -> &AuthorizationReceipt {
        &self.ucan_receipt
    }

    #[must_use]
    pub const fn receipt(&self) -> &FinalProfileReceipt {
        &self.receipt
    }
}

/// Fail-closed final-profile error with redacted evidence only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalProfileFailure {
    reason: FinalProfileReason,
    receipt: Box<FinalProfileReceipt>,
}

impl FinalProfileFailure {
    #[must_use]
    pub const fn reason(&self) -> FinalProfileReason {
        self.reason
    }

    #[must_use]
    pub const fn receipt(&self) -> &FinalProfileReceipt {
        &self.receipt
    }
}

impl fmt::Display for FinalProfileFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "final UCAN profile enforcement denied: {}",
            self.reason.as_str()
        )
    }
}

impl std::error::Error for FinalProfileFailure {}

struct PreparedRequest {
    context: ReceiptContext,
    delegated: bool,
}

struct ProspectiveAuthority {
    decision: AuthorityDecision,
    receipt: FinalProfileReceipt,
    context: ReceiptContext,
}

#[derive(Default)]
struct GateState {
    called: bool,
    prospective: Option<ProspectiveAuthority>,
    failure: Option<FinalProfileFailure>,
}

struct BasaltReplayGate<'a, P, O> {
    policy: &'a Policy,
    trusted_policy_ref: &'a BasaltPolicyRef,
    mapping: &'a FinalProfileMapping,
    request: &'a EnforcementRequest,
    owner: &'a O,
    final_replay: &'a P,
    now_seconds: i64,
    prepared: &'a PreparedRequest,
    state: RefCell<GateState>,
}

impl<P, O> BasaltReplayGate<'_, P, O>
where
    P: V1InvocationReplay,
    O: ResourceOwnerVerifier,
{
    fn prepare(
        &self,
        invocation_cid: &[u8],
        command: &str,
        subject: &str,
    ) -> Result<ProspectiveAuthority, FinalProfileReason> {
        let action = self
            .mapping
            .action_for(command)
            .ok_or(FinalProfileReason::UnknownCommand)?;
        let owner_evidence = self
            .owner
            .verify(subject, self.request.resource())
            .map_err(|error| match error {
                ResourceOwnerError::Denied => FinalProfileReason::ResourceOwnerDenied,
                ResourceOwnerError::Unavailable => FinalProfileReason::ResourceOwnerUnavailable,
                ResourceOwnerError::MalformedEvidence => {
                    FinalProfileReason::ResourceOwnerEvidenceMalformed
                }
            })?;
        let operation = bind_enforcement_action(action, self.request)
            .map_err(|_| FinalProfileReason::OperationMismatch)?;
        let operation_profile = operation_profile_digest(action);
        let request_binding = *operation.request_binding().as_bytes();
        let verified = VerifiedUcanAuthorityInput::from_final_verified(
            subject,
            operation_profile,
            request_binding,
            self.prepared.context.invocation,
            self.prepared.delegated,
        );
        let decision = build_final_authority_decision(
            self.policy,
            self.trusted_policy_ref,
            self.request,
            &verified,
            self.now_seconds,
        )
        .map_err(|_| FinalProfileReason::InternalAuthorityDenied)?;
        if !decision.is_allowed() {
            return Err(FinalProfileReason::PolicyDenied);
        }
        let context = ReceiptContext {
            action: Some(action),
            invocation_cid: Some(identity(b"invocation-cid", invocation_cid)),
            command: Some(identity(b"command", command.as_bytes())),
            subject: Some(identity(b"subject", subject.as_bytes())),
            resource_owner_evidence: Some(owner_evidence),
            request_binding: Some(request_binding),
            operation_profile: Some(operation_profile),
            ..self.prepared.context
        };
        let receipt = build_receipt(FinalProfileReason::Allowed, true, &context);
        Ok(ProspectiveAuthority {
            decision,
            receipt,
            context,
        })
    }

    fn record_failure(&self, reason: FinalProfileReason, context: &ReceiptContext) {
        let mut state = self.state.borrow_mut();
        state.failure = Some(failure(reason, context));
    }
}

impl<P, O> V1InvocationReplay for BasaltReplayGate<'_, P, O>
where
    P: V1InvocationReplay,
    O: ResourceOwnerVerifier,
{
    fn admit(
        &self,
        invocation_cid: &[u8],
        command: &str,
        subject: &str,
    ) -> Result<(), V1ReplayError> {
        {
            let mut state = self.state.borrow_mut();
            if state.called {
                return Err(V1ReplayError::Denied);
            }
            state.called = true;
        }

        let prospective = match self.prepare(invocation_cid, command, subject) {
            Ok(prospective) => prospective,
            Err(reason) => {
                let context = ReceiptContext {
                    invocation_cid: Some(identity(b"invocation-cid", invocation_cid)),
                    command: Some(identity(b"command", command.as_bytes())),
                    subject: Some(identity(b"subject", subject.as_bytes())),
                    ..self.prepared.context
                };
                self.record_failure(reason, &context);
                return Err(replay_error_for(reason));
            }
        };
        self.state.borrow_mut().prospective = Some(prospective);

        match self.final_replay.admit(invocation_cid, command, subject) {
            Ok(()) => Ok(()),
            Err(error) => {
                let context = self
                    .state
                    .borrow_mut()
                    .prospective
                    .take()
                    .expect("a final replay call requires prepared Basalt authority")
                    .context;
                self.record_failure(FinalProfileReason::ReplayDenied, &context);
                Err(error)
            }
        }
    }
}

/// Verify a final UCAN Invocation and return live exact-request Basalt authority.
///
/// Basalt mapping, policy, allocation, and receipt construction finish inside
/// UCAN's replay hook. The hook calls the final replay port only after every
/// Basalt preflight succeeds.
///
/// # Errors
///
/// Returns redacted evidence when UCAN, mapping, owner, policy, or replay denies.
///
/// # Panics
///
/// Panics only if UCAN violates its contract by returning live authority without
/// first calling the supplied replay gate exactly once.
#[allow(clippy::too_many_arguments)]
pub fn verify_final_profile_enforcement<'request, 'invocation, R, S, V, P, O>(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    mapping: &FinalProfileMapping,
    request: &FinalProfileEnforcementRequest<'request, 'invocation>,
    key_resolver: &R,
    delegation_store: &S,
    revocation: &V,
    final_replay: &P,
    resource_owner: &O,
    now_seconds: i64,
    clock_skew_seconds: i64,
) -> Result<LiveFinalProfileEnforcement<'request, 'invocation>, FinalProfileFailure>
where
    R: DidKeyResolver,
    S: V1DelegationStore,
    V: V1RevocationObservation,
    P: V1InvocationReplay,
    O: ResourceOwnerVerifier,
{
    let prepared = prepare_request(trusted_policy_ref, mapping, request)?;
    let gate = BasaltReplayGate {
        policy,
        trusted_policy_ref,
        mapping,
        request: request.enforcement,
        owner: resource_owner,
        final_replay,
        now_seconds,
        prepared: &prepared,
        state: RefCell::new(GateState::default()),
    };
    let outcome = authorize_invocation(
        request.invocation_envelope,
        key_resolver,
        delegation_store,
        revocation,
        &gate,
        request.expected_executor,
        now_seconds,
        clock_skew_seconds,
    )
    .map_err(|_| failure(FinalProfileReason::UcanBackendDenied, &prepared.context))?;

    match outcome {
        AuthOutcome::Allowed(authorization, ucan_receipt) => {
            let prospective = gate
                .state
                .borrow_mut()
                .prospective
                .take()
                .expect("UCAN live authorization requires prepared Basalt authority");
            Ok(LiveFinalProfileEnforcement {
                request: request.enforcement,
                authorization,
                ucan_receipt,
                receipt: prospective.receipt,
                _authority_decision: prospective.decision,
            })
        }
        AuthOutcome::Denied { .. } => Err(gate
            .state
            .borrow_mut()
            .failure
            .take()
            .unwrap_or_else(|| failure(FinalProfileReason::UcanDenied, &prepared.context))),
    }
}

fn prepare_request(
    trusted_policy_ref: &BasaltPolicyRef,
    mapping: &FinalProfileMapping,
    request: &FinalProfileEnforcementRequest<'_, '_>,
) -> Result<PreparedRequest, FinalProfileFailure> {
    let context = ReceiptContext {
        mapping: *mapping.identity(),
        policy: decode_blake3_reference(trusted_policy_ref.artifact_ref.as_str()),
        action: None,
        invocation: *blake3::hash(request.invocation_envelope).as_bytes(),
        invocation_cid: None,
        command: None,
        subject: None,
        resource_owner_evidence: None,
        request_binding: None,
        operation_profile: None,
    };
    if admit_final_profile(request.profile).is_err() {
        return Err(failure(FinalProfileReason::ProtocolProfileDenied, &context));
    }
    if !request.enforcement.capabilities().is_empty() {
        return Err(failure(FinalProfileReason::OperationMismatch, &context));
    }
    let (_, fields) = parse_envelope(request.invocation_envelope, SpecKind::Invocation)
        .map_err(|_| failure(FinalProfileReason::UcanDenied, &context))?;
    let delegated = fields.iter().any(|(name, value)| {
        name == FIELD_PROOFS && matches!(value, Cbor::Array(proofs) if !proofs.is_empty())
    });
    Ok(PreparedRequest { context, delegated })
}

const fn replay_error_for(reason: FinalProfileReason) -> V1ReplayError {
    match reason {
        FinalProfileReason::ResourceOwnerUnavailable => V1ReplayError::Unavailable,
        FinalProfileReason::ResourceOwnerEvidenceMalformed => V1ReplayError::Malformed,
        _ => V1ReplayError::Denied,
    }
}

fn failure(reason: FinalProfileReason, context: &ReceiptContext) -> FinalProfileFailure {
    FinalProfileFailure {
        reason,
        receipt: Box::new(build_receipt(reason, false, context)),
    }
}

fn identity(label: &[u8], value: &[u8]) -> [u8; blake3::OUT_LEN] {
    const IDENTITY_DOMAIN: &[u8] = b"basalt/ucan-v1-enforcement-identity/v1";
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, IDENTITY_DOMAIN);
    hash_field(&mut hasher, label);
    hash_field(&mut hasher, value);
    *hasher.finalize().as_bytes()
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("identity fields must fit u64");
    hasher.update(&length.to_be_bytes());
    hasher.update(value);
}

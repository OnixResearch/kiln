use ucan::{CompactToken, TokenSigner};

use super::core::{bind_holder_request, verify_and_plan};
use super::issue::{
    IssueFailure, IssuerAdmission, issue_plan, issue_plan_to_recipient, validate_issuer_admission,
    validate_recipient,
};
use super::model::{ExchangeRequest, HolderDelivery, OidcPolicy, RejectReason, ReplayKey};
use super::network::{DocumentPort, JwksCache, KeyLoadFailure, load_verification_key};
use super::receipt::{ExchangeReceipt, bind_recipient, receipt_for_failure, receipt_for_success};

/// One replay-store commit outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayCommit {
    /// The replay key was reserved for the first time.
    Fresh,
    /// The replay key was already reserved.
    Duplicate,
}

/// Stable replay-store failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayFailure {
    /// The store is unavailable.
    Unavailable,
    /// The store rejected malformed or contradictory input.
    Invalid,
}

/// Caller-owned consumptive replay capability.
pub trait ReplayPort {
    /// Atomically reserve one replay key until `expires_at_seconds`, a safe
    /// upper bound: the same OIDC token is not accepted again after it.
    ///
    /// # Errors
    ///
    /// Returns a stable failure when storage cannot make a durable decision.
    fn commit(
        &mut self,
        key: &ReplayKey,
        expires_at_seconds: u64,
    ) -> Result<ReplayCommit, ReplayFailure>;
}

/// Successful exchange result. The compact token remains live bearer material.
pub struct ExchangeResult {
    /// Issued compact UCAN. Callers must not log or persist it as evidence.
    pub token: CompactToken,
    /// Metadata-only exchange receipt.
    pub receipt: ExchangeReceipt,
}

impl std::fmt::Debug for ExchangeResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExchangeResult")
            .field("token", &"[REDACTED]")
            .field("receipt", &self.receipt)
            .finish()
    }
}

/// Terminal exchange failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeFailureClass {
    /// Discovery or JWK selection failed.
    KeyLoad(KeyLoadFailure),
    /// Pure verification or attenuation failed.
    Verification(RejectReason),
    /// Replay storage failed or reported a duplicate.
    Replay(ReplayFailure),
    /// The replay key was already consumed.
    Duplicate,
    /// UCAN issuance failed.
    Issue(IssueFailure),
}

/// Terminal failure with metadata-only evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeFailure {
    /// Stable failure class.
    pub class: ExchangeFailureClass,
    /// Redacted failure receipt.
    pub receipt: Box<ExchangeReceipt>,
}

/// Complete imperative exchange shell.
///
/// The shell retrieves public keys, runs the pure core, commits replay once,
/// and then asks the caller-owned signer to issue one attenuated UCAN.
///
/// # Errors
///
/// Returns a redacted typed failure. No failure contains OIDC or UCAN bearer text.
#[allow(
    clippy::too_many_arguments,
    reason = "explicit ports keep exchange authority visible"
)]
pub fn exchange(
    token: &str,
    now_seconds: u64,
    policy: &OidcPolicy,
    request: &ExchangeRequest,
    cache: &mut JwksCache,
    documents: &mut dyn DocumentPort,
    replay: &mut dyn ReplayPort,
    issuer: &IssuerAdmission,
    signer: &dyn TokenSigner,
) -> Result<ExchangeResult, ExchangeFailure> {
    exchange_inner(
        token,
        now_seconds,
        policy,
        request,
        cache,
        documents,
        replay,
        issuer,
        signer,
        None,
    )
}

// r[impl ucan_token_flow.oidc.holder_recipient]
/// Exchange for an explicit holder, bound to one exact native Mantle request.
///
/// Before key load and replay, `holder.request` must name `holder.recipient`
/// as its subject, the policy's service audience and project, and the ability
/// of `request.operation`; `request.resource` must equal its canonical
/// resource. The issued UCAN names the recipient and carries exactly that
/// canonical resource, so the admitted service and project stay bound into
/// the grant. A recipient equal to the issuing signer is refused. The
/// recipient is a delivery binding, not proof of holder possession: the
/// receiver must verify a holder-signed invocation before dispatch.
///
/// # Errors
/// Returns the same fail-closed, redacted failures as [`exchange`]; receipts
/// bind the recipient.
#[allow(
    clippy::too_many_arguments,
    reason = "explicit ports keep exchange authority visible"
)]
pub fn exchange_to_recipient(
    token: &str,
    now_seconds: u64,
    policy: &OidcPolicy,
    request: &ExchangeRequest,
    cache: &mut JwksCache,
    documents: &mut dyn DocumentPort,
    replay: &mut dyn ReplayPort,
    issuer: &IssuerAdmission,
    signer: &dyn TokenSigner,
    holder: &HolderDelivery<'_>,
) -> Result<ExchangeResult, ExchangeFailure> {
    match exchange_inner(
        token,
        now_seconds,
        policy,
        request,
        cache,
        documents,
        replay,
        issuer,
        signer,
        Some(holder),
    ) {
        Ok(mut result) => {
            bind_recipient(&mut result.receipt, holder.recipient);
            Ok(result)
        }
        Err(mut failure) => {
            bind_recipient(&mut failure.receipt, holder.recipient);
            Err(failure)
        }
    }
}

/// Admit the holder binding with no effects; returns the canonical resource.
fn admit_holder(
    policy: &OidcPolicy,
    request: &ExchangeRequest,
    signer: &dyn TokenSigner,
    holder: &HolderDelivery<'_>,
) -> Result<String, ExchangeFailure> {
    validate_recipient(holder.recipient, signer).map_err(|class| ExchangeFailure {
        receipt: Box::new(receipt_for_failure(
            policy,
            request,
            issue_failure_reason(class),
            None,
        )),
        class: ExchangeFailureClass::Issue(class),
    })?;
    bind_holder_request(policy, request, holder).map_err(|class| ExchangeFailure {
        receipt: Box::new(receipt_for_failure(policy, request, class.as_str(), None)),
        class: ExchangeFailureClass::Verification(class),
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "explicit ports keep exchange authority visible"
)]
fn exchange_inner(
    token: &str,
    now_seconds: u64,
    policy: &OidcPolicy,
    request: &ExchangeRequest,
    cache: &mut JwksCache,
    documents: &mut dyn DocumentPort,
    replay: &mut dyn ReplayPort,
    issuer: &IssuerAdmission,
    signer: &dyn TokenSigner,
    holder: Option<&HolderDelivery<'_>>,
) -> Result<ExchangeResult, ExchangeFailure> {
    validate_issuer_admission(issuer, signer).map_err(|class| ExchangeFailure {
        receipt: Box::new(receipt_for_failure(
            policy,
            request,
            issue_failure_reason(class),
            None,
        )),
        class: ExchangeFailureClass::Issue(class),
    })?;
    let holder_resource = holder
        .map(|holder| admit_holder(policy, request, signer, holder))
        .transpose()?;
    let key =
        load_verification_key(token, now_seconds, policy, cache, documents).map_err(|class| {
            ExchangeFailure {
                receipt: Box::new(receipt_for_failure(
                    policy,
                    request,
                    key_load_reason(class),
                    None,
                )),
                class: ExchangeFailureClass::KeyLoad(class),
            }
        })?;
    let mut plan = verify_and_plan(token, &key, now_seconds, policy, request).map_err(|class| {
        ExchangeFailure {
            receipt: Box::new(receipt_for_failure(
                policy,
                request,
                class.as_str(),
                Some(key.key_id.as_str()),
            )),
            class: ExchangeFailureClass::Verification(class),
        }
    })?;
    if let Some(resource) = holder_resource {
        // Equal to the admitted request resource; never a caller-chosen prefix.
        plan.resource = resource;
    }
    let replay_outcome = replay
        .commit(&plan.replay_key, plan.replay_expires_at_seconds)
        .map_err(|class| ExchangeFailure {
            receipt: Box::new(receipt_for_failure(
                policy,
                request,
                replay_failure_reason(class),
                Some(plan.key_id.as_str()),
            )),
            class: ExchangeFailureClass::Replay(class),
        })?;
    if replay_outcome == ReplayCommit::Duplicate {
        return Err(ExchangeFailure {
            class: ExchangeFailureClass::Duplicate,
            receipt: Box::new(receipt_for_failure(
                policy,
                request,
                "replay-duplicate",
                Some(plan.key_id.as_str()),
            )),
        });
    }
    let token = holder
        .map_or_else(
            || issue_plan(&plan, issuer, signer),
            |holder| issue_plan_to_recipient(&plan, holder.recipient, issuer, signer),
        )
        .map_err(|class| ExchangeFailure {
            receipt: Box::new(receipt_for_failure(
                policy,
                request,
                issue_failure_reason(class),
                Some(plan.key_id.as_str()),
            )),
            class: ExchangeFailureClass::Issue(class),
        })?;
    let receipt = receipt_for_success(policy, &plan);
    Ok(ExchangeResult { token, receipt })
}

const fn replay_failure_reason(failure: ReplayFailure) -> &'static str {
    match failure {
        ReplayFailure::Unavailable => "replay-unavailable",
        ReplayFailure::Invalid => "replay-invalid",
    }
}

const fn issue_failure_reason(failure: IssueFailure) -> &'static str {
    match failure {
        IssueFailure::Audience => "issuance-audience",
        IssueFailure::Capability => "issuance-capability",
        IssueFailure::Time => "issuance-time",
        IssueFailure::Token => "issuance-token",
        IssueFailure::IssuerBinding => "issuance-issuer-binding",
    }
}

const fn key_load_reason(failure: KeyLoadFailure) -> &'static str {
    match failure {
        KeyLoadFailure::Token(reason) => reason.as_str(),
        KeyLoadFailure::Discovery(_) => "discovery",
        KeyLoadFailure::DiscoveryIdentity => "discovery-identity",
        KeyLoadFailure::Jwks(_) => "jwks",
        KeyLoadFailure::MalformedJwks => "malformed-jwks",
        KeyLoadFailure::ExcessiveKeys => "excessive-keys",
        KeyLoadFailure::UnknownKey => "unknown-key",
        KeyLoadFailure::Stale => "stale-jwks",
    }
}

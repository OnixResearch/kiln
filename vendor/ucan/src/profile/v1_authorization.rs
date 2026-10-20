//! Final UCAN `1.0.0` authorization shell.
//!
//! The shell owns the caller-owned CID-addressed Delegation store, the
//! caller-owned revocation observation port, the caller-owned replay commit,
//! the private live authorization value, and deterministic redacted
//! receipts. The no-std core in `ucan-core` owns commands, policy, chain
//! composition, and admission.

#![allow(
    clippy::module_name_repetitions,
    clippy::type_complexity,
    // The orchestration below deliberately performs bounded narrowing and
    // uses decimal length literals; every value is limit-checked.
    clippy::cast_possible_truncation,
    clippy::doc_markdown
)]

use core::fmt;
use core::fmt::Write as _;

pub use authz_core::ChainError;
pub use authz_core::CommandError;
pub use authz_core::DelegationFacts;
pub use authz_core::EffectiveAuthority;
pub use authz_core::FinalCommand;
pub use authz_core::InvocationFacts;
pub use authz_core::Policy;
pub use authz_core::PolicyError;
use ucan_core::profile::v1 as wire_core;
use ucan_core::profile::v1_authorization as authz_core;

use crate::profile::v1::Cbor;
use crate::profile::v1::WireError;

/// Bound on a single resolved Delegation envelope.
pub const MAX_DELEGATION_BYTES: usize = 64 * 1024;
/// Bound on the total resolved proof bytes.
pub const MAX_TOTAL_PROOF_BYTES: usize = 256 * 1024;

/// Authorization outcome error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthError {
    Wire(WireError),
    Chain(authz_core::ChainError),
    MissingProof,
    ProofStoreUnavailable,
    OversizedProof,
    ProofBytesExceeded,
    SubstituteProof,
    MalformedInvocation,
    MalformedDelegation,
    UnsupportedKid,
    UnresolvedKey,
    InvalidInvocationSignature,
    InvalidDelegationSignature,
    Policy(authz_core::PolicyError),
    ReplayDenied { class: V1ReplayError },
    LimitExceeded,
}

impl fmt::Display for AuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(formatter, "wire error: {error}"),
            Self::Chain(error) => write!(formatter, "authority chain error: {error}"),
            Self::MissingProof => formatter.write_str("proof store returned no Delegation for a requested CID"),
            Self::ProofStoreUnavailable => formatter.write_str("proof store was unavailable"),
            Self::OversizedProof => formatter.write_str("a resolved Delegation exceeds the size limit"),
            Self::ProofBytesExceeded => formatter.write_str("total resolved proof bytes exceed the limit"),
            Self::SubstituteProof => formatter.write_str("a resolved Delegation does not bind to its requested CID"),
            Self::MalformedInvocation => formatter.write_str("Invocation envelope is malformed"),
            Self::MalformedDelegation => formatter.write_str("resolved Delegation envelope is malformed"),
            Self::UnsupportedKid => formatter.write_str("a Delegation keyspec is not supported"),
            Self::UnresolvedKey => formatter.write_str("the issuer key could not be resolved"),
            Self::InvalidInvocationSignature => formatter.write_str("the Invocation signature did not verify"),
            Self::InvalidDelegationSignature => formatter.write_str("a Delegation signature did not verify"),
            Self::Policy(error) => write!(formatter, "policy error: {error}"),
            Self::ReplayDenied { class } => write!(formatter, "replay denied: {}", class.as_str()),
            Self::LimitExceeded => formatter.write_str("authorization limit exceeded"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Caller-owned Delegation-store failure class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V1DelegationStoreError {
    /// The backend cannot answer now.
    Unavailable,
}

/// Caller-owned CID-addressed Delegation store port.
pub trait V1DelegationStore {
    /// Return the envelope bytes for a requested proof CID.
    ///
    /// # Errors
    ///
    /// Returns [`V1DelegationStoreError::Unavailable`] when the backend cannot
    /// answer. A successful `None` means that the CID is absent.
    fn get(&self, cid: &[u8]) -> Result<Option<Vec<u8>>, V1DelegationStoreError>;
}

/// Caller-owned revocation observation port. The observation is fail-closed:
/// unknown or unavailable means revoked.
pub trait V1RevocationObservation {
    /// Return whether the proof CID is revoked.
    fn is_revoked(&self, cid: &[u8]) -> bool;
}

/// Closed caller-owned replay failure classes. These classes cannot carry
/// credential or request payload text into a receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V1ReplayError {
    Duplicate,
    Stale,
    Malformed,
    Denied,
    Unavailable,
}

impl V1ReplayError {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::Stale => "stale",
            Self::Malformed => "malformed",
            Self::Denied => "denied",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Caller-owned Invocation replay admittance port. Called at most once per
/// admission attempt.
pub trait V1InvocationReplay {
    /// Admit an Invocation once, or return a closed failure class.
    ///
    /// # Errors
    ///
    /// Returns a typed class when the Invocation is not admitted.
    fn admit(&self, invocation_cid: &[u8], command: &str, subject: &str) -> Result<(), V1ReplayError>;
}

/// Private-construction, non-clone, non-serde live authorization value
/// borrowing the exact Invocation request. No public constructor exists, so
/// a persisted receipt cannot recreate it.
pub struct LiveAuthorization<'a> {
    envelope: &'a [u8],
    invocation_cid: Vec<u8>,
    plan: authz_core::EffectiveAuthority,
}

impl fmt::Debug for LiveAuthorization<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveAuthorization")
            .field("invocation_cid", &hex(&self.invocation_cid))
            .field("subject", &self.plan.subject)
            .field("executor", &self.plan.executor)
            .field("command", &self.plan.effective_command.as_str())
            .finish_non_exhaustive()
    }
}

impl<'a> LiveAuthorization<'a> {
    /// Return whether the admission was allowed (always true here; the type
    /// only exists after an allow).
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        true
    }

    /// Return the exact Invocation envelope bytes.
    #[must_use]
    pub const fn invocation_envelope(&self) -> &'a [u8] {
        self.envelope
    }

    /// Return the Invocation CID bytes.
    #[must_use]
    pub const fn invocation_cid(&self) -> &[u8] {
        self.invocation_cid.as_slice()
    }

    /// Return the effective subject.
    #[must_use]
    pub const fn subject(&self) -> &str {
        self.plan.subject.as_str()
    }

    /// Return the root authority issuer.
    #[must_use]
    pub const fn root_issuer(&self) -> &str {
        self.plan.root_issuer.as_str()
    }

    /// Return the admitted executor.
    #[must_use]
    pub const fn executor(&self) -> &str {
        self.plan.executor.as_str()
    }

    /// Return the effective command.
    #[must_use]
    pub const fn command(&self) -> &FinalCommand {
        &self.plan.effective_command
    }
}

/// Redacted authorization evidence. It carries identities, classes, and
/// reasons only; raw credentials, proofs, arguments, metadata, and policy
/// payloads are absent.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct AuthorizationReceipt {
    pub schema: String,
    pub profile: String,
    pub spec_cohort: String,
    pub allowed: bool,
    pub reason: Option<String>,
    pub invocation_cid: Vec<u8>,
    pub command_identity: Vec<u8>,
    pub subject_identity: Vec<u8>,
    pub executor_identity: Vec<u8>,
    pub proof_cids: Vec<Vec<u8>>,
    pub root_issuer_identity: Vec<u8>,
    pub time_valid: bool,
    pub revocation_class: String,
    pub replay_class: String,
    pub limits: ReceiptLimits,
    pub non_claims: Vec<String>,
}

/// Bounded receipt limit facts.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ReceiptLimits {
    pub max_delegation_bytes: usize,
    pub max_proof_count: usize,
}

/// Compute a deterministic domain-framed BLAKE3 digest over a receipt.
/// `serde_json` canonical sorting is applied before hashing.
#[must_use]
pub fn receipt_blake3(receipt: &AuthorizationReceipt) -> Vec<u8> {
    const DOMAIN: &[u8] = b"ucan.v1.authorization-receipt.digest\0";
    let bytes = receipt_canonical_bytes(receipt);
    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN);
    hasher.update(&bytes);
    hasher.finalize().as_bytes().to_vec()
}

fn receipt_canonical_bytes(receipt: &AuthorizationReceipt) -> Vec<u8> {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    map.insert("schema".to_owned(), serde_json::json!(receipt.schema));
    map.insert("profile".to_owned(), serde_json::json!(receipt.profile));
    map.insert("spec_cohort".to_owned(), serde_json::json!(receipt.spec_cohort));
    map.insert("allowed".to_owned(), serde_json::json!(receipt.allowed));
    map.insert("reason".to_owned(), serde_json::json!(receipt.reason));
    map.insert("invocation_cid".to_owned(), serde_json::json!(hex(&receipt.invocation_cid)));
    map.insert("command_identity".to_owned(), serde_json::json!(hex(&receipt.command_identity)));
    map.insert("subject_identity".to_owned(), serde_json::json!(hex(&receipt.subject_identity)));
    map.insert("executor_identity".to_owned(), serde_json::json!(hex(&receipt.executor_identity)));
    map.insert(
        "proof_cids".to_owned(),
        serde_json::json!(receipt.proof_cids.iter().map(|cid| hex(cid)).collect::<Vec<_>>()),
    );
    map.insert("root_issuer_identity".to_owned(), serde_json::json!(hex(&receipt.root_issuer_identity)));
    map.insert("time_valid".to_owned(), serde_json::json!(receipt.time_valid));
    map.insert("revocation_class".to_owned(), serde_json::json!(receipt.revocation_class));
    map.insert("replay_class".to_owned(), serde_json::json!(receipt.replay_class));
    map.insert(
        "limits".to_owned(),
        serde_json::json!({
            "max_delegation_bytes": receipt.limits.max_delegation_bytes,
            "max_proof_count": receipt.limits.max_proof_count,
        }),
    );
    map.insert("non_claims".to_owned(), serde_json::json!(receipt.non_claims));
    serde_json::to_vec(&serde_json::Value::Object(map.into_iter().collect())).expect("receipt is serializable")
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

/// Outcome of an orchestrated authorization attempt.
#[derive(Debug)]
pub enum AuthOutcome<'a> {
    Allowed(LiveAuthorization<'a>, AuthorizationReceipt),
    Denied {
        reason: Option<String>,
        receipt: AuthorizationReceipt,
    },
}

/// Orchestrate full authorization with strict preflight ordering: all
/// fallible work happens before the single caller-owned replay commit.
///
/// # Errors
///
/// Returns a hard [`AuthError`] for wire, proof-store, key, signature, or
/// limit failures; admission denials are returned as [`AuthOutcome::Denied`].
pub fn authorize_invocation<'a, R, S, V, P>(
    invocation_bytes: &'a [u8],
    key_resolver: &R,
    store: &S,
    revocation: &V,
    replay: &P,
    expected_executor: &str,
    now_seconds: i64,
    clock_skew: i64,
) -> Result<AuthOutcome<'a>, AuthError>
where
    R: crate::profile::v1::DidKeyResolver,
    S: V1DelegationStore,
    V: V1RevocationObservation,
    P: V1InvocationReplay,
{
    // 1. Verify the Invocation wire envelope.
    let (_, invocation_fields) = crate::profile::v1::parse_envelope(invocation_bytes, wire_core::SpecKind::Invocation)
        .map_err(AuthError::Wire)?;
    let invocation_issuer =
        field_text(&invocation_fields, wire_core::FIELD_ISSUER).ok_or(AuthError::MalformedInvocation)?;
    let invocation_key = key_resolver.resolve_did_key(&invocation_issuer).map_err(|_| AuthError::UnresolvedKey)?;
    if crate::profile::v1::verify_envelope(invocation_bytes, &invocation_key) != Ok(wire_core::SpecKind::Invocation) {
        return Err(AuthError::InvalidInvocationSignature);
    }
    let inv_envelope_cid =
        crate::profile::v1::cid_v1_of_payload(&crate::profile::v1::canonical_payload_bytes(&invocation_fields));

    // 2. Resolve, bound, and verify proofs in strict order.
    let resolved = resolve_delegations(&invocation_fields, expected_executor, key_resolver, store, revocation)?;

    // 3. Pure admission core.
    let mut work = authz_core::MAX_CHAIN_WORK;
    match authz_core::compose_chain(&resolved.invocation, &resolved.delegations, now_seconds, clock_skew, &mut work) {
        Ok(plan) => {
            // 4. Build all allowed evidence before the one caller-owned replay
            // commit. Only infallible moves and private-value construction
            // remain after replay accepts.
            let receipt = build_receipt(&inv_envelope_cid, &plan, true, None);
            let command_text = plan.effective_command.as_str();
            let subject = plan.subject.as_str();
            if let Err(class) = replay.admit(&inv_envelope_cid, command_text, subject) {
                let text = format!("replay denied: {}", class.as_str());
                let denied_receipt =
                    build_receipt(&inv_envelope_cid, &plan, false, Some(AuthError::ReplayDenied { class }));
                return Ok(AuthOutcome::Denied {
                    reason: Some(text),
                    receipt: denied_receipt,
                });
            }
            Ok(AuthOutcome::Allowed(
                LiveAuthorization {
                    envelope: invocation_bytes,
                    invocation_cid: inv_envelope_cid,
                    plan,
                },
                receipt,
            ))
        }
        Err(error) => {
            let plan = EffectiveAuthority {
                subject: resolved.invocation.subject.unwrap_or_default(),
                executor: resolved.invocation.executor.clone(),
                root_issuer: resolved.invocation.issuer.clone(),
                effective_command: resolved.invocation.command.clone(),
                proof_cids: resolved.invocation.proofs.clone(),
            };
            let text = format!("authority chain denied: {error}");
            let receipt = build_receipt(&inv_envelope_cid, &plan, false, Some(AuthError::Chain(error)));
            Ok(AuthOutcome::Denied {
                reason: Some(text),
                receipt,
            })
        }
    }
}

struct ResolvedContext {
    invocation: authz_core::InvocationFacts,
    delegations: Vec<DelegationFacts>,
}

fn resolve_delegations<R, S, V>(
    invocation_fields: &[(String, Cbor)],
    expected_executor: &str,
    key_resolver: &R,
    store: &S,
    revocation: &V,
) -> Result<ResolvedContext, AuthError>
where
    R: crate::profile::v1::DidKeyResolver,
    S: V1DelegationStore,
    V: V1RevocationObservation,
{
    let issuer = field_text(invocation_fields, wire_core::FIELD_ISSUER).ok_or(AuthError::MalformedInvocation)?;
    let audience = field_text(invocation_fields, wire_core::FIELD_AUDIENCE).ok_or(AuthError::MalformedInvocation)?;
    let subject = field_text(invocation_fields, wire_core::FIELD_SUBJECT);
    let command =
        field_text(invocation_fields, wire_core::FIELD_COMMAND).and_then(|text| FinalCommand::new(&text).ok());
    let args = field_map_to_ipld(invocation_fields, wire_core::FIELD_ARGS).unwrap_or(wire_core::IpldValue::Map(vec![]));
    let proofs = field_bytes_list(invocation_fields, wire_core::FIELD_PROOFS).unwrap_or_default();
    let (not_before, expires_at) = field_times(invocation_fields);

    let invocation = authz_core::InvocationFacts {
        issuer,
        executor: audience.clone(),
        executor_allowed: audience == expected_executor,
        subject,
        command: command.ok_or(AuthError::MalformedInvocation)?,
        args,
        proofs: proofs.clone(),
        not_before,
        expires_at,
    };

    let mut delegations = Vec::new();
    let mut total_bytes = 0_usize;
    for cid_requested in &proofs {
        let Some(bytes) = store.get(cid_requested).map_err(|_| AuthError::ProofStoreUnavailable)? else {
            return Err(AuthError::MissingProof);
        };
        if bytes.len() > MAX_DELEGATION_BYTES {
            return Err(AuthError::OversizedProof);
        }
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > MAX_TOTAL_PROOF_BYTES {
            return Err(AuthError::ProofBytesExceeded);
        }
        let facts = resolve_one_delegation(key_resolver, revocation, cid_requested, &bytes)?;
        delegations.push(facts);
    }
    Ok(ResolvedContext {
        invocation,
        delegations,
    })
}

fn resolve_one_delegation<R, V>(
    key_resolver: &R,
    revocation: &V,
    cid_requested: &[u8],
    bytes: &[u8],
) -> Result<DelegationFacts, AuthError>
where
    R: crate::profile::v1::DidKeyResolver,
    V: V1RevocationObservation,
{
    let (_, fields) =
        crate::profile::v1::parse_envelope(bytes, wire_core::SpecKind::Delegation).map_err(AuthError::Wire)?;

    // The CID binds only the canonical payload bytes, matching wire rules.
    let payload_bytes = crate::profile::v1::canonical_payload_bytes(&fields);
    let computed_cid = crate::profile::v1::cid_v1_of_payload(&payload_bytes);
    if computed_cid != cid_requested {
        return Err(AuthError::SubstituteProof);
    }

    let issuer = field_text(&fields, wire_core::FIELD_ISSUER).ok_or(AuthError::MalformedDelegation)?;
    let audience = field_text(&fields, wire_core::FIELD_AUDIENCE).ok_or(AuthError::MalformedDelegation)?;
    let subject = field_text(&fields, wire_core::FIELD_SUBJECT);
    let delegation_command =
        field_text(&fields, wire_core::FIELD_COMMAND).and_then(|text| FinalCommand::new(&text).ok());
    let policy = field_map_to_ipld(&fields, wire_core::FIELD_POLICY)
        .map(|value| Policy::parse(&value))
        .transpose()
        .map_err(AuthError::Policy)?;
    let (not_before, expires_at) = field_times(&fields);
    let revoked = revocation.is_revoked(cid_requested);

    // Verify the delegation signature against the resolved issuer key.
    let key = key_resolver.resolve_did_key(&issuer).map_err(|_| AuthError::UnresolvedKey)?;
    match crate::profile::v1::verify_envelope(bytes, &key) {
        Ok(wire_core::SpecKind::Delegation) => {}
        _ => return Err(AuthError::InvalidDelegationSignature),
    }

    Ok(DelegationFacts {
        issuer,
        audience,
        subject,
        command: delegation_command,
        policy,
        not_before,
        expires_at,
        cid: cid_requested.to_vec(),
        revoked,
    })
}

fn build_receipt(
    invocation_cid: &[u8],
    plan: &EffectiveAuthority,
    allowed: bool,
    error: Option<AuthError>,
) -> AuthorizationReceipt {
    let time_valid = allowed || matches!(&error, Some(AuthError::ReplayDenied { .. }));
    let revoked_class = match &error {
        Some(AuthError::Chain(ChainError::Revoked { .. })) => "observed-revoked".to_owned(),
        Some(AuthError::ReplayDenied { .. }) => "observed-not-revoked".to_owned(),
        None if allowed => "observed-not-revoked".to_owned(),
        _ => "not-established".to_owned(),
    };
    let replay_class = match &error {
        Some(AuthError::ReplayDenied { class }) => class.as_str().to_owned(),
        None if allowed => "committed".to_owned(),
        _ => "not-called".to_owned(),
    };
    let reason = error.map(|error| error.to_string());
    AuthorizationReceipt {
        schema: "ucan.v1.authorization-receipt.v1".to_owned(),
        profile: "ucan-wg-1.0.0".to_owned(),
        spec_cohort: "overview-9955aa1f".to_owned(),
        allowed,
        reason,
        invocation_cid: invocation_cid.to_vec(),
        command_identity: receipt_identity(b"command", plan.effective_command.as_str().as_bytes()),
        subject_identity: receipt_identity(b"subject", plan.subject.as_bytes()),
        executor_identity: receipt_identity(b"executor", plan.executor.as_bytes()),
        proof_cids: plan.proof_cids.clone(),
        root_issuer_identity: receipt_identity(b"root-issuer", plan.root_issuer.as_bytes()),
        time_valid,
        revocation_class: revoked_class,
        replay_class,
        limits: ReceiptLimits {
            max_delegation_bytes: MAX_DELEGATION_BYTES,
            max_proof_count: authz_core::MAX_CHAIN_PROOFS,
        },
        non_claims: vec![
            "does not prove transport security".to_owned(),
            "does not prove global freshness or durable replay storage".to_owned(),
            "does not prove external resource ownership".to_owned(),
            "does not prove policy completeness or backend truth".to_owned(),
            "does not prove effect correctness".to_owned(),
            "does not recreate live authorization".to_owned(),
        ],
    }
}

fn receipt_identity(label: &[u8], value: &[u8]) -> Vec<u8> {
    const DOMAIN: &[u8] = b"ucan.v1.authorization-receipt.identity\0";
    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN);
    hasher.update(label);
    hasher.update(&[0]);
    hasher.update(value);
    hasher.finalize().as_bytes().to_vec()
}

fn field_text(fields: &[(String, Cbor)], key: &str) -> Option<String> {
    fields.iter().find(|(name, _)| name == key).and_then(|(_, value)| match value {
        Cbor::Text(text) => Some(text.clone()),
        _ => None,
    })
}

fn field_times(fields: &[(String, Cbor)]) -> (Option<i64>, Option<i64>) {
    let nbf = fields
        .iter()
        .find(|(name, _)| name == wire_core::FIELD_NOT_BEFORE)
        .and_then(|(_, value)| match value {
            Cbor::UInt(value) => i64::try_from(*value).ok(),
            Cbor::NegInt(value) => Some(*value),
            _ => None,
        });
    let exp = fields.iter().find(|(name, _)| name == wire_core::FIELD_EXPIRES).and_then(|(_, value)| match value {
        Cbor::UInt(value) => i64::try_from(*value).ok(),
        Cbor::NegInt(value) => Some(*value),
        _ => None,
    });
    (nbf, exp)
}

fn field_bytes_list(fields: &[(String, Cbor)], key: &str) -> Option<Vec<Vec<u8>>> {
    fields.iter().find(|(name, _)| name == key).and_then(|(_, value)| match value {
        Cbor::Array(items) => Some(
            items
                .iter()
                .filter_map(|item| match item {
                    Cbor::Bytes(bytes) => Some(bytes.clone()),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    })
}

fn field_map_to_ipld(fields: &[(String, Cbor)], key: &str) -> Option<wire_core::IpldValue> {
    fields.iter().find(|(name, _)| name == key).map(|(_, value)| cbor_to_ipld(value))
}

fn cbor_to_ipld(value: &Cbor) -> wire_core::IpldValue {
    match value {
        Cbor::UInt(value) => wire_core::IpldValue::Int(i64::try_from(*value).unwrap_or(i64::MAX)),
        Cbor::NegInt(value) => wire_core::IpldValue::Int(*value),
        Cbor::Bytes(bytes) => wire_core::IpldValue::Bytes(bytes.clone()),
        Cbor::Text(text) => wire_core::IpldValue::Text(text.clone()),
        Cbor::Array(items) => wire_core::IpldValue::List(items.iter().map(cbor_to_ipld).collect()),
        Cbor::Map(items) => {
            wire_core::IpldValue::Map(items.iter().map(|(key, value)| (key.clone(), cbor_to_ipld(value))).collect())
        }
        Cbor::Null => wire_core::IpldValue::Null,
        Cbor::Bool(value) => wire_core::IpldValue::Bool(*value),
    }
}

#[cfg(test)]
#[path = "v1_authorization/tests.rs"]
mod tests;

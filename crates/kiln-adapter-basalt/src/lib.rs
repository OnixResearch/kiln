//! Basalt-backed implementation authority adapter for Kiln protected operations.
//!
//! The adapter owns live sessions, verifier services and the bounded redacted
//! decision log. Kiln shells own current work, attempt and fence facts. A
//! request, a decision or a receipt is never transferable authority.

// The denial is the bounded redacted audit record itself; keep the single
// record-and-return shape used by the core port.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use basalt::authority_core::BasaltPolicyRef;
use basalt::kiln::{Denied, Invocation, Operation, Request, Session, Verifier};
use basalt::ucan_enforcement::UcanEnforcementBoundaries;
use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityCredential, AuthorityDecision, AuthorityDenialReason,
    AuthorityProfile, AuthorityProvenance, AuthorityRequest, AuthoritySessionId,
    IMPLEMENTATION_AUTHORITY_DECISION_SCHEMA, MAX_AUTHORITY_DECISIONS, MAX_AUTHORITY_FIELD_BYTES,
    ProtectedAuthority, coverage,
};
use ucan::{
    CaveatPolicySet, HolderInvocationLimits, HolderInvocationNonce,
    HolderInvocationReplayAdmission, KeyResolver, ProofStore, RevocationChecker, SignerSignature,
    VerificationLimits, VerificationTime,
};

/// Maximum live sessions retained by one adapter.
pub const MAX_AUTHORITY_SESSIONS: usize = 64;
/// Maximum local-operator identity bytes.
pub const MAX_OPERATOR_BYTES: usize = 256;

/// Caller-owned verifier services and limits.
pub struct AuthorityServices {
    /// Issuer and holder key resolver.
    pub key_resolver: Box<dyn KeyResolver>,
    /// Proof store.
    pub proof_store: Box<dyn ProofStore>,
    /// Current revocation checker.
    pub revocations: Box<dyn RevocationChecker>,
    /// Non-consuming caveat observations.
    pub caveat_policies: Box<dyn CaveatPolicySet>,
    /// Final state-consuming replay admission.
    pub replay: Box<dyn HolderInvocationReplayAdmission>,
    /// Compact-token and proof limits.
    pub verification_limits: VerificationLimits,
    /// Holder canonical request limits.
    pub holder_limits: HolderInvocationLimits,
}

impl std::fmt::Debug for AuthorityServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorityServices")
            .field("verification_limits", &self.verification_limits)
            .field("holder_limits", &self.holder_limits)
            .finish_non_exhaustive()
    }
}

/// Operator-loaded verified authority configuration.
pub struct VerifiedAuthorityConfig {
    /// Parsed Nickel policy.
    pub policy: basalt::Policy,
    /// Exact policy artifact reference and schema.
    pub policy_reference: BasaltPolicyRef,
    /// Deployment authority generation. Must be non-zero.
    pub generation: u64,
    /// Deployment audience.
    pub audience: String,
    /// Operator-admitted root issuers.
    pub trusted_roots: Vec<String>,
}

impl std::fmt::Debug for VerifiedAuthorityConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedAuthorityConfig")
            .field("policy_reference", &self.policy_reference)
            .field("generation", &self.generation)
            .field("audience", &self.audience)
            .field("trusted_roots", &self.trusted_roots)
            .finish_non_exhaustive()
    }
}

impl VerifiedAuthorityConfig {
    /// Build the reviewed configuration over Basalt's packaged policy.
    ///
    /// # Errors
    /// Returns a message when the packaged policy does not parse or the
    /// generation/audience/root bindings are invalid.
    pub fn from_packaged_policy(
        generation: u64,
        audience: impl Into<String>,
        trusted_roots: Vec<String>,
    ) -> Result<Self, String> {
        let policy = basalt::parse_policy_json(basalt::DEFAULT_POLICY_JSON)
            .map_err(|error| format!("packaged Basalt policy: {error}"))?;
        let artifact_ref = format!(
            "blake3:{}",
            blake3::hash(basalt::DEFAULT_POLICY_JSON.as_bytes()).to_hex()
        );
        Self::new(
            policy,
            BasaltPolicyRef::new(
                artifact_ref,
                basalt::SUPPORTED_SCHEMA_VERSION,
                "kiln-authority",
            ),
            generation,
            audience,
            trusted_roots,
        )
    }

    /// Build one exact verified configuration.
    ///
    /// # Errors
    /// Returns a message for a zero generation, an empty or oversized policy
    /// reference, or an invalid audience or root identity.
    pub fn new(
        policy: basalt::Policy,
        policy_reference: BasaltPolicyRef,
        generation: u64,
        audience: impl Into<String>,
        trusted_roots: Vec<String>,
    ) -> Result<Self, String> {
        let audience = audience.into();
        if generation == 0 {
            return Err("authority generation must be non-zero".into());
        }
        if !valid_operator_text(&policy_reference.artifact_ref)
            || !valid_operator_text(&policy_reference.schema_version)
        {
            return Err("policy reference must be bounded non-empty trimmed ASCII".into());
        }
        if !valid_operator_text(&audience) {
            return Err("authority audience must be bounded non-empty trimmed ASCII".into());
        }
        for root in &trusted_roots {
            if !valid_operator_text(root) {
                return Err("trusted root must be bounded non-empty trimmed ASCII".into());
            }
        }
        Ok(Self {
            policy,
            policy_reference,
            generation,
            audience,
            trusted_roots,
        })
    }
}

struct VerifiedMode {
    policy: basalt::Policy,
    policy_reference: BasaltPolicyRef,
    generation: u64,
    audience: String,
    trusted_roots: Vec<String>,
    services: AuthorityServices,
}

enum Mode {
    Verified(Box<VerifiedMode>),
    LocalOperator {
        operator: String,
        projection_enabled: bool,
    },
}

struct LiveSession {
    key: String,
    request: AuthorityRequest,
    principal: String,
    provenance: AuthorityProvenance,
    audience: String,
    generation: u64,
    policy_reference: String,
    upstream_decision: String,
    session: Option<Session>,
}

impl std::fmt::Debug for LiveSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveSession")
            .field("operation", &self.request.operation)
            .field("principal", &self.principal)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// Basalt-backed protected-authority adapter.
pub struct BasaltAuthority {
    mode: Mode,
    sessions: BTreeMap<u64, LiveSession>,
    next_session: u64,
    decisions: Vec<AuthorityDecision>,
}

impl std::fmt::Debug for BasaltAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BasaltAuthority")
            .field("profile", &self.profile_label())
            .field("sessions", &self.sessions.len())
            .field("decisions", &self.decisions.len())
            .finish_non_exhaustive()
    }
}

impl BasaltAuthority {
    /// Build a verified UCAN authority adapter.
    ///
    /// # Errors
    /// Returns the configuration diagnostic.
    pub fn verified(
        config: VerifiedAuthorityConfig,
        services: AuthorityServices,
    ) -> Result<Self, String> {
        Ok(Self {
            mode: Mode::Verified(Box::new(VerifiedMode {
                policy: config.policy,
                policy_reference: config.policy_reference,
                generation: config.generation,
                audience: config.audience,
                trusted_roots: config.trusted_roots,
                services,
            })),
            sessions: BTreeMap::new(),
            next_session: 1,
            decisions: Vec::new(),
        })
    }

    /// Build the explicit local-operator compatibility profile.
    ///
    /// # Errors
    /// Returns a message for an invalid operator identity.
    pub fn local_operator(
        operator: impl Into<String>,
        projection_enabled: bool,
    ) -> Result<Self, String> {
        let operator = operator.into();
        if !valid_operator_text(&operator) {
            return Err("local operator must be bounded non-empty trimmed ASCII".into());
        }
        Ok(Self {
            mode: Mode::LocalOperator {
                operator,
                projection_enabled,
            },
            sessions: BTreeMap::new(),
            next_session: 1,
            decisions: Vec::new(),
        })
    }

    /// Redacted mode label.
    #[must_use]
    pub const fn profile_label(&self) -> &'static str {
        match self.mode {
            Mode::Verified(_) => "verified-ucan",
            Mode::LocalOperator { .. } => "local-operator",
        }
    }

    fn admit_evaluate(
        &self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        now_unix_s: u64,
    ) -> Result<LiveSession, AuthorityDenialReason> {
        match profile {
            AuthorityProfile::Missing => Err(AuthorityDenialReason::MissingEvidence),
            AuthorityProfile::VerifiedUcan(credential) => {
                self.admit_verified(request, credential, now_unix_s)
            }
            AuthorityProfile::LocalOperator { operator } => {
                self.admit_local_operator(request, operator)
            }
        }
    }

    fn admit_local_operator(
        &self,
        request: &AuthorityRequest,
        operator: &str,
    ) -> Result<LiveSession, AuthorityDenialReason> {
        let Mode::LocalOperator {
            operator: configured,
            projection_enabled,
        } = &self.mode
        else {
            return Err(AuthorityDenialReason::LocalOperatorNotSelected);
        };
        if *projection_enabled {
            return Err(AuthorityDenialReason::ProjectionRequiresOperator);
        }
        if configured != operator {
            return Err(AuthorityDenialReason::PrincipalMismatch);
        }
        if !valid_operator_text(operator) {
            return Err(AuthorityDenialReason::RequestInvalid);
        }
        let policy_reference = bounded_reference(&format!("local-operator:{operator}"));
        Ok(LiveSession {
            key: session_key(request, operator),
            request: request.clone(),
            principal: operator.into(),
            provenance: AuthorityProvenance::LocalOperator,
            audience: "local-operator".into(),
            generation: 0,
            upstream_decision: local_decision_reference(request, operator),
            policy_reference,
            session: None,
        })
    }

    fn admit_verified(
        &self,
        request: &AuthorityRequest,
        credential: &AuthorityCredential,
        now_unix_s: u64,
    ) -> Result<LiveSession, AuthorityDenialReason> {
        let Mode::Verified(mode) = &self.mode else {
            return Err(AuthorityDenialReason::VerifierUnavailable);
        };
        let exact = basalt_request(request, credential, mode.generation, &mode.audience);
        let operation = exact.operation;
        let signature = SignerSignature::from_bytes(*credential.holder_signature());
        let invocation = Invocation {
            compact_token: credential.compact_token(),
            nonce: HolderInvocationNonce::from_bytes(*credential.nonce()),
            holder_signature: &signature,
        };
        let boundaries = UcanEnforcementBoundaries {
            verification_time: VerificationTime::from_unix_seconds(now_unix_s),
            key_resolver: mode.services.key_resolver.as_ref(),
            proof_store: mode.services.proof_store.as_ref(),
            revocations: mode.services.revocations.as_ref(),
            caveat_policies: mode.services.caveat_policies.as_ref(),
            replay: mode.services.replay.as_ref(),
            verification_limits: mode.services.verification_limits,
            holder_limits: mode.services.holder_limits,
        };
        let verifier = Verifier {
            policy: &mode.policy,
            policy_ref: &mode.policy_reference,
            generation: mode.generation,
            audience: &mode.audience,
            trusted_roots: &mode.trusted_roots,
        };
        let session = verifier
            .verify(exact, &invocation, &boundaries)
            .map_err(|Denied| AuthorityDenialReason::CredentialDenied)?;
        if session
            .require(operation, &request.target, request.request_digest)
            .is_err()
            || (request.operation.requires_candidate()
                && session.require_candidate(request.candidate).is_err())
        {
            return Err(AuthorityDenialReason::BindingMismatch);
        }
        let principal = credential.subject();
        Ok(LiveSession {
            key: session_key(request, principal),
            request: request.clone(),
            principal: principal.into(),
            provenance: AuthorityProvenance::VerifiedUcan,
            audience: mode.audience.clone(),
            generation: mode.generation,
            policy_reference: session.policy_identity().into(),
            upstream_decision: session.decision_id().into(),
            session: Some(session),
        })
    }

    fn find_session(&self, key: &str) -> Option<u64> {
        self.sessions
            .iter()
            .find(|(_, live)| live.key == key)
            .map(|(id, _)| *id)
    }

    fn insert_session(&mut self, live: LiveSession) -> u64 {
        if self.sessions.len() >= MAX_AUTHORITY_SESSIONS
            && let Some(oldest) = self.sessions.keys().next().copied()
        {
            self.sessions.remove(&oldest);
        }
        let id = self.next_session;
        self.next_session = self.next_session.saturating_add(1);
        self.sessions.insert(id, live);
        id
    }

    fn record(&mut self, decision: AuthorityDecision) -> AuthorityDecision {
        if self.decisions.len() >= MAX_AUTHORITY_DECISIONS {
            self.decisions.remove(0);
        }
        self.decisions.push(decision.clone());
        decision
    }

    fn allowed_decision(&self, id: u64, live: &LiveSession) -> AuthorityDecision {
        let entry = coverage(live.request.operation);
        AuthorityDecision {
            schema: IMPLEMENTATION_AUTHORITY_DECISION_SCHEMA.into(),
            allowed: true,
            operation: live.request.operation,
            boundary: entry.owner,
            effect: entry.effect,
            provenance: live.provenance,
            principal: live.principal.clone(),
            audience: live.audience.clone(),
            generation: live.generation,
            policy_ref: live.policy_reference.clone(),
            upstream_decision: live.upstream_decision.clone(),
            request_identity: live.request.digest_hex(),
            session: Some(AuthoritySessionId(id)),
            reason: AuthorityDenialReason::Allowed,
        }
    }

    fn revalidate_verified(
        &self,
        live: &LiveSession,
        now_unix_s: u64,
    ) -> Result<(), AuthorityDenialReason> {
        let Mode::Verified(mode) = &self.mode else {
            return Err(AuthorityDenialReason::VerifierUnavailable);
        };
        let session = live
            .session
            .as_ref()
            .ok_or(AuthorityDenialReason::SessionNotFound)?;
        let boundaries = UcanEnforcementBoundaries {
            verification_time: VerificationTime::from_unix_seconds(now_unix_s),
            key_resolver: mode.services.key_resolver.as_ref(),
            proof_store: mode.services.proof_store.as_ref(),
            revocations: mode.services.revocations.as_ref(),
            caveat_policies: mode.services.caveat_policies.as_ref(),
            replay: mode.services.replay.as_ref(),
            verification_limits: mode.services.verification_limits,
            holder_limits: mode.services.holder_limits,
        };
        Verifier {
            policy: &mode.policy,
            policy_ref: &mode.policy_reference,
            generation: mode.generation,
            audience: &mode.audience,
            trusted_roots: &mode.trusted_roots,
        }
        .revalidate(session, &boundaries)
        .map_err(|Denied| AuthorityDenialReason::RevalidationDenied)
    }
}

// r[impl kiln.implementation.authority.binding]
impl ProtectedAuthority for BasaltAuthority {
    fn admit(
        &mut self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<AuthorityDecision, AuthorityDecision> {
        if request.validate().is_err() {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::RequestInvalid,
            )));
        }
        if currentness != AttemptCurrentness::Current {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::AttemptNotCurrent,
            )));
        }
        let principal = match profile {
            AuthorityProfile::Missing => String::new(),
            AuthorityProfile::VerifiedUcan(credential) => credential.subject().into(),
            AuthorityProfile::LocalOperator { operator } => operator.clone(),
        };
        if !principal.is_empty()
            && let Some(existing) = self.find_session(&session_key(request, &principal))
        {
            return match self.revalidate(
                AuthoritySessionId(existing),
                request,
                currentness,
                now_unix_s,
            ) {
                Ok(()) => {
                    let decision = match self.sessions.get(&existing) {
                        Some(live) => self.allowed_decision(existing, live),
                        None => {
                            AuthorityDecision::deny(request, AuthorityDenialReason::SessionNotFound)
                        }
                    };
                    if decision.allowed {
                        Ok(decision)
                    } else {
                        Err(self.record(decision))
                    }
                }
                Err(decision) => Err(decision),
            };
        }
        match self.admit_evaluate(request, profile, now_unix_s) {
            Ok(live) => {
                let id = self.insert_session(live);
                let decision = match self.sessions.get(&id) {
                    Some(live) => self.allowed_decision(id, live),
                    None => {
                        AuthorityDecision::deny(request, AuthorityDenialReason::SessionNotFound)
                    }
                };
                if decision.allowed {
                    Ok(self.record(decision))
                } else {
                    Err(self.record(decision))
                }
            }
            Err(reason) => Err(self.record(AuthorityDecision::deny(request, reason))),
        }
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> Result<(), AuthorityDecision> {
        let id = session.0;
        if currentness != AttemptCurrentness::Current {
            self.sessions.remove(&id);
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::AttemptNotCurrent,
            )));
        }
        let Some(live) = self.sessions.get(&id) else {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::SessionNotFound,
            )));
        };
        if live.request.digest_hex() != request.digest_hex()
            || live.request.operation != request.operation
        {
            self.sessions.remove(&id);
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::BindingMismatch,
            )));
        }
        let verdict = match &self.mode {
            Mode::LocalOperator { .. } => Ok(()),
            Mode::Verified(_) => self.revalidate_verified(live, now_unix_s),
        };
        match verdict {
            Ok(()) => {
                let decision = self.allowed_decision(id, live);
                self.record(decision);
                Ok(())
            }
            Err(reason) => {
                self.sessions.remove(&id);
                Err(self.record(AuthorityDecision::deny(request, reason)))
            }
        }
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.sessions.remove(&session.0);
    }

    fn decisions(&self) -> &[AuthorityDecision] {
        &self.decisions
    }
}

fn basalt_request(
    request: &AuthorityRequest,
    credential: &AuthorityCredential,
    generation: u64,
    audience: &str,
) -> Request {
    Request {
        operation: basalt_operation(request.operation),
        subject: credential.subject().into(),
        audience: audience.into(),
        project: request.project.clone(),
        work: request.work.clone(),
        work_version: request.work_version,
        attempt: request.attempt.clone(),
        fence: request.fence,
        generation,
        request_digest: request.request_digest,
        target: request.target.clone(),
        candidate: request.candidate,
    }
}

const fn basalt_operation(
    operation: kiln_core::implementation::authority::ProtectedOperation,
) -> Operation {
    use kiln_core::implementation::authority::ProtectedOperation as Protected;
    match operation {
        Protected::ClaimAcquire => Operation::ClaimAcquire,
        Protected::ClaimRenew => Operation::ClaimRenew,
        Protected::Dispatch => Operation::Dispatch,
        Protected::CandidateSubmit => Operation::CandidateSubmit,
        Protected::VerificationAdmit => Operation::VerificationAdmit,
        Protected::AttemptCancel => Operation::AttemptCancel,
        Protected::PublicationPublish => Operation::PublicationPublish,
        Protected::IntegrationLand => Operation::IntegrationLand,
        Protected::LifecycleComplete => Operation::LifecycleComplete,
    }
}

fn session_key(request: &AuthorityRequest, principal: &str) -> String {
    format!("{}:{principal}", request.digest_hex())
}

fn local_decision_reference(request: &AuthorityRequest, operator: &str) -> String {
    let mut material = String::from("kiln.local-operator-decision.v1:");
    material.push_str(operator);
    material.push(':');
    material.push_str(&request.digest_hex());
    blake3::hash(material.as_bytes()).to_hex().to_string()
}

fn bounded_reference(value: &str) -> String {
    if value.len() <= MAX_AUTHORITY_FIELD_BYTES {
        value.into()
    } else {
        let mut hasher = blake3::Hasher::new();
        hasher.update(value.as_bytes());
        format!("blake3:{}", hasher.finalize().to_hex())
    }
}

fn valid_operator_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OPERATOR_BYTES
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::authority::{
        AUTHORITY_DIGEST_BYTES, ProtectedOperation, coverage,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use ucan::token::DenyUnknownCaveats;
    use ucan::{
        Ed25519PublicKey, HolderInvocationReplayCandidate, HolderInvocationReplayError, IssuerDid,
        KeyResolverError, NoRevocations, ProofCollection,
    };

    struct MissingKeys;

    impl KeyResolver for MissingKeys {
        fn resolve_key(&self, issuer: &IssuerDid) -> Result<Ed25519PublicKey, KeyResolverError> {
            Err(KeyResolverError::MissingKey {
                issuer: issuer.as_str().into(),
            })
        }
    }

    #[derive(Clone, Default)]
    struct CountingReplay(Arc<AtomicUsize>);

    impl HolderInvocationReplayAdmission for CountingReplay {
        fn admit_holder_invocation(
            &self,
            _candidate: &HolderInvocationReplayCandidate<'_>,
        ) -> Result<(), HolderInvocationReplayError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn services(replay: CountingReplay) -> AuthorityServices {
        AuthorityServices {
            key_resolver: Box::new(MissingKeys),
            proof_store: Box::new(ProofCollection::default()),
            revocations: Box::new(NoRevocations),
            caveat_policies: Box::new(DenyUnknownCaveats),
            replay: Box::new(replay),
            verification_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        }
    }

    fn config() -> VerifiedAuthorityConfig {
        VerifiedAuthorityConfig::from_packaged_policy(7, "kiln-service-a", Vec::new())
            .unwrap_or_else(|error| unreachable!("{error}"))
    }

    fn exact_request(operation: ProtectedOperation) -> Request {
        Request {
            operation: basalt_operation(operation),
            subject: "did:key:zHolder".into(),
            audience: "kiln-service-a".into(),
            project: "OnixResearch/kiln".into(),
            work: "kiln.implementation.work".into(),
            work_version: 3,
            attempt: "attempt-a".into(),
            fence: 5,
            generation: 7,
            request_digest: [41; AUTHORITY_DIGEST_BYTES],
            target: "candidate/approved".into(),
            candidate: if operation.requires_candidate() {
                [43; AUTHORITY_DIGEST_BYTES]
            } else {
                [0; AUTHORITY_DIGEST_BYTES]
            },
        }
    }

    fn authority_request(operation: ProtectedOperation) -> AuthorityRequest {
        AuthorityRequest {
            operation,
            project: "OnixResearch/kiln".into(),
            work: "kiln.implementation.work".into(),
            work_version: 3,
            attempt: "attempt-a".into(),
            fence: 5,
            request_digest: [41; AUTHORITY_DIGEST_BYTES],
            target: "candidate/approved".into(),
            candidate: if operation.requires_candidate() {
                [43; AUTHORITY_DIGEST_BYTES]
            } else {
                [0; AUTHORITY_DIGEST_BYTES]
            },
        }
    }

    #[test]
    // r[verify kiln.implementation.authority.coverage.scenario.complete]
    fn packaged_policy_covers_every_protected_operation() {
        let config = config();
        let verifier = Verifier {
            policy: &config.policy,
            policy_ref: &config.policy_reference,
            generation: config.generation,
            audience: &config.audience,
            trusted_roots: &config.trusted_roots,
        };
        for operation in ProtectedOperation::ALL {
            assert!(
                verifier.enforcement(&exact_request(operation)).is_ok(),
                "{operation:?}"
            );
            let action = basalt_operation(operation).action();
            assert_eq!(
                action.contract_id(),
                format!("kiln-{}", coverage(operation).basalt_operation)
            );
        }
    }

    #[test]
    fn policy_without_one_contract_denies_that_operation() {
        let mut document: serde_json::Value = serde_json::from_str(basalt::DEFAULT_POLICY_JSON)
            .unwrap_or_else(|error| unreachable!("{error}"));
        document
            .get_mut("contracts")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap_or_else(|| unreachable!("packaged policy contracts"))
            .remove("kiln-claim-acquire");
        let text = serde_json::to_string(&document).unwrap_or_else(|error| unreachable!("{error}"));
        let config = VerifiedAuthorityConfig::new(
            basalt::parse_policy_json(&text).unwrap_or_else(|error| unreachable!("{error}")),
            BasaltPolicyRef::new(
                "blake3:missing-contract",
                basalt::SUPPORTED_SCHEMA_VERSION,
                "kiln-authority",
            ),
            7,
            "kiln-service-a",
            Vec::new(),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        let verifier = Verifier {
            policy: &config.policy,
            policy_ref: &config.policy_reference,
            generation: config.generation,
            audience: &config.audience,
            trusted_roots: &config.trusted_roots,
        };
        assert!(
            verifier
                .enforcement(&exact_request(ProtectedOperation::ClaimAcquire))
                .is_err()
        );
        assert!(
            verifier
                .enforcement(&exact_request(ProtectedOperation::Dispatch))
                .is_ok()
        );
    }

    #[test]
    fn local_operator_profile_admits_and_revalidates() {
        let mut authority = BasaltAuthority::local_operator("operator-a", false)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let request = authority_request(ProtectedOperation::ClaimAcquire);
        let profile = AuthorityProfile::LocalOperator {
            operator: "operator-a".into(),
        };
        let decision = authority
            .admit(&request, &profile, AttemptCurrentness::Current, 1)
            .unwrap_or_else(|denial| unreachable!("{denial:?}"));
        assert!(decision.allowed);
        assert_eq!(decision.provenance, AuthorityProvenance::LocalOperator);
        let session = decision
            .session
            .unwrap_or_else(|| unreachable!("allowed decision requires a session"));
        authority
            .revalidate(session, &request, AttemptCurrentness::Current, 2)
            .unwrap_or_else(|denial| unreachable!("{denial:?}"));
        assert_eq!(authority.decisions().len(), 2);
        let mut substituted = request.clone();
        substituted.fence += 1;
        assert_eq!(
            authority
                .revalidate(session, &substituted, AttemptCurrentness::Current, 2)
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::BindingMismatch)
        );
    }

    #[test]
    fn local_operator_profile_denies_projection_and_verified_profiles() {
        let request = authority_request(ProtectedOperation::Dispatch);
        let mut projection = BasaltAuthority::local_operator("operator-a", true)
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            projection
                .admit(
                    &request,
                    &AuthorityProfile::LocalOperator {
                        operator: "operator-a".into()
                    },
                    AttemptCurrentness::Current,
                    1
                )
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::ProjectionRequiresOperator)
        );
        let mut verified = BasaltAuthority::verified(config(), services(CountingReplay::default()))
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            verified
                .admit(
                    &request,
                    &AuthorityProfile::LocalOperator {
                        operator: "operator-a".into()
                    },
                    AttemptCurrentness::Current,
                    1
                )
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::LocalOperatorNotSelected)
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.enforcement.scenario.absent]
    fn malformed_verified_token_denies_before_replay() {
        let replay = CountingReplay::default();
        let counter = Arc::clone(&replay.0);
        let mut authority = BasaltAuthority::verified(config(), services(replay))
            .unwrap_or_else(|error| unreachable!("{error}"));
        let request = authority_request(ProtectedOperation::CandidateSubmit);
        let credential = AuthorityCredential::new(
            "did:key:zHolder",
            "not-a-compact-token",
            [0; kiln_core::implementation::authority::AUTHORITY_NONCE_BYTES],
            [0; kiln_core::implementation::authority::AUTHORITY_SIGNATURE_BYTES],
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let decision = authority
            .admit(
                &request,
                &AuthorityProfile::VerifiedUcan(credential),
                AttemptCurrentness::Current,
                1,
            )
            .err()
            .unwrap_or_else(|| unreachable!("malformed token must deny"));
        assert_eq!(decision.reason, AuthorityDenialReason::CredentialDenied);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn currentness_denies_before_any_admission() {
        let mut authority = BasaltAuthority::local_operator("operator-a", false)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let request = authority_request(ProtectedOperation::ClaimRenew);
        assert_eq!(
            authority
                .admit(
                    &request,
                    &AuthorityProfile::LocalOperator {
                        operator: "operator-a".into()
                    },
                    AttemptCurrentness::Cancelled,
                    1
                )
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::AttemptNotCurrent)
        );
        assert!(authority.decisions().len() == 1);
    }
}

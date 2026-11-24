//! Basalt-backed `SourceAuthority` adapter and `kiln://source/v1/` holder signer.
//!
//! Bridges Kiln's pure `SourceAuthority` port (`kiln_core::projection::authority`)
//! to vendored `basalt::kiln::{Verifier::verify_source, revalidate_source,
//! source_enforcement, SourceRequest, SourceScope, SourceSession}` and
//! `kiln://source/v1/` UCANs.

#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use basalt::authority_core::BasaltPolicyRef;
use basalt::kiln::{
    Denied, Invocation, PathScope as BasaltPathScope, SourceAttempt, SourceOperation,
    SourceRequest, SourceScope, SourceSession, Verifier,
};
use basalt::ucan_enforcement::{UcanEnforcementBoundaries, bind_enforcement_action};
use kiln_core::implementation::authority::{
    AUTHORITY_SIGNATURE_BYTES, AttemptCurrentness, AuthorityCredential, AuthorityProfile,
    AuthorityProvenance, AuthoritySessionId,
};
use kiln_core::projection::authority::{
    AttemptFence, CapabilityContext, EffectiveScopeFacts, EffectiveSourceScope,
    MAX_DELEGATION_DEPTH, MAX_SOURCE_DECISIONS, MAX_SOURCE_GRANTS, MAX_SOURCE_SESSIONS,
    OriginScope, PathScope, RefEvolution, SessionCapability, SnapshotRule, SourceAuthority,
    SourceAuthorityDecision, SourceAuthorityRequest, SourceCurrentness, SourceDenialReason,
    SourceGrant, SourceViewOperation, check_request, operation_for_ability, source_coverage,
};
use kiln_core::projection::sharing::SharePlan;
use kiln_core::types::{ContentId, CoreResult, Diagnostic};
use ucan::{
    AudienceDid, CapabilityDocument, CapabilitySet, CompactToken, ED25519_SECRET_KEY_BYTES,
    Ed25519InMemorySigner, HolderInvocationLimits, HolderInvocationNonce,
    HolderInvocationSigningRequest, ProofReference, ProofReferences, ProofToken, SignerSignature,
    TokenError, TokenSigner, TokenTimeBounds, VerificationLimits, VerificationTime,
    issue_token_with_signer, proof_reference, sign_holder_invocation,
    verify_compact_token_str_with_resolvers_and_revocations,
};

use crate::signer::NonceSource;
use crate::state::{BoundReplay, BoundRevocations, NoReplay};
use crate::{AuthorityServices, MILLIS_PER_SECOND, VerifiedAuthorityConfig, revocation_instant_ms};

const SOURCE_V1_PREFIX: &str = "kiln://source/v1/";
const SNAPSHOT_RULE_DOMAIN: &[u8] = b"kiln.source-snapshot-rule.v1\0";
const MAX_FIELD_BYTES: usize = 512;
const MAX_COMPONENTS: usize = 64;

/// Deterministic bounded printable ASCII encoding of a [`SnapshotRule`] for
/// `basalt::kiln::SourceRequest::snapshot` (`<= 512` bytes, trimmed ASCII).
#[must_use]
pub fn encode_snapshot_rule(rule: &SnapshotRule) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SNAPSHOT_RULE_DOMAIN);
    match rule {
        SnapshotRule::Exact { revision } => {
            hasher.update(b"exact\0");
            let ns = revision.provider_namespace.as_str().as_bytes();
            let kind = revision.native_kind.as_str().as_bytes();
            let id = revision.native_id.as_str().as_bytes();
            hasher.update(&(ns.len() as u64).to_le_bytes());
            hasher.update(ns);
            hasher.update(&(kind.len() as u64).to_le_bytes());
            hasher.update(kind);
            hasher.update(&(id.len() as u64).to_le_bytes());
            hasher.update(id);
            format!("exact:{}", hasher.finalize().to_hex())
        }
        SnapshotRule::Ref { name, evolution } => {
            let evo = match evolution {
                RefEvolution::FastForward => "ff",
                RefEvolution::Rewrite => "rw",
            };
            hasher.update(b"ref\0");
            hasher.update(evo.as_bytes());
            hasher.update(b"\0");
            hasher.update(&(name.len() as u64).to_le_bytes());
            hasher.update(name.as_bytes());
            format!("ref:{evo}:{}", hasher.finalize().to_hex())
        }
    }
}

/// Convert a Kiln [`SourceViewOperation`] to Basalt's [`SourceOperation`].
#[must_use]
pub const fn basalt_source_operation(operation: SourceViewOperation) -> SourceOperation {
    match operation {
        SourceViewOperation::Discover => SourceOperation::Discover,
        SourceViewOperation::ReadContent => SourceOperation::ReadContent,
        SourceViewOperation::ReadHistory => SourceOperation::ReadHistory,
        SourceViewOperation::PrepareWorkspace => SourceOperation::PrepareWorkspace,
        SourceViewOperation::ModifySource => SourceOperation::ModifySource,
        SourceViewOperation::CreateSource => SourceOperation::CreateSource,
        SourceViewOperation::Delegate => SourceOperation::Delegate,
        SourceViewOperation::RetrieveResult => SourceOperation::RetrieveResult,
    }
}

struct VerifiedSourceMode {
    policy: basalt::Policy,
    policy_reference: BasaltPolicyRef,
    generation: u64,
    audience: String,
    trusted_roots: Vec<String>,
    max_delegation_depth: u8,
    services: AuthorityServices,
}

struct LiveSourceSession {
    request_identity: String,
    scope: EffectiveSourceScope,
    basalt_session: SourceSession,
}

/// Basalt-backed `SourceAuthority` adapter enforcing `kiln://source/v1/`
/// UCAN grants, chain delegation depth, revocation, single-use replay, and
/// `check_request`.
pub struct BasaltSourceAuthority {
    mode: VerifiedSourceMode,
    sessions: BTreeMap<u64, LiveSourceSession>,
    next_session: u64,
    decisions: Vec<SourceAuthorityDecision>,
}

impl std::fmt::Debug for BasaltSourceAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BasaltSourceAuthority")
            .field("generation", &self.mode.generation)
            .field("audience", &self.mode.audience)
            .field("max_delegation_depth", &self.mode.max_delegation_depth)
            .field("sessions", &self.sessions.len())
            .field("decisions", &self.decisions.len())
            .finish_non_exhaustive()
    }
}

impl BasaltSourceAuthority {
    /// Build a verified `kiln://source/v1/` authority adapter.
    ///
    /// `max_delegation_depth` is bounded to `<= MAX_DELEGATION_DEPTH` (`8`).
    ///
    /// # Errors
    /// Returns a message when `max_delegation_depth` exceeds `MAX_DELEGATION_DEPTH`.
    pub fn verified(
        config: VerifiedAuthorityConfig,
        services: AuthorityServices,
        max_delegation_depth: u8,
    ) -> Result<Self, String> {
        if max_delegation_depth > MAX_DELEGATION_DEPTH {
            return Err(format!(
                "source max_delegation_depth {max_delegation_depth} exceeds {MAX_DELEGATION_DEPTH}"
            ));
        }
        Ok(Self {
            mode: VerifiedSourceMode {
                policy: config.policy,
                policy_reference: config.policy_reference,
                generation: config.generation,
                audience: config.audience,
                trusted_roots: config.trusted_roots,
                max_delegation_depth,
                services,
            },
            sessions: BTreeMap::new(),
            next_session: 1,
            decisions: Vec::new(),
        })
    }

    fn record(&mut self, decision: SourceAuthorityDecision) -> SourceAuthorityDecision {
        if self.decisions.len() >= MAX_SOURCE_DECISIONS {
            self.decisions.remove(0);
        }
        self.decisions.push(decision.clone());
        decision
    }

    fn deny(
        &mut self,
        request: &SourceAuthorityRequest,
        reason: SourceDenialReason,
    ) -> SourceAuthorityDecision {
        self.record(SourceAuthorityDecision::deny(request, reason))
    }

    fn verifier(&self) -> Verifier<'_> {
        Verifier {
            policy: &self.mode.policy,
            policy_ref: &self.mode.policy_reference,
            generation: self.mode.generation,
            audience: &self.mode.audience,
            trusted_roots: &self.mode.trusted_roots,
            max_delegation_depth: self.mode.max_delegation_depth,
            implementation_v1: basalt::kiln::LegacyFraming::Refuse,
        }
    }

    fn insert_session(&mut self, live: LiveSourceSession) -> AuthoritySessionId {
        if self.sessions.len() >= MAX_SOURCE_SESSIONS
            && let Some(oldest) = self.sessions.keys().next().copied()
        {
            self.sessions.remove(&oldest);
        }
        let id = self.next_session;
        self.next_session = self.next_session.saturating_add(1);
        self.sessions.insert(id, live);
        AuthoritySessionId(id)
    }

    fn map_token_error(error: &TokenError) -> SourceDenialReason {
        match error {
            TokenError::Revocation { .. } | TokenError::Revoked { .. } => {
                SourceDenialReason::Revoked
            }
            TokenError::Temporal { message } if message.contains("expired") => {
                SourceDenialReason::Expired
            }
            _ => SourceDenialReason::CredentialDenied,
        }
    }

    fn verify_token_and_depth(
        &self,
        compact_token: &str,
        now_unix_s: u64,
    ) -> Result<(u64, u8), SourceDenialReason> {
        let verification_time = VerificationTime::from_unix_seconds(now_unix_s);
        let revocations = BoundRevocations {
            view: self.mode.services.revocations.as_ref(),
            now_ms: revocation_instant_ms(now_unix_s),
        };
        let verified = verify_compact_token_str_with_resolvers_and_revocations(
            compact_token,
            verification_time,
            self.mode.services.key_resolver.as_ref(),
            self.mode.services.proof_store.as_ref(),
            &revocations,
            self.mode.services.verification_limits,
        )
        .map_err(|error| Self::map_token_error(&error))?;

        let expires_s = verified
            .effective_delegation()
            .time_bounds()
            .expires_at_seconds();
        if expires_s <= now_unix_s {
            return Err(SourceDenialReason::Expired);
        }

        let depth = self.max_proof_chain_depth(&verified, verification_time, &revocations)?;
        if depth > self.mode.max_delegation_depth {
            return Err(SourceDenialReason::CredentialDenied);
        }
        Ok((expires_s, depth))
    }

    fn max_proof_chain_depth(
        &self,
        leaf: &ucan::VerifiedToken,
        verification_time: VerificationTime,
        revocations: &BoundRevocations<'_>,
    ) -> Result<u8, SourceDenialReason> {
        let mut tokens = vec![leaf.clone()];
        let mut parents = vec![Vec::<usize>::new()];
        let mut known: HashMap<ProofReference, usize> = HashMap::new();
        let mut pending = vec![0_usize];
        let mut budget = self.mode.services.verification_limits.max_proof_count();
        while let Some(node) = pending.pop() {
            let references = tokens
                .get(node)
                .ok_or(SourceDenialReason::CredentialDenied)?
                .proofs()
                .as_slice()
                .to_vec();
            for reference in references {
                budget = budget
                    .checked_sub(1)
                    .ok_or(SourceDenialReason::CredentialDenied)?;
                let parent_idx = if let Some(&idx) = known.get(&reference) {
                    idx
                } else {
                    let proof = self
                        .mode
                        .services
                        .proof_store
                        .resolve_proof(&reference)
                        .map_err(|_| SourceDenialReason::CredentialDenied)?;
                    let proof_token = match &proof {
                        ProofToken::Borrowed(token) => *token,
                        ProofToken::Owned(token) => token,
                    };
                    if proof_reference(proof_token) != reference {
                        return Err(SourceDenialReason::CredentialDenied);
                    }
                    let verified_parent = verify_compact_token_str_with_resolvers_and_revocations(
                        proof_token.as_str(),
                        verification_time,
                        self.mode.services.key_resolver.as_ref(),
                        self.mode.services.proof_store.as_ref(),
                        revocations,
                        self.mode.services.verification_limits,
                    )
                    .map_err(|error| Self::map_token_error(&error))?;
                    let idx = tokens.len();
                    tokens.push(verified_parent);
                    parents.push(Vec::new());
                    known.insert(reference, idx);
                    pending.push(idx);
                    idx
                };
                parents
                    .get_mut(node)
                    .ok_or(SourceDenialReason::CredentialDenied)?
                    .push(parent_idx);
            }
        }
        let mut remaining_walk = self.mode.services.verification_limits.max_proof_count();
        longest_chain_depth(&parents, 0, 0, &mut remaining_walk)
    }

    fn evaluate_admit(
        &self,
        request: &SourceAuthorityRequest,
        profile: &AuthorityProfile,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<LiveSourceSession, SourceDenialReason> {
        // Invariant B: cheap / shell checks run BEFORE Verifier::verify_source
        // so invalid, handle-scoped, stale-attempt, or mismatched requests
        // never consume replay nonces.
        if request.validate().is_err() {
            return Err(SourceDenialReason::RequestInvalid);
        }
        let req_path = match &request.scope {
            OriginScope::Handle { .. } => return Err(SourceDenialReason::HandleConfersNothing),
            OriginScope::Components { path } => path,
        };
        if request.attempt.is_some() && current.attempt != AttemptCurrentness::Current {
            return Err(SourceDenialReason::AttemptNotCurrent);
        }
        let credential = match profile {
            AuthorityProfile::Missing => return Err(SourceDenialReason::MissingEvidence),
            AuthorityProfile::LocalOperator { .. } => {
                return Err(SourceDenialReason::VerifierUnavailable);
            }
            AuthorityProfile::VerifiedUcan(credential) => credential,
        };
        if current.generation != self.mode.generation {
            return Err(SourceDenialReason::StaleGeneration);
        }
        if !request.snapshot.admits(&current.snapshot) {
            return Err(SourceDenialReason::SnapshotNotAdmitted);
        }
        let basalt_op = basalt_source_operation(request.operation);
        if basalt_op.requires_entry() && req_path.subtree {
            return Err(SourceDenialReason::RequestInvalid);
        }
        let (expires_s, chain_depth) =
            self.verify_token_and_depth(credential.compact_token(), now_unix_s)?;

        let exact = basalt_source_request(
            request,
            req_path,
            credential.subject(),
            self.mode.generation,
            &self.mode.audience,
        );
        let signature = SignerSignature::from_bytes(*credential.holder_signature());
        let invocation = Invocation {
            compact_token: credential.compact_token(),
            nonce: HolderInvocationNonce::from_bytes(*credential.nonce()),
            holder_signature: &signature,
        };
        let now_ms = now_unix_s.saturating_mul(MILLIS_PER_SECOND);
        let token_expires_ms = expires_s.saturating_mul(MILLIS_PER_SECOND);
        let revocations = BoundRevocations {
            view: self.mode.services.revocations.as_ref(),
            now_ms: revocation_instant_ms(now_unix_s),
        };
        let replay = BoundReplay {
            ledger: self.mode.services.replay.as_ref(),
            generation: self.mode.generation,
            token_ref: *blake3::hash(credential.compact_token().as_bytes()).as_bytes(),
            expires_ms: token_expires_ms
                .max(now_ms.saturating_add(self.mode.services.replay_retention_ms)),
            now_ms,
        };
        let boundaries = UcanEnforcementBoundaries {
            verification_time: VerificationTime::from_unix_seconds(now_unix_s),
            key_resolver: self.mode.services.key_resolver.as_ref(),
            proof_store: self.mode.services.proof_store.as_ref(),
            revocations: &revocations,
            caveat_policies: self.mode.services.caveat_policies.as_ref(),
            replay: &replay,
            verification_limits: self.mode.services.verification_limits,
            holder_limits: self.mode.services.holder_limits,
        };

        // Invariant C: always verify through Basalt without admit-level session
        // caching so single-use replay consumption is enforced on every call.
        let basalt_session = self
            .verifier()
            .verify_source(exact, &invocation, &boundaries)
            .map_err(|Denied| SourceDenialReason::CredentialDenied)?;

        let basalt_scope = if req_path.subtree {
            BasaltPathScope::Subtree
        } else {
            BasaltPathScope::Entry
        };
        if basalt_session
            .require(
                basalt_op,
                &req_path.components,
                basalt_scope,
                request.request_digest,
            )
            .is_err()
        {
            return Err(SourceDenialReason::BindingMismatch);
        }

        // Invariant D: parse effective_capabilities strictly via kiln://source/v1/
        // framing, admitting ONLY request.requested_grant().
        let admitted: Vec<SourceGrant> = request.requested_grant().into_iter().collect();
        let capabilities = parse_effective_capabilities(
            basalt_session.effective_capabilities(),
            credential.subject(),
            &self.mode.audience,
            self.mode.generation,
            request,
            req_path,
            current.revocation_generation,
        );
        let facts = EffectiveScopeFacts {
            audience: credential.subject().to_string(),
            repository: request.repository.clone(),
            snapshot: request.snapshot.clone(),
            revision: current.snapshot.revision.clone(),
            view_binding: request.view_binding.clone(),
            admitted,
            capabilities,
            not_after_s: basalt_session.expires_unix_s(),
            depth: self.mode.max_delegation_depth.saturating_sub(chain_depth),
            generation: self.mode.generation,
            revocation_generation: current.revocation_generation,
            provenance: AuthorityProvenance::VerifiedUcan,
        };
        let scope =
            EffectiveSourceScope::new(facts).map_err(|_| SourceDenialReason::RequestInvalid)?;
        check_request(&scope, request, current, now_unix_s)?;
        Ok(LiveSourceSession {
            request_identity: request.digest_hex(),
            scope,
            basalt_session,
        })
    }

    fn revalidate_verified(
        &self,
        live: &LiveSourceSession,
        now_unix_s: u64,
    ) -> Result<(), SourceDenialReason> {
        let revocations = BoundRevocations {
            view: self.mode.services.revocations.as_ref(),
            now_ms: revocation_instant_ms(now_unix_s),
        };
        let boundaries = UcanEnforcementBoundaries {
            verification_time: VerificationTime::from_unix_seconds(now_unix_s),
            key_resolver: self.mode.services.key_resolver.as_ref(),
            proof_store: self.mode.services.proof_store.as_ref(),
            revocations: &revocations,
            caveat_policies: self.mode.services.caveat_policies.as_ref(),
            replay: &NoReplay,
            verification_limits: self.mode.services.verification_limits,
            holder_limits: self.mode.services.holder_limits,
        };
        match self
            .verifier()
            .revalidate_source(&live.basalt_session, &boundaries)
        {
            Ok(()) => Ok(()),
            Err(Denied) => {
                // Distinguish revocation / expiry for accurate diagnostics.
                // Note: live session does not expose raw token string, so check
                // expires_unix_s first; if not expired, check whether revocation
                // view failed/revoked via verifier's boundaries? We know
                // check_request already checked live_at, generation, and
                // revocation_generation, so any failure inside revalidate_source
                // at this point is either token/proof revocation, stale
                // revocation view, or policy change.
                if now_unix_s >= live.basalt_session.expires_unix_s() {
                    Err(SourceDenialReason::Expired)
                } else {
                    Err(SourceDenialReason::Revoked)
                }
            }
        }
    }
}

// r[impl kiln.source.projection.authority.port]
impl SourceAuthority for BasaltSourceAuthority {
    fn admit(
        &mut self,
        request: &SourceAuthorityRequest,
        profile: &AuthorityProfile,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<SourceAuthorityDecision, SourceAuthorityDecision> {
        match self.evaluate_admit(request, profile, current, now_unix_s) {
            Ok(live) => {
                let policy_ref = live.basalt_session.policy_identity().to_string();
                let id = self.insert_session(live);
                let Some(stored) = self.sessions.get(&id.0) else {
                    return Err(self.deny(request, SourceDenialReason::SessionNotFound));
                };
                let decision =
                    SourceAuthorityDecision::allowed(request, id, &stored.scope, policy_ref);
                Ok(self.record(decision))
            }
            Err(reason) => Err(self.deny(request, reason)),
        }
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &SourceAuthorityRequest,
        current: &SourceCurrentness,
        now_unix_s: u64,
    ) -> Result<(), SourceAuthorityDecision> {
        let Some(live) = self.sessions.get(&session.0) else {
            return Err(self.deny(request, SourceDenialReason::SessionNotFound));
        };
        if live.request_identity != request.digest_hex() {
            self.sessions.remove(&session.0);
            return Err(self.deny(request, SourceDenialReason::BindingMismatch));
        }
        if let Err(reason) = check_request(&live.scope, request, current, now_unix_s) {
            self.sessions.remove(&session.0);
            return Err(self.deny(request, reason));
        }
        if let Err(reason) = self.revalidate_verified(live, now_unix_s) {
            self.sessions.remove(&session.0);
            return Err(self.deny(request, reason));
        }
        let Some(live) = self.sessions.get(&session.0) else {
            return Err(self.deny(request, SourceDenialReason::SessionNotFound));
        };
        let decision = SourceAuthorityDecision::allowed(
            request,
            session,
            &live.scope,
            live.basalt_session.policy_identity().to_string(),
        );
        self.record(decision);
        Ok(())
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.sessions.remove(&session.0);
    }

    fn effective_scope(&self, session: AuthoritySessionId) -> Option<&EffectiveSourceScope> {
        self.sessions.get(&session.0).map(|live| &live.scope)
    }

    fn decisions(&self) -> &[SourceAuthorityDecision] {
        &self.decisions
    }
}

fn longest_chain_depth(
    parents: &[Vec<usize>],
    node: usize,
    depth: u8,
    budget: &mut usize,
) -> Result<u8, SourceDenialReason> {
    let node_parents = parents
        .get(node)
        .ok_or(SourceDenialReason::CredentialDenied)?;
    if node_parents.is_empty() {
        return Ok(depth);
    }
    let next_depth = depth
        .checked_add(1)
        .ok_or(SourceDenialReason::CredentialDenied)?;
    let mut deepest = next_depth;
    for &parent in node_parents {
        *budget = budget
            .checked_sub(1)
            .ok_or(SourceDenialReason::CredentialDenied)?;
        let candidate = longest_chain_depth(parents, parent, next_depth, budget)?;
        deepest = deepest.max(candidate);
    }
    Ok(deepest)
}

fn basalt_source_request(
    request: &SourceAuthorityRequest,
    path: &PathScope,
    subject: &str,
    generation: u64,
    audience: &str,
) -> SourceRequest {
    SourceRequest {
        operation: basalt_source_operation(request.operation),
        subject: subject.into(),
        audience: audience.into(),
        generation,
        repository: request.repository.as_str().into(),
        attempt: request.attempt.as_ref().map(|attempt| SourceAttempt {
            attempt: attempt.attempt.as_str().into(),
            fence: attempt.fence,
        }),
        path: path.components.clone(),
        scope: if path.subtree {
            BasaltPathScope::Subtree
        } else {
            BasaltPathScope::Entry
        },
        snapshot: encode_snapshot_rule(&request.snapshot),
        view: request.view_binding.as_str().into(),
        request_digest: request.request_digest,
    }
}

// ---------------------------------------------------------------------------
// Strict `kiln://source/v1/` capability parser (Invariant D)

fn parse_effective_capabilities(
    set: &CapabilitySet,
    holder_subject: &str,
    deployment_audience: &str,
    generation: u64,
    request: &SourceAuthorityRequest,
    req_path: &PathScope,
    revocation_generation: u64,
) -> Vec<SessionCapability> {
    let encoded_snapshot = encode_snapshot_rule(&request.snapshot);
    let expected_digest_hex = hex_bytes(&request.request_digest);
    let context = CapabilityContext {
        audience: holder_subject.into(),
        repository: request.repository.clone(),
        snapshot: request.snapshot.clone(),
        view_binding: request.view_binding.clone(),
        generation,
        revocation_generation,
    };

    let mut merged: BTreeMap<PathScope, BTreeSet<SourceViewOperation>> = BTreeMap::new();
    for cap in set.as_slice() {
        let Some(operation) = operation_for_ability(&cap.ability) else {
            continue;
        };
        let Some(path) = parse_capability_resource(
            &cap.resource,
            deployment_audience,
            generation,
            request.repository.as_str(),
            request.attempt.as_ref(),
            &encoded_snapshot,
            request.view_binding.as_str(),
            &expected_digest_hex,
            req_path,
        ) else {
            continue;
        };
        merged.entry(path).or_default().insert(operation);
    }

    // Prioritize confirming capabilities for the admitted request if > MAX_SOURCE_GRANTS.
    let mut entries: Vec<(PathScope, BTreeSet<SourceViewOperation>)> = merged.into_iter().collect();
    if entries.len() > MAX_SOURCE_GRANTS {
        entries.sort_by_key(|(path, ops)| {
            let covers_req = ops.contains(&request.operation) && path.contains(req_path);
            !covers_req
        });
        entries.truncate(MAX_SOURCE_GRANTS);
    }

    entries
        .into_iter()
        .map(|(path, operations)| SessionCapability {
            grant: SourceGrant { path, operations },
            context: context.clone(),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn parse_capability_resource(
    resource: &str,
    expected_audience: &str,
    expected_generation: u64,
    expected_repository: &str,
    expected_attempt: Option<&AttemptFence>,
    expected_snapshot: &str,
    expected_view: &str,
    expected_digest_hex: &str,
    req_path: &PathScope,
) -> Option<PathScope> {
    let rest = resource.strip_prefix(SOURCE_V1_PREFIX)?.as_bytes();
    let (aud, rest) = take_text(rest)?;
    if aud != expected_audience {
        return None;
    }
    let (gen_val, rest) = take_number(rest)?;
    if gen_val != expected_generation {
        return None;
    }
    let (repo, rest) = take_text(rest)?;
    if repo != expected_repository {
        return None;
    }
    if rest.is_empty() {
        return Some(PathScope::root());
    }
    let (ctx_marker, mut rest) = take_token(rest)?;
    match (ctx_marker, expected_attempt) {
        (b"n", None) => {}
        (b"a", Some(attempt)) => {
            if rest.is_empty() {
                return Some(PathScope::root());
            }
            let (att_id, after_id) = take_text(rest)?;
            if att_id != attempt.attempt.as_str() {
                return None;
            }
            if after_id.is_empty() {
                return Some(PathScope::root());
            }
            let (fence_val, after_fence) = take_number(after_id)?;
            if fence_val != attempt.fence {
                return None;
            }
            rest = after_fence;
        }
        _ => return None,
    }

    let mut components = Vec::new();
    let terminal = loop {
        if rest.is_empty() {
            break None;
        }
        let (tok, after) = take_token(rest)?;
        if tok == b"e" {
            if components.is_empty() {
                return None;
            }
            rest = after;
            break Some(BasaltPathScope::Entry);
        }
        if tok == b"s" {
            rest = after;
            break Some(BasaltPathScope::Subtree);
        }
        let (comp, after_comp) = take_text(rest)?;
        components.push(comp);
        if components.len() > MAX_COMPONENTS {
            return None;
        }
        rest = after_comp;
    };

    if !rest.is_empty() {
        let (snap, after_snap) = take_text(rest)?;
        if snap != expected_snapshot {
            return None;
        }
        rest = after_snap;
    }
    if !rest.is_empty() {
        let (view, after_view) = take_text(rest)?;
        if view != expected_view {
            return None;
        }
        rest = after_view;
    }
    if !rest.is_empty() {
        let (dig, after_dig) = take_token(rest)?;
        if dig != expected_digest_hex.as_bytes() || !after_dig.is_empty() {
            return None;
        }
    }

    let path_scope = match terminal {
        None => PathScope {
            components,
            subtree: true,
        },
        Some(BasaltPathScope::Entry) => PathScope {
            components,
            subtree: false,
        },
        Some(BasaltPathScope::Subtree) => {
            // Invariant D: `s/` in Basalt only covers subtree-scoped requests at
            // that exact node, never descendants or entries. Emit it ONLY when
            // the admitted request targets that exact subtree node.
            if !req_path.subtree || req_path.components != components {
                return None;
            }
            PathScope {
                components,
                subtree: true,
            }
        }
    };
    path_scope.validate().ok()?;
    Some(path_scope)
}

fn take_token(rest: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = rest.iter().position(|byte| *byte == b'/')?;
    Some((rest.get(..end)?, rest.get(end.checked_add(1)?..)?))
}

fn canonical_decimal(digits: &[u8]) -> Option<u64> {
    if digits.is_empty()
        || (digits.len() > 1 && digits.first() == Some(&b'0'))
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

fn take_number(rest: &[u8]) -> Option<(u64, &[u8])> {
    let (digits, after) = take_token(rest)?;
    Some((canonical_decimal(digits)?, after))
}

fn take_text(rest: &[u8]) -> Option<(String, &[u8])> {
    let (len_bytes, after_len) = take_token(rest)?;
    let length = usize::try_from(canonical_decimal(len_bytes)?).ok()?;
    if length == 0 || length > MAX_FIELD_BYTES {
        return None;
    }
    let (hex_bytes, after_hex) = take_token(after_len)?;
    if hex_bytes.len() != length * 2 {
        return None;
    }
    let mut decoded = Vec::with_capacity(length);
    for pair in hex_bytes.chunks_exact(2) {
        let hi = hex_nibble(*pair.first()?)?;
        let lo = hex_nibble(*pair.get(1)?)?;
        decoded.push((hi << 4) | lo);
    }
    let text = String::from_utf8(decoded).ok()?;
    Some((text, after_hex))
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

// ---------------------------------------------------------------------------
// Holder key, SourceSigner, and token issuance helpers

/// One source holder's Ed25519 key and compact `kiln://source/v1/` UCAN grant.
pub struct SourceHolderKey {
    signer: Ed25519InMemorySigner,
    did: String,
    token: String,
}

impl std::fmt::Debug for SourceHolderKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceHolderKey")
            .field("did", &self.did)
            .field("token_bytes", &self.token.len())
            .finish_non_exhaustive()
    }
}

impl SourceHolderKey {
    /// Bind a source holder to its Ed25519 seed and compact UCAN grant.
    ///
    /// # Errors
    /// Returns a message when the DID cannot be derived or the token is invalid.
    pub fn new(seed: [u8; ED25519_SECRET_KEY_BYTES], token: String) -> Result<Self, String> {
        let signer = Ed25519InMemorySigner::from_seed_bytes(seed);
        let did = signer
            .issuer()
            .map_err(|_| "source holder DID cannot be derived".to_string())?
            .as_str()
            .to_string();
        if token.is_empty() || !token.is_ascii() {
            return Err("source holder grant must be a non-empty compact token".into());
        }
        Ok(Self { signer, did, token })
    }

    /// DID of the source holder.
    #[must_use]
    pub fn did(&self) -> &str {
        &self.did
    }
}

/// Signs [`SourceAuthorityRequest`] values for a single `kiln://source/v1/` holder.
pub struct SourceSigner {
    policy: basalt::Policy,
    policy_reference: BasaltPolicyRef,
    generation: u64,
    audience: String,
    max_delegation_depth: u8,
    holder: SourceHolderKey,
    nonce: NonceSource,
}

impl std::fmt::Debug for SourceSigner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceSigner")
            .field("generation", &self.generation)
            .field("audience", &self.audience)
            .field("holder", &self.holder)
            .finish_non_exhaustive()
    }
}

impl SourceSigner {
    /// Build a source signer bound to the verifier's policy and deployment.
    ///
    /// # Errors
    /// Returns a message when `max_delegation_depth` exceeds `MAX_DELEGATION_DEPTH`.
    pub fn new(
        config: &VerifiedAuthorityConfig,
        holder: SourceHolderKey,
        max_delegation_depth: u8,
        nonce: NonceSource,
    ) -> Result<Self, String> {
        if max_delegation_depth > MAX_DELEGATION_DEPTH {
            return Err(format!(
                "source max_delegation_depth {max_delegation_depth} exceeds {MAX_DELEGATION_DEPTH}"
            ));
        }
        Ok(Self {
            policy: config.policy.clone(),
            policy_reference: config.policy_reference.clone(),
            generation: config.generation,
            audience: config.audience.clone(),
            max_delegation_depth,
            holder,
            nonce,
        })
    }

    /// DID of the signing holder.
    #[must_use]
    pub fn did(&self) -> &str {
        self.holder.did()
    }

    /// Sign one exact [`SourceAuthorityRequest`].
    ///
    /// # Errors
    /// Returns a diagnostic when the request is invalid, cannot be canonicalized,
    /// no fresh nonce is available, or holder signing fails.
    pub fn sign(&self, request: &SourceAuthorityRequest) -> CoreResult<AuthorityCredential> {
        let denied = |message: &str| Diagnostic::new("source_authority_signer", "holder", message);
        request.validate()?;
        let req_path = match &request.scope {
            OriginScope::Components { path } => path,
            OriginScope::Handle { .. } => {
                return Err(denied("a handle request cannot be signed as source scope"));
            }
        };
        let exact = basalt_source_request(
            request,
            req_path,
            &self.holder.did,
            self.generation,
            &self.audience,
        );
        let operation = exact.operation;
        let enforcement = Verifier {
            policy: &self.policy,
            policy_ref: &self.policy_reference,
            generation: self.generation,
            audience: &self.audience,
            trusted_roots: &[],
            max_delegation_depth: self.max_delegation_depth,
            implementation_v1: basalt::kiln::LegacyFraming::Refuse,
        }
        .source_enforcement(&exact)
        .map_err(|_| denied("source request has no canonical resource under the policy"))?;
        let binding = bind_enforcement_action(operation.action(), &enforcement)
            .map_err(|_| denied("source operation binding refused"))?;
        let nonce = (self.nonce)().ok_or_else(|| denied("no fresh nonce is available"))?;
        let audience = AudienceDid::from(
            self.holder
                .signer
                .issuer()
                .map_err(|_| denied("holder DID cannot be derived"))?,
        );
        let signature = sign_holder_invocation(
            &HolderInvocationSigningRequest::new(
                &self.holder.token,
                &audience,
                enforcement.resource(),
                enforcement.ability(),
                binding,
                HolderInvocationNonce::from_bytes(nonce),
            ),
            &self.holder.signer,
            VerificationLimits::default(),
            HolderInvocationLimits::default(),
        )
        .map_err(|_| denied("holder source invocation cannot be signed"))?;
        let signature: [u8; AUTHORITY_SIGNATURE_BYTES] = signature
            .as_bytes()
            .try_into()
            .map_err(|_| denied("holder signature width"))?;
        AuthorityCredential::new(
            self.holder.did.clone(),
            self.holder.token.clone(),
            nonce,
            signature,
        )
    }
}

/// Issue a `kiln://source/v1/` compact UCAN token for a set of `(PathScope, operations)` grants.
///
/// Subtree scopes (`subtree: true`) emit `terminal: None` so the prefix covers
/// both entry-only (`e/`) and subtree (`s/`) requests under that path. Exact
/// entry scopes (`subtree: false`) emit `terminal: Some(Entry)`.
///
/// # Errors
/// Returns a message if any scope fails framing or token signing fails.
#[allow(clippy::too_many_arguments)]
pub fn issue_source_grant(
    issuer_seed: [u8; ED25519_SECRET_KEY_BYTES],
    recipient_did: &str,
    audience: &str,
    generation: u64,
    repository: &ContentId,
    attempt: Option<&AttemptFence>,
    grants: &[(PathScope, Vec<SourceViewOperation>)],
    proofs: &[&str],
    not_before_s: u64,
    expires_s: u64,
) -> Result<String, String> {
    let issuer = Ed25519InMemorySigner::from_seed_bytes(issuer_seed);
    let recipient = AudienceDid::new(recipient_did.to_string())
        .map_err(|error| format!("invalid recipient DID: {error}"))?;
    let basalt_attempt = attempt.map(|a| SourceAttempt {
        attempt: a.attempt.as_str().to_string(),
        fence: a.fence,
    });
    let mut docs = Vec::new();
    for (path, ops) in grants {
        path.validate()
            .map_err(|diag| format!("{}: {}", diag.code, diag.message))?;
        let prefix = SourceScope {
            audience,
            generation,
            repository: repository.as_str(),
            attempt: basalt_attempt.as_ref(),
            path: &path.components,
            terminal: if path.subtree {
                None
            } else {
                Some(BasaltPathScope::Entry)
            },
        }
        .prefix()
        .map_err(|Denied| "source scope framing denied".to_string())?;
        for op in ops {
            docs.push(
                CapabilityDocument::new(prefix.clone(), source_coverage(*op).ability.to_string())
                    .map_err(|error| format!("capability document failed: {error}"))?,
            );
        }
    }
    let cap_set =
        CapabilitySet::new(docs).map_err(|error| format!("capability set failed: {error}"))?;
    let mut refs = Vec::with_capacity(proofs.len());
    for proof in proofs {
        let compact =
            CompactToken::new(proof).map_err(|error| format!("invalid proof token: {error}"))?;
        refs.push(proof_reference(&compact));
    }
    let bounds = TokenTimeBounds::new(not_before_s, expires_s)
        .map_err(|error| format!("invalid token bounds: {error}"))?;
    let token = issue_token_with_signer(
        &issuer,
        &recipient,
        &cap_set,
        &ProofReferences::from_references(refs),
        bounds,
    )
    .map_err(|error| format!("failed to issue source token: {error}"))?;
    Ok(token.as_str().to_string())
}

/// Materialize an admitted [`SharePlan`] into a real delegated `kiln://source/v1/`
/// compact UCAN token signed by the sharer.
///
/// # Errors
/// Returns a message if framing, proof parsing, or token issuance fails.
pub fn issue_delegated_source_grant(
    plan: &SharePlan,
    issuer_seed: [u8; ED25519_SECRET_KEY_BYTES],
    audience: &str,
    attempt: Option<&AttemptFence>,
    proofs: &[&str],
    not_before_s: u64,
) -> Result<String, String> {
    let grants: Vec<(PathScope, Vec<SourceViewOperation>)> = plan
        .grants()
        .iter()
        .map(|grant| {
            (
                grant.path.clone(),
                grant.operations.iter().copied().collect(),
            )
        })
        .collect();
    issue_source_grant(
        issuer_seed,
        plan.recipient(),
        audience,
        plan.generation(),
        plan.repository(),
        attempt,
        &grants,
        proofs,
        not_before_s,
        plan.not_after_s(),
    )
}

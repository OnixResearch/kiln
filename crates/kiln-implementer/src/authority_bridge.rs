//! Shell-side authority request construction, currentness and admission.
//!
//! The core owns request and decision facts. This module reads Kiln's current
//! work/attempt/fence state and calls the configured protected-authority port
//! before each protected effect.

use kiln_core::implementation::authority::{
    AUTHORITY_DIGEST_BYTES, AttemptCurrentness, AuthorityDecision, AuthorityRequest,
    AuthoritySelection, AuthoritySessionId, NO_EXPECTED_TARGET, ProtectedAuthority,
    ProtectedOperation, payload_digest_v2,
};
use kiln_core::implementation::{
    AttemptLedger, AttemptPhase, ImplementationAttempt, ImplementationWork, TransportLaunch,
    TransportLaunches, WorkLedger,
};
use kiln_core::types::{ContentId, ExternalRef};

use super::{ATTEMPT_KIND, Implementer, TRANSPORT_LAUNCH_KIND, WORK_KIND, load, text};

/// What one protected payload binds beyond its parts (C2).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PayloadBinding<'a> {
    /// The attempt's recorded transport launch, or none before launch.
    pub(crate) incarnation: Option<&'a ContentId>,
    /// Stable reservation or effect identity; a fresh invocation of the same
    /// effect reuses it.
    pub(crate) effect: &'a str,
    /// Expected remote target state, or `NO_EXPECTED_TARGET`.
    pub(crate) expected_target: &'a str,
}

/// Worker incarnation of one recorded transport launch, or none before launch.
#[must_use]
pub(crate) fn incarnation(launch: Option<&TransportLaunch>) -> Option<ContentId> {
    kiln_core::implementation::authority::incarnation_identity(launch?).ok()
}

/// Canonical digest of one external reference.
#[must_use]
pub(crate) fn external_digest(reference: &ExternalRef) -> [u8; AUTHORITY_DIGEST_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln.implementation.authority-external.v1");
    for part in [
        reference.provider_namespace.as_str(),
        reference.native_kind.as_str(),
        reference.native_id.as_str(),
    ] {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

// r[impl kiln.implementation.authority.payload]
/// Build one exact request from trusted work and attempt facts.
///
/// # Errors
/// Returns a message when the work identity cannot be formed or the effect
/// identity is empty.
pub(crate) fn request_for(
    operation: ProtectedOperation,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    binding: PayloadBinding<'_>,
    payload: &[&str],
    candidate: Option<&ExternalRef>,
) -> Result<AuthorityRequest, String> {
    if binding.effect.is_empty() || binding.expected_target.is_empty() {
        return Err("authority: payload requires an effect identity and expected target".into());
    }
    let work_identity = work
        .identity()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let target = format!(
        "kiln://implementation/v1/{}/{}/{}",
        operation.basalt_operation(),
        work_identity.as_str(),
        attempt.identity.as_str()
    );
    Ok(AuthorityRequest {
        operation,
        project: work.project.clone(),
        work: work_identity.as_str().into(),
        work_version: work.revision,
        attempt: attempt.identity.as_str().into(),
        fence: attempt.fence,
        request_digest: payload_digest_v2(
            operation,
            binding.incarnation,
            binding.effect,
            binding.expected_target,
            payload,
        ),
        target,
        candidate: candidate.map_or([0; AUTHORITY_DIGEST_BYTES], external_digest),
    })
}

/// The work version an attempt froze: the current version when it still
/// matches, otherwise the retained superseded version (a `FinishActive`
/// supersession keeps the attempt current on it). Every protected request of
/// an attempt names this work, never a newer revision of the same key.
#[must_use]
pub(crate) fn frozen_work<'a>(
    work: &'a WorkLedger,
    attempt: &ImplementationAttempt,
) -> Option<&'a ImplementationWork> {
    let frozen = |candidate: &&ImplementationWork| {
        candidate.revision == attempt.work_revision
            && candidate
                .identity()
                .is_ok_and(|identity| identity == attempt.work_identity)
    };
    work.get(&attempt.canonical_key)
        .filter(frozen)
        .or_else(|| work.superseded().iter().find(frozen))
}

/// Build one exact request over the attempt's frozen work from a ledger.
///
/// # Errors
/// Returns a message when the attempt's frozen work is not retained or its
/// identity is invalid.
pub(crate) fn request_from_ledger(
    ledger: &WorkLedger,
    attempt: &ImplementationAttempt,
    operation: ProtectedOperation,
    binding: PayloadBinding<'_>,
    payload: &[&str],
    candidate: Option<&ExternalRef>,
) -> Result<AuthorityRequest, String> {
    let work = frozen_work(ledger, attempt)
        .ok_or_else(|| "authority: attempt work missing".to_string())?;
    request_for(operation, work, attempt, binding, payload, candidate)
}

/// Redacted shell error for one denial.
#[must_use]
pub(crate) fn authority_error(decision: &AuthorityDecision) -> String {
    format!(
        "authority: {} ({})",
        decision.reason.as_str(),
        decision.request_identity
    )
}

/// Admit one request through a borrowable authority slot. The selected
/// profile presents fresh evidence for this exact request.
///
/// # Errors
/// Returns a redacted message when no authority is configured, the entry
/// point did not load the holder role of the operation, or the request denies.
pub(crate) fn admit_with(
    slot: &mut Option<Box<dyn ProtectedAuthority>>,
    selection: &AuthoritySelection,
    request: &AuthorityRequest,
    currentness: AttemptCurrentness,
    now_ms: u64,
) -> Result<AuthoritySessionId, String> {
    let Some(authority) = slot.as_mut() else {
        return Err("authority: no authority profile selected".into());
    };
    // r[impl kiln.implementation.authority.selection.scenario.scoped]
    // Refused before anything is signed, asked of the verifier, mutated or
    // read: this process holds no key for the operation's role.
    if !selection.holds(request.operation) {
        return Err(format!(
            "authority: holder-not-loaded ({:?} needs the {:?} holder, which this entry point does not load)",
            request.operation,
            kiln_core::implementation::grant::GrantRole::of(request.operation),
        ));
    }
    let profile = selection.profile_for(request);
    match authority.admit(request, &profile, currentness, now_ms / 1_000) {
        Ok(decision) => decision
            .session
            .ok_or_else(|| "authority: allowed decision had no live session".into()),
        Err(decision) => Err(authority_error(&decision)),
    }
}

/// Revalidate one live session through a borrowable authority slot.
///
/// # Errors
/// Returns a redacted message when no authority is configured or revalidation denies.
pub(crate) fn revalidate_with(
    slot: &mut Option<Box<dyn ProtectedAuthority>>,
    session: AuthoritySessionId,
    request: &AuthorityRequest,
    currentness: AttemptCurrentness,
    now_ms: u64,
) -> Result<(), String> {
    let Some(authority) = slot.as_mut() else {
        return Err("authority: no authority profile selected".into());
    };
    authority
        .revalidate(session, request, currentness, now_ms / 1_000)
        .map_err(|decision| authority_error(&decision))
}

/// The selected authority, the last admitted session and the attempt's
/// recorded launches, lent to one transaction so a `Dispatch` decision is
/// admitted before that transaction saves anything.
pub(crate) struct DispatchGate<'a> {
    pub(crate) authority: &'a mut Option<Box<dyn ProtectedAuthority>>,
    pub(crate) profile: &'a AuthoritySelection,
    pub(crate) last: &'a mut Option<(AuthoritySessionId, AuthorityRequest)>,
    pub(crate) launches: &'a TransportLaunches,
    pub(crate) now_ms: u64,
}

// r[impl kiln.implementation.authority.dispatch]
/// Admit one exact `Dispatch` request for `attempt` as it will be saved: its
/// reservation effect (or the attempt before one exists), its recorded
/// incarnation and `payload`. The caller has already applied the attempt
/// ledger's own fence and phase checks in this transaction, so the attempt
/// is current. When the last admitted session is for this exact request (a
/// pre-check before credentials), that session is revalidated instead of
/// admitting a second grant; otherwise a fresh invocation is admitted and
/// becomes the last session.
///
/// # Errors
/// Returns the redacted denial; the caller then saves nothing.
pub(crate) fn authorize_dispatch(
    gate: &mut DispatchGate<'_>,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    payload: &[&str],
) -> Result<(), String> {
    let effect = attempt
        .dispatch
        .as_ref()
        .map_or(attempt.identity.as_str(), |dispatch| {
            dispatch.effect_id.as_str()
        });
    let incarnation = incarnation(gate.launches.recall(&attempt.identity).as_ref());
    let request = request_for(
        ProtectedOperation::Dispatch,
        work,
        attempt,
        PayloadBinding {
            incarnation: incarnation.as_ref(),
            effect,
            expected_target: NO_EXPECTED_TARGET,
        },
        payload,
        None,
    )?;
    let current = AttemptCurrentness::Current;
    if let Some((session, admitted)) = gate.last.as_ref()
        && admitted == &request
    {
        return revalidate_with(gate.authority, *session, &request, current, gate.now_ms);
    }
    let session = admit_with(gate.authority, gate.profile, &request, current, gate.now_ms)?;
    *gate.last = Some((session, request.clone()));
    revalidate_with(gate.authority, session, &request, current, gate.now_ms)
}

/// Trusted facts one protected request is built from, read in one transaction.
#[derive(Clone, Debug)]
pub(crate) struct AuthorityFacts {
    pub(crate) work: ImplementationWork,
    pub(crate) attempt: ImplementationAttempt,
    /// The attempt's worker incarnation, or none before its first launch.
    pub(crate) incarnation: Option<ContentId>,
}

impl AuthorityFacts {
    /// Payload binding over this attempt's incarnation.
    pub(crate) fn binding<'a>(
        &'a self,
        effect: &'a str,
        expected_target: &'a str,
    ) -> PayloadBinding<'a> {
        PayloadBinding {
            incarnation: self.incarnation.as_ref(),
            effect,
            expected_target,
        }
    }

    /// Build one exact request from these facts.
    pub(crate) fn request(
        &self,
        operation: ProtectedOperation,
        effect: &str,
        expected_target: &str,
        payload: &[&str],
        candidate: Option<&ExternalRef>,
    ) -> Result<AuthorityRequest, String> {
        request_for(
            operation,
            &self.work,
            &self.attempt,
            self.binding(effect, expected_target),
            payload,
            candidate,
        )
    }
}

impl Implementer {
    /// Currentness of one attempt for the authority port.
    pub(crate) fn attempt_currentness(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> AttemptCurrentness {
        let Ok(transaction) = self.connection.transaction() else {
            return AttemptCurrentness::Unavailable;
        };
        let Ok((attempts, _)) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND) else {
            return AttemptCurrentness::Unavailable;
        };
        let Some(attempt) = attempts.get(identity) else {
            return AttemptCurrentness::Unavailable;
        };
        if attempt.fence != fence {
            return AttemptCurrentness::FenceReplaced;
        }
        match attempt.phase {
            AttemptPhase::Cancelled => AttemptCurrentness::Cancelled,
            AttemptPhase::Superseded => AttemptCurrentness::Superseded,
            AttemptPhase::Claimed
            | AttemptPhase::Executing
            | AttemptPhase::CandidateProduced
            | AttemptPhase::Verifying => {
                if attempts.is_current(identity, fence, now_ms) {
                    AttemptCurrentness::Current
                } else {
                    AttemptCurrentness::LeaseExpired
                }
            }
            AttemptPhase::Verified => {
                if attempt.superseded_by.is_some() {
                    AttemptCurrentness::Superseded
                } else {
                    AttemptCurrentness::Current
                }
            }
            AttemptPhase::Blocked | AttemptPhase::Failed => AttemptCurrentness::Unavailable,
        }
    }

    /// Read the exact frozen work, attempt and incarnation for one protected
    /// request.
    ///
    /// # Errors
    /// Returns a message when the attempt, fence or frozen work is absent.
    pub(crate) fn authority_facts(
        &mut self,
        identity: &ContentId,
        fence: u64,
    ) -> Result<AuthorityFacts, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let (launches, _) = load::<TransportLaunches>(&transaction, TRANSPORT_LAUNCH_KIND)?;
        let attempt = attempts
            .get(identity)
            .cloned()
            .ok_or_else(|| "authority: attempt missing".to_string())?;
        if attempt.fence != fence {
            return Err("authority: attempt fence was replaced".into());
        }
        let work = frozen_work(&work, &attempt)
            .cloned()
            .ok_or_else(|| "authority: attempt work missing".to_string())?;
        transaction.commit().map_err(text)?;
        Ok(AuthorityFacts {
            work,
            incarnation: incarnation(launches.recall(identity).as_ref()),
            attempt,
        })
    }

    /// Admit one exact request and retain its session as the last admission.
    ///
    /// # Errors
    /// Returns a redacted message on denial.
    pub(crate) fn admit_authority(
        &mut self,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_ms: u64,
    ) -> Result<AuthoritySessionId, String> {
        let session = admit_with(
            &mut self.authority,
            &self.authority_profile,
            request,
            currentness,
            now_ms,
        )?;
        self.authority_last = Some((session, request.clone()));
        Ok(session)
    }

    /// Revalidate one live session.
    ///
    /// # Errors
    /// Returns a redacted message on denial.
    pub(crate) fn revalidate_authority(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_ms: u64,
    ) -> Result<(), String> {
        revalidate_with(&mut self.authority, session, request, currentness, now_ms)
    }

    // r[impl kiln.implementation.authority.currentness]
    /// Admit and immediately revalidate one request.
    ///
    /// # Errors
    /// Returns a redacted message on denial.
    pub(crate) fn authorize_and_revalidate(
        &mut self,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_ms: u64,
    ) -> Result<(), String> {
        let session = self.admit_authority(request, currentness, now_ms)?;
        self.revalidate_authority(session, request, currentness, now_ms)
    }

    /// Revalidate the most recently admitted session before a later effect.
    ///
    /// # Errors
    /// Returns a message when no session was admitted in this process.
    pub(crate) fn revalidate_last_authority(
        &mut self,
        currentness: AttemptCurrentness,
        now_ms: u64,
    ) -> Result<(), String> {
        let Some((session, request)) = self.authority_last.clone() else {
            return Err("authority: no admitted session for this effect".into());
        };
        self.revalidate_authority(session, &request, currentness, now_ms)
    }
}

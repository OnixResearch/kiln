//! Shell-side authority request construction, currentness and admission.
//!
//! The core owns request and decision facts. This module reads Kiln's current
//! work/attempt/fence state and calls the configured protected-authority port
//! before each protected effect.

use kiln_core::implementation::authority::{
    AttemptCurrentness, AuthorityDecision, AuthorityProfile, AuthorityRequest, AuthoritySessionId,
    ProtectedAuthority, ProtectedOperation,
};
use kiln_core::implementation::{
    AttemptLedger, AttemptPhase, ImplementationAttempt, ImplementationWork, WorkLedger,
};
use kiln_core::types::{ContentId, ExternalRef};

use super::{ATTEMPT_KIND, Implementer, WORK_KIND, load, text};

/// Canonical digest of one protected payload.
#[must_use]
pub(crate) fn payload_digest(
    operation: ProtectedOperation,
    parts: &[&str],
) -> [u8; kiln_core::implementation::authority::AUTHORITY_DIGEST_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln.implementation.authority-payload.v1");
    hasher.update(operation.basalt_operation().as_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Canonical digest of one external reference.
#[must_use]
pub(crate) fn external_digest(
    reference: &ExternalRef,
) -> [u8; kiln_core::implementation::authority::AUTHORITY_DIGEST_BYTES] {
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

/// Build one exact request from trusted work and attempt facts.
///
/// # Errors
/// Returns a message when the work identity cannot be formed.
pub(crate) fn request_for(
    operation: ProtectedOperation,
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    payload: &[&str],
    candidate: Option<&ExternalRef>,
) -> Result<AuthorityRequest, String> {
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
        request_digest: payload_digest(operation, payload),
        target,
        candidate: candidate.map_or(
            [0; kiln_core::implementation::authority::AUTHORITY_DIGEST_BYTES],
            external_digest,
        ),
    })
}

/// Build one exact request using the attempt's current work from a ledger.
///
/// # Errors
/// Returns a message when the attempt's work is absent or its identity is invalid.
pub(crate) fn request_from_ledger(
    ledger: &WorkLedger,
    attempt: &ImplementationAttempt,
    operation: ProtectedOperation,
    payload: &[&str],
    candidate: Option<&ExternalRef>,
) -> Result<AuthorityRequest, String> {
    let work = ledger
        .get(&attempt.canonical_key)
        .ok_or_else(|| "authority: attempt work missing".to_string())?;
    request_for(operation, work, attempt, payload, candidate)
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

/// Admit one request through a borrowable authority slot.
///
/// # Errors
/// Returns a redacted message when no authority is configured or the request denys.
pub(crate) fn admit_with(
    slot: &mut Option<Box<dyn ProtectedAuthority>>,
    profile: &AuthorityProfile,
    request: &AuthorityRequest,
    currentness: AttemptCurrentness,
    now_ms: u64,
) -> Result<AuthoritySessionId, String> {
    let Some(authority) = slot.as_mut() else {
        return Err("authority: no verified session configured".into());
    };
    match authority.admit(request, profile, currentness, now_ms / 1_000) {
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
        return Err("authority: no verified session configured".into());
    };
    authority
        .revalidate(session, request, currentness, now_ms / 1_000)
        .map_err(|decision| authority_error(&decision))
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

    /// Read the exact current work and attempt for one protected request.
    ///
    /// # Errors
    /// Returns a message when the attempt, fence or work is absent.
    pub(crate) fn authority_facts(
        &mut self,
        identity: &ContentId,
        fence: u64,
    ) -> Result<(ImplementationWork, ImplementationAttempt), String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (work, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let attempt = attempts
            .get(identity)
            .cloned()
            .ok_or_else(|| "authority: attempt missing".to_string())?;
        if attempt.fence != fence {
            return Err("authority: attempt fence was replaced".into());
        }
        let work = work
            .get(&attempt.canonical_key)
            .cloned()
            .ok_or_else(|| "authority: attempt work missing".to_string())?;
        transaction.commit().map_err(text)?;
        Ok((work, attempt))
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

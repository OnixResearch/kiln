//! Verified finite transcript helpers for UCAN replay-admission decisions.
//!
//! This module proves only fail-closed admission over caller/shell-supplied
//! replay outcome facts. Nonce uniqueness truth, request identity provenance,
//! clock freshness, durable replay storage, crash recovery, backend I/O, parser
//! behavior, and consumer envelope semantics remain explicit runtime boundaries
//! in callers.

use vstd::prelude::*;

verus! {

pub const REPLAY_ALLOWED: u8 = 0;
pub const REPLAY_DUPLICATE: u8 = 1;
pub const REPLAY_STALE: u8 = 2;
pub const REPLAY_MALFORMED: u8 = 3;
pub const REPLAY_DENIED: u8 = 4;
pub const REPLAY_BACKEND: u8 = 5;
pub const REPLAY_ACCEPTED: u8 = 255;

/// Caller/shell-supplied public fact for one replay-admission decision.
pub struct ReplayAdmissionFact {
    pub outcome: u8,
}

/// Caller/shell-supplied finite replay-admission transcript.
pub struct ReplayAdmissionTranscript {
    pub facts: Vec<ReplayAdmissionFact>,
}

/// A known replay-admission outcome is either allowed or one fail-closed denial class.
// r[ucan-replay-admission.known-outcome]
pub open spec fn known_replay_admission_outcome_spec(outcome: u8) -> bool {
    outcome == REPLAY_ALLOWED
        || outcome == REPLAY_DUPLICATE
        || outcome == REPLAY_STALE
        || outcome == REPLAY_MALFORMED
        || outcome == REPLAY_DENIED
        || outcome == REPLAY_BACKEND
}

/// Transcript acceptance is exactly a non-empty transcript with no failed replay outcome.
// r[ucan-replay-admission.accepts-allowed-transcript]
pub open spec fn replay_admission_transcript_accepts_spec(
    transcript: &ReplayAdmissionTranscript,
) -> bool {
    transcript.facts.len() > 0 && first_failed_replay_admission_spec(transcript) == REPLAY_ACCEPTED
}

/// First failed replay-admission outcome in transcript order, or accepted sentinel.
// r[ucan-replay-admission.first-failure]
pub open spec fn first_failed_replay_admission_spec(transcript: &ReplayAdmissionTranscript) -> u8 {
    if transcript.facts.len() == 0 {
        REPLAY_ACCEPTED
    } else {
        first_failed_replay_admission_from_spec(transcript, 0)
    }
}

pub open spec fn first_failed_replay_admission_from_spec(
    transcript: &ReplayAdmissionTranscript,
    index: int,
) -> u8
    decreases transcript.facts.len() - index
{
    if index >= transcript.facts.len() {
        REPLAY_ACCEPTED
    } else if !known_replay_admission_outcome_spec(transcript.facts[index].outcome)
        || transcript.facts[index].outcome != REPLAY_ALLOWED
    {
        transcript.facts[index].outcome
    } else {
        first_failed_replay_admission_from_spec(transcript, index + 1)
    }
}

/// Check whether a finite replay-admission transcript accepts.
// r[ucan-replay-admission.accepts-allowed-transcript]
// r[impl ucan-replay-admission.accepts-allowed-transcript]
pub fn replay_admission_transcript_accepts(
    transcript: &ReplayAdmissionTranscript,
) -> (result: bool)
    ensures result == replay_admission_transcript_accepts_spec(transcript),
{
    transcript.facts.len() > 0 && first_failed_replay_admission(transcript) == REPLAY_ACCEPTED
}

/// Return the first failed replay-admission outcome in transcript order.
// r[ucan-replay-admission.first-failure]
// r[impl ucan-replay-admission.first-failure]
pub fn first_failed_replay_admission(transcript: &ReplayAdmissionTranscript) -> (outcome: u8)
    ensures outcome == first_failed_replay_admission_spec(transcript),
{
    let mut index: usize = 0;
    while index < transcript.facts.len()
        invariant
            0 <= index <= transcript.facts.len(),
            forall|i: int| 0 <= i < index ==> #[trigger]
                known_replay_admission_outcome_spec(transcript.facts[i].outcome)
                    && transcript.facts[i].outcome == REPLAY_ALLOWED,
            first_failed_replay_admission_spec(transcript)
                == first_failed_replay_admission_from_spec(transcript, index as int),
        decreases transcript.facts.len() - index
    {
        let outcome = transcript.facts[index].outcome;
        if !known_replay_admission_outcome(outcome) || outcome != REPLAY_ALLOWED {
            return outcome;
        }
        index += 1;
    }
    REPLAY_ACCEPTED
}

// r[impl ucan-replay-admission.known-outcome]
pub fn known_replay_admission_outcome(outcome: u8) -> (result: bool)
    ensures result == known_replay_admission_outcome_spec(outcome),
{
    outcome == REPLAY_ALLOWED
        || outcome == REPLAY_DUPLICATE
        || outcome == REPLAY_STALE
        || outcome == REPLAY_MALFORMED
        || outcome == REPLAY_DENIED
        || outcome == REPLAY_BACKEND
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn accepting_transcript() -> ReplayAdmissionTranscript {
        ReplayAdmissionTranscript {
            facts: vec![ReplayAdmissionFact {
                outcome: REPLAY_ALLOWED,
            }],
        }
    }

    #[test]
    fn accepting_transcript_has_accepted_sentinel() {
        let transcript = accepting_transcript();
        assert!(replay_admission_transcript_accepts(&transcript));
        assert_eq!(first_failed_replay_admission(&transcript), REPLAY_ACCEPTED);
    }

    #[test]
    fn first_failed_outcome_uses_transcript_order() {
        let transcript = ReplayAdmissionTranscript {
            facts: vec![
                ReplayAdmissionFact {
                    outcome: REPLAY_ALLOWED,
                },
                ReplayAdmissionFact { outcome: REPLAY_STALE },
                ReplayAdmissionFact {
                    outcome: REPLAY_BACKEND,
                },
            ],
        };
        assert!(!replay_admission_transcript_accepts(&transcript));
        assert_eq!(first_failed_replay_admission(&transcript), REPLAY_STALE);
    }

    #[test]
    fn every_known_denial_rejects() {
        for outcome in [
            REPLAY_DUPLICATE,
            REPLAY_STALE,
            REPLAY_MALFORMED,
            REPLAY_DENIED,
            REPLAY_BACKEND,
        ] {
            let transcript = ReplayAdmissionTranscript {
                facts: vec![ReplayAdmissionFact { outcome }],
            };
            assert!(!replay_admission_transcript_accepts(&transcript));
            assert_eq!(first_failed_replay_admission(&transcript), outcome);
        }
    }

    #[test]
    fn unknown_outcome_rejects() {
        let transcript = ReplayAdmissionTranscript {
            facts: vec![ReplayAdmissionFact { outcome: 42 }],
        };
        assert!(!replay_admission_transcript_accepts(&transcript));
        assert_eq!(first_failed_replay_admission(&transcript), 42);
    }

    #[test]
    fn empty_transcript_rejects_but_has_sentinel() {
        let transcript = ReplayAdmissionTranscript { facts: vec![] };
        assert!(!replay_admission_transcript_accepts(&transcript));
        assert_eq!(first_failed_replay_admission(&transcript), REPLAY_ACCEPTED);
    }
}

//! Verified finite transcript helpers for UCAN backend-operation admission.
//!
//! This module proves only fail-closed admission over caller/shell-supplied
//! backend operation facts. Resolver/proof-store/revocation backend truth,
//! freshness, provenance, I/O, parsing, hashing, and cryptography remain explicit
//! trusted/runtime boundaries in callers.

use vstd::prelude::*;

verus! {

pub const BACKEND_KEY_RESOLUTION: u8 = 0;
pub const BACKEND_PROOF_LOAD: u8 = 1;
pub const BACKEND_REVOCATION: u8 = 2;
pub const BACKEND_ACCEPTED: u8 = 255;

/// Caller/shell-supplied public fact for one backend operation.
pub struct BackendOperationFact {
    pub operation: u8,
    pub succeeded: bool,
}

/// Caller/shell-supplied finite backend transcript.
pub struct BackendTranscript {
    pub operations: Vec<BackendOperationFact>,
}

/// A known backend operation is one of the UCAN shell backend kinds.
// r[ucan-backend-transcript.known-operation]
pub open spec fn known_backend_operation_spec(operation: u8) -> bool {
    operation == BACKEND_KEY_RESOLUTION
        || operation == BACKEND_PROOF_LOAD
        || operation == BACKEND_REVOCATION
}

/// Transcript acceptance is exactly non-empty transcript whose first failed operation is accepted sentinel.
// r[ucan-backend-transcript.accepts-successful-operations]
pub open spec fn backend_transcript_accepts_spec(transcript: &BackendTranscript) -> bool {
    transcript.operations.len() > 0 && first_failed_backend_operation_spec(transcript) == BACKEND_ACCEPTED
}

/// First failed operation in transcript order, or accepted sentinel.
// r[ucan-backend-transcript.first-failure]
pub open spec fn first_failed_backend_operation_spec(transcript: &BackendTranscript) -> u8 {
    if transcript.operations.len() == 0 {
        BACKEND_ACCEPTED
    } else {
        first_failed_backend_operation_from_spec(transcript, 0)
    }
}

pub open spec fn first_failed_backend_operation_from_spec(transcript: &BackendTranscript, index: int) -> u8
    decreases transcript.operations.len() - index
{
    if index >= transcript.operations.len() {
        BACKEND_ACCEPTED
    } else if !known_backend_operation_spec(transcript.operations[index].operation)
        || !transcript.operations[index].succeeded
    {
        transcript.operations[index].operation
    } else {
        first_failed_backend_operation_from_spec(transcript, index + 1)
    }
}

/// Check whether a finite backend transcript accepts.
// r[ucan-backend-transcript.accepts-successful-operations]
// r[impl ucan-backend-transcript.accepts-successful-operations]
pub fn backend_transcript_accepts(transcript: &BackendTranscript) -> (result: bool)
    ensures result == backend_transcript_accepts_spec(transcript),
{
    transcript.operations.len() > 0 && first_failed_backend_operation(transcript) == BACKEND_ACCEPTED
}

/// Return the first failed backend operation in fixed transcript order.
// r[ucan-backend-transcript.first-failure]
// r[impl ucan-backend-transcript.first-failure]
pub fn first_failed_backend_operation(transcript: &BackendTranscript) -> (operation: u8)
    ensures operation == first_failed_backend_operation_spec(transcript),
{
    let mut index: usize = 0;
    while index < transcript.operations.len()
        invariant
            0 <= index <= transcript.operations.len(),
            forall|i: int| 0 <= i < index ==> #[trigger]
                known_backend_operation_spec(transcript.operations[i].operation)
                    && transcript.operations[i].succeeded,
            first_failed_backend_operation_spec(transcript)
                == first_failed_backend_operation_from_spec(transcript, index as int),
        decreases transcript.operations.len() - index
    {
        let operation = transcript.operations[index].operation;
        if !known_backend_operation(operation) || !transcript.operations[index].succeeded {
            return operation;
        }
        index += 1;
    }
    BACKEND_ACCEPTED
}

// r[impl ucan-backend-transcript.known-operation]
pub fn known_backend_operation(operation: u8) -> (result: bool)
    ensures result == known_backend_operation_spec(operation),
{
    operation == BACKEND_KEY_RESOLUTION
        || operation == BACKEND_PROOF_LOAD
        || operation == BACKEND_REVOCATION
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn accepting_transcript() -> BackendTranscript {
        BackendTranscript {
            operations: vec![
                BackendOperationFact {
                    operation: BACKEND_KEY_RESOLUTION,
                    succeeded: true,
                },
                BackendOperationFact {
                    operation: BACKEND_PROOF_LOAD,
                    succeeded: true,
                },
                BackendOperationFact {
                    operation: BACKEND_REVOCATION,
                    succeeded: true,
                },
            ],
        }
    }

    #[test]
    fn accepting_transcript_has_accepted_sentinel() {
        let transcript = accepting_transcript();
        assert!(backend_transcript_accepts(&transcript));
        assert_eq!(first_failed_backend_operation(&transcript), BACKEND_ACCEPTED);
    }

    #[test]
    fn first_failed_operation_uses_transcript_order() {
        let mut transcript = accepting_transcript();
        transcript.operations[1].succeeded = false;
        transcript.operations[2].succeeded = false;
        assert!(!backend_transcript_accepts(&transcript));
        assert_eq!(first_failed_backend_operation(&transcript), BACKEND_PROOF_LOAD);
    }

    #[test]
    fn unknown_operation_rejects() {
        let transcript = BackendTranscript {
            operations: vec![BackendOperationFact {
                operation: 42,
                succeeded: true,
            }],
        };
        assert!(!backend_transcript_accepts(&transcript));
        assert_eq!(first_failed_backend_operation(&transcript), 42);
    }
}

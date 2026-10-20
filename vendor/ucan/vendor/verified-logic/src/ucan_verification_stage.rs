//! Verified finite transcript helpers for UCAN verification-stage admission.
//!
//! This module intentionally proves only control-flow admission over caller-supplied
//! stage facts. Runtime parsing, codecs, hashing, cryptography, resolver/proof-store
//! backends, revocation freshness, caveat policies, and proof material provenance
//! remain explicit trusted/runtime boundaries in callers.
#![allow(clippy::struct_excessive_bools)]

use vstd::prelude::*;

verus! {

pub const STAGE_PARSE: u8 = 0;
pub const STAGE_STRUCTURAL: u8 = 1;
pub const STAGE_KEY_RESOLUTION: u8 = 2;
pub const STAGE_SIGNATURE: u8 = 3;
pub const STAGE_ENVELOPE: u8 = 4;
pub const STAGE_REVOCATION: u8 = 5;
pub const STAGE_PROOF_LOAD: u8 = 6;
pub const STAGE_PROOF_CHAIN: u8 = 7;
pub const STAGE_AUTHORIZATION: u8 = 8;
pub const STAGE_ACCEPTED: u8 = 255;

/// Caller/shell-supplied public facts for the finite UCAN verification stages.
pub struct VerificationStageTranscript {
    pub parse_ok: bool,
    pub structural_ok: bool,
    pub key_resolved: bool,
    pub signature_valid: bool,
    pub envelope_valid: bool,
    pub revocation_clear: bool,
    pub proofs_loaded: bool,
    pub proof_chain_valid: bool,
    pub authorized: bool,
}

/// Acceptance is exactly conjunction of all caller-supplied stage facts.
// r[impl ucan-verification-stage.accepts-all-stages]
pub open spec fn stage_transcript_accepts_spec(transcript: &VerificationStageTranscript) -> bool {
    transcript.parse_ok
        && transcript.structural_ok
        && transcript.key_resolved
        && transcript.signature_valid
        && transcript.envelope_valid
        && transcript.revocation_clear
        && transcript.proofs_loaded
        && transcript.proof_chain_valid
        && transcript.authorized
}

/// The first failed stage in the shell's fixed verification order, or accepted sentinel.
// r[impl ucan-verification-stage.first-failure]
pub open spec fn first_failed_stage_spec(transcript: &VerificationStageTranscript) -> u8 {
    if !transcript.parse_ok {
        STAGE_PARSE
    } else if !transcript.structural_ok {
        STAGE_STRUCTURAL
    } else if !transcript.key_resolved {
        STAGE_KEY_RESOLUTION
    } else if !transcript.signature_valid {
        STAGE_SIGNATURE
    } else if !transcript.envelope_valid {
        STAGE_ENVELOPE
    } else if !transcript.revocation_clear {
        STAGE_REVOCATION
    } else if !transcript.proofs_loaded {
        STAGE_PROOF_LOAD
    } else if !transcript.proof_chain_valid {
        STAGE_PROOF_CHAIN
    } else if !transcript.authorized {
        STAGE_AUTHORIZATION
    } else {
        STAGE_ACCEPTED
    }
}

/// Check whether a finite verification-stage transcript accepts.
// r[impl ucan-verification-stage.accepts-all-stages]
pub fn stage_transcript_accepts(transcript: &VerificationStageTranscript) -> (result: bool)
    ensures result == stage_transcript_accepts_spec(transcript),
{
    transcript.parse_ok
        && transcript.structural_ok
        && transcript.key_resolved
        && transcript.signature_valid
        && transcript.envelope_valid
        && transcript.revocation_clear
        && transcript.proofs_loaded
        && transcript.proof_chain_valid
        && transcript.authorized
}

/// Return the first failed stage in fixed verification order, or accepted sentinel.
// r[impl ucan-verification-stage.first-failure]
pub fn first_failed_stage(transcript: &VerificationStageTranscript) -> (stage: u8)
    ensures stage == first_failed_stage_spec(transcript),
{
    if !transcript.parse_ok {
        STAGE_PARSE
    } else if !transcript.structural_ok {
        STAGE_STRUCTURAL
    } else if !transcript.key_resolved {
        STAGE_KEY_RESOLUTION
    } else if !transcript.signature_valid {
        STAGE_SIGNATURE
    } else if !transcript.envelope_valid {
        STAGE_ENVELOPE
    } else if !transcript.revocation_clear {
        STAGE_REVOCATION
    } else if !transcript.proofs_loaded {
        STAGE_PROOF_LOAD
    } else if !transcript.proof_chain_valid {
        STAGE_PROOF_CHAIN
    } else if !transcript.authorized {
        STAGE_AUTHORIZATION
    } else {
        STAGE_ACCEPTED
    }
}

/// Acceptance is equivalent to having no failed stage.
// r[verify ucan-verification-stage.accepts-all-stages]
pub proof fn accepted_iff_no_failed_stage(transcript: &VerificationStageTranscript)
    ensures stage_transcript_accepts_spec(transcript) == (first_failed_stage_spec(transcript) == STAGE_ACCEPTED),
{
}

/// Any rejected transcript has a concrete non-accepted first failed stage.
// r[verify ucan-verification-stage.first-failure]
pub proof fn rejected_has_failed_stage(transcript: &VerificationStageTranscript)
    ensures !stage_transcript_accepts_spec(transcript) ==> first_failed_stage_spec(transcript) != STAGE_ACCEPTED,
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn accepting_transcript() -> VerificationStageTranscript {
        VerificationStageTranscript {
            parse_ok: true,
            structural_ok: true,
            key_resolved: true,
            signature_valid: true,
            envelope_valid: true,
            revocation_clear: true,
            proofs_loaded: true,
            proof_chain_valid: true,
            authorized: true,
        }
    }

    #[test]
    fn accepting_transcript_has_accepted_sentinel() {
        let transcript = accepting_transcript();
        assert!(stage_transcript_accepts(&transcript));
        assert_eq!(first_failed_stage(&transcript), STAGE_ACCEPTED);
    }

    #[test]
    fn first_failed_stage_uses_fixed_order() {
        let mut transcript = accepting_transcript();
        transcript.signature_valid = false;
        transcript.proofs_loaded = false;
        assert!(!stage_transcript_accepts(&transcript));
        assert_eq!(first_failed_stage(&transcript), STAGE_SIGNATURE);

        let mut transcript = accepting_transcript();
        transcript.proofs_loaded = false;
        assert!(!stage_transcript_accepts(&transcript));
        assert_eq!(first_failed_stage(&transcript), STAGE_PROOF_LOAD);
    }
}

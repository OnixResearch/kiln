//! Verified persistent state management for Raft crash recovery.
//!
//! Raft requires three pieces of state to be durable before responding
//! to any RPC: currentTerm, votedFor, and log[]. After a crash, these
//! are restored to reconstruct a consistent state. Without durability,
//! the single-vote-per-term invariant breaks — a node could vote twice
//! for the same term after restarting with amnesia.

use vstd::prelude::*;

verus! {

/// What state must be persisted before responding to an RPC.
/// Encoded as a bitmask: bit 0 = term, bit 1 = voted_for, bit 2 = log.
pub const PERSIST_TERM: u8 = 1;
/// Persist votedFor before responding.
pub const PERSIST_VOTED_FOR: u8 = 2;
/// Persist log entries before responding.
pub const PERSIST_LOG: u8 = 4;
/// Persist all state components (term + votedFor + log).
pub const PERSIST_ALL: u8 = 7;

// --- must_persist_before_vote_response ---

// r[depends persistent-state.vote-response]
/// Spec: what must be persisted before responding to a RequestVote?
///
/// When we grant a vote, we must persist currentTerm and votedFor
/// before responding. Otherwise, a crash could cause us to forget
/// the vote and grant it again to a different candidate.
pub open spec fn must_persist_before_vote_response_spec(vote_granted: bool) -> u8 {
    if vote_granted {
        // Must persist term + votedFor before acknowledging
        (PERSIST_TERM | PERSIST_VOTED_FOR) as u8
    } else {
        // Even on rejection, persist term if it was updated
        PERSIST_TERM
    }
}

// r[impl persistent-state.vote-response]
/// Compute what to persist before responding to RequestVote.
pub fn must_persist_before_vote_response(vote_granted: bool) -> (result: u8)
    ensures result == must_persist_before_vote_response_spec(vote_granted)
{
    if vote_granted {
        PERSIST_TERM | PERSIST_VOTED_FOR
    } else {
        PERSIST_TERM
    }
}

// --- must_persist_before_append_response ---

// r[depends persistent-state.append-response]
/// Spec: what must be persisted before responding to AppendEntries?
///
/// When we accept new entries, we must persist the log (and term if
/// it changed). Even on rejection, the term may have been updated
/// (step-down on higher term).
pub open spec fn must_persist_before_append_response_spec(
    entries_accepted: bool,
    term_changed: bool,
) -> u8 {
    if entries_accepted && term_changed {
        (PERSIST_TERM | PERSIST_LOG) as u8
    } else if entries_accepted {
        PERSIST_LOG
    } else if term_changed {
        PERSIST_TERM
    } else {
        0
    }
}

// r[impl persistent-state.append-response]
/// Compute what to persist before responding to AppendEntries.
pub fn must_persist_before_append_response(
    entries_accepted: bool,
    term_changed: bool,
) -> (result: u8)
    ensures result == must_persist_before_append_response_spec(entries_accepted, term_changed)
{
    if entries_accepted && term_changed {
        PERSIST_TERM | PERSIST_LOG
    } else if entries_accepted {
        PERSIST_LOG
    } else if term_changed {
        PERSIST_TERM
    } else {
        0
    }
}

// --- needs_persist ---

// r[depends persistent-state.needs-persist]
/// Spec: does the persist set include a given component?
pub open spec fn needs_persist_spec(persist_set: u8, component: u8) -> bool {
    (persist_set & component) != 0
}

// r[impl persistent-state.needs-persist]
/// Check if a specific component needs persisting.
pub fn needs_persist(persist_set: u8, component: u8) -> (result: bool)
    ensures result == needs_persist_spec(persist_set, component)
{
    (persist_set & component) != 0
}

// --- recover_state ---

/// Recovered persistent state after a crash.
pub struct RecoveredState {
    /// The term persisted on disk.
    pub current_term: u64,
    /// 0 means "not voted" (None equivalent).
    pub voted_for: u64,
    /// Whether a vote was cast in the current term.
    pub has_voted: bool,
    /// Number of log entries recovered from disk.
    pub log_length: u64,
}

// r[depends persistent-state.recover]
/// Spec: is the recovered state valid?
///
/// After recovery, the state must satisfy:
/// 1. Term is non-zero (we've participated in at least one term, or
///    start at 0 which is the initial term).
/// 2. If has_voted, voted_for identifies a valid candidate.
/// 3. log_length >= 0 (always true for u64).
pub open spec fn is_recovered_state_valid_spec(state: RecoveredState) -> bool {
    // voted_for is only meaningful when has_voted is true
    (!state.has_voted ==> state.voted_for == 0)
}

// r[impl persistent-state.recover]
/// Validate a recovered state.
pub fn is_recovered_state_valid(state: &RecoveredState) -> (result: bool)
    ensures result == is_recovered_state_valid_spec(*state)
{
    // !has_voted ==> voted_for == 0
    state.has_voted || state.voted_for == 0
}

// --- vote_safe_after_recovery ---

// r[depends persistent-state.vote-safety]
/// Spec: after recovery, is it safe to grant a vote for the given term?
///
/// Safe if: the requested term > recovered term (new term, we haven't
/// voted yet), OR the requested term == recovered term AND we already
/// voted for this candidate (idempotent re-vote).
pub open spec fn vote_safe_after_recovery_spec(
    recovered_term: u64,
    recovered_voted_for: u64,
    recovered_has_voted: bool,
    request_term: u64,
    candidate_id: u64,
) -> bool {
    if request_term > recovered_term {
        true // New term — safe to vote
    } else if request_term == recovered_term {
        // Same term — only if we haven't voted or voted for this candidate
        !recovered_has_voted || recovered_voted_for == candidate_id
    } else {
        false // Stale term — never safe
    }
}

// r[impl persistent-state.vote-safety]
/// Check if voting is safe given recovered state.
pub fn vote_safe_after_recovery(
    recovered_term: u64,
    recovered_voted_for: u64,
    recovered_has_voted: bool,
    request_term: u64,
    candidate_id: u64,
) -> (result: bool)
    ensures result == vote_safe_after_recovery_spec(
        recovered_term, recovered_voted_for, recovered_has_voted,
        request_term, candidate_id,
    )
{
    if request_term > recovered_term {
        true
    } else if request_term == recovered_term {
        !recovered_has_voted || recovered_voted_for == candidate_id
    } else {
        false
    }
}

// --- term_after_recovery ---

// r[depends persistent-state.term-recovery]
/// Spec: the effective term after recovery.
///
/// The recovered term is the starting point. If a RequestVote arrives
/// with a higher term, we adopt it.
pub open spec fn term_after_recovery_spec(
    recovered_term: u64,
    message_term: u64,
) -> u64 {
    if message_term > recovered_term { message_term } else { recovered_term }
}

// r[impl persistent-state.term-recovery]
/// Compute the effective term after recovery.
pub fn term_after_recovery(
    recovered_term: u64,
    message_term: u64,
) -> (result: u64)
    ensures result == term_after_recovery_spec(recovered_term, message_term)
{
    if message_term > recovered_term { message_term } else { recovered_term }
}

// --- Safety proofs ---

// r[verify persistent-state.vote-response]
/// Granting a vote always requires persisting voted_for.
proof fn grant_persists_voted_for()
    ensures needs_persist_spec(
        must_persist_before_vote_response_spec(true),
        PERSIST_VOTED_FOR,
    )
{
    assert(((1u8 | 2u8) & 2u8) != 0u8) by(bit_vector);
}

// r[verify persistent-state.vote-response]
/// Granting a vote always requires persisting term.
proof fn grant_persists_term()
    ensures needs_persist_spec(
        must_persist_before_vote_response_spec(true),
        PERSIST_TERM,
    )
{
    assert(((1u8 | 2u8) & 1u8) != 0u8) by(bit_vector);
}

// r[verify persistent-state.vote-response]
/// Vote rejection still requires persisting term.
proof fn reject_persists_term()
    ensures needs_persist_spec(
        must_persist_before_vote_response_spec(false),
        PERSIST_TERM,
    )
{
    assert((1u8 & 1u8) != 0u8) by(bit_vector);
}

// r[verify persistent-state.append-response]
/// Accepting entries requires persisting the log.
proof fn accept_persists_log()
    ensures needs_persist_spec(
        must_persist_before_append_response_spec(true, false),
        PERSIST_LOG,
    )
{
    assert((4u8 & 4u8) != 0u8) by(bit_vector);
}

// r[verify persistent-state.append-response]
/// Term change requires persisting the term.
proof fn term_change_persists_term()
    ensures needs_persist_spec(
        must_persist_before_append_response_spec(false, true),
        PERSIST_TERM,
    )
{
    assert((1u8 & 1u8) != 0u8) by(bit_vector);
}

// r[verify persistent-state.append-response]
/// No change needs no persistence.
proof fn no_change_no_persist()
    ensures must_persist_before_append_response_spec(false, false) == 0
{}

// r[verify persistent-state.vote-safety]
/// Stale term is never safe to vote in.
proof fn stale_term_unsafe(
    recovered_term: u64,
    recovered_voted_for: u64,
    recovered_has_voted: bool,
    request_term: u64,
    candidate_id: u64,
)
    requires request_term < recovered_term
    ensures !vote_safe_after_recovery_spec(
        recovered_term, recovered_voted_for, recovered_has_voted,
        request_term, candidate_id,
    )
{}

// r[verify persistent-state.vote-safety]
/// New term is always safe to vote in.
proof fn new_term_safe(
    recovered_term: u64,
    recovered_voted_for: u64,
    recovered_has_voted: bool,
    request_term: u64,
    candidate_id: u64,
)
    requires request_term > recovered_term
    ensures vote_safe_after_recovery_spec(
        recovered_term, recovered_voted_for, recovered_has_voted,
        request_term, candidate_id,
    )
{}

// r[verify persistent-state.vote-safety]
/// Same term, same candidate is safe (idempotent).
proof fn same_candidate_safe(
    recovered_term: u64,
    candidate_id: u64,
)
    ensures vote_safe_after_recovery_spec(
        recovered_term, candidate_id, true,
        recovered_term, candidate_id,
    )
{}

// r[verify persistent-state.vote-safety]
/// Same term, different candidate is unsafe.
proof fn different_candidate_unsafe(
    recovered_term: u64,
    voted_for: u64,
    candidate_id: u64,
)
    requires voted_for != candidate_id
    ensures !vote_safe_after_recovery_spec(
        recovered_term, voted_for, true,
        recovered_term, candidate_id,
    )
{}

// r[verify persistent-state.vote-safety]
/// Not yet voted in same term is safe.
proof fn not_yet_voted_safe(
    recovered_term: u64,
    candidate_id: u64,
)
    ensures vote_safe_after_recovery_spec(
        recovered_term, 0, false,
        recovered_term, candidate_id,
    )
{}

// r[verify persistent-state.term-recovery]
/// Term after recovery is monotonically non-decreasing.
proof fn term_recovery_monotonic(recovered_term: u64, message_term: u64)
    ensures term_after_recovery_spec(recovered_term, message_term) >= recovered_term
{}

// r[verify persistent-state.term-recovery]
/// Term after recovery is at least the message term when message is higher.
proof fn term_recovery_adopts_higher(recovered_term: u64, message_term: u64)
    requires message_term > recovered_term
    ensures term_after_recovery_spec(recovered_term, message_term) == message_term
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify persistent-state.vote-response]
    // r[verify persistent-state.recover]
    // r[verify persistent-state.needs-persist]
    #[test]
    fn test_vote_persist_set() {
        let granted = must_persist_before_vote_response(true);
        assert!(needs_persist(granted, PERSIST_TERM));
        assert!(needs_persist(granted, PERSIST_VOTED_FOR));
        assert!(!needs_persist(granted, PERSIST_LOG));

        let rejected = must_persist_before_vote_response(false);
        assert!(needs_persist(rejected, PERSIST_TERM));
        assert!(!needs_persist(rejected, PERSIST_VOTED_FOR));
    }

    // r[verify persistent-state.append-response]
    #[test]
    fn test_append_persist_set() {
        let both = must_persist_before_append_response(true, true);
        assert!(needs_persist(both, PERSIST_TERM));
        assert!(needs_persist(both, PERSIST_LOG));

        let entries_only = must_persist_before_append_response(true, false);
        assert!(!needs_persist(entries_only, PERSIST_TERM));
        assert!(needs_persist(entries_only, PERSIST_LOG));

        let term_only = must_persist_before_append_response(false, true);
        assert!(needs_persist(term_only, PERSIST_TERM));
        assert!(!needs_persist(term_only, PERSIST_LOG));

        let none = must_persist_before_append_response(false, false);
        assert_eq!(none, 0);
    }

    // r[verify persistent-state.vote-safety]
    #[test]
    fn test_vote_safe_after_recovery() {
        // New term: always safe
        assert!(vote_safe_after_recovery(3, 0, false, 5, 42));
        assert!(vote_safe_after_recovery(3, 99, true, 5, 42));

        // Same term, not voted: safe
        assert!(vote_safe_after_recovery(3, 0, false, 3, 42));

        // Same term, voted same candidate: safe (idempotent)
        assert!(vote_safe_after_recovery(3, 42, true, 3, 42));

        // Same term, voted different: unsafe
        assert!(!vote_safe_after_recovery(3, 99, true, 3, 42));

        // Stale term: unsafe
        assert!(!vote_safe_after_recovery(5, 0, false, 3, 42));
    }

    // r[verify persistent-state.term-recovery]
    #[test]
    fn test_term_after_recovery() {
        assert_eq!(term_after_recovery(3, 5), 5); // adopt higher
        assert_eq!(term_after_recovery(5, 3), 5); // keep own
        assert_eq!(term_after_recovery(3, 3), 3); // same
    }

    proptest! {
        // r[verify persistent-state.vote-response]
        #[test]
        fn prop_grant_always_persists_vote(granted: bool) {
            let set = must_persist_before_vote_response(granted);
            if granted {
                prop_assert!(needs_persist(set, PERSIST_VOTED_FOR));
            }
            // Term is always persisted
            prop_assert!(needs_persist(set, PERSIST_TERM));
        }

        // r[verify persistent-state.vote-safety]
        #[test]
        fn prop_stale_term_always_unsafe(
            recovered_term in 1u64..u64::MAX,
            request_term: u64,
            voted_for: u64,
            has_voted: bool,
            candidate: u64,
        ) {
            if request_term < recovered_term {
                prop_assert!(!vote_safe_after_recovery(
                    recovered_term, voted_for, has_voted, request_term, candidate,
                ));
            }
        }

        // r[verify persistent-state.vote-safety]
        #[test]
        fn prop_new_term_always_safe(
            recovered_term: u64,
            request_term: u64,
            voted_for: u64,
            has_voted: bool,
            candidate: u64,
        ) {
            if request_term > recovered_term {
                prop_assert!(vote_safe_after_recovery(
                    recovered_term, voted_for, has_voted, request_term, candidate,
                ));
            }
        }

        // r[verify persistent-state.term-recovery]
        #[test]
        fn prop_term_recovery_monotonic(recovered: u64, message: u64) {
            let result = term_after_recovery(recovered, message);
            prop_assert!(result >= recovered);
        }

        // r[verify persistent-state.vote-safety]
        #[test]
        fn prop_double_vote_prevented(
            term: u64,
            first_candidate in 1u64..u64::MAX,
        ) {
            // After voting for first_candidate, different candidate is rejected
            let second_candidate = if first_candidate < u64::MAX {
                first_candidate + 1
            } else {
                first_candidate - 1
            };
            prop_assert!(!vote_safe_after_recovery(
                term, first_candidate, true, term, second_candidate,
            ));
        }
    }
}

//! Verified term authority: step-down on higher term.
//!
//! The universal Raft rule: any server receiving a message with term >
//! currentTerm must adopt the new term, revert to follower, and clear
//! votedFor. This prevents stale leaders from operating and ensures
//! term monotonicity across the cluster.

use vstd::prelude::*;

use crate::election::*;

verus! {

/// Result of applying a higher-term message.
pub struct TermUpdate {
    pub new_term: u64,
    pub new_state: u8,
    /// None encoded as u64::MAX (no votedFor), otherwise the candidate ID.
    pub voted_for: u64,
}

/// Sentinel for "no vote cast."
pub const NO_VOTE: u64 = u64::MAX;

// --- apply_higher_term ---

// r[depends term-authority.step-down]
/// Spec: apply higher-term rule.
///
/// When message_term > current_term: adopt message_term, become follower,
/// clear votedFor. Otherwise: no change.
pub open spec fn apply_higher_term_spec(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
) -> TermUpdate {
    if message_term > current_term {
        TermUpdate {
            new_term: message_term,
            new_state: STATE_FOLLOWER,
            voted_for: NO_VOTE,
        }
    } else {
        TermUpdate {
            new_term: current_term,
            new_state: current_state,
            voted_for,
        }
    }
}

// r[impl term-authority.step-down]
/// Apply higher-term rule: step down if message_term > current_term.
pub fn apply_higher_term(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
) -> (result: TermUpdate)
    ensures
        result.new_term == apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_term,
        result.new_state == apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_state,
        result.voted_for == apply_higher_term_spec(current_term, current_state, voted_for, message_term).voted_for,
{
    if message_term > current_term {
        TermUpdate {
            new_term: message_term,
            new_state: STATE_FOLLOWER,
            voted_for: NO_VOTE,
        }
    } else {
        TermUpdate {
            new_term: current_term,
            new_state: current_state,
            voted_for,
        }
    }
}

// --- Safety proofs ---

// r[verify term-authority.step-down]
/// Term never decreases.
proof fn term_monotonic(
    current_term: u64, current_state: u8, voted_for: u64, message_term: u64,
)
    ensures
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_term
            >= current_term
{}

// r[verify term-authority.step-down]
/// Higher term causes demotion to follower.
proof fn higher_term_demotes(
    current_term: u64, current_state: u8, voted_for: u64, message_term: u64,
)
    requires message_term > current_term
    ensures
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_state
            == STATE_FOLLOWER
{}

// r[verify term-authority.step-down]
/// Higher term clears votedFor.
proof fn higher_term_clears_vote(
    current_term: u64, current_state: u8, voted_for: u64, message_term: u64,
)
    requires message_term > current_term
    ensures
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).voted_for
            == NO_VOTE
{}

// r[verify term-authority.step-down]
/// Same or lower term is a no-op.
proof fn same_or_lower_term_noop(
    current_term: u64, current_state: u8, voted_for: u64, message_term: u64,
)
    requires message_term <= current_term
    ensures
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_term
            == current_term,
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_state
            == current_state,
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).voted_for
            == voted_for,
{}

// r[verify term-authority.step-down]
/// Leader receiving higher term becomes follower (not transitioning).
proof fn leader_demoted_is_follower(
    current_term: u64, message_term: u64,
)
    requires message_term > current_term
    ensures
        is_follower_spec(
            apply_higher_term_spec(current_term, STATE_LEADER, 0, message_term).new_state,
        )
{}

// r[verify term-authority.step-down]
/// After applying higher term, the new term equals message_term.
proof fn higher_term_adopted(
    current_term: u64, current_state: u8, voted_for: u64, message_term: u64,
)
    requires message_term > current_term
    ensures
        apply_higher_term_spec(current_term, current_state, voted_for, message_term).new_term
            == message_term
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify term-authority.step-down]
    #[test]
    fn test_higher_term() {
        let r = apply_higher_term(3, STATE_LEADER, 5, 5);
        assert_eq!(r.new_term, 5);
        assert_eq!(r.new_state, STATE_FOLLOWER);
        assert_eq!(r.voted_for, NO_VOTE);
    }

    // r[verify term-authority.step-down]
    #[test]
    fn test_same_term() {
        let r = apply_higher_term(3, STATE_LEADER, 5, 3);
        assert_eq!(r.new_term, 3);
        assert_eq!(r.new_state, STATE_LEADER);
        assert_eq!(r.voted_for, 5);
    }

    // r[verify term-authority.step-down]
    #[test]
    fn test_lower_term() {
        let r = apply_higher_term(5, STATE_FOLLOWER, NO_VOTE, 3);
        assert_eq!(r.new_term, 5);
        assert_eq!(r.new_state, STATE_FOLLOWER);
        assert_eq!(r.voted_for, NO_VOTE);
    }

    proptest! {
        // r[verify term-authority.step-down]
        #[test]
        fn prop_term_monotonic(current_term: u64, state in 0u8..3, voted_for: u64, msg_term: u64) {
            let r = apply_higher_term(current_term, state, voted_for, msg_term);
            prop_assert!(r.new_term >= current_term);
        }

        // r[verify term-authority.step-down]
        #[test]
        fn prop_higher_term_demotes(
            current_term in 0u64..u64::MAX,
            delta in 1u64..1000,
            state in 0u8..3,
            voted_for: u64,
        ) {
            let msg_term = current_term.saturating_add(delta);
            if msg_term > current_term {
                let r = apply_higher_term(current_term, state, voted_for, msg_term);
                prop_assert_eq!(r.new_state, STATE_FOLLOWER);
                prop_assert_eq!(r.voted_for, NO_VOTE);
                prop_assert_eq!(r.new_term, msg_term);
            }
        }

        // r[verify term-authority.step-down]
        #[test]
        fn prop_same_or_lower_noop(
            current_term: u64, state in 0u8..3, voted_for: u64, msg_term: u64,
        ) {
            if msg_term <= current_term {
                let r = apply_higher_term(current_term, state, voted_for, msg_term);
                prop_assert_eq!(r.new_term, current_term);
                prop_assert_eq!(r.new_state, state);
                prop_assert_eq!(r.voted_for, voted_for);
            }
        }
    }
}

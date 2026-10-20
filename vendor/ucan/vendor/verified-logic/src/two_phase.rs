//! Verified two-phase commit decision logic.
//!
//! Models the coordinator's commit/abort decision as a pure function over
//! an array of participant votes. The safety property: commit happens only
//! when every participant voted yes. Abort is always safe.
//!
//! Votes are `Option<bool>`: None = no vote yet, Some(true) = yes,
//! Some(false) = no.

#![allow(clippy::nonminimal_bool, clippy::ptr_arg, clippy::len_zero)]

use vstd::prelude::*;

verus! {

/// Decision constants (int-encoded, same pattern as batch::FlushDecision).
/// 0 = Pending, 1 = Commit, 2 = Abort.

// --- can_commit ---

// r[depends two-phase.can-commit]
/// Spec: can commit iff every vote is Some(true).
pub open spec fn can_commit_spec(votes: Seq<Option<bool>>) -> bool {
    votes.len() > 0
    && forall|i: int| 0 <= i < votes.len() as int ==> votes[i] == Some(true)
}

// r[impl two-phase.can-commit]
/// Returns true if all participants voted yes.
pub fn can_commit(votes: &Vec<Option<bool>>) -> (result: bool)
    ensures result == can_commit_spec(votes@)
{
    if votes.len() == 0 {
        return false;
    }
    let mut i: usize = 0;
    while i < votes.len()
        invariant
            i <= votes.len(),
            votes.len() > 0,
            forall|j: int| 0 <= j < i as int ==> votes@[j] == Some(true),
        decreases votes.len() - i,
    {
        if votes[i] != Some(true) {
            return false;
        }
        i += 1;
    }
    true
}

// --- must_abort ---

// r[depends two-phase.must-abort]
/// Spec: must abort if any vote is Some(false).
pub open spec fn must_abort_spec(votes: Seq<Option<bool>>) -> bool {
    exists|i: int| 0 <= i < votes.len() as int && votes[i] == Some(false)
}

// r[impl two-phase.must-abort]
/// Returns true if any participant voted no.
pub fn must_abort(votes: &Vec<Option<bool>>) -> (result: bool)
    ensures result == must_abort_spec(votes@)
{
    let mut i: usize = 0;
    while i < votes.len()
        invariant
            i <= votes.len(),
            forall|j: int| 0 <= j < i as int ==> votes@[j] != Some(false),
        decreases votes.len() - i,
    {
        if votes[i] == Some(false) {
            return true;
        }
        i += 1;
    }
    false
}

// --- decide ---

// r[depends two-phase.decide]
/// Spec: decision is 1 (commit), 2 (abort), or 0 (pending).
pub open spec fn decide_spec(votes: Seq<Option<bool>>) -> int {
    if can_commit_spec(votes) {
        1int
    } else if must_abort_spec(votes) {
        2int
    } else {
        0int
    }
}

// r[impl two-phase.decide]
/// Returns the coordinator's decision: 0 = pending, 1 = commit, 2 = abort.
pub fn decide(votes: &Vec<Option<bool>>) -> (result: u8)
    ensures result as int == decide_spec(votes@)
{
    if can_commit(votes) {
        1
    } else if must_abort(votes) {
        2
    } else {
        0
    }
}

// --- Safety proofs ---

// r[verify two-phase.can-commit]
/// Commit requires unanimity: if we can commit, every vote is Some(true).
proof fn commit_requires_unanimity(votes: Seq<Option<bool>>)
    requires can_commit_spec(votes)
    ensures forall|i: int| 0 <= i < votes.len() as int ==> votes[i] == Some(true)
{}

// r[verify two-phase.must-abort]
/// Abort is always safe: if any vote is no, we cannot commit.
proof fn abort_prevents_commit(votes: Seq<Option<bool>>)
    requires must_abort_spec(votes)
    ensures !can_commit_spec(votes)
{
    // There exists i where votes[i] == Some(false).
    // But can_commit requires forall i, votes[i] == Some(true).
    // Some(false) != Some(true), contradiction.
}

// r[verify two-phase.decide]
/// Decision uniqueness: can_commit and must_abort are mutually exclusive.
proof fn decision_uniqueness(votes: Seq<Option<bool>>)
    ensures !(can_commit_spec(votes) && must_abort_spec(votes))
{
    if can_commit_spec(votes) && must_abort_spec(votes) {
        // can_commit: forall i, votes[i] == Some(true)
        // must_abort: exists i, votes[i] == Some(false)
        // Contradiction at the witness index.
    }
}

// r[verify two-phase.decide]
/// The decision is always one of {0, 1, 2}.
proof fn decide_in_range(votes: Seq<Option<bool>>)
    ensures
        decide_spec(votes) == 0
        || decide_spec(votes) == 1
        || decide_spec(votes) == 2
{}

// r[verify two-phase.can-commit]
/// An empty vote set cannot commit.
proof fn empty_cannot_commit()
    ensures !can_commit_spec(Seq::<Option<bool>>::empty())
{}

// r[verify two-phase.decide]
/// A single yes vote means commit for a 1-participant transaction.
proof fn single_yes_commits()
    ensures can_commit_spec(seq![Some(true)])
{}

// r[verify two-phase.decide]
/// All None means pending (not commit, not abort).
proof fn all_none_is_pending(n: nat)
    requires n > 0
    ensures
        decide_spec(Seq::new(n, |_i: int| None::<bool>)) == 0
{
    let votes = Seq::new(n, |_i: int| None::<bool>);
    // None != Some(true), so can_commit fails at index 0
    assert(votes[0int] == None::<bool>);
    assert(votes[0int] != Some(true));
    // None != Some(false), so must_abort is false (no witness)
    assert forall|i: int| 0 <= i < votes.len() as int implies votes[i] != Some(false)
    by {}
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify two-phase.can-commit]
    #[test]
    fn test_can_commit() {
        assert!(can_commit(&vec![Some(true), Some(true), Some(true)]));
        assert!(!can_commit(&vec![Some(true), Some(false), Some(true)]));
        assert!(!can_commit(&vec![Some(true), None, Some(true)]));
        assert!(!can_commit(&vec![]));
    }

    // r[verify two-phase.must-abort]
    #[test]
    fn test_must_abort() {
        assert!(must_abort(&vec![Some(true), Some(false)]));
        assert!(!must_abort(&vec![Some(true), Some(true)]));
        assert!(!must_abort(&vec![Some(true), None]));
        assert!(!must_abort(&vec![]));
    }

    // r[verify two-phase.decide]
    #[test]
    fn test_decide() {
        assert_eq!(decide(&vec![Some(true), Some(true)]), 1); // commit
        assert_eq!(decide(&vec![Some(true), Some(false)]), 2); // abort
        assert_eq!(decide(&vec![Some(true), None]), 0); // pending
        assert_eq!(decide(&vec![]), 0); // empty = pending
    }

    proptest! {
        // r[verify two-phase.decide]
        #[test]
        fn prop_decision_mutual_exclusion(
            votes in proptest::collection::vec(
                proptest::option::of(any::<bool>()), 1..=10
            ),
        ) {
            let c = can_commit(&votes);
            let a = must_abort(&votes);
            prop_assert!(!(c && a));
        }

        // r[verify two-phase.can-commit]
        #[test]
        fn prop_commit_implies_all_yes(
            votes in proptest::collection::vec(
                proptest::option::of(any::<bool>()), 1..=10
            ),
        ) {
            if can_commit(&votes) {
                for v in &votes {
                    prop_assert_eq!(*v, Some(true));
                }
            }
        }

        // r[verify two-phase.must-abort]
        #[test]
        fn prop_abort_implies_some_no(
            votes in proptest::collection::vec(
                proptest::option::of(any::<bool>()), 1..=10
            ),
        ) {
            if must_abort(&votes) {
                prop_assert!(votes.contains(&Some(false)));
            }
        }

        // r[verify two-phase.decide]
        #[test]
        fn prop_decide_range(
            votes in proptest::collection::vec(
                proptest::option::of(any::<bool>()), 0..=10
            ),
        ) {
            let d = decide(&votes);
            prop_assert!(d <= 2);
        }

        // r[verify two-phase.can-commit]
        #[test]
        fn prop_all_yes_commits(n in 1usize..=20) {
            let votes: Vec<Option<bool>> = vec![Some(true); n];
            prop_assert!(can_commit(&votes));
            prop_assert_eq!(decide(&votes), 1);
        }
    }
}

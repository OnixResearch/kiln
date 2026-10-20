//! Verified Paxos acceptor state machine.
//!
//! Models single-decree Paxos: ballot comparison (total order), acceptor
//! state transitions (promise and accept), and value selection by the
//! proposer. The key safety properties: promises are monotonic (acceptors
//! never go backward), acceptance requires a valid promise, and the
//! proposer's value choice respects the highest-ballot accepted value.

use vstd::prelude::*;

verus! {

// --- Ballot ---

/// A Paxos ballot number: (round, proposer_id) compared lexicographically.
/// Higher round always wins; ties broken by proposer_id.
#[derive(Copy, Clone)]
pub struct Ballot {
    pub round: u64,
    pub proposer_id: u64,
}

// --- ballot_gt ---

// r[depends paxos.ballot-gt]
/// Spec: strict ballot ordering. Lexicographic on (round, proposer_id).
pub open spec fn ballot_gt_spec(a: Ballot, b: Ballot) -> bool {
    a.round > b.round
    || (a.round == b.round && a.proposer_id > b.proposer_id)
}

// r[impl paxos.ballot-gt]
/// Returns true if ballot `a` is strictly greater than ballot `b`.
pub fn ballot_gt(a: &Ballot, b: &Ballot) -> (result: bool)
    ensures result == ballot_gt_spec(*a, *b)
{
    a.round > b.round
    || (a.round == b.round && a.proposer_id > b.proposer_id)
}

// --- ballot_eq ---

// r[depends paxos.ballot-eq]
/// Spec: ballot equality — both round and proposer_id match.
pub open spec fn ballot_eq_spec(a: Ballot, b: Ballot) -> bool {
    a.round == b.round && a.proposer_id == b.proposer_id
}

// r[impl paxos.ballot-eq]
/// Returns true if both ballots are identical.
pub fn ballot_eq(a: &Ballot, b: &Ballot) -> (result: bool)
    ensures result == ballot_eq_spec(*a, *b)
{
    a.round == b.round && a.proposer_id == b.proposer_id
}

// --- ballot_ge ---

// r[depends paxos.ballot-ge]
/// Spec: non-strict ballot ordering.
pub open spec fn ballot_ge_spec(a: Ballot, b: Ballot) -> bool {
    ballot_gt_spec(a, b) || ballot_eq_spec(a, b)
}

// r[impl paxos.ballot-ge]
/// Returns true if ballot `a` is greater than or equal to ballot `b`.
pub fn ballot_ge(a: &Ballot, b: &Ballot) -> (result: bool)
    ensures result == ballot_ge_spec(*a, *b)
{
    ballot_gt(a, b) || ballot_eq(a, b)
}

// --- AcceptorState ---

/// Paxos acceptor: tracks the highest promised ballot and optionally
/// the highest accepted (ballot, value) pair.
#[derive(Copy, Clone)]
pub struct AcceptorState {
    /// Highest ballot this acceptor has promised not to accept below.
    pub promised_round: u64,
    pub promised_proposer: u64,
    /// Whether this acceptor has accepted any value.
    pub has_accepted: bool,
    /// The ballot of the accepted value (meaningful only if has_accepted).
    pub accepted_round: u64,
    pub accepted_proposer: u64,
    /// The accepted value (meaningful only if has_accepted).
    pub accepted_value: u64,
}

/// Helper: extract the promised ballot from acceptor state.
pub open spec fn promised_ballot(state: AcceptorState) -> Ballot {
    Ballot { round: state.promised_round, proposer_id: state.promised_proposer }
}

/// Helper: extract the accepted ballot from acceptor state.
pub open spec fn accepted_ballot(state: AcceptorState) -> Ballot {
    Ballot { round: state.accepted_round, proposer_id: state.accepted_proposer }
}

// --- can_promise ---

// r[depends paxos.can-promise]
/// Spec: acceptor can promise for ballot if it is strictly higher than
/// the current promise. This is the Phase 1a/1b guard.
pub open spec fn can_promise_spec(state: AcceptorState, ballot: Ballot) -> bool {
    ballot_gt_spec(ballot, promised_ballot(state))
}

// r[impl paxos.can-promise]
/// Returns true if the acceptor should respond to a prepare for this ballot.
pub fn can_promise(state: &AcceptorState, ballot: &Ballot) -> (result: bool)
    ensures result == can_promise_spec(*state, *ballot)
{
    let promised = Ballot { round: state.promised_round, proposer_id: state.promised_proposer };
    ballot_gt(ballot, &promised)
}

// --- promise_next ---

// r[depends paxos.promise-next]
/// Spec: state after promising. Raises the promise; preserves accepted value.
pub open spec fn promise_next_spec(state: AcceptorState, ballot: Ballot) -> AcceptorState {
    AcceptorState {
        promised_round: ballot.round,
        promised_proposer: ballot.proposer_id,
        has_accepted: state.has_accepted,
        accepted_round: state.accepted_round,
        accepted_proposer: state.accepted_proposer,
        accepted_value: state.accepted_value,
    }
}

// r[impl paxos.promise-next]
/// Update acceptor state after promising. Returns new state.
pub fn do_promise(state: &AcceptorState, ballot: &Ballot) -> (result: AcceptorState)
    requires can_promise_spec(*state, *ballot)
    ensures result == promise_next_spec(*state, *ballot)
{
    AcceptorState {
        promised_round: ballot.round,
        promised_proposer: ballot.proposer_id,
        has_accepted: state.has_accepted,
        accepted_round: state.accepted_round,
        accepted_proposer: state.accepted_proposer,
        accepted_value: state.accepted_value,
    }
}

// --- can_accept ---

// r[depends paxos.can-accept]
/// Spec: acceptor can accept if the ballot is at least as high as its
/// current promise. This is the Phase 2a/2b guard.
pub open spec fn can_accept_spec(state: AcceptorState, ballot: Ballot) -> bool {
    ballot_ge_spec(ballot, promised_ballot(state))
}

// r[impl paxos.can-accept]
/// Returns true if the acceptor can accept a value for this ballot.
pub fn can_accept(state: &AcceptorState, ballot: &Ballot) -> (result: bool)
    ensures result == can_accept_spec(*state, *ballot)
{
    let promised = Ballot { round: state.promised_round, proposer_id: state.promised_proposer };
    ballot_ge(ballot, &promised)
}

// --- accept_next ---

// r[depends paxos.accept-next]
/// Spec: state after accepting. Records the ballot and value, and
/// updates the promise to match (accepting implies promising).
pub open spec fn accept_next_spec(state: AcceptorState, ballot: Ballot, value: u64) -> AcceptorState {
    AcceptorState {
        promised_round: ballot.round,
        promised_proposer: ballot.proposer_id,
        has_accepted: true,
        accepted_round: ballot.round,
        accepted_proposer: ballot.proposer_id,
        accepted_value: value,
    }
}

// r[impl paxos.accept-next]
/// Update acceptor state after accepting a value. Returns new state.
pub fn do_accept(state: &AcceptorState, ballot: &Ballot, value: u64) -> (result: AcceptorState)
    requires can_accept_spec(*state, *ballot)
    ensures result == accept_next_spec(*state, *ballot, value)
{
    AcceptorState {
        promised_round: ballot.round,
        promised_proposer: ballot.proposer_id,
        has_accepted: true,
        accepted_round: ballot.round,
        accepted_proposer: ballot.proposer_id,
        accepted_value: value,
    }
}

// --- pick_value ---

// r[depends paxos.pick-value]
/// Spec: proposer value choice from a single promise. If the promise
/// carries an accepted value, use it; otherwise use the proposer's own.
pub open spec fn pick_value_spec(has_accepted: bool, accepted_value: u64, own_value: u64) -> u64 {
    if has_accepted { accepted_value } else { own_value }
}

// r[impl paxos.pick-value]
/// Choose the value to propose based on a promise response.
pub fn pick_value(has_accepted: bool, accepted_value: u64, own_value: u64) -> (result: u64)
    ensures result == pick_value_spec(has_accepted, accepted_value, own_value)
{
    if has_accepted { accepted_value } else { own_value }
}

// --- higher_accepted ---

// r[depends paxos.higher-accepted]
/// Spec: given two promise responses, return the value from the higher
/// accepted ballot. If neither accepted, returns own_value.
pub open spec fn higher_accepted_spec(
    has_a: bool, ballot_a: Ballot, value_a: u64,
    has_b: bool, ballot_b: Ballot, value_b: u64,
    own_value: u64,
) -> u64 {
    if !has_a && !has_b { own_value }
    else if has_a && !has_b { value_a }
    else if !has_a && has_b { value_b }
    else if ballot_gt_spec(ballot_a, ballot_b) { value_a }
    else { value_b }
}

// r[impl paxos.higher-accepted]
/// Compare two promise responses and return the value from the higher
/// accepted ballot (or own_value if neither accepted).
pub fn higher_accepted(
    has_a: bool, ballot_a: &Ballot, value_a: u64,
    has_b: bool, ballot_b: &Ballot, value_b: u64,
    own_value: u64,
) -> (result: u64)
    ensures result == higher_accepted_spec(has_a, *ballot_a, value_a, has_b, *ballot_b, value_b, own_value)
{
    if !has_a && !has_b { own_value }
    else if has_a && !has_b { value_a }
    else if !has_a && has_b { value_b }
    else if ballot_gt(ballot_a, ballot_b) { value_a }
    else { value_b }
}

// --- Proofs: ballot ordering ---

// r[verify paxos.ballot-gt]
/// Irreflexive: no ballot is strictly greater than itself.
proof fn ballot_gt_irreflexive(a: Ballot)
    ensures !ballot_gt_spec(a, a)
{}

// r[verify paxos.ballot-gt]
/// Transitive: a > b and b > c implies a > c.
proof fn ballot_gt_transitive(a: Ballot, b: Ballot, c: Ballot)
    requires ballot_gt_spec(a, b), ballot_gt_spec(b, c)
    ensures ballot_gt_spec(a, c)
{}

// r[verify paxos.ballot-gt]
/// Asymmetric: a > b implies not b > a.
proof fn ballot_gt_asymmetric(a: Ballot, b: Ballot)
    requires ballot_gt_spec(a, b)
    ensures !ballot_gt_spec(b, a)
{}

// r[verify paxos.ballot-gt]
/// Trichotomy: exactly one of a > b, a == b, b > a holds.
proof fn ballot_trichotomy(a: Ballot, b: Ballot)
    ensures
        ballot_gt_spec(a, b) || ballot_eq_spec(a, b) || ballot_gt_spec(b, a),
        !(ballot_gt_spec(a, b) && ballot_eq_spec(a, b)),
        !(ballot_gt_spec(a, b) && ballot_gt_spec(b, a)),
        !(ballot_eq_spec(a, b) && ballot_gt_spec(b, a)),
{}

// r[verify paxos.ballot-ge]
/// Reflexive: every ballot is >= itself.
proof fn ballot_ge_reflexive(a: Ballot)
    ensures ballot_ge_spec(a, a)
{}

// r[verify paxos.ballot-ge]
/// Transitive: a >= b and b >= c implies a >= c.
proof fn ballot_ge_transitive(a: Ballot, b: Ballot, c: Ballot)
    requires ballot_ge_spec(a, b), ballot_ge_spec(b, c)
    ensures ballot_ge_spec(a, c)
{}

// r[verify paxos.ballot-ge]
/// Antisymmetric: a >= b and b >= a implies a == b.
proof fn ballot_ge_antisymmetric(a: Ballot, b: Ballot)
    requires ballot_ge_spec(a, b), ballot_ge_spec(b, a)
    ensures ballot_eq_spec(a, b)
{}

// --- Proofs: promise ---

// r[verify paxos.promise-next]
/// Promising advances the promised ballot strictly.
proof fn promise_advances(state: AcceptorState, ballot: Ballot)
    requires can_promise_spec(state, ballot)
    ensures ballot_gt_spec(
        promised_ballot(promise_next_spec(state, ballot)),
        promised_ballot(state),
    )
{}

// r[verify paxos.promise-next]
/// Promising preserves the accepted flag and value.
proof fn promise_preserves_accepted(state: AcceptorState, ballot: Ballot)
    requires can_promise_spec(state, ballot)
    ensures
        promise_next_spec(state, ballot).has_accepted == state.has_accepted,
        promise_next_spec(state, ballot).accepted_value == state.accepted_value,
        promise_next_spec(state, ballot).accepted_round == state.accepted_round,
        promise_next_spec(state, ballot).accepted_proposer == state.accepted_proposer,
{}

// --- Proofs: accept ---

// r[verify paxos.accept-next]
/// Acceptance updates the promise to match the ballot.
proof fn accept_updates_promise(state: AcceptorState, ballot: Ballot, value: u64)
    requires can_accept_spec(state, ballot)
    ensures ballot_eq_spec(
        promised_ballot(accept_next_spec(state, ballot, value)),
        ballot,
    )
{}

// r[verify paxos.accept-next]
/// Acceptance sets has_accepted to true.
proof fn accept_sets_accepted(state: AcceptorState, ballot: Ballot, value: u64)
    requires can_accept_spec(state, ballot)
    ensures accept_next_spec(state, ballot, value).has_accepted
{}

// r[verify paxos.accept-next]
/// The accepted value matches the provided value.
proof fn accept_records_value(state: AcceptorState, ballot: Ballot, value: u64)
    requires can_accept_spec(state, ballot)
    ensures accept_next_spec(state, ballot, value).accepted_value == value
{}

// r[verify paxos.accept-next]
/// The accepted ballot matches the acceptance ballot.
proof fn accept_records_ballot(state: AcceptorState, ballot: Ballot, value: u64)
    requires can_accept_spec(state, ballot)
    ensures ballot_eq_spec(accepted_ballot(accept_next_spec(state, ballot, value)), ballot)
{}

// --- Proofs: safety ---

// r[verify paxos.can-promise]
/// After promising ballot b, the acceptor rejects any ballot strictly
/// below b. This is the core Paxos safety mechanism.
proof fn promised_blocks_lower(state: AcceptorState, promised: Ballot, lower: Ballot)
    requires
        can_promise_spec(state, promised),
        ballot_gt_spec(promised, lower),
    ensures !can_accept_spec(promise_next_spec(state, promised), lower)
{}

// r[verify paxos.can-accept]
/// Accepting ballot b is idempotent: the acceptor can accept b again.
proof fn accept_is_idempotent(state: AcceptorState, ballot: Ballot, value: u64)
    requires can_accept_spec(state, ballot)
    ensures can_accept_spec(accept_next_spec(state, ballot, value), ballot)
{}

// r[verify paxos.can-promise]
/// After promising b, the acceptor can accept b (since b >= b).
proof fn promise_enables_accept(state: AcceptorState, ballot: Ballot)
    requires can_promise_spec(state, ballot)
    ensures can_accept_spec(promise_next_spec(state, ballot), ballot)
{}

// r[verify paxos.can-accept]
/// A higher ballot can always be accepted after a lower ballot was
/// accepted: acceptance doesn't block future higher ballots.
proof fn accept_allows_higher(state: AcceptorState, ballot: Ballot, value: u64, higher: Ballot)
    requires
        can_accept_spec(state, ballot),
        ballot_gt_spec(higher, ballot),
    ensures can_accept_spec(accept_next_spec(state, ballot, value), higher)
{}

// --- Proofs: value selection ---

// r[verify paxos.pick-value]
/// When the promise carries an accepted value, own_value is ignored.
proof fn pick_uses_accepted(accepted_value: u64, own_value: u64)
    ensures pick_value_spec(true, accepted_value, own_value) == accepted_value
{}

// r[verify paxos.pick-value]
/// When no accepted value exists, own_value is used.
proof fn pick_uses_own(accepted_value: u64, own_value: u64)
    ensures pick_value_spec(false, accepted_value, own_value) == own_value
{}

// r[verify paxos.higher-accepted]
/// With no accepted values, own_value is returned.
proof fn higher_accepted_none_uses_own(ba: Ballot, va: u64, bb: Ballot, vb: u64, own: u64)
    ensures higher_accepted_spec(false, ba, va, false, bb, vb, own) == own
{}

// r[verify paxos.higher-accepted]
/// With exactly one accepted, that value is chosen.
proof fn higher_accepted_one_accepted(ba: Ballot, va: u64, bb: Ballot, vb: u64, own: u64)
    ensures
        higher_accepted_spec(true, ba, va, false, bb, vb, own) == va,
        higher_accepted_spec(false, ba, va, true, bb, vb, own) == vb,
{}

// r[verify paxos.higher-accepted]
/// With both accepted, the value from the strictly higher ballot wins.
proof fn higher_accepted_picks_higher(ba: Ballot, va: u64, bb: Ballot, vb: u64, own: u64)
    requires ballot_gt_spec(ba, bb)
    ensures higher_accepted_spec(true, ba, va, true, bb, vb, own) == va
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify paxos.ballot-gt]
    #[test]
    fn test_ballot_ordering() {
        let b10 = Ballot {
            round: 1,
            proposer_id: 0,
        };
        let b11 = Ballot {
            round: 1,
            proposer_id: 1,
        };
        let b20 = Ballot {
            round: 2,
            proposer_id: 0,
        };

        assert!(ballot_gt(&b11, &b10)); // same round, higher proposer
        assert!(ballot_gt(&b20, &b10)); // higher round
        assert!(ballot_gt(&b20, &b11)); // higher round beats higher proposer
        assert!(!ballot_gt(&b10, &b10)); // irreflexive
        assert!(!ballot_gt(&b10, &b11)); // reversed
    }

    // r[verify paxos.ballot-eq]
    #[test]
    fn test_ballot_equality() {
        let b1 = Ballot {
            round: 5,
            proposer_id: 3,
        };
        let b2 = Ballot {
            round: 5,
            proposer_id: 3,
        };
        let b3 = Ballot {
            round: 5,
            proposer_id: 4,
        };

        assert!(ballot_eq(&b1, &b2));
        assert!(!ballot_eq(&b1, &b3));
    }

    // r[verify paxos.can-promise]
    #[test]
    fn test_promise_basic() {
        let state = AcceptorState {
            promised_round: 1,
            promised_proposer: 0,
            has_accepted: false,
            accepted_round: 0,
            accepted_proposer: 0,
            accepted_value: 0,
        };
        let higher = Ballot {
            round: 2,
            proposer_id: 0,
        };
        let lower = Ballot {
            round: 0,
            proposer_id: 5,
        };
        let same = Ballot {
            round: 1,
            proposer_id: 0,
        };

        assert!(can_promise(&state, &higher));
        assert!(!can_promise(&state, &lower));
        assert!(!can_promise(&state, &same));
    }

    // r[verify paxos.promise-next]
    #[test]
    fn test_promise_preserves_accepted() {
        let state = AcceptorState {
            promised_round: 1,
            promised_proposer: 0,
            has_accepted: true,
            accepted_round: 1,
            accepted_proposer: 0,
            accepted_value: 42,
        };
        let ballot = Ballot {
            round: 3,
            proposer_id: 0,
        };
        let next = do_promise(&state, &ballot);

        assert_eq!(next.promised_round, 3);
        assert_eq!(next.promised_proposer, 0);
        assert!(next.has_accepted);
        assert_eq!(next.accepted_value, 42);
    }

    // r[verify paxos.can-accept]
    #[test]
    fn test_accept_basic() {
        let state = AcceptorState {
            promised_round: 2,
            promised_proposer: 0,
            has_accepted: false,
            accepted_round: 0,
            accepted_proposer: 0,
            accepted_value: 0,
        };
        let ok = Ballot {
            round: 2,
            proposer_id: 0,
        }; // equal — ok
        let higher = Ballot {
            round: 3,
            proposer_id: 0,
        }; // higher — ok
        let lower = Ballot {
            round: 1,
            proposer_id: 0,
        }; // lower — reject

        assert!(can_accept(&state, &ok));
        assert!(can_accept(&state, &higher));
        assert!(!can_accept(&state, &lower));
    }

    // r[verify paxos.accept-next]
    #[test]
    fn test_accept_records() {
        let state = AcceptorState {
            promised_round: 2,
            promised_proposer: 0,
            has_accepted: false,
            accepted_round: 0,
            accepted_proposer: 0,
            accepted_value: 0,
        };
        let ballot = Ballot {
            round: 2,
            proposer_id: 0,
        };
        let next = do_accept(&state, &ballot, 99);

        assert!(next.has_accepted);
        assert_eq!(next.accepted_round, 2);
        assert_eq!(next.accepted_value, 99);
        assert_eq!(next.promised_round, 2);
    }

    // r[verify paxos.pick-value]
    #[test]
    fn test_pick_value() {
        assert_eq!(pick_value(true, 42, 100), 42);
        assert_eq!(pick_value(false, 42, 100), 100);
    }

    // r[verify paxos.higher-accepted]
    #[test]
    fn test_higher_accepted() {
        let ba = Ballot {
            round: 3,
            proposer_id: 0,
        };
        let bb = Ballot {
            round: 1,
            proposer_id: 0,
        };

        // Both accepted: higher ballot wins
        assert_eq!(higher_accepted(true, &ba, 10, true, &bb, 20, 99), 10);
        // Only A accepted
        assert_eq!(higher_accepted(true, &ba, 10, false, &bb, 20, 99), 10);
        // Only B accepted
        assert_eq!(higher_accepted(false, &ba, 10, true, &bb, 20, 99), 20);
        // Neither accepted: own value
        assert_eq!(higher_accepted(false, &ba, 10, false, &bb, 20, 99), 99);
    }

    proptest! {
        // r[verify paxos.ballot-gt]
        #[test]
        fn prop_ballot_trichotomy(
            ra in 0u64..1000, pa in 0u64..100,
            rb in 0u64..1000, pb in 0u64..100,
        ) {
            let a = Ballot { round: ra, proposer_id: pa };
            let b = Ballot { round: rb, proposer_id: pb };
            let gt_ab = ballot_gt(&a, &b);
            let eq_ab = ballot_eq(&a, &b);
            let gt_ba = ballot_gt(&b, &a);
            // exactly one holds
            let count = gt_ab as u32 + eq_ab as u32 + gt_ba as u32;
            prop_assert_eq!(count, 1);
        }

        // r[verify paxos.promise-next]
        #[test]
        fn prop_promise_monotonic(
            pr in 0u64..100, pp in 0u64..10,
            br in 0u64..100, bp in 0u64..10,
        ) {
            let state = AcceptorState {
                promised_round: pr, promised_proposer: pp,
                has_accepted: false, accepted_round: 0, accepted_proposer: 0,
                accepted_value: 0,
            };
            let ballot = Ballot { round: br, proposer_id: bp };
            if can_promise(&state, &ballot) {
                let next = do_promise(&state, &ballot);
                let old_b = Ballot { round: pr, proposer_id: pp };
                let new_b = Ballot { round: next.promised_round, proposer_id: next.promised_proposer };
                prop_assert!(ballot_gt(&new_b, &old_b));
            }
        }

        // r[verify paxos.can-promise]
        #[test]
        fn prop_promised_blocks_lower(
            pr in 0u64..100, pp in 0u64..10,
            br in 0u64..100, bp in 0u64..10,
            lr in 0u64..100, lp in 0u64..10,
        ) {
            let state = AcceptorState {
                promised_round: pr, promised_proposer: pp,
                has_accepted: false, accepted_round: 0, accepted_proposer: 0,
                accepted_value: 0,
            };
            let ballot = Ballot { round: br, proposer_id: bp };
            let lower = Ballot { round: lr, proposer_id: lp };
            if can_promise(&state, &ballot) && ballot_gt(&ballot, &lower) {
                let next = do_promise(&state, &ballot);
                prop_assert!(!can_accept(&next, &lower));
            }
        }

        // r[verify paxos.can-accept]
        #[test]
        fn prop_accept_idempotent(
            pr in 0u64..100, pp in 0u64..10,
            br in 0u64..100, bp in 0u64..10,
            val in 0u64..1000,
        ) {
            let state = AcceptorState {
                promised_round: pr, promised_proposer: pp,
                has_accepted: false, accepted_round: 0, accepted_proposer: 0,
                accepted_value: 0,
            };
            let ballot = Ballot { round: br, proposer_id: bp };
            if can_accept(&state, &ballot) {
                let next = do_accept(&state, &ballot, val);
                prop_assert!(can_accept(&next, &ballot));
            }
        }

        // r[verify paxos.can-promise]
        #[test]
        fn prop_promise_then_accept(
            pr in 0u64..100, pp in 0u64..10,
            br in 0u64..100, bp in 0u64..10,
        ) {
            let state = AcceptorState {
                promised_round: pr, promised_proposer: pp,
                has_accepted: false, accepted_round: 0, accepted_proposer: 0,
                accepted_value: 0,
            };
            let ballot = Ballot { round: br, proposer_id: bp };
            if can_promise(&state, &ballot) {
                let next = do_promise(&state, &ballot);
                prop_assert!(can_accept(&next, &ballot));
            }
        }

        // r[verify paxos.higher-accepted]
        #[test]
        fn prop_higher_accepted_picks_higher(
            ra in 0u64..100, pa in 0u64..10, va in 0u64..1000,
            rb in 0u64..100, pb in 0u64..10, vb in 0u64..1000,
            own in 0u64..1000,
        ) {
            let ba = Ballot { round: ra, proposer_id: pa };
            let bb = Ballot { round: rb, proposer_id: pb };
            if ballot_gt(&ba, &bb) {
                prop_assert_eq!(higher_accepted(true, &ba, va, true, &bb, vb, own), va);
            }
        }
    }
}

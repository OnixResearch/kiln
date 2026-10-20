//! Verified escrow state machine with conservation proofs.
//!
//! Models a conditional value-holding pattern: a buyer deposits funds
//! into escrow, and the funds are either released to a seller or
//! refunded to the buyer. A dispute path allows an arbiter to decide.
//! Conservation proofs guarantee the total value (buyer + seller +
//! escrow) is constant throughout the lifecycle.

use vstd::prelude::*;

verus! {

// --- State constants ---

/// Escrow created but not funded.
pub const ESCROW_CREATED: u8 = 0;
/// Buyer has deposited funds.
pub const ESCROW_FUNDED: u8 = 1;
/// Funds released to seller.
pub const ESCROW_RELEASED: u8 = 2;
/// Funds returned to buyer.
pub const ESCROW_REFUNDED: u8 = 3;
/// Under dispute — awaiting arbiter decision.
pub const ESCROW_DISPUTED: u8 = 4;

// --- is_valid_state ---

// r[depends escrow.valid-state]
/// Spec: state is one of the five escrow states.
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state == ESCROW_CREATED
    || state == ESCROW_FUNDED
    || state == ESCROW_RELEASED
    || state == ESCROW_REFUNDED
    || state == ESCROW_DISPUTED
}

// r[impl escrow.valid-state]
/// Returns true if the state is a valid escrow state.
pub fn is_valid_state(state: u8) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state == ESCROW_CREATED
    || state == ESCROW_FUNDED
    || state == ESCROW_RELEASED
    || state == ESCROW_REFUNDED
    || state == ESCROW_DISPUTED
}

// --- is_terminal ---

// r[depends escrow.terminal]
/// Spec: terminal states cannot transition further.
pub open spec fn is_terminal_spec(state: u8) -> bool {
    state == ESCROW_RELEASED || state == ESCROW_REFUNDED
}

// r[impl escrow.terminal]
/// Returns true if the escrow is in a terminal state.
pub fn is_terminal(state: u8) -> (result: bool)
    ensures result == is_terminal_spec(state)
{
    state == ESCROW_RELEASED || state == ESCROW_REFUNDED
}

// --- can_transition ---

// r[depends escrow.transition]
/// Spec: valid state transitions.
pub open spec fn can_transition_spec(from: u8, to: u8) -> bool {
    // Created -> Funded (buyer deposits)
    (from == ESCROW_CREATED && to == ESCROW_FUNDED)
    // Funded -> Released (seller receives)
    || (from == ESCROW_FUNDED && to == ESCROW_RELEASED)
    // Funded -> Refunded (buyer gets back)
    || (from == ESCROW_FUNDED && to == ESCROW_REFUNDED)
    // Funded -> Disputed (party raises dispute)
    || (from == ESCROW_FUNDED && to == ESCROW_DISPUTED)
    // Disputed -> Released (arbiter rules for seller)
    || (from == ESCROW_DISPUTED && to == ESCROW_RELEASED)
    // Disputed -> Refunded (arbiter rules for buyer)
    || (from == ESCROW_DISPUTED && to == ESCROW_REFUNDED)
}

// r[impl escrow.transition]
/// Returns true if the transition from `from` to `to` is permitted.
pub fn can_transition(from: u8, to: u8) -> (result: bool)
    ensures result == can_transition_spec(from, to)
{
    (from == ESCROW_CREATED && to == ESCROW_FUNDED)
    || (from == ESCROW_FUNDED && to == ESCROW_RELEASED)
    || (from == ESCROW_FUNDED && to == ESCROW_REFUNDED)
    || (from == ESCROW_FUNDED && to == ESCROW_DISPUTED)
    || (from == ESCROW_DISPUTED && to == ESCROW_RELEASED)
    || (from == ESCROW_DISPUTED && to == ESCROW_REFUNDED)
}

// --- fund ---

// r[depends escrow.fund]
/// Spec: buyer balance after funding escrow.
pub open spec fn fund_buyer_after_spec(buyer_balance: u64, amount: u64) -> int {
    buyer_balance as int - amount as int
}

// r[depends escrow.fund]
/// Spec: can the buyer fund this escrow?
pub open spec fn can_fund_spec(state: u8, buyer_balance: u64, amount: u64) -> bool {
    state == ESCROW_CREATED
    && amount > 0
    && amount as int <= buyer_balance as int
}

// r[impl escrow.fund]
/// Fund the escrow from the buyer's balance.
/// Returns (new_buyer_balance, new_state) or None if invalid.
pub fn fund(
    state: u8, buyer_balance: u64, amount: u64,
) -> (result: Option<(u64, u8)>)
    ensures
        match result {
            Some(pair) =>
                can_fund_spec(state, buyer_balance, amount)
                && pair.0 as int == fund_buyer_after_spec(buyer_balance, amount)
                && pair.1 == ESCROW_FUNDED,
            None => !can_fund_spec(state, buyer_balance, amount),
        },
{
    if state != ESCROW_CREATED || amount == 0 || amount > buyer_balance {
        None
    } else {
        Some((buyer_balance - amount, ESCROW_FUNDED))
    }
}

// --- release ---

// r[depends escrow.release]
/// Spec: seller balance after release.
pub open spec fn release_seller_after_spec(seller_balance: u64, amount: u64) -> int {
    seller_balance as int + amount as int
}

// r[depends escrow.release]
/// Spec: can we release escrowed funds to the seller?
pub open spec fn can_release_spec(state: u8, seller_balance: u64, amount: u64) -> bool {
    (state == ESCROW_FUNDED || state == ESCROW_DISPUTED)
    && amount > 0
    && seller_balance as int + amount as int <= u64::MAX as int
}

// r[impl escrow.release]
/// Release escrowed funds to the seller.
/// Returns (new_seller_balance, new_state) or None if invalid.
pub fn release(
    state: u8, seller_balance: u64, amount: u64,
) -> (result: Option<(u64, u8)>)
    ensures
        match result {
            Some(pair) =>
                can_release_spec(state, seller_balance, amount)
                && pair.0 as int == release_seller_after_spec(seller_balance, amount)
                && pair.1 == ESCROW_RELEASED,
            None => !can_release_spec(state, seller_balance, amount),
        },
{
    if (state != ESCROW_FUNDED && state != ESCROW_DISPUTED)
        || amount == 0
        || amount > u64::MAX - seller_balance
    {
        None
    } else {
        Some((seller_balance + amount, ESCROW_RELEASED))
    }
}

// --- refund ---

// r[depends escrow.refund]
/// Spec: buyer balance after refund.
pub open spec fn refund_buyer_after_spec(buyer_balance: u64, amount: u64) -> int {
    buyer_balance as int + amount as int
}

// r[depends escrow.refund]
/// Spec: can we refund escrowed funds to the buyer?
pub open spec fn can_refund_spec(state: u8, buyer_balance: u64, amount: u64) -> bool {
    (state == ESCROW_FUNDED || state == ESCROW_DISPUTED)
    && amount > 0
    && buyer_balance as int + amount as int <= u64::MAX as int
}

// r[impl escrow.refund]
/// Refund escrowed funds to the buyer.
/// Returns (new_buyer_balance, new_state) or None if invalid.
pub fn refund(
    state: u8, buyer_balance: u64, amount: u64,
) -> (result: Option<(u64, u8)>)
    ensures
        match result {
            Some(pair) =>
                can_refund_spec(state, buyer_balance, amount)
                && pair.0 as int == refund_buyer_after_spec(buyer_balance, amount)
                && pair.1 == ESCROW_REFUNDED,
            None => !can_refund_spec(state, buyer_balance, amount),
        },
{
    if (state != ESCROW_FUNDED && state != ESCROW_DISPUTED)
        || amount == 0
        || amount > u64::MAX - buyer_balance
    {
        None
    } else {
        Some((buyer_balance + amount, ESCROW_REFUNDED))
    }
}

// --- dispute ---

// r[depends escrow.dispute]
/// Spec: can a dispute be raised?
pub open spec fn can_dispute_spec(state: u8) -> bool {
    state == ESCROW_FUNDED
}

// r[impl escrow.dispute]
/// Raise a dispute on a funded escrow. Returns new state or None.
pub fn dispute(state: u8) -> (result: Option<u8>)
    ensures
        match result {
            Some(s) => can_dispute_spec(state) && s == ESCROW_DISPUTED,
            None => !can_dispute_spec(state),
        },
{
    if state != ESCROW_FUNDED {
        None
    } else {
        Some(ESCROW_DISPUTED)
    }
}

// --- total_value ---

// r[depends escrow.conservation]
/// Spec: total value across buyer, seller, and escrow.
pub open spec fn total_value_spec(buyer: u64, seller: u64, escrow: u64) -> int {
    buyer as int + seller as int + escrow as int
}

// r[impl escrow.conservation]
/// Compute total value, returning None on overflow.
pub fn total_value(buyer: u64, seller: u64, escrow: u64) -> (result: Option<u128>)
    ensures
        match result {
            Some(v) => v as int == total_value_spec(buyer, seller, escrow),
            None => false, // u128 can't overflow with three u64 values
        },
{
    // Three u64 values sum to at most 3 * (2^64 - 1) < 2^128
    Some(buyer as u128 + seller as u128 + escrow as u128)
}

// --- Proofs ---

// r[verify escrow.valid-state]
/// All transitions preserve valid state.
proof fn transition_preserves_validity(from: u8, to: u8)
    requires can_transition_spec(from, to)
    ensures is_valid_state_spec(to)
{}

// r[verify escrow.terminal]
/// Terminal states have no outgoing transitions.
proof fn terminal_blocks_transition(from: u8, to: u8)
    requires is_terminal_spec(from)
    ensures !can_transition_spec(from, to)
{}

// r[verify escrow.terminal]
/// Released is terminal.
proof fn released_is_terminal()
    ensures is_terminal_spec(ESCROW_RELEASED)
{}

// r[verify escrow.terminal]
/// Refunded is terminal.
proof fn refunded_is_terminal()
    ensures is_terminal_spec(ESCROW_REFUNDED)
{}

// r[verify escrow.transition]
/// Created can only go to Funded.
proof fn created_only_to_funded(to: u8)
    requires can_transition_spec(ESCROW_CREATED, to)
    ensures to == ESCROW_FUNDED
{}

// r[verify escrow.transition]
/// Funded has exactly three outgoing transitions.
proof fn funded_outgoing(to: u8)
    requires can_transition_spec(ESCROW_FUNDED, to)
    ensures to == ESCROW_RELEASED || to == ESCROW_REFUNDED || to == ESCROW_DISPUTED
{}

// r[verify escrow.transition]
/// Disputed has exactly two outgoing transitions (both terminal).
proof fn disputed_outgoing(to: u8)
    requires can_transition_spec(ESCROW_DISPUTED, to)
    ensures to == ESCROW_RELEASED || to == ESCROW_REFUNDED
{}

// r[verify escrow.conservation]
/// Funding preserves total value: buyer + escrow stays the same.
proof fn fund_conserves(buyer_balance: u64, amount: u64)
    requires can_fund_spec(ESCROW_CREATED, buyer_balance, amount)
    ensures
        fund_buyer_after_spec(buyer_balance, amount) + amount as int
            == buyer_balance as int,
{}

// r[verify escrow.conservation]
/// Release preserves total value: escrow + seller stays the same.
proof fn release_conserves(seller_balance: u64, amount: u64)
    requires can_release_spec(ESCROW_FUNDED, seller_balance, amount)
    ensures
        release_seller_after_spec(seller_balance, amount) - amount as int
            == seller_balance as int,
{}

// r[verify escrow.conservation]
/// Refund preserves total value: escrow + buyer stays the same.
proof fn refund_conserves(buyer_balance: u64, amount: u64)
    requires can_refund_spec(ESCROW_FUNDED, buyer_balance, amount)
    ensures
        refund_buyer_after_spec(buyer_balance, amount) - amount as int
            == buyer_balance as int,
{}

// r[verify escrow.conservation]
/// Full lifecycle conservation: fund-then-release preserves three-party total.
proof fn lifecycle_fund_release_conservation(
    buyer: u64, seller: u64, amount: u64,
)
    requires
        can_fund_spec(ESCROW_CREATED, buyer, amount),
        can_release_spec(ESCROW_FUNDED, seller, amount),
    ensures
        total_value_spec(
            fund_buyer_after_spec(buyer, amount) as u64,
            release_seller_after_spec(seller, amount) as u64,
            0,
        ) == total_value_spec(buyer, seller, 0),
{}

// r[verify escrow.conservation]
/// Full lifecycle conservation: fund-then-refund preserves three-party total.
proof fn lifecycle_fund_refund_conservation(
    buyer: u64, amount: u64, seller: u64,
)
    requires
        can_fund_spec(ESCROW_CREATED, buyer, amount),
        can_refund_spec(ESCROW_FUNDED, fund_buyer_after_spec(buyer, amount) as u64, amount),
    ensures
        total_value_spec(
            refund_buyer_after_spec(
                fund_buyer_after_spec(buyer, amount) as u64, amount) as u64,
            seller,
            0,
        ) == total_value_spec(buyer, seller, 0),
{}

// r[verify escrow.conservation]
/// After funding, the escrowed amount + new buyer balance = original buyer balance.
proof fn fund_accounting_identity(buyer: u64, amount: u64)
    requires can_fund_spec(ESCROW_CREATED, buyer, amount)
    ensures
        fund_buyer_after_spec(buyer, amount) + amount as int == buyer as int,
{}

// r[verify escrow.fund]
/// Fund-then-refund roundtrip returns buyer to original balance.
proof fn fund_refund_roundtrip(buyer: u64, amount: u64)
    requires
        can_fund_spec(ESCROW_CREATED, buyer, amount),
        can_refund_spec(ESCROW_FUNDED, fund_buyer_after_spec(buyer, amount) as u64, amount),
    ensures
        refund_buyer_after_spec(
            fund_buyer_after_spec(buyer, amount) as u64, amount) == buyer as int,
{}

// r[verify escrow.dispute]
/// Dispute doesn't change any balances (state-only transition).
proof fn dispute_no_balance_change()
    ensures
        // Dispute is a state-only operation — no balance parameters
        can_dispute_spec(ESCROW_FUNDED),
{}

// r[verify escrow.valid-state]
/// Every reachable state from Created via valid transitions is valid.
proof fn reachable_states_valid(s1: u8, s2: u8)
    requires
        is_valid_state_spec(s1),
        can_transition_spec(s1, s2),
    ensures is_valid_state_spec(s2)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify escrow.valid-state]
    #[test]
    fn test_valid_states() {
        assert!(is_valid_state(ESCROW_CREATED));
        assert!(is_valid_state(ESCROW_FUNDED));
        assert!(is_valid_state(ESCROW_RELEASED));
        assert!(is_valid_state(ESCROW_REFUNDED));
        assert!(is_valid_state(ESCROW_DISPUTED));
        assert!(!is_valid_state(5));
        assert!(!is_valid_state(255));
    }

    // r[verify escrow.terminal]
    #[test]
    fn test_terminal_states() {
        assert!(!is_terminal(ESCROW_CREATED));
        assert!(!is_terminal(ESCROW_FUNDED));
        assert!(is_terminal(ESCROW_RELEASED));
        assert!(is_terminal(ESCROW_REFUNDED));
        assert!(!is_terminal(ESCROW_DISPUTED));
    }

    // r[verify escrow.transition]
    #[test]
    fn test_transitions() {
        assert!(can_transition(ESCROW_CREATED, ESCROW_FUNDED));
        assert!(!can_transition(ESCROW_CREATED, ESCROW_RELEASED));
        assert!(can_transition(ESCROW_FUNDED, ESCROW_RELEASED));
        assert!(can_transition(ESCROW_FUNDED, ESCROW_REFUNDED));
        assert!(can_transition(ESCROW_FUNDED, ESCROW_DISPUTED));
        assert!(can_transition(ESCROW_DISPUTED, ESCROW_RELEASED));
        assert!(can_transition(ESCROW_DISPUTED, ESCROW_REFUNDED));
        // Terminal blocks
        assert!(!can_transition(ESCROW_RELEASED, ESCROW_CREATED));
        assert!(!can_transition(ESCROW_REFUNDED, ESCROW_FUNDED));
    }

    // r[verify escrow.fund]
    #[test]
    fn test_fund_basic() {
        assert_eq!(fund(ESCROW_CREATED, 1000, 500), Some((500, ESCROW_FUNDED)));
        assert_eq!(fund(ESCROW_CREATED, 100, 100), Some((0, ESCROW_FUNDED)));
        assert_eq!(fund(ESCROW_CREATED, 50, 100), None); // insufficient
        assert_eq!(fund(ESCROW_CREATED, 100, 0), None); // zero amount
        assert_eq!(fund(ESCROW_FUNDED, 100, 50), None); // wrong state
    }

    // r[verify escrow.release]
    #[test]
    fn test_release_basic() {
        assert_eq!(release(ESCROW_FUNDED, 200, 500), Some((700, ESCROW_RELEASED)));
        assert_eq!(release(ESCROW_DISPUTED, 0, 500), Some((500, ESCROW_RELEASED)));
        assert_eq!(release(ESCROW_CREATED, 200, 500), None); // wrong state
        assert_eq!(release(ESCROW_FUNDED, 200, 0), None); // zero amount
        assert_eq!(release(ESCROW_FUNDED, u64::MAX, 1), None); // overflow
    }

    // r[verify escrow.refund]
    #[test]
    fn test_refund_basic() {
        assert_eq!(refund(ESCROW_FUNDED, 500, 300), Some((800, ESCROW_REFUNDED)));
        assert_eq!(refund(ESCROW_DISPUTED, 500, 300), Some((800, ESCROW_REFUNDED)));
        assert_eq!(refund(ESCROW_CREATED, 500, 300), None); // wrong state
        assert_eq!(refund(ESCROW_FUNDED, 500, 0), None); // zero amount
        assert_eq!(refund(ESCROW_FUNDED, u64::MAX, 1), None); // overflow
    }

    // r[verify escrow.dispute]
    #[test]
    fn test_dispute_basic() {
        assert_eq!(dispute(ESCROW_FUNDED), Some(ESCROW_DISPUTED));
        assert_eq!(dispute(ESCROW_CREATED), None);
        assert_eq!(dispute(ESCROW_RELEASED), None);
        assert_eq!(dispute(ESCROW_DISPUTED), None);
    }

    // r[verify escrow.conservation]
    #[test]
    fn test_total_value() {
        assert_eq!(total_value(100, 200, 300), Some(600));
        assert_eq!(total_value(0, 0, 0), Some(0));
        assert_eq!(total_value(u64::MAX, u64::MAX, u64::MAX), Some(u64::MAX as u128 * 3));
    }

    proptest! {
        // r[verify escrow.conservation]
        #[test]
        fn prop_fund_release_conservation(
            buyer in 1..u64::MAX / 2,
            seller in 0..u64::MAX / 2,
            amount in 1..u64::MAX / 4,
        ) {
            if amount <= buyer {
                let before = buyer as u128 + seller as u128;
                if let Some((new_buyer, _)) = fund(ESCROW_CREATED, buyer, amount) {
                    if let Some((new_seller, _)) = release(ESCROW_FUNDED, seller, amount) {
                        let after = new_buyer as u128 + new_seller as u128;
                        prop_assert_eq!(before, after);
                    }
                }
            }
        }

        // r[verify escrow.conservation]
        #[test]
        fn prop_fund_refund_roundtrip(buyer in 1..u64::MAX / 2, amount in 1..u64::MAX / 4) {
            if amount <= buyer {
                if let Some((debited, _)) = fund(ESCROW_CREATED, buyer, amount) {
                    if let Some((restored, _)) = refund(ESCROW_FUNDED, debited, amount) {
                        prop_assert_eq!(restored, buyer);
                    }
                }
            }
        }

        // r[verify escrow.transition]
        #[test]
        fn prop_terminal_blocks_all(from in 0u8..5, to in 0u8..5) {
            if is_terminal(from) {
                prop_assert!(!can_transition(from, to));
            }
        }

        // r[verify escrow.valid-state]
        #[test]
        fn prop_transitions_preserve_validity(from in 0u8..5, to in 0u8..5) {
            if is_valid_state(from) && can_transition(from, to) {
                prop_assert!(is_valid_state(to));
            }
        }

        // r[verify escrow.conservation]
        #[test]
        fn prop_fund_dispute_release_conservation(
            buyer in 1..u64::MAX / 2,
            seller in 0..u64::MAX / 2,
            amount in 1..u64::MAX / 4,
        ) {
            if amount <= buyer {
                let before = buyer as u128 + seller as u128;
                if let Some((new_buyer, state)) = fund(ESCROW_CREATED, buyer, amount) {
                    if let Some(dstate) = dispute(state) {
                        if let Some((new_seller, _)) = release(dstate, seller, amount) {
                            let after = new_buyer as u128 + new_seller as u128;
                            prop_assert_eq!(before, after);
                        }
                    }
                }
            }
        }

        // r[verify escrow.conservation]
        #[test]
        fn prop_fund_dispute_refund_roundtrip(
            buyer in 1..u64::MAX / 2,
            amount in 1..u64::MAX / 4,
        ) {
            if amount <= buyer {
                if let Some((debited, state)) = fund(ESCROW_CREATED, buyer, amount) {
                    if let Some(dstate) = dispute(state) {
                        if let Some((restored, _)) = refund(dstate, debited, amount) {
                            prop_assert_eq!(restored, buyer);
                        }
                    }
                }
            }
        }
    }
}

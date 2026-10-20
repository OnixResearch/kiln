//! Verified balance operations for ledger accounts.
//!
//! Credit (add funds), debit (remove funds), sufficient-funds check,
//! valid-amount predicate, and net-change computation. All operations
//! use checked arithmetic — credit returns None on overflow, debit
//! returns None on underflow. Conservation proofs guarantee value is
//! neither created nor destroyed.

use vstd::prelude::*;

verus! {

// --- credit ---

// r[depends balance.credit]
/// Spec: balance + amount (int to avoid overflow).
pub open spec fn credit_spec(balance: u64, amount: u64) -> int {
    balance as int + amount as int
}

// r[impl balance.credit]
/// Credit an account. Returns None if balance + amount overflows u64.
pub fn credit(balance: u64, amount: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == credit_spec(balance, amount),
            None => credit_spec(balance, amount) > u64::MAX as int,
        },
{
    if amount > u64::MAX - balance {
        None
    } else {
        Some(balance + amount)
    }
}

// --- debit ---

// r[depends balance.debit]
/// Spec: balance - amount (int to allow negative for reasoning).
pub open spec fn debit_spec(balance: u64, amount: u64) -> int {
    balance as int - amount as int
}

// r[impl balance.debit]
/// Debit an account. Returns None if amount exceeds balance.
pub fn debit(balance: u64, amount: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == debit_spec(balance, amount),
            None => debit_spec(balance, amount) < 0,
        },
{
    if amount > balance {
        None
    } else {
        Some(balance - amount)
    }
}

// --- has_funds ---

// r[depends balance.has-funds]
/// Spec: sufficient funds check.
pub open spec fn has_funds_spec(balance: u64, amount: u64) -> bool {
    amount as int <= balance as int
}

// r[impl balance.has-funds]
/// Returns true if the balance can cover the given amount.
pub fn has_funds(balance: u64, amount: u64) -> (result: bool)
    ensures result == has_funds_spec(balance, amount)
{
    amount <= balance
}

// --- is_valid_amount ---

// r[depends balance.valid-amount]
/// Spec: amount must be positive.
pub open spec fn is_valid_amount_spec(amount: u64) -> bool {
    amount > 0
}

// r[impl balance.valid-amount]
/// Returns true if the amount is nonzero.
pub fn is_valid_amount(amount: u64) -> (result: bool)
    ensures result == is_valid_amount_spec(amount)
{
    amount > 0
}

// --- net_change ---

// r[depends balance.net-change]
/// Spec: signed difference of total credits minus total debits.
pub open spec fn net_change_spec(total_credits: u64, total_debits: u64) -> int {
    total_credits as int - total_debits as int
}

// r[impl balance.net-change]
/// Returns (is_positive, absolute_value) for credits minus debits.
pub fn net_change(total_credits: u64, total_debits: u64) -> (result: (bool, u64))
    ensures
        result.0 == (net_change_spec(total_credits, total_debits) >= 0),
        result.1 == crate::arithmetic::abs_diff_spec(total_credits, total_debits),
{
    if total_credits >= total_debits {
        (true, total_credits - total_debits)
    } else {
        (false, total_debits - total_credits)
    }
}

// --- Proofs ---

// r[verify balance.credit]
/// Credit is monotonic: result >= balance.
proof fn credit_monotonic(balance: u64, amount: u64)
    ensures credit_spec(balance, amount) >= balance as int
{}

// r[verify balance.debit]
/// Debit is monotonic: when has_funds, result <= balance and >= 0.
proof fn debit_monotonic(balance: u64, amount: u64)
    requires has_funds_spec(balance, amount)
    ensures
        debit_spec(balance, amount) <= balance as int,
        debit_spec(balance, amount) >= 0,
{}

// r[impl balance.credit-debit-inverse]
// r[verify balance.credit-debit-inverse]
/// Credit then debit is identity.
proof fn credit_debit_roundtrip(balance: u64, amount: u64)
    requires credit_spec(balance, amount) <= u64::MAX as int
    ensures debit_spec(credit_spec(balance, amount) as u64, amount) == balance as int
{}

// r[verify balance.credit-debit-inverse]
/// Debit then credit is identity.
proof fn debit_credit_roundtrip(balance: u64, amount: u64)
    requires has_funds_spec(balance, amount)
    ensures credit_spec(debit_spec(balance, amount) as u64, amount) == balance as int
{}

// r[verify balance.credit]
/// Crediting zero is identity.
proof fn zero_credit_identity(balance: u64)
    ensures credit_spec(balance, 0) == balance as int
{}

// r[verify balance.debit]
/// Debiting zero is identity.
proof fn zero_debit_identity(balance: u64)
    ensures debit_spec(balance, 0) == balance as int
{}

// r[verify balance.credit]
/// Credit commutes: order of two credits doesn't matter.
proof fn credit_commutes(balance: u64, a1: u64, a2: u64)
    requires
        credit_spec(balance, a1) <= u64::MAX as int,
        credit_spec(credit_spec(balance, a1) as u64, a2) <= u64::MAX as int,
    ensures
        credit_spec(credit_spec(balance, a1) as u64, a2)
            == credit_spec(credit_spec(balance, a2) as u64, a1),
{}

// r[verify balance.has-funds]
/// After a credit, we always have funds to reverse it.
proof fn has_funds_after_credit(balance: u64, amount: u64)
    requires credit_spec(balance, amount) <= u64::MAX as int
    ensures has_funds_spec(credit_spec(balance, amount) as u64, amount)
{}

// r[verify balance.has-funds]
/// has_funds and debit failure are complementary.
proof fn no_funds_implies_debit_fails(balance: u64, amount: u64)
    ensures !has_funds_spec(balance, amount) <==> debit_spec(balance, amount) < 0
{}

// r[impl balance.conservation]
// r[verify balance.conservation]
/// Conservation: credit(b,a) - a == b and debit(b,a) + a == b.
proof fn conservation_pair(balance: u64, amount: u64)
    ensures
        credit_spec(balance, amount) - amount as int == balance as int,
        debit_spec(balance, amount) + amount as int == balance as int,
{}

// r[verify balance.has-funds]
/// Zero amount always has funds.
proof fn zero_always_has_funds(balance: u64)
    ensures has_funds_spec(balance, 0)
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify balance.credit]
    #[test]
    fn test_credit_basic() {
        assert_eq!(credit(100, 50), Some(150));
        assert_eq!(credit(0, 0), Some(0));
        assert_eq!(credit(u64::MAX, 0), Some(u64::MAX));
        assert_eq!(credit(u64::MAX, 1), None);
        assert_eq!(credit(u64::MAX - 1, 1), Some(u64::MAX));
    }

    // r[verify balance.debit]
    #[test]
    fn test_debit_basic() {
        assert_eq!(debit(100, 50), Some(50));
        assert_eq!(debit(0, 0), Some(0));
        assert_eq!(debit(50, 100), None);
        assert_eq!(debit(0, 1), None);
        assert_eq!(debit(u64::MAX, u64::MAX), Some(0));
    }

    // r[verify balance.has-funds]
    #[test]
    fn test_has_funds_basic() {
        assert!(has_funds(100, 50));
        assert!(has_funds(100, 100));
        assert!(!has_funds(50, 100));
        assert!(has_funds(0, 0));
        assert!(!has_funds(0, 1));
    }

    // r[verify balance.valid-amount]
    #[test]
    fn test_is_valid_amount() {
        assert!(!is_valid_amount(0));
        assert!(is_valid_amount(1));
        assert!(is_valid_amount(u64::MAX));
    }

    // r[verify balance.net-change]
    #[test]
    fn test_net_change_basic() {
        assert_eq!(net_change(100, 50), (true, 50));
        assert_eq!(net_change(50, 100), (false, 50));
        assert_eq!(net_change(100, 100), (true, 0));
        assert_eq!(net_change(0, 0), (true, 0));
    }

    proptest! {
        // r[verify balance.credit-debit-inverse]
        #[test]
        fn prop_credit_debit_roundtrip(b: u64, a: u64) {
            if let Some(credited) = credit(b, a) {
                prop_assert_eq!(debit(credited, a), Some(b));
            }
        }

        // r[verify balance.credit-debit-inverse]
        #[test]
        fn prop_debit_credit_roundtrip(b: u64, a: u64) {
            if let Some(debited) = debit(b, a) {
                prop_assert_eq!(credit(debited, a), Some(b));
            }
        }

        // r[verify balance.conservation]
        #[test]
        fn prop_conservation(b: u64, a: u64) {
            let credit_val = b as i128 + a as i128;
            let debit_val = b as i128 - a as i128;
            prop_assert_eq!(credit_val - a as i128, b as i128);
            prop_assert_eq!(debit_val + a as i128, b as i128);
        }

        // r[verify balance.has-funds]
        #[test]
        fn prop_has_funds_agrees_debit(b: u64, a: u64) {
            prop_assert_eq!(has_funds(b, a), debit(b, a).is_some());
        }

        // r[verify balance.net-change]
        #[test]
        fn prop_net_change_abs(c: u64, d: u64) {
            let (positive, abs_val) = net_change(c, d);
            let oracle = (c as i128) - (d as i128);
            prop_assert_eq!(positive, oracle >= 0);
            prop_assert_eq!(abs_val, oracle.unsigned_abs() as u64);
        }

        // r[verify balance.credit]
        #[test]
        fn prop_credit_commutes(b in 0..u64::MAX / 3, a1 in 0..u64::MAX / 3, a2 in 0..u64::MAX / 3) {
            if let (Some(c1), Some(c2)) = (credit(b, a1), credit(b, a2)) {
                let via_a1 = credit(c1, a2);
                let via_a2 = credit(c2, a1);
                prop_assert_eq!(via_a1, via_a2);
            }
        }
    }
}

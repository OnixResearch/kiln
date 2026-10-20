//! Verified double-entry bookkeeping invariants.
//!
//! Models the fundamental accounting identity: total debits equal total
//! credits. Provides summation over `Seq<u64>`, balanced-entry verification,
//! trial balance, and merge properties. Inductive proofs guarantee sum
//! distributes over concatenation and that balanced entries compose.

#![allow(clippy::len_zero)]

use vstd::prelude::*;

verus! {

// --- sum ---

// r[depends double-entry.sum]
/// Spec: recursive sum from index `from`.
pub open spec fn sum_spec(amounts: Seq<u64>, from: int) -> int
    decreases amounts.len() - from
{
    if from >= amounts.len() {
        0
    } else {
        amounts[from] as int + sum_spec(amounts, from + 1)
    }
}

// r[depends double-entry.sum]
/// Spec: sum of all elements.
pub open spec fn sum_all_spec(amounts: Seq<u64>) -> int {
    sum_spec(amounts, 0)
}

// r[impl double-entry.sum]
/// Sum amounts with overflow detection. Returns None if sum exceeds u64::MAX.
pub fn sum_amounts(amounts: &[u64]) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == sum_all_spec(amounts@),
            None => sum_all_spec(amounts@) > u64::MAX as int,
        },
{
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < amounts.len()
        invariant
            0 <= i <= amounts@.len(),
            acc as int == sum_spec(amounts@, 0) - sum_spec(amounts@, i as int),
            acc as int <= u64::MAX as int,
        decreases amounts@.len() - i,
    {
        if amounts[i] > u64::MAX - acc {
            proof {
                // Unfold: sum_spec(amounts@, i) = amounts@[i] + sum_spec(amounts@, i+1)
                // sum_spec(amounts@, i+1) >= 0, so sum_spec(amounts@, i) >= amounts@[i]
                sum_nonneg(amounts@, (i + 1) as int);
                assert(sum_spec(amounts@, i as int) >= amounts@[i as int] as int);
            }
            return None;
        }
        acc = acc + amounts[i];
        i = i + 1;
    }
    Some(acc)
}

// --- is_balanced ---

// r[depends double-entry.balanced]
/// Spec: total debits equal total credits.
pub open spec fn is_balanced_spec(debits: Seq<u64>, credits: Seq<u64>) -> bool {
    sum_all_spec(debits) == sum_all_spec(credits)
}

// r[impl double-entry.balanced]
/// Check if debits and credits are balanced. Returns None on overflow.
pub fn is_balanced(debits: &[u64], credits: &[u64]) -> (result: Option<bool>)
    ensures
        match result {
            Some(v) => v == is_balanced_spec(debits@, credits@),
            None => sum_all_spec(debits@) > u64::MAX as int
                    || sum_all_spec(credits@) > u64::MAX as int,
        },
{
    let d = sum_amounts(debits);
    let c = sum_amounts(credits);
    match (d, c) {
        (Some(dv), Some(cv)) => Some(dv == cv),
        _ => None,
    }
}

// --- trial_balance ---

// r[depends double-entry.trial-balance]
/// Spec: debits minus credits (zero when balanced).
pub open spec fn trial_balance_spec(debits: Seq<u64>, credits: Seq<u64>) -> int {
    sum_all_spec(debits) - sum_all_spec(credits)
}

// r[impl double-entry.trial-balance]
/// Trial balance as (debits_exceed, abs_value). None on overflow.
pub fn trial_balance(debits: &[u64], credits: &[u64]) -> (result: Option<(bool, u64)>)
    ensures
        match result {
            Some(pair) =>
                pair.0 == (trial_balance_spec(debits@, credits@) >= 0)
                && pair.1 as int == if trial_balance_spec(debits@, credits@) >= 0 {
                    trial_balance_spec(debits@, credits@)
                } else {
                    -trial_balance_spec(debits@, credits@)
                },
            None => sum_all_spec(debits@) > u64::MAX as int
                    || sum_all_spec(credits@) > u64::MAX as int,
        },
{
    let d = sum_amounts(debits);
    let c = sum_amounts(credits);
    match (d, c) {
        (Some(dv), Some(cv)) => {
            if dv >= cv {
                Some((true, dv - cv))
            } else {
                Some((false, cv - dv))
            }
        }
        _ => None,
    }
}

// --- all_positive ---

// r[depends double-entry.all-positive]
/// Spec: every element is positive.
pub open spec fn all_positive_spec(amounts: Seq<u64>) -> bool {
    forall|i: int| #![trigger amounts[i]]
        0 <= i < amounts.len() ==> amounts[i] > 0
}

// r[impl double-entry.all-positive]
/// Returns true if every amount is nonzero.
pub fn all_positive(amounts: &[u64]) -> (result: bool)
    ensures result == all_positive_spec(amounts@)
{
    let mut i: usize = 0;
    while i < amounts.len()
        invariant
            0 <= i <= amounts@.len(),
            forall|j: int| #![trigger amounts@[j]]
                0 <= j < i as int ==> amounts@[j] > 0,
        decreases amounts@.len() - i,
    {
        if amounts[i] == 0 {
            return false;
        }
        i = i + 1;
    }
    true
}

// --- is_simple_entry ---

// r[depends double-entry.simple-entry]
/// Spec: single-entry transaction — one debit and one credit of the same amount.
pub open spec fn is_simple_entry_spec(debit_amount: u64, credit_amount: u64) -> bool {
    debit_amount == credit_amount && debit_amount > 0
}

// r[impl double-entry.simple-entry]
/// Returns true when debit and credit amounts are equal and positive.
pub fn is_simple_entry(debit_amount: u64, credit_amount: u64) -> (result: bool)
    ensures result == is_simple_entry_spec(debit_amount, credit_amount)
{
    debit_amount == credit_amount && debit_amount > 0
}

// --- Inductive helpers ---

// r[verify double-entry.sum]
/// Sum from any index is non-negative.
pub proof fn sum_nonneg(amounts: Seq<u64>, from: int)
    ensures sum_spec(amounts, from) >= 0
    decreases amounts.len() - from
{
    if from < amounts.len() {
        sum_nonneg(amounts, from + 1);
    }
}

// r[verify double-entry.sum]
/// Appending an element increases the sum by that element.
pub proof fn sum_append(amounts: Seq<u64>, x: u64)
    ensures sum_all_spec(amounts.push(x)) == sum_all_spec(amounts) + x as int
    decreases amounts.len()
{
    sum_append_from(amounts, x, 0);
}

/// Helper: sum_append from arbitrary index.
proof fn sum_append_from(amounts: Seq<u64>, x: u64, from: int)
    requires from >= 0
    ensures sum_spec(amounts.push(x), from) == sum_spec(amounts, from) + if from <= amounts.len() as int { x as int } else { 0 }
    decreases amounts.len() - from
{
    let extended = amounts.push(x);
    if from >= extended.len() {
        // both sums are 0
    } else if from == amounts.len() {
        // amounts[from] doesn't exist, extended[from] == x
        assert(extended[from] == x);
        assert(sum_spec(amounts, from) == 0);
        assert(sum_spec(extended, from + 1) == 0);
    } else {
        // from < amounts.len(): both have the same element at from
        assert(extended[from] == amounts[from]);
        sum_append_from(amounts, x, from + 1);
    }
}

// r[verify double-entry.sum]
/// Sum distributes over concatenation.
pub proof fn sum_concat(a: Seq<u64>, b: Seq<u64>)
    ensures sum_all_spec(a + b) == sum_all_spec(a) + sum_all_spec(b)
    decreases b.len()
{
    if b.len() == 0 {
        assert(a + b =~= a);
    } else {
        let b_prefix = b.subrange(0, b.len() - 1);
        let last = b[b.len() - 1];
        assert((a + b) =~= (a + b_prefix).push(last));
        sum_append(a + b_prefix, last);
        sum_concat(a, b_prefix);
        // Also need: sum_all_spec(b) == sum_all_spec(b_prefix) + last
        assert(b =~= b_prefix.push(last));
        sum_append(b_prefix, last);
    }
}

// --- Proofs ---

// r[verify double-entry.sum]
/// Sum of empty sequence is zero.
proof fn sum_empty()
    ensures sum_all_spec(Seq::<u64>::empty()) == 0
{}

// r[verify double-entry.sum]
/// Sum of single element is that element.
proof fn sum_single(x: u64)
    ensures sum_all_spec(seq![x]) == x as int
{
    // Unfold: sum_spec(seq![x], 0) = x + sum_spec(seq![x], 1) = x + 0
    assert(sum_spec(seq![x], 1) == 0);
}

// r[verify double-entry.balanced]
/// Balanced implies zero trial balance.
proof fn balanced_implies_zero_trial(debits: Seq<u64>, credits: Seq<u64>)
    requires is_balanced_spec(debits, credits)
    ensures trial_balance_spec(debits, credits) == 0
{}

// r[verify double-entry.trial-balance]
/// Zero trial balance implies balanced.
proof fn zero_trial_implies_balanced(debits: Seq<u64>, credits: Seq<u64>)
    requires trial_balance_spec(debits, credits) == 0
    ensures is_balanced_spec(debits, credits)
{}

// r[verify double-entry.balanced]
/// Empty sequences are balanced.
proof fn empty_is_balanced()
    ensures is_balanced_spec(Seq::<u64>::empty(), Seq::<u64>::empty())
{}

// r[verify double-entry.balanced]
/// A single equal amount on both sides is balanced.
proof fn single_equal_balanced(a: u64)
    ensures is_balanced_spec(seq![a], seq![a])
{}

// r[verify double-entry.balanced]
/// Balanced is symmetric: swapping debits and credits preserves balance.
proof fn balanced_symmetric(debits: Seq<u64>, credits: Seq<u64>)
    ensures is_balanced_spec(debits, credits) == is_balanced_spec(credits, debits)
{}

// r[verify double-entry.balanced]
/// Concatenating two balanced pairs stays balanced.
pub proof fn balanced_concat(d1: Seq<u64>, c1: Seq<u64>, d2: Seq<u64>, c2: Seq<u64>)
    requires is_balanced_spec(d1, c1), is_balanced_spec(d2, c2)
    ensures is_balanced_spec(d1 + d2, c1 + c2)
{
    sum_concat(d1, d2);
    sum_concat(c1, c2);
}

// r[verify double-entry.balanced]
/// Adding an equal amount to both sides preserves balance.
pub proof fn balanced_entry_preserves(debits: Seq<u64>, credits: Seq<u64>, amount: u64)
    requires is_balanced_spec(debits, credits)
    ensures is_balanced_spec(debits.push(amount), credits.push(amount))
{
    sum_append(debits, amount);
    sum_append(credits, amount);
}

// r[verify double-entry.trial-balance]
/// Trial balance is additive across concatenated entries.
pub proof fn trial_balance_additive(
    d1: Seq<u64>, c1: Seq<u64>, d2: Seq<u64>, c2: Seq<u64>,
)
    ensures trial_balance_spec(d1 + d2, c1 + c2)
        == trial_balance_spec(d1, c1) + trial_balance_spec(d2, c2)
{
    sum_concat(d1, d2);
    sum_concat(c1, c2);
}

// r[verify double-entry.simple-entry]
/// A simple entry is always balanced.
proof fn simple_entry_balanced(amount: u64)
    requires is_simple_entry_spec(amount, amount)
    ensures is_balanced_spec(seq![amount], seq![amount])
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify double-entry.sum]
    #[test]
    fn test_sum_amounts_basic() {
        assert_eq!(sum_amounts(&[]), Some(0));
        assert_eq!(sum_amounts(&[1, 2, 3]), Some(6));
        assert_eq!(sum_amounts(&[u64::MAX]), Some(u64::MAX));
        assert_eq!(sum_amounts(&[u64::MAX, 1]), None);
        assert_eq!(sum_amounts(&[0, 0, 0]), Some(0));
    }

    // r[verify double-entry.balanced]
    #[test]
    fn test_is_balanced_basic() {
        assert_eq!(is_balanced(&[], &[]), Some(true));
        assert_eq!(is_balanced(&[100], &[100]), Some(true));
        assert_eq!(is_balanced(&[60, 40], &[100]), Some(true));
        assert_eq!(is_balanced(&[100], &[50]), Some(false));
    }

    // r[verify double-entry.trial-balance]
    #[test]
    fn test_trial_balance_basic() {
        assert_eq!(trial_balance(&[100], &[100]), Some((true, 0)));
        assert_eq!(trial_balance(&[100], &[60]), Some((true, 40)));
        assert_eq!(trial_balance(&[60], &[100]), Some((false, 40)));
        assert_eq!(trial_balance(&[], &[]), Some((true, 0)));
    }

    // r[verify double-entry.all-positive]
    #[test]
    fn test_all_positive_basic() {
        assert!(all_positive(&[]));
        assert!(all_positive(&[1, 2, 3]));
        assert!(!all_positive(&[1, 0, 3]));
        assert!(!all_positive(&[0]));
    }

    // r[verify double-entry.simple-entry]
    #[test]
    fn test_is_simple_entry() {
        assert!(is_simple_entry(100, 100));
        assert!(!is_simple_entry(100, 50));
        assert!(!is_simple_entry(0, 0));
    }

    proptest! {
        // r[verify double-entry.balanced]
        #[test]
        fn prop_balanced_symmetric(
            d in proptest::collection::vec(0..1000u64, 0..10),
            c in proptest::collection::vec(0..1000u64, 0..10),
        ) {
            let b1 = is_balanced(&d, &c);
            let b2 = is_balanced(&c, &d);
            prop_assert_eq!(b1, b2);
        }

        // r[verify double-entry.balanced]
        #[test]
        fn prop_balanced_implies_zero_trial(
            d in proptest::collection::vec(0..1000u64, 0..10),
            c in proptest::collection::vec(0..1000u64, 0..10),
        ) {
            if let (Some(true), Some(tb)) = (is_balanced(&d, &c), trial_balance(&d, &c)) {
                prop_assert_eq!(tb, (true, 0));
            }
        }

        // r[verify double-entry.sum]
        #[test]
        fn prop_sum_matches_oracle(
            amounts in proptest::collection::vec(0..1_000_000u64, 0..20),
        ) {
            let oracle: u128 = amounts.iter().map(|&x| x as u128).sum();
            match sum_amounts(&amounts) {
                Some(v) => prop_assert_eq!(v as u128, oracle),
                None => prop_assert!(oracle > u64::MAX as u128),
            }
        }

        // r[verify double-entry.balanced]
        #[test]
        fn prop_add_equal_preserves_balance(
            d in proptest::collection::vec(0..1000u64, 0..5),
            c in proptest::collection::vec(0..1000u64, 0..5),
            x in 1..1000u64,
        ) {
            if let Some(true) = is_balanced(&d, &c) {
                let mut d2 = d.clone();
                let mut c2 = c.clone();
                d2.push(x);
                c2.push(x);
                prop_assert_eq!(is_balanced(&d2, &c2), Some(true));
            }
        }

        // r[verify double-entry.trial-balance]
        #[test]
        fn prop_trial_balance_oracle(
            d in proptest::collection::vec(0..1_000_000u64, 0..10),
            c in proptest::collection::vec(0..1_000_000u64, 0..10),
        ) {
            let d_sum: i128 = d.iter().map(|&x| x as i128).sum();
            let c_sum: i128 = c.iter().map(|&x| x as i128).sum();
            let expected = d_sum - c_sum;
            if let Some((positive, abs_val)) = trial_balance(&d, &c) {
                prop_assert_eq!(positive, expected >= 0);
                prop_assert_eq!(abs_val, expected.unsigned_abs() as u64);
            }
        }

        // r[verify double-entry.all-positive]
        #[test]
        fn prop_all_positive_oracle(
            amounts in proptest::collection::vec(0..100u64, 0..20),
        ) {
            let oracle = amounts.iter().all(|&x| x > 0);
            prop_assert_eq!(all_positive(&amounts), oracle);
        }
    }
}

//! Verified job idempotency tokens.
//!
//! Prevents duplicate execution by tracking consumed tokens. A token
//! is checked against a consumed set before execution; if already
//! consumed, the execution is skipped. Proofs cover freshness,
//! consumed permanence (monotonicity), and set size monotonicity.

use vstd::prelude::*;

verus! {

// ============================================================
// is_consumed: membership check
// ============================================================

// r[depends job-idempotency.is-consumed]
/// Spec: is the token in the consumed set?
pub open spec fn is_consumed_spec(token: u64, consumed: Seq<u64>) -> bool {
    exists|i: int| 0 <= i < consumed.len() as int && consumed[i] == token
}

// r[impl job-idempotency.is-consumed]
/// Check if a token has already been consumed.
pub fn is_consumed(token: u64, consumed: &[u64]) -> (result: bool)
    ensures result == is_consumed_spec(token, consumed@)
{
    let mut i: usize = 0;
    while i < consumed.len()
        invariant
            0 <= i <= consumed@.len(),
            !exists|k: int| 0 <= k < i as int && consumed@[k] == token,
        decreases consumed@.len() - i,
    {
        if consumed[i] == token {
            // Witness: i satisfies the existential.
            assert(consumed@[i as int] == token);
            return true;
        }
        i = i + 1;
    }
    false
}

// ============================================================
// try_execute: idempotent execution guard
// ============================================================

// r[depends job-idempotency.try-execute]
/// Spec: try to execute. Returns (proceed, should_append_token).
/// Fresh token → (true, true). Already consumed → (false, false).
pub open spec fn try_execute_spec(token: u64, consumed: Seq<u64>) -> (bool, bool) {
    if is_consumed_spec(token, consumed) {
        (false, false)
    } else {
        (true, true)
    }
}

// r[impl job-idempotency.try-execute]
/// Try to execute with an idempotency token.
/// Returns (proceed, should_append_token).
/// The caller appends the token to the consumed set when should_append is true.
pub fn try_execute(token: u64, consumed: &[u64]) -> (result: (bool, bool))
    ensures result == try_execute_spec(token, consumed@)
{
    let found = is_consumed(token, consumed);
    if found {
        (false, false)
    } else {
        (true, true)
    }
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-idempotency.is-consumed]
/// A fresh token is not consumed.
proof fn fresh_not_consumed(token: u64, consumed: Seq<u64>)
    requires forall|i: int| 0 <= i < consumed.len() as int ==> consumed[i] != token
    ensures !is_consumed_spec(token, consumed)
{}

// r[verify job-idempotency.is-consumed]
/// A consumed token stays consumed after appending any new token.
proof fn consumed_stays_consumed(token: u64, consumed: Seq<u64>, new_token: u64)
    requires is_consumed_spec(token, consumed)
    ensures is_consumed_spec(token, consumed.push(new_token))
{
    let witness = choose|i: int| 0 <= i < consumed.len() as int && consumed[i] == token;
    assert(consumed.push(new_token)[witness] == token);
}

// r[verify job-idempotency.is-consumed]
/// An appended token is consumed in the new set.
proof fn appended_is_consumed(token: u64, consumed: Seq<u64>)
    ensures is_consumed_spec(token, consumed.push(token))
{
    let idx = consumed.len() as int;
    assert(consumed.push(token)[idx] == token);
}

// r[verify job-idempotency.try-execute]
/// Fresh token → proceed.
proof fn fresh_proceeds(token: u64, consumed: Seq<u64>)
    requires !is_consumed_spec(token, consumed)
    ensures
        try_execute_spec(token, consumed).0 == true,
        try_execute_spec(token, consumed).1 == true,
{}

// r[verify job-idempotency.try-execute]
/// Already consumed → skip.
proof fn consumed_skips(token: u64, consumed: Seq<u64>)
    requires is_consumed_spec(token, consumed)
    ensures
        try_execute_spec(token, consumed).0 == false,
        try_execute_spec(token, consumed).1 == false,
{}

// r[verify job-idempotency.try-execute]
/// After try_execute with a fresh token and appending, the token is consumed.
proof fn execute_then_consumed(token: u64, consumed: Seq<u64>)
    requires !is_consumed_spec(token, consumed)
    ensures ({
        let new_consumed = consumed.push(token);
        is_consumed_spec(token, new_consumed)
    })
{
    appended_is_consumed(token, consumed);
}

// r[verify job-idempotency.is-consumed]
/// Monotonicity: consumed set size never decreases after try_execute.
/// Fresh token: size grows by 1. Duplicate: size unchanged.
proof fn size_monotonic(token: u64, consumed: Seq<u64>)
    ensures ({
        let (proceed, should_append) = try_execute_spec(token, consumed);
        if should_append {
            consumed.push(token).len() == consumed.len() + 1
        } else {
            true // size unchanged (caller doesn't append)
        }
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-idempotency.is-consumed]
    #[test]
    fn test_is_consumed() {
        assert!(!is_consumed(1, &[]));
        assert!(is_consumed(1, &[1, 2, 3]));
        assert!(!is_consumed(4, &[1, 2, 3]));
        assert!(is_consumed(3, &[1, 2, 3]));
    }

    // r[verify job-idempotency.try-execute]
    #[test]
    fn test_try_execute() {
        assert_eq!(try_execute(1, &[]), (true, true));
        assert_eq!(try_execute(1, &[1]), (false, false));
        assert_eq!(try_execute(2, &[1]), (true, true));
    }

    // r[verify job-idempotency.try-execute]
    #[test]
    fn test_execute_sequence() {
        let mut consumed: Vec<u64> = vec![];

        let (proceed, append) = try_execute(42, &consumed);
        assert!(proceed);
        assert!(append);
        consumed.push(42);

        let (proceed2, append2) = try_execute(42, &consumed);
        assert!(!proceed2);
        assert!(!append2);

        let (proceed3, append3) = try_execute(99, &consumed);
        assert!(proceed3);
        assert!(append3);
        consumed.push(99);

        assert_eq!(consumed.len(), 2);
    }

    proptest! {
        // r[verify job-idempotency.is-consumed]
        #[test]
        fn prop_fresh_not_consumed(token in 1000u64..2000) {
            let consumed: Vec<u64> = (0u64..100).collect();
            prop_assert!(!is_consumed(token, &consumed));
        }

        // r[verify job-idempotency.is-consumed]
        #[test]
        fn prop_present_is_consumed(idx in 0usize..100) {
            let consumed: Vec<u64> = (0u64..100).collect();
            prop_assert!(is_consumed(idx as u64, &consumed));
        }

        // r[verify job-idempotency.try-execute]
        #[test]
        fn prop_execute_once_then_skip(token in 0u64..1000) {
            let consumed: Vec<u64> = vec![];
            let (proceed, _) = try_execute(token, &consumed);
            prop_assert!(proceed);

            let consumed2 = vec![token];
            let (proceed2, _) = try_execute(token, &consumed2);
            prop_assert!(!proceed2);
        }

        // r[verify job-idempotency.is-consumed]
        #[test]
        fn prop_consumed_monotonic(
            existing in proptest::collection::vec(0u64..100, 0..20),
            new_token in 0u64..200,
        ) {
            // Every token consumed in existing is still consumed after push.
            let mut extended = existing.clone();
            extended.push(new_token);
            for &t in &existing {
                if is_consumed(t, &existing) {
                    prop_assert!(is_consumed(t, &extended));
                }
            }
        }
    }
}

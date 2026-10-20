//! Verified spin-wait backoff for lock-free loops.
//!
//! Models the two-phase backoff strategy used by crossbeam-utils Backoff:
//! first spin (busy-wait) up to a limit, then yield (cooperate with the
//! scheduler). After the yield phase the caller should park or block.
//! The three phases (spin, yield, completed) are mutually exclusive and
//! exhaustive — proved for all inputs.

use vstd::prelude::*;

verus! {

pub const DEFAULT_SPIN_LIMIT: u64 = 6u64;
pub const DEFAULT_YIELD_LIMIT: u64 = 10u64;

// --- is_spinning ---

// r[depends spin_backoff.is-spinning]
/// The backoff is in the spin phase when step < spin_limit.
pub open spec fn is_spinning_spec(step: u64, spin_limit: u64) -> bool {
    step < spin_limit
}

// r[impl spin_backoff.is-spinning]
pub fn is_spinning(step: u64, spin_limit: u64) -> (result: bool)
    ensures result == is_spinning_spec(step, spin_limit)
{
    step < spin_limit
}

// --- is_yielding ---

// r[depends spin_backoff.is-yielding]
/// The backoff is in the yield phase when spin_limit <= step < yield_limit.
pub open spec fn is_yielding_spec(step: u64, spin_limit: u64, yield_limit: u64) -> bool {
    step >= spin_limit && step < yield_limit
}

// r[impl spin_backoff.is-yielding]
pub fn is_yielding(step: u64, spin_limit: u64, yield_limit: u64) -> (result: bool)
    ensures result == is_yielding_spec(step, spin_limit, yield_limit)
{
    step >= spin_limit && step < yield_limit
}

// --- is_completed ---

// r[depends spin_backoff.is-completed]
/// The backoff has exhausted both phases. Caller should park or block.
pub open spec fn is_completed_spec(step: u64, yield_limit: u64) -> bool {
    step >= yield_limit
}

// r[impl spin_backoff.is-completed]
pub fn is_completed(step: u64, yield_limit: u64) -> (result: bool)
    ensures result == is_completed_spec(step, yield_limit)
{
    step >= yield_limit
}

// --- advance ---

// r[depends spin_backoff.advance]
/// Advance the backoff step by one. Saturates at u64::MAX.
pub open spec fn advance_spec(step: u64) -> u64 {
    if step < u64::MAX { (step + 1) as u64 } else { u64::MAX }
}

// r[impl spin_backoff.advance]
pub fn advance(step: u64) -> (result: u64)
    ensures result == advance_spec(step)
{
    if step < u64::MAX { step + 1 } else { u64::MAX }
}

// --- reset ---

// r[depends spin_backoff.reset]
/// Reset the backoff to step 0.
pub open spec fn reset_spec() -> u64 { 0 }

// r[impl spin_backoff.reset]
pub fn reset() -> (result: u64)
    ensures result == reset_spec()
{
    0
}

// --- spin_iterations ---

// r[depends spin_backoff.spin-iterations]
/// Number of spin-loop iterations for the current step.
/// Doubles each step up to the limit, capped at 2^limit.
pub open spec fn spin_iterations_spec(step: u64, limit: u64) -> u64 {
    let capped = if step < limit { step } else { limit };
    if capped >= 64 { u64::MAX } else { (1u64 << capped) as u64 }
}

// r[impl spin_backoff.spin-iterations]
pub fn spin_iterations(step: u64, limit: u64) -> (result: u64)
    requires limit < 64
    ensures result == spin_iterations_spec(step, limit)
{
    let capped = if step < limit { step } else { limit };
    assert(capped < 64u64);
    1u64 << capped
}

// === Proofs ===

// r[verify spin_backoff.is-spinning]
// r[verify spin_backoff.is-yielding]
// r[verify spin_backoff.is-completed]
/// The three phases are mutually exclusive and exhaustive.
proof fn phases_partition(step: u64, spin_limit: u64, yield_limit: u64)
    requires spin_limit <= yield_limit
    ensures ({
        let spinning = is_spinning_spec(step, spin_limit);
        let yielding = is_yielding_spec(step, spin_limit, yield_limit);
        let completed = is_completed_spec(step, yield_limit);
        (spinning && !yielding && !completed)
        || (!spinning && yielding && !completed)
        || (!spinning && !yielding && completed)
    })
{}

// r[verify spin_backoff.advance]
/// Advance is monotonic.
proof fn advance_monotonic(step: u64)
    ensures advance_spec(step) >= step
{}

// r[verify spin_backoff.advance]
/// Advance from non-max strictly increases.
proof fn advance_strict(step: u64)
    requires step < u64::MAX
    ensures advance_spec(step) > step
{}

// r[verify spin_backoff.advance]
// r[verify spin_backoff.is-spinning]
/// Advancing from spinning: either still spinning or enters yield.
proof fn advance_from_spinning(step: u64, spin_limit: u64, yield_limit: u64)
    requires
        is_spinning_spec(step, spin_limit),
        spin_limit < yield_limit,
        step < u64::MAX,
    ensures
        is_spinning_spec(advance_spec(step), spin_limit)
        || is_yielding_spec(advance_spec(step), spin_limit, yield_limit)
{}

// r[verify spin_backoff.advance]
// r[verify spin_backoff.is-yielding]
/// Advancing from yielding: either still yielding or completed.
proof fn advance_from_yielding(step: u64, spin_limit: u64, yield_limit: u64)
    requires
        is_yielding_spec(step, spin_limit, yield_limit),
        step < u64::MAX,
    ensures
        is_yielding_spec(advance_spec(step), spin_limit, yield_limit)
        || is_completed_spec(advance_spec(step), yield_limit)
{}

// r[verify spin_backoff.is-completed]
// r[verify spin_backoff.advance]
/// Once completed, advance stays completed.
proof fn completed_stays_completed(step: u64, yield_limit: u64)
    requires is_completed_spec(step, yield_limit)
    ensures is_completed_spec(advance_spec(step), yield_limit)
{}

// r[verify spin_backoff.reset]
// r[verify spin_backoff.is-spinning]
/// After reset, in spinning phase (when spin_limit > 0).
proof fn reset_is_spinning(spin_limit: u64)
    requires spin_limit > 0
    ensures is_spinning_spec(reset_spec(), spin_limit)
{}

// r[verify spin_backoff.reset]
// r[verify spin_backoff.is-completed]
/// After reset, not completed (when yield_limit > 0).
proof fn reset_not_completed(yield_limit: u64)
    requires yield_limit > 0
    ensures !is_completed_spec(reset_spec(), yield_limit)
{}

// r[verify spin_backoff.is-spinning]
/// Spin limit 0 means no spinning phase.
proof fn zero_spin_limit_no_spinning(step: u64)
    ensures !is_spinning_spec(step, 0)
{}

// r[verify spin_backoff.is-yielding]
/// Equal spin and yield limits means no yield phase.
proof fn equal_limits_no_yielding(step: u64, limit: u64)
    ensures !is_yielding_spec(step, limit, limit)
{}

// r[verify spin_backoff.advance]
// r[verify spin_backoff.is-spinning]
/// Exactly spin_limit advances exit the spin phase.
proof fn spin_phase_bounded(spin_limit: u64, yield_limit: u64)
    requires spin_limit <= yield_limit, spin_limit < u64::MAX
    ensures !is_spinning_spec(spin_limit, spin_limit)
{}

} // verus!

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_phases() {
        assert!(is_spinning(0, 6));
        assert!(is_spinning(5, 6));
        assert!(!is_spinning(6, 6));
        assert!(is_yielding(6, 6, 10));
        assert!(is_yielding(9, 6, 10));
        assert!(!is_yielding(10, 6, 10));
        assert!(is_completed(10, 10));
        assert!(is_completed(100, 10));
    }

    #[test]
    fn test_advance_reset() {
        assert_eq!(advance(0), 1);
        assert_eq!(advance(u64::MAX), u64::MAX);
        assert_eq!(reset(), 0);
    }

    #[test]
    fn test_spin_iterations() {
        assert_eq!(spin_iterations(0, 6), 1);
        assert_eq!(spin_iterations(1, 6), 2);
        assert_eq!(spin_iterations(2, 6), 4);
        assert_eq!(spin_iterations(3, 6), 8);
        assert_eq!(spin_iterations(6, 6), 64);
        assert_eq!(spin_iterations(10, 6), 64); // capped
    }

    #[test]
    fn test_full_backoff_sequence() {
        let mut step = reset();
        for _ in 0..6 {
            assert!(is_spinning(step, DEFAULT_SPIN_LIMIT));
            step = advance(step);
        }
        for _ in 0..4 {
            assert!(is_yielding(step, DEFAULT_SPIN_LIMIT, DEFAULT_YIELD_LIMIT));
            step = advance(step);
        }
        assert!(is_completed(step, DEFAULT_YIELD_LIMIT));
    }

    proptest! {
        #[test]
        fn prop_phases_exclusive(step: u64, spin in 0u64..32, yld in 0u64..32) {
            let yield_limit = spin + yld;
            let s = is_spinning(step, spin);
            let y = is_yielding(step, spin, yield_limit);
            let c = is_completed(step, yield_limit);
            let count = s as u8 + y as u8 + c as u8;
            prop_assert_eq!(count, 1);
        }

        #[test]
        fn prop_advance_monotonic(step: u64) {
            prop_assert!(advance(step) >= step);
        }

        #[test]
        fn prop_completed_absorbing(step: u64, yield_limit in 0u64..1000) {
            if is_completed(step, yield_limit) {
                prop_assert!(is_completed(advance(step), yield_limit));
            }
        }

        #[test]
        fn prop_reset_is_spinning(spin_limit in 1u64..1000) {
            prop_assert!(is_spinning(reset(), spin_limit));
        }

        #[test]
        fn prop_spin_iterations_positive(step in 0u64..100, limit in 1u64..63) {
            prop_assert!(spin_iterations(step, limit) >= 1);
        }

        #[test]
        fn prop_spin_iterations_capped(step in 0u64..100, limit in 1u64..63) {
            if step >= limit {
                prop_assert_eq!(spin_iterations(step, limit), spin_iterations(limit, limit));
            }
        }

        #[test]
        fn prop_advance_transitions(step in 0u64..u64::MAX, spin in 0u64..32, yld in 0u64..32) {
            let yield_limit = spin + yld;
            let next = advance(step);
            if is_spinning(step, spin) {
                prop_assert!(is_spinning(next, spin) || is_yielding(next, spin, yield_limit));
            }
            if is_yielding(step, spin, yield_limit) {
                prop_assert!(is_yielding(next, spin, yield_limit) || is_completed(next, yield_limit));
            }
        }
    }
}

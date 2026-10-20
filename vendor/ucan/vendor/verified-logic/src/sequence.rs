//! Verified monotonic sequence generator with batch reservation.
//!
//! Batch state predicates, range computation, overflow protection, and
//! CAS expected-value derivation. Sequence values never decrease; overflow
//! returns None instead of wrapping.

use vstd::prelude::*;

verus! {

// --- Batch state ---

// r[depends sequence.batch-state]
pub open spec fn should_refill_batch_spec(next: u64, batch_end: u64) -> bool {
    next >= batch_end
}

// r[impl sequence.batch-state]
pub fn should_refill_batch(next: u64, batch_end: u64) -> (result: bool)
    ensures result == should_refill_batch_spec(next, batch_end)
{
    next >= batch_end
}

// r[depends sequence.batch-state]
pub open spec fn batch_remaining_spec(next: u64, batch_end: u64) -> u64 {
    if next >= batch_end { 0u64 } else { (batch_end - next) as u64 }
}

// r[impl sequence.batch-state]
pub fn batch_remaining(next: u64, batch_end: u64) -> (result: u64)
    ensures result == batch_remaining_spec(next, batch_end)
{
    if next >= batch_end { 0 } else { batch_end - next }
}

// --- Batch computation ---

// r[depends sequence.batch-computation]
pub open spec fn compute_batch_end_spec(batch_start: u64, batch_size: u64) -> Option<u64> {
    if batch_start as int + batch_size as int > u64::MAX as int {
        None
    } else {
        Some((batch_start + batch_size) as u64)
    }
}

// r[impl sequence.batch-computation]
pub fn compute_batch_end(batch_start: u64, batch_size: u64) -> (result: Option<u64>)
    ensures result == compute_batch_end_spec(batch_start, batch_size)
{
    if batch_size > u64::MAX - batch_start {
        None
    } else {
        Some(batch_start + batch_size)
    }
}

// r[depends sequence.batch-computation]
pub open spec fn compute_next_after_refill_spec(batch_start: u64) -> Option<u64> {
    if batch_start == u64::MAX { None } else { Some((batch_start + 1) as u64) }
}

// r[impl sequence.batch-computation]
pub fn compute_next_after_refill(batch_start: u64) -> (result: Option<u64>)
    ensures result == compute_next_after_refill_spec(batch_start)
{
    if batch_start == u64::MAX { None } else { Some(batch_start + 1) }
}

// --- Range ---

// r[depends sequence.range-start]
pub open spec fn compute_range_start_spec(current: u64) -> Option<u64> {
    if current == u64::MAX { None } else { Some((current + 1) as u64) }
}

// r[impl sequence.range-start]
pub fn compute_range_start(current: u64) -> (result: Option<u64>)
    ensures result == compute_range_start_spec(current)
{
    if current == u64::MAX { None } else { Some(current + 1) }
}

// r[depends sequence.range-start]
pub open spec fn is_initial_reservation_spec(current: u64, start_value: u64) -> bool {
    current == start_value
}

// r[impl sequence.range-start]
pub fn is_initial_reservation(current: u64, start_value: u64) -> (result: bool)
    ensures result == is_initial_reservation_spec(current, start_value)
{
    current == start_value
}

// --- CAS ---

// r[depends sequence.cas-expected]
pub open spec fn compute_cas_expected_spec(current: u64, start_value: u64) -> Option<u64> {
    if current == start_value { None } else { Some(current) }
}

// r[impl sequence.cas-expected]
pub fn compute_cas_expected(current: u64, start_value: u64) -> (result: Option<u64>)
    ensures result == compute_cas_expected_spec(current, start_value)
{
    if current == start_value { None } else { Some(current) }
}

// --- Sequence value (the main operation) ---

// Result: 0 = overflow, 1 = ok
pub const SEQ_OVERFLOW: u8 = 0u8;
pub const SEQ_OK: u8 = 1u8;

// r[depends sequence.monotonic-values]
pub open spec fn compute_new_sequence_value_spec(current: u64, count: u64) -> (u8, u64) {
    if current as int + count as int > u64::MAX as int {
        (SEQ_OVERFLOW as u8, 0u64)
    } else {
        (SEQ_OK as u8, (current + count) as u64)
    }
}

// r[impl sequence.monotonic-values]
pub fn compute_new_sequence_value(current: u64, count: u64) -> (result: (u8, u64))
    ensures result == compute_new_sequence_value_spec(current, count)
{
    if count > u64::MAX - current {
        (SEQ_OVERFLOW, 0)
    } else {
        (SEQ_OK, current + count)
    }
}

// === Proofs ===

// r[verify sequence.monotonic-values]
proof fn new_value_gt_current(current: u64, count: u64)
    requires count > 0
    ensures
        compute_new_sequence_value_spec(current, count).0 == SEQ_OVERFLOW
        || compute_new_sequence_value_spec(current, count).1 > current
{}

// r[verify sequence.monotonic-values]
proof fn overflow_never_wraps(current: u64, count: u64)
    requires
        current as int + count as int > u64::MAX as int
    ensures compute_new_sequence_value_spec(current, count).0 == SEQ_OVERFLOW
{}

// r[verify sequence.batch-state]
proof fn refill_iff_remaining_zero(next: u64, batch_end: u64)
    ensures should_refill_batch_spec(next, batch_end) == (batch_remaining_spec(next, batch_end) == 0)
{}

// r[verify sequence.batch-computation]
proof fn batch_end_some_implies_ge_start(start: u64, size: u64)
    requires compute_batch_end_spec(start, size).is_some()
    ensures compute_batch_end_spec(start, size).unwrap() >= start
{}

// r[verify sequence.batch-computation]
proof fn next_after_refill_gt_start(start: u64)
    requires start < u64::MAX
    ensures compute_next_after_refill_spec(start).unwrap() > start
{}

// r[verify sequence.range-start]
proof fn range_start_gt_current(current: u64)
    requires current < u64::MAX
    ensures compute_range_start_spec(current).unwrap() > current
{}

// r[verify sequence.cas-expected]
proof fn cas_none_iff_initial(current: u64, start_value: u64)
    ensures compute_cas_expected_spec(current, start_value).is_none()
        == is_initial_reservation_spec(current, start_value)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sequence.batch-state]
    #[test]
    fn test_batch_state() {
        assert!(should_refill_batch(100, 100));
        assert!(should_refill_batch(150, 100));
        assert!(!should_refill_batch(50, 100));
        assert_eq!(batch_remaining(50, 100), 50);
        assert_eq!(batch_remaining(100, 100), 0);
        assert_eq!(batch_remaining(150, 100), 0);
    }

    // r[verify sequence.batch-computation]
    #[test]
    fn test_batch_computation() {
        assert_eq!(compute_batch_end(100, 50), Some(150));
        assert_eq!(compute_batch_end(u64::MAX - 10, 20), None);
        assert_eq!(compute_next_after_refill(100), Some(101));
        assert_eq!(compute_next_after_refill(u64::MAX), None);
    }

    // r[verify sequence.range-start]
    #[test]
    fn test_range() {
        assert_eq!(compute_range_start(50), Some(51));
        assert_eq!(compute_range_start(u64::MAX), None);
        assert!(is_initial_reservation(0, 0));
        assert!(!is_initial_reservation(50, 0));
    }

    // r[verify sequence.cas-expected]
    #[test]
    fn test_cas() {
        assert_eq!(compute_cas_expected(0, 0), None);
        assert_eq!(compute_cas_expected(100, 0), Some(100));
    }

    // r[verify sequence.monotonic-values]
    #[test]
    fn test_sequence_value() {
        let (status, val) = compute_new_sequence_value(100, 10);
        assert_eq!(status, SEQ_OK);
        assert_eq!(val, 110);
        let (status2, _) = compute_new_sequence_value(u64::MAX - 5, 10);
        assert_eq!(status2, SEQ_OVERFLOW);
    }

    proptest! {
        // r[verify sequence.monotonic-values]
        #[test]
        fn prop_new_value_monotonic(current: u64, count in 1u64..10000) {
            let (status, val) = compute_new_sequence_value(current, count);
            if status == SEQ_OK {
                prop_assert!(val > current);
            }
        }

        // r[verify sequence.batch-state]
        #[test]
        fn prop_refill_iff_zero(next: u64, batch_end: u64) {
            prop_assert_eq!(
                should_refill_batch(next, batch_end),
                batch_remaining(next, batch_end) == 0
            );
        }

        // r[verify sequence.batch-computation]
        #[test]
        fn prop_batch_end_ge_start(start: u64, size: u64) {
            if let Some(end) = compute_batch_end(start, size) {
                prop_assert!(end >= start);
            }
        }

        // r[verify sequence.cas-expected]
        #[test]
        fn prop_cas_none_iff_initial(current: u64, start: u64) {
            prop_assert_eq!(
                compute_cas_expected(current, start).is_none(),
                is_initial_reservation(current, start)
            );
        }
    }
}

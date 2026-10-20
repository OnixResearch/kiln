//! Verified sequential access detection for prefetch triggering.
//!
//! Models detection of sequential file reads to decide when to trigger
//! prefetching. Functions cover offset continuity checking, consecutive
//! counter updates, prefetch decisions, and size calculations. All
//! operations handle overflow properly and have machine-checked proofs.

use vstd::prelude::*;

verus! {

// --- is_sequential ---

// r[depends sequential.is-sequential]
/// Spec: whether the current offset continues the last access.
/// True if offset == last_offset + last_size, but false on overflow.
pub open spec fn is_sequential_spec(offset: u64, last_offset: u64, last_size: u64) -> bool {
    if last_offset as int + last_size as int > u64::MAX as int {
        false
    } else {
        offset == last_offset + last_size
    }
}

// r[impl sequential.is-sequential]
/// Returns true if the current offset immediately follows the last access.
/// Handles overflow by returning false when `last_offset` + `last_size` would overflow.
pub fn is_sequential(offset: u64, last_offset: u64, last_size: u64) -> (result: bool)
    ensures result == is_sequential_spec(offset, last_offset, last_size)
{
    if last_offset > u64::MAX - last_size {
        false
    } else {
        offset == last_offset + last_size
    }
}

// r[verify sequential.is-sequential]
/// is_sequential is deterministic: same inputs always give same output.
proof fn is_sequential_deterministic(offset: u64, last_offset: u64, last_size: u64)
    ensures is_sequential_spec(offset, last_offset, last_size) == is_sequential_spec(offset, last_offset, last_size)
{}

// r[verify sequential.is-sequential]
/// Initial state (offset 0, last_offset 0, last_size 0) is sequential.
proof fn is_sequential_initial_state()
    ensures is_sequential_spec(0, 0, 0)
{}

// r[verify sequential.is-sequential]
/// When no overflow, is_sequential matches arithmetic equality.
proof fn is_sequential_no_overflow(offset: u64, last_offset: u64, last_size: u64)
    requires last_offset as int + last_size as int <= u64::MAX as int
    ensures is_sequential_spec(offset, last_offset, last_size) == (offset == last_offset + last_size)
{}

// r[verify sequential.is-sequential]
/// When overflow would occur, is_sequential is always false.
proof fn is_sequential_overflow_false(offset: u64, last_offset: u64, last_size: u64)
    requires last_offset as int + last_size as int > u64::MAX as int
    ensures !is_sequential_spec(offset, last_offset, last_size)
{}

// --- update_consecutive ---

// r[depends sequential.update-consecutive]
/// Spec: update consecutive counter based on sequential detection.
pub open spec fn update_consecutive_spec(is_seq: bool, consecutive: u64) -> u64 {
    if is_seq {
        if consecutive < u64::MAX { (consecutive + 1) as u64 } else { u64::MAX }
    } else {
        0
    }
}

// r[impl sequential.update-consecutive]
/// Updates the consecutive access counter.
/// If sequential, increments (saturating at `u64::MAX`). If not, resets to 0.
pub fn update_consecutive(is_seq: bool, consecutive: u64) -> (result: u64)
    ensures result == update_consecutive_spec(is_seq, consecutive)
{
    if is_seq {
        if consecutive < u64::MAX { consecutive + 1 } else { u64::MAX }
    } else {
        0
    }
}

// r[verify sequential.update-consecutive]
/// Consecutive resets to 0 on non-sequential access.
proof fn consecutive_resets_on_non_sequential(consecutive: u64)
    ensures update_consecutive_spec(false, consecutive) == 0
{}

// r[verify sequential.update-consecutive]
/// Consecutive increases when sequential (unless at max).
proof fn consecutive_increases_when_sequential(consecutive: u64)
    requires consecutive < u64::MAX
    ensures update_consecutive_spec(true, consecutive) == consecutive + 1
{}

// r[verify sequential.update-consecutive]
/// Result is always 0 or greater than or equal to previous when sequential.
proof fn update_consecutive_monotonic(is_seq: bool, consecutive: u64)
    ensures
        update_consecutive_spec(is_seq, consecutive) == 0 ||
        update_consecutive_spec(is_seq, consecutive) >= consecutive
{}

// r[verify sequential.update-consecutive]
/// Saturates at u64::MAX.
proof fn consecutive_saturates(consecutive: u64)
    requires consecutive == u64::MAX
    ensures update_consecutive_spec(true, consecutive) == u64::MAX
{}

// --- should_prefetch ---

// r[depends sequential.should-prefetch]
/// Spec: whether prefetching should be triggered.
pub open spec fn should_prefetch_spec(consecutive: u64, threshold: u64) -> bool {
    consecutive >= threshold
}

// r[impl sequential.should-prefetch]
/// Returns true if the consecutive count meets the prefetch threshold.
/// Requires threshold > 0 to prevent spurious triggering.
pub fn should_prefetch(consecutive: u64, threshold: u64) -> (result: bool)
    requires threshold > 0
    ensures result == should_prefetch_spec(consecutive, threshold)
{
    consecutive >= threshold
}

// r[verify sequential.should-prefetch]
/// should_prefetch is false when consecutive < threshold.
proof fn should_prefetch_below_threshold(consecutive: u64, threshold: u64)
    requires consecutive < threshold
    ensures !should_prefetch_spec(consecutive, threshold)
{}

// r[verify sequential.should-prefetch]
/// should_prefetch is true when consecutive >= threshold.
proof fn should_prefetch_at_threshold(consecutive: u64, threshold: u64)
    requires consecutive >= threshold
    ensures should_prefetch_spec(consecutive, threshold)
{}

// r[verify sequential.should-prefetch]
/// should_prefetch is monotonic in consecutive count.
proof fn should_prefetch_monotonic(consecutive1: u64, consecutive2: u64, threshold: u64)
    requires consecutive1 <= consecutive2
    ensures should_prefetch_spec(consecutive1, threshold) ==> should_prefetch_spec(consecutive2, threshold)
{}

// --- prefetch_size ---

// r[depends sequential.prefetch-size]
/// Spec: calculate prefetch size with overflow protection.
pub open spec fn prefetch_size_spec(consecutive: u64, chunk_size: u64, max_prefetch: u64) -> u64 {
    if consecutive == 0 {
        0
    } else if consecutive as int * chunk_size as int > u64::MAX as int {
        max_prefetch
    } else {
        if (consecutive * chunk_size) as int <= max_prefetch as int {
            (consecutive * chunk_size) as u64
        } else {
            max_prefetch
        }
    }
}

// r[impl sequential.prefetch-size]
/// Calculates prefetch size as min(consecutive * `chunk_size`, `max_prefetch`).
/// Handles multiplication overflow by saturating to `max_prefetch`.
/// Requires `chunk_size` > 0.
pub fn prefetch_size(consecutive: u64, chunk_size: u64, max_prefetch: u64) -> (result: u64)
    requires chunk_size > 0
    ensures result == prefetch_size_spec(consecutive, chunk_size, max_prefetch)
{
    if consecutive == 0 {
        0
    } else {
        // Use checked multiplication to detect overflow
        match consecutive.checked_mul(chunk_size) {
            Some(product) => {
                if product <= max_prefetch {
                    product
                } else {
                    max_prefetch
                }
            }
            None => {
                // Multiplication overflow - return max_prefetch
                max_prefetch
            }
        }
    }
}

// r[verify sequential.prefetch-size]
/// Prefetch size is always <= max_prefetch.
proof fn prefetch_size_bounded(consecutive: u64, chunk_size: u64, max_prefetch: u64)
    requires chunk_size > 0
    ensures prefetch_size_spec(consecutive, chunk_size, max_prefetch) <= max_prefetch
{}

// r[verify sequential.prefetch-size]
/// Prefetch size is monotonic when consecutive values are 0.
proof fn prefetch_size_monotonic_from_zero(consecutive: u64, chunk_size: u64, max_prefetch: u64)
    requires chunk_size > 0
    ensures
        prefetch_size_spec(0, chunk_size, max_prefetch) <=
        prefetch_size_spec(consecutive, chunk_size, max_prefetch)
{
    // prefetch_size_spec(0, _, _) == 0, and 0 <= anything
}

// r[verify sequential.prefetch-size]
/// When no overflow and within bounds, prefetch size equals the product.
proof fn prefetch_size_exact(consecutive: u64, chunk_size: u64, max_prefetch: u64)
    requires
        chunk_size > 0,
        consecutive > 0,
        consecutive as int * chunk_size as int <= u64::MAX as int,
        consecutive * chunk_size <= max_prefetch
    ensures prefetch_size_spec(consecutive, chunk_size, max_prefetch) == consecutive * chunk_size
{}

// r[verify sequential.prefetch-size]
/// On multiplication overflow, prefetch size is max_prefetch.
proof fn prefetch_size_overflow(consecutive: u64, chunk_size: u64, max_prefetch: u64)
    requires
        chunk_size > 0,
        consecutive > 0,
        consecutive as int * chunk_size as int > u64::MAX as int
    ensures prefetch_size_spec(consecutive, chunk_size, max_prefetch) == max_prefetch
{}

// r[verify sequential.prefetch-size]
/// When consecutive is 0, prefetch size is 0.
proof fn prefetch_size_zero(chunk_size: u64, max_prefetch: u64)
    requires chunk_size > 0
    ensures prefetch_size_spec(0, chunk_size, max_prefetch) == 0
{}

// --- cross-function properties ---

// r[verify sequential.is-sequential]
// r[verify sequential.update-consecutive]
/// Sequential detection followed by consecutive update preserves sequentiality.
proof fn sequential_then_update_preserves(offset: u64, last_offset: u64, last_size: u64, consecutive: u64)
    ensures
        is_sequential_spec(offset, last_offset, last_size) ==>
        update_consecutive_spec(true, consecutive) > 0
{}

// r[verify sequential.update-consecutive]
// r[verify sequential.should-prefetch]
/// Consecutive update may trigger prefetching when threshold is met.
proof fn update_may_trigger_prefetch(consecutive: u64, threshold: u64)
    requires threshold > 0, consecutive + 1 >= threshold
    ensures should_prefetch_spec(update_consecutive_spec(true, consecutive), threshold)
{}

// r[verify sequential.should-prefetch]
// r[verify sequential.prefetch-size]
/// When prefetch is triggered, size is computed correctly.
proof fn prefetch_triggered_has_size(consecutive: u64, threshold: u64, chunk_size: u64, max_prefetch: u64)
    requires
        threshold > 0,
        chunk_size > 0,
        should_prefetch_spec(consecutive, threshold)
    ensures prefetch_size_spec(consecutive, chunk_size, max_prefetch) <= max_prefetch
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sequential.is-sequential]
    #[test]
    fn test_is_sequential() {
        assert!(is_sequential(0, 0, 0)); // initial state
        assert!(is_sequential(5, 0, 5)); // simple continuation
        assert!(is_sequential(100, 50, 50)); // larger continuation
        assert!(!is_sequential(10, 0, 5)); // gap
        assert!(!is_sequential(0, u64::MAX, 1)); // overflow
        assert!(!is_sequential(0, u64::MAX - 10, 20)); // overflow
    }

    // r[verify sequential.update-consecutive]
    #[test]
    fn test_update_consecutive() {
        assert_eq!(update_consecutive(false, 0), 0);
        assert_eq!(update_consecutive(false, 10), 0);
        assert_eq!(update_consecutive(true, 0), 1);
        assert_eq!(update_consecutive(true, 5), 6);
        assert_eq!(update_consecutive(true, u64::MAX), u64::MAX);
        assert_eq!(update_consecutive(true, u64::MAX - 1), u64::MAX);
    }

    // r[verify sequential.should-prefetch]
    #[test]
    fn test_should_prefetch() {
        assert!(!should_prefetch(0, 1));
        assert!(!should_prefetch(2, 3));
        assert!(should_prefetch(3, 3));
        assert!(should_prefetch(10, 3));
        assert!(should_prefetch(u64::MAX, 1));
    }

    // r[verify sequential.prefetch-size]
    #[test]
    fn test_prefetch_size() {
        assert_eq!(prefetch_size(0, 1024, 8192), 0);
        assert_eq!(prefetch_size(4, 1024, 8192), 4096);
        assert_eq!(prefetch_size(10, 1024, 8192), 8192); // capped
        assert_eq!(prefetch_size(u64::MAX, 2, 1000), 1000); // overflow
    }

    proptest! {
        // r[verify sequential.is-sequential]
        #[test]
        fn prop_is_sequential_deterministic(offset: u64, last_offset: u64, last_size: u64) {
            let result1 = is_sequential(offset, last_offset, last_size);
            let result2 = is_sequential(offset, last_offset, last_size);
            prop_assert_eq!(result1, result2);
        }

        // r[verify sequential.update-consecutive]
        #[test]
        fn prop_consecutive_resets_non_sequential(consecutive: u64) {
            prop_assert_eq!(update_consecutive(false, consecutive), 0);
        }

        // r[verify sequential.update-consecutive]
        #[test]
        fn prop_consecutive_increases_sequential(consecutive in 0u64..u64::MAX) {
            prop_assert_eq!(update_consecutive(true, consecutive), consecutive + 1);
        }

        // r[verify sequential.update-consecutive]
        #[test]
        fn prop_consecutive_saturates(consecutive: u64) {
            let result = update_consecutive(true, consecutive);
            prop_assert!(result == 0 || result > consecutive || result == u64::MAX);
        }

        // r[verify sequential.should-prefetch]
        #[test]
        fn prop_should_prefetch_monotonic(
            consecutive1: u64, consecutive2: u64, threshold in 1u64..=u64::MAX
        ) {
            if consecutive1 <= consecutive2 && should_prefetch(consecutive1, threshold) {
                prop_assert!(should_prefetch(consecutive2, threshold));
            }
        }

        // r[verify sequential.prefetch-size]
        #[test]
        fn prop_prefetch_size_bounded(
            consecutive: u64, chunk_size in 1u64..=u64::MAX, max_prefetch: u64
        ) {
            let size = prefetch_size(consecutive, chunk_size, max_prefetch);
            prop_assert!(size <= max_prefetch);
        }

        // r[verify sequential.prefetch-size]
        #[test]
        fn prop_prefetch_size_monotonic_from_zero(
            consecutive: u64, chunk_size in 1u64..1024u64, max_prefetch: u64
        ) {
            let size_zero = prefetch_size(0, chunk_size, max_prefetch);
            let size_cons = prefetch_size(consecutive, chunk_size, max_prefetch);
            prop_assert!(size_zero <= size_cons);
        }

        // r[verify sequential.is-sequential]
        #[test]
        fn prop_is_sequential_no_overflow(offset: u64, last_offset: u64, last_size: u64) {
            if last_offset <= u64::MAX - last_size {
                let expected = offset == last_offset + last_size;
                prop_assert_eq!(is_sequential(offset, last_offset, last_size), expected);
            }
        }

        // r[verify sequential.is-sequential]
        #[test]
        fn prop_is_sequential_overflow_false(offset: u64, last_offset: u64, last_size: u64) {
            if last_offset > u64::MAX - last_size {
                prop_assert!(!is_sequential(offset, last_offset, last_size));
            }
        }

        // r[verify sequential.update-consecutive]
        // r[verify sequential.should-prefetch]
        #[test]
        fn prop_lifecycle_consecutive_to_prefetch(
            consecutive: u64, threshold in 1u64..100u64
        ) {
            let updated = update_consecutive(true, consecutive);
            if updated >= threshold {
                prop_assert!(should_prefetch(updated, threshold));
            }
        }

        // r[verify sequential.should-prefetch]
        // r[verify sequential.prefetch-size]
        #[test]
        fn prop_lifecycle_prefetch_to_size(
            consecutive: u64, threshold in 1u64..100u64,
            chunk_size in 1u64..1024u64, max_prefetch in 1000u64..=u64::MAX
        ) {
            if should_prefetch(consecutive, threshold) {
                let size = prefetch_size(consecutive, chunk_size, max_prefetch);
                prop_assert!(size <= max_prefetch);
                prop_assert!(size > 0 || consecutive == 0);
            }
        }
    }
}

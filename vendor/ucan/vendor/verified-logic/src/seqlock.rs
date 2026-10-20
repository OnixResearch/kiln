//! Verified sequence lock (seqlock) protocol.
//!
//! A seqlock uses an even/odd sequence counter for reader-writer
//! synchronization. Writers increment the counter to odd before writing
//! and back to even after. Readers sample the counter before and after
//! reading — if the counter changed or was odd, the read is invalid
//! and must be retried.

use vstd::prelude::*;

verus! {

// --- is_write_in_progress ---

// r[depends seqlock.write-in-progress]
/// A write is in progress when the sequence counter is odd.
pub open spec fn is_write_in_progress_spec(seq_num: u64) -> bool {
    seq_num % 2 != 0
}

// r[impl seqlock.write-in-progress]
pub fn is_write_in_progress(seq_num: u64) -> (result: bool)
    ensures result == is_write_in_progress_spec(seq_num)
{
    seq_num % 2 != 0
}

// --- begin_write ---

// r[depends seqlock.begin-write]
/// Begin a write: increment counter from even to odd.
/// Returns the new (odd) sequence number.
pub open spec fn begin_write_spec(seq_num: u64) -> u64 {
    (seq_num + 1) as u64
}

// r[impl seqlock.begin-write]
pub fn begin_write(seq_num: u64) -> (result: u64)
    requires seq_num < u64::MAX, !is_write_in_progress_spec(seq_num)
    ensures result == begin_write_spec(seq_num)
{
    seq_num + 1
}

// --- end_write ---

// r[depends seqlock.end-write]
/// End a write: increment counter from odd to even.
/// Returns the new (even) sequence number.
pub open spec fn end_write_spec(seq_num: u64) -> u64 {
    (seq_num + 1) as u64
}

// r[impl seqlock.end-write]
pub fn end_write(seq_num: u64) -> (result: u64)
    requires seq_num < u64::MAX, is_write_in_progress_spec(seq_num)
    ensures result == end_write_spec(seq_num)
{
    seq_num + 1
}

// --- is_valid_read ---

// r[depends seqlock.valid-read]
/// A read is valid when the sequence counter was even before and
/// unchanged after the read.
pub open spec fn is_valid_read_spec(seq_before: u64, seq_after: u64) -> bool {
    seq_before == seq_after && seq_before % 2 == 0
}

// r[impl seqlock.valid-read]
pub fn is_valid_read(seq_before: u64, seq_after: u64) -> (result: bool)
    ensures result == is_valid_read_spec(seq_before, seq_after)
{
    seq_before == seq_after && seq_before % 2 == 0
}

// --- should_retry ---

// r[depends seqlock.should-retry]
/// A read should be retried when it is not valid.
pub open spec fn should_retry_spec(seq_before: u64, seq_after: u64) -> bool {
    !is_valid_read_spec(seq_before, seq_after)
}

// r[impl seqlock.should-retry]
pub fn should_retry(seq_before: u64, seq_after: u64) -> (result: bool)
    ensures result == should_retry_spec(seq_before, seq_after)
{
    !is_valid_read(seq_before, seq_after)
}

// --- writer_version ---

// r[depends seqlock.writer-version]
/// The version (completed write count) is seq_num / 2.
pub open spec fn writer_version_spec(seq_num: u64) -> u64 {
    (seq_num / 2) as u64
}

// r[impl seqlock.writer-version]
pub fn writer_version(seq_num: u64) -> (result: u64)
    ensures result == writer_version_spec(seq_num)
{
    seq_num / 2
}

// === Proofs ===

// r[verify seqlock.write-in-progress]
/// Zero is not a write in progress.
proof fn zero_not_writing()
    ensures !is_write_in_progress_spec(0)
{}

// r[verify seqlock.begin-write]
/// begin_write produces an odd counter.
proof fn begin_write_is_odd(seq_num: u64)
    requires seq_num < u64::MAX, !is_write_in_progress_spec(seq_num)
    ensures is_write_in_progress_spec(begin_write_spec(seq_num))
{}

// r[verify seqlock.end-write]
/// end_write produces an even counter.
proof fn end_write_is_even(seq_num: u64)
    requires seq_num < u64::MAX, is_write_in_progress_spec(seq_num)
    ensures !is_write_in_progress_spec(end_write_spec(seq_num))
{}

// r[verify seqlock.begin-write]
// r[verify seqlock.end-write]
/// A full write cycle (begin + end) increases the counter by 2.
proof fn write_cycle_adds_two(seq_num: u64)
    requires
        seq_num < u64::MAX - 1,
        !is_write_in_progress_spec(seq_num),
    ensures
        end_write_spec(begin_write_spec(seq_num)) == (seq_num + 2) as u64
{}

// r[verify seqlock.begin-write]
// r[verify seqlock.end-write]
/// After a full write cycle, the counter is still even.
proof fn write_cycle_preserves_even(seq_num: u64)
    requires
        seq_num < u64::MAX - 1,
        !is_write_in_progress_spec(seq_num),
    ensures
        !is_write_in_progress_spec(end_write_spec(begin_write_spec(seq_num)))
{}

// r[verify seqlock.valid-read]
/// A read during an idle period (even, unchanged) is valid.
proof fn idle_read_is_valid(seq_num: u64)
    requires !is_write_in_progress_spec(seq_num)
    ensures is_valid_read_spec(seq_num, seq_num)
{}

// r[verify seqlock.valid-read]
/// A read that sees an odd counter is invalid.
proof fn odd_read_is_invalid(seq_num: u64, seq_after: u64)
    requires is_write_in_progress_spec(seq_num)
    ensures !is_valid_read_spec(seq_num, seq_after)
{}

// r[verify seqlock.valid-read]
/// A read that sees a changed counter is invalid.
proof fn changed_read_is_invalid(seq_before: u64, seq_after: u64)
    requires seq_before != seq_after
    ensures !is_valid_read_spec(seq_before, seq_after)
{}

// r[verify seqlock.should-retry]
/// should_retry is complement of is_valid_read.
proof fn retry_complement(seq_before: u64, seq_after: u64)
    ensures should_retry_spec(seq_before, seq_after)
        == !is_valid_read_spec(seq_before, seq_after)
{}

// r[verify seqlock.begin-write]
/// begin_write strictly increases the counter.
proof fn begin_write_monotonic(seq_num: u64)
    requires seq_num < u64::MAX, !is_write_in_progress_spec(seq_num)
    ensures begin_write_spec(seq_num) > seq_num
{}

// r[verify seqlock.end-write]
/// end_write strictly increases the counter.
proof fn end_write_monotonic(seq_num: u64)
    requires seq_num < u64::MAX, is_write_in_progress_spec(seq_num)
    ensures end_write_spec(seq_num) > seq_num
{}

// r[verify seqlock.writer-version]
/// Version zero at start.
proof fn initial_version()
    ensures writer_version_spec(0) == 0
{}

// r[verify seqlock.writer-version]
// r[verify seqlock.begin-write]
// r[verify seqlock.end-write]
/// A full write cycle increments the version by one.
proof fn write_cycle_increments_version(seq_num: u64)
    requires
        seq_num < u64::MAX - 1,
        !is_write_in_progress_spec(seq_num),
    ensures
        writer_version_spec(end_write_spec(begin_write_spec(seq_num)))
            == (writer_version_spec(seq_num) + 1) as u64
{}

// r[verify seqlock.valid-read]
// r[verify seqlock.begin-write]
/// Reading the counter after begin_write (before end) always produces an invalid read
/// when the before-sample was taken before begin_write.
proof fn read_spanning_write_invalid(seq_before: u64, write_start: u64)
    requires
        !is_write_in_progress_spec(seq_before),
        !is_write_in_progress_spec(write_start),
        write_start >= seq_before,
        write_start < u64::MAX,
    ensures
        !is_valid_read_spec(seq_before, begin_write_spec(write_start))
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify seqlock.write-in-progress]
    #[test]
    fn test_write_in_progress() {
        assert!(!is_write_in_progress(0));
        assert!(is_write_in_progress(1));
        assert!(!is_write_in_progress(2));
        assert!(is_write_in_progress(3));
        assert!(!is_write_in_progress(u64::MAX - 1)); // even
        assert!(is_write_in_progress(u64::MAX)); // odd
    }

    // r[verify seqlock.begin-write]
    // r[verify seqlock.end-write]
    #[test]
    fn test_write_cycle() {
        let s0 = 0u64;
        let s1 = begin_write(s0);
        assert_eq!(s1, 1);
        assert!(is_write_in_progress(s1));
        let s2 = end_write(s1);
        assert_eq!(s2, 2);
        assert!(!is_write_in_progress(s2));
    }

    // r[verify seqlock.valid-read]
    #[test]
    fn test_valid_read() {
        assert!(is_valid_read(0, 0));
        assert!(is_valid_read(4, 4));
        assert!(!is_valid_read(0, 2)); // changed
        assert!(!is_valid_read(1, 1)); // odd
        assert!(!is_valid_read(1, 3)); // odd and changed
    }

    // r[verify seqlock.should-retry]
    #[test]
    fn test_should_retry() {
        assert!(!should_retry(0, 0));
        assert!(should_retry(0, 2));
        assert!(should_retry(1, 1));
    }

    // r[verify seqlock.writer-version]
    #[test]
    fn test_writer_version() {
        assert_eq!(writer_version(0), 0);
        assert_eq!(writer_version(2), 1);
        assert_eq!(writer_version(4), 2);
        assert_eq!(writer_version(100), 50);
    }

    proptest! {
        // r[verify seqlock.write-in-progress]
        #[test]
        fn prop_even_not_writing(n in 0u64..u64::MAX / 2) {
            prop_assert!(!is_write_in_progress(n * 2));
        }

        // r[verify seqlock.begin-write]
        // r[verify seqlock.end-write]
        #[test]
        fn prop_write_cycle_even(seq in (0u64..u64::MAX / 2).prop_map(|n| n * 2)) {
            if seq < u64::MAX - 1 {
                let after = end_write(begin_write(seq));
                prop_assert!(!is_write_in_progress(after));
                prop_assert_eq!(after, seq + 2);
            }
        }

        // r[verify seqlock.valid-read]
        #[test]
        fn prop_idle_read_valid(seq in (0u64..u64::MAX / 2).prop_map(|n| n * 2)) {
            prop_assert!(is_valid_read(seq, seq));
        }

        // r[verify seqlock.valid-read]
        #[test]
        fn prop_odd_read_invalid(seq: u64, after: u64) {
            if is_write_in_progress(seq) {
                prop_assert!(!is_valid_read(seq, after));
            }
        }

        // r[verify seqlock.valid-read]
        #[test]
        fn prop_changed_read_invalid(before: u64, after: u64) {
            if before != after {
                prop_assert!(!is_valid_read(before, after));
            }
        }

        // r[verify seqlock.should-retry]
        #[test]
        fn prop_retry_complement(before: u64, after: u64) {
            prop_assert_eq!(should_retry(before, after), !is_valid_read(before, after));
        }

        // r[verify seqlock.writer-version]
        #[test]
        fn prop_write_cycle_increments_version(seq in (0u64..u64::MAX / 2 - 1).prop_map(|n| n * 2)) {
            let v_before = writer_version(seq);
            let after = end_write(begin_write(seq));
            let v_after = writer_version(after);
            prop_assert_eq!(v_after, v_before + 1);
        }

        // r[verify seqlock.begin-write]
        // r[verify seqlock.end-write]
        #[test]
        fn prop_write_monotonic(seq in (0u64..u64::MAX / 2).prop_map(|n| n * 2)) {
            if seq < u64::MAX {
                let s1 = begin_write(seq);
                prop_assert!(s1 > seq);
                if s1 < u64::MAX {
                    let s2 = end_write(s1);
                    prop_assert!(s2 > s1);
                }
            }
        }
    }
}

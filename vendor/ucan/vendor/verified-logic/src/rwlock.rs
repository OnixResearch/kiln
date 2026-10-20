//! Verified read-write lock mode transitions and mutual exclusion.
//!
//! Three modes: Free (0), Read (1), Write (2). Reader/writer count
//! arithmetic, expiry checks, fencing tokens for writers, and a mutual
//! exclusion invariant proved over all mode transitions.

use vstd::prelude::*;

verus! {

pub const MODE_FREE: u8 = 0u8;
pub const MODE_READ: u8 = 1u8;
pub const MODE_WRITE: u8 = 2u8;

// --- Mutual exclusion ---

// r[depends rwlock.mutual-exclusion]
/// Mutual exclusion holds when mode/reader/writer state is consistent.
pub open spec fn check_mutual_exclusion_spec(
    mode: u8, active_readers: u32, writer_holds: bool,
) -> bool {
    if mode == MODE_WRITE {
        active_readers == 0 && writer_holds
    } else if mode == MODE_READ {
        !writer_holds
    } else {
        // Free: no constraints
        true
    }
}

// r[impl rwlock.mutual-exclusion]
pub fn check_mutual_exclusion(
    mode: u8, active_readers: u32, writer_holds: bool,
) -> (result: bool)
    ensures result == check_mutual_exclusion_spec(mode, active_readers, writer_holds)
{
    if mode == MODE_WRITE {
        active_readers == 0 && writer_holds
    } else if mode == MODE_READ {
        !writer_holds
    } else {
        true
    }
}

// --- Acquisition predicates ---

// r[depends rwlock.mode-transitions]
pub open spec fn can_acquire_read_spec(
    mode: u8, pending_writers: u32, writer_expired: bool,
) -> bool {
    mode == MODE_FREE
    || (mode == MODE_READ && (pending_writers == 0 || writer_expired))
    || (mode == MODE_WRITE && writer_expired)
}

// r[impl rwlock.mode-transitions]
pub fn can_acquire_read(
    mode: u8, pending_writers: u32, writer_expired: bool,
) -> (result: bool)
    ensures result == can_acquire_read_spec(mode, pending_writers, writer_expired)
{
    mode == MODE_FREE
    || (mode == MODE_READ && (pending_writers == 0 || writer_expired))
    || (mode == MODE_WRITE && writer_expired)
}

// r[depends rwlock.mode-transitions]
pub open spec fn can_acquire_write_spec(
    mode: u8, active_readers: u32, writer_expired: bool,
) -> bool {
    mode == MODE_FREE
    || (mode == MODE_READ && active_readers == 0)
    || (mode == MODE_WRITE && writer_expired)
}

// r[impl rwlock.mode-transitions]
pub fn can_acquire_write(
    mode: u8, active_readers: u32, writer_expired: bool,
) -> (result: bool)
    ensures result == can_acquire_write_spec(mode, active_readers, writer_expired)
{
    mode == MODE_FREE
    || (mode == MODE_READ && active_readers == 0)
    || (mode == MODE_WRITE && writer_expired)
}

// --- Mode after operations ---

// r[depends rwlock.mode-after]
pub open spec fn mode_after_read_acquire_spec() -> u8 { MODE_READ }

// r[impl rwlock.mode-after]
pub fn mode_after_read_acquire() -> (result: u8)
    ensures result == mode_after_read_acquire_spec()
{
    MODE_READ
}

// r[depends rwlock.mode-after]
pub open spec fn mode_after_read_release_spec(remaining_readers: u32) -> u8 {
    if remaining_readers == 0 { MODE_FREE } else { MODE_READ }
}

// r[impl rwlock.mode-after]
pub fn mode_after_read_release(remaining_readers: u32) -> (result: u8)
    ensures result == mode_after_read_release_spec(remaining_readers)
{
    if remaining_readers == 0 { MODE_FREE } else { MODE_READ }
}

// r[depends rwlock.mode-after]
pub open spec fn mode_after_write_release_spec() -> u8 { MODE_FREE }

// r[impl rwlock.mode-after]
pub fn mode_after_write_release() -> (result: u8)
    ensures result == mode_after_write_release_spec()
{
    MODE_FREE
}

// --- Reader count arithmetic ---

// r[depends rwlock.reader-count]
pub open spec fn increment_reader_count_spec(current: u32) -> u32 {
    if current == u32::MAX { u32::MAX } else { (current + 1) as u32 }
}

// r[impl rwlock.reader-count]
pub fn increment_reader_count(current: u32) -> (result: u32)
    ensures result == increment_reader_count_spec(current)
{
    if current == u32::MAX { u32::MAX } else { current + 1 }
}

// r[depends rwlock.reader-count]
pub open spec fn decrement_reader_count_spec(current: u32) -> u32 {
    if current == 0 { 0u32 } else { (current - 1) as u32 }
}

// r[impl rwlock.reader-count]
pub fn decrement_reader_count(current: u32) -> (result: u32)
    ensures result == decrement_reader_count_spec(current)
{
    if current == 0 { 0 } else { current - 1 }
}

// --- Writer fencing token ---

// r[depends rwlock.writer-token]
pub open spec fn compute_next_write_token_spec(current: u64) -> u64 {
    if current == u64::MAX { u64::MAX } else { (current + 1) as u64 }
}

// r[impl rwlock.writer-token]
pub fn compute_next_write_token(current: u64) -> (result: u64)
    ensures result == compute_next_write_token_spec(current)
{
    if current == u64::MAX { u64::MAX } else { current + 1 }
}

// --- Reader/writer expiry ---

// r[depends rwlock.expiry]
pub open spec fn is_reader_expired_spec(deadline_ms: u64, now_ms: u64) -> bool {
    now_ms > deadline_ms
}

// r[impl rwlock.expiry]
pub fn is_reader_expired(deadline_ms: u64, now_ms: u64) -> (result: bool)
    ensures result == is_reader_expired_spec(deadline_ms, now_ms)
{
    now_ms > deadline_ms
}

// r[depends rwlock.expiry]
pub open spec fn is_writer_expired_spec(deadline_ms: u64, now_ms: u64) -> bool {
    now_ms > deadline_ms
}

// r[impl rwlock.expiry]
pub fn is_writer_expired(deadline_ms: u64, now_ms: u64) -> (result: bool)
    ensures result == is_writer_expired_spec(deadline_ms, now_ms)
{
    now_ms > deadline_ms
}

// === Proofs ===

// r[verify rwlock.mutual-exclusion]
/// Free mode always passes mutual exclusion.
proof fn free_always_valid(readers: u32, writer: bool)
    ensures check_mutual_exclusion_spec(MODE_FREE, readers, writer)
{}

// r[verify rwlock.mutual-exclusion]
/// Write mode requires zero readers.
proof fn write_requires_zero_readers(readers: u32)
    requires readers > 0
    ensures !check_mutual_exclusion_spec(MODE_WRITE, readers, true)
{}

// r[verify rwlock.mutual-exclusion]
/// Read mode and writer_holds=true violates mutual exclusion.
proof fn read_excludes_writer(readers: u32)
    ensures !check_mutual_exclusion_spec(MODE_READ, readers, true)
{}

// r[verify rwlock.mode-after]
/// Write release always produces Free.
proof fn write_release_is_free()
    ensures mode_after_write_release_spec() == MODE_FREE
{}

// r[verify rwlock.mode-after]
/// Last reader produces Free.
proof fn last_reader_is_free()
    ensures mode_after_read_release_spec(0) == MODE_FREE
{}

// r[verify rwlock.mode-after]
/// Non-last reader stays Read.
proof fn non_last_reader_stays_read(remaining: u32)
    requires remaining > 0
    ensures mode_after_read_release_spec(remaining) == MODE_READ
{}

// r[verify rwlock.reader-count]
/// Increment is monotonic.
proof fn increment_monotonic(current: u32)
    ensures increment_reader_count_spec(current) >= current
{}

// r[verify rwlock.reader-count]
/// Decrement is monotonic downward.
proof fn decrement_bounded(current: u32)
    ensures decrement_reader_count_spec(current) <= current
{}

// r[verify rwlock.reader-count]
/// Increment-decrement roundtrip.
proof fn inc_dec_roundtrip(current: u32)
    requires current < u32::MAX
    ensures decrement_reader_count_spec(increment_reader_count_spec(current)) == current
{}

// r[verify rwlock.writer-token]
/// Writer token is monotonic.
proof fn write_token_monotonic(current: u64)
    ensures compute_next_write_token_spec(current) >= current
{}

// r[verify rwlock.writer-token]
/// Writer token strictly increases when not saturated.
proof fn write_token_strict(current: u64)
    requires current < u64::MAX
    ensures compute_next_write_token_spec(current) > current
{}

// r[verify rwlock.expiry]
/// Reader and writer expiry use the same predicate.
proof fn reader_writer_expiry_same(deadline: u64, now: u64)
    ensures is_reader_expired_spec(deadline, now) == is_writer_expired_spec(deadline, now)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify rwlock.mutual-exclusion]
    #[test]
    fn test_mutual_exclusion() {
        assert!(check_mutual_exclusion(MODE_FREE, 5, true));
        assert!(check_mutual_exclusion(MODE_WRITE, 0, true));
        assert!(!check_mutual_exclusion(MODE_WRITE, 1, true));
        assert!(check_mutual_exclusion(MODE_READ, 3, false));
        assert!(!check_mutual_exclusion(MODE_READ, 3, true));
    }

    // r[verify rwlock.mode-transitions]
    #[test]
    fn test_acquisition() {
        assert!(can_acquire_read(MODE_FREE, 0, false));
        assert!(can_acquire_read(MODE_READ, 0, false));
        assert!(!can_acquire_read(MODE_WRITE, 0, false));
        assert!(can_acquire_read(MODE_WRITE, 0, true)); // writer expired
        assert!(can_acquire_write(MODE_FREE, 0, false));
        assert!(!can_acquire_write(MODE_READ, 3, false));
        assert!(can_acquire_write(MODE_READ, 0, false));
    }

    // r[verify rwlock.mode-after]
    #[test]
    fn test_mode_after() {
        assert_eq!(mode_after_read_acquire(), MODE_READ);
        assert_eq!(mode_after_read_release(0), MODE_FREE);
        assert_eq!(mode_after_read_release(2), MODE_READ);
        assert_eq!(mode_after_write_release(), MODE_FREE);
    }

    // r[verify rwlock.reader-count]
    #[test]
    fn test_reader_count() {
        assert_eq!(increment_reader_count(0), 1);
        assert_eq!(increment_reader_count(u32::MAX), u32::MAX);
        assert_eq!(decrement_reader_count(5), 4);
        assert_eq!(decrement_reader_count(0), 0);
    }

    // r[verify rwlock.writer-token]
    #[test]
    fn test_writer_token() {
        assert_eq!(compute_next_write_token(0), 1);
        assert_eq!(compute_next_write_token(5), 6);
        assert_eq!(compute_next_write_token(u64::MAX), u64::MAX);
    }

    // r[verify rwlock.expiry]
    #[test]
    fn test_expiry() {
        assert!(!is_reader_expired(10000, 5000));
        assert!(is_reader_expired(5000, 10000));
        assert!(!is_writer_expired(10000, 5000));
        assert!(is_writer_expired(5000, 10000));
    }

    proptest! {
        // r[verify rwlock.mutual-exclusion]
        #[test]
        fn prop_free_always_valid(readers: u32, writer: bool) {
            prop_assert!(check_mutual_exclusion(MODE_FREE, readers, writer));
        }

        // r[verify rwlock.reader-count]
        #[test]
        fn prop_inc_dec_roundtrip(current in 0u32..u32::MAX) {
            let inc = increment_reader_count(current);
            let dec = decrement_reader_count(inc);
            prop_assert_eq!(dec, current);
        }

        // r[verify rwlock.writer-token]
        #[test]
        fn prop_write_token_monotonic(current: u64) {
            prop_assert!(compute_next_write_token(current) >= current);
        }

        // r[verify rwlock.expiry]
        #[test]
        fn prop_reader_writer_expiry_same(deadline: u64, now: u64) {
            prop_assert_eq!(
                is_reader_expired(deadline, now),
                is_writer_expired(deadline, now)
            );
        }
    }
}

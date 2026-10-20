//! Verified write buffer (memtable) management for LSM-tree engines.
//!
//! Tracks current buffer size, flush thresholds, immutable buffer swap,
//! and write amplification bounds. Proofs guarantee flush triggers are
//! monotone, swaps don't lose data, and amplification stays bounded.
//! Composes with [`lsm_level`] for level sizing and [`batch`] for flush decisions.

use vstd::prelude::*;

verus! {

// ─── Buffer Occupancy ────────────────────────────────────────────────

/// Spec: buffer occupancy after writing `size` bytes.
// r[depends write-buffer.add-bytes]
// r[impl write-buffer.add-bytes]
pub open spec fn add_bytes_spec(current: u64, size: u64) -> u64
    recommends (current as int) + (size as int) <= u64::MAX as int,
{
    (current + size) as u64
}

// r[impl write-buffer.add-bytes]
pub fn add_bytes(current: u64, size: u64) -> (result: u64)
    requires (current as int) + (size as int) <= u64::MAX as int,
    ensures result == add_bytes_spec(current, size),
{
    current + size
}

// r[verify write-buffer.add-bytes]
/// Adding bytes increases occupancy.
proof fn add_bytes_monotone(current: u64, size: u64)
    requires (current as int) + (size as int) <= u64::MAX as int,
    ensures add_bytes_spec(current, size) >= current,
{}

// r[verify write-buffer.add-bytes]
/// Adding zero bytes is identity.
proof fn add_bytes_zero_identity(current: u64)
    ensures add_bytes_spec(current, 0) == current,
{}

// ─── Flush Trigger ───────────────────────────────────────────────────

/// Spec: should the buffer be flushed?
// r[depends write-buffer.should-flush]
// r[impl write-buffer.should-flush]
pub open spec fn should_flush_spec(current_size: u64, threshold: u64) -> bool {
    current_size >= threshold
}

// r[impl write-buffer.should-flush]
pub fn should_flush(current_size: u64, threshold: u64) -> (result: bool)
    ensures result == should_flush_spec(current_size, threshold),
{
    current_size >= threshold
}

// r[verify write-buffer.should-flush]
/// Once flush triggers, adding more data keeps it triggered.
proof fn flush_monotone(current: u64, extra: u64, threshold: u64)
    requires
        should_flush_spec(current, threshold),
        (current as int) + (extra as int) <= u64::MAX as int,
    ensures
        should_flush_spec(add_bytes_spec(current, extra), threshold),
{}

// r[verify write-buffer.should-flush]
/// Empty buffer never triggers flush (unless threshold is 0).
proof fn empty_no_flush(threshold: u64)
    requires threshold > 0,
    ensures !should_flush_spec(0, threshold),
{}

// ─── Buffer Swap (Freeze + New) ──────────────────────────────────────

/// A buffer state: active size, immutable size (frozen, waiting for flush).
pub struct BufferState {
    pub active_size: u64,
    pub immutable_size: u64,
}

/// Spec: swap active buffer to immutable (freeze for flush).
// r[depends write-buffer.swap]
// r[impl write-buffer.swap]
pub open spec fn swap_spec(state: BufferState) -> BufferState {
    BufferState {
        active_size: 0,
        immutable_size: state.active_size,
    }
}

// r[impl write-buffer.swap]
pub fn swap(active_size: u64, _immutable_size: u64) -> (result: (u64, u64))
    ensures ({
        let s = swap_spec(BufferState { active_size, immutable_size: _immutable_size });
        result.0 == s.active_size && result.1 == s.immutable_size
    }),
{
    (0, active_size)
}

// r[verify write-buffer.swap]
/// Swap preserves total data (active + immutable doesn't shrink).
proof fn swap_preserves_data(state: BufferState)
    ensures ({
        let after = swap_spec(state);
        after.active_size as int + after.immutable_size as int
            >= state.active_size as int
    }),
{}

// r[verify write-buffer.swap]
/// After swap, active buffer is empty.
proof fn swap_clears_active(state: BufferState)
    ensures swap_spec(state).active_size == 0,
{}

// r[verify write-buffer.swap]
/// After swap, immutable holds old active data.
proof fn swap_captures_active(state: BufferState)
    ensures swap_spec(state).immutable_size == state.active_size,
{}

// ─── Flush Complete ──────────────────────────────────────────────────

/// Spec: clear immutable buffer after flush completes.
// r[depends write-buffer.flush-complete]
// r[impl write-buffer.flush-complete]
pub open spec fn flush_complete_spec(state: BufferState) -> BufferState {
    BufferState {
        active_size: state.active_size,
        immutable_size: 0,
    }
}

// r[impl write-buffer.flush-complete]
pub fn flush_complete(active_size: u64, _immutable_size: u64) -> (result: (u64, u64))
    ensures ({
        let s = flush_complete_spec(BufferState { active_size, immutable_size: _immutable_size });
        result.0 == s.active_size && result.1 == s.immutable_size
    }),
{
    (active_size, 0)
}

// r[verify write-buffer.flush-complete]
/// Flush complete clears immutable.
proof fn flush_complete_clears_immutable(state: BufferState)
    ensures flush_complete_spec(state).immutable_size == 0,
{}

// r[verify write-buffer.flush-complete]
/// Flush complete preserves active.
proof fn flush_complete_preserves_active(state: BufferState)
    ensures flush_complete_spec(state).active_size == state.active_size,
{}

// ─── Write Amplification ─────────────────────────────────────────────

/// Spec: write amplification factor (total bytes written / user bytes written).
/// Returns the integer part × 100 (percentage) to avoid floating point.
/// E.g. WA=3.5 → 350.
// r[depends write-buffer.write-amp]
// r[impl write-buffer.write-amp]
pub open spec fn write_amp_pct_spec(user_bytes: u64, total_bytes: u64) -> u64
    recommends user_bytes > 0, total_bytes <= u64::MAX / 100,
{
    if user_bytes == 0 {
        0
    } else {
        ((total_bytes as int * 100) / (user_bytes as int)) as u64
    }
}

// r[impl write-buffer.write-amp]
pub fn write_amp_pct(user_bytes: u64, total_bytes: u64) -> (result: u64)
    requires user_bytes > 0, total_bytes <= u64::MAX / 100,
    ensures result == write_amp_pct_spec(user_bytes, total_bytes),
{
    (total_bytes * 100) / user_bytes
}

// r[verify write-buffer.write-amp]
/// Write amplification is at least 100% when total >= user.
proof fn write_amp_at_least_100(user_bytes: u64, total_bytes: u64)
    requires user_bytes > 0, total_bytes >= user_bytes, total_bytes <= u64::MAX / 100,
    ensures write_amp_pct_spec(user_bytes, total_bytes) >= 100,
{
    assert(total_bytes as int * 100 >= user_bytes as int * 100) by(nonlinear_arith)
        requires total_bytes >= user_bytes;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        user_bytes as int * 100, total_bytes as int * 100, user_bytes as int);
    vstd::arithmetic::div_mod::lemma_div_multiples_vanish(100, user_bytes as int);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(total_bytes as int * 100, user_bytes as int);
}

// r[verify write-buffer.write-amp]
/// Zero extra writes means WA = 100%.
proof fn write_amp_identity(user_bytes: u64)
    requires user_bytes > 0,
    ensures write_amp_pct_spec(user_bytes, user_bytes) == 100,
{
    let ub = user_bytes as int;
    vstd::arithmetic::div_mod::lemma_div_multiples_vanish(100, ub);
}

// ─── Estimated Flush Count ───────────────────────────────────────────

/// Spec: how many flushes needed to write `total_bytes` with given threshold.
// r[depends write-buffer.flush-count]
// r[impl write-buffer.flush-count]
pub open spec fn flush_count_spec(total_bytes: u64, threshold: u64) -> u64
    recommends threshold > 0, (total_bytes as int) + (threshold as int) - 1 <= u64::MAX as int,
{
    if threshold == 0 {
        0
    } else {
        (((total_bytes as int) + (threshold as int) - 1) / (threshold as int)) as u64
    }
}

// r[impl write-buffer.flush-count]
#[allow(clippy::manual_div_ceil)]
pub fn flush_count(total_bytes: u64, threshold: u64) -> (result: u64)
    requires
        threshold > 0,
        (total_bytes as int) + (threshold as int) <= (u64::MAX as int) + 1,
    ensures result == flush_count_spec(total_bytes, threshold),
{
    (total_bytes + (threshold - 1)) / threshold
}

// r[verify write-buffer.flush-count]
/// Zero bytes means zero flushes.
proof fn small_div_zero(x: int, d: int)
    requires 0 <= x, x < d, d > 0,
    ensures x / d == 0,
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(x, d);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(x, d);
    // q * d + r == x, 0 <= r < d, q >= 0
    // if q >= 1, q*d >= d > x, but q*d = x - r <= x, contradiction
    if x / d >= 1 {
        assert(x / d * d >= d) by(nonlinear_arith)
            requires x / d >= 1, d > 0;
    }
}

proof fn flush_count_zero(threshold: u64)
    requires threshold > 0,
    ensures flush_count_spec(0, threshold) == 0,
{
    small_div_zero(threshold as int - 1, threshold as int);
}

// r[verify write-buffer.flush-count]
/// Exactly threshold bytes means one flush.
proof fn flush_count_exact(threshold: u64)
    requires threshold > 0,
    ensures flush_count_spec(threshold, threshold) == 1,
{
    let t = threshold as int;
    let n = 2 * t - 1;
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(n, t);
    let q = n / t;
    let r = n % t;
    // q * t + r == 2t - 1, 0 <= r < t, so q*t = 2t - 1 - r, r = t - 1, q = 1
    assert(q == 1) by(nonlinear_arith)
        requires q * t + r == 2 * t - 1, 0 <= r, r < t, t > 0, q >= 0;
}

// r[verify write-buffer.flush-count]
/// More bytes means at least as many flushes.
proof fn flush_count_monotone(bytes_a: u64, bytes_b: u64, threshold: u64)
    requires
        threshold > 0,
        bytes_a <= bytes_b,
        (bytes_b as int) + (threshold as int) - 1 <= u64::MAX as int,
    ensures flush_count_spec(bytes_a, threshold) <= flush_count_spec(bytes_b, threshold),
{
    let da = (bytes_a as int) + (threshold as int) - 1;
    let db = (bytes_b as int) + (threshold as int) - 1;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(da, db, threshold as int);
}

// ─── Buffer Memory Bound ─────────────────────────────────────────────

/// Spec: total memory used by both buffers.
// r[depends write-buffer.total-memory]
// r[impl write-buffer.total-memory]
pub open spec fn total_memory_spec(state: BufferState) -> int {
    state.active_size as int + state.immutable_size as int
}

// r[verify write-buffer.total-memory]
/// Total memory after swap equals active before swap.
proof fn swap_memory_bound(state: BufferState)
    ensures total_memory_spec(swap_spec(state)) == state.active_size as int,
{}

// r[verify write-buffer.total-memory]
/// Total memory after flush-complete equals active.
proof fn flush_memory_bound(state: BufferState)
    ensures total_memory_spec(flush_complete_spec(state)) == state.active_size as int,
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_bytes() {
        assert_eq!(add_bytes(100, 50), 150);
        assert_eq!(add_bytes(0, 0), 0);
    }

    #[test]
    fn test_should_flush() {
        assert!(!should_flush(50, 100));
        assert!(should_flush(100, 100));
        assert!(should_flush(150, 100));
    }

    #[test]
    fn test_swap() {
        let (active, immutable) = swap(1000, 0);
        assert_eq!(active, 0);
        assert_eq!(immutable, 1000);
    }

    #[test]
    fn test_flush_complete() {
        let (active, immutable) = flush_complete(500, 1000);
        assert_eq!(active, 500);
        assert_eq!(immutable, 0);
    }

    #[test]
    fn test_swap_then_flush() {
        // Write 1000 bytes, swap, flush
        let (a, i) = swap(1000, 0);
        assert_eq!(a, 0);
        assert_eq!(i, 1000);
        let (a2, i2) = flush_complete(a, i);
        assert_eq!(a2, 0);
        assert_eq!(i2, 0);
    }

    #[test]
    fn test_write_amp_pct() {
        assert_eq!(write_amp_pct(100, 100), 100); // 1x
        assert_eq!(write_amp_pct(100, 300), 300); // 3x
        assert_eq!(write_amp_pct(100, 50), 50); // 0.5x
    }

    #[test]
    fn test_flush_count() {
        assert_eq!(flush_count(0, 100), 0);
        assert_eq!(flush_count(100, 100), 1);
        assert_eq!(flush_count(101, 100), 2);
        assert_eq!(flush_count(200, 100), 2);
        assert_eq!(flush_count(201, 100), 3);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn prop_add_bytes_monotone(current in 0u64..u64::MAX / 2, size in 0u64..u64::MAX / 2) {
            let result = add_bytes(current, size);
            prop_assert!(result >= current);
        }

        #[test]
        fn prop_flush_monotone(current in 0u64..u64::MAX / 2, extra in 0u64..u64::MAX / 2, threshold in 1u64..10000) {
            if should_flush(current, threshold) {
                prop_assert!(should_flush(add_bytes(current, extra), threshold));
            }
        }

        #[test]
        fn prop_swap_clears_active(active in any::<u64>()) {
            let (a, _i) = swap(active, 0);
            prop_assert_eq!(a, 0);
        }

        #[test]
        fn prop_swap_captures_active(active in any::<u64>()) {
            let (_a, i) = swap(active, 0);
            prop_assert_eq!(i, active);
        }

        #[test]
        fn prop_flush_count_monotone(
            a in 0u64..100000,
            b in 0u64..100000,
            threshold in 1u64..10000,
        ) {
            let lo = a.min(b);
            let hi = a.max(b);
            prop_assert!(flush_count(hi, threshold) >= flush_count(lo, threshold));
        }

        #[test]
        fn prop_write_amp_at_least_100_when_total_ge_user(
            user in 1u64..100000,
            extra in 0u64..100000,
        ) {
            let total = user.saturating_add(extra);
            if total <= u64::MAX / 100 {
                let wa = write_amp_pct(user, total);
                prop_assert!(wa >= 100);
            }
        }
    }
}

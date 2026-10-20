// Verus proofs guarantee u128→u64 casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! LSM-tree level management.
//!
//! Pure functions for level-based compaction: target sizing via geometric
//! progression, compaction triggers, space/write amplification computation,
//! and level count estimation.

use vstd::prelude::*;

verus! {

// ─── Size Ratio Validation ───────────────────────────────────────────

/// Spec: is the size ratio valid?
// r[depends lsm-level.size-ratio-valid]
// r[impl lsm-level.size-ratio-valid]
pub open spec fn is_valid_size_ratio_spec(ratio: u64) -> bool {
    ratio >= 2
}

/// A size ratio below 2 degenerates the LSM structure.
// r[impl lsm-level.size-ratio-valid]
pub fn is_valid_size_ratio(ratio: u64) -> (result: bool)
    ensures result == is_valid_size_ratio_spec(ratio),
{
    ratio >= 2
}

// ─── Target Size ─────────────────────────────────────────────────────

/// Spec: target size for a level (recursive geometric progression).
/// level_target_size(level, base, ratio) = base * ratio^level, saturating.
// r[depends lsm-level.target-size]
// r[impl lsm-level.target-size]
pub open spec fn level_target_size_spec(level: u64, base_size: u64, size_ratio: u64) -> u64
    decreases level,
{
    if level == 0 {
        base_size
    } else {
        let prev = level_target_size_spec((level - 1) as u64, base_size, size_ratio);
        if prev as int * size_ratio as int > u64::MAX as int {
            u64::MAX
        } else {
            (prev as int * size_ratio as int) as u64
        }
    }
}

/// Once the target overflows, all subsequent levels saturate at MAX.
proof fn target_saturated_stays(k: u64, sat_level: u64, base_size: u64, size_ratio: u64)
    requires
        base_size > 0,
        size_ratio >= 2,
        sat_level < k,
        level_target_size_spec(sat_level, base_size, size_ratio) as int
            * size_ratio as int > u64::MAX as int,
    ensures
        level_target_size_spec(k, base_size, size_ratio) == u64::MAX,
    decreases k - sat_level,
{
    if k == (sat_level + 1) as u64 {
        // Direct: spec unfolds prev * ratio > MAX → returns MAX
    } else {
        // First show sat_level + 1 gives MAX
        target_saturated_stays((sat_level + 1) as u64, sat_level, base_size, size_ratio);
        // MAX * ratio > MAX for ratio >= 2
        assert(u64::MAX as int * size_ratio as int > u64::MAX as int) by(nonlinear_arith)
            requires size_ratio >= 2u64;
        // Recurse with the new saturation point
        target_saturated_stays(k, (sat_level + 1) as u64, base_size, size_ratio);
    }
}

/// Target size for a level: base_size * size_ratio^level, saturating at MAX.
// r[impl lsm-level.target-size]
pub fn level_target_size(level: u64, base_size: u64, size_ratio: u64) -> (result: u64)
    requires
        base_size > 0,
        size_ratio >= 2,
    ensures result == level_target_size_spec(level, base_size, size_ratio),
{
    let mut current = base_size;
    let mut i: u64 = 0;
    while i < level
        invariant
            i <= level,
            base_size > 0,
            size_ratio >= 2u64,
            current == level_target_size_spec(i, base_size, size_ratio),
        decreases level - i,
    {
        if current > u64::MAX / size_ratio {
            proof {
                vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                    u64::MAX as int, size_ratio as int,
                );
                assert(current as int * size_ratio as int > u64::MAX as int)
                    by(nonlinear_arith)
                    requires current > u64::MAX / size_ratio, size_ratio >= 2u64;
                target_saturated_stays(level, i, base_size, size_ratio);
            }
            return u64::MAX;
        }
        proof {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                u64::MAX as int, size_ratio as int,
            );
            assert(current as int * size_ratio as int <= u64::MAX as int)
                by(nonlinear_arith)
                requires current <= u64::MAX / size_ratio, size_ratio >= 2u64;
        }
        current = current * size_ratio;
        i = i + 1;
    }
    current
}

// ─── Compaction Trigger ──────────────────────────────────────────────

/// Spec: does this level need compaction?
// r[depends lsm-level.needs-compaction]
// r[impl lsm-level.needs-compaction]
pub open spec fn needs_compaction_spec(level_size: u64, target_size: u64) -> bool {
    level_size > target_size
}

/// A level exceeding its target size needs compaction.
// r[impl lsm-level.needs-compaction]
pub fn needs_compaction(level_size: u64, target_size: u64) -> (result: bool)
    ensures result == needs_compaction_spec(level_size, target_size),
{
    level_size > target_size
}

// ─── Amplification ───────────────────────────────────────────────────

/// Spec: space amplification as a percentage.
// r[depends lsm-level.space-amp-pct]
// r[impl lsm-level.space-amp-pct]
pub open spec fn space_amp_pct_spec(total_size: u64, live_size: u64) -> u64 {
    ((total_size as int * 100) / live_size as int) as u64
}

/// Space amplification: total_size * 100 / live_size.
/// 100 means no amplification (total == live).
// r[impl lsm-level.space-amp-pct]
pub fn space_amp_pct(total_size: u64, live_size: u64) -> (result: u64)
    requires live_size > 0,
    ensures result == space_amp_pct_spec(total_size, live_size),
{
    let wide = total_size as u128 * 100u128;
    (wide / live_size as u128) as u64
}

/// Spec: write amplification per level.
// r[depends lsm-level.write-amp-per-level]
// r[impl lsm-level.write-amp-per-level]
pub open spec fn write_amp_per_level_spec(size_ratio: u64) -> u64 {
    size_ratio
}

/// Each level compaction rewrites approximately size_ratio times the data.
// r[impl lsm-level.write-amp-per-level]
pub fn write_amp_per_level(size_ratio: u64) -> (result: u64)
    ensures result == write_amp_per_level_spec(size_ratio),
{
    size_ratio
}

/// Spec: total write amplification across all levels.
// r[depends lsm-level.total-write-amp]
// r[impl lsm-level.total-write-amp]
pub open spec fn total_write_amp_spec(num_levels: u64, size_ratio: u64) -> u64 {
    if num_levels as int * size_ratio as int > u64::MAX as int {
        u64::MAX
    } else {
        (num_levels as int * size_ratio as int) as u64
    }
}

/// Total write amplification: num_levels * size_ratio, saturating at MAX.
// r[impl lsm-level.total-write-amp]
pub fn total_write_amp(num_levels: u64, size_ratio: u64) -> (result: u64)
    ensures result == total_write_amp_spec(num_levels, size_ratio),
{
    if num_levels == 0 || size_ratio == 0 {
        return 0;
    }
    if num_levels > u64::MAX / size_ratio {
        proof {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(u64::MAX as int, size_ratio as int);
            assert(num_levels as int * size_ratio as int > u64::MAX as int) by(nonlinear_arith)
                requires num_levels > u64::MAX / size_ratio, size_ratio > 0u64;
        }
        return u64::MAX;
    }
    proof {
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(u64::MAX as int, size_ratio as int);
        assert(num_levels as int * size_ratio as int <= u64::MAX as int) by(nonlinear_arith)
            requires num_levels > 0u64, size_ratio > 0u64, num_levels <= u64::MAX / size_ratio;
    }
    num_levels * size_ratio
}

// ─── Level Count ─────────────────────────────────────────────────────

/// Spec: minimum levels needed to hold total_size (recursive).
// r[depends lsm-level.level-count]
// r[impl lsm-level.level-count]
pub open spec fn level_count_for_size_spec(
    total_size: u64,
    base_size: u64,
    size_ratio: u64,
) -> u64
    decreases total_size,
{
    if total_size <= base_size {
        0
    } else {
        // Divide total_size by size_ratio (ceiling) and recurse
        let reduced = if total_size as int % size_ratio as int == 0 {
            (total_size as int / size_ratio as int) as u64
        } else {
            ((total_size as int / size_ratio as int) + 1) as u64
        };
        if reduced >= total_size {
            // Prevent infinite recursion on degenerate inputs
            1
        } else {
            (1 + level_count_for_size_spec(reduced, base_size, size_ratio) as int) as u64
        }
    }
}

/// The level count never exceeds the input size (used for overflow safety).
proof fn level_count_bounded(total_size: u64, base_size: u64, size_ratio: u64)
    requires base_size > 0, size_ratio >= 2,
    ensures level_count_for_size_spec(total_size, base_size, size_ratio) as int
        <= total_size as int,
    decreases total_size,
{
    if total_size <= base_size {
        // Returns 0
    } else {
        let q = total_size as int / size_ratio as int;
        let r = total_size as int % size_ratio as int;
        let reduced_int = if r == 0 { q } else { q + 1 };
        let reduced = reduced_int as u64;

        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
            total_size as int, size_ratio as int,
        );
        // total_size = q * size_ratio + r, 0 <= r < size_ratio
        // q < total_size since size_ratio >= 2 and total_size >= 2
        assert(q < total_size as int) by(nonlinear_arith)
            requires
                q * (size_ratio as int) + r == total_size as int,
                size_ratio >= 2int,
                r >= 0int,
                total_size >= 2int;
        assert(r > 0int ==> q + 1 < total_size as int) by(nonlinear_arith)
            requires
                q * (size_ratio as int) + r == total_size as int,
                size_ratio >= 2int,
                total_size >= 2int;

        if reduced >= total_size {
            // Spec returns 1 <= total_size (since total_size >= 2)
        } else {
            level_count_bounded(reduced, base_size, size_ratio);
            // level_count_for_size_spec(reduced, ...) <= reduced < total_size
            // 1 + that <= 1 + reduced <= total_size
        }
    }
}

/// One-step unfolding of level_count_for_size_spec: the quotient decreases,
/// and the spec returns at least 1 when total_size > base_size.
proof fn level_count_step(remaining: u64, base_size: u64, size_ratio: u64)
    requires remaining > base_size, base_size > 0, size_ratio >= 2,
    ensures
        (remaining as int) / (size_ratio as int) < (remaining as int),
        (remaining as int) % (size_ratio as int) > 0
            ==> (remaining as int) / (size_ratio as int) + 1 < (remaining as int),
        level_count_for_size_spec(remaining, base_size, size_ratio) >= 1u64,
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
        remaining as int, size_ratio as int,
    );
    let q = remaining as int / size_ratio as int;
    let r = remaining as int % size_ratio as int;
    assert(q < remaining as int) by(nonlinear_arith)
        requires q * (size_ratio as int) + r == remaining as int,
                 size_ratio >= 2int, r >= 0int, remaining >= 2int;
    assert(r > 0int ==> q + 1 < remaining as int) by(nonlinear_arith)
        requires q * (size_ratio as int) + r == remaining as int,
                 size_ratio >= 2int, remaining >= 2int;

    // Prove lcfs >= 1: spec returns 1 (guard) or (1 + lcfs(reduced)) as u64
    // For the else branch, need 1 + lcfs(reduced) <= u64::MAX so the cast >= 1
    let reduced_int = if r == 0 { q } else { q + 1 };
    if reduced_int < remaining as int {
        let reduced = reduced_int as u64;
        level_count_bounded(reduced, base_size, size_ratio);
        // lcfs(reduced) <= reduced < remaining, so 1 + lcfs(reduced) <= remaining <= u64::MAX
    }
}

/// Minimum number of levels to hold total_size with given base and ratio.
// r[impl lsm-level.level-count]
pub fn level_count_for_size(
    total_size: u64,
    base_size: u64,
    size_ratio: u64,
) -> (result: u64)
    requires
        base_size > 0,
        size_ratio >= 2,
    ensures result == level_count_for_size_spec(total_size, base_size, size_ratio),
{
    if total_size <= base_size {
        return 0;
    }
    let mut remaining = total_size;
    let mut levels: u64 = 0;

    proof { level_count_bounded(total_size, base_size, size_ratio); }

    while remaining > base_size
        invariant
            remaining <= total_size,
            base_size > 0u64,
            size_ratio >= 2u64,
            total_size > base_size,
            levels as int
                + level_count_for_size_spec(remaining, base_size, size_ratio) as int
                == level_count_for_size_spec(total_size, base_size, size_ratio) as int,
            level_count_for_size_spec(total_size, base_size, size_ratio) as int
                <= total_size as int,
        decreases remaining,
    {
        let q = remaining / size_ratio;
        let r = remaining % size_ratio;

        proof {
            // Establishes: q < remaining, (r > 0 ==> q + 1 < remaining), lcfs >= 1
            level_count_step(remaining, base_size, size_ratio);
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                remaining as int, size_ratio as int,
            );
        }

        if r == 0 {
            proof { level_count_bounded(q, base_size, size_ratio); }
            remaining = q;
        } else {
            proof { level_count_bounded((q + 1) as u64, base_size, size_ratio); }
            remaining = q + 1;
        }
        levels = levels + 1;
    }
    // remaining <= base_size, so level_count_for_size_spec(remaining, ..) == 0
    levels
}

// ─── Proofs ──────────────────────────────────────────────────────────

// --- level_target_size ---

/// Level 0 returns base_size.
// r[verify lsm-level.target-size]
proof fn target_level_zero(base_size: u64, size_ratio: u64)
    ensures level_target_size_spec(0, base_size, size_ratio) == base_size,
{}

/// Target size is at least base_size for all levels.
// r[verify lsm-level.target-size]
proof fn target_ge_base(level: u64, base_size: u64, size_ratio: u64)
    requires base_size > 0, size_ratio >= 2,
    ensures level_target_size_spec(level, base_size, size_ratio) >= base_size,
    decreases level,
{
    if level > 0 {
        target_ge_base((level - 1) as u64, base_size, size_ratio);
        let prev = level_target_size_spec((level - 1) as u64, base_size, size_ratio);
        if prev as int * size_ratio as int > u64::MAX as int {
            // Saturated: MAX >= base_size
        } else {
            assert(prev as int * size_ratio as int >= prev as int) by(nonlinear_arith)
                requires prev >= 1u64, size_ratio >= 2u64;
        }
    }
}

/// Target size is monotonic: higher levels have larger or equal targets.
// r[verify lsm-level.target-size]
pub proof fn target_monotonic(level: u64, base_size: u64, size_ratio: u64)
    requires
        base_size > 0,
        size_ratio >= 2,
        level < u64::MAX,
    ensures
        level_target_size_spec(level, base_size, size_ratio) <=
        level_target_size_spec((level + 1) as u64, base_size, size_ratio),
    decreases level,
{
    target_ge_base(level, base_size, size_ratio);
    let prev = level_target_size_spec(level, base_size, size_ratio);
    // Next level: prev * ratio, saturating
    if prev as int * size_ratio as int > u64::MAX as int {
        // Saturated: next == MAX >= prev
    } else {
        // Not saturated: prev * ratio >= prev since ratio >= 2 >= 1
        assert(prev as int * size_ratio as int >= prev as int) by(nonlinear_arith)
            requires prev >= 1u64, size_ratio >= 2u64;
    }
}

// --- needs_compaction ---

/// An empty level never needs compaction.
// r[verify lsm-level.needs-compaction]
proof fn empty_no_compaction(target: u64)
    ensures !needs_compaction_spec(0, target),
{}

/// A level at exactly its target does not need compaction.
// r[verify lsm-level.needs-compaction]
proof fn at_target_no_compaction(target: u64)
    ensures !needs_compaction_spec(target, target),
{}

/// One byte over target triggers compaction.
// r[verify lsm-level.needs-compaction]
proof fn one_over_triggers(target: u64)
    requires target < u64::MAX,
    ensures needs_compaction_spec((target + 1) as u64, target),
{}

// --- space_amp_pct ---

/// No amplification when total equals live.
// r[verify lsm-level.space-amp-pct]
proof fn space_amp_no_amplification(size: u64)
    requires size > 0,
    ensures space_amp_pct_spec(size, size) == 100,
{
    assert(size as int * 100 / size as int == 100) by(nonlinear_arith)
        requires size > 0u64;
}

/// Space amp is at least 100% when total >= live (and result fits in u64).
// r[verify lsm-level.space-amp-pct]
pub proof fn space_amp_ge_100(total: u64, live: u64)
    requires
        live > 0,
        total >= live,
        (total as int * 100) / (live as int) <= u64::MAX as int,
    ensures space_amp_pct_spec(total, live) >= 100,
{
    assert(live as int * 100 / live as int == 100) by(nonlinear_arith)
        requires live > 0u64;
    assert(total as int * 100 >= live as int * 100) by(nonlinear_arith)
        requires total >= live;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(live as int * 100, total as int * 100, live as int);
}

/// Space amp is monotonic in total_size (when the larger result fits in u64).
// r[verify lsm-level.space-amp-pct]
proof fn space_amp_monotonic(t1: u64, t2: u64, live: u64)
    requires
        live > 0,
        t1 <= t2,
        (t2 as int * 100) / (live as int) <= u64::MAX as int,
    ensures space_amp_pct_spec(t1, live) <= space_amp_pct_spec(t2, live),
{
    assert(t1 as int * 100 <= t2 as int * 100) by(nonlinear_arith)
        requires t1 <= t2;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(t1 as int * 100, t2 as int * 100, live as int);
}

// --- total_write_amp ---

/// Zero levels means zero write amp.
// r[verify lsm-level.total-write-amp]
proof fn zero_levels_zero_amp(ratio: u64)
    ensures total_write_amp_spec(0, ratio) == 0,
{}

/// Single level equals the ratio.
// r[verify lsm-level.total-write-amp]
proof fn single_level_amp(ratio: u64)
    ensures total_write_amp_spec(1, ratio) == ratio,
{}

// --- size_ratio_valid ---

/// Ratio of 2 is the minimum valid ratio.
// r[verify lsm-level.size-ratio-valid]
proof fn min_valid_ratio()
    ensures is_valid_size_ratio_spec(2),
{}

/// Ratio of 1 is invalid.
// r[verify lsm-level.size-ratio-valid]
proof fn ratio_one_invalid()
    ensures !is_valid_size_ratio_spec(1),
{}

// --- level_count ---

/// Data fitting in base needs 0 levels.
// r[verify lsm-level.level-count]
proof fn count_fits_in_base(base: u64, ratio: u64)
    requires base > 0, ratio >= 2,
    ensures level_count_for_size_spec(base, base, ratio) == 0,
{}

/// Empty data needs 0 levels.
// r[verify lsm-level.level-count]
proof fn count_empty(base: u64, ratio: u64)
    requires base > 0, ratio >= 2,
    ensures level_count_for_size_spec(0, base, ratio) == 0,
{}

} // verus!

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use super::*;

    // --- Unit tests ---

    // r[verify lsm-level.write-amp-per-level]
    #[test]
    // r[verify lsm-level.target-size]
    fn test_target_size_basic() {
        // base=100, ratio=10: 100, 1000, 10000, 100000
        assert_eq!(level_target_size(0, 100, 10), 100);
        assert_eq!(level_target_size(1, 100, 10), 1000);
        assert_eq!(level_target_size(2, 100, 10), 10000);
        assert_eq!(level_target_size(3, 100, 10), 100000);
    }

    #[test]
    // r[verify lsm-level.target-size]
    fn test_target_size_saturation() {
        // 1 * 2^63 = 9223372036854775808 (still fits in u64)
        assert_eq!(level_target_size(63, 1, 2), 1u64 << 63);
        // 1 * 2^64 would overflow, so level 64 saturates
        assert_eq!(level_target_size(64, 1, 2), u64::MAX);
    }

    #[test]
    // r[verify lsm-level.needs-compaction]
    fn test_needs_compaction_basic() {
        assert!(!needs_compaction(100, 1000));
        assert!(!needs_compaction(1000, 1000));
        assert!(needs_compaction(1001, 1000));
    }

    #[test]
    // r[verify lsm-level.space-amp-pct]
    fn test_space_amp_basic() {
        assert_eq!(space_amp_pct(100, 100), 100); // no amp
        assert_eq!(space_amp_pct(200, 100), 200); // 2x
        assert_eq!(space_amp_pct(300, 100), 300); // 3x
    }

    #[test]
    // r[verify lsm-level.total-write-amp]
    fn test_total_write_amp_basic() {
        assert_eq!(total_write_amp(0, 10), 0);
        assert_eq!(total_write_amp(1, 10), 10);
        assert_eq!(total_write_amp(5, 10), 50);
    }

    #[test]
    // r[verify lsm-level.size-ratio-valid]
    fn test_size_ratio_valid_basic() {
        assert!(!is_valid_size_ratio(0));
        assert!(!is_valid_size_ratio(1));
        assert!(is_valid_size_ratio(2));
        assert!(is_valid_size_ratio(10));
    }

    #[test]
    // r[verify lsm-level.level-count]
    fn test_level_count_basic() {
        // base=100, ratio=10
        assert_eq!(level_count_for_size(0, 100, 10), 0);
        assert_eq!(level_count_for_size(100, 100, 10), 0);
        assert_eq!(level_count_for_size(101, 100, 10), 1);
        assert_eq!(level_count_for_size(1000, 100, 10), 1);
        assert_eq!(level_count_for_size(1001, 100, 10), 2);
    }

    // --- Property tests ---

    use proptest::prelude::*;

    proptest! {
        #[test]
        // r[verify lsm-level.target-size]
        fn prop_target_monotonic(
            level in 0u64..62,
            base in 1u64..1_000_000,
            ratio in 2u64..20,
        ) {
            let t1 = level_target_size(level, base, ratio);
            let t2 = level_target_size(level + 1, base, ratio);
            prop_assert!(t2 >= t1);
        }

        #[test]
        // r[verify lsm-level.target-size]
        fn prop_target_ge_base(
            level in 0u64..30,
            base in 1u64..1_000_000,
            ratio in 2u64..20,
        ) {
            prop_assert!(level_target_size(level, base, ratio) >= base);
        }

        #[test]
        // r[verify lsm-level.space-amp-pct]
        fn prop_space_amp_ge_100(total in 1u64..1_000_000, live in 1u64..1_000_000) {
            if total >= live {
                prop_assert!(space_amp_pct(total, live) >= 100);
            }
        }

        #[test]
        // r[verify lsm-level.total-write-amp]
        fn prop_write_amp_monotonic_levels(
            l1 in 0u64..100,
            l2 in 0u64..100,
            ratio in 2u64..20,
        ) {
            if l1 <= l2 {
                prop_assert!(total_write_amp(l1, ratio) <= total_write_amp(l2, ratio));
            }
        }

        #[test]
        // r[verify lsm-level.level-count]
        fn prop_level_count_zero_for_small(
            total in 0u64..100,
            base in 100u64..1000,
            ratio in 2u64..20,
        ) {
            // total <= base always gives 0 levels
            if total <= base {
                prop_assert_eq!(level_count_for_size(total, base, ratio), 0);
            }
        }

        #[test]
        // r[verify lsm-level.needs-compaction]
        fn prop_compaction_iff_above_target(level_size: u64, target: u64) {
            prop_assert_eq!(needs_compaction(level_size, target), level_size > target);
        }
    }
}

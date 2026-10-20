//! Verified cron field matching.
//!
//! A cron field is a (min, max, step) triple. A value matches when
//! `min <= value <= max` and `(value - min) % step == 0`. Wildcard `*`
//! is (field_min, field_max, 1). Step `*/N` is (field_min, field_max, N).
//! Range `A-B` is (A, B, 1). Range with step `A-B/N` is (A, B, N).

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_mod_multiples_basic;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_mod_pos_bound;
use vstd::prelude::*;

verus! {

// ============================================================
// match_field
// ============================================================

// r[depends cron-field.match-field]
/// Spec: does `value` match the cron field (min, max, step)?
/// True when value is in [min, max] and on a step boundary from min.
pub open spec fn match_field_spec(value: u64, min: u64, max: u64, step: u64) -> bool {
    min <= value && value <= max
    && step >= 1
    && (value - min) % (step as int) == 0
}

// r[impl cron-field.match-field]
/// Returns true if `value` matches the cron field (min, max, step).
pub fn match_field(value: u64, min: u64, max: u64, step: u64) -> (result: bool)
    requires step >= 1
    ensures result == match_field_spec(value, min, max, step)
{
    if value < min || value > max {
        return false;
    }
    (value - min) % step == 0
}

// ============================================================
// is_valid_field
// ============================================================

// r[depends cron-field.valid-field]
/// Spec: is the field (min, max, step) valid within [field_min, field_max]?
pub open spec fn is_valid_field_spec(
    min: u64, max: u64, step: u64, field_min: u64, field_max: u64,
) -> bool {
    field_min <= min
    && min <= max
    && max <= field_max
    && step >= 1
}

// r[impl cron-field.valid-field]
/// Returns true if the cron field bounds are valid.
pub fn is_valid_field(
    min: u64, max: u64, step: u64, field_min: u64, field_max: u64,
) -> (result: bool)
    ensures result == is_valid_field_spec(min, max, step, field_min, field_max)
{
    field_min <= min
    && min <= max
    && max <= field_max
    && step >= 1
}

// ============================================================
// first_match — first matching value in a field
// ============================================================

// r[depends cron-field.match-field]
/// Spec: the first value >= `from` that matches the field, or max + 1 if none.
pub open spec fn first_match_spec(from: u64, min: u64, max: u64, step: u64) -> u64
    recommends step >= 1, min <= max
{
    if from > max {
        (max + 1) as u64
    } else if from <= min {
        min
    } else {
        // from is in (min, max]. Find the next step boundary >= from.
        let offset: int = from as int - min as int;
        let remainder: int = offset % step as int;
        if remainder == 0 {
            from
        } else {
            let next: int = from as int + (step as int - remainder);
            if next > max as int {
                (max + 1) as u64
            } else {
                next as u64
            }
        }
    }
}

// r[impl cron-field.match-field]
/// Returns the first matching value >= `from` in the field, or max + 1 if none.
pub fn first_match(from: u64, min: u64, max: u64, step: u64) -> (result: u64)
    requires
        step >= 1,
        min <= max,
        (max as int) + 1 <= u64::MAX as int,
    ensures result == first_match_spec(from, min, max, step)
{
    if from > max {
        return max + 1;
    }
    if from <= min {
        return min;
    }
    let offset: u64 = from - min;
    let remainder: u64 = offset % step;
    if remainder == 0 {
        from
    } else {
        let bump: u64 = step - remainder;
        if bump > max - from {
            max + 1
        } else {
            from + bump
        }
    }
}

// ============================================================
// Proofs
// ============================================================

// r[verify cron-field.match-field]
/// Wildcard (step=1, full range) matches every value in range.
proof fn wildcard_matches_all(value: u64, field_min: u64, field_max: u64)
    requires
        field_min <= value,
        value <= field_max,
    ensures match_field_spec(value, field_min, field_max, 1)
{}

// r[verify cron-field.match-field]
/// A value below min never matches.
proof fn below_min_never_matches(value: u64, min: u64, max: u64, step: u64)
    requires value < min
    ensures !match_field_spec(value, min, max, step)
{}

// r[verify cron-field.match-field]
/// A value above max never matches.
proof fn above_max_never_matches(value: u64, min: u64, max: u64, step: u64)
    requires value > max
    ensures !match_field_spec(value, min, max, step)
{}

// r[verify cron-field.match-field]
/// Min always matches (it's on the zero-th step).
proof fn min_always_matches(min: u64, max: u64, step: u64)
    requires min <= max, step >= 1
    ensures match_field_spec(min, min, max, step)
{
    // (min - min) == 0, and 0 % step == 0 for any step >= 1
    assert((min - min) % (step as int) == 0) by(nonlinear_arith)
        requires step >= 1;
}

// r[verify cron-field.match-field]
/// Match is deterministic: same inputs always give the same result.
proof fn match_deterministic(value: u64, min: u64, max: u64, step: u64)
    ensures
        match_field_spec(value, min, max, step)
            == match_field_spec(value, min, max, step)
{}

// r[verify cron-field.valid-field]
/// Step zero is always invalid.
proof fn step_zero_invalid(min: u64, max: u64, field_min: u64, field_max: u64)
    ensures !is_valid_field_spec(min, max, 0, field_min, field_max)
{}

// r[verify cron-field.valid-field]
/// Min > max is always invalid.
proof fn min_gt_max_invalid(min: u64, max: u64, step: u64, field_min: u64, field_max: u64)
    requires min > max
    ensures !is_valid_field_spec(min, max, step, field_min, field_max)
{}

proof fn round_up_divisible(offset: int, step: int)
    requires
        step >= 1,
        offset >= 0,
    ensures ({
        let r = offset % step;
        r == 0 || (offset + step - r) % step == 0
    })
{
    let r = offset % step;
    if r != 0 {
        let q = offset / step;
        lemma_fundamental_div_mod(offset, step);
        assert(offset - r == step * q) by(nonlinear_arith)
            requires offset == step * q + r;
        assert(offset + step - r == (q + 1) * step) by(nonlinear_arith)
            requires offset - r == step * q;
        lemma_mod_multiples_basic(q + 1, step);
    }
}

// r[verify cron-field.match-field]
/// first_match result either matches the field or signals no-match (max+1).
pub proof fn first_match_correct(from: u64, min: u64, max: u64, step: u64)
    requires
        step >= 1,
        min <= max,
        (max as int) + 1 <= u64::MAX as int,
    ensures ({
        let r = first_match_spec(from, min, max, step);
        r == (max + 1) as u64 || match_field_spec(r, min, max, step)
    })
{
    let r = first_match_spec(from, min, max, step);
    if from > max {
        assert(r == (max + 1) as u64);
    } else if from <= min {
        assert(r == min);
        min_always_matches(min, max, step);
    } else {
        let offset: int = from as int - min as int;
        let remainder: int = offset % step as int;
        if remainder == 0 {
            assert(r == from);
            assert(match_field_spec(from, min, max, step));
        } else {
            let delta: int = step as int - remainder;
            let next: int = from as int + delta;
            if next > max as int {
                assert(r == (max + 1) as u64);
            } else {
                assert(r == next as u64);
                round_up_divisible(offset, step as int);
                assert(from as int == min as int + offset) by(nonlinear_arith)
                    requires offset == from as int - min as int;
                assert(next - min as int == offset + delta) by(nonlinear_arith)
                    requires from as int == min as int + offset, next == from as int + delta;
                assert(match_field_spec(next as u64, min, max, step));
            }
        }
    }
}

// r[verify cron-field.match-field]
/// first_match result is >= from (never goes backward).
pub proof fn first_match_ge_from(from: u64, min: u64, max: u64, step: u64)
    requires
        step >= 1,
        min <= max,
        (max as int) + 1 <= u64::MAX as int,
        from <= max + 1,
    ensures
        first_match_spec(from, min, max, step) >= from
            || first_match_spec(from, min, max, step) == min,
{
    let r = first_match_spec(from, min, max, step);
    if from > max {
        assert(from == max + 1);
        assert(r == (max + 1) as u64);
    } else if from <= min {
        assert(r == min);
    } else {
        let offset: int = from as int - min as int;
        let remainder: int = offset % step as int;
        if remainder == 0 {
            assert(r == from);
        } else {
            lemma_mod_pos_bound(offset, step as int);
            let next: int = from as int + (step as int - remainder);
            assert(step as int - remainder > 0) by(nonlinear_arith)
                requires 0 <= remainder < step as int, remainder != 0;
            assert(next > from as int) by(nonlinear_arith)
                requires next == from as int + (step as int - remainder), step as int - remainder > 0;
            if next > max as int {
                assert(r == (max + 1) as u64);
                assert((max + 1) >= from) by(nonlinear_arith)
                    requires from <= max;
            } else {
                assert(r == next as u64);
                assert(next as u64 >= from) by(nonlinear_arith)
                    requires next > from as int, next <= max as int, max as int <= u64::MAX as int;
            }
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify cron-field.match-field]
    #[test]
    fn test_match_field() {
        // Wildcard: */1 over [0, 59]
        assert!(match_field(35, 0, 59, 1));
        assert!(match_field(0, 0, 59, 1));
        assert!(match_field(59, 0, 59, 1));

        // Step: */15 over [0, 59]
        assert!(match_field(0, 0, 59, 15));
        assert!(match_field(15, 0, 59, 15));
        assert!(match_field(30, 0, 59, 15));
        assert!(match_field(45, 0, 59, 15));
        assert!(!match_field(7, 0, 59, 15));
        assert!(!match_field(14, 0, 59, 15));

        // Range: 2-10/3
        assert!(match_field(2, 2, 10, 3));
        assert!(match_field(5, 2, 10, 3));
        assert!(match_field(8, 2, 10, 3));
        assert!(!match_field(7, 2, 10, 3));
        assert!(!match_field(1, 2, 10, 3));
        assert!(!match_field(11, 2, 10, 3));
    }

    // r[verify cron-field.valid-field]
    #[test]
    fn test_valid_field() {
        // Valid minute field */15
        assert!(is_valid_field(0, 59, 15, 0, 59));
        // Valid hour field 3-3
        assert!(is_valid_field(3, 3, 1, 0, 23));
        // Step zero
        assert!(!is_valid_field(0, 59, 0, 0, 59));
        // Min > max
        assert!(!is_valid_field(10, 5, 1, 0, 59));
        // Exceeds field bounds
        assert!(!is_valid_field(0, 60, 1, 0, 59));
    }

    // r[verify cron-field.match-field]
    #[test]
    fn test_first_match() {
        // from <= min: returns min
        assert_eq!(first_match(0, 5, 10, 1), 5);
        // from in range, on step
        assert_eq!(first_match(6, 0, 59, 3), 6);
        // from in range, not on step
        assert_eq!(first_match(7, 0, 59, 5), 10);
        // from > max: returns max + 1
        assert_eq!(first_match(60, 0, 59, 1), 60);
        // from just before step hit
        assert_eq!(first_match(14, 0, 59, 15), 15);
        // No match in range (from + bump > max)
        assert_eq!(first_match(9, 0, 10, 5), 10);
    }

    proptest! {
        // r[verify cron-field.match-field]
        #[test]
        fn prop_min_always_matches(
            min in 0u64..100,
            range in 0u64..100,
            step in 1u64..50,
        ) {
            let max = min + range;
            prop_assert!(match_field(min, min, max, step));
        }

        // r[verify cron-field.match-field]
        #[test]
        fn prop_wildcard_matches_all(value in 0u64..60) {
            prop_assert!(match_field(value, 0, 59, 1));
        }

        // r[verify cron-field.match-field]
        #[test]
        fn prop_below_min_never_matches(
            min in 1u64..100,
            max in 0u64..100,
            step in 1u64..50,
        ) {
            if min <= max {
                prop_assert!(!match_field(min - 1, min, max, step));
            }
        }

        // r[verify cron-field.match-field]
        #[test]
        fn prop_above_max_never_matches(
            max in 0u64..100,
            step in 1u64..50,
        ) {
            prop_assert!(!match_field(max + 1, 0, max, step));
        }

        // r[verify cron-field.match-field]
        #[test]
        fn prop_step_hits_are_spaced(
            min in 0u64..30,
            step in 1u64..15,
            k in 0u64..10,
        ) {
            let value = min + k * step;
            let max = min + 20 * step;
            if value <= max {
                prop_assert!(match_field(value, min, max, step));
            }
        }

        // r[verify cron-field.match-field]
        #[test]
        fn prop_first_match_is_valid_or_sentinel(
            from in 0u64..65,
            min in 0u64..60,
            max_offset in 0u64..60,
            step in 1u64..30,
        ) {
            let max = min + max_offset;
            if max < u64::MAX {
                let r = first_match(from, min, max, step);
                if r == max + 1 {
                    // No match — OK
                } else {
                    prop_assert!(match_field(r, min, max, step),
                        "first_match({}, {}, {}, {}) = {} doesn't match", from, min, max, step, r);
                    prop_assert!(r >= from || r == min,
                        "first_match went backward: from={}, result={}", from, r);
                }
            }
        }
    }
}

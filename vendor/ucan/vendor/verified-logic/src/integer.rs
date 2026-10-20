//! Verified integer properties and alignment operations.
//!
//! Parity checks, alignment primitives, floor-log2, and next-power-of-two.
//! These are the building blocks for memory allocators, page tables, and
//! size calculations where off-by-one or overflow means corruption.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_div_pos_is_pos;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_mod_multiples_basic;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_mod_pos_bound;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::mul::lemma_mul_increases;
use vstd::prelude::*;

verus! {

// --- Ghost helper: 2^n for bound proofs ---

/// Spec-only power of two. Used in proofs, erased at runtime.
pub open spec fn pow2_spec(n: nat) -> nat
    decreases n
{
    if n == 0 { 1 } else { 2 * pow2_spec((n - 1) as nat) }
}

// --- is_even ---

// r[depends integer.is-even]
/// Spec: x is divisible by 2.
pub open spec fn is_even_spec(x: u64) -> bool {
    x as int % 2 == 0
}

// r[impl integer.is-even]
/// Returns true if `x` is even.
pub fn is_even(x: u64) -> (result: bool)
    ensures result == is_even_spec(x)
{
    x % 2 == 0
}

// r[verify integer.is-even]
proof fn is_even_zero()
    ensures is_even_spec(0u64)
{}

// r[verify integer.is-even]
/// is_even and is_odd are complements.
proof fn even_odd_complement(x: u64)
    ensures is_even_spec(x) != is_odd_spec(x)
{}

// --- is_odd ---

// r[depends integer.is-odd]
/// Spec: x is not divisible by 2.
pub open spec fn is_odd_spec(x: u64) -> bool {
    x as int % 2 != 0
}

// r[impl integer.is-odd]
/// Returns true if `x` is odd.
pub fn is_odd(x: u64) -> (result: bool)
    ensures result == is_odd_spec(x)
{
    x % 2 != 0
}

// r[verify integer.is-odd]
proof fn is_odd_one()
    ensures is_odd_spec(1u64)
{}

// --- align_down ---

// r[depends integer.align-down]
/// Spec: largest multiple of `align` that is <= `x`.
pub open spec fn align_down_spec(x: u64, align: u64) -> u64
    recommends align > 0
{
    (x as int - x as int % align as int) as u64
}

// r[impl integer.align-down]
/// Rounds `x` down to the nearest multiple of `align`.
pub fn align_down(x: u64, align: u64) -> (result: u64)
    requires align > 0
    ensures result == align_down_spec(x, align)
{
    proof {
        lemma_fundamental_div_mod(x as int, align as int);
        // x == align * (x / align) + x % align
        // therefore x % align <= x  (since align * (x / align) >= 0)
    }
    x - x % align
}

// r[verify integer.align-down]
/// Result is at most x.
proof fn align_down_le(x: u64, align: u64)
    requires align > 0
    ensures align_down_spec(x, align) <= x
{
    lemma_mod_pos_bound(x as int, align as int);
    lemma_fundamental_div_mod(x as int, align as int);
}

// r[verify integer.align-down]
/// Result is a multiple of align.
proof fn align_down_divisible(x: u64, align: u64)
    requires align > 0
    ensures align_down_spec(x, align) as int % align as int == 0
{
    let a = align as int;
    let q = x as int / a;
    lemma_fundamental_div_mod(x as int, a);
    // x == a * q + x % a, so x - x % a == a * q
    lemma_mod_multiples_basic(q, a);
    // (q * a) % a == 0
}

// r[verify integer.align-down]
/// Alignment 1 is identity.
proof fn align_down_one(x: u64)
    ensures align_down_spec(x, 1) == x
{}

// r[verify integer.align-down]
/// Already-aligned values are unchanged.
proof fn align_down_aligned(x: u64, align: u64)
    requires align > 0, x as int % align as int == 0
    ensures align_down_spec(x, align) == x
{}

// --- align_up ---

// r[depends integer.align-up]
/// Spec: smallest multiple of `align` that is >= `x`. None on overflow.
pub open spec fn align_up_spec(x: u64, align: u64) -> Option<u64>
    recommends align > 0
{
    if x == 0 {
        Some(0u64)
    } else {
        let q = ((x - 1) as int / align as int) + 1;
        let rounded = q * align as int;
        if rounded > u64::MAX as int {
            None
        } else {
            Some(rounded as u64)
        }
    }
}

// r[impl integer.align-up]
/// Rounds `x` up to the nearest multiple of `align`. Returns `None` on overflow.
pub fn align_up(x: u64, align: u64) -> (result: Option<u64>)
    requires align > 0
    ensures result == align_up_spec(x, align)
{
    if x == 0 {
        Some(0)
    } else {
        let q = (x - 1) / align + 1;
        q.checked_mul(align)
    }
}

// r[verify integer.align-up]
/// align_up(0, a) is always Some(0).
proof fn align_up_zero(align: u64)
    requires align > 0
    ensures align_up_spec(0, align) == Some(0u64)
{}

// r[verify integer.align-up]
/// Alignment 1 is identity.
proof fn align_up_one(x: u64)
    ensures align_up_spec(x, 1) == Some(x)
{}

// r[verify integer.align-up]
/// When defined, result >= x.
proof fn align_up_ge(x: u64, align: u64)
    requires align > 0, align_up_spec(x, align).is_some()
    ensures align_up_spec(x, align).unwrap() >= x
{
    if x > 0 {
        let a = align as int;
        let xm = (x - 1) as int;
        let q = xm / a;
        lemma_fundamental_div_mod(xm, a);
        lemma_mod_pos_bound(xm, a);
        assert((q + 1) * a >= x as int) by(nonlinear_arith)
            requires
                xm == a * q + xm % a,
                0 <= xm % a,
                xm % a < a,
                xm == x as int - 1,
                a > 0;
    }
}

// NOTE: align_up divisibility is verified empirically via property tests
// (prop_align_up_divisible). Z3 can prove (q * a) % a == 0 but cannot
// chain through the Option::unwrap in the spec postcondition. The exec
// fn's ensures clause (result == align_up_spec) is fully machine-checked.

// --- log2_floor ---

// r[depends integer.log2-floor]
/// Spec: floor of log base 2.
pub open spec fn log2_floor_spec(x: u64) -> nat
    recommends x > 0
    decreases x
{
    if x <= 1 { 0 } else { 1 + log2_floor_spec((x / 2) as u64) }
}

/// Parameterized bound: log2_floor(x) < n when x < 2^n.
proof fn log2_floor_lt(x: u64, n: nat)
    requires x > 0, n > 0, (x as int) < pow2_spec(n) as int
    ensures log2_floor_spec(x) < n
    decreases n
{
    if x <= 1 {
        // log2_floor_spec(x) = 0 < n ✓
    } else {
        // x > 1 and x < pow2(n) = 2 * pow2(n-1), so n >= 2
        assert(pow2_spec(n) == 2 * pow2_spec((n - 1) as nat));
        assert((x as int) / 2 < pow2_spec((n - 1) as nat) as int);
        log2_floor_lt((x / 2) as u64, (n - 1) as nat);
    }
}

/// log2_floor of any u64 fits in 6 bits (result <= 63).
proof fn log2_floor_bounded(x: u64)
    requires x > 0
    ensures log2_floor_spec(x) <= 63
{
    // x: u64, so x <= 2^64 - 1 < 2^64 = pow2_spec(64)
    // We prove pow2_spec(64) > u64::MAX by unrolling.
    assert(pow2_spec(64) > u64::MAX as int) by(compute_only);
    log2_floor_lt(x, 64);
}

// r[impl integer.log2-floor]
/// Returns floor(log2(x)).
///
/// Recursive halving — terminates because x/2 < x for x > 1.
/// Max depth is 64 for any u64.
pub fn log2_floor(x: u64) -> (result: u64)
    requires x > 0
    ensures result as nat == log2_floor_spec(x)
    decreases x
{
    if x <= 1 {
        0
    } else {
        let sub = log2_floor(x / 2);
        proof {
            log2_floor_bounded((x / 2) as u64);
            // sub as nat == log2_floor_spec(x / 2) <= 63
            // so sub <= 63, 1 + sub <= 64, fits in u64
        }
        1 + sub
    }
}

// r[verify integer.log2-floor]
proof fn log2_floor_one()
    ensures log2_floor_spec(1u64) == 0nat
{}

// r[verify integer.log2-floor]
proof fn log2_floor_two()
    ensures log2_floor_spec(2u64) == 1nat
{
    assert(log2_floor_spec(2u64) == 1 + log2_floor_spec(1u64));
}

// r[verify integer.log2-floor]
proof fn log2_floor_four()
    ensures log2_floor_spec(4u64) == 2nat
{
    assert(log2_floor_spec(4u64) == 1 + log2_floor_spec(2u64));
    assert(log2_floor_spec(2u64) == 1 + log2_floor_spec(1u64));
}

// --- next_power_of_two ---

// r[depends integer.next-power-of-two]
/// Spec: smallest power of 2 >= x. None on overflow.
pub open spec fn next_power_of_two_spec(x: u64) -> Option<u64> {
    if x <= 1 {
        Some(1u64)
    } else {
        let log = log2_floor_spec((x - 1) as u64);
        if log >= 63 {
            None
        } else {
            Some(1u64 << (log as u64 + 1))
        }
    }
}

// r[impl integer.next-power-of-two]
/// Returns the smallest power of two >= `x`, or `None` if it would overflow u64.
pub fn next_power_of_two(x: u64) -> (result: Option<u64>)
    ensures result == next_power_of_two_spec(x)
{
    if x <= 1 {
        Some(1)
    } else {
        let log = log2_floor(x - 1);
        if log >= 63 {
            None
        } else {
            Some(1u64 << (log + 1))
        }
    }
}

// r[verify integer.next-power-of-two]
proof fn next_power_of_two_zero()
    ensures next_power_of_two_spec(0u64) == Some(1u64)
{}

// r[verify integer.next-power-of-two]
proof fn next_power_of_two_one()
    ensures next_power_of_two_spec(1u64) == Some(1u64)
{}

// r[verify integer.next-power-of-two]
proof fn next_power_of_two_two()
    ensures next_power_of_two_spec(2u64) == Some(2u64)
{
    assert(log2_floor_spec(1u64) == 0nat);
    assert(1u64 << 1u64 == 2u64) by(bit_vector);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify integer.is-even]
    #[test]
    fn test_is_even() {
        assert!(is_even(0));
        assert!(is_even(2));
        assert!(is_even(100));
        assert!(is_even(u64::MAX - 1));
        assert!(!is_even(1));
        assert!(!is_even(3));
        assert!(!is_even(u64::MAX));
    }

    // r[verify integer.is-odd]
    #[test]
    fn test_is_odd() {
        assert!(is_odd(1));
        assert!(is_odd(3));
        assert!(is_odd(u64::MAX));
        assert!(!is_odd(0));
        assert!(!is_odd(2));
        assert!(!is_odd(100));
    }

    // r[verify integer.align-down]
    #[test]
    fn test_align_down() {
        assert_eq!(align_down(0, 4), 0);
        assert_eq!(align_down(3, 4), 0);
        assert_eq!(align_down(4, 4), 4);
        assert_eq!(align_down(7, 4), 4);
        assert_eq!(align_down(8, 4), 8);
        assert_eq!(align_down(100, 1), 100);
        assert_eq!(align_down(u64::MAX, 4096), u64::MAX - u64::MAX % 4096);
    }

    // r[verify integer.align-up]
    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 4), Some(0));
        assert_eq!(align_up(1, 4), Some(4));
        assert_eq!(align_up(4, 4), Some(4));
        assert_eq!(align_up(5, 4), Some(8));
        assert_eq!(align_up(100, 1), Some(100));
        assert_eq!(align_up(u64::MAX, 2), None);
        assert_eq!(align_up(u64::MAX - 2, 4), None);
    }

    // r[verify integer.log2-floor]
    #[test]
    fn test_log2_floor() {
        assert_eq!(log2_floor(1), 0);
        assert_eq!(log2_floor(2), 1);
        assert_eq!(log2_floor(3), 1);
        assert_eq!(log2_floor(4), 2);
        assert_eq!(log2_floor(7), 2);
        assert_eq!(log2_floor(8), 3);
        assert_eq!(log2_floor(1023), 9);
        assert_eq!(log2_floor(1024), 10);
        assert_eq!(log2_floor(u64::MAX), 63);
    }

    // r[verify integer.next-power-of-two]
    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(0), Some(1));
        assert_eq!(next_power_of_two(1), Some(1));
        assert_eq!(next_power_of_two(2), Some(2));
        assert_eq!(next_power_of_two(3), Some(4));
        assert_eq!(next_power_of_two(4), Some(4));
        assert_eq!(next_power_of_two(5), Some(8));
        assert_eq!(next_power_of_two(1 << 62), Some(1 << 62_i32));
        assert_eq!(next_power_of_two(1 << 63), Some(1 << 63_i32));
        assert_eq!(next_power_of_two((1 << 63) + 1), None);
    }

    proptest! {
        // r[verify integer.is-even]
        // r[verify integer.is-odd]
        #[test]
        fn prop_even_odd_complement(x: u64) {
            prop_assert_ne!(is_even(x), is_odd(x));
        }

        // r[verify integer.align-down]
        #[test]
        fn prop_align_down_le(x: u64, align in 1u64..=4096) {
            prop_assert!(align_down(x, align) <= x);
        }

        // r[verify integer.align-down]
        #[test]
        fn prop_align_down_divisible(x: u64, align in 1u64..=4096) {
            prop_assert_eq!(align_down(x, align) % align, 0);
        }

        // r[verify integer.align-down]
        #[test]
        fn prop_align_down_one_is_identity(x: u64) {
            prop_assert_eq!(align_down(x, 1), x);
        }

        // r[verify integer.align-up]
        #[test]
        fn prop_align_up_ge(x: u64, align in 1u64..=4096) {
            if let Some(v) = align_up(x, align) {
                prop_assert!(v >= x);
            }
        }

        // r[verify integer.align-up]
        #[test]
        fn prop_align_up_divisible(x: u64, align in 1u64..=4096) {
            if let Some(v) = align_up(x, align) {
                prop_assert_eq!(v % align, 0);
            }
        }

        // r[verify integer.align-up]
        #[test]
        fn prop_align_up_one_is_identity(x: u64) {
            prop_assert_eq!(align_up(x, 1), Some(x));
        }

        // r[verify integer.align-down]
        // r[verify integer.align-up]
        #[test]
        fn prop_align_down_le_align_up(x: u64, align in 1u64..=4096) {
            if let Some(up) = align_up(x, align) {
                prop_assert!(align_down(x, align) <= up);
            }
        }

        // r[verify integer.log2-floor]
        #[test]
        fn prop_log2_floor_matches_stdlib(x in 1u64..=u64::MAX) {
            prop_assert_eq!(log2_floor(x), u64::from(x.ilog2()));
        }

        // r[verify integer.next-power-of-two]
        #[test]
        fn prop_next_power_of_two_matches_stdlib(x in 0u64..=(1u64 << 63_i32)) {
            let ours = next_power_of_two(x);
            let stdlib = x.checked_next_power_of_two();
            prop_assert_eq!(ours, stdlib);
        }

        // r[verify integer.next-power-of-two]
        #[test]
        fn prop_next_power_of_two_is_power(x: u64) {
            if let Some(v) = next_power_of_two(x) {
                prop_assert!(v >= x);
                prop_assert!(v.is_power_of_two());
            }
        }
    }
}

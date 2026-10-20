//! Verified fetch-and-add primitives.
//!
//! Models unconditional atomic read-modify-write over `u64` cells. The
//! operation returns the old value and stores the updated value. Both wrapping
//! and saturating variants are modeled, along with algebraic laws and a small
//! CAS refinement bridge.

use vstd::prelude::*;

use crate::arithmetic::saturating_add;
#[cfg(verus_keep_ghost)]
use crate::arithmetic::saturating_add_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::cas_result_spec;

verus! {

pub open spec fn u64_modulus_spec() -> int {
    u64::MAX as int + 1
}

// r[impl fetch-add.wrapping]
// r[depends fetch-add.wrapping]
pub open spec fn u64_wrapping_add_spec(a: u64, b: u64) -> u64 {
    (((a as int) + (b as int)) % u64_modulus_spec()) as u64
}

// r[impl fetch-add.saturating]
// r[depends fetch-add.saturating]
pub open spec fn u64_saturating_add_spec(a: u64, b: u64) -> u64 {
    saturating_add_spec(a, b)
}

// r[impl fetch-add.fetch-sub]
// r[depends fetch-add.fetch-sub]
pub open spec fn u64_wrapping_neg_spec(x: u64) -> u64 {
    if x == 0 {
        0u64
    } else {
        (u64_modulus_spec() - x as int) as u64
    }
}

proof fn wrapping_add_matches_spec(a: u64, b: u64)
    ensures u64_wrapping_add_spec(a, b) == a.wrapping_add(b)
{
    assert(u64_wrapping_add_spec(a, b) == a.wrapping_add(b)) by(bit_vector);
}

// r[impl fetch-add.wrapping]
pub fn u64_wrapping_add(a: u64, b: u64) -> (result: u64)
    ensures result == u64_wrapping_add_spec(a, b)
{
    let result = a.wrapping_add(b);
    proof {
        wrapping_add_matches_spec(a, b);
    }
    result
}

// r[impl fetch-add.saturating]
pub fn u64_saturating_add(a: u64, b: u64) -> (result: u64)
    ensures result == u64_saturating_add_spec(a, b)
{
    saturating_add(a, b)
}

// r[impl fetch-add.fetch-sub]
pub fn u64_wrapping_neg(x: u64) -> (result: u64)
    ensures result == u64_wrapping_neg_spec(x)
{
    0u64.wrapping_sub(x)
}

// r[impl fetch-add.semantics]
// r[depends fetch-add.semantics]
pub open spec fn faa_wrapping_spec(current: u64, addend: u64) -> (u64, u64) {
    (current, u64_wrapping_add_spec(current, addend))
}

// r[impl fetch-add.semantics]
// r[depends fetch-add.semantics]
pub open spec fn faa_saturating_spec(current: u64, addend: u64) -> (u64, u64) {
    (current, u64_saturating_add_spec(current, addend))
}

// r[impl fetch-add.cas-refinement]
// r[depends fetch-add.cas-refinement]
pub open spec fn faa_via_cas_spec(current: u64, addend: u64) -> (u64, u64) {
    let desired = u64_wrapping_add_spec(current, addend);
    let (_ok, after) = cas_result_spec(current, current, desired);
    (current, after)
}

// r[impl fetch-add.fetch-sub]
// r[depends fetch-add.fetch-sub]
pub open spec fn fetch_sub_wrapping_spec(current: u64, subtrahend: u64) -> (u64, u64) {
    faa_wrapping_spec(current, u64_wrapping_neg_spec(subtrahend))
}

// r[impl fetch-add.semantics]
pub fn fetch_add_wrapping(current: u64, addend: u64) -> (result: (u64, u64))
    ensures result == faa_wrapping_spec(current, addend)
{
    (current, u64_wrapping_add(current, addend))
}

// r[impl fetch-add.semantics]
pub fn fetch_add_saturating(current: u64, addend: u64) -> (result: (u64, u64))
    ensures result == faa_saturating_spec(current, addend)
{
    (current, u64_saturating_add(current, addend))
}

// r[impl fetch-add.fetch-sub]
pub fn fetch_sub_wrapping(current: u64, subtrahend: u64) -> (result: (u64, u64))
    ensures result == fetch_sub_wrapping_spec(current, subtrahend)
{
    (current, u64_wrapping_add(current, u64_wrapping_neg(subtrahend)))
}

// r[impl fetch-add.identity]
// r[verify fetch-add.identity]
proof fn faa_identity(current: u64)
    ensures
        faa_wrapping_spec(current, 0u64) == (current, current),
        faa_saturating_spec(current, 0u64) == (current, current),
{
    wrapping_add_identity(current);
}

// r[verify fetch-add.semantics]
proof fn faa_return_semantics(current: u64, addend: u64)
    ensures
        faa_wrapping_spec(current, addend).0 == current,
        faa_wrapping_spec(current, addend).1 == u64_wrapping_add_spec(current, addend),
        faa_saturating_spec(current, addend).0 == current,
        faa_saturating_spec(current, addend).1 == u64_saturating_add_spec(current, addend),
{}

// r[verify fetch-add.wrapping]
proof fn wrapping_add_commutative(a: u64, b: u64)
    ensures u64_wrapping_add_spec(a, b) == u64_wrapping_add_spec(b, a)
{
    wrapping_add_matches_spec(a, b);
    wrapping_add_matches_spec(b, a);
    assert(a.wrapping_add(b) == b.wrapping_add(a)) by(bit_vector);
}

// r[verify fetch-add.wrapping]
proof fn wrapping_add_identity(a: u64)
    ensures
        u64_wrapping_add_spec(a, 0u64) == a,
        u64_wrapping_add_spec(0u64, a) == a,
{
    wrapping_add_matches_spec(a, 0u64);
    wrapping_add_matches_spec(0u64, a);
    assert(a.wrapping_add(0u64) == a) by(bit_vector);
    assert(0u64.wrapping_add(a) == a) by(bit_vector);
}

// r[verify fetch-add.wrapping]
proof fn wrapping_add_overflow_case()
    ensures u64_wrapping_add_spec(u64::MAX, 1u64) == 0u64
{}

// r[verify fetch-add.saturating]
proof fn saturating_add_clamp(a: u64, b: u64)
    requires (a as int) + (b as int) > u64::MAX as int
    ensures u64_saturating_add_spec(a, b) == u64::MAX
{}

// r[verify fetch-add.saturating]
proof fn saturating_add_idempotent_at_max(b: u64)
    ensures u64_saturating_add_spec(u64::MAX, b) == u64::MAX
{}

// r[impl fetch-add.commutativity]
// r[verify fetch-add.commutativity]
proof fn faa_commutative(current: u64, a: u64, b: u64)
    ensures
        faa_wrapping_spec(faa_wrapping_spec(current, a).1, b).1
            == faa_wrapping_spec(faa_wrapping_spec(current, b).1, a).1,
        faa_saturating_spec(faa_saturating_spec(current, a).1, b).1
            == faa_saturating_spec(faa_saturating_spec(current, b).1, a).1,
{
    assert(
        faa_wrapping_spec(faa_wrapping_spec(current, a).1, b).1
            == faa_wrapping_spec(faa_wrapping_spec(current, b).1, a).1
    ) by(bit_vector);
}

// r[impl fetch-add.associativity]
// r[verify fetch-add.associativity]
proof fn faa_associative(current: u64, a: u64, b: u64)
    ensures
        faa_wrapping_spec(faa_wrapping_spec(current, a).1, b).1
            == faa_wrapping_spec(current, u64_wrapping_add_spec(a, b)).1,
        faa_saturating_spec(faa_saturating_spec(current, a).1, b).1
            == faa_saturating_spec(current, u64_saturating_add_spec(a, b)).1,
{
    assert(
        faa_wrapping_spec(faa_wrapping_spec(current, a).1, b).1
            == faa_wrapping_spec(current, u64_wrapping_add_spec(a, b)).1
    ) by(bit_vector);
}

// r[impl fetch-add.monotonicity]
// r[verify fetch-add.monotonicity]
proof fn faa_monotone_no_wrap(current: u64, addend: u64)
    requires
        addend > 0,
        (current as int) + (addend as int) <= u64::MAX as int,
    ensures faa_wrapping_spec(current, addend).1 > current,
{
    let sum = current as int + addend as int;
    assert(u64_modulus_spec() == u64::MAX as int + 1);
    assert(sum == current as int + addend as int);
    assert(sum <= u64::MAX as int);
    assert(sum < u64_modulus_spec()) by(nonlinear_arith)
        requires
            sum <= u64::MAX as int,
            u64_modulus_spec() == u64::MAX as int + 1;
    assert(sum % u64_modulus_spec() == sum) by(nonlinear_arith)
        requires
            0 <= sum,
            sum < u64_modulus_spec(),
            u64_modulus_spec() > 0;
    assert(u64_wrapping_add_spec(current, addend) == sum as u64);
    assert(sum > current as int) by(nonlinear_arith)
        requires
            addend > 0,
            sum == current as int + addend as int;
}

// r[verify fetch-add.cas-refinement]
proof fn cas_refines_faa(current: u64, addend: u64)
    ensures faa_via_cas_spec(current, addend) == faa_wrapping_spec(current, addend)
{}

// r[verify fetch-add.fetch-sub]
proof fn fetch_sub_is_negated_add(current: u64, subtrahend: u64)
    ensures fetch_sub_wrapping_spec(current, subtrahend)
        == faa_wrapping_spec(current, u64_wrapping_neg_spec(subtrahend))
{}

// r[verify fetch-add.fetch-sub]
proof fn add_then_sub_roundtrip(current: u64, amount: u64)
    requires (current as int) + (amount as int) <= u64::MAX as int
    ensures fetch_sub_wrapping_spec(faa_wrapping_spec(current, amount).1, amount).1 == current
{
    assert(fetch_sub_wrapping_spec(faa_wrapping_spec(current, amount).1, amount).1 == current) by(bit_vector);
}

// r[verify fetch-add.fetch-sub]
proof fn sub_then_add_roundtrip(current: u64, amount: u64)
    requires current >= amount
    ensures faa_wrapping_spec(fetch_sub_wrapping_spec(current, amount).1, amount).1 == current
{
    assert(faa_wrapping_spec(fetch_sub_wrapping_spec(current, amount).1, amount).1 == current) by(bit_vector);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify fetch-add.wrapping]
    #[test]
    fn test_wrapping_add_edges() {
        assert_eq!(u64_wrapping_add(0, 0), 0);
        assert_eq!(u64_wrapping_add(u64::MAX, 1), 0);
        assert_eq!(u64_wrapping_add(u64::MAX, 0), u64::MAX);
    }

    // r[verify fetch-add.saturating]
    #[test]
    fn test_saturating_add_edges() {
        assert_eq!(u64_saturating_add(0, 0), 0);
        assert_eq!(u64_saturating_add(u64::MAX, 1), u64::MAX);
        assert_eq!(u64_saturating_add(u64::MAX - 1, 1), u64::MAX);
    }

    // r[verify fetch-add.semantics]
    #[test]
    fn test_fetch_add_return_semantics() {
        assert_eq!(fetch_add_wrapping(10, 7), (10, 17));
        assert_eq!(fetch_add_saturating(u64::MAX, 1), (u64::MAX, u64::MAX));
    }

    // r[verify fetch-add.fetch-sub]
    #[test]
    fn test_fetch_sub_roundtrip() {
        let (_, after_add) = fetch_add_wrapping(41, 8);
        let (_, after_sub) = fetch_sub_wrapping(after_add, 8);
        assert_eq!(after_sub, 41);
    }

    proptest! {
        // r[verify fetch-add.commutativity]
        #[test]
        fn prop_wrapping_commutative(current: u64, a: u64, b: u64) {
            let (_, ab_mid) = fetch_add_wrapping(current, a);
            let (_, ab_end) = fetch_add_wrapping(ab_mid, b);
            let (_, ba_mid) = fetch_add_wrapping(current, b);
            let (_, ba_end) = fetch_add_wrapping(ba_mid, a);
            prop_assert_eq!(ab_end, ba_end);
        }

        // r[verify fetch-add.associativity]
        #[test]
        fn prop_wrapping_associative(current: u64, a: u64, b: u64) {
            let (_, ab_mid) = fetch_add_wrapping(current, a);
            let (_, ab_end) = fetch_add_wrapping(ab_mid, b);
            let (_, combined) = fetch_add_wrapping(current, a.wrapping_add(b));
            prop_assert_eq!(ab_end, combined);
        }

        // r[verify fetch-add.fetch-sub]
        #[test]
        fn prop_add_sub_roundtrip(current: u64, amount: u64) {
            prop_assume!((current as u128) + (amount as u128) <= (u64::MAX as u128));
            let (_, after_add) = fetch_add_wrapping(current, amount);
            let (_, after_sub) = fetch_sub_wrapping(after_add, amount);
            prop_assert_eq!(after_sub, current);
        }

        // r[verify fetch-add.cas-refinement]
        #[test]
        fn prop_cas_refinement(current: u64, addend: u64) {
            let desired = current.wrapping_add(addend);
            let (_, after) = crate::cas_loop::cas_result(current, current, desired);
            prop_assert_eq!((current, after), fetch_add_wrapping(current, addend));
        }

        // r[verify fetch-add.saturating]
        #[test]
        fn prop_saturating_bounded(a: u64, b: u64) {
            let result = u64_saturating_add(a, b);
            prop_assert!(result >= a || result == u64::MAX);
        }
    }
}

//! Verified polynomial evaluation for Shamir secret sharing.
//!
//! Coefficients use ascending degree order: `coeffs[i]` is the coefficient
//! of `x^i`.

use vstd::prelude::*;

use crate::shamir_field;
use crate::shamir_field::*;

pub type Polynomial = Vec<u64>;

verus! {

// r[depends shamir-poly.horner-correctness]
/// Spec: polynomial evaluation in recursive Horner form.
pub open spec fn poly_eval_sum_spec(coeffs: Seq<u64>, x: u64, p: u64) -> u64
    recommends valid_modulus_spec(p), forall|i: int| 0 <= i < coeffs.len() ==> coeffs[i] < p
    decreases coeffs.len()
{
    if coeffs.len() == 0 {
        0
    } else {
        field_add_spec(
            coeffs[0],
            field_mul_spec(x, poly_eval_sum_spec(coeffs.subrange(1, coeffs.len() as int), x, p), p),
            p,
        )
    }
}

// r[depends shamir-poly.degree-bound]
// r[impl shamir-poly.degree-bound]
/// Spec: degree upper bound of coefficient vector.
pub open spec fn poly_degree_spec(coeffs: Seq<u64>) -> int {
    if coeffs.len() == 0 { -1 } else { coeffs.len() as int - 1 }
}

fn poly_eval_from(coeffs: &[u64], x: u64, p: u64, from: usize) -> (result: u64)
    requires
        valid_modulus_spec(p),
        x < p,
        from <= coeffs@.len(),
        forall|i: int| 0 <= i < coeffs@.len() ==> coeffs@[i] < p,
    ensures
        result == poly_eval_sum_spec(coeffs@.subrange(from as int, coeffs@.len() as int), x, p),
        result < p,
    decreases coeffs@.len() - from
{
    if from >= coeffs.len() {
        0
    } else {
        let tail = poly_eval_from(coeffs, x, p, from + 1);
        let prod = field_mul(x, tail, p);
        proof {
            let ghost s = coeffs@.subrange(from as int, coeffs@.len() as int);
            assert(s.len() > 0);
            assert(s[0] == coeffs@[from as int]);
            assert(s.subrange(1, s.len() as int) =~= coeffs@.subrange(from as int + 1, coeffs@.len() as int));
            assert(tail == poly_eval_sum_spec(coeffs@.subrange(from as int + 1, coeffs@.len() as int), x, p));
            assert(prod == field_mul_spec(x, tail, p));
            assert(poly_eval_sum_spec(s, x, p)
                == field_add_spec(
                    s[0],
                    field_mul_spec(x, poly_eval_sum_spec(s.subrange(1, s.len() as int), x, p), p),
                    p,
                ));
        }
        field_add(coeffs[from], prod, p)
    }
}

// r[impl shamir-poly.horner-correctness]
/// Evaluates a polynomial at `x` by Horner's method.
pub fn poly_eval_horner(coeffs: &[u64], x: u64, p: u64) -> (result: u64)
    requires
        valid_modulus_spec(p),
        x < p,
        forall|i: int| 0 <= i < coeffs@.len() ==> coeffs@[i] < p,
    ensures result == poly_eval_sum_spec(coeffs@, x, p)
{
    let result = poly_eval_from(coeffs, x, p, 0);
    proof {
        assert(coeffs@.subrange(0, coeffs@.len() as int) =~= coeffs@);
    }
    result
}

// r[verify shamir-poly.degree-bound]
/// A coefficient vector of length `k` has degree at most `k-1`.
pub proof fn poly_degree_bound(coeffs: Seq<u64>)
    ensures poly_degree_spec(coeffs) < coeffs.len() as int || coeffs.len() == 0
{
}

// r[verify shamir-poly.horner-correctness]
/// Constant polynomial evaluates to its constant term.
pub proof fn constant_poly_eval(c: u64, x: u64, p: u64)
    requires valid_modulus_spec(p), c < p, x < p
    ensures poly_eval_sum_spec(seq![c], x, p) == c
{
    let ghost s = seq![c];
    assert(s.len() == 1);
    assert(s[0] == c);
    assert(s.subrange(1, s.len() as int) =~= Seq::<u64>::empty());
    assert(poly_eval_sum_spec(Seq::<u64>::empty(), x, p) == 0);
    field_mul_zero(x, p);
    field_add_zero(c, p);
    assert(poly_eval_sum_spec(s, x, p)
        == field_add_spec(c, field_mul_spec(x, 0, p), p));
}

// r[verify shamir-poly.horner-correctness]
/// Linear polynomial matches `a0 + a1*x`.
pub proof fn linear_poly_eval(a0: u64, a1: u64, x: u64, p: u64)
    requires valid_modulus_spec(p), a0 < p, a1 < p, x < p
    ensures poly_eval_sum_spec(seq![a0, a1], x, p) == field_add_spec(a0, field_mul_spec(a1, x, p), p)
{
    let ghost s = seq![a0, a1];
    assert(s.len() == 2);
    assert(s[0] == a0);
    assert(s.subrange(1, s.len() as int) =~= seq![a1]);
    constant_poly_eval(a1, x, p);
    field_commutative(x, a1, p);
    assert(poly_eval_sum_spec(s, x, p)
        == field_add_spec(a0, field_mul_spec(x, poly_eval_sum_spec(seq![a1], x, p), p), p));
    assert(poly_eval_sum_spec(seq![a1], x, p) == a1);
    assert(field_mul_spec(x, a1, p) == field_mul_spec(a1, x, p));
}

// r[verify shamir-poly.horner-correctness]
/// Evaluating at zero returns the constant term for non-empty polynomials.
pub proof fn poly_at_zero_is_constant(coeffs: Seq<u64>, p: u64)
    requires
        valid_modulus_spec(p),
        coeffs.len() > 0,
        forall|i: int| 0 <= i < coeffs.len() ==> coeffs[i] < p,
    ensures poly_eval_sum_spec(coeffs, 0, p) == coeffs[0]
    decreases coeffs.len()
{
    if coeffs.len() == 1 {
        constant_poly_eval(coeffs[0], 0, p);
    } else {
        poly_at_zero_is_constant(coeffs.subrange(1, coeffs.len() as int), p);
        field_mul_zero(poly_eval_sum_spec(coeffs.subrange(1, coeffs.len() as int), 0, p), p);
        field_add_zero(coeffs[0], p);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn direct_eval(coeffs: &[u64], x: u64, p: u64) -> u64 {
        let mut power = 1u64;
        let mut acc = 0u64;
        for &c in coeffs {
            acc = (acc + ((c * power) % p)) % p;
            power = (power * x) % p;
        }
        acc
    }

    // r[verify shamir-poly.horner-correctness]
    #[test]
    fn test_constant_and_linear() {
        assert_eq!(poly_eval_horner(&[9], 12, 17), 9);
        assert_eq!(poly_eval_horner(&[3, 4], 5, 17), (3 + 4 * 5) % 17);
    }

    fn degree_runtime(coeffs: &[u64]) -> Option<usize> {
        coeffs.len().checked_sub(1)
    }

    // r[verify shamir-poly.degree-bound]
    #[test]
    fn test_degree_bound() {
        assert_eq!(degree_runtime(&[1u64, 2u64, 3u64]), Some(2));
        assert_eq!(degree_runtime(&[]), None);
    }

    proptest! {
        // r[verify shamir-poly.horner-correctness]
        #[test]
        fn prop_horner_matches_direct(
            coeffs in proptest::collection::vec(0u64..17, 0..6),
            x in 0u64..17,
        ) {
            let p = 17u64;
            let got = poly_eval_horner(&coeffs, x, p);
            let want = direct_eval(&coeffs, x, p);
            prop_assert_eq!(got, want);
        }

        // r[verify shamir-poly.horner-correctness]
        #[test]
        fn prop_eval_at_zero_is_constant(
            coeffs in proptest::collection::vec(0u64..17, 1..6),
        ) {
            let p = 17u64;
            prop_assert_eq!(poly_eval_horner(&coeffs, 0, p), coeffs[0] % p);
        }

        // r[verify shamir-poly.degree-bound]
        #[test]
        fn prop_degree_is_len_minus_one(
            coeffs in proptest::collection::vec(0u64..17, 1..8),
        ) {
            prop_assert_eq!(degree_runtime(&coeffs), coeffs.len().checked_sub(1));
        }
    }
}

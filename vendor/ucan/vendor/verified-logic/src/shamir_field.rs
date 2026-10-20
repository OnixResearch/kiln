//! Verified finite-field primitives for Shamir secret sharing.
//!
//! Field elements are `u64` values modulo an explicit modulus `p`.
//! We require `p < 2^32` so products of two field elements fit in `u64`.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_mod_pos_bound;
use vstd::prelude::*;

verus! {

pub const MAX_FIELD_MODULUS: u64 = 0x1_0000_0000u64;

// r[depends shamir-field.prime-modulus]
// r[depends shamir-field.closure]
/// Spec: modulus is prime and small enough that products fit in `u64`.
pub open spec fn valid_modulus_spec(p: u64) -> bool {
    &&& 1 < p < MAX_FIELD_MODULUS
    &&& forall|d: int| #![trigger (p as int) % d] 2 <= d < p as int ==> (p as int) % d != 0
}

proof fn divisor_has_small_cofactor(p: int, d: int)
    requires
        2 <= d < p,
        p % d == 0,
        d * d > p,
    ensures
        2 <= p / d,
        (p / d) * (p / d) < p,
        p % (p / d) == 0,
{
    let q = p / d;
    assert(d * q == p) by(nonlinear_arith)
        requires d > 0, p % d == 0, q == p / d;
    assert(2 <= q) by(nonlinear_arith)
        requires 2 <= d < p, p % d == 0, q == p / d;
    assert(q < d) by(nonlinear_arith)
        requires d > 0, p % d == 0, d * d > p, q == p / d;
    assert(q > 0) by(nonlinear_arith)
        requires 2 <= q;
    assert(q * q < q * d) by(nonlinear_arith)
        requires q > 0, q < d;
    assert(q * q < p) by(nonlinear_arith)
        requires q * q < q * d, d * q == p;
    assert(p % q == 0) by(nonlinear_arith)
        requires q > 0, d * q == p;
}

proof fn no_small_divisors_implies_prime(p: u64, limit: u64)
    requires
        1 < p < MAX_FIELD_MODULUS,
        forall|e: int| #![trigger (p as int) % e] 2 <= e < limit as int ==> (p as int) % e != 0,
        (limit as int) * (limit as int) > p as int,
    ensures valid_modulus_spec(p)
{
    assert forall|d: int| #![trigger (p as int) % d] 2 <= d < p as int implies (p as int) % d != 0 by {
        if (p as int) % d == 0 {
            if d < limit as int {
                assert(false);
            } else {
                assert(d * d > p as int) by(nonlinear_arith)
                    requires d >= limit as int, (limit as int) * (limit as int) > p as int;
                divisor_has_small_cofactor(p as int, d);
                let q = (p as int) / d;
                assert(q < limit as int) by(nonlinear_arith)
                    requires q * q < p as int, (limit as int) * (limit as int) > p as int;
                assert((p as int) % q != 0);
                assert(false);
            }
        }
    }
}

proof fn loop_limit_square_gt(p: u64, d: u64)
    requires d > 0, d > p / d
    ensures (d as int) * (d as int) > p as int
{
    let q = p / d;
    lemma_fundamental_div_mod(p as int, d as int);
    assert((p as int) == ((q as int) * (d as int)) + ((p % d) as int));
    assert(((p % d) as int) < (d as int)) by(nonlinear_arith)
        requires d > 0;
    assert(((d as int) * (d as int)) > (p as int)) by(nonlinear_arith)
        requires d > q, (p as int) == ((q as int) * (d as int)) + ((p % d) as int), ((p % d) as int) < (d as int);
}

// r[impl shamir-field.prime-modulus]
// r[impl shamir-field.closure]
/// Runtime modulus validator matching `valid_modulus_spec`.
pub fn valid_modulus(p: u64) -> (result: bool)
    ensures result == valid_modulus_spec(p)
{
    if p <= 1 || p >= MAX_FIELD_MODULUS {
        return false;
    }
    let mut d: u64 = 2;
    while d <= p / d
        invariant
            2 <= d <= p,
            forall|e: int| #![trigger (p as int) % e] 2 <= e < d as int ==> (p as int) % e != 0,
        decreases p - d
    {
        if p % d == 0 {
            assert(d < p) by(nonlinear_arith)
                requires d > 0, d <= p / d, p > 1;
            assert(2 <= d < p);
            assert(!valid_modulus_spec(p));
            return false;
        }
        proof {
            assert forall|e: int| #![trigger (p as int) % e] 2 <= e < d as int + 1 implies (p as int) % e != 0 by {
                if e < d as int {
                } else {
                    assert(e == d as int);
                    assert((p as int) % e != 0);
                }
            }
        }
        assert(d < p) by(nonlinear_arith)
            requires d > 0, d <= p / d, p > 1;
        assert(d < u64::MAX);
        d += 1;
    }
    proof {
        loop_limit_square_gt(p, d);
        no_small_divisors_implies_prime(p, d);
    }
    true
}

// r[depends shamir-field.closure]
/// Spec: value is a valid field element modulo `p`.
pub open spec fn valid_elem_spec(a: u64, p: u64) -> bool {
    valid_modulus_spec(p) && a < p
}

// r[depends shamir-field.closure]
/// Spec: field addition modulo `p`.
pub open spec fn field_add_spec(a: u64, b: u64, p: u64) -> u64
    recommends valid_elem_spec(a, p), valid_elem_spec(b, p)
{
    let sum = a as int + b as int;
    if sum >= p as int { (sum - p as int) as u64 } else { sum as u64 }
}

// r[depends shamir-field.subtraction]
/// Spec: field subtraction modulo `p`.
pub open spec fn field_sub_spec(a: u64, b: u64, p: u64) -> u64
    recommends valid_elem_spec(a, p), valid_elem_spec(b, p)
{
    if a >= b {
        (a - b) as u64
    } else {
        (p - (b - a)) as u64
    }
}

// r[depends shamir-field.closure]
/// Spec: field multiplication modulo `p`.
pub open spec fn field_mul_spec(a: u64, b: u64, p: u64) -> u64
    recommends valid_elem_spec(a, p), valid_elem_spec(b, p)
{
    (((a as int) * (b as int)) % (p as int)) as u64
}

// r[depends shamir-field.multiplicative-inverse]
/// Spec helper: first inverse witness at or after `cand`.
pub open spec fn field_inv_from_spec(a: u64, p: u64, cand: int) -> Option<u64>
    recommends valid_elem_spec(a, p), 1 <= cand
    decreases if cand <= p as int { p as int - cand } else { 0 }
{
    if cand >= p as int {
        None
    } else if field_mul_spec(a, cand as u64, p) == 1 {
        Some(cand as u64)
    } else {
        field_inv_from_spec(a, p, cand + 1)
    }
}

// r[depends shamir-field.multiplicative-inverse]
/// Spec: multiplicative inverse if one exists.
pub open spec fn field_inv_spec(a: u64, p: u64) -> Option<u64>
    recommends valid_elem_spec(a, p)
{
    if a == 0 { None } else { field_inv_from_spec(a, p, 1) }
}

// r[impl shamir-field.closure]
// r[impl shamir-field.additive-identity]
// r[impl shamir-field.commutativity]
/// Adds two field elements modulo `p`.
pub fn field_add(a: u64, b: u64, p: u64) -> (result: u64)
    requires valid_elem_spec(a, p), valid_elem_spec(b, p)
    ensures result == field_add_spec(a, b, p), result < p
{
    assert((a as int) + (b as int) < (u64::MAX as int)) by(nonlinear_arith)
        requires a < p, b < p, p < MAX_FIELD_MODULUS;
    let sum = a + b;
    if sum >= p {
        assert(sum - p < p) by(nonlinear_arith)
            requires sum < 2 * p, sum >= p, p > 0;
        sum - p
    } else {
        assert(sum < p);
        sum
    }
}

// r[impl shamir-field.subtraction]
/// Subtracts two field elements modulo `p`.
pub fn field_sub(a: u64, b: u64, p: u64) -> (result: u64)
    requires valid_elem_spec(a, p), valid_elem_spec(b, p)
    ensures result == field_sub_spec(a, b, p), result < p
{
    if a >= b {
        assert(a - b < p) by(nonlinear_arith)
            requires a < p, b <= a;
        a - b
    } else {
        assert(b - a > 0) by(nonlinear_arith)
            requires a < b;
        assert(p - (b - a) < p) by(nonlinear_arith)
            requires b < p, a < b, p > 0;
        p - (b - a)
    }
}

// r[impl shamir-field.closure]
// r[impl shamir-field.multiplicative-identity]
// r[impl shamir-field.commutativity]
/// Multiplies two field elements modulo `p`.
pub fn field_mul(a: u64, b: u64, p: u64) -> (result: u64)
    requires valid_elem_spec(a, p), valid_elem_spec(b, p)
    ensures result == field_mul_spec(a, b, p), result < p
{
    assert(a as int * b as int <= u64::MAX as int) by(nonlinear_arith)
        requires a < p, b < p, p < MAX_FIELD_MODULUS;
    let product = a * b;
    proof {
        lemma_mod_pos_bound(product as int, p as int);
    }
    assert(product % p < p);
    product % p
}

proof fn gcd_positive(a: u64, b: u64)
    requires a > 0 || b > 0
    ensures crate::arithmetic::gcd_spec(a, b) > 0
    decreases b
{
    if b == 0 {
    } else {
        gcd_positive(b, a % b);
    }
}

proof fn gcd_divides(a: u64, b: u64)
    requires a > 0 || b > 0
    ensures
        crate::arithmetic::gcd_spec(a, b) > 0,
        a % crate::arithmetic::gcd_spec(a, b) == 0,
        b % crate::arithmetic::gcd_spec(a, b) == 0,
    decreases b
{
    if b == 0 {
        assert(crate::arithmetic::gcd_spec(a, 0) == a);
        assert(a > 0);
        assert(a % a == 0);
    } else {
        gcd_divides(b, a % b);
        let g = crate::arithmetic::gcd_spec(b, a % b);
        gcd_positive(b, a % b);
        lemma_fundamental_div_mod(a as int, b as int);
        lemma_fundamental_div_mod(b as int, g as int);
        lemma_fundamental_div_mod((a % b) as int, g as int);
        assert((a / b) as int == (a as int) / (b as int));
        assert((a % b) as int == (a as int) % (b as int));
        assert((a as int) == (b as int) * ((a / b) as int) + ((a % b) as int));
        assert((a as int) == ((a / b) as int) * (b as int) + ((a % b) as int)) by(nonlinear_arith)
            requires (a as int) == (b as int) * ((a / b) as int) + ((a % b) as int);
        assert((b as int) == (((b / g) as int) * (g as int)) + ((b % g) as int));
        assert(((a % b) as int) == ((((a % b) / g) as int) * (g as int)) + (((a % b) % g) as int));
        assert((b as int) == ((b / g) as int) * (g as int));
        assert(((a % b) as int) == (((a % b) / g) as int) * (g as int));
        assert((a as int) == (g as int) * ((((a / b) * (b / g)) as int) + (((a % b) / g) as int))) by(nonlinear_arith)
            requires
                (a as int) == ((a / b) as int) * (b as int) + ((a % b) as int),
                (b as int) == ((b / g) as int) * (g as int),
                ((a % b) as int) == (((a % b) / g) as int) * (g as int);
        assert((a % g) == 0) by(nonlinear_arith)
            requires g > 0, (a as int) == (g as int) * ((((a / b) * (b / g)) as int) + (((a % b) / g) as int));
    }
}

proof fn gcd_prime_nonzero_is_one(a: u64, p: u64)
    requires valid_elem_spec(a, p), a > 0
    ensures crate::arithmetic::gcd_spec(p, a) == 1
{
    let g = crate::arithmetic::gcd_spec(p, a);
    gcd_divides(p, a);
    assert(g <= a) by(nonlinear_arith)
        requires g > 0, a > 0, a % g == 0;
    assert(g < p) by(nonlinear_arith)
        requires g <= a, a < p;
    if g != 1 {
        assert(2 <= g < p);
        assert((p as int) % (g as int) == 0) by(nonlinear_arith)
            requires p % g == 0;
        assert(false);
    }
}

proof fn bezout_gcd(a: u64, b: u64) -> (result: (int, int))
    requires a > 0 || b > 0
    ensures result.0 * (a as int) + result.1 * (b as int) == crate::arithmetic::gcd_spec(a, b) as int
    decreases b
{
    if b == 0 {
        (1int, 0int)
    } else {
        let prev = bezout_gcd(b, a % b);
        let s = prev.0;
        let t = prev.1;
        lemma_fundamental_div_mod(a as int, b as int);
        let q = a / b;
        assert((a as int) == ((q as int) * (b as int)) + ((a % b) as int));
        assert((s - t * (q as int)) * (b as int)
            == s * (b as int) - t * ((q as int) * (b as int))) by(nonlinear_arith);
        assert(t * (a as int) == t * ((q as int) * (b as int)) + t * ((a % b) as int)) by(nonlinear_arith)
            requires (a as int) == ((q as int) * (b as int)) + ((a % b) as int);
        assert(t * (a as int) + (s - t * (q as int)) * (b as int)
            == t * (((q as int) * (b as int)) + ((a % b) as int)) + (s - t * (q as int)) * (b as int)) by(nonlinear_arith)
            requires (a as int) == ((q as int) * (b as int)) + ((a % b) as int);
        assert(t * (((q as int) * (b as int)) + ((a % b) as int)) + (s - t * (q as int)) * (b as int)
            == s * (b as int) + t * ((a % b) as int)) by(nonlinear_arith);
        assert(s * (b as int) + t * ((a % b) as int) == crate::arithmetic::gcd_spec(a, b) as int);
        (t, s - t * (q as int))
    }
}

proof fn field_inv_from_has_witness(a: u64, p: u64, cand: int, target: int)
    requires
        valid_elem_spec(a, p),
        1 <= cand <= target < p as int,
        field_mul_spec(a, target as u64, p) == 1,
    ensures field_inv_from_spec(a, p, cand).is_some()
    decreases target - cand
{
    if cand == target {
        assert(field_inv_from_spec(a, p, cand) == Some(target as u64));
    } else {
        field_inv_from_has_witness(a, p, cand + 1, target);
    }
}

proof fn field_inv_exists(a: u64, p: u64)
    requires valid_elem_spec(a, p), a > 0
    ensures field_inv_from_spec(a, p, 1).is_some()
{
    gcd_prime_nonzero_is_one(a, p);
    let coeffs = bezout_gcd(p, a);
    let t = coeffs.1;
    let q = t / (p as int);
    let r = t % (p as int);
    lemma_fundamental_div_mod(t, p as int);
    assert(t == q * (p as int) + r);
    assert(0 <= r < p as int);
    assert(coeffs.0 * (p as int) + coeffs.1 * (a as int) == 1);
    let m = -(coeffs.0 + q * (a as int));
    assert(coeffs.1 * (a as int) == q * (p as int) * (a as int) + (a as int) * r) by(nonlinear_arith)
        requires t == q * (p as int) + r, coeffs.1 == t;
    assert((a as int) * r == 1 + (p as int) * m) by(nonlinear_arith)
        requires
            coeffs.0 * (p as int) + coeffs.1 * (a as int) == 1,
            coeffs.1 * (a as int) == q * (p as int) * (a as int) + (a as int) * r,
            m == -(coeffs.0 + q * (a as int));
    assert(r != 0) by(nonlinear_arith)
        requires (a as int) * r == 1 + (p as int) * m, p > 1;
    assert((a as int) * r > 0) by(nonlinear_arith)
        requires a > 0, r > 0;
    assert((a as int) * r < (p as int) * (p as int)) by(nonlinear_arith)
        requires a < p, 0 <= r < p;
    assert(0 <= m < p as int) by(nonlinear_arith)
        requires
            (a as int) * r == 1 + (p as int) * m,
            (a as int) * r > 0,
            (a as int) * r < (p as int) * (p as int),
            p > 1;
    assert(((a as int) * r) % (p as int) == 1) by(nonlinear_arith)
        requires (a as int) * r == 1 + (p as int) * m, 0 <= m, p > 1;
    assert(field_mul_spec(a, r as u64, p) == 1);
    field_inv_from_has_witness(a, p, 1, r);
}

// r[impl shamir-field.multiplicative-inverse]
/// Finds a multiplicative inverse by linear search over the field.
pub fn field_inv(a: u64, p: u64) -> (result: Option<u64>)
    requires valid_elem_spec(a, p)
    ensures
        a == 0 ==> result.is_none(),
        a > 0 ==> result.is_some(),
        result.is_some() ==> {
            let inv = result.unwrap();
            &&& inv < p
            &&& field_mul_spec(a, inv, p) == 1
        },
{
    if a == 0 {
        return None;
    }
    proof {
        field_inv_exists(a, p);
    }
    let mut cand: u64 = 1;
    while cand < p
        invariant
            valid_elem_spec(a, p),
            a > 0,
            1 <= cand <= p,
            field_inv_from_spec(a, p, cand as int) == field_inv_from_spec(a, p, 1),
        decreases p - cand
    {
        if field_mul(a, cand, p) == 1 {
            return Some(cand);
        }
        proof {
            assert(field_inv_from_spec(a, p, cand as int) == field_inv_from_spec(a, p, cand as int + 1));
        }
        cand += 1;
    }
    proof {
        assert(field_inv_from_spec(a, p, 1).is_some());
        assert(field_inv_from_spec(a, p, p as int).is_none());
        assert(false);
    }
    None
}

// r[verify shamir-field.additive-identity]
/// Zero is additive identity.
pub proof fn field_add_zero(a: u64, p: u64)
    requires valid_elem_spec(a, p)
    ensures
        field_add_spec(a, 0, p) == a,
        field_add_spec(0, a, p) == a,
{
    assert(a as int + 0 < p as int) by(nonlinear_arith)
        requires a < p;
}

// r[verify shamir-field.multiplicative-identity]
/// One is multiplicative identity.
pub proof fn field_mul_one(a: u64, p: u64)
    requires valid_elem_spec(a, p), 1 < p
    ensures
        field_mul_spec(a, 1, p) == a,
        field_mul_spec(1, a, p) == a,
{
    assert((a as int * 1) % p as int == a as int) by(nonlinear_arith)
        requires a < p, p > 0;
}

// r[verify shamir-field.commutativity]
/// Addition and multiplication commute.
pub proof fn field_commutative(a: u64, b: u64, p: u64)
    requires valid_elem_spec(a, p), valid_elem_spec(b, p)
    ensures
        field_add_spec(a, b, p) == field_add_spec(b, a, p),
        field_mul_spec(a, b, p) == field_mul_spec(b, a, p),
{
    assert(a as int + b as int == b as int + a as int) by(nonlinear_arith);
    assert((a as int * b as int) % p as int == (b as int * a as int) % p as int) by(nonlinear_arith);
}

// r[verify shamir-field.closure]
/// Closure: results stay in the field range.
pub proof fn field_closed(a: u64, b: u64, p: u64)
    requires valid_elem_spec(a, p), valid_elem_spec(b, p)
    ensures
        field_add_spec(a, b, p) < p,
        field_mul_spec(a, b, p) < p,
        field_sub_spec(a, b, p) < p,
{
    if (a as int) + (b as int) >= (p as int) {
        assert(field_add_spec(a, b, p) == (((a as int) + (b as int) - (p as int)) as u64));
        assert(((a as int) + (b as int) - (p as int)) < (p as int)) by(nonlinear_arith)
            requires a < p, b < p;
    } else {
        assert(field_add_spec(a, b, p) == (((a as int) + (b as int)) as u64));
        assert((a as int) + (b as int) < (p as int));
    }
    lemma_mod_pos_bound((a as int) * (b as int), p as int);
    if a >= b {
        assert(field_sub_spec(a, b, p) == a - b);
        assert(a - b < p) by(nonlinear_arith)
            requires a < p, b <= a;
    } else {
        assert(field_sub_spec(a, b, p) == p - (b - a));
        assert(p - (b - a) < p) by(nonlinear_arith)
            requires a < b, b < p, p > 0;
    }
}

// r[verify shamir-field.subtraction]
/// Subtraction is additive inverse modulo `p`.
pub proof fn field_sub_add_roundtrip(a: u64, b: u64, p: u64)
    requires valid_elem_spec(a, p), valid_elem_spec(b, p)
    ensures field_add_spec(field_sub_spec(a, b, p), b, p) == a
{
    if a >= b {
        assert(field_sub_spec(a, b, p) == a - b);
        assert(field_add_spec((a - b) as u64, b, p) == a) by(nonlinear_arith)
            requires a < p, b <= a;
    } else {
        assert(field_sub_spec(a, b, p) == p - (b - a));
        assert(field_add_spec((p - (b - a)) as u64, b, p) == a) by(nonlinear_arith)
            requires a < b, b < p, p > 0;
    }
}

// r[verify shamir-field.multiplicative-inverse]
/// Spec helper: inverse witness returned by `field_inv_from_spec` is correct.
pub proof fn field_inv_from_spec_correct(a: u64, p: u64, cand: int)
    requires
        valid_elem_spec(a, p),
        1 <= cand,
        field_inv_from_spec(a, p, cand).is_some(),
    ensures
        field_mul_spec(a, field_inv_from_spec(a, p, cand).unwrap(), p) == 1,
        field_inv_from_spec(a, p, cand).unwrap() < p,
    decreases if cand <= p as int { p as int - cand } else { 0 }
{
    if cand >= p as int {
        assert(false);
    } else if field_mul_spec(a, cand as u64, p) == 1 {
        assert(field_inv_from_spec(a, p, cand) == Some(cand as u64));
    } else {
        field_inv_from_spec_correct(a, p, cand + 1);
    }
}

// r[verify shamir-field.multiplicative-inverse]
/// If the inverse spec returns a witness, it multiplies to one.
pub proof fn field_inv_spec_correct(a: u64, p: u64)
    requires valid_elem_spec(a, p), field_inv_spec(a, p).is_some()
    ensures field_mul_spec(a, field_inv_spec(a, p).unwrap(), p) == 1
{
    assert(a != 0);
    field_inv_from_spec_correct(a, p, 1);
}

// r[verify shamir-field.multiplicative-inverse]
/// Zero has no inverse.
pub proof fn zero_has_no_inverse(p: u64)
    requires valid_modulus_spec(p)
    ensures field_inv_spec(0, p).is_none()
{
}

// r[verify shamir-field.closure]
/// Multiplying by zero yields zero.
pub proof fn field_mul_zero(a: u64, p: u64)
    requires valid_elem_spec(a, p)
    ensures
        field_mul_spec(a, 0, p) == 0,
        field_mul_spec(0, a, p) == 0,
{
    assert((a as int * 0) % p as int == 0) by(nonlinear_arith)
        requires p > 0;
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn small_primes() -> [u64; 5] {
        [5, 7, 11, 17, 257]
    }

    // r[verify shamir-field.closure]
    #[test]
    fn test_field_add_mul_closure() {
        assert_eq!(field_add(9, 11, 17), 3);
        assert_eq!(field_mul(7, 5, 17), 1);
        assert_eq!(field_sub(3, 5, 17), 15);
    }

    // r[verify shamir-field.additive-identity]
    #[test]
    fn test_additive_identity() {
        assert_eq!(field_add(9, 0, 17), 9);
        assert_eq!(field_add(0, 9, 17), 9);
    }

    // r[verify shamir-field.multiplicative-identity]
    #[test]
    fn test_multiplicative_identity() {
        assert_eq!(field_mul(9, 1, 17), 9);
        assert_eq!(field_mul(1, 9, 17), 9);
    }

    // r[verify shamir-field.commutativity]
    #[test]
    fn test_commutativity() {
        assert_eq!(field_add(6, 13, 17), field_add(13, 6, 17));
        assert_eq!(field_mul(6, 13, 17), field_mul(13, 6, 17));
    }

    // r[verify shamir-field.multiplicative-inverse]
    #[test]
    fn test_inverse_vectors() {
        assert_eq!(field_inv(1, 17), Some(1));
        assert_eq!(field_inv(2, 17), Some(9));
        assert_eq!(field_inv(3, 17), Some(6));
        assert_eq!(field_inv(0, 17), None);
    }

    // r[verify shamir-field.prime-modulus]
    // r[verify shamir-field.closure]
    #[test]
    fn test_prime_modulus_validation() {
        assert!(valid_modulus(17));
        assert!(valid_modulus(257));
        assert!(!valid_modulus(15));
        assert!(!valid_modulus(21));
    }

    proptest! {
        // r[verify shamir-field.closure]
        #[test]
        fn prop_field_ops_stay_in_range(
            prime_idx in 0usize..5,
            a in 0u64..300,
            b in 0u64..300,
        ) {
            let p = small_primes()[prime_idx];
            let a = a % p;
            let b = b % p;
            prop_assert!(field_add(a, b, p) < p);
            prop_assert!(field_sub(a, b, p) < p);
            prop_assert!(field_mul(a, b, p) < p);
        }

        // r[verify shamir-field.commutativity]
        #[test]
        fn prop_commutative(
            prime_idx in 0usize..5,
            a in 0u64..300,
            b in 0u64..300,
        ) {
            let p = small_primes()[prime_idx];
            let a = a % p;
            let b = b % p;
            prop_assert_eq!(field_add(a, b, p), field_add(b, a, p));
            prop_assert_eq!(field_mul(a, b, p), field_mul(b, a, p));
        }

        // r[verify shamir-field.subtraction]
        #[test]
        fn prop_subtraction_roundtrip(
            prime_idx in 0usize..5,
            a in 0u64..300,
            b in 0u64..300,
        ) {
            let p = small_primes()[prime_idx];
            let a = a % p;
            let b = b % p;
            let diff = field_sub(a, b, p);
            prop_assert_eq!(field_add(diff, b, p), a);
        }

        // r[verify shamir-field.multiplicative-inverse]
        #[test]
        fn prop_inverse_for_nonzero_small_primes(
            prime_idx in 0usize..5,
            a in 1u64..300,
        ) {
            let p = small_primes()[prime_idx];
            let a = a % p;
            if a != 0 {
                let inv = field_inv(a, p).expect("nonzero element should invert in tested prime field");
                prop_assert!(inv < p);
                prop_assert_eq!(field_mul(a, inv, p), 1);
            }
        }
    }
}

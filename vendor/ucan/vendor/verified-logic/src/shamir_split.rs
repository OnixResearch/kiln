//! Share generation for Shamir secret sharing.

use vstd::prelude::*;

use crate::shamir_field;
use crate::shamir_field::*;
use crate::shamir_poly;
use crate::shamir_poly::*;

pub type Share = (u64, u64);

verus! {

// r[depends shamir-split.parameter-validation]
/// Spec: split parameters are valid.
pub open spec fn split_params_valid_spec(random_coeffs: Seq<u64>, xs: Seq<u64>, p: u64) -> bool {
    &&& valid_modulus_spec(p)
    &&& random_coeffs.len() >= 1
    &&& random_coeffs.len() + 1 <= xs.len()
}

// r[depends shamir-split.constant-term-preservation]
// r[impl shamir-split.constant-term-preservation]
/// Spec: polynomial coefficients with the secret as constant term.
pub open spec fn split_coeffs_spec(secret: u64, random_coeffs: Seq<u64>) -> Seq<u64>
    recommends random_coeffs.len() >= 1
{
    seq![secret] + random_coeffs
}

// r[depends shamir-split.share-count]
/// Spec: split output from index `from` onward.
pub open spec fn split_from_spec(secret: u64, random_coeffs: Seq<u64>, xs: Seq<u64>, p: u64, from: int) -> Seq<Share>
    recommends
        split_params_valid_spec(random_coeffs, xs, p),
        secret < p,
        forall|i: int| #![trigger random_coeffs[i]] 0 <= i < random_coeffs.len() ==> random_coeffs[i] < p,
        forall|i: int| #![trigger xs[i]] 0 <= i < xs.len() ==> 0 < xs[i] < p,
        0 <= from <= xs.len(),
    decreases xs.len() - from
{
    if from >= xs.len() {
        seq![]
    } else {
        let coeffs = split_coeffs_spec(secret, random_coeffs);
        let share = (xs[from], poly_eval_sum_spec(coeffs, xs[from], p));
        seq![share] + split_from_spec(secret, random_coeffs, xs, p, from + 1)
    }
}

// r[depends shamir-split.share-count]
/// Spec: full split output.
pub open spec fn split_spec(secret: u64, random_coeffs: Seq<u64>, xs: Seq<u64>, p: u64) -> Seq<Share>
    recommends
        split_params_valid_spec(random_coeffs, xs, p),
        secret < p,
        forall|i: int| #![trigger random_coeffs[i]] 0 <= i < random_coeffs.len() ==> random_coeffs[i] < p,
        forall|i: int| #![trigger xs[i]] 0 <= i < xs.len() ==> 0 < xs[i] < p,
{
    split_from_spec(secret, random_coeffs, xs, p, 0)
}

// r[verify shamir-split.constant-term-preservation]
/// The constructed polynomial evaluates to the secret at x = 0.
pub proof fn split_constant_term_preserved(secret: u64, random_coeffs: Seq<u64>, p: u64)
    requires
        valid_modulus_spec(p),
        secret < p,
        random_coeffs.len() >= 1,
        forall|i: int| #![trigger random_coeffs[i]] 0 <= i < random_coeffs.len() ==> random_coeffs[i] < p,
    ensures poly_eval_sum_spec(split_coeffs_spec(secret, random_coeffs), 0, p) == secret
{
    poly_at_zero_is_constant(split_coeffs_spec(secret, random_coeffs), p);
}

proof fn split_from_count_and_xs(secret: u64, random_coeffs: Seq<u64>, xs: Seq<u64>, p: u64, from: int)
    requires
        split_params_valid_spec(random_coeffs, xs, p),
        secret < p,
        forall|i: int| #![trigger random_coeffs[i]] 0 <= i < random_coeffs.len() ==> random_coeffs[i] < p,
        forall|i: int| #![trigger xs[i]] 0 <= i < xs.len() ==> 0 < xs[i] < p,
        0 <= from <= xs.len(),
    ensures
        split_from_spec(secret, random_coeffs, xs, p, from).len() == xs.len() - from,
        forall|i: int| 0 <= i < xs.len() - from ==> {
            &&& split_from_spec(secret, random_coeffs, xs, p, from)[i].0 == xs[from + i]
            &&& split_from_spec(secret, random_coeffs, xs, p, from)[i].1 == poly_eval_sum_spec(split_coeffs_spec(secret, random_coeffs), xs[from + i], p)
        },
    decreases xs.len() - from
{
    if from < xs.len() {
        split_from_count_and_xs(secret, random_coeffs, xs, p, from + 1);
    }
}

// r[verify shamir-split.share-count]
/// Split preserves the requested x-coordinates and count.
pub proof fn split_count_and_xs(secret: u64, random_coeffs: Seq<u64>, xs: Seq<u64>, p: u64)
    requires
        split_params_valid_spec(random_coeffs, xs, p),
        secret < p,
        forall|i: int| #![trigger random_coeffs[i]] 0 <= i < random_coeffs.len() ==> random_coeffs[i] < p,
        forall|i: int| #![trigger xs[i]] 0 <= i < xs.len() ==> 0 < xs[i] < p,
    ensures
        split_spec(secret, random_coeffs, xs, p).len() == xs.len(),
        forall|i: int| 0 <= i < xs.len() ==> split_spec(secret, random_coeffs, xs, p)[i].0 == xs[i],
{
    split_from_count_and_xs(secret, random_coeffs, xs, p, 0);
}

fn xs_are_valid(xs: &[u64], p: u64) -> (result: bool)
    ensures result ==> forall|i: int| #![trigger xs@[i]] 0 <= i < xs@.len() ==> 0 < xs@[i] < p
{
    let mut i: usize = 0;
    while i < xs.len()
        invariant
            0 <= i <= xs.len(),
            forall|k: int| #![trigger xs@[k]] 0 <= k < i ==> 0 < xs@[k] < p,
        decreases xs.len() - i
    {
        if xs[i] == 0 || xs[i] >= p {
            return false;
        }
        let mut j: usize = 0;
        while j < i
            invariant 0 <= j <= i < xs.len()
            decreases i - j
        {
            if xs[j] == xs[i] {
                return false;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    true
}

// r[impl shamir-split.share-count]
// r[impl shamir-split.parameter-validation]
/// Generates shares by evaluating the secret polynomial at the requested x-coordinates.
pub fn split(secret: u64, random_coeffs: &[u64], xs: &[u64], p: u64) -> (result: Option<Vec<Share>>)
    ensures
        result.is_some() ==> {
            let shares = result.unwrap();
            &&& shares@.len() == xs@.len()
            &&& forall|i: int| 0 <= i < xs@.len() ==> shares@[i].0 == xs@[i]
        }
{
    if !shamir_field::valid_modulus(p) || secret >= p || random_coeffs.len() == 0 || random_coeffs.len() >= xs.len() {
        return None;
    }
    if !xs_are_valid(xs, p) {
        return None;
    }

    let mut coeffs: Vec<u64> = Vec::new();
    coeffs.push(secret);
    let mut i: usize = 0;
    while i < random_coeffs.len()
        invariant
            0 <= i <= random_coeffs@.len(),
            split_params_valid_spec(random_coeffs@, xs@, p),
            secret < p,
            coeffs@ == seq![secret] + random_coeffs@.subrange(0, i as int),
            forall|k: int| #![trigger random_coeffs@[k]] 0 <= k < i ==> random_coeffs@[k] < p,
        decreases random_coeffs@.len() - i
    {
        if random_coeffs[i] >= p {
            return None;
        }
        coeffs.push(random_coeffs[i]);
        i = i + 1;
    }

    proof {
        assert(forall|k: int| #![trigger random_coeffs@[k]] 0 <= k < random_coeffs@.len() ==> random_coeffs@[k] < p);
    }

    let mut shares: Vec<Share> = Vec::new();
    let mut j: usize = 0;
    while j < xs.len()
        invariant
            0 <= j <= xs@.len(),
            split_params_valid_spec(random_coeffs@, xs@, p),
            secret < p,
            forall|k: int| #![trigger random_coeffs@[k]] 0 <= k < random_coeffs@.len() ==> random_coeffs@[k] < p,
            forall|k: int| #![trigger xs@[k]] 0 <= k < xs@.len() ==> 0 < xs@[k] < p,
            coeffs@ == split_coeffs_spec(secret, random_coeffs@),
            shares@.len() == j,
            forall|k: int| #![trigger shares@[k]] 0 <= k < j ==> shares@[k].0 == xs@[k],
        decreases xs@.len() - j
    {
        let y = shamir_poly::poly_eval_horner(&coeffs, xs[j], p);
        shares.push((xs[j], y));
        proof {
            assert(shares@.len() == j as int + 1);
            assert(shares@[j as int].0 == xs@[j as int]);
            assert forall|k: int| #![trigger shares@[k]] 0 <= k < j + 1 implies shares@[k].0 == xs@[k] by {
                if k < j {
                } else {
                    assert(k == j);
                }
            }
        }
        j = j + 1;
    }
    proof {
        assert(j == xs@.len());
    }
    Some(shares)
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::shamir_poly::poly_eval_horner;

    // r[verify shamir-split.parameter-validation]
    #[test]
    fn test_split_rejects_bad_params() {
        assert!(split(5, &[], &[1, 2], 17).is_none());
        assert!(split(5, &[3, 4], &[1, 2], 17).is_none());
        assert!(split(5, &[3], &[0, 2], 17).is_none());
        assert!(split(5, &[3], &[1, 2], 15).is_none());
    }

    // r[verify shamir-split.share-count]
    #[test]
    fn test_split_count() {
        let shares = split(9, &[4, 7], &[1, 2, 3, 4], 17).unwrap();
        assert_eq!(shares.len(), 4);
        assert_eq!(shares[0].0, 1);
        assert_eq!(shares[1].0, 2);
    }

    // r[verify shamir-split.constant-term-preservation]
    #[test]
    fn test_poly_at_zero_is_secret() {
        let coeffs = vec![9u64, 4, 7];
        assert_eq!(poly_eval_horner(&coeffs, 0, 17), 9);
    }

    proptest! {
        // r[verify shamir-split.share-count]
        #[test]
        fn prop_split_count_matches_xs(
            secret in 0u64..17,
            a1 in 0u64..17,
            a2 in 0u64..17,
        ) {
            let xs = vec![1u64, 2, 3, 4, 5];
            let shares = split(secret, &[a1, a2], &xs, 17).unwrap();
            prop_assert_eq!(shares.len(), xs.len());
            for (share, x) in shares.iter().zip(xs.iter()) {
                prop_assert_eq!(share.0, *x);
            }
        }

        // r[verify shamir-split.constant-term-preservation]
        #[test]
        fn prop_shares_match_poly_eval(
            secret in 0u64..17,
            a1 in 0u64..17,
            a2 in 0u64..17,
        ) {
            let xs = vec![1u64, 2, 3, 4];
            let shares = split(secret, &[a1, a2], &xs, 17).unwrap();
            let coeffs = vec![secret, a1, a2];
            for (x, y) in shares {
                prop_assert_eq!(y, poly_eval_horner(&coeffs, x, 17));
            }
        }
    }
}

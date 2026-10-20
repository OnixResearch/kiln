//! Secret reconstruction from Shamir shares.

use vstd::prelude::*;

use crate::shamir_field::*;
use crate::shamir_lagrange;
use crate::shamir_lagrange::*;
use crate::shamir_split::Share;

verus! {

// r[depends shamir-reconstruct.correctness]
/// Spec: x-values of the first `k` shares.
pub open spec fn share_xs_prefix_spec(shares: Seq<Share>, k: int) -> Seq<u64>
    recommends 0 <= k <= shares.len()
{
    Seq::new(k as nat, |i: int| shares[i].0)
}

// r[depends shamir-reconstruct.correctness]
/// Spec: y-values of the first `k` shares.
pub open spec fn share_ys_prefix_spec(shares: Seq<Share>, k: int) -> Seq<u64>
    recommends 0 <= k <= shares.len()
{
    Seq::new(k as nat, |i: int| shares[i].1)
}

// r[impl shamir-reconstruct.correctness]
// r[impl shamir-reconstruct.insufficient-shares]
/// Reconstructs a secret from the first `k` shares.
pub fn reconstruct(shares: &[Share], k: usize, p: u64) -> (result: Option<u64>)
    ensures result.is_some() ==> result.unwrap() < p
{
    if !crate::shamir_field::valid_modulus(p) || k < 2 || shares.len() < k {
        return None;
    }

    let mut xs: Vec<u64> = Vec::new();
    let mut ys: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < k
        invariant
            0 <= i <= k,
            k <= shares@.len(),
            xs@ == share_xs_prefix_spec(shares@, i as int),
            ys@ == share_ys_prefix_spec(shares@, i as int),
            forall|j: int| #![trigger xs@[j]] 0 <= j < xs@.len() ==> 0 < xs@[j] < p,
            forall|j: int| #![trigger ys@[j]] 0 <= j < ys@.len() ==> ys@[j] < p,
        decreases k - i
    {
        let (x, y) = shares[i];
        if x == 0 || x >= p || y >= p {
            return None;
        }
        xs.push(x);
        ys.push(y);
        i = i + 1;
    }

    shamir_lagrange::lagrange_at_zero(&xs, &ys, p)
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::shamir_split::split;

    // r[verify shamir-reconstruct.correctness]
    #[test]
    fn test_reconstruct_two_of_two() {
        let shares = split(9, &[4], &[1, 3], 17).unwrap();
        assert_eq!(reconstruct(&shares, 2, 17), Some(9));
    }

    // r[verify shamir-reconstruct.correctness]
    #[test]
    fn test_reconstruct_three_of_five_subset() {
        let shares = split(6, &[3, 8], &[1, 2, 3, 4, 5], 17).unwrap();
        let subset = vec![shares[0], shares[2], shares[4]];
        assert_eq!(reconstruct(&subset, 3, 17), Some(6));
    }

    // r[verify shamir-reconstruct.insufficient-shares]
    #[test]
    fn test_insufficient_shares_usually_fail() {
        let shares = split(6, &[3, 8], &[1, 2, 3, 4, 5], 17).unwrap();
        assert_ne!(reconstruct(&shares[..2], 2, 17), Some(6));
    }

    proptest! {
        // r[verify shamir-reconstruct.correctness]
        #[test]
        fn prop_roundtrip_three_of_five(
            secret in 0u64..17,
            a1 in 0u64..17,
            a2 in 0u64..17,
            pick in 0usize..10,
        ) {
            let all = split(secret, &[a1, a2], &[1, 2, 3, 4, 5], 17).unwrap();
            let subsets = [
                vec![all[0], all[1], all[2]],
                vec![all[0], all[1], all[3]],
                vec![all[0], all[1], all[4]],
                vec![all[0], all[2], all[3]],
                vec![all[0], all[2], all[4]],
                vec![all[0], all[3], all[4]],
                vec![all[1], all[2], all[3]],
                vec![all[1], all[2], all[4]],
                vec![all[1], all[3], all[4]],
                vec![all[2], all[3], all[4]],
            ];
            let subset = &subsets[pick];
            prop_assert_eq!(reconstruct(subset, 3, 17), Some(secret));
        }

        // r[verify shamir-reconstruct.insufficient-shares]
        #[test]
        fn prop_one_share_short_rarely_matches(
            secret in 0u64..17,
            a1 in 0u64..17,
            a2 in 0u64..17,
        ) {
            let all = split(secret, &[a1, a2], &[1, 2, 3, 4, 5], 17).unwrap();
            let guess = reconstruct(&all[..2], 2, 17);
            if guess == Some(secret) {
                prop_assert!(a2 == 0);
            } else {
                prop_assert_ne!(guess, Some(secret));
            }
        }
    }
}

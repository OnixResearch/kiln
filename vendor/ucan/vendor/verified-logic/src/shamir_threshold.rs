//! Threshold-security witnesses for Shamir secret sharing.

use vstd::prelude::*;

use crate::shamir_lagrange::*;

verus! {

// r[depends shamir-threshold.information-theoretic-security]
// r[impl shamir-threshold.information-theoretic-security]
// r[impl shamir-threshold.perfect-reconstruction-threshold]
/// Spec: augment the visible shares with `(0, secret)`.
pub open spec fn threshold_completion_spec(secret: u64, xs: Seq<u64>, ys: Seq<u64>, p: u64) -> bool {
    let aug_xs = seq![0u64] + xs;
    let aug_ys = seq![secret] + ys;
    &&& valid_interp_points_spec(aug_xs, p)
    &&& ys.len() == xs.len()
    &&& forall|i: int| 0 <= i < ys.len() ==> ys[i] < p
    &&& lagrange_eval_spec(aug_xs, aug_ys, 0, p) == secret
    &&& forall|i: int| 0 <= i < xs.len() ==> lagrange_eval_spec(aug_xs, aug_ys, xs[i], p) == ys[i]
}

// r[verify shamir-threshold.information-theoretic-security]
/// Any visible `k-1` shares can be completed with any target secret.
pub proof fn threshold_completion_exists(secret: u64, xs: Seq<u64>, ys: Seq<u64>, p: u64)
    requires
        ys.len() == xs.len(),
        secret < p,
        forall|i: int| 0 <= i < ys.len() ==> ys[i] < p,
        valid_interp_points_spec(seq![0u64] + xs, p),
    ensures threshold_completion_spec(secret, xs, ys, p)
{
    let aug_xs = seq![0u64] + xs;
    let aug_ys = seq![secret] + ys;
    lagrange_eval_at_point(aug_xs, aug_ys, 0, p);
    assert forall|i: int| 0 <= i < xs.len() implies lagrange_eval_spec(aug_xs, aug_ys, xs[i], p) == ys[i] by {
        lagrange_eval_at_point(aug_xs, aug_ys, i + 1, p);
    }
}

// r[verify shamir-threshold.perfect-reconstruction-threshold]
/// Once the secret point is added, the completed value vector is unique.
pub proof fn threshold_completion_unique(secret1: u64, secret2: u64, xs: Seq<u64>, ys: Seq<u64>, p: u64)
    requires
        ys.len() == xs.len(),
        secret1 < p,
        secret2 < p,
        forall|i: int| 0 <= i < ys.len() ==> ys[i] < p,
        valid_interp_points_spec(seq![0u64] + xs, p),
        lagrange_eval_spec(seq![0u64] + xs, seq![secret1] + ys, 0, p)
            == lagrange_eval_spec(seq![0u64] + xs, seq![secret2] + ys, 0, p),
    ensures secret1 == secret2
{
    assert(lagrange_eval_spec(seq![0u64] + xs, seq![secret1] + ys, 0, p) == secret1) by {
        lagrange_eval_at_point(seq![0u64] + xs, seq![secret1] + ys, 0, p);
    }
    assert(lagrange_eval_spec(seq![0u64] + xs, seq![secret2] + ys, 0, p) == secret2) by {
        lagrange_eval_at_point(seq![0u64] + xs, seq![secret2] + ys, 0, p);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shamir_reconstruct::reconstruct;
    use crate::shamir_split::split;

    // r[verify shamir-threshold.information-theoretic-security]
    #[test]
    fn test_single_share_all_secrets_possible() {
        let p = 17u64;
        let share = (3u64, 8u64);
        for secret in 0..p {
            let slope = ((share.1 + p) - secret) * 6 % p; // inv(3) == 6 mod 17
            let second = (1u64, (secret + slope) % p);
            let shares = vec![second, share];
            assert_eq!(reconstruct(&shares, 2, p), Some(secret));
        }
    }

    // r[verify shamir-threshold.perfect-reconstruction-threshold]
    #[test]
    fn test_three_shares_fix_secret() {
        let shares = split(9, &[4, 7], &[1, 2, 3, 4], 17).unwrap();
        let subset = vec![shares[0], shares[1], shares[3]];
        assert_eq!(reconstruct(&subset, 3, 17), Some(9));
    }
}

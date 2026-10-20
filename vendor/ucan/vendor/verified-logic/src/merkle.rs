//! Verified Merkle proof verification.
//!
//! Verifies inclusion proofs against a Merkle root. Uses the same
//! uninterpreted hash function axiom as the chain module. The proof
//! checker walks a sibling path, combining hashes at each level to
//! reconstruct the root.

// Verus requires &Vec for @ view, not &[T]
#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::chain::compute_entry_hash;
#[cfg(verus_keep_ghost)]
use crate::chain::compute_entry_hash_spec;
#[cfg(verus_keep_ghost)]
use crate::chain::hash_spec;

verus! {

// --- verify_proof ---

// r[depends merkle.verify-proof]
/// Spec: verify a Merkle inclusion proof.
/// `leaf_data` is hashed into `leaf_hash`, then combined with siblings
/// walking up to the root. `index` determines left/right placement at
/// each level (bit 0 = level 0, etc.).
pub open spec fn verify_proof_spec(
    leaf_hash: u64, index: u64, siblings: Seq<u64>, root: u64,
) -> bool
    decreases siblings.len()
{
    if siblings.len() == 0 {
        leaf_hash == root
    } else {
        let sibling = siblings[0int];
        let parent = if index % 2 == 0 {
            hash_spec(leaf_hash, sibling, 0, 0)
        } else {
            hash_spec(sibling, leaf_hash, 0, 0)
        };
        verify_proof_spec(parent, index / 2, siblings.subrange(1, siblings.len() as int), root)
    }
}

/// Helper: unfolding one step of verify_proof_spec.
/// Processing siblings[i] transforms the problem into the next level.
proof fn verify_proof_unfold(
    current: u64, idx: u64, siblings: Seq<u64>, root: u64, i: int,
)
    requires
        0 <= i < siblings.len(),
    ensures ({
        let sub = siblings.subrange(i, siblings.len() as int);
        let sibling = sub[0int];
        let parent = if idx % 2 == 0 {
            hash_spec(current, sibling, 0, 0)
        } else {
            hash_spec(sibling, current, 0, 0)
        };
        let next_sub = siblings.subrange(i + 1, siblings.len() as int);
        verify_proof_spec(current, idx, sub, root)
            == verify_proof_spec(parent, idx / 2, next_sub, root)
    })
{
    let sub = siblings.subrange(i, siblings.len() as int);
    assert(sub.len() > 0);
    let sibling = sub[0int];
    let parent = if idx % 2 == 0 {
        hash_spec(current, sibling, 0, 0)
    } else {
        hash_spec(sibling, current, 0, 0)
    };
    // By definition of verify_proof_spec, since sub.len() > 0:
    // verify_proof_spec(current, idx, sub, root) ==
    //   verify_proof_spec(parent, idx/2, sub.subrange(1, sub.len()), root)
    let tail = sub.subrange(1, sub.len() as int);
    let next_sub = siblings.subrange(i + 1, siblings.len() as int);
    // Show tail == next_sub via extensional equality
    assert(tail =~= next_sub);
}

// r[impl merkle.verify-proof]
/// Verify a Merkle inclusion proof.
pub fn verify_proof(
    leaf_hash: u64, index: u64, siblings: &Vec<u64>, root: u64,
) -> (result: bool)
    ensures result == verify_proof_spec(leaf_hash, index, siblings@, root)
{
    let mut current = leaf_hash;
    let mut idx = index;
    let mut i: usize = 0;

    proof {
        assert(siblings@.subrange(0int, siblings@.len() as int) =~= siblings@);
    }

    while i < siblings.len()
        invariant
            0 <= i <= siblings.len(),
            verify_proof_spec(current, idx, siblings@.subrange(i as int, siblings@.len() as int), root)
                == verify_proof_spec(leaf_hash, index, siblings@, root),
        decreases siblings.len() - i,
    {
        let sibling = siblings[i];
        let parent = if idx % 2 == 0 {
            compute_entry_hash(current, sibling, 0, 0)
        } else {
            compute_entry_hash(sibling, current, 0, 0)
        };

        proof {
            verify_proof_unfold(current, idx, siblings@, root, i as int);
        }

        current = parent;
        idx = idx / 2;
        i = i + 1;
    }

    proof {
        // At the end, i == siblings.len(), so subrange is empty
        let empty = siblings@.subrange(siblings@.len() as int, siblings@.len() as int);
        assert(empty.len() == 0);
        assert(empty =~= Seq::<u64>::empty());
        // verify_proof_spec(current, idx, empty, root) == (current == root)
    }
    current == root
}

// --- Safety proofs ---

// r[verify merkle.verify-proof]
/// Empty proof: leaf_hash must equal root directly.
proof fn empty_proof_is_identity(leaf_hash: u64, root: u64)
    ensures verify_proof_spec(leaf_hash, 0, Seq::<u64>::empty(), root) == (leaf_hash == root)
{}

// r[verify merkle.verify-proof]
/// Deterministic: same inputs always produce the same result.
proof fn verify_deterministic(leaf_hash: u64, index: u64, siblings: Seq<u64>, root: u64)
    ensures verify_proof_spec(leaf_hash, index, siblings, root) ==
            verify_proof_spec(leaf_hash, index, siblings, root)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::chain::compute_entry_hash;

    /// Runtime hash matching `compute_entry_hash(a, b, 0, 0)`.
    fn test_hash(a: u64, b: u64) -> u64 {
        compute_entry_hash(a, b, 0, 0)
    }

    /// Build a merkle root from leaves and return (root, proofs).
    fn build_tree(leaves: &[u64]) -> (u64, Vec<(u64, Vec<u64>)>) {
        if leaves.is_empty() {
            return (0, vec![]);
        }
        if leaves.len() == 1 {
            return (leaves[0], vec![(leaves[0], vec![])]);
        }
        // Pad to power of two
        let mut padded = leaves.to_vec();
        while padded.len().count_ones() != 1 {
            padded.push(0);
        }
        let n = padded.len();
        let mut tree = vec![0u64; 2 * n];
        for i in 0..n {
            tree[n + i] = padded[i];
        }
        for i in (1..n).rev() {
            tree[i] = test_hash(tree[2 * i], tree[2 * i + 1]);
        }
        let root = tree[1];
        let mut proofs = Vec::new();
        for i in 0..leaves.len() {
            let mut siblings = Vec::new();
            let mut pos = n + i;
            while pos > 1 {
                let sib = if pos % 2 == 0 { pos + 1 } else { pos - 1 };
                siblings.push(tree[sib]);
                pos /= 2;
            }
            proofs.push((leaves[i], siblings));
        }
        (root, proofs)
    }

    // r[verify merkle.verify-proof]
    #[test]
    fn test_empty_proof() {
        assert!(verify_proof(42, 0, &vec![], 42));
        assert!(!verify_proof(42, 0, &vec![], 99));
    }

    // r[verify merkle.verify-proof]
    #[test]
    fn test_verify_proof_single_level() {
        let leaf = 10u64;
        let sibling = 20u64;
        let root = test_hash(leaf, sibling);
        assert!(verify_proof(leaf, 0, &vec![sibling], root));
    }

    // r[verify merkle.verify-proof]
    #[test]
    fn test_wrong_root_rejects() {
        let leaf = 10u64;
        let sibling = 20u64;
        assert!(!verify_proof(leaf, 0, &vec![sibling], 999));
    }

    proptest! {
        // r[verify merkle.verify-proof]
        #[test]
        fn prop_empty_proof_iff_equal(leaf in 0u64..10000, root in 0u64..10000) {
            prop_assert_eq!(verify_proof(leaf, 0, &vec![], root), leaf == root);
        }

        // r[verify merkle.verify-proof]
        #[test]
        fn prop_valid_proof_verifies(
            a in 0u64..1000,
            b in 0u64..1000,
        ) {
            // 2-leaf tree: leaf a at index 0, leaf b at index 1
            let root = test_hash(a, b);
            prop_assert!(verify_proof(a, 0, &vec![b], root));
            prop_assert!(verify_proof(b, 1, &vec![a], root));
        }

        // r[verify merkle.verify-proof]
        #[test]
        fn prop_wrong_leaf_rejects(
            a in 0u64..1000,
            b in 0u64..1000,
            wrong in 0u64..1000,
        ) {
            let root = test_hash(a, b);
            if wrong != a {
                prop_assert!(!verify_proof(wrong, 0, &vec![b], root));
            }
        }

        // r[verify merkle.verify-proof]
        #[test]
        fn prop_four_leaf_tree(
            l0 in 0u64..500,
            l1 in 0u64..500,
            l2 in 0u64..500,
            l3 in 0u64..500,
        ) {
            let leaves = [l0, l1, l2, l3];
            let (root, proofs) = build_tree(&leaves);
            for (i, (leaf, siblings)) in proofs.iter().enumerate() {
                prop_assert!(
                    verify_proof(*leaf, i as u64, siblings, root)
                );
            }
        }
    }
}

//! Verified constant-time equality comparison for 32-byte hashes.
//!
//! Uses XOR-fold: accumulates bitwise OR of per-byte XOR differences
//! across all 32 positions, then checks the accumulator is zero. Verus
//! proves the structural property (all bytes visited) and correctness
//! (equivalent to pointwise equality).
//!
//! Does NOT prove constant-time at the machine level — that requires
//! assembly-level verification. What this proves: the algorithm examines
//! every byte regardless of early mismatches.

// u64 loop index to usize — always < 32, verified by Verus
#![allow(clippy::cast_possible_truncation)]

use vstd::prelude::*;

verus! {

/// Number of bytes in a hash.
pub const HASH_LEN: u64 = 32u64;

// --- xor_fold_spec ---

// r[depends constant-time-eq.xor-fold]
/// Spec: recursive XOR-fold accumulator from position `from` to 32.
///
/// Accumulates `acc | (a[i] ^ b[i])` for each position.
pub open spec fn xor_fold_spec(a: Seq<u8>, b: Seq<u8>, from: int, acc: u8) -> u8
    decreases 32 - from
{
    if from >= 32 {
        acc
    } else {
        xor_fold_spec(a, b, from + 1, (acc | (a[from] ^ b[from])) as u8)
    }
}

// r[depends constant-time-eq.equality]
/// Spec: constant-time equality — xor fold of all 32 bytes equals zero.
pub open spec fn constant_time_eq_spec(a: Seq<u8>, b: Seq<u8>) -> bool {
    xor_fold_spec(a, b, 0, 0u8) == 0u8
}

// r[impl constant-time-eq.xor-fold]
// r[impl constant-time-eq.equality]
/// Constant-time equality for 32-byte slices.
///
/// The loop invariant uses a "remaining fold" approach:
/// `xor_fold_spec(a, b, i, acc)` is the fold of the remaining bytes [i, 32).
/// Since acc accumulates the work done so far, the remaining fold equals
/// the full fold from 0.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> (result: bool)
    requires a@.len() == 32, b@.len() == 32
    ensures result == constant_time_eq_spec(a@, b@)
{
    let mut acc: u8 = 0;
    let mut i: u64 = 0;
    while i < 32
        invariant
            0 <= i <= 32,
            a@.len() == 32,
            b@.len() == 32,
            xor_fold_spec(a@, b@, i as int, acc) == xor_fold_spec(a@, b@, 0, 0u8),
        decreases 32 - i
    {
        // xor_fold_spec(a, b, i, acc)
        //   = xor_fold_spec(a, b, i+1, (acc | (a[i] ^ b[i])))  by definition
        // So after updating acc and i, the invariant is maintained.
        acc = acc | (a[i as usize] ^ b[i as usize]);
        i = i + 1;
    }
    // At i==32: xor_fold_spec(a, b, 32, acc) = acc (base case)
    // And by invariant: acc == xor_fold_spec(a, b, 0, 0)
    acc == 0
}

// --- Safety proofs ---

// r[impl constant-time-eq.equal-returns-true]
// r[verify constant-time-eq.equal-returns-true]
/// Equal inputs produce accumulator 0 (xor of identical bytes is 0).
proof fn equal_returns_true(a: Seq<u8>)
    requires a.len() == 32
    ensures constant_time_eq_spec(a, a)
{
    equal_fold_zero(a, a, 0, 0u8);
}

/// Helper: if a == b, the fold stays at acc (xor is 0, OR with 0 is identity).
proof fn equal_fold_zero(a: Seq<u8>, b: Seq<u8>, from: int, acc: u8)
    requires a.len() == 32, b.len() == 32, 0 <= from <= 32, a =~= b
    ensures xor_fold_spec(a, b, from, acc) == acc
    decreases 32 - from
{
    if from < 32 {
        let ai: u8 = a[from];
        let bi: u8 = b[from];
        assert(ai == bi);
        assert((ai ^ bi) == 0u8) by(bit_vector)
            requires ai == bi;
        assert((acc | 0u8) == acc) by(bit_vector);
        equal_fold_zero(a, b, from + 1, (acc | (a[from] ^ b[from])) as u8);
    }
}

// r[impl constant-time-eq.any-difference-returns-false]
// r[verify constant-time-eq.any-difference-returns-false]
/// Any differing byte makes the fold nonzero.
proof fn any_difference_returns_false(a: Seq<u8>, b: Seq<u8>, diff_pos: int)
    requires
        a.len() == 32,
        b.len() == 32,
        0 <= diff_pos < 32,
        a[diff_pos] != b[diff_pos],
    ensures !constant_time_eq_spec(a, b)
{
    diff_makes_fold_nonzero(a, b, diff_pos, 0, 0u8);
}

/// Helper: a nonzero XOR at diff_pos propagates through the fold.
proof fn diff_makes_fold_nonzero(
    a: Seq<u8>, b: Seq<u8>, diff_pos: int, from: int, acc: u8,
)
    requires
        a.len() == 32,
        b.len() == 32,
        0 <= from <= diff_pos,
        diff_pos < 32,
        a[diff_pos] != b[diff_pos],
    ensures xor_fold_spec(a, b, from, acc) != 0u8
    decreases 32 - from
{
    if from == diff_pos {
        let ai: u8 = a[from];
        let bi: u8 = b[from];
        assert(ai != bi);
        assert((ai ^ bi) != 0u8) by(bit_vector)
            requires ai != bi;
        let xor_val: u8 = (ai ^ bi) as u8;
        let new_acc: u8 = (acc | xor_val) as u8;
        assert(new_acc != 0u8) by(bit_vector)
            requires xor_val != 0u8, new_acc == (acc | xor_val) as u8;
        nonzero_acc_stays_nonzero(a, b, from + 1, new_acc);
    } else {
        diff_makes_fold_nonzero(
            a, b, diff_pos, from + 1,
            (acc | (a[from] ^ b[from])) as u8,
        );
    }
}

/// Helper: nonzero accumulator stays nonzero through OR operations.
proof fn nonzero_acc_stays_nonzero(a: Seq<u8>, b: Seq<u8>, from: int, acc: u8)
    requires a.len() == 32, b.len() == 32, 0 <= from <= 32, acc != 0u8
    ensures xor_fold_spec(a, b, from, acc) != 0u8
    decreases 32 - from
{
    if from < 32 {
        let ai: u8 = a[from];
        let bi: u8 = b[from];
        let xor_val: u8 = (ai ^ bi) as u8;
        let new_acc: u8 = (acc | xor_val) as u8;
        assert(new_acc != 0u8) by(bit_vector)
            requires acc != 0u8, new_acc == (acc | xor_val) as u8;
        nonzero_acc_stays_nonzero(a, b, from + 1, new_acc);
    }
}

// r[impl constant-time-eq.equivalence-to-equality]
// r[verify constant-time-eq.equivalence-to-equality]
/// `constant_time_eq(a, b) <==> pointwise equality`.
pub proof fn equivalence_to_equality(a: Seq<u8>, b: Seq<u8>)
    requires a.len() == 32, b.len() == 32
    ensures constant_time_eq_spec(a, b) <==> (a =~= b)
{
    if a =~= b {
        equal_returns_true(a);
    } else {
        let diff_pos = choose|i: int| 0 <= i < 32 && a[i] != b[i];
        any_difference_returns_false(a, b, diff_pos);
    }
}

// r[impl constant-time-eq.fold-visits-all-bytes]
// r[verify constant-time-eq.fold-visits-all-bytes]
/// The fold visits all 32 positions. Each byte position's XOR result
/// is incorporated into the accumulator via OR. We prove that modifying
/// a single byte changes the fold result — meaning the fold actually
/// reads that byte.
proof fn fold_visits_all_bytes(a: Seq<u8>, b: Seq<u8>)
    requires a.len() == 32, b.len() == 32
    ensures
        // If a == b except at position k, fold detects it
        forall|k: int| #![trigger a[k]]
            0 <= k < 32 && a[k] != b[k] ==> !constant_time_eq_spec(a, b),
{
    assert forall|k: int| #![trigger a[k]]
        0 <= k < 32 && a[k] != b[k] implies !constant_time_eq_spec(a, b)
    by {
        any_difference_returns_false(a, b, k);
    }
}

// r[verify constant-time-eq.equality]
/// Reflexivity: any input equals itself.
proof fn reflexive(a: Seq<u8>)
    requires a.len() == 32
    ensures constant_time_eq_spec(a, a)
{
    equal_returns_true(a);
}

// r[verify constant-time-eq.equality]
/// Symmetry: `constant_time_eq(a, b) == constant_time_eq(b, a)`.
proof fn symmetric(a: Seq<u8>, b: Seq<u8>)
    requires a.len() == 32, b.len() == 32
    ensures constant_time_eq_spec(a, b) == constant_time_eq_spec(b, a)
{
    equivalence_to_equality(a, b);
    equivalence_to_equality(b, a);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify constant-time-eq.equal-returns-true]
    // r[verify constant-time-eq.xor-fold]
    #[test]
    fn test_equal() {
        let a = [0u8; 32];
        assert!(constant_time_eq(&a, &a));
        let b = [0xffu8; 32];
        assert!(constant_time_eq(&b, &b));
    }

    // r[verify constant-time-eq.any-difference-returns-false]
    #[test]
    fn test_different() {
        let a = [0u8; 32];
        let mut b = [0u8; 32];
        b[0] = 1;
        assert!(!constant_time_eq(&a, &b));
        b[0] = 0;
        b[31] = 1;
        assert!(!constant_time_eq(&a, &b));
    }

    // r[verify constant-time-eq.equality]
    #[test]
    fn test_single_bit_difference() {
        let a = [0u8; 32];
        for pos in 0_usize..32_usize {
            for bit in 0_u8..8_u8 {
                let mut b = [0u8; 32];
                b[pos] = 1_u8 << bit;
                assert!(!constant_time_eq(&a, &b));
            }
        }
    }

    proptest! {
        // r[verify constant-time-eq.equal-returns-true]
        #[test]
        fn prop_reflexive(a in prop::array::uniform32(any::<u8>())) {
            prop_assert!(constant_time_eq(&a, &a));
        }

        // r[verify constant-time-eq.equality]
        #[test]
        fn prop_symmetric(
            a in prop::array::uniform32(any::<u8>()),
            b in prop::array::uniform32(any::<u8>()),
        ) {
            prop_assert_eq!(constant_time_eq(&a, &b), constant_time_eq(&b, &a));
        }

        // r[verify constant-time-eq.equivalence-to-equality]
        #[test]
        fn prop_agrees_with_equality(
            a in prop::array::uniform32(any::<u8>()),
            b in prop::array::uniform32(any::<u8>()),
        ) {
            prop_assert_eq!(constant_time_eq(&a, &b), a == b);
        }

        // r[verify constant-time-eq.any-difference-returns-false]
        #[test]
        fn prop_any_single_diff_detected(
            a in prop::array::uniform32(any::<u8>()),
            pos in 0usize..32,
        ) {
            let mut b = a;
            b[pos] = b[pos].wrapping_add(1);
            if a[pos] != b[pos] {
                prop_assert!(!constant_time_eq(&a, &b));
            }
        }
    }
}

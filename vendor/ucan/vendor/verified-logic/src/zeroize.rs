//! Verified memory scrubbing for sensitive data.
//!
//! Writes zero to every byte in a buffer and proves the result is
//! all-zeros. Pairs with `constant_time_eq` — scrub the secret, then
//! verify the scrub. The loop visits every position regardless of
//! content (no early exit).

use vstd::prelude::*;

verus! {

// ============================================================
// zeroize: write zero to every byte
// ============================================================

// r[depends zeroize.scrub]
/// Spec: a zeroed buffer of length n.
pub open spec fn zeroed_spec(n: nat) -> Seq<u8> {
    Seq::new(n, |_i: int| 0u8)
}

// r[depends zeroize.scrub]
/// Spec: zeroize produces a zero-filled sequence of the same length.
pub open spec fn zeroize_spec(buf: Seq<u8>) -> Seq<u8> {
    zeroed_spec(buf.len())
}

// r[impl zeroize.scrub]
/// Writes zero to every byte in the buffer.
pub fn zeroize(buf: &mut Vec<u8>)
    ensures
        buf@.len() == old(buf)@.len(),
        buf@ =~= zeroize_spec(old(buf)@),
{
    let len = buf.len();
    let mut i: usize = 0;
    while i < len
        invariant
            len == old(buf)@.len(),
            buf@.len() == len,
            i <= len,
            forall|j: int| 0 <= j < i as int ==> buf@[j] == 0u8,
            forall|j: int| i as int <= j < len as int ==> buf@[j] == old(buf)@[j],
        decreases len - i,
    {
        buf.set(i, 0u8);
        i = i + 1;
    }
    assert(buf@ =~= zeroed_spec(old(buf)@.len()));
}

// ============================================================
// is_zeroed: check that every byte is zero
// ============================================================

// r[depends zeroize.detects-nonzero]
/// Spec: every byte in the buffer is zero.
pub open spec fn is_zeroed_spec(buf: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < buf.len() ==> buf[i] == 0u8
}

// r[impl zeroize.detects-nonzero]
/// Returns true if every byte in the buffer is zero.
pub fn is_zeroed(buf: &[u8]) -> (result: bool)
    ensures result == is_zeroed_spec(buf@),
{
    let mut i: usize = 0;
    while i < buf.len()
        invariant
            i <= buf.len(),
            forall|j: int| 0 <= j < i as int ==> buf@[j] == 0u8,
        decreases buf.len() - i,
    {
        if buf[i] != 0u8 {
            return false;
        }
        i = i + 1;
    }
    true
}

// ============================================================
// Proofs
// ============================================================

// r[verify zeroize.scrub]
/// Zeroize produces all zeros.
proof fn zeroize_all_zero(buf: Seq<u8>)
    ensures forall|i: int| 0 <= i < buf.len() ==> zeroize_spec(buf)[i] == 0u8,
{
    // By definition: zeroed_spec(n)[i] == 0 for all valid i.
}

// r[verify zeroize.scrub]
/// Zeroize preserves length.
proof fn zeroize_length_preserving(buf: Seq<u8>)
    ensures zeroize_spec(buf).len() == buf.len(),
{
    // zeroed_spec(buf.len()).len() == buf.len() by Seq::new.
}

// r[verify zeroize.idempotent]
/// Zeroizing an already-zeroed buffer changes nothing.
proof fn zeroize_idempotent(buf: Seq<u8>)
    ensures zeroize_spec(zeroize_spec(buf)) =~= zeroize_spec(buf),
{
    // zeroize(zeroize(buf)) = zeroed(zeroed(n).len()) = zeroed(n) = zeroize(buf).
}

// r[verify zeroize.detects-nonzero]
/// is_zeroed returns true on a zeroized buffer.
proof fn zeroize_then_check(buf: Seq<u8>)
    ensures is_zeroed_spec(zeroize_spec(buf)),
{
    // zeroize_spec(buf) = zeroed_spec(n), every element is 0.
}

// r[verify zeroize.detects-nonzero]
/// is_zeroed returns false if any byte is nonzero.
proof fn nonzero_detected(buf: Seq<u8>, pos: int)
    requires 0 <= pos < buf.len(), buf[pos] != 0u8,
    ensures !is_zeroed_spec(buf),
{
    // Witness: buf[pos] != 0 contradicts the forall.
}

// r[verify zeroize.scrub]
/// A zeroed buffer of length n equals any other zeroed buffer of length n.
proof fn zeroed_unique(n: nat)
    ensures forall|a: Seq<u8>, b: Seq<u8>|
        a.len() == n && b.len() == n
        && is_zeroed_spec(a) && is_zeroed_spec(b)
        ==> a =~= b,
{}

// r[impl zeroize.idempotent]
// r[verify zeroize.idempotent]
/// Zeroize applied to zeroed_spec is identity.
proof fn zeroize_zeroed_identity(n: nat)
    ensures zeroize_spec(zeroed_spec(n)) =~= zeroed_spec(n),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify zeroize.scrub]
    #[test]
    fn test_zeroize_basic() {
        let mut buf = vec![1u8, 2, 3, 4, 5];
        zeroize(&mut buf);
        assert_eq!(buf, vec![0u8; 5]);
    }

    // r[verify zeroize.scrub]
    #[test]
    fn test_zeroize_empty() {
        let mut buf: Vec<u8> = vec![];
        zeroize(&mut buf);
        assert!(buf.is_empty());
    }

    // r[verify zeroize.scrub]
    #[test]
    fn test_zeroize_all_ff() {
        let mut buf = vec![0xffu8; 32];
        zeroize(&mut buf);
        assert_eq!(buf, vec![0u8; 32]);
    }

    // r[verify zeroize.detects-nonzero]
    #[test]
    fn test_is_zeroed_true() {
        assert!(is_zeroed(&[0u8; 32]));
        assert!(is_zeroed(&[]));
    }

    // r[verify zeroize.detects-nonzero]
    #[test]
    fn test_is_zeroed_false() {
        assert!(!is_zeroed(&[0, 0, 1]));
        assert!(!is_zeroed(&[1]));
    }

    // r[verify zeroize.idempotent]
    #[test]
    fn test_zeroize_idempotent() {
        let mut buf = vec![42u8; 16];
        zeroize(&mut buf);
        let snapshot = buf.clone();
        zeroize(&mut buf);
        assert_eq!(buf, snapshot);
    }

    proptest! {
        // r[verify zeroize.scrub]
        #[test]
        fn prop_zeroize_all_zero(buf in proptest::collection::vec(any::<u8>(), 0..256)) {
            let mut v = buf;
            zeroize(&mut v);
            prop_assert!(v.iter().all(|&b| b == 0));
        }

        // r[verify zeroize.scrub]
        #[test]
        fn prop_zeroize_preserves_length(buf in proptest::collection::vec(any::<u8>(), 0..256)) {
            let orig_len = buf.len();
            let mut v = buf;
            zeroize(&mut v);
            prop_assert_eq!(v.len(), orig_len);
        }

        // r[verify zeroize.detects-nonzero]
        #[test]
        fn prop_zeroize_then_is_zeroed(buf in proptest::collection::vec(any::<u8>(), 0..256)) {
            let mut v = buf;
            zeroize(&mut v);
            prop_assert!(is_zeroed(&v));
        }

        // r[verify zeroize.idempotent]
        #[test]
        fn prop_zeroize_idempotent(buf in proptest::collection::vec(any::<u8>(), 0..256)) {
            let mut v = buf;
            zeroize(&mut v);
            let first = v.clone();
            zeroize(&mut v);
            prop_assert_eq!(v, first);
        }

        // r[verify zeroize.detects-nonzero]
        #[test]
        fn prop_nonzero_byte_detected(
            buf in proptest::collection::vec(0u8..=0u8, 1..64),
            pos in 0usize..64,
        ) {
            let pos = pos % buf.len();
            let mut v = buf;
            v[pos] = 1;
            prop_assert!(!is_zeroed(&v));
        }
    }
}

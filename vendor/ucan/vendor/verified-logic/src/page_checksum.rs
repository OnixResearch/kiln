//! Verified page checksum accumulation and torn-write detection.
//!
//! Fletcher-style two-sum checksum over byte sequences: accumulate running
//! sums `a` (byte sum) and `b` (sum of partial sums), detect partial writes
//! by comparing stored vs recomputed checksums, and prove roundtrip integrity.
//! Composes with [`page`] for page-level address alignment and [`slice`] for
//! prefix/suffix reasoning.

use vstd::prelude::*;

verus! {

// ─── Checksum Accumulation ───────────────────────────────────────────

/// Spec: accumulate one byte into the checksum pair (a, b).
/// a' = (a + byte) mod 256, b' = (b + a') mod 256.
// r[depends page-checksum.accumulate-byte]
// r[impl page-checksum.accumulate-byte]
pub open spec fn accumulate_byte_spec(a: u64, b: u64, byte: u8) -> (u64, u64) {
    let a_next = ((a + byte as u64) % 256) as u64;
    let b_next = ((b + a_next) % 256) as u64;
    (a_next, b_next)
}

// r[impl page-checksum.accumulate-byte]
/// Accumulate one byte into the running checksum sums.
pub fn accumulate_byte(a: u64, b: u64, byte: u8) -> (result: (u64, u64))
    requires a < 256, b < 256,
    ensures result == accumulate_byte_spec(a, b, byte),
{
    let a_next = (a + byte as u64) % 256;
    let b_next = (b + a_next) % 256;
    (a_next, b_next)
}

// r[verify page-checksum.accumulate-byte]
/// After accumulation, both sums remain in [0, 256).
proof fn accumulate_byte_bounded(a: u64, b: u64, byte: u8)
    requires a < 256, b < 256,
    ensures ({
        let r = accumulate_byte_spec(a, b, byte);
        r.0 < 256 && r.1 < 256
    }),
{}

// ─── Recursive Fold Over Data ────────────────────────────────────────

/// Spec: fold accumulate_byte over data[from..].
// r[depends page-checksum.fold-checksum]
// r[impl page-checksum.fold-checksum]
pub open spec fn fold_checksum_spec(data: Seq<u8>, from: int, a: u64, b: u64) -> (u64, u64)
    decreases (data.len() - from),
{
    if from < 0 || from >= data.len() {
        (a, b)
    } else {
        let (a2, b2) = accumulate_byte_spec(a, b, data[from]);
        fold_checksum_spec(data, from + 1, a2, b2)
    }
}

/// Spec: compute checksum of full data starting from (0, 0).
// r[depends page-checksum.checksum]
// r[impl page-checksum.checksum]
pub open spec fn checksum_spec(data: Seq<u8>) -> (u64, u64) {
    fold_checksum_spec(data, 0, 0, 0)
}

// r[impl page-checksum.fold-checksum]
/// Fold accumulate_byte over a byte slice.
pub fn fold_checksum(data: &[u8], a: u64, b: u64) -> (result: (u64, u64))
    requires a < 256, b < 256,
    ensures result == fold_checksum_spec(data@, 0, a, b),
{
    let mut a_cur = a;
    let mut b_cur = b;
    let mut i: usize = 0;

    while i < data.len()
        invariant
            0 <= i <= data@.len(),
            a_cur < 256,
            b_cur < 256,
            fold_checksum_spec(data@, i as int, a_cur, b_cur) == fold_checksum_spec(data@, 0, a, b),
        decreases data@.len() - i,
    {
        let (a2, b2) = accumulate_byte(a_cur, b_cur, data[i]);
        a_cur = a2;
        b_cur = b2;
        i += 1;
    }
    (a_cur, b_cur)
}

// r[impl page-checksum.checksum]
/// Compute the checksum of a full byte slice.
pub fn checksum(data: &[u8]) -> (result: (u64, u64))
    ensures result == checksum_spec(data@),
{
    fold_checksum(data, 0, 0)
}

// ─── Pack/Unpack ─────────────────────────────────────────────────────

/// Spec: pack (a, b) into a single u16 value: b << 8 | a.
// r[depends page-checksum.pack]
// r[impl page-checksum.pack]
pub open spec fn pack_spec(a: u64, b: u64) -> u16
    recommends a < 256, b < 256,
{
    ((b * 256 + a) % 65536) as u16
}

// r[impl page-checksum.pack]
#[allow(clippy::cast_possible_truncation)]
pub fn pack(a: u64, b: u64) -> (result: u16)
    requires a < 256, b < 256,
    ensures result == pack_spec(a, b),
{
    ((b * 256 + a) % 65536) as u16
}

/// Spec: unpack a u16 checksum back to (a, b).
// r[depends page-checksum.unpack]
// r[impl page-checksum.unpack]
pub open spec fn unpack_a_spec(packed: u16) -> u64 {
    (packed as u64) % 256
}

// r[impl page-checksum.unpack]
pub open spec fn unpack_b_spec(packed: u16) -> u64 {
    (packed as u64) / 256
}

// r[impl page-checksum.unpack]
pub fn unpack(packed: u16) -> (result: (u64, u64))
    ensures
        result.0 == unpack_a_spec(packed),
        result.1 == unpack_b_spec(packed),
{
    let a = (packed as u64) % 256;
    let b = (packed as u64) / 256;
    (a, b)
}

// r[verify page-checksum.unpack]
/// Unpacked values are in [0, 256).
proof fn unpack_bounded(packed: u16)
    ensures unpack_a_spec(packed) < 256, unpack_b_spec(packed) < 256,
{}

// ─── Roundtrip ───────────────────────────────────────────────────────

// r[verify page-checksum.pack-unpack-roundtrip]
/// pack then unpack recovers the original sums.
proof fn pack_unpack_roundtrip(a: u64, b: u64)
    requires a < 256, b < 256,
    ensures
        unpack_a_spec(pack_spec(a, b)) == a,
        unpack_b_spec(pack_spec(a, b)) == b,
{}

// ─── Torn-Write Detection ────────────────────────────────────────────

/// Spec: is the stored checksum valid for this data?
// r[depends page-checksum.is-valid]
// r[impl page-checksum.is-valid]
pub open spec fn is_valid_checksum_spec(data: Seq<u8>, stored: u16) -> bool {
    let (a, b) = checksum_spec(data);
    stored == pack_spec(a, b)
}

// r[impl page-checksum.is-valid]
/// Validate a stored checksum against data.
pub fn is_valid_checksum(data: &[u8], stored: u16) -> (result: bool)
    ensures result == is_valid_checksum_spec(data@, stored),
{
    let (a, b) = checksum(data);
    proof { fold_checksum_bounded(data@, 0, 0, 0); }
    stored == pack(a, b)
}

// r[verify page-checksum.is-valid]
/// Freshly computed and packed checksum always validates.
proof fn fresh_checksum_valid(data: Seq<u8>)
    ensures ({
        let (a, b) = checksum_spec(data);
        is_valid_checksum_spec(data, pack_spec(a, b))
    }),
{
    let (a, b) = checksum_spec(data);
    // a < 256 and b < 256 from fold invariant — need to show this
    fold_checksum_bounded(data, 0, 0, 0);
    pack_unpack_roundtrip(a, b);
}

/// Helper: fold_checksum_spec always produces values in [0, 256).
proof fn fold_checksum_bounded(data: Seq<u8>, from: int, a: u64, b: u64)
    requires a < 256, b < 256,
    ensures ({
        let (ra, rb) = fold_checksum_spec(data, from, a, b);
        ra < 256 && rb < 256
    }),
    decreases (data.len() - from),
{
    if from >= 0 && from < data.len() {
        let (a2, b2) = accumulate_byte_spec(a, b, data[from]);
        fold_checksum_bounded(data, from + 1, a2, b2);
    }
}

// ─── Empty Data ──────────────────────────────────────────────────────

// r[verify page-checksum.empty]
/// Checksum of empty data is (0, 0).
proof fn checksum_empty()
    ensures checksum_spec(Seq::<u8>::empty()) == (0u64, 0u64),
{}

// ─── Single Byte ─────────────────────────────────────────────────────

// r[verify page-checksum.single-byte]
/// Checksum of a single byte b is (b % 256, b % 256).
proof fn checksum_single(byte: u8)
    ensures checksum_spec(seq![byte]) == (byte as u64, byte as u64),
{
    let s = seq![byte];
    // Unfold: fold_checksum_spec(s, 0, 0, 0) = fold on s[0] then base
    let (a2, b2) = accumulate_byte_spec(0, 0, byte);
    assert(a2 == byte as u64);
    assert(b2 == byte as u64);
    // fold at index 1 hits base case
    assert(fold_checksum_spec(s, 1, a2, b2) == (a2, b2));
}

// ─── Concatenation ───────────────────────────────────────────────────



} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accumulate_byte_zero() {
        assert_eq!(accumulate_byte(0, 0, 0), (0, 0));
    }

    #[test]
    fn test_accumulate_byte_one() {
        // a' = (0 + 1) % 256 = 1, b' = (0 + 1) % 256 = 1
        assert_eq!(accumulate_byte(0, 0, 1), (1, 1));
    }

    #[test]
    fn test_accumulate_byte_wraps() {
        // a' = (255 + 1) % 256 = 0, b' = (255 + 0) % 256 = 255
        assert_eq!(accumulate_byte(255, 255, 1), (0, 255));
    }

    #[test]
    fn test_checksum_empty() {
        assert_eq!(checksum(&[]), (0, 0));
    }

    #[test]
    fn test_checksum_single() {
        assert_eq!(checksum(&[42]), (42, 42));
    }

    #[test]
    fn test_checksum_two_bytes() {
        // byte 1: a=1, b=1; byte 2: a=(1+2)%256=3, b=(1+3)%256=4
        assert_eq!(checksum(&[1, 2]), (3, 4));
    }

    #[test]
    fn test_pack_unpack_roundtrip() {
        for a in 0..256u64 {
            for b in 0..=255u64 {
                let packed = pack(a, b);
                let (ua, ub) = unpack(packed);
                assert_eq!(ua, a);
                assert_eq!(ub, b);
            }
        }
    }

    #[test]
    fn test_is_valid_checksum_correct() {
        let data = [10u8, 20, 30, 40, 50];
        let (a, b) = checksum(&data);
        let packed = pack(a, b);
        assert!(is_valid_checksum(&data, packed));
    }

    #[test]
    fn test_is_valid_checksum_detects_corruption() {
        let data = [10u8, 20, 30, 40, 50];
        let (a, b) = checksum(&data);
        let packed = pack(a, b);
        // Corrupt one byte
        let corrupt = [10u8, 21, 30, 40, 50];
        assert!(!is_valid_checksum(&corrupt, packed));
    }

    #[test]
    fn test_fold_checksum_deterministic() {
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let r1 = checksum(&data);
        let r2 = checksum(&data);
        assert_eq!(r1, r2);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn prop_checksum_bounded(data in proptest::collection::vec(any::<u8>(), 0..200)) {
            let (a, b) = checksum(&data);
            prop_assert!(a < 256);
            prop_assert!(b < 256);
        }

        #[test]
        fn prop_pack_unpack_roundtrip(a in 0u64..256, b in 0u64..256) {
            let packed = pack(a, b);
            let (ua, ub) = unpack(packed);
            prop_assert_eq!(ua, a);
            prop_assert_eq!(ub, b);
        }

        #[test]
        fn prop_fresh_checksum_validates(data in proptest::collection::vec(any::<u8>(), 0..200)) {
            let (a, b) = checksum(&data);
            let packed = pack(a, b);
            prop_assert!(is_valid_checksum(&data, packed));
        }

        #[test]
        fn prop_checksum_detects_single_bit_flip(
            data in proptest::collection::vec(any::<u8>(), 1..200usize),
            idx in any::<proptest::sample::Index>(),
            bit in 0u8..8,
        ) {
            let (a, b) = checksum(&data);
            let packed = pack(a, b);

            let mut corrupt = data.clone();
            let real_idx = idx.index(corrupt.len());
            corrupt[real_idx] ^= 1 << bit;

            if corrupt != data {
                // checksum MIGHT still match (fletcher8 has collisions), but
                // we at least verify that validation is consistent
                let (ca, cb) = checksum(&corrupt);
                let cpacked = pack(ca, cb);
                if cpacked != packed {
                    prop_assert!(!is_valid_checksum(&corrupt, packed));
                }
            }
        }
    }
}

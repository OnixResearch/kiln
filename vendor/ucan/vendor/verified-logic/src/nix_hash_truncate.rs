//! Verified Nix hash truncation via XOR folding.
//!
//! Matches Nix's `compressHash`: initialize a `target_len`-byte output buffer
//! with zeros, then XOR each input byte into `out[i % target_len]`.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

pub closed spec fn zero_bytes_spec(n: nat) -> Seq<u8> {
    Seq::new(n, |i: int| 0u8)
}

pub closed spec fn xor_step_spec(acc: Seq<u8>, idx: nat, byte: u8) -> Seq<u8>
    recommends idx < acc.len()
{
    acc.update(idx as int, acc[idx as int] ^ byte)
}

pub closed spec fn compress_hash_prefix_spec(hash: Seq<u8>, target_len: nat, upto: nat) -> Seq<u8>
    recommends upto <= hash.len()
    decreases upto
{
    if target_len == 0 {
        Seq::<u8>::empty()
    } else if upto == 0 {
        zero_bytes_spec(target_len)
    } else {
        let prev = compress_hash_prefix_spec(hash, target_len, (upto - 1) as nat);
        let idx = (((upto - 1) as int) % (target_len as int)) as nat;
        xor_step_spec(prev, idx, hash[(upto - 1) as int])
    }
}

// r[depends nix-hash-truncate.compress]
/// Spec: compress a hash to `target_len` bytes by XOR folding.
pub open spec fn compress_hash_spec(hash: Seq<u8>, target_len: nat) -> Seq<u8> {
    if target_len == 0 {
        Seq::<u8>::empty()
    } else {
        compress_hash_prefix_spec(hash, target_len, hash.len())
    }
}

// r[verify nix-hash-truncate.output-length]
/// Zero buffer has the requested length.
proof fn zero_bytes_len(n: nat)
    ensures zero_bytes_spec(n).len() == n
{}

proof fn compress_hash_prefix_len(hash: Seq<u8>, target_len: nat, upto: nat)
    requires upto <= hash.len()
    ensures compress_hash_prefix_spec(hash, target_len, upto).len() == target_len
    decreases upto
{
    if target_len != 0 && upto != 0 {
        compress_hash_prefix_len(hash, target_len, (upto - 1) as nat);
    }
}

// r[verify nix-hash-truncate.output-length]
/// Compressed output length is exactly `target_len`.
pub proof fn compress_hash_output_len(hash: Seq<u8>, target_len: nat)
    ensures compress_hash_spec(hash, target_len).len() == target_len
{
    if target_len != 0 {
        compress_hash_prefix_len(hash, target_len, hash.len());
    }
}

// r[verify nix-hash-truncate.determinism]
/// Compression is deterministic because the spec is pure.
pub proof fn compress_hash_deterministic(hash: Seq<u8>, target_len: nat)
    ensures compress_hash_spec(hash, target_len) == compress_hash_spec(hash, target_len)
{}

pub open spec fn seq_xor_spec(a: Seq<u8>, b: Seq<u8>) -> Seq<u8>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| (a[i] ^ b[i]) as u8)
}

proof fn xor_assoc_comm(x: u8, y: u8, z: u8, w: u8)
    ensures (x ^ z) ^ (y ^ w) == (x ^ y) ^ (z ^ w)
{
    let x64: u64 = x as u64;
    let y64: u64 = y as u64;
    let z64: u64 = z as u64;
    let w64: u64 = w as u64;
    assert((x64 ^ z64) ^ (y64 ^ w64) == (x64 ^ y64) ^ (z64 ^ w64)) by(bit_vector);
}

proof fn xor_zero(x: u8)
    ensures x ^ 0u8 == x
{
    let x64: u64 = x as u64;
    assert(x64 ^ 0u64 == x64) by(bit_vector);
}

proof fn zero_bytes_xor(n: nat)
    ensures seq_xor_spec(zero_bytes_spec(n), zero_bytes_spec(n)) == zero_bytes_spec(n)
{
    assert forall|i: int| 0 <= i < n as int implies
        (zero_bytes_spec(n)[i] ^ zero_bytes_spec(n)[i]) as u8 == 0u8
    by {
        xor_zero(0u8);
    }
    assert(seq_xor_spec(zero_bytes_spec(n), zero_bytes_spec(n)) =~= zero_bytes_spec(n));
}

proof fn xor_step_distributes(
    acc_a: Seq<u8>, acc_b: Seq<u8>, idx: nat, byte_a: u8, byte_b: u8,
)
    requires
        acc_a.len() == acc_b.len(),
        idx < acc_a.len(),
    ensures
        seq_xor_spec(
            xor_step_spec(acc_a, idx, byte_a),
            xor_step_spec(acc_b, idx, byte_b),
        ) == xor_step_spec(
            seq_xor_spec(acc_a, acc_b),
            idx,
            (byte_a ^ byte_b) as u8,
        )
{
    let lhs = seq_xor_spec(
        xor_step_spec(acc_a, idx, byte_a),
        xor_step_spec(acc_b, idx, byte_b),
    );
    let rhs = xor_step_spec(
        seq_xor_spec(acc_a, acc_b),
        idx,
        (byte_a ^ byte_b) as u8,
    );
    assert(lhs =~= rhs) by {
        assert forall|i: int| 0 <= i < lhs.len() implies lhs[i] == rhs[i] by {
            if i == idx as int {
                xor_assoc_comm(acc_a[i], acc_b[i], byte_a, byte_b);
            }
        }
    }
}

proof fn compress_hash_prefix_xor_linear(
    a: Seq<u8>, b: Seq<u8>, target_len: nat, upto: nat,
)
    requires
        a.len() == b.len(),
        upto <= a.len(),
        target_len > 0,
    ensures
        seq_xor_spec(
            compress_hash_prefix_spec(a, target_len, upto),
            compress_hash_prefix_spec(b, target_len, upto),
        ) == compress_hash_prefix_spec(
            seq_xor_spec(a, b),
            target_len,
            upto,
        )
    decreases upto
{
    if upto == 0 {
        zero_bytes_xor(target_len);
    } else {
        compress_hash_prefix_xor_linear(a, b, target_len, (upto - 1) as nat);
        compress_hash_prefix_len(a, target_len, (upto - 1) as nat);
        compress_hash_prefix_len(b, target_len, (upto - 1) as nat);
        let idx = (((upto - 1) as int) % (target_len as int)) as nat;
        xor_step_distributes(
            compress_hash_prefix_spec(a, target_len, (upto - 1) as nat),
            compress_hash_prefix_spec(b, target_len, (upto - 1) as nat),
            idx,
            a[(upto - 1) as int],
            b[(upto - 1) as int],
        );
    }
}

// r[verify nix-hash-truncate.xor-properties]
/// XOR linearity: compress(a XOR b, n) == compress(a, n) XOR compress(b, n).
pub proof fn compress_hash_xor_linear(a: Seq<u8>, b: Seq<u8>, target_len: nat)
    requires a.len() == b.len()
    ensures
        seq_xor_spec(
            compress_hash_spec(a, target_len),
            compress_hash_spec(b, target_len),
        ) == compress_hash_spec(
            seq_xor_spec(a, b),
            target_len,
        )
{
    if target_len == 0 {
        assert(seq_xor_spec(Seq::<u8>::empty(), Seq::<u8>::empty()) =~= Seq::<u8>::empty());
    } else {
        compress_hash_prefix_xor_linear(a, b, target_len, a.len());
    }
}

// r[verify nix-hash-truncate.xor-properties]
/// compress_hash of all zeros returns all zeros.
pub proof fn compress_hash_zero(n: nat, target_len: nat)
    ensures compress_hash_spec(zero_bytes_spec(n), target_len) == zero_bytes_spec(target_len)
{
    if target_len > 0 {
        compress_hash_prefix_zero(zero_bytes_spec(n), target_len, n);
    }
}

proof fn compress_hash_prefix_zero(hash: Seq<u8>, target_len: nat, upto: nat)
    requires
        target_len > 0,
        upto <= hash.len(),
        forall|i: int| 0 <= i < upto as int ==> hash[i] == 0u8,
    ensures
        compress_hash_prefix_spec(hash, target_len, upto) == zero_bytes_spec(target_len)
    decreases upto
{
    if upto != 0 {
        compress_hash_prefix_zero(hash, target_len, (upto - 1) as nat);
        compress_hash_prefix_len(hash, target_len, (upto - 1) as nat);
        let idx = (((upto - 1) as int) % (target_len as int)) as nat;
        xor_zero(0u8);
        assert(xor_step_spec(zero_bytes_spec(target_len), idx, 0u8)
            =~= zero_bytes_spec(target_len));
    }
}

// r[impl nix-hash-truncate.compress]
/// Compress a hash to `target_len` bytes by XOR folding.
pub fn compress_hash(hash: &Vec<u8>, target_len: usize) -> (result: Vec<u8>)
    ensures result@ == compress_hash_spec(hash@, target_len as nat)
{
    if target_len == 0 {
        return Vec::new();
    }

    let mut result: Vec<u8> = Vec::new();
    let mut j: usize = 0;
    while j < target_len
        invariant
            j <= target_len,
            result@ == zero_bytes_spec(j as nat),
        decreases target_len - j,
    {
        result.push(0u8);
        j = j + 1;
    }

    let mut i: usize = 0;
    while i < hash.len()
        invariant
            target_len > 0,
            i <= hash.len(),
            result.len() == target_len,
            result@ == compress_hash_prefix_spec(hash@, target_len as nat, i as nat),
        decreases hash.len() - i,
    {
        let idx = i % target_len;
        let cur = result[idx];
        let next = cur ^ hash[i];
        result.set(idx, next);
        i = i + 1;
    }

    proof {
        assert(result@ == compress_hash_spec(hash@, target_len as nat));
    }
    result
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn xor_vec(a: &[u8], b: &[u8]) -> Vec<u8> {
        a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
    }

    // r[verify nix-hash-truncate.output-length]
    #[test]
    fn test_compress_hash_output_len() {
        assert_eq!(compress_hash(&vec![1, 2, 3, 4], 2), vec![1 ^ 3, 2 ^ 4]);
        assert_eq!(compress_hash(&vec![1, 2, 3], 5), vec![1, 2, 3, 0, 0]);
        assert_eq!(compress_hash(&vec![1, 2, 3], 0), Vec::<u8>::new());
    }

    // r[verify nix-hash-truncate.compress]
    #[test]
    fn test_compress_hash_identity_case() {
        let input = vec![10, 20, 30, 40];
        assert_eq!(compress_hash(&input, input.len()), input);
    }

    // r[verify nix-hash-truncate.compress]
    #[test]
    fn test_compress_hash_fold_case() {
        let input: Vec<u8> = (0..32u8).collect();
        let result = compress_hash(&input, 20);
        let expected: Vec<u8> =
            (0..20usize).map(|i| if i < 12 { input[i] ^ input[i + 20] } else { input[i] }).collect();
        assert_eq!(result, expected);
    }

    // r[verify nix-hash-truncate.xor-properties]
    #[test]
    fn test_compress_hash_zero_input() {
        let input = vec![0u8; 32];
        assert_eq!(compress_hash(&input, 20), vec![0u8; 20]);
    }

    proptest! {
        // r[verify nix-hash-truncate.output-length]
        #[test]
        fn prop_compress_hash_output_len(
            input in proptest::collection::vec(any::<u8>(), 0..64),
            target_len in 0usize..32,
        ) {
            prop_assert_eq!(compress_hash(&input, target_len).len(), target_len);
        }

        // r[verify nix-hash-truncate.determinism]
        #[test]
        fn prop_compress_hash_deterministic(
            input in proptest::collection::vec(any::<u8>(), 0..64),
            target_len in 0usize..32,
        ) {
            let a = compress_hash(&input, target_len);
            let b = compress_hash(&input, target_len);
            prop_assert_eq!(a, b);
        }

        // r[verify nix-hash-truncate.xor-properties]
        #[test]
        fn prop_compress_hash_xor_linearity(
            a in proptest::collection::vec(any::<u8>(), 0..64),
            b in proptest::collection::vec(any::<u8>(), 0..64),
            target_len in 1usize..32,
        ) {
            let len = a.len().min(b.len());
            let a = a[..len].to_vec();
            let b = b[..len].to_vec();
            let axb = xor_vec(&a, &b);
            let ca = compress_hash(&a, target_len);
            let cb = compress_hash(&b, target_len);
            let cab = compress_hash(&axb, target_len);
            prop_assert_eq!(xor_vec(&ca, &cb), cab);
        }
    }
}

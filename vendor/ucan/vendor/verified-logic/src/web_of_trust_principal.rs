//! Verified web-of-trust principal identifiers.
//!
//! Models canonical principal IDs as full 32-byte BLAKE3 digests of
//! canonical public-key bytes. Equality compares all 32 digest bytes.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
fn blake3_digest_runtime(_input: &[u8]) -> Vec<u8> {
    vec![0u8; 32]
}

#[cfg(not(verus_keep_ghost))]
fn blake3_digest_runtime(input: &[u8]) -> Vec<u8> {
    blake3::hash(input).as_bytes().to_vec()
}

verus! {

pub const PRINCIPAL_ID_LEN: usize = 32;

/// Principal identifier derived from canonical public-key bytes.
pub struct PrincipalId {
    pub digest: Vec<u8>,
}

// r[depends web-of-trust.principal-fingerprint]
/// Uninterpreted per-byte BLAKE3 model over canonical key bytes.
pub uninterp spec fn blake3_digest_byte_spec(input: Seq<u8>, idx: int) -> u8;

// r[depends web-of-trust.principal-fingerprint]
/// Full 32-byte BLAKE3 digest model.
pub open spec fn blake3_digest_spec(input: Seq<u8>) -> Seq<u8> {
    Seq::new(PRINCIPAL_ID_LEN as nat, |i: int| blake3_digest_byte_spec(input, i))
}

// r[depends web-of-trust.principal-fingerprint]
/// Canonical principal-ID bytes derived from canonical key bytes.
pub open spec fn principal_id_bytes_spec(canonical_key_bytes: Seq<u8>) -> Seq<u8> {
    blake3_digest_spec(canonical_key_bytes)
}

/// Well-formed principal IDs are exactly 32 bytes long.
// r[impl web-of-trust.principal-fingerprint]
pub open spec fn principal_id_valid_spec(id: PrincipalId) -> bool {
    id.digest.len() == PRINCIPAL_ID_LEN
}

/// Raw principal digest bytes are admissible exactly when they are full-width IDs.
// r[impl web-of-trust.principal-digest-admission]
pub open spec fn principal_digest_bytes_valid_spec(bytes: Seq<u8>) -> bool {
    bytes.len() == PRINCIPAL_ID_LEN
}

/// Prefix equality over the first `upto` digest bytes.
// r[impl web-of-trust.principal-fingerprint]
pub open spec fn principal_id_equal_prefix_spec(a: Seq<u8>, b: Seq<u8>, upto: int) -> bool
    recommends a.len() == PRINCIPAL_ID_LEN, b.len() == PRINCIPAL_ID_LEN, 0 <= upto <= PRINCIPAL_ID_LEN
{
    forall|i: int| 0 <= i < upto ==> a[i] == b[i]
}

// r[depends web-of-trust.principal-fingerprint]
/// Full principal-ID equality uses all 32 digest bytes.
pub open spec fn principal_id_equal_bytes_spec(a: Seq<u8>, b: Seq<u8>) -> bool
    recommends a.len() == PRINCIPAL_ID_LEN, b.len() == PRINCIPAL_ID_LEN
{
    principal_id_equal_prefix_spec(a, b, PRINCIPAL_ID_LEN as int)
}

proof fn principal_id_equal_prefix_step(a: Seq<u8>, b: Seq<u8>, upto: int)
    requires
        a.len() == PRINCIPAL_ID_LEN,
        b.len() == PRINCIPAL_ID_LEN,
        0 <= upto < PRINCIPAL_ID_LEN,
    ensures
        principal_id_equal_prefix_spec(a, b, upto + 1)
            == (principal_id_equal_prefix_spec(a, b, upto) && a[upto] == b[upto]),
{
    assert(principal_id_equal_prefix_spec(a, b, upto + 1)
        ==> (principal_id_equal_prefix_spec(a, b, upto) && a[upto] == b[upto])) by {
        if principal_id_equal_prefix_spec(a, b, upto + 1) {
            assert(principal_id_equal_prefix_spec(a, b, upto));
            assert(a[upto] == b[upto]);
        }
    }
    assert((principal_id_equal_prefix_spec(a, b, upto) && a[upto] == b[upto])
        ==> principal_id_equal_prefix_spec(a, b, upto + 1)) by {
        if principal_id_equal_prefix_spec(a, b, upto) && a[upto] == b[upto] {
            assert forall|i: int| 0 <= i < upto + 1 implies a[i] == b[i] by {
                if i < upto {
                } else {
                    assert(i == upto);
                }
            }
        }
    }
}

// r[impl web-of-trust.principal-fingerprint]
/// Trusted runtime bridge for the full 32-byte BLAKE3 digest.
/// @verified-logic:permanent-axiom — crypto primitive, runtime-only
#[verifier::external_body]
pub fn blake3_digest_bridge(input: &[u8]) -> (result: Vec<u8>)
    ensures
        result@ == blake3_digest_spec(input@),
        result@.len() == PRINCIPAL_ID_LEN,
{
    blake3_digest_runtime(input)
}

// r[impl web-of-trust.principal-fingerprint]
/// Derive a canonical principal ID from canonical public-key bytes.
pub fn principal_id_from_bytes(canonical_key_bytes: &[u8]) -> (result: PrincipalId)
    ensures
        result.digest@ == principal_id_bytes_spec(canonical_key_bytes@),
        principal_id_valid_spec(result),
{
    let digest = blake3_digest_bridge(canonical_key_bytes);
    PrincipalId { digest }
}

// r[impl web-of-trust.principal-digest-admission]
/// Admit raw principal-ID digest bytes only when they are exactly 32 bytes.
pub fn principal_id_from_digest_bytes(bytes: Vec<u8>) -> (result: Option<PrincipalId>)
    ensures
        principal_digest_bytes_valid_spec(bytes@) ==> (result matches Some(id) && id.digest@ == bytes@ && principal_id_valid_spec(id)),
        !principal_digest_bytes_valid_spec(bytes@) ==> result is None,
{
    if bytes.len() == PRINCIPAL_ID_LEN {
        Some(PrincipalId { digest: bytes })
    } else {
        None
    }
}

// r[impl web-of-trust.principal-fingerprint]
/// Equality helper that compares all 32 digest bytes.
pub fn principal_ids_equal(a: &PrincipalId, b: &PrincipalId) -> (result: bool)
    requires
        principal_id_valid_spec(*a),
        principal_id_valid_spec(*b),
    ensures
        result == principal_id_equal_bytes_spec(a.digest@, b.digest@),
{
    let mut i: usize = 0;
    let mut eq = true;
    while i < PRINCIPAL_ID_LEN
        invariant
            i <= PRINCIPAL_ID_LEN,
            principal_id_valid_spec(*a),
            principal_id_valid_spec(*b),
            eq == principal_id_equal_prefix_spec(a.digest@, b.digest@, i as int),
        decreases PRINCIPAL_ID_LEN - i,
    {
        proof {
            principal_id_equal_prefix_step(a.digest@, b.digest@, i as int);
        }
        eq = eq && a.digest[i] == b.digest[i];
        i += 1;
    }
    eq
}

// r[verify web-of-trust.principal-fingerprint]
/// Equal canonical key bytes derive the same principal-ID bytes.
pub proof fn principal_id_deterministic(key1: Seq<u8>, key2: Seq<u8>)
    requires key1 == key2
    ensures principal_id_bytes_spec(key1) == principal_id_bytes_spec(key2)
{
}

// r[verify web-of-trust.principal-fingerprint]
/// Principal equality is full 32-byte equality, not prefix equality.
pub proof fn principal_id_full_eq(a: Seq<u8>, b: Seq<u8>)
    requires
        a.len() == PRINCIPAL_ID_LEN,
        b.len() == PRINCIPAL_ID_LEN,
    ensures
        principal_id_equal_bytes_spec(a, b) == (a =~= b),
{
    assert(principal_id_equal_bytes_spec(a, b) ==> a =~= b) by {
        if principal_id_equal_bytes_spec(a, b) {
            assert(a.len() == b.len());
            assert forall|i: int| 0 <= i < a.len() implies a[i] == b[i] by {
                assert(a.len() == PRINCIPAL_ID_LEN);
                assert(0 <= i < PRINCIPAL_ID_LEN);
            }
        }
    }
    assert((a =~= b) ==> principal_id_equal_bytes_spec(a, b)) by {
        if a =~= b {
            assert forall|i: int| 0 <= i < PRINCIPAL_ID_LEN implies a[i] == b[i] by {
            }
        }
    }
}

// r[verify web-of-trust.principal-fingerprint]
/// Principal-ID bytes match the exact canonical key-byte digest model.
pub proof fn principal_id_matches_key_bytes(canonical_key_bytes: Seq<u8>)
    ensures
        principal_id_bytes_spec(canonical_key_bytes) == blake3_digest_spec(canonical_key_bytes),
        principal_id_bytes_spec(canonical_key_bytes).len() == PRINCIPAL_ID_LEN,
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify web-of-trust.principal-fingerprint]
    #[test]
    fn test_same_key_bytes_give_same_principal_id() {
        let key = vec![0x01, 0x23, 0x45, 0x67, 0x89];
        let id1 = principal_id_from_bytes(&key);
        let id2 = principal_id_from_bytes(&key);
        assert_eq!(id1.digest, id2.digest);
        assert_eq!(id1.digest.len(), PRINCIPAL_ID_LEN);
        assert!(principal_ids_equal(&id1, &id2));
    }

    // r[verify web-of-trust.principal-fingerprint]
    #[test]
    fn test_full_digest_comparison_distinguishes_principals() {
        let mut left = vec![0u8; PRINCIPAL_ID_LEN];
        left[PRINCIPAL_ID_LEN - 1] = 0x11;
        let mut right = left.clone();
        right[PRINCIPAL_ID_LEN - 1] = 0x22;

        let id1 = PrincipalId { digest: left };
        let id2 = PrincipalId { digest: right };

        assert_eq!(&id1.digest[..PRINCIPAL_ID_LEN - 1], &id2.digest[..PRINCIPAL_ID_LEN - 1]);
        assert!(!principal_ids_equal(&id1, &id2));
    }

    // r[verify web-of-trust.principal-digest-admission]
    #[test]
    fn test_principal_digest_admission_accepts_full_width_digest() {
        let digest = vec![0x5au8; PRINCIPAL_ID_LEN];
        let id = principal_id_from_digest_bytes(digest.clone()).expect("full-width digest accepted");
        assert_eq!(id.digest, digest);
        assert_eq!(id.digest.len(), PRINCIPAL_ID_LEN);
    }

    // r[verify web-of-trust.principal-digest-admission]
    #[test]
    fn test_principal_digest_admission_rejects_bad_widths() {
        assert!(principal_id_from_digest_bytes(vec![0u8; PRINCIPAL_ID_LEN - 1]).is_none());
        assert!(principal_id_from_digest_bytes(vec![0u8; PRINCIPAL_ID_LEN + 1]).is_none());
    }

    proptest! {
        // r[verify web-of-trust.principal-fingerprint]
        #[test]
        fn prop_principal_id_determinism(key in prop::collection::vec(any::<u8>(), 0..128)) {
            let id1 = principal_id_from_bytes(&key);
            let id2 = principal_id_from_bytes(&key);
            prop_assert_eq!(&id1.digest, &id2.digest);
            prop_assert_eq!(id1.digest.len(), PRINCIPAL_ID_LEN);
            prop_assert!(principal_ids_equal(&id1, &id2));
        }

        // r[verify web-of-trust.principal-fingerprint]
        #[test]
        fn prop_principal_id_full_equality_uses_last_byte(mut key in prop::collection::vec(any::<u8>(), 0..128)) {
            let id1 = principal_id_from_bytes(&key);
            if key.is_empty() {
                key.push(0);
            } else {
                let last = key.len() - 1;
                key[last] ^= 0xff;
            }
            let id2 = principal_id_from_bytes(&key);
            if id1.digest == id2.digest {
                prop_assert!(principal_ids_equal(&id1, &id2));
            } else {
                prop_assert!(!principal_ids_equal(&id1, &id2));
            }
        }
    }
}

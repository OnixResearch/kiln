//! Verified Nix store-path fingerprint construction.
//!
//! This module also carries store-path name validation because fingerprint
//! construction depends on names being colon-free, non-empty, and made of the
//! allowed ASCII characters.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

pub closed spec fn text_hash_inputs_prefix_spec(parts: Seq<Seq<u8>>, upto: nat) -> Seq<u8>
    recommends upto <= parts.len()
    decreases upto
{
    if upto == 0 {
        Seq::<u8>::empty()
    } else {
        text_hash_inputs_prefix_spec(parts, (upto - 1) as nat) + parts[(upto - 1) as int] + seq![58u8]
    }
}

pub open spec fn no_colon_spec(s: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> s[i] != 58u8
}

// r[depends nix-store-path.valid-name]
/// Spec: valid Nix store path name.
pub open spec fn valid_name_spec(name: Seq<u8>) -> bool {
    1 <= name.len() <= 211
        && name[0int] != 46u8
        && forall|i: int| #![trigger name[i]] 0 <= i < name.len() ==> {
            let b = name[i];
            ((b >= 97u8 && b <= 122u8)
                || (b >= 65u8 && b <= 90u8)
                || (b >= 48u8 && b <= 57u8)
                || b == 43u8 || b == 45u8 || b == 46u8
                || b == 95u8 || b == 63u8 || b == 61u8)
        }
}

// r[impl nix-store-path.valid-name]
/// Check whether a byte string is a valid Nix store-path name.
pub fn valid_name(name: &Vec<u8>) -> (result: bool)
    ensures result == valid_name_spec(name@)
{
    if name.len() == 0 || name.len() > 211 || name[0] == 46u8 {
        return false;
    }

    let mut i: usize = 0;
    while i < name.len()
        invariant
            0 < name.len() <= 211,
            name@[0int] != 46u8,
            i <= name.len(),
            forall|j: int| #![trigger name@[j]] 0 <= j < i as int ==> {
                let b = name@[j];
                ((b >= 97u8 && b <= 122u8)
                    || (b >= 65u8 && b <= 90u8)
                    || (b >= 48u8 && b <= 57u8)
                    || b == 43u8 || b == 45u8 || b == 46u8
                    || b == 95u8 || b == 63u8 || b == 61u8)
            },
        decreases name.len() - i,
    {
        let b = name[i];
        if !((b >= 97u8 && b <= 122u8)
            || (b >= 65u8 && b <= 90u8)
            || (b >= 48u8 && b <= 57u8)
            || b == 43u8 || b == 45u8 || b == 46u8
            || b == 95u8 || b == 63u8 || b == 61u8)
        {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify nix-store-path.valid-name]
/// Empty names are rejected.
pub proof fn valid_name_rejects_empty()
    ensures !valid_name_spec(Seq::<u8>::empty())
{}

// r[verify nix-store-path.valid-name]
/// Dot-prefixed names are rejected.
pub proof fn valid_name_rejects_dot_prefixed(rest: Seq<u8>)
    ensures !valid_name_spec(seq![46u8] + rest)
{}

// r[verify nix-store-path.valid-name]
/// Names longer than 211 bytes are rejected.
pub proof fn valid_name_rejects_too_long(name: Seq<u8>)
    requires name.len() > 211
    ensures !valid_name_spec(name)
{}

// r[verify nix-store-path.valid-name]
/// Any disallowed character makes the name invalid.
pub proof fn valid_name_rejects_invalid_char(prefix: Seq<u8>, bad: u8, suffix: Seq<u8>)
    requires
        !((bad >= 97u8 && bad <= 122u8)
            || (bad >= 65u8 && bad <= 90u8)
            || (bad >= 48u8 && bad <= 57u8)
            || bad == 43u8 || bad == 45u8 || bad == 46u8
            || bad == 95u8 || bad == 63u8 || bad == 61u8),
        (prefix + seq![bad] + suffix).len() <= 211,
        (prefix + seq![bad] + suffix).len() > 0,
        (prefix + seq![bad] + suffix)[0int] != 46u8,
    ensures !valid_name_spec(prefix + seq![bad] + suffix)
{
    let name = prefix + seq![bad] + suffix;
    assert(!(forall|i: int| #![trigger name[i]] 0 <= i < name.len() ==> {
        let b = name[i];
        ((b >= 97u8 && b <= 122u8)
            || (b >= 65u8 && b <= 90u8)
            || (b >= 48u8 && b <= 57u8)
            || b == 43u8 || b == 45u8 || b == 46u8
            || b == 95u8 || b == 63u8 || b == 61u8)
    })) by {
        assert(0 <= prefix.len() < name.len());
        assert(name[prefix.len() as int] == bad);
    }
}

// r[depends nix-fingerprint.regular-output]
/// Spec: regular output fingerprint.
pub open spec fn regular_fingerprint_spec(
    output_name: Seq<u8>, drv_hash: Seq<u8>, name: Seq<u8>,
) -> Seq<u8> {
    seq![111u8, 117u8, 116u8, 112u8, 117u8, 116u8, 58u8]
        + output_name
        + seq![58u8, 115u8, 104u8, 97u8, 50u8, 53u8, 54u8, 58u8]
        + drv_hash
        + seq![58u8, 47u8, 110u8, 105u8, 120u8, 47u8, 115u8, 116u8, 111u8, 114u8, 101u8, 58u8]
        + name
}

// r[depends nix-fingerprint.fixed-output]
/// Spec: fixed-output fingerprint.
pub open spec fn fixed_fingerprint_spec(
    hash_algo: Seq<u8>, hash: Seq<u8>, name: Seq<u8>,
) -> Seq<u8> {
    seq![102u8, 105u8, 120u8, 101u8, 100u8, 58u8, 111u8, 117u8, 116u8, 58u8]
        + hash_algo
        + seq![58u8]
        + hash
        + seq![58u8]
        + name
}

// r[depends nix-fingerprint.text-hash]
/// Spec: text-hash fingerprint.
pub open spec fn text_hash_fingerprint_spec(
    input_hashes: Seq<Seq<u8>>, text_hash: Seq<u8>, name: Seq<u8>,
) -> Seq<u8> {
    seq![116u8, 101u8, 120u8, 116u8, 58u8]
        + text_hash_inputs_prefix_spec(input_hashes, input_hashes.len())
        + text_hash
        + seq![58u8, 47u8, 110u8, 105u8, 120u8, 47u8, 115u8, 116u8, 111u8, 114u8, 101u8, 58u8]
        + name
}

fn append_slice(dst: &mut Vec<u8>, src: &[u8])
    ensures dst@ == old(dst)@ + src@
{
    let ghost old_dst = dst@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            i <= src.len(),
            dst@ == old_dst + src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        dst.push(src[i]);
        i = i + 1;
    }
}

fn append_vec(dst: &mut Vec<u8>, src: &Vec<u8>)
    ensures dst@ == old(dst)@ + src@
{
    let ghost old_dst = dst@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            i <= src.len(),
            dst@ == old_dst + src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        dst.push(src[i]);
        i = i + 1;
    }
}

// r[impl nix-fingerprint.regular-output]
/// Construct a regular output fingerprint.
pub fn regular_fingerprint(
    output_name: &Vec<u8>, drv_hash: &Vec<u8>, name: &Vec<u8>,
) -> (result: Vec<u8>)
    ensures result@ == regular_fingerprint_spec(output_name@, drv_hash@, name@)
{
    let mut result = Vec::new();
    append_slice(&mut result, &[111u8, 117u8, 116u8, 112u8, 117u8, 116u8, 58u8]);
    append_vec(&mut result, output_name);
    append_slice(&mut result, &[58u8, 115u8, 104u8, 97u8, 50u8, 53u8, 54u8, 58u8]);
    append_vec(&mut result, drv_hash);
    append_slice(&mut result, &[58u8, 47u8, 110u8, 105u8, 120u8, 47u8, 115u8, 116u8, 111u8, 114u8, 101u8, 58u8]);
    append_vec(&mut result, name);
    result
}

// r[impl nix-fingerprint.fixed-output]
/// Construct a fixed-output fingerprint.
pub fn fixed_fingerprint(
    hash_algo: &Vec<u8>, hash: &Vec<u8>, name: &Vec<u8>,
) -> (result: Vec<u8>)
    ensures result@ == fixed_fingerprint_spec(hash_algo@, hash@, name@)
{
    let mut result = Vec::new();
    append_slice(&mut result, &[102u8, 105u8, 120u8, 101u8, 100u8, 58u8, 111u8, 117u8, 116u8, 58u8]);
    append_vec(&mut result, hash_algo);
    append_slice(&mut result, &[58u8]);
    append_vec(&mut result, hash);
    append_slice(&mut result, &[58u8]);
    append_vec(&mut result, name);
    result
}

// r[impl nix-fingerprint.text-hash]
/// Construct a text-hash fingerprint from already-sorted input hashes.
pub fn text_hash_fingerprint(
    input_hashes: &Vec<Vec<u8>>, text_hash: &Vec<u8>, name: &Vec<u8>,
) -> (result: Vec<u8>)
    ensures result@ == text_hash_fingerprint_spec(
        input_hashes@.map_values(|v: Vec<u8>| v@), text_hash@, name@)
{
    let ghost inputs = input_hashes@.map_values(|v: Vec<u8>| v@);
    let mut result = Vec::new();
    append_slice(&mut result, &[116u8, 101u8, 120u8, 116u8, 58u8]);

    let mut i: usize = 0;
    while i < input_hashes.len()
        invariant
            i <= input_hashes.len(),
            inputs == input_hashes@.map_values(|v: Vec<u8>| v@),
            result@ == seq![116u8, 101u8, 120u8, 116u8, 58u8]
                + text_hash_inputs_prefix_spec(inputs, i as nat),
        decreases input_hashes.len() - i,
    {
        append_vec(&mut result, &input_hashes[i]);
        append_slice(&mut result, &[58u8]);
        i = i + 1;
    }

    append_vec(&mut result, text_hash);
    append_slice(&mut result, &[58u8, 47u8, 110u8, 105u8, 120u8, 47u8, 115u8, 116u8, 111u8, 114u8, 101u8, 58u8]);
    append_vec(&mut result, name);
    result
}

// r[verify nix-fingerprint.injectivity]
/// Regular and fixed-output fingerprints are disjoint by prefix.
pub proof fn regular_and_fixed_disjoint(
    output_name: Seq<u8>, drv_hash: Seq<u8>, regular_name: Seq<u8>,
    algo: Seq<u8>, hash: Seq<u8>, fixed_name: Seq<u8>,
)
    ensures regular_fingerprint_spec(output_name, drv_hash, regular_name)
        != fixed_fingerprint_spec(algo, hash, fixed_name)
{
    assert(regular_fingerprint_spec(output_name, drv_hash, regular_name)[0int] == 111u8);
    assert(fixed_fingerprint_spec(algo, hash, fixed_name)[0int] == 102u8);
}

// r[verify nix-fingerprint.injectivity]
/// Different names produce different regular fingerprints when the other fields match.
pub proof fn regular_fingerprint_name_injective(
    output_name: Seq<u8>, drv_hash: Seq<u8>, a: Seq<u8>, b: Seq<u8>,
)
    requires a != b
    ensures regular_fingerprint_spec(output_name, drv_hash, a)
        != regular_fingerprint_spec(output_name, drv_hash, b)
{
    let prefix = seq![111u8, 117u8, 116u8, 112u8, 117u8, 116u8, 58u8]
        + output_name
        + seq![58u8, 115u8, 104u8, 97u8, 50u8, 53u8, 54u8, 58u8]
        + drv_hash
        + seq![58u8, 47u8, 110u8, 105u8, 120u8, 47u8, 115u8, 116u8, 111u8, 114u8, 101u8, 58u8];
    let fa = regular_fingerprint_spec(output_name, drv_hash, a);
    let fb = regular_fingerprint_spec(output_name, drv_hash, b);
    assert(fa != fb) by {
        if fa == fb {
            assert(a =~= fa.subrange(prefix.len() as int, fa.len() as int));
            assert(b =~= fb.subrange(prefix.len() as int, fb.len() as int));
            assert(a =~= b);
        }
    }
}

// r[verify nix-fingerprint.injectivity]
/// Regular and text-hash fingerprints are disjoint by prefix.
pub proof fn regular_and_text_disjoint(
    output_name: Seq<u8>, drv_hash: Seq<u8>, regular_name: Seq<u8>,
    input_hashes: Seq<Seq<u8>>, text_hash: Seq<u8>, text_name: Seq<u8>,
)
    ensures regular_fingerprint_spec(output_name, drv_hash, regular_name)
        != text_hash_fingerprint_spec(input_hashes, text_hash, text_name)
{
    assert(regular_fingerprint_spec(output_name, drv_hash, regular_name)[0int] == 111u8);
    assert(text_hash_fingerprint_spec(input_hashes, text_hash, text_name)[0int] == 116u8);
}

// r[verify nix-fingerprint.injectivity]
/// Fixed and text-hash fingerprints are disjoint by prefix.
pub proof fn fixed_and_text_disjoint(
    algo: Seq<u8>, hash: Seq<u8>, fixed_name: Seq<u8>,
    input_hashes: Seq<Seq<u8>>, text_hash: Seq<u8>, text_name: Seq<u8>,
)
    ensures fixed_fingerprint_spec(algo, hash, fixed_name)
        != text_hash_fingerprint_spec(input_hashes, text_hash, text_name)
{
    assert(fixed_fingerprint_spec(algo, hash, fixed_name)[0int] == 102u8);
    assert(text_hash_fingerprint_spec(input_hashes, text_hash, text_name)[0int] == 116u8);
}

// r[verify nix-fingerprint.injectivity]
/// Different names produce different fixed-output fingerprints when other fields match.
pub proof fn fixed_fingerprint_name_injective(
    algo: Seq<u8>, hash: Seq<u8>, a: Seq<u8>, b: Seq<u8>,
)
    requires a != b
    ensures fixed_fingerprint_spec(algo, hash, a)
        != fixed_fingerprint_spec(algo, hash, b)
{
    let prefix = seq![102u8, 105u8, 120u8, 101u8, 100u8, 58u8, 111u8, 117u8, 116u8, 58u8]
        + algo
        + seq![58u8]
        + hash
        + seq![58u8];
    let fa = fixed_fingerprint_spec(algo, hash, a);
    let fb = fixed_fingerprint_spec(algo, hash, b);
    assert(fa != fb) by {
        if fa == fb {
            assert(a =~= fa.subrange(prefix.len() as int, fa.len() as int));
            assert(b =~= fb.subrange(prefix.len() as int, fb.len() as int));
            assert(a =~= b);
        }
    }
}

// r[verify nix-fingerprint.injectivity]
/// Different hashes produce different fixed-output fingerprints when other fields match,
/// given hashes contain no colons.
pub proof fn fixed_fingerprint_hash_injective(
    algo: Seq<u8>, ha: Seq<u8>, hb: Seq<u8>, name: Seq<u8>,
)
    requires
        ha != hb,
        no_colon_spec(ha),
        no_colon_spec(hb),
    ensures fixed_fingerprint_spec(algo, ha, name)
        != fixed_fingerprint_spec(algo, hb, name)
{
    let pre = seq![102u8, 105u8, 120u8, 101u8, 100u8, 58u8, 111u8, 117u8, 116u8, 58u8]
        + algo + seq![58u8];
    let fa = fixed_fingerprint_spec(algo, ha, name);
    let fb = fixed_fingerprint_spec(algo, hb, name);
    assert(fa != fb) by {
        if fa == fb {
            if ha.len() == hb.len() {
                assert(ha =~= fa.subrange(pre.len() as int, (pre.len() + ha.len()) as int));
                assert(hb =~= fb.subrange(pre.len() as int, (pre.len() + hb.len()) as int));
                assert(ha =~= hb);
            } else {
                assert(fa.len() != fb.len());
            }
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn v(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    // r[verify nix-store-path.valid-name]
    #[test]
    fn test_valid_name() {
        assert!(valid_name(&v("hello-world_1.0")));
        assert!(valid_name(&v("A+Z?=")));
        assert!(!valid_name(&v("")));
        assert!(!valid_name(&v(".hidden")));
        assert!(!valid_name(&v("bad:name")));
        assert!(!valid_name(&vec![b'a'; 212]));
    }

    // r[verify nix-fingerprint.regular-output]
    #[test]
    fn test_regular_fingerprint() {
        let fp = regular_fingerprint(&v("out"), &v("deadbeef"), &v("hello-1.0"));
        assert_eq!(fp, v("output:out:sha256:deadbeef:/nix/store:hello-1.0"));
    }

    // r[verify nix-fingerprint.fixed-output]
    #[test]
    fn test_fixed_fingerprint() {
        let fp = fixed_fingerprint(&v("sha256"), &v("abcd"), &v("hello-1.0"));
        assert_eq!(fp, v("fixed:out:sha256:abcd:hello-1.0"));
    }

    // r[verify nix-fingerprint.text-hash]
    #[test]
    fn test_text_hash_fingerprint() {
        let fp = text_hash_fingerprint(&vec![v("aaaa"), v("bbbb")], &v("cccc"), &v("note.txt"));
        assert_eq!(fp, v("text:aaaa:bbbb:cccc:/nix/store:note.txt"));
    }

    proptest! {
        // r[verify nix-store-path.valid-name]
        #[test]
        fn prop_valid_name_accepts_allowed_chars(
            s in "[A-Za-z0-9+\\-._?=]{1,64}",
        ) {
            prop_assume!(!s.starts_with('.'));
            prop_assert!(valid_name(&s.into_bytes()));
        }

        // r[verify nix-store-path.valid-name]
        #[test]
        fn prop_valid_name_rejects_colon(
            prefix in "[A-Za-z0-9+\\-._?=]{0,20}",
            suffix in "[A-Za-z0-9+\\-._?=]{0,20}",
        ) {
            let mut s = prefix.into_bytes();
            s.push(b':');
            s.extend_from_slice(&suffix.into_bytes());
            prop_assert!(!valid_name(&s));
        }

        // r[verify nix-fingerprint.injectivity]
        #[test]
        fn prop_regular_and_fixed_differ(
            hash in "[0-9a-f]{1,20}",
            name in "[A-Za-z0-9+\\-._?=]{1,20}",
        ) {
            let reg = regular_fingerprint(&v("out"), &hash.clone().into_bytes(), &name.clone().into_bytes());
            let fix = fixed_fingerprint(&v("sha256"), &hash.into_bytes(), &name.into_bytes());
            prop_assert_ne!(reg, fix);
        }

        // r[verify nix-fingerprint.injectivity]
        #[test]
        fn prop_regular_name_injective(
            hash in "[0-9a-f]{1,20}",
            a in "[A-Za-z0-9+\\-._?=]{1,20}",
            b in "[A-Za-z0-9+\\-._?=]{1,20}",
        ) {
            prop_assume!(a != b);
            let fa = regular_fingerprint(&v("out"), &hash.clone().into_bytes(), &a.into_bytes());
            let fb = regular_fingerprint(&v("out"), &hash.into_bytes(), &b.into_bytes());
            prop_assert_ne!(fa, fb);
        }

        // r[verify nix-fingerprint.regular-output]
        #[test]
        fn prop_regular_fingerprint_format(
            hash in "[0-9a-f]{64}",
            name in "[a-z][A-Za-z0-9+\\-._?=]{0,20}",
        ) {
            let fp = regular_fingerprint(&v("out"), &hash.clone().into_bytes(), &name.clone().into_bytes());
            let expected = format!("output:out:sha256:{hash}:/nix/store:{name}");
            prop_assert_eq!(fp, expected.into_bytes());
        }

        // r[verify nix-fingerprint.fixed-output]
        #[test]
        fn prop_fixed_fingerprint_format(
            hash in "[0-9a-f]{64}",
            name in "[a-z][A-Za-z0-9+\\-._?=]{0,20}",
        ) {
            let fp = fixed_fingerprint(&v("sha256"), &hash.clone().into_bytes(), &name.clone().into_bytes());
            let expected = format!("fixed:out:sha256:{hash}:{name}");
            prop_assert_eq!(fp, expected.into_bytes());
        }

        // r[verify nix-fingerprint.injectivity]
        #[test]
        fn prop_fixed_name_injective(
            hash in "[0-9a-f]{1,20}",
            a in "[A-Za-z0-9+\\-._?=]{1,20}",
            b in "[A-Za-z0-9+\\-._?=]{1,20}",
        ) {
            prop_assume!(a != b);
            let fa = fixed_fingerprint(&v("sha256"), &hash.clone().into_bytes(), &a.into_bytes());
            let fb = fixed_fingerprint(&v("sha256"), &hash.into_bytes(), &b.into_bytes());
            prop_assert_ne!(fa, fb);
        }
    }
}

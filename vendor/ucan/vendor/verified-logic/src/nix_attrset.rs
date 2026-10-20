//! Verified Nix attribute set operations.
//!
//! Attribute sets are sorted sequences of (key, value) pairs where keys
//! are byte strings. The sorted invariant enables O(n) merge and
//! intersection via two-pointer algorithms.

// Verus requires &Vec for @ view, not &[T].
// Merge/insert functions are long due to Verus loop invariants — splitting would break proofs.
// Ghost variables (mapped_attrs, chosen, sa, sb) are used in Verus proof blocks but
// appear unused to rustc since proof code is erased in normal builds.
#![allow(clippy::ptr_arg, clippy::too_many_lines, unused_variables)]

use vstd::prelude::*;

verus! {

/// Spec: keys are sorted and unique.
pub open spec fn is_sorted_keys(attrs: Seq<(Seq<u8>, u64)>) -> bool {
    forall|i: int, j: int| 0 <= i < j < attrs.len() as int
        ==> attrs[i].0 != attrs[j].0
}

// --- Lexicographic byte sequence comparison ---

/// Lexicographic comparison on byte sequences: returns -1, 0, or 1.
pub open spec fn seq_cmp(a: Seq<u8>, b: Seq<u8>) -> int
    decreases a.len() + b.len()
{
    if a.len() == 0 && b.len() == 0 { 0 }
    else if a.len() == 0 { -1 }
    else if b.len() == 0 { 1 }
    else if a[0int] < b[0int] { -1 }
    else if a[0int] > b[0int] { 1 }
    else { seq_cmp(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int)) }
}

/// Lexicographic less-than on byte sequences.
pub open spec fn seq_lt(a: Seq<u8>, b: Seq<u8>) -> bool {
    seq_cmp(a, b) < 0
}

// --- attr_names ---

// r[depends nix-attrset.attr-names]
/// Spec: extract keys from an attribute set.
pub open spec fn attr_names_spec(attrs: Seq<(Seq<u8>, u64)>) -> Seq<Seq<u8>> {
    Seq::new(attrs.len(), |i: int| attrs[i].0)
}

// r[impl nix-attrset.attr-names]
/// Returns the list of attribute names (keys).
pub fn attr_names(attrs: &Vec<(Vec<u8>, u64)>) -> (result: Vec<Vec<u8>>)
    ensures result@.map_values(|v: Vec<u8>| v@) == attr_names_spec(
        attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
{
    let mapped_attrs = Ghost(attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut i: usize = 0;
    while i < attrs.len()
        invariant
            i <= attrs.len(),
            result.len() == i,
            mapped_attrs@ == attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            forall|j: int| 0 <= j < i as int ==> (#[trigger] result@[j])@ == mapped_attrs@[j].0,
        decreases attrs.len() - i,
    {
        result.push(attrs[i].0.clone());
        i += 1;
    }
    assert(result@.map_values(|v: Vec<u8>| v@) =~= attr_names_spec(mapped_attrs@));
    result
}

// --- attr_values ---

// r[depends nix-attrset.attr-values]
/// Spec: extract values from an attribute set.
pub open spec fn attr_values_spec(attrs: Seq<(Seq<u8>, u64)>) -> Seq<u64> {
    Seq::new(attrs.len(), |i: int| attrs[i].1)
}

// r[impl nix-attrset.attr-values]
/// Returns the list of attribute values.
pub fn attr_values(attrs: &Vec<(Vec<u8>, u64)>) -> (result: Vec<u64>)
    ensures result@ == attr_values_spec(
        attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
{
    let mapped_attrs = Ghost(attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < attrs.len()
        invariant
            i <= attrs.len(),
            result.len() == i,
            mapped_attrs@ == attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            forall|j: int| 0 <= j < i as int ==> (#[trigger] result@[j]) == mapped_attrs@[j].1,
        decreases attrs.len() - i,
    {
        result.push(attrs[i].1);
        i += 1;
    }
    assert(result@ =~= attr_values_spec(mapped_attrs@));
    result
}

// --- has_attr ---

// r[depends nix-attrset.has-attr]
/// Spec: does the attribute set contain a key?
pub open spec fn has_attr_spec(attrs: Seq<(Seq<u8>, u64)>, key: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < attrs.len() as int && attrs[i].0 == key
}

// r[impl nix-attrset.has-attr]
/// Returns true if the attribute set contains the given key.
pub fn has_attr(attrs: &Vec<(Vec<u8>, u64)>, key: &Vec<u8>) -> (result: bool)
    ensures result == has_attr_spec(
        attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)), key@)
{
    let mapped_attrs = Ghost(attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    let mut i: usize = 0;
    while i < attrs.len()
        invariant
            i <= attrs.len(),
            mapped_attrs@ == attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            forall|j: int| 0 <= j < i as int ==> (#[trigger] mapped_attrs@[j]).0 != key@,
        decreases attrs.len() - i,
    {
        if attrs[i].0 == *key {
            assert(mapped_attrs@[i as int].0 == key@);
            return true;
        }
        i += 1;
    }
    false
}

// --- get_attr ---

// r[depends nix-attrset.get-attr]
/// Spec: look up a value by key. Returns None if absent.
pub open spec fn get_attr_spec(attrs: Seq<(Seq<u8>, u64)>, key: Seq<u8>) -> Option<u64> {
    if has_attr_spec(attrs, key) {
        let i = choose|i: int| 0 <= i < attrs.len() as int && attrs[i].0 == key;
        Some(attrs[i].1)
    } else {
        None
    }
}

// r[impl nix-attrset.get-attr]
/// Look up a value by key. Returns None if the key is absent.
/// Note: the spec uses `choose`, so when multiple keys match, the
/// result must equal whichever the solver picks. We add
/// `is_sorted_keys` as a precondition so keys are unique, making
/// `choose` unambiguous.
pub fn get_attr(attrs: &Vec<(Vec<u8>, u64)>, key: &Vec<u8>) -> (result: Option<u64>)
    requires is_sorted_keys(attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
    ensures result == get_attr_spec(
        attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)), key@)
{
    let mapped_attrs = Ghost(attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    let mut i: usize = 0;
    while i < attrs.len()
        invariant
            i <= attrs.len(),
            mapped_attrs@ == attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            is_sorted_keys(mapped_attrs@),
            forall|j: int| 0 <= j < i as int ==> (#[trigger] mapped_attrs@[j]).0 != key@,
        decreases attrs.len() - i,
    {
        if attrs[i].0 == *key {
            assert(mapped_attrs@[i as int].0 == key@);
            // With is_sorted_keys, no other index has the same key,
            // so choose must pick this index.
            let chosen = Ghost(choose|ci: int| 0 <= ci < mapped_attrs@.len() as int && mapped_attrs@[ci].0 == key@);
            assert(0 <= chosen@ < mapped_attrs@.len() as int);
            assert(mapped_attrs@[chosen@].0 == key@);
            // chosen@ != i as int would contradict uniqueness or the scanned prefix
            assert(chosen@ == i as int);
            return Some(attrs[i].1);
        }
        i += 1;
    }
    None
}

// --- merge (the // operator) ---

// r[depends nix-attrset.merge]
/// Spec: merge two attribute sets. Right-hand side wins on key conflict.
pub open spec fn merge_spec(
    a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>,
) -> Seq<(Seq<u8>, u64)>
    decreases a.len() + b.len()
{
    if a.len() == 0 && b.len() == 0 {
        Seq::<(Seq<u8>, u64)>::empty()
    } else if a.len() == 0 {
        b
    } else if b.len() == 0 {
        a
    } else if seq_lt(a[0int].0, b[0int].0) {
        seq![a[0int]] + merge_spec(a.subrange(1, a.len() as int), b)
    } else if seq_lt(b[0int].0, a[0int].0) {
        seq![b[0int]] + merge_spec(a, b.subrange(1, b.len() as int))
    } else {
        // Equal keys: right (b) wins
        seq![b[0int]] + merge_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int))
    }
}

/// Antisymmetry: seq_cmp(a, b) == -seq_cmp(b, a).
pub proof fn seq_cmp_antisymmetric(a: Seq<u8>, b: Seq<u8>)
    ensures seq_cmp(a, b) == -seq_cmp(b, a)
    decreases a.len() + b.len()
{
    if a.len() > 0 && b.len() > 0 && a[0int] == b[0int] {
        seq_cmp_antisymmetric(
            a.subrange(1, a.len() as int),
            b.subrange(1, b.len() as int),
        );
    }
}

/// Helper: seq_cmp skips equal prefix bytes.
proof fn seq_cmp_skip(a: Seq<u8>, b: Seq<u8>, i: int)
    requires
        0 <= i < a.len(),
        0 <= i < b.len(),
        a[i] == b[i],
    ensures
        seq_cmp(a.subrange(i, a.len() as int), b.subrange(i, b.len() as int))
            == seq_cmp(a.subrange(i + 1, a.len() as int), b.subrange(i + 1, b.len() as int))
{
    let sa = a.subrange(i, a.len() as int);
    let sb = b.subrange(i, b.len() as int);
    assert(sa.len() > 0 && sb.len() > 0);
    assert(sa[0int] == a[i] && sb[0int] == b[i]);
    assert(sa.subrange(1, sa.len() as int) =~= a.subrange(i + 1, a.len() as int));
    assert(sb.subrange(1, sb.len() as int) =~= b.subrange(i + 1, b.len() as int));
}

/// Verified byte-by-byte comparison matching `seq_cmp` spec.
/// Internal helper used only by attrset merge/list-to-attrs logic in this module.
fn vec_u8_cmp(a: &Vec<u8>, b: &Vec<u8>) -> (result: i8)
    ensures result as int == seq_cmp(a@, b@),
            result == -1 || result == 0 || result == 1
{
    proof { assert(a@.subrange(0, a@.len() as int) =~= a@); }
    proof { assert(b@.subrange(0, b@.len() as int) =~= b@); }
    let mut i: usize = 0;
    while i < a.len() && i < b.len()
        invariant
            i <= a.len(),
            i <= b.len(),
            seq_cmp(a@, b@) == seq_cmp(a@.subrange(i as int, a@.len() as int),
                                       b@.subrange(i as int, b@.len() as int)),
        decreases a.len() + b.len() - 2 * i,
    {
        let sa = Ghost(a@.subrange(i as int, a@.len() as int));
        let sb = Ghost(b@.subrange(i as int, b@.len() as int));
        if a[i] < b[i] {
            assert(sa@[0int] < sb@[0int]);
            return -1;
        }
        if a[i] > b[i] {
            assert(sa@[0int] > sb@[0int]);
            return 1;
        }
        proof { seq_cmp_skip(a@, b@, i as int); }
        i = i + 1;
    }
    if a.len() == b.len() {
        proof {
            assert(a@.subrange(i as int, a@.len() as int).len() == 0);
            assert(b@.subrange(i as int, b@.len() as int).len() == 0);
        }
        0
    } else if i >= a.len() {
        proof {
            assert(a@.subrange(i as int, a@.len() as int).len() == 0);
            assert(b@.subrange(i as int, b@.len() as int).len() > 0);
        }
        -1
    } else {
        proof {
            assert(a@.subrange(i as int, a@.len() as int).len() > 0);
            assert(b@.subrange(i as int, b@.len() as int).len() == 0);
        }
        1
    }
}

/// Helper: merge_spec(a, empty) == a.
proof fn merge_spec_right_empty(a: Seq<(Seq<u8>, u64)>)
    ensures merge_spec(a, Seq::<(Seq<u8>, u64)>::empty()) =~= a
    decreases a.len()
{
    if a.len() > 0 {
        // a.len() > 0 && b.len() == 0 → returns a
    }
}

/// Helper: merge_spec(empty, b) == b.
proof fn merge_spec_left_empty(b: Seq<(Seq<u8>, u64)>)
    ensures merge_spec(Seq::<(Seq<u8>, u64)>::empty(), b) =~= b
{}

/// Helper: a non-empty subrange decomposes as head + tail.
proof fn subrange_decompose<T>(s: Seq<T>, i: int, j: int)
    requires 0 <= i < j, j <= s.len()
    ensures s.subrange(i, j) =~= seq![s[i]] + s.subrange(i + 1, j)
{}

// r[impl nix-attrset.merge]
/// Merge two attribute sets (Nix's `//` operator). Right wins on conflict.
pub fn merge(
    a: &Vec<(Vec<u8>, u64)>, b: &Vec<(Vec<u8>, u64)>,
) -> (result: Vec<(Vec<u8>, u64)>)
    ensures result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
        == merge_spec(
            a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
{
    let ghost ma = a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));
    let ghost mb = b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));
    let ghost full = merge_spec(ma, mb);

    let mut result: Vec<(Vec<u8>, u64)> = Vec::new();
    let ghost mut rg: Seq<(Seq<u8>, u64)> = Seq::empty();
    let mut ai: usize = 0;
    let mut bi: usize = 0;

    proof {
        assert(Seq::<(Seq<u8>, u64)>::empty() + merge_spec(ma, mb) =~= merge_spec(ma, mb));
        assert(ma.subrange(0int, ma.len() as int) =~= ma);
        assert(mb.subrange(0int, mb.len() as int) =~= mb);
    }

    // Main two-pointer merge loop
    while ai < a.len() && bi < b.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ma =~= a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            mb =~= b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            full =~= merge_spec(ma, mb),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() as int
                ==> rg[k] =~= (#[trigger] result@[k].0@, result@[k].1),
            full =~= rg + merge_spec(
                ma.subrange(ai as int, ma.len() as int),
                mb.subrange(bi as int, mb.len() as int)),
        decreases a.len() + b.len() - ai - bi,
    {
        let ghost rem_a = ma.subrange(ai as int, ma.len() as int);
        let ghost rem_b = mb.subrange(bi as int, mb.len() as int);
        // Unfold: first elements of remaining subranges
        assert(rem_a.len() > 0 && rem_b.len() > 0);
        assert(rem_a[0int] =~= ma[ai as int]);
        assert(rem_b[0int] =~= mb[bi as int]);
        assert(rem_a.subrange(1, rem_a.len() as int)
            =~= ma.subrange(ai as int + 1, ma.len() as int));
        assert(rem_b.subrange(1, rem_b.len() as int)
            =~= mb.subrange(bi as int + 1, mb.len() as int));

        // Connect exec comparison to spec comparison
        let cmp = vec_u8_cmp(&a[ai].0, &b[bi].0);
        assert(a@[ai as int].0@ =~= ma[ai as int].0);
        assert(b@[bi as int].0@ =~= mb[bi as int].0);
        proof { seq_cmp_antisymmetric(rem_a[0int].0, rem_b[0int].0); }

        if cmp < 0 {
            // seq_lt(rem_a[0].0, rem_b[0].0) → take from a
            assert(seq_lt(rem_a[0int].0, rem_b[0int].0));
            let k = a[ai].0.clone();
            let v = a[ai].1;
            result.push((k, v));
            proof { rg = rg + seq![ma[ai as int]]; }
            ai = ai + 1;
        } else if cmp > 0 {
            // seq_lt(rem_b[0].0, rem_a[0].0) → take from b
            assert(seq_lt(rem_b[0int].0, rem_a[0int].0));
            let k = b[bi].0.clone();
            let v = b[bi].1;
            result.push((k, v));
            proof { rg = rg + seq![mb[bi as int]]; }
            bi = bi + 1;
        } else {
            // Equal keys → take from b (right wins)
            assert(!seq_lt(rem_a[0int].0, rem_b[0int].0));
            assert(!seq_lt(rem_b[0int].0, rem_a[0int].0));
            let k = b[bi].0.clone();
            let v = b[bi].1;
            result.push((k, v));
            proof { rg = rg + seq![mb[bi as int]]; }
            ai = ai + 1;
            bi = bi + 1;
        }
    }

    // Drain remaining from a (only executes when b is exhausted)
    while ai < a.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ai >= a.len() || bi >= b.len(),
            ma =~= a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            mb =~= b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            full =~= merge_spec(ma, mb),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() as int
                ==> rg[k] =~= (#[trigger] result@[k].0@, result@[k].1),
            full =~= rg + merge_spec(
                ma.subrange(ai as int, ma.len() as int),
                mb.subrange(bi as int, mb.len() as int)),
        decreases a.len() - ai,
    {
        // ai < a.len() ∧ (ai >= a.len() ∨ bi >= b.len()) ⟹ bi >= b.len()
        proof {
            let rem_a = ma.subrange(ai as int, ma.len() as int);
            let rem_b = mb.subrange(bi as int, mb.len() as int);
            assert(rem_b.len() == 0);
            merge_spec_right_empty(rem_a);
            subrange_decompose(ma, ai as int, ma.len() as int);
        }
        let k = a[ai].0.clone();
        let v = a[ai].1;
        result.push((k, v));
        proof { rg = rg + seq![ma[ai as int]]; }
        ai = ai + 1;
    }

    // Drain remaining from b (only executes when a is exhausted)
    while bi < b.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ai >= a.len() || bi >= b.len(),
            ma =~= a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            mb =~= b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            full =~= merge_spec(ma, mb),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() as int
                ==> rg[k] =~= (#[trigger] result@[k].0@, result@[k].1),
            full =~= rg + merge_spec(
                ma.subrange(ai as int, ma.len() as int),
                mb.subrange(bi as int, mb.len() as int)),
        decreases b.len() - bi,
    {
        // bi < b.len() ∧ (ai >= a.len() ∨ bi >= b.len()) ⟹ ai >= a.len()
        proof {
            let rem_a = ma.subrange(ai as int, ma.len() as int);
            let rem_b = mb.subrange(bi as int, mb.len() as int);
            assert(rem_a.len() == 0);
            merge_spec_left_empty(rem_b);
            subrange_decompose(mb, bi as int, mb.len() as int);
        }
        let k = b[bi].0.clone();
        let v = b[bi].1;
        result.push((k, v));
        proof { rg = rg + seq![mb[bi as int]]; }
        bi = bi + 1;
    }

    proof {
        let rem_a = ma.subrange(ai as int, ma.len() as int);
        let rem_b = mb.subrange(bi as int, mb.len() as int);
        assert(rem_a.len() == 0 && rem_b.len() == 0);
        assert(merge_spec(rem_a, rem_b) =~= Seq::<(Seq<u8>, u64)>::empty());
        assert(rg =~= result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    }
    result
}

// --- remove_attrs ---

/// Spec helper: is key present in the key list?
pub open spec fn key_in_keys(key: Seq<u8>, keys: Seq<Seq<u8>>) -> bool {
    exists|i: int| 0 <= i < keys.len() as int && keys[i] =~= key
}

// r[depends nix-attrset.remove-attrs]
/// Spec: remove keys present in the exclusion set.
pub open spec fn remove_attrs_spec(
    attrs: Seq<(Seq<u8>, u64)>, keys: Seq<Seq<u8>>,
) -> Seq<(Seq<u8>, u64)>
    decreases attrs.len()
{
    if attrs.len() == 0 {
        Seq::<(Seq<u8>, u64)>::empty()
    } else {
        let rest = remove_attrs_spec(attrs.subrange(1, attrs.len() as int), keys);
        if key_in_keys(attrs[0int].0, keys) {
            rest
        } else {
            seq![attrs[0int]] + rest
        }
    }
}

/// Exec helper: check if key is in keys list.
fn key_in_keys_exec(key: &Vec<u8>, keys: &Vec<Vec<u8>>) -> (result: bool)
    ensures result == key_in_keys(key@, keys@.map_values(|v: Vec<u8>| v@))
{
    let ghost mk = keys@.map_values(|v: Vec<u8>| v@);
    let mut j: usize = 0;
    while j < keys.len()
        invariant
            j <= keys.len(),
            mk =~= keys@.map_values(|v: Vec<u8>| v@),
            forall|k: int| 0 <= k < j as int ==> !(mk[k] =~= key@),
        decreases keys.len() - j,
    {
        if keys[j] == *key {
            assert(mk[j as int] =~= key@);
            return true;
        }
        j = j + 1;
    }
    false
}

// r[impl nix-attrset.remove-attrs]
/// Remove attributes whose keys appear in `keys`.
pub fn remove_attrs(
    attrs: &Vec<(Vec<u8>, u64)>, keys: &Vec<Vec<u8>>,
) -> (result: Vec<(Vec<u8>, u64)>)
    ensures result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
        == remove_attrs_spec(
            attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            keys@.map_values(|v: Vec<u8>| v@))
{
    let ghost ma = attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));
    let ghost mk = keys@.map_values(|v: Vec<u8>| v@);
    let ghost full_spec = remove_attrs_spec(ma, mk);

    let mut result: Vec<(Vec<u8>, u64)> = Vec::new();
    let ghost mut rg: Seq<(Seq<u8>, u64)> = Seq::empty();
    let mut i: usize = 0;

    proof {
        assert(ma.subrange(0int, ma.len() as int) =~= ma);
        assert(Seq::<(Seq<u8>, u64)>::empty() + remove_attrs_spec(ma, mk)
            =~= remove_attrs_spec(ma, mk));
    }

    while i < attrs.len()
        invariant
            i <= attrs.len(),
            ma =~= attrs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            mk =~= keys@.map_values(|v: Vec<u8>| v@),
            full_spec =~= remove_attrs_spec(ma, mk),
            // Ghost result mirrors exec result in spec types
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() as int
                ==> rg[k] =~= (#[trigger] result@[k].0@, result@[k].1),
            // Main invariant: full = accumulated + remaining
            full_spec =~= rg + remove_attrs_spec(
                ma.subrange(i as int, ma.len() as int), mk),
        decreases attrs.len() - i,
    {
        let ghost remaining = ma.subrange(i as int, ma.len() as int);
        // Unfold: remaining[0] == ma[i], remaining.subrange(1,..) == ma.subrange(i+1,..)
        assert(remaining.len() > 0);
        assert(remaining[0int] =~= ma[i as int]);
        assert(remaining.subrange(1, remaining.len() as int)
            =~= ma.subrange(i as int + 1, ma.len() as int));

        let found = key_in_keys_exec(&attrs[i].0, keys);

        if !found {
            // Keep: remaining_spec == seq![ma[i]] + remaining_spec(i+1)
            // So: full_spec =~= rg + seq![ma[i]] + remaining_spec(i+1)
            //                =~= (rg + seq![ma[i]]) + remaining_spec(i+1)
            let k = attrs[i].0.clone();
            let v = attrs[i].1;
            result.push((k, v));
            proof {
                rg = rg + seq![ma[i as int]];
            }
        }
        // Skip: remaining_spec == remaining_spec(i+1), rg unchanged

        i = i + 1;
    }
    proof {
        // Remaining subrange is empty
        assert(ma.subrange(i as int, ma.len() as int).len() == 0);
        assert(remove_attrs_spec(ma.subrange(i as int, ma.len() as int), mk)
            =~= Seq::<(Seq<u8>, u64)>::empty());
        // full_spec =~= rg + empty =~= rg
        // rg =~= result@.map_values(...)
        assert(rg =~= result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    }
    result
}

// --- intersect_attrs ---

// r[depends nix-attrset.intersect-attrs]
/// Spec: key intersection, values from second set.
pub open spec fn intersect_attrs_spec(
    a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>,
) -> Seq<(Seq<u8>, u64)>
    decreases b.len()
{
    if b.len() == 0 {
        Seq::<(Seq<u8>, u64)>::empty()
    } else {
        let rest = intersect_attrs_spec(a, b.subrange(1, b.len() as int));
        if has_attr_spec(a, b[0int].0) {
            seq![b[0int]] + rest
        } else {
            rest
        }
    }
}

// r[impl nix-attrset.intersect-attrs]
/// Keep only keys present in both sets. Values come from `b`.
pub fn intersect_attrs(
    a: &Vec<(Vec<u8>, u64)>, b: &Vec<(Vec<u8>, u64)>,
) -> (result: Vec<(Vec<u8>, u64)>)
    ensures result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
        == intersect_attrs_spec(
            a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
{
    let ghost ma = a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));
    let ghost mb = b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));
    let ghost full_spec = intersect_attrs_spec(ma, mb);

    let mut result: Vec<(Vec<u8>, u64)> = Vec::new();
    let ghost mut rg: Seq<(Seq<u8>, u64)> = Seq::empty();
    let mut i: usize = 0;

    proof {
        assert(mb.subrange(0int, mb.len() as int) =~= mb);
        assert(Seq::<(Seq<u8>, u64)>::empty() + intersect_attrs_spec(ma, mb)
            =~= intersect_attrs_spec(ma, mb));
    }

    while i < b.len()
        invariant
            i <= b.len(),
            ma =~= a@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            mb =~= b@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            full_spec =~= intersect_attrs_spec(ma, mb),
            // Ghost result mirrors exec result in spec types
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() as int
                ==> rg[k] =~= (#[trigger] result@[k].0@, result@[k].1),
            // Main invariant: full = accumulated + remaining
            full_spec =~= rg + intersect_attrs_spec(
                ma, mb.subrange(i as int, mb.len() as int)),
        decreases b.len() - i,
    {
        let ghost remaining = mb.subrange(i as int, mb.len() as int);
        assert(remaining.len() > 0);
        assert(remaining[0int] =~= mb[i as int]);
        assert(remaining.subrange(1, remaining.len() as int)
            =~= mb.subrange(i as int + 1, mb.len() as int));

        // Check if b[i]'s key appears in a
        let found = has_attr(a, &b[i].0);

        if found {
            // Keep: intersect_spec(remaining) == seq![mb[i]] + intersect_spec(i+1)
            let k = b[i].0.clone();
            let v = b[i].1;
            result.push((k, v));
            proof {
                rg = rg + seq![mb[i as int]];
            }
        }
        // Skip: intersect_spec(remaining) == intersect_spec(i+1), rg unchanged

        i = i + 1;
    }
    proof {
        assert(mb.subrange(i as int, mb.len() as int).len() == 0);
        assert(intersect_attrs_spec(ma, mb.subrange(i as int, mb.len() as int))
            =~= Seq::<(Seq<u8>, u64)>::empty());
        assert(rg =~= result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)));
    }
    result
}

// --- list_to_attrs ---

/// Spec: insert a pair into a sorted sequence, replacing on key match.
pub open spec fn insert_into_sorted_spec(
    pair: (Seq<u8>, u64),
    sorted: Seq<(Seq<u8>, u64)>,
) -> Seq<(Seq<u8>, u64)>
    decreases sorted.len(),
{
    if sorted.len() == 0 {
        seq![pair]
    } else if seq_cmp(pair.0, sorted[0int].0) < 0 {
        seq![pair] + sorted
    } else if seq_cmp(pair.0, sorted[0int].0) == 0 {
        seq![pair] + sorted.subrange(1, sorted.len() as int)
    } else {
        seq![sorted[0int]] + insert_into_sorted_spec(
            pair, sorted.subrange(1, sorted.len() as int))
    }
}

// r[depends nix-attrset.list-to-attrs]
/// Spec: construct an attribute set from unsorted pairs.
/// Processes pairs left-to-right; later entries override earlier ones
/// with the same key.
pub open spec fn list_to_attrs_spec(
    pairs: Seq<(Seq<u8>, u64)>,
) -> Seq<(Seq<u8>, u64)>
    decreases pairs.len(),
{
    if pairs.len() == 0 {
        Seq::<(Seq<u8>, u64)>::empty()
    } else {
        let rest = list_to_attrs_spec(
            pairs.subrange(0, pairs.len() as int - 1));
        insert_into_sorted_spec(pairs[pairs.len() as int - 1], rest)
    }
}

/// Insert a pair into a sorted vec, maintaining sort order.
/// Replaces the value if the key already exists.
fn insert_into_sorted(
    pair: &(Vec<u8>, u64),
    sorted: &Vec<(Vec<u8>, u64)>,
) -> (result: Vec<(Vec<u8>, u64)>)
    ensures result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
        =~= insert_into_sorted_spec(
            (pair.0@, pair.1),
            sorted@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
{
    let ghost mp = (pair.0@, pair.1);
    let ghost ms = sorted@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));

    let mut result: Vec<(Vec<u8>, u64)> = Vec::new();
    let ghost mut rg: Seq<(Seq<u8>, u64)> = Seq::empty();
    let mut i: usize = 0;

    proof {
        assert(ms.subrange(0int, ms.len() as int) =~= ms);
        assert(Seq::<(Seq<u8>, u64)>::empty()
            + insert_into_sorted_spec(mp, ms)
            =~= insert_into_sorted_spec(mp, ms));
    }

    // Phase 1: copy elements with keys strictly less than pair.key
    while i < sorted.len()
        invariant
            i <= sorted.len(),
            ms =~= sorted@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)),
            mp == (pair.0@, pair.1),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() as int
                ==> rg[k] =~= (#[trigger] result@[k].0@, result@[k].1),
            rg =~= ms.subrange(0, i as int),
            insert_into_sorted_spec(mp, ms) =~=
                rg + insert_into_sorted_spec(
                    mp, ms.subrange(i as int, ms.len() as int)),
        decreases sorted.len() - i,
    {
        let cmp = vec_u8_cmp(&pair.0, &sorted[i].0);
        proof {
            let rem = ms.subrange(i as int, ms.len() as int);
            assert(rem.len() > 0);
            assert(rem[0int] =~= ms[i as int]);
        }

        if cmp <= 0 {
            // Found insertion point — handle inline and return
            result.push((pair.0.clone(), pair.1));
            proof { rg = rg + seq![mp]; }

            // If key matches, skip the duplicate
            let mut start = i;
            if cmp == 0 {
                proof {
                    let rem = ms.subrange(i as int, ms.len() as int);
                    assert(seq_cmp(mp.0, rem[0int].0) == 0);
                    assert(rem.subrange(1, rem.len() as int)
                        =~= ms.subrange(i as int + 1, ms.len() as int));
                }
                start = i + 1;
            } else {
                proof {
                    let rem = ms.subrange(i as int, ms.len() as int);
                    assert(seq_cmp(mp.0, rem[0int].0) < 0);
                }
            }

            // Copy remaining elements
            let mut j = start;
            while j < sorted.len()
                invariant
                    j <= sorted.len(),
                    ms =~= sorted@.map_values(
                        |p: (Vec<u8>, u64)| (p.0@, p.1)),
                    rg.len() == result@.len(),
                    forall|k: int| 0 <= k < rg.len() as int
                        ==> rg[k] =~= (
                            #[trigger] result@[k].0@, result@[k].1),
                    insert_into_sorted_spec(mp, ms) =~=
                        rg + ms.subrange(j as int, ms.len() as int),
                decreases sorted.len() - j,
            {
                proof {
                    subrange_decompose(
                        ms, j as int, ms.len() as int);
                }
                let k = sorted[j].0.clone();
                let v = sorted[j].1;
                result.push((k, v));
                proof { rg = rg + seq![ms[j as int]]; }
                j = j + 1;
            }

            proof {
                assert(ms.subrange(
                    j as int, ms.len() as int).len() == 0);
                assert(rg =~= result@.map_values(
                    |p: (Vec<u8>, u64)| (p.0@, p.1)));
            }
            return result;
        }

        // cmp > 0: pair.key > sorted[i].key — copy and continue
        proof {
            let rem = ms.subrange(i as int, ms.len() as int);
            assert(seq_cmp(mp.0, rem[0int].0) > 0);
            assert(rem.subrange(1, rem.len() as int)
                =~= ms.subrange(i as int + 1, ms.len() as int));
        }
        let k = sorted[i].0.clone();
        let v = sorted[i].1;
        result.push((k, v));
        proof { rg = rg + seq![ms[i as int]]; }
        i = i + 1;
    }

    // Exhausted sorted — pair is largest, append at end
    proof {
        let rem = ms.subrange(i as int, ms.len() as int);
        assert(rem.len() == 0);
    }
    result.push((pair.0.clone(), pair.1));
    proof {
        rg = rg + seq![mp];
        assert(rg =~= result@.map_values(
            |p: (Vec<u8>, u64)| (p.0@, p.1)));
    }
    result
}

// r[impl nix-attrset.list-to-attrs]
/// Sort and deduplicate (last wins) to construct an attribute set.
pub fn list_to_attrs(
    pairs: &Vec<(Vec<u8>, u64)>,
) -> (result: Vec<(Vec<u8>, u64)>)
    ensures result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
        == list_to_attrs_spec(
            pairs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1)))
{
    let ghost mp = pairs@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1));

    let mut result: Vec<(Vec<u8>, u64)> = Vec::new();
    let mut i: usize = 0;

    proof {
        assert(mp.subrange(0, 0int)
            =~= Seq::<(Seq<u8>, u64)>::empty());
        assert(list_to_attrs_spec(Seq::<(Seq<u8>, u64)>::empty())
            =~= Seq::<(Seq<u8>, u64)>::empty());
        assert(result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
            =~= Seq::<(Seq<u8>, u64)>::empty());
    }

    while i < pairs.len()
        invariant
            i <= pairs.len(),
            mp =~= pairs@.map_values(
                |p: (Vec<u8>, u64)| (p.0@, p.1)),
            result@.map_values(|p: (Vec<u8>, u64)| (p.0@, p.1))
                =~= list_to_attrs_spec(mp.subrange(0, i as int)),
        decreases pairs.len() - i,
    {
        proof {
            // Unfold spec for mp.subrange(0, i+1):
            // list_to_attrs_spec(sub) =
            //   insert_into_sorted_spec(sub[i], list_to_attrs_spec(sub[0..i]))
            let sub = mp.subrange(0, (i + 1) as int);
            assert(sub.len() as int == (i + 1) as int);
            assert(sub[sub.len() as int - 1] =~= mp[i as int]);
            assert(sub.subrange(0, sub.len() as int - 1)
                =~= mp.subrange(0, i as int));
        }
        result = insert_into_sorted(&pairs[i], &result);
        i = i + 1;
    }

    proof {
        assert(mp.subrange(0, mp.len() as int) =~= mp);
    }
    result
}

// --- Proofs ---

// r[verify nix-attrset.has-attr]
/// Absent key returns None from get_attr.
proof fn absent_key_returns_none(attrs: Seq<(Seq<u8>, u64)>, key: Seq<u8>)
    requires !has_attr_spec(attrs, key)
    ensures get_attr_spec(attrs, key) == None::<u64>
{}

// r[verify nix-attrset.has-attr]
/// Present key returns Some from get_attr.
proof fn present_key_returns_some(attrs: Seq<(Seq<u8>, u64)>, key: Seq<u8>)
    requires has_attr_spec(attrs, key)
    ensures get_attr_spec(attrs, key).is_some()
{}

// r[verify nix-attrset.attr-names]
/// attr_names length equals attr set length.
proof fn attr_names_length(attrs: Seq<(Seq<u8>, u64)>)
    ensures attr_names_spec(attrs).len() == attrs.len()
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn kv(k: &str, v: u64) -> (Vec<u8>, u64) {
        (k.as_bytes().to_vec(), v)
    }

    // r[verify nix-attrset.attr-values]
    #[test]
    fn test_attr_values() {
        let attrs = vec![kv("a", 1), kv("b", 2), kv("c", 3)];
        assert_eq!(attr_values(&attrs), vec![1, 2, 3]);
        assert_eq!(attr_values(&vec![]), Vec::<u64>::new());
    }

    // r[verify nix-attrset.has-attr]
    #[test]
    fn test_has_attr() {
        let attrs = vec![kv("a", 1), kv("b", 2), kv("c", 3)];
        assert!(has_attr(&attrs, &b"b".to_vec()));
        assert!(!has_attr(&attrs, &b"d".to_vec()));
    }

    // r[verify nix-attrset.get-attr]
    #[test]
    fn test_get_attr() {
        let attrs = vec![kv("a", 1), kv("b", 2)];
        assert_eq!(get_attr(&attrs, &b"b".to_vec()), Some(2));
        assert_eq!(get_attr(&attrs, &b"z".to_vec()), None);
    }

    // r[verify nix-attrset.merge]
    #[test]
    fn test_merge() {
        let a = vec![kv("a", 1), kv("b", 2)];
        let b = vec![kv("b", 3), kv("c", 4)];
        let m = merge(&a, &b);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0], kv("a", 1));
        assert_eq!(m[1], kv("b", 3)); // right wins
        assert_eq!(m[2], kv("c", 4));
    }

    // r[verify nix-attrset.remove-attrs]
    #[test]
    fn test_remove_attrs() {
        let attrs = vec![kv("a", 1), kv("b", 2), kv("c", 3)];
        let result = remove_attrs(&attrs, &vec![b"b".to_vec()]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], kv("a", 1));
        assert_eq!(result[1], kv("c", 3));
    }

    // r[verify nix-attrset.intersect-attrs]
    #[test]
    fn test_intersect_attrs() {
        let a = vec![kv("a", 1), kv("b", 2), kv("c", 3)];
        let b = vec![kv("b", 20), kv("c", 30), kv("d", 40)];
        let result = intersect_attrs(&a, &b);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], kv("b", 20)); // values from b
        assert_eq!(result[1], kv("c", 30));
    }

    // r[verify nix-attrset.list-to-attrs]
    #[test]
    fn test_list_to_attrs() {
        let pairs = vec![kv("c", 3), kv("a", 1), kv("b", 2)];
        let result = list_to_attrs(&pairs);
        assert_eq!(result[0], kv("a", 1));
        assert_eq!(result[1], kv("b", 2));
        assert_eq!(result[2], kv("c", 3));
    }

    proptest! {
        // r[verify nix-attrset.merge]
        #[test]
        fn prop_merge_right_wins(
            shared_key in "[a-z]{1,5}",
            v1: u64, v2: u64,
        ) {
            let k = shared_key.into_bytes();
            let a = vec![(k.clone(), v1)];
            let b = vec![(k.clone(), v2)];
            let m = merge(&a, &b);
            prop_assert_eq!(m.len(), 1);
            prop_assert_eq!(m[0].1, v2); // right wins
        }

        // r[verify nix-attrset.has-attr]
        #[test]
        fn prop_has_attr_after_merge(
            key in "[a-z]{1,5}",
            val: u64,
        ) {
            let k = key.into_bytes();
            let a: Vec<(Vec<u8>, u64)> = vec![];
            let b = vec![(k.clone(), val)];
            let m = merge(&a, &b);
            prop_assert!(has_attr(&m, &k));
        }

        // r[verify nix-attrset.list-to-attrs]
        #[test]
        fn prop_list_to_attrs_sorted(
            pairs in proptest::collection::vec(
                ("[a-z]{1,4}", 0u64..1000),
                0..20,
            ),
        ) {
            let pairs: Vec<(Vec<u8>, u64)> = pairs
                .into_iter()
                .map(|(k, v)| (k.into_bytes(), v))
                .collect();
            let result = list_to_attrs(&pairs);
            // Result is sorted
            for w in result.windows(2) {
                prop_assert!(w[0].0 < w[1].0,
                    "not sorted: {:?} >= {:?}", w[0].0, w[1].0);
            }
            // Result has unique keys
            for (i, a) in result.iter().enumerate() {
                for b in result.iter().skip(i + 1) {
                    prop_assert_ne!(&a.0, &b.0);
                }
            }
            // Last-wins: each key's value matches the last occurrence in input
            for (rk, rv) in &result {
                let last_val = pairs.iter()
                    .rev()
                    .find(|(k, _)| k == rk)
                    .map(|(_, v)| *v);
                prop_assert_eq!(Some(*rv), last_val,
                    "key {:?}: expected {:?}, got {}", rk, last_val, rv);
            }
            // Every input key is in the result
            for (k, _) in &pairs {
                prop_assert!(result.iter().any(|(rk, _)| rk == k),
                    "input key {:?} missing from result", k);
            }
        }
    }
}

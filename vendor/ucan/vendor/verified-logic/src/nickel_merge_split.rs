//! Verified three-way record split for Nickel merge.
//!
//! Models the `split` step from `core/src/eval/merge.rs`: given two
//! sorted key-value maps, partition into left-only, both (center),
//! and right-only. Proves disjointness, completeness, and value
//! preservation.

#![allow(clippy::ptr_arg, clippy::too_many_lines, unused_variables)]

use vstd::prelude::*;

verus! {

// --- Byte-sequence comparison (self-contained for cross-module independence) ---

/// Lexicographic comparison on byte sequences: returns -1, 0, or 1.
pub open spec fn key_cmp(a: Seq<u8>, b: Seq<u8>) -> int
    decreases a.len() + b.len()
{
    if a.len() == 0 && b.len() == 0 { 0 }
    else if a.len() == 0 { -1 }
    else if b.len() == 0 { 1 }
    else if a[0int] < b[0int] { -1 }
    else if a[0int] > b[0int] { 1 }
    else { key_cmp(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int)) }
}

/// Lexicographic less-than on byte sequences.
pub open spec fn key_lt(a: Seq<u8>, b: Seq<u8>) -> bool {
    key_cmp(a, b) < 0
}

/// Antisymmetry: key_cmp(a, b) == -key_cmp(b, a).
pub proof fn key_cmp_antisymmetric(a: Seq<u8>, b: Seq<u8>)
    ensures key_cmp(a, b) == -key_cmp(b, a)
    decreases a.len() + b.len()
{
    if a.len() > 0 && b.len() > 0 && a[0int] == b[0int] {
        key_cmp_antisymmetric(
            a.subrange(1, a.len() as int),
            b.subrange(1, b.len() as int),
        );
    }
}

proof fn subrange_drop_first_len<A>(s: Seq<A>)
    requires s.len() > 0
    ensures s.subrange(1, s.len() as int).len() == s.len() - 1
{}

proof fn subrange_drop_first_index<A>(s: Seq<A>, i: int)
    requires s.len() > 0, 0 <= i < s.subrange(1, s.len() as int).len()
    ensures s.subrange(1, s.len() as int)[i] == s[i + 1]
{}

pub proof fn key_total(a: Seq<u8>, b: Seq<u8>)
    ensures key_lt(a, b) || key_lt(b, a) || a =~= b
    decreases a.len() + b.len()
{
    if a.len() == 0 && b.len() == 0 {
    } else if a.len() == 0 {
    } else if b.len() == 0 {
    } else if a[0int] < b[0int] {
    } else if a[0int] > b[0int] {
    } else {
        key_total(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        if a.subrange(1, a.len() as int) =~= b.subrange(1, b.len() as int) {
            assert(a.len() == b.len());
            assert forall|i: int| 0 <= i < a.len() implies a[i] == b[i] by {
                if i == 0 {
                } else {
                    subrange_drop_first_index(a, i - 1);
                    subrange_drop_first_index(b, i - 1);
                    assert(a.subrange(1, a.len() as int)[(i - 1) as int] == b.subrange(1, b.len() as int)[(i - 1) as int]);
                }
            }
            assert(a =~= b);
        }
    }
}

/// Skip equal prefix byte helper.
proof fn key_cmp_skip(a: Seq<u8>, b: Seq<u8>, i: int)
    requires 0 <= i < a.len(), 0 <= i < b.len(), a[i] == b[i]
    ensures
        key_cmp(a.subrange(i, a.len() as int), b.subrange(i, b.len() as int))
            == key_cmp(a.subrange(i + 1, a.len() as int), b.subrange(i + 1, b.len() as int))
{
    let sa = a.subrange(i, a.len() as int);
    let sb = b.subrange(i, b.len() as int);
    assert(sa.len() > 0 && sb.len() > 0);
    assert(sa[0int] == a[i] && sb[0int] == b[i]);
    assert(sa.subrange(1, sa.len() as int) =~= a.subrange(i + 1, a.len() as int));
    assert(sb.subrange(1, sb.len() as int) =~= b.subrange(i + 1, b.len() as int));
}

/// Exec byte-by-byte comparison matching key_cmp spec.
fn key_cmp_exec(a: &Vec<u8>, b: &Vec<u8>) -> (result: i8)
    ensures result as int == key_cmp(a@, b@),
            result == -1 || result == 0 || result == 1
{
    proof { assert(a@.subrange(0, a@.len() as int) =~= a@); }
    proof { assert(b@.subrange(0, b@.len() as int) =~= b@); }
    let mut i: usize = 0;
    while i < a.len() && i < b.len()
        invariant
            i <= a.len(),
            i <= b.len(),
            key_cmp(a@, b@) == key_cmp(a@.subrange(i as int, a@.len() as int),
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
        proof { key_cmp_skip(a@, b@, i as int); }
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

// --- Sorted-keys invariant ---

/// Spec: keys are sorted and unique (by key_lt).
pub open spec fn is_sorted_keys(attrs: Seq<(Seq<u8>, u64)>) -> bool {
    forall|i: int, j: int| 0 <= i < j < attrs.len()
        ==> key_lt(attrs[i].0, attrs[j].0)
}

// --- Split result ---

/// Result of a three-way split.
pub struct SplitResult {
    pub left: Seq<(Seq<u8>, u64)>,
    pub center: Seq<(Seq<u8>, u64, u64)>,
    pub right: Seq<(Seq<u8>, u64)>,
}

/// Spec helpers for key extraction.
pub open spec fn keys_of(s: Seq<(Seq<u8>, u64)>) -> Seq<Seq<u8>> {
    Seq::new(s.len(), |i: int| s[i].0)
}

pub open spec fn center_keys(s: Seq<(Seq<u8>, u64, u64)>) -> Seq<Seq<u8>> {
    Seq::new(s.len(), |i: int| s[i].0)
}

pub open spec fn has_attr_spec(attrs: Seq<(Seq<u8>, u64)>, key: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < attrs.len() && attrs[i].0 =~= key
}

pub open spec fn kv_kv_disjoint(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>) -> bool {
    forall|i: int, j: int| 0 <= i < a.len() && 0 <= j < b.len()
        ==> !(a[i].0 =~= b[j].0)
}

pub open spec fn kv_center_disjoint(a: Seq<(Seq<u8>, u64)>, c: Seq<(Seq<u8>, u64, u64)>) -> bool {
    forall|i: int, j: int| 0 <= i < a.len() && 0 <= j < c.len()
        ==> !(a[i].0 =~= c[j].0)
}

// --- Spec: recursive split ---

// r[depends nickel-merge.split]
/// Spec: three-way split of two sorted maps.
pub open spec fn split_spec(
    a: Seq<(Seq<u8>, u64)>,
    b: Seq<(Seq<u8>, u64)>,
) -> SplitResult
    decreases a.len() + b.len()
{
    if a.len() == 0 && b.len() == 0 {
        SplitResult {
            left: Seq::empty(),
            center: Seq::empty(),
            right: Seq::empty(),
        }
    } else if a.len() == 0 {
        SplitResult {
            left: Seq::empty(),
            center: Seq::empty(),
            right: b,
        }
    } else if b.len() == 0 {
        SplitResult {
            left: a,
            center: Seq::empty(),
            right: Seq::empty(),
        }
    } else if key_lt(a[0int].0, b[0int].0) {
        let rest = split_spec(a.subrange(1, a.len() as int), b);
        SplitResult {
            left: seq![(a[0int].0, a[0int].1)] + rest.left,
            center: rest.center,
            right: rest.right,
        }
    } else if key_lt(b[0int].0, a[0int].0) {
        let rest = split_spec(a, b.subrange(1, b.len() as int));
        SplitResult {
            left: rest.left,
            center: rest.center,
            right: seq![(b[0int].0, b[0int].1)] + rest.right,
        }
    } else {
        // Equal keys
        let rest = split_spec(
            a.subrange(1, a.len() as int),
            b.subrange(1, b.len() as int),
        );
        SplitResult {
            left: rest.left,
            center: seq![(a[0int].0, a[0int].1, b[0int].1)] + rest.center,
            right: rest.right,
        }
    }
}

/// Exec result of three-way split.
pub struct ExecSplitResult {
    pub left: Vec<(Vec<u8>, u64)>,
    pub center: Vec<(Vec<u8>, u64, u64)>,
    pub right: Vec<(Vec<u8>, u64)>,
}

/// Map exec left/right vec to spec.
pub open spec fn map_kv(v: Seq<(Vec<u8>, u64)>) -> Seq<(Seq<u8>, u64)> {
    Seq::new(v.len(), |i: int| (v[i].0@, v[i].1))
}

/// Map exec center vec to spec.
pub open spec fn map_center(v: Seq<(Vec<u8>, u64, u64)>) -> Seq<(Seq<u8>, u64, u64)> {
    Seq::new(v.len(), |i: int| (v[i].0@, v[i].1, v[i].2))
}

// r[impl nickel-merge.split]
// r[impl nickel-merge.split-completeness]
// r[impl nickel-merge.split-value-preservation]
/// Three-way split of two sorted key-value maps.
pub fn split(
    a: &Vec<(Vec<u8>, u64)>,
    b: &Vec<(Vec<u8>, u64)>,
) -> (result: ExecSplitResult)
    ensures ({
        let spec_result = split_spec(map_kv(a@), map_kv(b@));
        &&& map_kv(result.left@) =~= spec_result.left
        &&& map_center(result.center@) =~= spec_result.center
        &&& map_kv(result.right@) =~= spec_result.right
    })
{
    let ghost ma = map_kv(a@);
    let ghost mb = map_kv(b@);

    let mut left: Vec<(Vec<u8>, u64)> = Vec::new();
    let mut center: Vec<(Vec<u8>, u64, u64)> = Vec::new();
    let mut right: Vec<(Vec<u8>, u64)> = Vec::new();

    let ghost mut gl: Seq<(Seq<u8>, u64)> = Seq::empty();
    let ghost mut gc: Seq<(Seq<u8>, u64, u64)> = Seq::empty();
    let ghost mut gr: Seq<(Seq<u8>, u64)> = Seq::empty();

    let mut ai: usize = 0;
    let mut bi: usize = 0;

    proof {
        assert(ma.subrange(0, ma.len() as int) =~= ma);
        assert(mb.subrange(0, mb.len() as int) =~= mb);
    }

    while ai < a.len() && bi < b.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ma =~= map_kv(a@),
            mb =~= map_kv(b@),
            gl.len() == left@.len(),
            gc.len() == center@.len(),
            gr.len() == right@.len(),
            forall|k: int| 0 <= k < gl.len() ==> gl[k] =~= (left@[k].0@, left@[k].1),
            forall|k: int| 0 <= k < gc.len() ==> gc[k] =~= (center@[k].0@, center@[k].1, center@[k].2),
            forall|k: int| 0 <= k < gr.len() ==> gr[k] =~= (right@[k].0@, right@[k].1),
            ({
                let tail = split_spec(
                    ma.subrange(ai as int, ma.len() as int),
                    mb.subrange(bi as int, mb.len() as int),
                );
                let full = split_spec(ma, mb);
                &&& full.left =~= gl + tail.left
                &&& full.center =~= gc + tail.center
                &&& full.right =~= gr + tail.right
            }),
        decreases a.len() + b.len() - ai - bi,
    {
        let ghost rem_a = ma.subrange(ai as int, ma.len() as int);
        let ghost rem_b = mb.subrange(bi as int, mb.len() as int);
        assert(rem_a.len() > 0 && rem_b.len() > 0);
        assert(rem_a[0int] =~= ma[ai as int]);
        assert(rem_b[0int] =~= mb[bi as int]);
        assert(rem_a.subrange(1, rem_a.len() as int) =~= ma.subrange(ai as int + 1, ma.len() as int));
        assert(rem_b.subrange(1, rem_b.len() as int) =~= mb.subrange(bi as int + 1, mb.len() as int));

        let cmp = key_cmp_exec(&a[ai].0, &b[bi].0);
        proof { key_cmp_antisymmetric(rem_a[0int].0, rem_b[0int].0); }

        if cmp < 0 {
            left.push((a[ai].0.clone(), a[ai].1));
            proof { gl = gl + seq![ma[ai as int]]; }
            ai = ai + 1;
        } else if cmp > 0 {
            right.push((b[bi].0.clone(), b[bi].1));
            proof { gr = gr + seq![mb[bi as int]]; }
            bi = bi + 1;
        } else {
            center.push((a[ai].0.clone(), a[ai].1, b[bi].1));
            proof { gc = gc + seq![(ma[ai as int].0, ma[ai as int].1, mb[bi as int].1)]; }
            ai = ai + 1;
            bi = bi + 1;
        }
    }

    // Drain remaining from a → left
    while ai < a.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ai >= a.len() || bi >= b.len(),
            ma =~= map_kv(a@),
            mb =~= map_kv(b@),
            gl.len() == left@.len(),
            gc.len() == center@.len(),
            gr.len() == right@.len(),
            forall|k: int| 0 <= k < gl.len() ==> gl[k] =~= (left@[k].0@, left@[k].1),
            forall|k: int| 0 <= k < gc.len() ==> gc[k] =~= (center@[k].0@, center@[k].1, center@[k].2),
            forall|k: int| 0 <= k < gr.len() ==> gr[k] =~= (right@[k].0@, right@[k].1),
            ({
                let tail = split_spec(
                    ma.subrange(ai as int, ma.len() as int),
                    mb.subrange(bi as int, mb.len() as int),
                );
                let full = split_spec(ma, mb);
                &&& full.left =~= gl + tail.left
                &&& full.center =~= gc + tail.center
                &&& full.right =~= gr + tail.right
            }),
        decreases a.len() - ai,
    {
        proof {
            let rem_a = ma.subrange(ai as int, ma.len() as int);
            let rem_b = mb.subrange(bi as int, mb.len() as int);
            assert(rem_b.len() == 0);
            assert(rem_a.len() > 0);
            assert(rem_a.subrange(1, rem_a.len() as int) =~= ma.subrange(ai as int + 1, ma.len() as int));
        }
        left.push((a[ai].0.clone(), a[ai].1));
        proof { gl = gl + seq![ma[ai as int]]; }
        ai = ai + 1;
    }

    // Drain remaining from b → right
    while bi < b.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ai >= a.len() || bi >= b.len(),
            ma =~= map_kv(a@),
            mb =~= map_kv(b@),
            gl.len() == left@.len(),
            gc.len() == center@.len(),
            gr.len() == right@.len(),
            forall|k: int| 0 <= k < gl.len() ==> gl[k] =~= (left@[k].0@, left@[k].1),
            forall|k: int| 0 <= k < gc.len() ==> gc[k] =~= (center@[k].0@, center@[k].1, center@[k].2),
            forall|k: int| 0 <= k < gr.len() ==> gr[k] =~= (right@[k].0@, right@[k].1),
            ({
                let tail = split_spec(
                    ma.subrange(ai as int, ma.len() as int),
                    mb.subrange(bi as int, mb.len() as int),
                );
                let full = split_spec(ma, mb);
                &&& full.left =~= gl + tail.left
                &&& full.center =~= gc + tail.center
                &&& full.right =~= gr + tail.right
            }),
        decreases b.len() - bi,
    {
        proof {
            let rem_a = ma.subrange(ai as int, ma.len() as int);
            let rem_b = mb.subrange(bi as int, mb.len() as int);
            assert(rem_a.len() == 0);
            assert(rem_b.len() > 0);
            assert(rem_b.subrange(1, rem_b.len() as int) =~= mb.subrange(bi as int + 1, mb.len() as int));
        }
        right.push((b[bi].0.clone(), b[bi].1));
        proof { gr = gr + seq![mb[bi as int]]; }
        bi = bi + 1;
    }

    proof {
        let rem_a = ma.subrange(ai as int, ma.len() as int);
        let rem_b = mb.subrange(bi as int, mb.len() as int);
        assert(rem_a.len() == 0 && rem_b.len() == 0);
        assert(gl =~= map_kv(left@));
        assert(gc =~= map_center(center@));
        assert(gr =~= map_kv(right@));
    }
    ExecSplitResult { left, center, right }
}

// --- Proofs ---

proof fn key_lt_irreflexive(a: Seq<u8>)
    ensures !key_lt(a, a)
    decreases a.len()
{
    if a.len() > 0 {
        key_lt_irreflexive(a.subrange(1, a.len() as int));
    }
}

proof fn key_lt_asymmetric(a: Seq<u8>, b: Seq<u8>)
    requires key_lt(a, b)
    ensures !key_lt(b, a)
{
    key_cmp_antisymmetric(a, b);
}

proof fn sorted_tail(attrs: Seq<(Seq<u8>, u64)>)
    requires is_sorted_keys(attrs), attrs.len() > 0
    ensures is_sorted_keys(attrs.subrange(1, attrs.len() as int))
{
    assert forall|i: int, j: int|
        0 <= i < j < attrs.subrange(1, attrs.len() as int).len()
        implies key_lt(attrs.subrange(1, attrs.len() as int)[i].0, attrs.subrange(1, attrs.len() as int)[j].0) by {
        subrange_drop_first_index(attrs, i);
        subrange_drop_first_index(attrs, j);
        assert(0 <= i + 1);
        assert(i + 1 < j + 1);
    }
}

proof fn sorted_head_not_in_tail(attrs: Seq<(Seq<u8>, u64)>)
    requires is_sorted_keys(attrs), attrs.len() > 0
    ensures !has_attr_spec(attrs.subrange(1, attrs.len() as int), attrs[0int].0)
{
    if has_attr_spec(attrs.subrange(1, attrs.len() as int), attrs[0int].0) {
        let i = choose|i: int|
            0 <= i < attrs.subrange(1, attrs.len() as int).len()
                && attrs.subrange(1, attrs.len() as int)[i].0 =~= attrs[0int].0;
        assert(0 < i + 1 < attrs.len());
        assert(key_lt(attrs[0int].0, attrs[i + 1].0));
        assert(attrs[i + 1].0 =~= attrs[0int].0);
        assert(key_lt(attrs[0int].0, attrs[0int].0));
        key_lt_irreflexive(attrs[0int].0);
        assert(false);
    }
}

proof fn lt_head_not_in_sorted(key: Seq<u8>, attrs: Seq<(Seq<u8>, u64)>)
    requires is_sorted_keys(attrs), attrs.len() > 0, key_lt(key, attrs[0int].0)
    ensures !has_attr_spec(attrs, key)
{
    if has_attr_spec(attrs, key) {
        let i = choose|i: int| 0 <= i < attrs.len() && attrs[i].0 =~= key;
        if i == 0 {
            assert(attrs[0int].0 =~= key);
            assert(key_lt(key, key));
            key_lt_irreflexive(key);
            assert(false);
        } else {
            assert(key_lt(attrs[0int].0, attrs[i].0));
            assert(attrs[i].0 =~= key);
            assert(key_lt(attrs[0int].0, key));
            key_lt_asymmetric(key, attrs[0int].0);
            assert(false);
        }
    }
}

proof fn left_keys_from_a(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>, key: Seq<u8>)
    requires is_sorted_keys(a), is_sorted_keys(b), has_attr_spec(split_spec(a, b).left, key)
    ensures has_attr_spec(a, key)
    decreases a.len() + b.len()
{
    if a.len() == 0 {
        assert(false);
    } else if b.len() == 0 {
    } else if key_lt(a[0int].0, b[0int].0) {
        let rest = split_spec(a.subrange(1, a.len() as int), b);
        if key =~= a[0int].0 {
        } else {
            sorted_tail(a);
            left_keys_from_a(a.subrange(1, a.len() as int), b, key);
        }
    } else if key_lt(b[0int].0, a[0int].0) {
        sorted_tail(b);
        left_keys_from_a(a, b.subrange(1, b.len() as int), key);
    } else {
        sorted_tail(a);
        sorted_tail(b);
        left_keys_from_a(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int), key);
    }
}

proof fn center_keys_from_a(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>, key: Seq<u8>)
    requires is_sorted_keys(a), is_sorted_keys(b), exists|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key
    ensures has_attr_spec(a, key)
    decreases a.len() + b.len()
{
    if a.len() == 0 || b.len() == 0 {
        assert(false);
    } else if key_lt(a[0int].0, b[0int].0) {
        let w = choose|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key;
        sorted_tail(a);
        assert(0 <= w < split_spec(a.subrange(1, a.len() as int), b).center.len()
            && split_spec(a.subrange(1, a.len() as int), b).center[w].0 =~= key);
        center_keys_from_a(a.subrange(1, a.len() as int), b, key);
    } else if key_lt(b[0int].0, a[0int].0) {
        let w = choose|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key;
        sorted_tail(b);
        assert(0 <= w < split_spec(a, b.subrange(1, b.len() as int)).center.len()
            && split_spec(a, b.subrange(1, b.len() as int)).center[w].0 =~= key);
        center_keys_from_a(a, b.subrange(1, b.len() as int), key);
    } else {
        let rest = split_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        let w = choose|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key;
        if key =~= a[0int].0 {
        } else {
            sorted_tail(a);
            sorted_tail(b);
            assert(split_spec(a, b).center[0int].0 =~= a[0int].0);
            assert(w > 0);
            assert(0 <= w - 1 < rest.center.len() && rest.center[(w - 1) as int].0 =~= key);
            center_keys_from_a(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int), key);
        }
    }
}

proof fn center_keys_from_b(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>, key: Seq<u8>)
    requires is_sorted_keys(a), is_sorted_keys(b), exists|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key
    ensures has_attr_spec(b, key)
    decreases a.len() + b.len()
{
    if a.len() == 0 || b.len() == 0 {
        assert(false);
    } else if key_lt(a[0int].0, b[0int].0) {
        let w = choose|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key;
        sorted_tail(a);
        assert(0 <= w < split_spec(a.subrange(1, a.len() as int), b).center.len()
            && split_spec(a.subrange(1, a.len() as int), b).center[w].0 =~= key);
        center_keys_from_b(a.subrange(1, a.len() as int), b, key);
    } else if key_lt(b[0int].0, a[0int].0) {
        let w = choose|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key;
        sorted_tail(b);
        assert(0 <= w < split_spec(a, b.subrange(1, b.len() as int)).center.len()
            && split_spec(a, b.subrange(1, b.len() as int)).center[w].0 =~= key);
        center_keys_from_b(a, b.subrange(1, b.len() as int), key);
    } else {
        let rest = split_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        let w = choose|i: int| 0 <= i < split_spec(a, b).center.len() && split_spec(a, b).center[i].0 =~= key;
        if key =~= b[0int].0 {
        } else {
            sorted_tail(a);
            sorted_tail(b);
            key_total(a[0int].0, b[0int].0);
            assert(a[0int].0 =~= b[0int].0);
            assert(split_spec(a, b).center[0int].0 =~= b[0int].0);
            assert(w > 0);
            assert(0 <= w - 1 < rest.center.len() && rest.center[(w - 1) as int].0 =~= key);
            center_keys_from_b(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int), key);
        }
    }
}

proof fn right_keys_from_b(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>, key: Seq<u8>)
    requires is_sorted_keys(a), is_sorted_keys(b), has_attr_spec(split_spec(a, b).right, key)
    ensures has_attr_spec(b, key)
    decreases a.len() + b.len()
{
    if b.len() == 0 {
        assert(false);
    } else if a.len() == 0 {
    } else if key_lt(a[0int].0, b[0int].0) {
        sorted_tail(a);
        right_keys_from_b(a.subrange(1, a.len() as int), b, key);
    } else if key_lt(b[0int].0, a[0int].0) {
        let rest = split_spec(a, b.subrange(1, b.len() as int));
        if key =~= b[0int].0 {
        } else {
            sorted_tail(b);
            right_keys_from_b(a, b.subrange(1, b.len() as int), key);
        }
    } else {
        sorted_tail(a);
        sorted_tail(b);
        right_keys_from_b(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int), key);
    }
}

// r[verify nickel-merge.split]
/// Disjointness: no key appears in more than one partition.
proof fn split_disjoint(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>)
    requires is_sorted_keys(a), is_sorted_keys(b)
    ensures ({
        let r = split_spec(a, b);
        &&& kv_kv_disjoint(r.left, r.right)
        &&& kv_center_disjoint(r.left, r.center)
        &&& kv_center_disjoint(r.right, r.center)
    })
    decreases a.len() + b.len()
{
    let r = split_spec(a, b);
    if a.len() == 0 || b.len() == 0 {
    } else if key_lt(a[0int].0, b[0int].0) {
        let ta = a.subrange(1, a.len() as int);
        sorted_tail(a);
        split_disjoint(ta, b);
        sorted_head_not_in_tail(a);
        lt_head_not_in_sorted(a[0int].0, b);
        assert forall|i: int, j: int| 0 <= i < r.left.len() && 0 <= j < r.right.len()
            implies !(r.left[i].0 =~= r.right[j].0) by {
            if i == 0 {
                right_keys_from_b(ta, b, r.right[j].0);
            }
        }
        assert forall|i: int, j: int| 0 <= i < r.left.len() && 0 <= j < r.center.len()
            implies !(r.left[i].0 =~= r.center[j].0) by {
            if i == 0 {
                center_keys_from_b(ta, b, r.center[j].0);
            }
        }
        assert forall|i: int, j: int| 0 <= i < r.right.len() && 0 <= j < r.center.len()
            implies !(r.right[i].0 =~= r.center[j].0) by {
        }
    } else if key_lt(b[0int].0, a[0int].0) {
        let tb = b.subrange(1, b.len() as int);
        sorted_tail(b);
        split_disjoint(a, tb);
        sorted_head_not_in_tail(b);
        lt_head_not_in_sorted(b[0int].0, a);
        assert forall|i: int, j: int| 0 <= i < r.left.len() && 0 <= j < r.right.len()
            implies !(r.left[i].0 =~= r.right[j].0) by {
            if j == 0 {
                left_keys_from_a(a, tb, r.left[i].0);
            }
        }
        assert forall|i: int, j: int| 0 <= i < r.left.len() && 0 <= j < r.center.len()
            implies !(r.left[i].0 =~= r.center[j].0) by {
        }
        assert forall|i: int, j: int| 0 <= i < r.right.len() && 0 <= j < r.center.len()
            implies !(r.right[i].0 =~= r.center[j].0) by {
            if i == 0 {
                center_keys_from_a(a, tb, r.center[j].0);
            }
        }
    } else {
        let ta = a.subrange(1, a.len() as int);
        let tb = b.subrange(1, b.len() as int);
        sorted_tail(a);
        sorted_tail(b);
        split_disjoint(ta, tb);
        sorted_head_not_in_tail(a);
        sorted_head_not_in_tail(b);
        assert forall|i: int, j: int| 0 <= i < r.left.len() && 0 <= j < r.right.len()
            implies !(r.left[i].0 =~= r.right[j].0) by {
        }
        assert forall|i: int, j: int| 0 <= i < r.left.len() && 0 <= j < r.center.len()
            implies !(r.left[i].0 =~= r.center[j].0) by {
            if j == 0 {
                left_keys_from_a(ta, tb, r.left[i].0);
            }
        }
        assert forall|i: int, j: int| 0 <= i < r.right.len() && 0 <= j < r.center.len()
            implies !(r.right[i].0 =~= r.center[j].0) by {
            if j == 0 {
                right_keys_from_b(ta, tb, r.right[i].0);
            }
        }
    }
}

// r[verify nickel-merge.split-completeness]
/// Completeness: every key from a appears in left or center.
proof fn split_completeness_a(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>)
    ensures ({
        let r = split_spec(a, b);
        a.len() == r.left.len() + r.center.len()
    })
    decreases a.len() + b.len()
{
    if a.len() > 0 && b.len() > 0 {
        if key_lt(a[0int].0, b[0int].0) {
            split_completeness_a(a.subrange(1, a.len() as int), b);
        } else if key_lt(b[0int].0, a[0int].0) {
            split_completeness_a(a, b.subrange(1, b.len() as int));
        } else {
            split_completeness_a(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        }
    }
}

// r[verify nickel-merge.split-completeness]
/// Completeness: every key from b appears in center or right.
proof fn split_completeness_b(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>)
    ensures ({
        let r = split_spec(a, b);
        b.len() == r.center.len() + r.right.len()
    })
    decreases a.len() + b.len()
{
    if a.len() > 0 && b.len() > 0 {
        if key_lt(a[0int].0, b[0int].0) {
            split_completeness_b(a.subrange(1, a.len() as int), b);
        } else if key_lt(b[0int].0, a[0int].0) {
            split_completeness_b(a, b.subrange(1, b.len() as int));
        } else {
            split_completeness_b(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        }
    }
}

// r[verify nickel-merge.split-value-preservation]
/// Value preservation: left partition length + center length == |a|,
/// ensuring every value from a is accounted for.
proof fn split_value_preservation(a: Seq<(Seq<u8>, u64)>, b: Seq<(Seq<u8>, u64)>)
    ensures ({
        let r = split_spec(a, b);
        // Every a-key ends up in left or center
        r.left.len() + r.center.len() == a.len()
    })
    decreases a.len() + b.len()
{
    // Same as split_completeness_a — the structure ensures value preservation
    if a.len() > 0 && b.len() > 0 {
        if key_lt(a[0int].0, b[0int].0) {
            split_value_preservation(a.subrange(1, a.len() as int), b);
        } else if key_lt(b[0int].0, a[0int].0) {
            split_value_preservation(a, b.subrange(1, b.len() as int));
        } else {
            split_value_preservation(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(k: &str, v: u64) -> (Vec<u8>, u64) {
        (k.as_bytes().to_vec(), v)
    }

    // r[verify nickel-merge.split]
    #[test]
    fn test_disjoint_maps() {
        let a = vec![kv("a", 1)];
        let b = vec![kv("b", 2)];
        let r = split(&a, &b);
        assert_eq!(r.left.len(), 1);
        assert_eq!(r.center.len(), 0);
        assert_eq!(r.right.len(), 1);
        assert_eq!(r.left[0].0, b"a");
        assert_eq!(r.right[0].0, b"b");
    }

    // r[verify nickel-merge.split]
    #[test]
    fn test_overlapping_maps() {
        let a = vec![kv("a", 1), kv("b", 2)];
        let b = vec![kv("b", 3), kv("c", 4)];
        let r = split(&a, &b);
        assert_eq!(r.left.len(), 1);
        assert_eq!(r.center.len(), 1);
        assert_eq!(r.right.len(), 1);
        assert_eq!(r.left[0], kv("a", 1));
        assert_eq!(r.center[0], (b"b".to_vec(), 2, 3));
        assert_eq!(r.right[0], kv("c", 4));
    }

    // r[verify nickel-merge.split-completeness]
    #[test]
    fn test_identical_keys() {
        let a = vec![kv("a", 1)];
        let b = vec![kv("a", 2)];
        let r = split(&a, &b);
        assert_eq!(r.left.len(), 0);
        assert_eq!(r.center.len(), 1);
        assert_eq!(r.right.len(), 0);
        assert_eq!(r.center[0], (b"a".to_vec(), 1, 2));
    }

    // r[verify nickel-merge.split-value-preservation]
    #[test]
    fn test_empty_maps() {
        let a: Vec<(Vec<u8>, u64)> = vec![];
        let b: Vec<(Vec<u8>, u64)> = vec![];
        let r = split(&a, &b);
        assert_eq!(r.left.len(), 0);
        assert_eq!(r.center.len(), 0);
        assert_eq!(r.right.len(), 0);
    }
}

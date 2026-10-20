//! Verified Nickel full record merge.
//!
//! Uses a direct two-pointer merge over sorted records, resolving equal-key
//! conflicts with Nickel field-merge semantics.
//! Proves identity, key coverage, disjoint commutativity, and neutral-record
//! associativity.

#![allow(clippy::too_many_lines, unused_variables)]

use vstd::prelude::*;

use crate::nickel_field_merge::FieldMergeResult;
use crate::nickel_field_merge::field_merge;
#[cfg(verus_keep_ghost)]
use crate::nickel_field_merge::field_merge_spec;
use crate::nickel_priority::*;

verus! {

// Re-use the key comparison from nickel_merge_split.
pub open spec fn key_cmp(a: Seq<u8>, b: Seq<u8>) -> int {
    crate::nickel_merge_split::key_cmp(a, b)
}

pub open spec fn key_lt(a: Seq<u8>, b: Seq<u8>) -> bool {
    crate::nickel_merge_split::key_lt(a, b)
}

/// A record field in the spec model.
pub struct Field {
    pub key: Seq<u8>,
    pub value: u64,
    pub priority: MergePriority,
}

/// Runtime field representation.
#[derive(Debug, PartialEq, Eq)]
pub struct ExecField {
    pub key: Vec<u8>,
    pub value: u64,
    pub priority: MergePriority,
}

pub open spec fn field_view(f: ExecField) -> Field {
    Field {
        key: f.key@,
        value: f.value,
        priority: f.priority,
    }
}

pub open spec fn field_ref_view(f: &ExecField) -> Field {
    Field {
        key: f.key@,
        value: f.value,
        priority: f.priority,
    }
}

pub open spec fn fields_view(fs: Seq<ExecField>) -> Seq<Field> {
    Seq::new(fs.len(), |i: int| field_view(fs[i]))
}

/// Spec: is a record sorted by key?
pub open spec fn is_sorted_record(r: Seq<Field>) -> bool {
    forall|i: int, j: int| 0 <= i < j < r.len()
        ==> key_lt(r[i].key, r[j].key)
}

/// Spec: all fields have priority Neutral.
pub open spec fn all_neutral(r: Seq<Field>) -> bool {
    forall|i: int| 0 <= i < r.len() ==> priority_eq(r[i].priority, MergePriority::Neutral)
}

/// Spec: does a record contain a key?
pub open spec fn has_key_spec(r: Seq<Field>, key: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < r.len() && r[i].key =~= key
}

/// Spec: keys of two records are disjoint.
pub open spec fn disjoint_keys(a: Seq<Field>, b: Seq<Field>) -> bool {
    forall|i: int, j: int| 0 <= i < a.len() && 0 <= j < b.len()
        ==> !(a[i].key =~= b[j].key)
}

/// Spec-level field resolution.
/// Higher priority wins. Equal-priority cases are right-biased.
pub open spec fn resolve_field_spec(left: Field, right: Field) -> Field {
    match field_merge_spec(left.value, left.priority, right.value, right.priority) {
        FieldMergeResult::Left(_, _) => left,
        FieldMergeResult::Right(_, _) => right,
        FieldMergeResult::Equal(_, _) => right,
        FieldMergeResult::Conflict => right,
    }
}

/// Spec: merge two sorted records. Uses two-pointer merge.
// r[depends nickel-record-merge.identity]
// r[depends nickel-record-merge.key-coverage]
// r[depends nickel-record-merge.disjoint-commutativity]
// r[depends nickel-record-merge.associativity]
pub open spec fn record_merge_spec(a: Seq<Field>, b: Seq<Field>) -> Seq<Field>
    decreases a.len() + b.len()
{
    if a.len() == 0 && b.len() == 0 {
        Seq::<Field>::empty()
    } else if a.len() == 0 {
        b
    } else if b.len() == 0 {
        a
    } else if key_lt(a[0int].key, b[0int].key) {
        seq![a[0int]] + record_merge_spec(a.subrange(1, a.len() as int), b)
    } else if key_lt(b[0int].key, a[0int].key) {
        seq![b[0int]] + record_merge_spec(a, b.subrange(1, b.len() as int))
    } else {
        seq![resolve_field_spec(a[0int], b[0int])] + record_merge_spec(
            a.subrange(1, a.len() as int),
            b.subrange(1, b.len() as int),
        )
    }
}

/// Empty record.
pub open spec fn empty_record() -> Seq<Field> {
    Seq::<Field>::empty()
}

proof fn subrange_drop_first_index<A>(s: Seq<A>, i: int)
    requires s.len() > 0, 0 <= i < s.subrange(1, s.len() as int).len()
    ensures s.subrange(1, s.len() as int)[i] == s[i + 1]
{}

proof fn seq_head_tail_eq<A>(s: Seq<A>)
    requires s.len() > 0
    ensures s =~= seq![s[0int]] + s.subrange(1, s.len() as int)
{
    assert forall|i: int| 0 <= i < s.len()
        implies s[i] == (seq![s[0int]] + s.subrange(1, s.len() as int))[i] by {
        if i == 0 {
        } else {
            subrange_drop_first_index(s, i - 1);
        }
    }
}

proof fn cons_subrange_tail<A>(head: A, tail: Seq<A>)
    ensures (seq![head] + tail).subrange(1, (seq![head] + tail).len() as int) =~= tail
{}

proof fn sorted_tail(r: Seq<Field>)
    requires is_sorted_record(r), r.len() > 0
    ensures is_sorted_record(r.subrange(1, r.len() as int))
{
    assert forall|i: int, j: int|
        0 <= i < j < r.subrange(1, r.len() as int).len()
        implies key_lt(r.subrange(1, r.len() as int)[i].key, r.subrange(1, r.len() as int)[j].key) by {
        subrange_drop_first_index(r, i);
        subrange_drop_first_index(r, j);
        assert(0 <= i + 1 < j + 1 < r.len());
    }
}

proof fn disjoint_tail_left(a: Seq<Field>, b: Seq<Field>)
    requires disjoint_keys(a, b), a.len() > 0
    ensures disjoint_keys(a.subrange(1, a.len() as int), b)
{
    assert forall|i: int, j: int|
        0 <= i < a.subrange(1, a.len() as int).len() && 0 <= j < b.len()
        implies !(a.subrange(1, a.len() as int)[i].key =~= b[j].key) by {
        subrange_drop_first_index(a, i);
        assert(0 <= i + 1 < a.len());
    }
}

proof fn disjoint_tail_right(a: Seq<Field>, b: Seq<Field>)
    requires disjoint_keys(a, b), b.len() > 0
    ensures disjoint_keys(a, b.subrange(1, b.len() as int))
{
    assert forall|i: int, j: int|
        0 <= i < a.len() && 0 <= j < b.subrange(1, b.len() as int).len()
        implies !(a[i].key =~= b.subrange(1, b.len() as int)[j].key) by {
        subrange_drop_first_index(b, j);
        assert(0 <= j + 1 < b.len());
    }
}

proof fn has_key_in_tail_if_not_head(r: Seq<Field>, key: Seq<u8>)
    requires has_key_spec(r, key), r.len() > 0, !(r[0int].key =~= key)
    ensures has_key_spec(r.subrange(1, r.len() as int), key)
{
    let i = choose|i: int| 0 <= i < r.len() && r[i].key =~= key;
    assert(i > 0);
    assert(0 <= i - 1 < r.subrange(1, r.len() as int).len());
    subrange_drop_first_index(r, i - 1);
}

proof fn has_key_in_cons(head: Field, tail: Seq<Field>, key: Seq<u8>)
    requires head.key =~= key || has_key_spec(tail, key)
    ensures has_key_spec(seq![head] + tail, key)
{
    if head.key =~= key {
        assert((seq![head] + tail)[0int].key =~= key);
    } else {
        let i = choose|i: int| 0 <= i < tail.len() && tail[i].key =~= key;
        assert(0 <= i + 1 < (seq![head] + tail).len());
        assert((seq![head] + tail)[i + 1].key =~= key);
    }
}

proof fn all_neutral_tail(r: Seq<Field>)
    requires all_neutral(r), r.len() > 0
    ensures all_neutral(r.subrange(1, r.len() as int))
{
    assert forall|i: int|
        0 <= i < r.subrange(1, r.len() as int).len()
        implies priority_eq(r.subrange(1, r.len() as int)[i].priority, MergePriority::Neutral) by {
        subrange_drop_first_index(r, i);
        assert(0 <= i + 1 < r.len());
    }
}

proof fn resolve_equal_priority_is_right(left: Field, right: Field)
    requires left.key =~= right.key, priority_eq(left.priority, right.priority)
    ensures resolve_field_spec(left, right) == right
{
    assert(!priority_lt(left.priority, right.priority));
    assert(!priority_lt(right.priority, left.priority));
    if left.value == right.value {
        assert(field_merge_spec(left.value, left.priority, right.value, right.priority)
            == FieldMergeResult::Equal(left.value, left.priority));
    } else {
        assert(field_merge_spec(left.value, left.priority, right.value, right.priority)
            == FieldMergeResult::Conflict);
    }
}

proof fn merge_expand_left(a: Seq<Field>, b: Seq<Field>)
    requires a.len() > 0, b.len() > 0, key_lt(a[0int].key, b[0int].key)
    ensures record_merge_spec(a, b) =~= seq![a[0int]] + record_merge_spec(a.subrange(1, a.len() as int), b)
{}

proof fn merge_expand_right(a: Seq<Field>, b: Seq<Field>)
    requires a.len() > 0, b.len() > 0, key_lt(b[0int].key, a[0int].key)
    ensures record_merge_spec(a, b) =~= seq![b[0int]] + record_merge_spec(a, b.subrange(1, b.len() as int))
{
    key_lt_asymmetric(b[0int].key, a[0int].key);
    assert(!key_lt(a[0int].key, b[0int].key));
}

proof fn merge_expand_equal(a: Seq<Field>, b: Seq<Field>)
    requires
        a.len() > 0,
        b.len() > 0,
        !key_lt(a[0int].key, b[0int].key),
        !key_lt(b[0int].key, a[0int].key),
    ensures record_merge_spec(a, b) =~=
        seq![resolve_field_spec(a[0int], b[0int])] + record_merge_spec(
            a.subrange(1, a.len() as int),
            b.subrange(1, b.len() as int),
        )
{}

proof fn merge_head_gt_if_lt_both(a: Seq<Field>, b: Seq<Field>, key: Seq<u8>)
    requires
        a.len() > 0,
        b.len() > 0,
        key_lt(key, a[0int].key),
        key_lt(key, b[0int].key),
    ensures key_lt(key, record_merge_spec(a, b)[0int].key)
{
    crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
    if key_lt(a[0int].key, b[0int].key) {
        assert(record_merge_spec(a, b)[0int].key =~= a[0int].key);
    } else if key_lt(b[0int].key, a[0int].key) {
        assert(record_merge_spec(a, b)[0int].key =~= b[0int].key);
    } else {
        assert(a[0int].key =~= b[0int].key);
        assert(resolve_field_spec(a[0int], b[0int]).key =~= a[0int].key);
        assert(record_merge_spec(a, b)[0int].key =~= resolve_field_spec(a[0int], b[0int]).key);
    }
}

proof fn merge_head_lt_if_both_lt(a: Seq<Field>, b: Seq<Field>, key: Seq<u8>)
    requires
        a.len() > 0,
        b.len() > 0,
        key_lt(a[0int].key, key),
        key_lt(b[0int].key, key),
    ensures key_lt(record_merge_spec(a, b)[0int].key, key)
{
    crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
    if key_lt(a[0int].key, b[0int].key) {
        assert(record_merge_spec(a, b)[0int].key =~= a[0int].key);
    } else if key_lt(b[0int].key, a[0int].key) {
        assert(record_merge_spec(a, b)[0int].key =~= b[0int].key);
    } else {
        assert(a[0int].key =~= b[0int].key);
        assert(resolve_field_spec(a[0int], b[0int]).key =~= a[0int].key);
        assert(record_merge_spec(a, b)[0int].key =~= resolve_field_spec(a[0int], b[0int]).key);
    }
}

proof fn key_lt_asymmetric(a: Seq<u8>, b: Seq<u8>)
    requires key_lt(a, b)
    ensures !key_lt(b, a)
{
    crate::nickel_merge_split::key_cmp_antisymmetric(a, b);
}

proof fn key_lt_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
    requires key_lt(a, b), key_lt(b, c)
    ensures key_lt(a, c)
    decreases a.len() + b.len() + c.len()
{
    if a.len() == 0 {
    } else if b.len() == 0 {
        assert(false);
    } else if c.len() == 0 {
        assert(false);
    } else if a[0int] < b[0int] {
        if b[0int] < c[0int] {
        } else {
            assert(b[0int] == c[0int]);
        }
    } else {
        assert(a[0int] == b[0int]);
        if b[0int] < c[0int] {
        } else {
            assert(b[0int] == c[0int]);
            key_lt_transitive(
                a.subrange(1, a.len() as int),
                b.subrange(1, b.len() as int),
                c.subrange(1, c.len() as int),
            );
        }
    }
}

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

proof fn record_merge_spec_right_empty(a: Seq<Field>)
    ensures record_merge_spec(a, Seq::<Field>::empty()) =~= a
    decreases a.len()
{
    if a.len() > 0 {
    }
}

proof fn record_merge_spec_left_empty(b: Seq<Field>)
    ensures record_merge_spec(Seq::<Field>::empty(), b) =~= b
{}

proof fn subrange_decompose<T>(s: Seq<T>, i: int, j: int)
    requires 0 <= i < j, j <= s.len()
    ensures s.subrange(i, j) =~= seq![s[i]] + s.subrange(i + 1, j)
{}

fn clone_priority(p: &MergePriority) -> (result: MergePriority)
    ensures result == *p
{
    match p {
        MergePriority::Bottom => MergePriority::Bottom,
        MergePriority::Neutral => MergePriority::Neutral,
        MergePriority::Numeral(n) => MergePriority::Numeral(*n),
        MergePriority::Top => MergePriority::Top,
    }
}

fn clone_field(f: &ExecField) -> (result: ExecField)
    ensures field_view(result) == field_ref_view(f)
{
    ExecField {
        key: f.key.clone(),
        value: f.value,
        priority: clone_priority(&f.priority),
    }
}

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

fn resolve_field(left: &ExecField, right: &ExecField) -> (result: ExecField)
    ensures field_view(result) == resolve_field_spec(field_ref_view(left), field_ref_view(right))
{
    let merged = field_merge(left.value, &left.priority, right.value, &right.priority);
    match merged {
        FieldMergeResult::Left(_, _) => clone_field(left),
        FieldMergeResult::Right(_, _) | FieldMergeResult::Equal(_, _) | FieldMergeResult::Conflict => clone_field(right),
    }
}

// r[impl nickel-record-merge.identity]
// r[impl nickel-record-merge.key-coverage]
// r[impl nickel-record-merge.disjoint-commutativity]
// r[impl nickel-record-merge.associativity]
pub fn record_merge(a: &Vec<ExecField>, b: &Vec<ExecField>) -> (result: Vec<ExecField>)
    ensures fields_view(result@) =~= record_merge_spec(fields_view(a@), fields_view(b@))
{
    let ghost ma = fields_view(a@);
    let ghost mb = fields_view(b@);
    let ghost full = record_merge_spec(ma, mb);

    let mut result: Vec<ExecField> = Vec::new();
    let ghost mut rg: Seq<Field> = Seq::empty();
    let mut ai: usize = 0;
    let mut bi: usize = 0;

    proof {
        assert(Seq::<Field>::empty() + record_merge_spec(ma, mb) =~= record_merge_spec(ma, mb));
        assert(ma.subrange(0int, ma.len() as int) =~= ma);
        assert(mb.subrange(0int, mb.len() as int) =~= mb);
    }

    while ai < a.len() && bi < b.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ma =~= fields_view(a@),
            mb =~= fields_view(b@),
            full =~= record_merge_spec(ma, mb),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len()
                ==> rg[k] =~= field_view(#[trigger] result@[k]),
            full =~= rg + record_merge_spec(
                ma.subrange(ai as int, ma.len() as int),
                mb.subrange(bi as int, mb.len() as int)),
        decreases a.len() + b.len() - ai - bi,
    {
        let ghost rem_a = ma.subrange(ai as int, ma.len() as int);
        let ghost rem_b = mb.subrange(bi as int, mb.len() as int);
        assert(rem_a.len() > 0 && rem_b.len() > 0);
        assert(rem_a[0int] =~= ma[ai as int]);
        assert(rem_b[0int] =~= mb[bi as int]);
        assert(rem_a.subrange(1, rem_a.len() as int)
            =~= ma.subrange(ai as int + 1, ma.len() as int));
        assert(rem_b.subrange(1, rem_b.len() as int)
            =~= mb.subrange(bi as int + 1, mb.len() as int));

        let cmp = key_cmp_exec(&a[ai].key, &b[bi].key);
        proof { crate::nickel_merge_split::key_cmp_antisymmetric(rem_a[0int].key, rem_b[0int].key); }

        if cmp < 0 {
            assert(key_lt(rem_a[0int].key, rem_b[0int].key));
            let chosen = clone_field(&a[ai]);
            result.push(chosen);
            proof { rg = rg + seq![ma[ai as int]]; }
            ai = ai + 1;
        } else if cmp > 0 {
            assert(key_lt(rem_b[0int].key, rem_a[0int].key));
            let chosen = clone_field(&b[bi]);
            result.push(chosen);
            proof { rg = rg + seq![mb[bi as int]]; }
            bi = bi + 1;
        } else {
            assert(!key_lt(rem_a[0int].key, rem_b[0int].key));
            assert(!key_lt(rem_b[0int].key, rem_a[0int].key));
            let chosen = resolve_field(&a[ai], &b[bi]);
            result.push(chosen);
            proof { rg = rg + seq![resolve_field_spec(ma[ai as int], mb[bi as int])]; }
            ai = ai + 1;
            bi = bi + 1;
        }
    }

    while ai < a.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ai >= a.len() || bi >= b.len(),
            ma =~= fields_view(a@),
            mb =~= fields_view(b@),
            full =~= record_merge_spec(ma, mb),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len()
                ==> rg[k] =~= field_view(#[trigger] result@[k]),
            full =~= rg + record_merge_spec(
                ma.subrange(ai as int, ma.len() as int),
                mb.subrange(bi as int, mb.len() as int)),
        decreases a.len() - ai,
    {
        proof {
            let rem_a = ma.subrange(ai as int, ma.len() as int);
            let rem_b = mb.subrange(bi as int, mb.len() as int);
            assert(rem_b.len() == 0);
            record_merge_spec_right_empty(rem_a);
            subrange_decompose(ma, ai as int, ma.len() as int);
        }
        let chosen = clone_field(&a[ai]);
        result.push(chosen);
        proof { rg = rg + seq![ma[ai as int]]; }
        ai = ai + 1;
    }

    while bi < b.len()
        invariant
            ai <= a.len(),
            bi <= b.len(),
            ai >= a.len() || bi >= b.len(),
            ma =~= fields_view(a@),
            mb =~= fields_view(b@),
            full =~= record_merge_spec(ma, mb),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len()
                ==> rg[k] =~= field_view(#[trigger] result@[k]),
            full =~= rg + record_merge_spec(
                ma.subrange(ai as int, ma.len() as int),
                mb.subrange(bi as int, mb.len() as int)),
        decreases b.len() - bi,
    {
        proof {
            let rem_a = ma.subrange(ai as int, ma.len() as int);
            let rem_b = mb.subrange(bi as int, mb.len() as int);
            assert(rem_a.len() == 0);
            record_merge_spec_left_empty(rem_b);
            subrange_decompose(mb, bi as int, mb.len() as int);
        }
        let chosen = clone_field(&b[bi]);
        result.push(chosen);
        proof { rg = rg + seq![mb[bi as int]]; }
        bi = bi + 1;
    }

    proof {
        let rem_a = ma.subrange(ai as int, ma.len() as int);
        let rem_b = mb.subrange(bi as int, mb.len() as int);
        assert(rem_a.len() == 0 && rem_b.len() == 0);
        assert(record_merge_spec(rem_a, rem_b) =~= Seq::<Field>::empty());
        assert(rg =~= fields_view(result@));
    }
    result
}

// --- Proofs ---

// r[verify nickel-record-merge.identity]
proof fn merge_left_identity(r: Seq<Field>)
    ensures record_merge_spec(empty_record(), r) =~= r
{}

// r[verify nickel-record-merge.identity]
proof fn merge_right_identity(r: Seq<Field>)
    ensures record_merge_spec(r, empty_record()) =~= r
{
    record_merge_spec_right_empty(r);
}

proof fn merge_key_bound(a: Seq<Field>, b: Seq<Field>)
    ensures record_merge_spec(a, b).len() <= a.len() + b.len()
    decreases a.len() + b.len()
{
    if a.len() > 0 && b.len() > 0 {
        if key_lt(a[0int].key, b[0int].key) {
            merge_key_bound(a.subrange(1, a.len() as int), b);
        } else if key_lt(b[0int].key, a[0int].key) {
            merge_key_bound(a, b.subrange(1, b.len() as int));
        } else {
            merge_key_bound(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int));
        }
    }
}

// r[verify nickel-record-merge.key-coverage]
proof fn merge_contains_left_key(a: Seq<Field>, b: Seq<Field>, key: Seq<u8>)
    requires has_key_spec(a, key)
    ensures has_key_spec(record_merge_spec(a, b), key)
    decreases a.len() + b.len()
{
    if a.len() == 0 {
        assert(false);
    } else if b.len() == 0 {
    } else if key_lt(a[0int].key, b[0int].key) {
        if key =~= a[0int].key {
            has_key_in_cons(a[0int], record_merge_spec(a.subrange(1, a.len() as int), b), key);
        } else {
            has_key_in_tail_if_not_head(a, key);
            merge_contains_left_key(a.subrange(1, a.len() as int), b, key);
            has_key_in_cons(a[0int], record_merge_spec(a.subrange(1, a.len() as int), b), key);
        }
    } else if key_lt(b[0int].key, a[0int].key) {
        merge_contains_left_key(a, b.subrange(1, b.len() as int), key);
        has_key_in_cons(b[0int], record_merge_spec(a, b.subrange(1, b.len() as int)), key);
    } else {
        if key =~= a[0int].key {
            crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
            assert(!key_lt(a[0int].key, b[0int].key));
            assert(!key_lt(b[0int].key, a[0int].key));
            assert(a[0int].key =~= b[0int].key);
            assert(resolve_field_spec(a[0int], b[0int]).key =~= key);
            has_key_in_cons(resolve_field_spec(a[0int], b[0int]), record_merge_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int)), key);
        } else {
            has_key_in_tail_if_not_head(a, key);
            merge_contains_left_key(
                a.subrange(1, a.len() as int),
                b.subrange(1, b.len() as int),
                key,
            );
            has_key_in_cons(resolve_field_spec(a[0int], b[0int]), record_merge_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int)), key);
        }
    }
}

// r[verify nickel-record-merge.key-coverage]
proof fn merge_contains_right_key(a: Seq<Field>, b: Seq<Field>, key: Seq<u8>)
    requires has_key_spec(b, key)
    ensures has_key_spec(record_merge_spec(a, b), key)
    decreases a.len() + b.len()
{
    if b.len() == 0 {
        assert(false);
    } else if a.len() == 0 {
    } else if key_lt(a[0int].key, b[0int].key) {
        merge_contains_right_key(a.subrange(1, a.len() as int), b, key);
        has_key_in_cons(a[0int], record_merge_spec(a.subrange(1, a.len() as int), b), key);
    } else if key_lt(b[0int].key, a[0int].key) {
        if key =~= b[0int].key {
            has_key_in_cons(b[0int], record_merge_spec(a, b.subrange(1, b.len() as int)), key);
        } else {
            has_key_in_tail_if_not_head(b, key);
            merge_contains_right_key(a, b.subrange(1, b.len() as int), key);
            has_key_in_cons(b[0int], record_merge_spec(a, b.subrange(1, b.len() as int)), key);
        }
    } else {
        if key =~= b[0int].key {
            crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
            assert(!key_lt(a[0int].key, b[0int].key));
            assert(!key_lt(b[0int].key, a[0int].key));
            assert(a[0int].key =~= b[0int].key);
            assert(resolve_field_spec(a[0int], b[0int]).key =~= key);
            has_key_in_cons(resolve_field_spec(a[0int], b[0int]), record_merge_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int)), key);
        } else {
            has_key_in_tail_if_not_head(b, key);
            merge_contains_right_key(
                a.subrange(1, a.len() as int),
                b.subrange(1, b.len() as int),
                key,
            );
            has_key_in_cons(resolve_field_spec(a[0int], b[0int]), record_merge_spec(a.subrange(1, a.len() as int), b.subrange(1, b.len() as int)), key);
        }
    }
}

// r[verify nickel-record-merge.key-coverage]
proof fn merge_key_from_inputs(a: Seq<Field>, b: Seq<Field>, key: Seq<u8>)
    requires has_key_spec(record_merge_spec(a, b), key)
    ensures has_key_spec(a, key) || has_key_spec(b, key)
    decreases a.len() + b.len()
{
    if a.len() == 0 {
        assert(has_key_spec(b, key));
    } else if b.len() == 0 {
        assert(has_key_spec(a, key));
    } else if key_lt(a[0int].key, b[0int].key) {
        if key =~= a[0int].key {
            assert(has_key_spec(a, key));
        } else {
            merge_key_from_inputs(a.subrange(1, a.len() as int), b, key);
        }
    } else if key_lt(b[0int].key, a[0int].key) {
        if key =~= b[0int].key {
            assert(has_key_spec(b, key));
        } else {
            merge_key_from_inputs(a, b.subrange(1, b.len() as int), key);
        }
    } else {
        if key =~= resolve_field_spec(a[0int], b[0int]).key {
            crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
            assert(!key_lt(a[0int].key, b[0int].key));
            assert(!key_lt(b[0int].key, a[0int].key));
            assert(a[0int].key =~= b[0int].key);
            assert(resolve_field_spec(a[0int], b[0int]).key =~= a[0int].key);
            assert(has_key_spec(a, key));
            assert(has_key_spec(b, key));
        } else {
            merge_key_from_inputs(
                a.subrange(1, a.len() as int),
                b.subrange(1, b.len() as int),
                key,
            );
        }
    }
}

// r[verify nickel-record-merge.disjoint-commutativity]
proof fn merge_disjoint_commutative(a: Seq<Field>, b: Seq<Field>)
    requires is_sorted_record(a), is_sorted_record(b), disjoint_keys(a, b)
    ensures record_merge_spec(a, b) =~= record_merge_spec(b, a)
    decreases a.len() + b.len()
{
    if a.len() == 0 {
        record_merge_spec_left_empty(b);
        record_merge_spec_right_empty(b);
    } else if b.len() == 0 {
        record_merge_spec_right_empty(a);
        record_merge_spec_left_empty(a);
    } else {
        crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
        assert(!(a[0int].key =~= b[0int].key));
        if key_lt(a[0int].key, b[0int].key) {
            let ta = a.subrange(1, a.len() as int);
            sorted_tail(a);
            disjoint_tail_left(a, b);
            merge_disjoint_commutative(ta, b);
            key_lt_asymmetric(a[0int].key, b[0int].key);
            assert(record_merge_spec(a, b) =~= seq![a[0int]] + record_merge_spec(ta, b));
            assert(record_merge_spec(b, a) =~= seq![a[0int]] + record_merge_spec(b, ta));
        } else {
            let tb = b.subrange(1, b.len() as int);
            assert(key_lt(b[0int].key, a[0int].key));
            sorted_tail(b);
            disjoint_tail_right(a, b);
            merge_disjoint_commutative(a, tb);
            key_lt_asymmetric(b[0int].key, a[0int].key);
            assert(record_merge_spec(a, b) =~= seq![b[0int]] + record_merge_spec(a, tb));
            assert(record_merge_spec(b, a) =~= seq![b[0int]] + record_merge_spec(tb, a));
        }
    }
}

// r[verify nickel-record-merge.associativity]
proof fn merge_associative_neutral(
    a: Seq<Field>, b: Seq<Field>, c: Seq<Field>,
)
    requires
        is_sorted_record(a),
        is_sorted_record(b),
        is_sorted_record(c),
        all_neutral(a),
        all_neutral(b),
        all_neutral(c),
    ensures
        record_merge_spec(record_merge_spec(a, b), c)
            =~= record_merge_spec(a, record_merge_spec(b, c)),
    decreases a.len() + b.len() + c.len()
{
    if a.len() == 0 {
        record_merge_spec_left_empty(b);
        record_merge_spec_left_empty(record_merge_spec(b, c));
        assert(record_merge_spec(record_merge_spec(a, b), c) =~= record_merge_spec(b, c));
        assert(record_merge_spec(a, record_merge_spec(b, c)) =~= record_merge_spec(b, c));
    } else if b.len() == 0 {
        record_merge_spec_right_empty(a);
        record_merge_spec_left_empty(c);
        record_merge_spec_right_empty(record_merge_spec(a, Seq::<Field>::empty()));
        record_merge_spec_right_empty(a);
        assert(record_merge_spec(record_merge_spec(a, b), c) =~= record_merge_spec(a, c));
        assert(record_merge_spec(a, record_merge_spec(b, c)) =~= record_merge_spec(a, c));
    } else if c.len() == 0 {
        record_merge_spec_right_empty(record_merge_spec(a, b));
        record_merge_spec_right_empty(b);
        record_merge_spec_right_empty(a);
        assert(record_merge_spec(record_merge_spec(a, b), c) =~= record_merge_spec(a, b));
        assert(record_merge_spec(a, record_merge_spec(b, c)) =~= record_merge_spec(a, b));
    } else {
        let ta = a.subrange(1, a.len() as int);
        let tb = b.subrange(1, b.len() as int);
        let tc = c.subrange(1, c.len() as int);

        crate::nickel_merge_split::key_total(a[0int].key, b[0int].key);
        crate::nickel_merge_split::key_total(a[0int].key, c[0int].key);
        crate::nickel_merge_split::key_total(b[0int].key, c[0int].key);

        if key_lt(a[0int].key, b[0int].key) {
            if key_lt(a[0int].key, c[0int].key) {
                sorted_tail(a);
                all_neutral_tail(a);
                merge_expand_left(a, b);
                merge_expand_left(seq![a[0int]] + record_merge_spec(ta, b), c);
                merge_head_gt_if_lt_both(b, c, a[0int].key);
                merge_expand_left(a, record_merge_spec(b, c));
                merge_associative_neutral(ta, b, c);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= record_merge_spec(seq![a[0int]] + record_merge_spec(ta, b), c));
                assert((seq![a[0int]] + record_merge_spec(ta, b)).subrange(
                    1,
                    (seq![a[0int]] + record_merge_spec(ta, b)).len() as int,
                ) =~= record_merge_spec(ta, b));
                assert(record_merge_spec(seq![a[0int]] + record_merge_spec(ta, b), c)
                    =~= seq![a[0int]] + record_merge_spec(record_merge_spec(ta, b), c));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= seq![a[0int]] + record_merge_spec(ta, record_merge_spec(b, c)));
            } else if key_lt(c[0int].key, a[0int].key) {
                sorted_tail(c);
                all_neutral_tail(c);
                merge_expand_left(a, b);
                merge_expand_right(seq![a[0int]] + record_merge_spec(ta, b), c);
                key_lt_transitive(c[0int].key, a[0int].key, b[0int].key);
                merge_expand_right(b, c);
                merge_expand_right(a, seq![c[0int]] + record_merge_spec(b, tc));
                merge_associative_neutral(a, b, tc);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= seq![c[0int]] + record_merge_spec(record_merge_spec(a, b), tc));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc)));
                assert((seq![c[0int]] + record_merge_spec(b, tc)).subrange(
                    1,
                    (seq![c[0int]] + record_merge_spec(b, tc)).len() as int,
                ) =~= record_merge_spec(b, tc));
                assert(record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc))
                    =~= seq![c[0int]] + record_merge_spec(a, record_merge_spec(b, tc)));
            } else {
                assert(a[0int].key =~= c[0int].key);
                sorted_tail(a);
                sorted_tail(c);
                all_neutral_tail(a);
                all_neutral_tail(c);
                merge_expand_left(a, b);
                merge_expand_equal(seq![a[0int]] + record_merge_spec(ta, b), c);
                resolve_equal_priority_is_right(a[0int], c[0int]);
                merge_expand_right(b, c);
                merge_expand_equal(a, seq![c[0int]] + record_merge_spec(b, tc));
                resolve_equal_priority_is_right(a[0int], c[0int]);
                merge_associative_neutral(ta, b, tc);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= record_merge_spec(seq![a[0int]] + record_merge_spec(ta, b), c));
                assert((seq![a[0int]] + record_merge_spec(ta, b)).subrange(
                    1,
                    (seq![a[0int]] + record_merge_spec(ta, b)).len() as int,
                ) =~= record_merge_spec(ta, b));
                assert(record_merge_spec(seq![a[0int]] + record_merge_spec(ta, b), c)
                    =~= seq![c[0int]] + record_merge_spec(record_merge_spec(ta, b), tc));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc)));
                assert((seq![c[0int]] + record_merge_spec(b, tc)).subrange(
                    1,
                    (seq![c[0int]] + record_merge_spec(b, tc)).len() as int,
                ) =~= record_merge_spec(b, tc));
                assert(record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc))
                    =~= seq![c[0int]] + record_merge_spec(ta, record_merge_spec(b, tc)));
            }
        } else if key_lt(b[0int].key, a[0int].key) {
            if key_lt(b[0int].key, c[0int].key) {
                sorted_tail(b);
                all_neutral_tail(b);
                merge_expand_right(a, b);
                merge_expand_left(seq![b[0int]] + record_merge_spec(a, tb), c);
                merge_expand_left(b, c);
                merge_expand_right(a, seq![b[0int]] + record_merge_spec(tb, c));
                merge_associative_neutral(a, tb, c);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= record_merge_spec(seq![b[0int]] + record_merge_spec(a, tb), c));
                cons_subrange_tail(b[0int], record_merge_spec(a, tb));
                assert(record_merge_spec(seq![b[0int]] + record_merge_spec(a, tb), c)
                    =~= seq![b[0int]] + record_merge_spec(record_merge_spec(a, tb), c));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![b[0int]] + record_merge_spec(tb, c)));
                cons_subrange_tail(b[0int], record_merge_spec(tb, c));
                assert(record_merge_spec(a, seq![b[0int]] + record_merge_spec(tb, c))
                    =~= seq![b[0int]] + record_merge_spec(a, record_merge_spec(tb, c)));
            } else if key_lt(c[0int].key, b[0int].key) {
                sorted_tail(c);
                all_neutral_tail(c);
                merge_expand_right(a, b);
                merge_expand_right(seq![b[0int]] + record_merge_spec(a, tb), c);
                key_lt_transitive(c[0int].key, b[0int].key, a[0int].key);
                merge_expand_right(b, c);
                merge_expand_right(a, seq![c[0int]] + record_merge_spec(b, tc));
                merge_associative_neutral(a, b, tc);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= seq![c[0int]] + record_merge_spec(record_merge_spec(a, b), tc));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc)));
                cons_subrange_tail(c[0int], record_merge_spec(b, tc));
                assert(record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc))
                    =~= seq![c[0int]] + record_merge_spec(a, record_merge_spec(b, tc)));
            } else {
                assert(b[0int].key =~= c[0int].key);
                sorted_tail(b);
                sorted_tail(c);
                all_neutral_tail(b);
                all_neutral_tail(c);
                merge_expand_right(a, b);
                merge_expand_equal(seq![b[0int]] + record_merge_spec(a, tb), c);
                resolve_equal_priority_is_right(b[0int], c[0int]);
                merge_expand_equal(b, c);
                resolve_equal_priority_is_right(b[0int], c[0int]);
                merge_expand_right(a, seq![c[0int]] + record_merge_spec(tb, tc));
                merge_associative_neutral(a, tb, tc);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= record_merge_spec(seq![b[0int]] + record_merge_spec(a, tb), c));
                cons_subrange_tail(b[0int], record_merge_spec(a, tb));
                assert(record_merge_spec(seq![b[0int]] + record_merge_spec(a, tb), c)
                    =~= seq![c[0int]] + record_merge_spec(record_merge_spec(a, tb), tc));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![c[0int]] + record_merge_spec(tb, tc)));
                cons_subrange_tail(c[0int], record_merge_spec(tb, tc));
                assert(record_merge_spec(a, seq![c[0int]] + record_merge_spec(tb, tc))
                    =~= seq![c[0int]] + record_merge_spec(a, record_merge_spec(tb, tc)));
            }
        } else {
            assert(a[0int].key =~= b[0int].key);
            if key_lt(a[0int].key, c[0int].key) {
                sorted_tail(a);
                sorted_tail(b);
                all_neutral_tail(a);
                all_neutral_tail(b);
                merge_expand_equal(a, b);
                resolve_equal_priority_is_right(a[0int], b[0int]);
                merge_expand_left(seq![b[0int]] + record_merge_spec(ta, tb), c);
                merge_expand_left(b, c);
                merge_expand_equal(a, seq![b[0int]] + record_merge_spec(tb, c));
                resolve_equal_priority_is_right(a[0int], b[0int]);
                merge_associative_neutral(ta, tb, c);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= record_merge_spec(seq![b[0int]] + record_merge_spec(ta, tb), c));
                cons_subrange_tail(b[0int], record_merge_spec(ta, tb));
                assert(record_merge_spec(seq![b[0int]] + record_merge_spec(ta, tb), c)
                    =~= seq![b[0int]] + record_merge_spec(record_merge_spec(ta, tb), c));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![b[0int]] + record_merge_spec(tb, c)));
                cons_subrange_tail(b[0int], record_merge_spec(tb, c));
                assert(record_merge_spec(a, seq![b[0int]] + record_merge_spec(tb, c))
                    =~= seq![b[0int]] + record_merge_spec(ta, record_merge_spec(tb, c)));
            } else if key_lt(c[0int].key, a[0int].key) {
                sorted_tail(c);
                all_neutral_tail(c);
                merge_expand_equal(a, b);
                resolve_equal_priority_is_right(a[0int], b[0int]);
                merge_expand_right(seq![b[0int]] + record_merge_spec(ta, tb), c);
                assert(b[0int].key =~= a[0int].key);
                assert(key_lt(c[0int].key, b[0int].key));
                merge_expand_right(b, c);
                merge_expand_right(a, seq![c[0int]] + record_merge_spec(b, tc));
                merge_associative_neutral(a, b, tc);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= seq![c[0int]] + record_merge_spec(record_merge_spec(a, b), tc));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc)));
                cons_subrange_tail(c[0int], record_merge_spec(b, tc));
                assert(record_merge_spec(a, seq![c[0int]] + record_merge_spec(b, tc))
                    =~= seq![c[0int]] + record_merge_spec(a, record_merge_spec(b, tc)));
            } else {
                assert(a[0int].key =~= c[0int].key);
                assert(b[0int].key =~= c[0int].key);
                sorted_tail(a);
                sorted_tail(b);
                sorted_tail(c);
                all_neutral_tail(a);
                all_neutral_tail(b);
                all_neutral_tail(c);
                merge_expand_equal(a, b);
                resolve_equal_priority_is_right(a[0int], b[0int]);
                merge_expand_equal(seq![b[0int]] + record_merge_spec(ta, tb), c);
                resolve_equal_priority_is_right(b[0int], c[0int]);
                merge_expand_equal(b, c);
                resolve_equal_priority_is_right(b[0int], c[0int]);
                merge_expand_equal(a, seq![c[0int]] + record_merge_spec(tb, tc));
                resolve_equal_priority_is_right(a[0int], c[0int]);
                merge_associative_neutral(ta, tb, tc);
                assert(record_merge_spec(record_merge_spec(a, b), c)
                    =~= record_merge_spec(seq![b[0int]] + record_merge_spec(ta, tb), c));
                cons_subrange_tail(b[0int], record_merge_spec(ta, tb));
                assert(record_merge_spec(seq![b[0int]] + record_merge_spec(ta, tb), c)
                    =~= seq![c[0int]] + record_merge_spec(record_merge_spec(ta, tb), tc));
                assert(record_merge_spec(a, record_merge_spec(b, c))
                    =~= record_merge_spec(a, seq![c[0int]] + record_merge_spec(tb, tc)));
                cons_subrange_tail(c[0int], record_merge_spec(tb, tc));
                assert(record_merge_spec(a, seq![c[0int]] + record_merge_spec(tb, tc))
                    =~= seq![c[0int]] + record_merge_spec(ta, record_merge_spec(tb, tc)));
            }
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn field(k: &str, v: u64, p: MergePriority) -> ExecField {
        ExecField {
            key: k.as_bytes().to_vec(),
            value: v,
            priority: p,
        }
    }

    // r[verify nickel-record-merge.identity]
    #[test]
    fn test_merge_identity() {
        let a = vec![field("a", 1, MergePriority::Neutral)];
        let empty: Vec<ExecField> = vec![];
        assert_eq!(record_merge(&a, &empty), a);
    }

    // r[verify nickel-record-merge.key-coverage]
    #[test]
    fn test_merge_key_coverage() {
        let a = vec![
            field("a", 1, MergePriority::Neutral),
            field("c", 3, MergePriority::Neutral),
        ];
        let b = vec![field("b", 2, MergePriority::Neutral), field("c", 4, MergePriority::Top)];
        let r = record_merge(&a, &b);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].key, b"a");
        assert_eq!(r[1].key, b"b");
        assert_eq!(r[2].key, b"c");
    }

    // r[verify nickel-record-merge.disjoint-commutativity]
    #[test]
    fn test_disjoint_commutativity() {
        let a = vec![field("a", 1, MergePriority::Neutral)];
        let b = vec![field("b", 2, MergePriority::Top)];
        assert_eq!(record_merge(&a, &b), record_merge(&b, &a));
    }

    // r[verify nickel-record-merge.associativity]
    #[test]
    fn test_associativity_runtime_for_neutral_records() {
        let a = vec![
            field("a", 1, MergePriority::Neutral),
            field("c", 3, MergePriority::Neutral),
        ];
        let b = vec![
            field("b", 2, MergePriority::Neutral),
            field("c", 4, MergePriority::Neutral),
        ];
        let c = vec![
            field("c", 5, MergePriority::Neutral),
            field("d", 6, MergePriority::Neutral),
        ];
        assert_eq!(record_merge(&record_merge(&a, &b), &c), record_merge(&a, &record_merge(&b, &c)));
    }
}

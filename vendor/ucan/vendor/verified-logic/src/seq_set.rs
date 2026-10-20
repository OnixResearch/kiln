//! Verified set-theoretic operations over integer sequences.
//!
//! Treats `Seq<int>` as an unordered collection and provides membership,
//! difference, intersection, subset, and distinctness predicates with
//! algebraic proofs (partition, transitivity, distinctness preservation).

// Verus exec code uses i64→usize casts bounded by invariants (0 <= x < cap).
// if-not-else patterns are in Verus proof blocks where restructuring would break proofs.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::if_not_else)]

use vstd::prelude::*;

verus! {

// ============================================================
// is_member
// ============================================================

// r[depends seq-set.membership]
/// Spec: does `x` appear somewhere in `s`?
pub open spec fn is_member_spec(s: Seq<int>, x: int) -> bool {
    exists|i: int| 0 <= i < s.len() && s[i] == x
}

// r[impl seq-set.membership]
/// Linear scan for membership.
pub fn is_member(s: &[i64], x: i64) -> (result: bool)
    ensures result == is_member_spec(s@.map(|_i: int, v: i64| v as int), x as int),
{
    let ghost s_int = s@.map(|_i: int, v: i64| v as int);
    let mut i: usize = 0;
    while i < s.len()
        invariant
            0 <= i <= s@.len(),
            s_int =~= s@.map(|_i: int, v: i64| v as int),
            forall|j: int| 0 <= j < i as int ==> s_int[j] != x as int,
        decreases s.len() - i,
    {
        if s[i] == x {
            assert(s_int[i as int] == x as int);
            return true;
        }
        i += 1;
    }
    false
}

// r[verify seq-set.membership]
/// Empty sequence has no members.
proof fn empty_no_members(x: int)
    ensures !is_member_spec(Seq::<int>::empty(), x),
{}

// r[verify seq-set.membership]
/// A pushed element is a member.
proof fn push_is_member(s: Seq<int>, x: int)
    ensures is_member_spec(s.push(x), x),
{
    assert(s.push(x)[s.len() as int] == x);
}

// r[verify seq-set.membership]
/// Membership in subsequence implies membership in parent.
proof fn member_subrange(s: Seq<int>, x: int, lo: int, hi: int)
    requires 0 <= lo <= hi <= s.len(), is_member_spec(s.subrange(lo, hi), x)
    ensures is_member_spec(s, x)
{
    let sub = s.subrange(lo, hi);
    let wi = choose|i: int| 0 <= i < sub.len() && sub[i] == x;
    assert(s[lo + wi] == x);
}

// ============================================================
// set_difference — index-based recursion
// ============================================================

// r[depends seq-set.difference]
/// Spec: elements of `a[from..]` not in `b`, building left-to-right.
pub open spec fn set_difference_from_spec(
    a: Seq<int>, b: Seq<int>, from: int,
) -> Seq<int>
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from >= a.len() {
        seq![]
    } else if is_member_spec(b, a[from]) {
        set_difference_from_spec(a, b, from + 1)
    } else {
        seq![a[from]] + set_difference_from_spec(a, b, from + 1)
    }
}

// r[depends seq-set.difference]
/// Full set difference: difference starting from index 0.
pub open spec fn set_difference_spec(a: Seq<int>, b: Seq<int>) -> Seq<int> {
    set_difference_from_spec(a, b, 0)
}

/// Helper: unfolding set_difference_from at position `from`.
proof fn set_difference_from_unfold(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from < a.len()
    ensures
        set_difference_from_spec(a, b, from) ==
            if is_member_spec(b, a[from]) {
                set_difference_from_spec(a, b, from + 1)
            } else {
                seq![a[from]] + set_difference_from_spec(a, b, from + 1)
            },
{}

// r[impl seq-set.difference]
/// Returns elements of `a` not in `b`, preserving order from `a`.
/// Uses a boolean array indexed by value for O(n+m) membership.
/// Requires all values in range [0, cap).
pub fn set_difference(a: &[i64], b: &[i64], cap: usize) -> (result: Vec<i64>)
    requires
        cap > 0,
        forall|i: int| #![trigger a@[i]] 0 <= i < a@.len() ==> 0 <= a@[i] < cap as i64,
        forall|i: int| #![trigger b@[i]] 0 <= i < b@.len() ==> 0 <= b@[i] < cap as i64,
    ensures
        result@.map(|_i: int, v: i64| v as int)
            =~= set_difference_spec(
                a@.map(|_i: int, v: i64| v as int),
                b@.map(|_i: int, v: i64| v as int),
            ),
{
    let ghost a_int = a@.map(|_i: int, v: i64| v as int);
    let ghost b_int = b@.map(|_i: int, v: i64| v as int);

    // Build membership array for b
    let mut in_b: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < cap
        invariant
            0 <= k <= cap,
            in_b@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> in_b@[j] == false,
        decreases cap - k,
    {
        in_b.push(false);
        k += 1;
    }

    let mut j: usize = 0;
    while j < b.len()
        invariant
            0 <= j <= b@.len(),
            in_b@.len() == cap as int,
            b_int =~= b@.map(|_i: int, v: i64| v as int),
            forall|i: int| #![trigger b@[i]] 0 <= i < b@.len() ==> 0 <= b@[i] < cap as i64,
            forall|idx: int| 0 <= idx < cap as int ==> (
                in_b@[idx] == true ==> exists|bi: int| 0 <= bi < j as int && b_int[bi] == idx
            ),
            forall|bi: int| 0 <= bi < j as int ==> in_b@[b_int[bi]] == true,
        decreases b.len() - j,
    {
        in_b.set(b[j] as usize, true);
        assert(b_int[j as int] == b@[j as int] as int);
        j += 1;
    }

    // Filter a using membership array
    let mut out: Vec<i64> = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            in_b@.len() == cap as int,
            a_int =~= a@.map(|_i: int, v: i64| v as int),
            b_int =~= b@.map(|_i: int, v: i64| v as int),
            forall|idx: int| 0 <= idx < cap as int ==> (
                in_b@[idx] == true ==> exists|bi: int| 0 <= bi < b@.len() && b_int[bi] == idx
            ),
            forall|bi: int| 0 <= bi < b@.len() ==> in_b@[b_int[bi]] == true,
            forall|ai: int| #![trigger a@[ai]] 0 <= ai < a@.len() ==> 0 <= a@[ai] < cap as i64,
            // Key invariant: out so far + remaining == total
            out@.map(|_i: int, v: i64| v as int) + set_difference_from_spec(a_int, b_int, i as int)
                =~= set_difference_from_spec(a_int, b_int, 0),
        decreases a.len() - i,
    {
        if !in_b[a[i] as usize] {
            // a[i] not in b
            proof {
                assert forall|bi: int| 0 <= bi < b_int.len() implies b_int[bi] != a_int[i as int]
                by {
                    if b_int[bi] == a_int[i as int] {
                        assert(in_b@[a_int[i as int]] == true);
                    }
                }
                // unfold: diff_from(a, b, i) == seq![a[i]] + diff_from(a, b, i+1)
                // so out + seq![a[i]] + diff_from(a, b, i+1) =~= diff_from(a, b, 0)
                assert(
                    (out@.map(|_i: int, v: i64| v as int) + set_difference_from_spec(a_int, b_int, i as int))
                    =~= set_difference_from_spec(a_int, b_int, 0)
                );
            }
            out.push(a[i]);
        } else {
            // a[i] in b
            proof {
                assert(in_b@[a_int[i as int]] == true);
                let bi = choose|bi: int| 0 <= bi < b@.len() && b_int[bi] == a_int[i as int];
                assert(b_int[bi] == a_int[i as int]);
                assert(is_member_spec(b_int, a_int[i as int]));
            }
        }
        i += 1;
    }

    out
}

// r[verify seq-set.difference]
/// Every element in the difference is a member of the source.
proof fn difference_from_subset(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from
    ensures forall|x: int| is_member_spec(set_difference_from_spec(a, b, from), x)
        ==> #[trigger] is_member_spec(a, x),
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        difference_from_subset(a, b, from + 1);
        // If a[from] not in b, result = seq![a[from]] + rest
        // a[from] is in a (at index `from`), rest elements are in a by IH
        if !is_member_spec(b, a[from]) {
            let result = seq![a[from]] + set_difference_from_spec(a, b, from + 1);
            assert forall|x: int| is_member_spec(result, x)
                implies #[trigger] is_member_spec(a, x)
            by {
                let wi = choose|i: int| 0 <= i < result.len() && result[i] == x;
                if wi == 0 {
                    assert(a[from] == x);
                } else {
                    let rest = set_difference_from_spec(a, b, from + 1);
                    assert(rest[wi - 1] == x);
                    assert(is_member_spec(rest, x));
                }
            }
        }
    }
}

// r[verify seq-set.difference]
/// Every element in the difference is a member of the source (top-level).
proof fn difference_subset(a: Seq<int>, b: Seq<int>)
    ensures forall|x: int| is_member_spec(set_difference_spec(a, b), x)
        ==> #[trigger] is_member_spec(a, x),
{
    difference_from_subset(a, b, 0);
}

// r[verify seq-set.difference]
/// No element in the difference is a member of the filter.
proof fn difference_from_disjoint(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from
    ensures forall|x: int| is_member_spec(set_difference_from_spec(a, b, from), x)
        ==> !is_member_spec(b, x),
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        difference_from_disjoint(a, b, from + 1);
        if !is_member_spec(b, a[from]) {
            let result = seq![a[from]] + set_difference_from_spec(a, b, from + 1);
            assert forall|x: int| is_member_spec(result, x) implies !is_member_spec(b, x)
            by {
                let wi = choose|i: int| 0 <= i < result.len() && result[i] == x;
                if wi == 0 {
                    assert(x == a[from]);
                } else {
                    let rest = set_difference_from_spec(a, b, from + 1);
                    assert(is_member_spec(rest, x));
                }
            }
        }
    }
}

// r[verify seq-set.difference]
/// No element in the difference is a member of the filter (top-level).
proof fn difference_disjoint(a: Seq<int>, b: Seq<int>)
    ensures forall|x: int| is_member_spec(set_difference_spec(a, b), x)
        ==> !is_member_spec(b, x),
{
    difference_from_disjoint(a, b, 0);
}

// ============================================================
// set_intersection — index-based recursion
// ============================================================

// r[depends seq-set.intersection]
/// Spec: elements of `a[from..]` that are also in `b`.
pub open spec fn set_intersection_from_spec(
    a: Seq<int>, b: Seq<int>, from: int,
) -> Seq<int>
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from >= a.len() {
        seq![]
    } else if is_member_spec(b, a[from]) {
        seq![a[from]] + set_intersection_from_spec(a, b, from + 1)
    } else {
        set_intersection_from_spec(a, b, from + 1)
    }
}

// r[depends seq-set.intersection]
/// Full set intersection: intersection starting from index 0.
pub open spec fn set_intersection_spec(a: Seq<int>, b: Seq<int>) -> Seq<int> {
    set_intersection_from_spec(a, b, 0)
}

// r[impl seq-set.intersection]
/// Returns elements of `a` that are also in `b`, preserving order from `a`.
pub fn set_intersection(a: &[i64], b: &[i64], cap: usize) -> (result: Vec<i64>)
    requires
        cap > 0,
        forall|i: int| #![trigger a@[i]] 0 <= i < a@.len() ==> 0 <= a@[i] < cap as i64,
        forall|i: int| #![trigger b@[i]] 0 <= i < b@.len() ==> 0 <= b@[i] < cap as i64,
    ensures
        result@.map(|_i: int, v: i64| v as int)
            =~= set_intersection_spec(
                a@.map(|_i: int, v: i64| v as int),
                b@.map(|_i: int, v: i64| v as int),
            ),
{
    let ghost a_int = a@.map(|_i: int, v: i64| v as int);
    let ghost b_int = b@.map(|_i: int, v: i64| v as int);

    // Build membership array for b
    let mut in_b: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < cap
        invariant
            0 <= k <= cap,
            in_b@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> in_b@[j] == false,
        decreases cap - k,
    {
        in_b.push(false);
        k += 1;
    }

    let mut j: usize = 0;
    while j < b.len()
        invariant
            0 <= j <= b@.len(),
            in_b@.len() == cap as int,
            b_int =~= b@.map(|_i: int, v: i64| v as int),
            forall|i: int| #![trigger b@[i]] 0 <= i < b@.len() ==> 0 <= b@[i] < cap as i64,
            forall|idx: int| 0 <= idx < cap as int ==> (
                in_b@[idx] == true ==> exists|bi: int| 0 <= bi < j as int && b_int[bi] == idx
            ),
            forall|bi: int| 0 <= bi < j as int ==> in_b@[b_int[bi]] == true,
        decreases b.len() - j,
    {
        in_b.set(b[j] as usize, true);
        assert(b_int[j as int] == b@[j as int] as int);
        j += 1;
    }

    // Filter a keeping only elements in b
    let mut out: Vec<i64> = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            in_b@.len() == cap as int,
            a_int =~= a@.map(|_i: int, v: i64| v as int),
            b_int =~= b@.map(|_i: int, v: i64| v as int),
            forall|idx: int| 0 <= idx < cap as int ==> (
                in_b@[idx] == true ==> exists|bi: int| 0 <= bi < b@.len() && b_int[bi] == idx
            ),
            forall|bi: int| 0 <= bi < b@.len() ==> in_b@[b_int[bi]] == true,
            forall|ai: int| #![trigger a@[ai]] 0 <= ai < a@.len() ==> 0 <= a@[ai] < cap as i64,
            out@.map(|_i: int, v: i64| v as int) + set_intersection_from_spec(a_int, b_int, i as int)
                =~= set_intersection_from_spec(a_int, b_int, 0),
        decreases a.len() - i,
    {
        if in_b[a[i] as usize] {
            proof {
                assert(in_b@[a_int[i as int]] == true);
                let bi = choose|bi: int| 0 <= bi < b@.len() && b_int[bi] == a_int[i as int];
                assert(b_int[bi] == a_int[i as int]);
                assert(is_member_spec(b_int, a_int[i as int]));
            }
            out.push(a[i]);
        } else {
            proof {
                assert forall|bi: int| 0 <= bi < b_int.len() implies b_int[bi] != a_int[i as int]
                by {
                    if b_int[bi] == a_int[i as int] {
                        assert(in_b@[a_int[i as int]] == true);
                    }
                }
            }
        }
        i += 1;
    }

    out
}

// r[verify seq-set.intersection]
/// Every element in the intersection is a member of both sequences.
proof fn intersection_from_in_both(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from
    ensures forall|x: int| is_member_spec(set_intersection_from_spec(a, b, from), x)
        ==> #[trigger] is_member_spec(a, x) && is_member_spec(b, x),
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        intersection_from_in_both(a, b, from + 1);
        if is_member_spec(b, a[from]) {
            let result = seq![a[from]] + set_intersection_from_spec(a, b, from + 1);
            assert forall|x: int| is_member_spec(result, x)
                implies #[trigger] is_member_spec(a, x) && is_member_spec(b, x)
            by {
                let wi = choose|i: int| 0 <= i < result.len() && result[i] == x;
                if wi == 0 {
                    assert(a[from] == x);
                    assert(is_member_spec(b, x));
                } else {
                    let rest = set_intersection_from_spec(a, b, from + 1);
                    assert(rest[wi - 1] == x);
                    assert(is_member_spec(rest, x));
                }
            }
        }
    }
}

// r[verify seq-set.intersection]
/// Top-level: intersection elements are in both.
proof fn intersection_in_both(a: Seq<int>, b: Seq<int>)
    ensures forall|x: int| is_member_spec(set_intersection_spec(a, b), x)
        ==> #[trigger] is_member_spec(a, x) && is_member_spec(b, x),
{
    intersection_from_in_both(a, b, 0);
}

// ============================================================
// is_subset
// ============================================================

// r[depends seq-set.subset]
/// Spec: every element of `a` is a member of `b`.
pub open spec fn is_subset_spec(a: Seq<int>, b: Seq<int>) -> bool {
    forall|x: int| is_member_spec(a, x) ==> is_member_spec(b, x)
}

// r[impl seq-set.subset]
/// Returns true if every element of `a` appears in `b`.
pub fn is_subset(a: &[i64], b: &[i64], cap: usize) -> (result: bool)
    requires
        cap > 0,
        forall|i: int| #![trigger a@[i]] 0 <= i < a@.len() ==> 0 <= a@[i] < cap as i64,
        forall|i: int| #![trigger b@[i]] 0 <= i < b@.len() ==> 0 <= b@[i] < cap as i64,
    ensures
        result == is_subset_spec(
            a@.map(|_i: int, v: i64| v as int),
            b@.map(|_i: int, v: i64| v as int),
        ),
{
    let ghost a_int = a@.map(|_i: int, v: i64| v as int);
    let ghost b_int = b@.map(|_i: int, v: i64| v as int);

    // Build membership array for b
    let mut in_b: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < cap
        invariant
            0 <= k <= cap,
            in_b@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> in_b@[j] == false,
        decreases cap - k,
    {
        in_b.push(false);
        k += 1;
    }

    let mut j: usize = 0;
    while j < b.len()
        invariant
            0 <= j <= b@.len(),
            in_b@.len() == cap as int,
            b_int =~= b@.map(|_i: int, v: i64| v as int),
            forall|i: int| #![trigger b@[i]] 0 <= i < b@.len() ==> 0 <= b@[i] < cap as i64,
            forall|idx: int| 0 <= idx < cap as int ==> (
                in_b@[idx] == true ==> exists|bi: int| 0 <= bi < j as int && b_int[bi] == idx
            ),
            forall|bi: int| 0 <= bi < j as int ==> in_b@[b_int[bi]] == true,
        decreases b.len() - j,
    {
        in_b.set(b[j] as usize, true);
        assert(b_int[j as int] == b@[j as int] as int);
        j += 1;
    }

    // Check every element of a is in b
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            in_b@.len() == cap as int,
            a_int =~= a@.map(|_i: int, v: i64| v as int),
            b_int =~= b@.map(|_i: int, v: i64| v as int),
            forall|idx: int| 0 <= idx < cap as int ==> (
                in_b@[idx] == true ==> exists|bi: int| 0 <= bi < b@.len() && b_int[bi] == idx
            ),
            forall|bi: int| 0 <= bi < b@.len() ==> in_b@[b_int[bi]] == true,
            forall|ai: int| #![trigger a@[ai]] 0 <= ai < a@.len() ==> 0 <= a@[ai] < cap as i64,
            forall|ai: int| 0 <= ai < i as int ==> is_member_spec(b_int, a_int[ai]),
        decreases a.len() - i,
    {
        if !in_b[a[i] as usize] {
            proof {
                assert(a_int[i as int] == a@[i as int] as int);
                assert(is_member_spec(a_int, a_int[i as int]));
                assert forall|bi: int| 0 <= bi < b_int.len() implies b_int[bi] != a_int[i as int]
                by {
                    if b_int[bi] == a_int[i as int] {
                        assert(in_b@[a_int[i as int]] == true);
                    }
                }
            }
            return false;
        }
        proof {
            assert(in_b@[a_int[i as int]] == true);
            let bi = choose|bi: int| 0 <= bi < b@.len() && b_int[bi] == a_int[i as int];
            assert(is_member_spec(b_int, a_int[i as int]));
        }
        i += 1;
    }

    proof {
        assert forall|x: int| is_member_spec(a_int, x) implies is_member_spec(b_int, x)
        by {
            let ai = choose|ai: int| 0 <= ai < a_int.len() && a_int[ai] == x;
            assert(is_member_spec(b_int, a_int[ai]));
        }
    }
    true
}

// r[verify seq-set.subset]
/// Reflexive: every sequence is a subset of itself.
proof fn subset_reflexive(a: Seq<int>)
    ensures is_subset_spec(a, a),
{}

// r[verify seq-set.subset]
/// Transitive: a ⊆ b ⊆ c implies a ⊆ c.
proof fn subset_transitive(a: Seq<int>, b: Seq<int>, c: Seq<int>)
    requires is_subset_spec(a, b), is_subset_spec(b, c)
    ensures is_subset_spec(a, c)
{}

// r[verify seq-set.subset]
/// Empty is subset of anything.
proof fn empty_subset(b: Seq<int>)
    ensures is_subset_spec(Seq::<int>::empty(), b),
{}

// ============================================================
// all_distinct
// ============================================================

// r[depends seq-set.distinct]
/// Spec: no element appears twice.
pub open spec fn all_distinct_spec(s: Seq<int>) -> bool {
    forall|i: int, j: int| 0 <= i < j < s.len() ==> s[i] != s[j]
}

// r[impl seq-set.distinct]
/// Returns true if all elements are distinct.
pub fn all_distinct(s: &[i64], cap: usize) -> (result: bool)
    requires
        cap > 0,
        forall|i: int| #![trigger s@[i]] 0 <= i < s@.len() ==> 0 <= s@[i] < cap as i64,
    ensures
        result == all_distinct_spec(s@.map(|_i: int, v: i64| v as int)),
{
    let ghost s_int = s@.map(|_i: int, v: i64| v as int);

    let mut seen: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < cap
        invariant
            0 <= k <= cap,
            seen@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> seen@[j] == false,
        decreases cap - k,
    {
        seen.push(false);
        k += 1;
    }

    let mut i: usize = 0;
    while i < s.len()
        invariant
            0 <= i <= s@.len(),
            seen@.len() == cap as int,
            s_int =~= s@.map(|_i: int, v: i64| v as int),
            forall|si: int| #![trigger s@[si]] 0 <= si < s@.len() ==> 0 <= s@[si] < cap as i64,
            // seen[v] == true iff v appeared in s[0..i]
            forall|idx: int| 0 <= idx < cap as int ==> (
                seen@[idx] == true <==> exists|si: int| 0 <= si < i as int && s_int[si] == idx
            ),
            // No duplicates in s[0..i]
            all_distinct_spec(s_int.subrange(0, i as int)),
        decreases s.len() - i,
    {
        let v = s[i] as usize;
        if seen[v] {
            proof {
                assert(seen@[s_int[i as int]] == true);
                let prev = choose|si: int| 0 <= si < i as int && s_int[si] == s_int[i as int];
                assert(s_int[prev] == s_int[i as int]);
                assert(!all_distinct_spec(s_int));
            }
            return false;
        }
        proof {
            assert(seen@[s_int[i as int]] == false);
            assert(forall|si: int| 0 <= si < i as int ==> s_int[si] != s_int[i as int]);
            let sub_new = s_int.subrange(0, i as int + 1);
            let sub_old = s_int.subrange(0, i as int);
            assert forall|a: int, b: int| 0 <= a < b < sub_new.len() implies sub_new[a] != sub_new[b]
            by {
                if b == i as int {
                    assert(sub_new[b] == s_int[i as int]);
                    assert(sub_new[a] == sub_old[a]);
                    assert(sub_old[a] == s_int[a]);
                    assert(s_int[a] != s_int[i as int]);
                } else {
                    assert(sub_new[a] == sub_old[a]);
                    assert(sub_new[b] == sub_old[b]);
                }
            }
        }
        seen.set(v, true);
        i += 1;
    }

    proof {
        assert(s_int.subrange(0, s_int.len() as int) =~= s_int);
    }
    true
}

// r[verify seq-set.distinct]
/// Empty sequence is trivially distinct.
proof fn empty_is_distinct()
    ensures all_distinct_spec(Seq::<int>::empty()),
{}

// r[verify seq-set.distinct]
/// Singleton is trivially distinct.
proof fn singleton_is_distinct(x: int)
    ensures all_distinct_spec(seq![x]),
{}

// ============================================================
// Partition theorem: diff.len() + inter.len() == a.len()
// ============================================================

/// Helper: partition holds from any starting index.
proof fn partition_from(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from
    ensures
        (set_difference_from_spec(a, b, from).len()
            + set_intersection_from_spec(a, b, from).len()) as int
            == if from >= a.len() { 0 } else { (a.len() - from) as int },
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        partition_from(a, b, from + 1);
    }
}

// r[verify seq-set.difference]
// r[verify seq-set.intersection]
/// For any a and b, |difference(a,b)| + |intersection(a,b)| == |a|.
proof fn difference_intersection_partition(a: Seq<int>, b: Seq<int>)
    ensures
        (set_difference_spec(a, b).len() + set_intersection_spec(a, b).len()) as int
            == a.len(),
{
    partition_from(a, b, 0);
}

// ============================================================
// Distinctness preservation
// ============================================================

// r[verify seq-set.difference]
/// Difference preserves distinctness.
proof fn difference_from_preserves_distinct(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from, all_distinct_spec(a)
    ensures all_distinct_spec(set_difference_from_spec(a, b, from))
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        difference_from_preserves_distinct(a, b, from + 1);
        if !is_member_spec(b, a[from]) {
            let rest = set_difference_from_spec(a, b, from + 1);
            let result = seq![a[from]] + rest;
            // a[from] can't appear in rest because rest only contains elements from a[from+1..],
            // and all_distinct means a[from] != a[j] for j > from
            difference_from_subset(a, b, from + 1);
            assert forall|i: int, j: int| 0 <= i < j < result.len()
                implies result[i] != result[j]
            by {
                if i == 0 {
                    // result[0] = a[from], result[j] = rest[j-1]
                    assert(result[j] == rest[j - 1]);
                    assert(is_member_spec(rest, rest[j - 1]));
                    // rest[j-1] is in a, at some index > from
                    assert(is_member_spec(a, rest[j - 1]));
                    let ak = choose|ak: int| 0 <= ak < a.len() && a[ak] == rest[j - 1];
                    // Must show ak > from: rest only contains a[from+1..] elements
                    // a[from] != a[ak] because all_distinct and either ak != from (trivial)
                    // or ak == from which would mean rest[j-1] == a[from] == result[0]
                    // But we need ak != from to conclude a[from] != a[ak]
                    // From difference_from_subset on from+1, rest elements come from a
                    // We need: rest[j-1] != a[from]
                    // Proof: rest is subset of set_difference_from(a, b, from+1)
                    // which only processes a[from+1..], so rest elements = a[k] for k > from
                    difference_from_indices_above(a, b, from + 1, rest[j - 1]);
                } else {
                    assert(result[i] == rest[i - 1]);
                    assert(result[j] == rest[j - 1]);
                }
            }
        }
    }
}

/// Helper: elements of set_difference_from(a, b, from) come from indices >= from.
proof fn difference_from_indices_above(a: Seq<int>, b: Seq<int>, from: int, x: int)
    requires
        0 <= from,
        all_distinct_spec(a),
        is_member_spec(set_difference_from_spec(a, b, from), x),
    ensures exists|k: int| from <= k < a.len() && a[k] == x
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        if !is_member_spec(b, a[from]) {
            let result = seq![a[from]] + set_difference_from_spec(a, b, from + 1);
            let wi = choose|i: int| 0 <= i < result.len() && result[i] == x;
            if wi == 0 {
                assert(a[from] == x);
            } else {
                let rest = set_difference_from_spec(a, b, from + 1);
                assert(is_member_spec(rest, x));
                difference_from_indices_above(a, b, from + 1, x);
            }
        } else {
            difference_from_indices_above(a, b, from + 1, x);
        }
    }
}

// r[verify seq-set.difference]
/// Top-level: difference preserves distinctness.
proof fn difference_preserves_distinct(a: Seq<int>, b: Seq<int>)
    requires all_distinct_spec(a)
    ensures all_distinct_spec(set_difference_spec(a, b))
{
    difference_from_preserves_distinct(a, b, 0);
}

// r[verify seq-set.intersection]
/// Intersection preserves distinctness.
proof fn intersection_from_preserves_distinct(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from, all_distinct_spec(a)
    ensures all_distinct_spec(set_intersection_from_spec(a, b, from))
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        intersection_from_preserves_distinct(a, b, from + 1);
        if is_member_spec(b, a[from]) {
            let rest = set_intersection_from_spec(a, b, from + 1);
            let result = seq![a[from]] + rest;
            intersection_from_in_both(a, b, from + 1);
            assert forall|i: int, j: int| 0 <= i < j < result.len()
                implies result[i] != result[j]
            by {
                if i == 0 {
                    assert(result[j] == rest[j - 1]);
                    assert(is_member_spec(rest, rest[j - 1]));
                    assert(is_member_spec(a, rest[j - 1]));
                    intersection_from_indices_above(a, b, from + 1, rest[j - 1]);
                } else {
                    assert(result[i] == rest[i - 1]);
                    assert(result[j] == rest[j - 1]);
                }
            }
        }
    }
}

/// Helper: elements of set_intersection_from(a, b, from) come from indices >= from.
proof fn intersection_from_indices_above(a: Seq<int>, b: Seq<int>, from: int, x: int)
    requires
        0 <= from,
        all_distinct_spec(a),
        is_member_spec(set_intersection_from_spec(a, b, from), x),
    ensures exists|k: int| from <= k < a.len() && a[k] == x
    decreases if from < a.len() as int { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int {
        if is_member_spec(b, a[from]) {
            let result = seq![a[from]] + set_intersection_from_spec(a, b, from + 1);
            let wi = choose|i: int| 0 <= i < result.len() && result[i] == x;
            if wi == 0 {
                assert(a[from] == x);
            } else {
                let rest = set_intersection_from_spec(a, b, from + 1);
                assert(is_member_spec(rest, x));
                intersection_from_indices_above(a, b, from + 1, x);
            }
        } else {
            intersection_from_indices_above(a, b, from + 1, x);
        }
    }
}

// r[verify seq-set.intersection]
/// Top-level: intersection preserves distinctness.
proof fn intersection_preserves_distinct(a: Seq<int>, b: Seq<int>)
    requires all_distinct_spec(a)
    ensures all_distinct_spec(set_intersection_spec(a, b))
{
    intersection_from_preserves_distinct(a, b, 0);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify seq-set.membership]
    #[test]
    fn test_is_member() {
        assert!(is_member(&[3, 1, 4], 1));
        assert!(is_member(&[3, 1, 4], 3));
        assert!(is_member(&[3, 1, 4], 4));
        assert!(!is_member(&[3, 1, 4], 5));
        assert!(!is_member(&[3, 1, 4], 0));
        assert!(!is_member(&[], 1));
    }

    // r[verify seq-set.difference]
    #[test]
    fn test_set_difference() {
        let r = set_difference(&[1, 2, 3], &[2, 4], 10);
        assert_eq!(r, vec![1, 3]);

        let r = set_difference(&[1, 2, 3], &[4, 5], 10);
        assert_eq!(r, vec![1, 2, 3]);

        let r = set_difference(&[1, 2], &[1, 2, 3], 10);
        assert_eq!(r, Vec::<i64>::new());

        let r = set_difference(&[1, 2, 3], &[], 10);
        assert_eq!(r, vec![1, 2, 3]);
    }

    // r[verify seq-set.intersection]
    #[test]
    fn test_set_intersection() {
        let r = set_intersection(&[1, 2, 3], &[2, 3, 4], 10);
        assert_eq!(r, vec![2, 3]);

        let r = set_intersection(&[1, 2], &[3, 4], 10);
        assert_eq!(r, Vec::<i64>::new());

        let r = set_intersection(&[1, 2, 3], &[1, 2, 3], 10);
        assert_eq!(r, vec![1, 2, 3]);
    }

    // r[verify seq-set.subset]
    #[test]
    fn test_is_subset() {
        assert!(is_subset(&[1, 2], &[1, 2, 3], 10));
        assert!(!is_subset(&[1, 4], &[1, 2, 3], 10));
        assert!(is_subset(&[], &[1, 2], 10));
        assert!(is_subset(&[1], &[1], 10));
    }

    // r[verify seq-set.distinct]
    #[test]
    fn test_all_distinct() {
        assert!(all_distinct(&[3, 1, 4, 5], 10));
        assert!(!all_distinct(&[3, 1, 4, 1], 10));
        assert!(all_distinct(&[], 10));
        assert!(all_distinct(&[7], 10));
    }

    fn small_seq() -> impl Strategy<Value = Vec<i64>> {
        proptest::collection::vec(0i64..20, 0..10)
    }

    fn distinct_seq() -> impl Strategy<Value = Vec<i64>> {
        (0usize..=10).prop_flat_map(|len| {
            proptest::collection::vec(0i64..20, len..=len).prop_filter("distinct", |v| {
                let mut s = v.clone();
                s.sort_unstable();
                s.dedup();
                s.len() == v.len()
            })
        })
    }

    proptest! {
        // r[verify seq-set.membership]
        #[test]
        fn prop_member_reflexive(s in small_seq()) {
            for &x in &s {
                prop_assert!(is_member(&s, x));
            }
        }

        // r[verify seq-set.difference]
        #[test]
        fn prop_difference_subset(a in small_seq(), b in small_seq()) {
            let diff = set_difference(&a, &b, 30);
            for &x in &diff {
                prop_assert!(is_member(&a, x), "diff element {} not in a", x);
                prop_assert!(!is_member(&b, x), "diff element {} in b", x);
            }
        }

        // r[verify seq-set.intersection]
        #[test]
        fn prop_intersection_in_both(a in small_seq(), b in small_seq()) {
            let inter = set_intersection(&a, &b, 30);
            for &x in &inter {
                prop_assert!(is_member(&a, x), "inter element {} not in a", x);
                prop_assert!(is_member(&b, x), "inter element {} not in b", x);
            }
        }

        // r[verify seq-set.difference]
        // r[verify seq-set.intersection]
        #[test]
        fn prop_partition(a in small_seq(), b in small_seq()) {
            let diff = set_difference(&a, &b, 30);
            let inter = set_intersection(&a, &b, 30);
            prop_assert_eq!(diff.len() + inter.len(), a.len());
        }

        // r[verify seq-set.subset]
        #[test]
        fn prop_subset_reflexive(a in small_seq()) {
            prop_assert!(is_subset(&a, &a, 30));
        }

        // r[verify seq-set.distinct]
        #[test]
        fn prop_difference_preserves_distinct(a in distinct_seq(), b in small_seq()) {
            let diff = set_difference(&a, &b, 30);
            prop_assert!(all_distinct(&diff, 30));
        }

        // r[verify seq-set.distinct]
        #[test]
        fn prop_intersection_preserves_distinct(a in distinct_seq(), b in small_seq()) {
            let inter = set_intersection(&a, &b, 30);
            prop_assert!(all_distinct(&inter, 30));
        }
    }
}

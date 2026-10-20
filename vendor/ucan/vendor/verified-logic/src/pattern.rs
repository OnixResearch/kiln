//! Verified pattern matching for capability authorization.
//!
//! Wildcard and set-membership patterns over integer item IDs.
//! A pattern is a sequence of allowed values, or a wildcard that
//! matches everything. Pattern containment is the subset relation.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// ============================================================
// Wildcard sentinel
// ============================================================

/// Sentinel value representing "match everything."
/// A pattern whose only element is WILDCARD matches any input.
/// Using -1 as the sentinel keeps the valid item space at non-negative integers.
pub open spec fn WILDCARD() -> int { -1 }

// r[depends pattern.wildcard-matches-all]
/// Spec: is this a wildcard pattern?
pub open spec fn is_wildcard_spec(pattern: Seq<int>) -> bool {
    pattern.len() == 1 && pattern[0] == WILDCARD()
}

// r[impl pattern.wildcard-matches-all]
/// Returns true if the pattern is the wildcard singleton.
pub fn is_wildcard(pattern: &[i64]) -> (result: bool)
    ensures result == is_wildcard_spec(pattern@.map(|_i: int, v: i64| v as int)),
{
    pattern.len() == 1 && pattern[0] == -1
}

// ============================================================
// match_pattern: does a value match a pattern?
// ============================================================

/// Spec: does the sequence contain the given value?
pub open spec fn seq_contains_int(s: Seq<int>, v: int) -> bool {
    exists|i: int| 0 <= i < s.len() && s[i] == v
}

// r[depends pattern.wildcard-matches-all]
/// Spec: does the value match the pattern?
/// Wildcard matches everything. Otherwise, the value must be a member.
pub open spec fn match_pattern_spec(pattern: Seq<int>, v: int) -> bool {
    is_wildcard_spec(pattern) || seq_contains_int(pattern, v)
}

// r[impl pattern.wildcard-matches-all]
/// Returns true if value matches the pattern (wildcard or membership).
pub fn match_pattern(pattern: &[i64], v: i64) -> (result: bool)
    ensures result == match_pattern_spec(
        pattern@.map(|_i: int, x: i64| x as int),
        v as int,
    ),
{
    if pattern.len() == 1 && pattern[0] == -1 {
        return true;
    }
    let ghost pat = pattern@.map(|_i: int, x: i64| x as int);
    let mut i: usize = 0;
    while i < pattern.len()
        invariant
            i <= pattern.len(),
            pat =~= pattern@.map(|_i: int, x: i64| x as int),
            !is_wildcard_spec(pat),
            forall|j: int| #![trigger pat[j]]
                0 <= j < i as int ==> pat[j] != v as int,
        decreases pattern.len() - i,
    {
        if pattern[i] == v {
            assert(pat[i as int] == v as int);
            return true;
        }
        i = i + 1;
    }
    false
}

// r[verify pattern.wildcard-matches-all]
/// Wildcard matches any value.
proof fn wildcard_matches_all(v: int)
    ensures match_pattern_spec(seq![WILDCARD()], v)
{}

// r[impl pattern.exact-match-reflexive]
// r[verify pattern.exact-match-reflexive]
/// A singleton pattern matches its own element.
proof fn exact_match_reflexive(x: int)
    requires x != WILDCARD()
    ensures match_pattern_spec(seq![x], x)
{
    assert(seq![x][0] == x);
}

// r[impl pattern.non-member-rejected]
// r[verify pattern.non-member-rejected]
/// A value not in the pattern (and pattern is not wildcard) is rejected.
proof fn non_member_rejected(pattern: Seq<int>, v: int)
    requires
        !is_wildcard_spec(pattern),
        !seq_contains_int(pattern, v),
    ensures !match_pattern_spec(pattern, v)
{}

// ============================================================
// pattern_contains: subset relation
// ============================================================

// r[depends pattern.containment-subset]
/// Spec: does pattern p1 contain pattern p2?
/// Wildcard contains everything. For non-wildcard patterns,
/// every item in p2 must also appear in p1.
pub open spec fn pattern_contains_spec(p1: Seq<int>, p2: Seq<int>) -> bool {
    is_wildcard_spec(p1)
    || forall|i: int| 0 <= i < p2.len() ==> seq_contains_int(p1, p2[i])
}

// r[impl pattern.containment-subset]
/// Returns true if p1 contains p2 (every item in p2 is in p1, or p1 is wildcard).
pub fn pattern_contains(p1: &[i64], p2: &[i64]) -> (result: bool)
    ensures result == pattern_contains_spec(
        p1@.map(|_i: int, x: i64| x as int),
        p2@.map(|_i: int, x: i64| x as int),
    ),
{
    if p1.len() == 1 && p1[0] == -1 {
        return true;
    }
    let ghost p1m = p1@.map(|_i: int, x: i64| x as int);
    let ghost p2m = p2@.map(|_i: int, x: i64| x as int);
    let mut k: usize = 0;
    while k < p2.len()
        invariant
            k <= p2@.len(),
            p1m =~= p1@.map(|_i: int, x: i64| x as int),
            p2m =~= p2@.map(|_i: int, x: i64| x as int),
            !is_wildcard_spec(p1m),
            forall|i: int| #![trigger p2m[i]]
                0 <= i < k as int ==> seq_contains_int(p1m, p2m[i]),
        decreases p2@.len() - k,
    {
        let target = p2[k];
        let mut found = false;
        let mut j: usize = 0;
        while j < p1.len()
            invariant
                j <= p1@.len(),
                p1m =~= p1@.map(|_i: int, x: i64| x as int),
                found ==> seq_contains_int(p1m, target as int),
                !found ==> forall|idx: int| #![trigger p1m[idx]]
                    0 <= idx < j as int ==> p1m[idx] != target as int,
            decreases p1@.len() - j,
        {
            if !found && p1[j] == target {
                assert(p1m[j as int] == target as int);
                found = true;
            }
            j += 1;
        }
        if !found {
            assert(p2m[k as int] == target as int);
            // j == p1@.len(), so the invariant covers all indices
            assert forall|idx: int| #![trigger p1m[idx]]
                !(0 <= idx < p1m.len() && p1m[idx] == target as int) by {
            };
            return false;
        }
        assert(p2m[k as int] == target as int);
        assert(seq_contains_int(p1m, p2m[k as int]));
        k += 1;
    }
    true
}

// r[impl pattern.wildcard-contains-any]
// r[verify pattern.wildcard-contains-any]
/// Wildcard contains any pattern.
proof fn wildcard_contains_any(p2: Seq<int>)
    ensures pattern_contains_spec(seq![WILDCARD()], p2)
{}

// r[impl pattern.non-wildcard-cannot-contain-wildcard]
// r[verify pattern.non-wildcard-cannot-contain-wildcard]
/// A non-wildcard pattern that doesn't contain the sentinel value
/// cannot contain the wildcard pattern.
proof fn non_wildcard_no_sentinel_cannot_contain_wildcard(p1: Seq<int>)
    requires
        !is_wildcard_spec(p1),
        !seq_contains_int(p1, WILDCARD()),
    ensures !pattern_contains_spec(p1, seq![WILDCARD()])
{
    // pattern_contains(p1, [-1]) = is_wildcard(p1) || forall i in 0..1: contains(p1, [-1][i])
    // is_wildcard(p1) is false. At i=0, [-1][0] = -1, need seq_contains_int(p1, -1). But that's false.
    assert(seq![WILDCARD()][0] == WILDCARD());
    assert(!seq_contains_int(p1, seq![WILDCARD()][0]));
}

// r[impl pattern.containment-reflexive]
// r[verify pattern.containment-reflexive]
/// Every pattern contains itself.
proof fn pattern_contains_reflexive(p: Seq<int>)
    ensures pattern_contains_spec(p, p)
{
    if !is_wildcard_spec(p) {
        assert forall|i: int| 0 <= i < p.len()
            implies seq_contains_int(p, p[i])
        by {
            assert(p[i] == p[i]);
        }
    }
}

// r[impl pattern.containment-transitive]
// r[verify pattern.containment-transitive]
/// Transitivity for well-formed patterns (no sentinel in non-wildcard patterns).
proof fn pattern_contains_transitive_wellformed(a: Seq<int>, b: Seq<int>, c: Seq<int>)
    requires
        pattern_contains_spec(a, b),
        pattern_contains_spec(b, c),
        // Well-formedness: non-wildcard patterns don't contain the sentinel
        !is_wildcard_spec(a) ==> !seq_contains_int(a, WILDCARD()),
        !is_wildcard_spec(b) ==> !seq_contains_int(b, WILDCARD()),
    ensures pattern_contains_spec(a, c)
{
    if is_wildcard_spec(a) {
        // Done.
    } else if is_wildcard_spec(b) {
        // a is not wildcard but contains b = [-1].
        // That means seq_contains_int(a, -1), but we required !seq_contains_int(a, -1).
        // Contradiction — this case is vacuously true.
        assert(seq_contains_int(a, WILDCARD()));
        assert(false);
    } else {
        assert forall|i: int| 0 <= i < c.len()
            implies seq_contains_int(a, c[i])
        by {
            let v = c[i];
            assert(seq_contains_int(b, v));
            let j = choose|j: int| 0 <= j < b.len() && b[j] == v;
            assert(seq_contains_int(a, b[j]));
        }
    }
}

// r[verify pattern.containment-subset]
/// Containment implies match, for well-formed patterns.
proof fn containment_implies_match_wellformed(p1: Seq<int>, p2: Seq<int>, v: int)
    requires
        pattern_contains_spec(p1, p2),
        match_pattern_spec(p2, v),
        !is_wildcard_spec(p1) ==> !seq_contains_int(p1, WILDCARD()),
        !is_wildcard_spec(p2) ==> !seq_contains_int(p2, WILDCARD()),
    ensures match_pattern_spec(p1, v)
{
    if is_wildcard_spec(p1) {
        // Done.
    } else if is_wildcard_spec(p2) {
        // p1 not wildcard, so !seq_contains_int(p1, -1).
        // pattern_contains(p1, [-1]) requires seq_contains_int(p1, -1). Contradiction.
        assert(seq_contains_int(p1, WILDCARD()));
        assert(false);
    } else {
        if seq_contains_int(p2, v) {
            let j = choose|j: int| 0 <= j < p2.len() && p2[j] == v;
            assert(seq_contains_int(p1, p2[j]));
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify pattern.wildcard-matches-all]
    #[test]
    fn test_wildcard_matches_all() {
        let wc: &[i64] = &[-1];
        assert!(match_pattern(wc, 0));
        assert!(match_pattern(wc, 42));
        assert!(match_pattern(wc, -1));
        assert!(match_pattern(wc, i64::MAX));
        assert!(match_pattern(wc, i64::MIN));
    }

    // r[verify pattern.exact-match-reflexive]
    #[test]
    fn test_exact_match() {
        assert!(match_pattern(&[5], 5));
        assert!(match_pattern(&[0], 0));
        assert!(!match_pattern(&[5], 6));
    }

    // r[verify pattern.non-member-rejected]
    #[test]
    fn test_non_member_rejected() {
        assert!(!match_pattern(&[1, 2, 3], 4));
        assert!(!match_pattern(&[10, 20], 30));
        assert!(!match_pattern(&[], 0));
    }

    // r[verify pattern.wildcard-matches-all]
    #[test]
    fn test_set_membership() {
        assert!(match_pattern(&[1, 2, 3], 1));
        assert!(match_pattern(&[1, 2, 3], 2));
        assert!(match_pattern(&[1, 2, 3], 3));
        assert!(!match_pattern(&[1, 2, 3], 4));
    }

    // r[verify pattern.wildcard-contains-any]
    #[test]
    fn test_wildcard_contains_any() {
        let wc: &[i64] = &[-1];
        assert!(pattern_contains(wc, &[]));
        assert!(pattern_contains(wc, &[1, 2, 3]));
        assert!(pattern_contains(wc, wc));
    }

    // r[verify pattern.containment-reflexive]
    #[test]
    fn test_containment_reflexive() {
        assert!(pattern_contains(&[1, 2, 3], &[1, 2, 3]));
        assert!(pattern_contains(&[5], &[5]));
        assert!(pattern_contains(&[], &[]));
    }

    // r[verify pattern.containment-subset]
    #[test]
    fn test_containment_subset() {
        assert!(pattern_contains(&[1, 2, 3], &[1, 2]));
        assert!(pattern_contains(&[1, 2, 3], &[3]));
        assert!(!pattern_contains(&[1, 2], &[1, 2, 3]));
        assert!(!pattern_contains(&[1], &[2]));
    }

    #[test]
    fn test_is_wildcard() {
        assert!(is_wildcard(&[-1]));
        assert!(!is_wildcard(&[0]));
        assert!(!is_wildcard(&[-1, 0]));
        assert!(!is_wildcard(&[]));
    }

    proptest! {
        // r[verify pattern.wildcard-matches-all]
        #[test]
        fn prop_wildcard_matches_all(v: i64) {
            prop_assert!(match_pattern(&[-1], v));
        }

        // r[verify pattern.exact-match-reflexive]
        #[test]
        fn prop_exact_match_reflexive(x in 0i64..=1000) {
            prop_assert!(match_pattern(&[x], x));
        }

        // r[verify pattern.containment-reflexive]
        #[test]
        fn prop_containment_reflexive(
            items in proptest::collection::vec(0i64..=100, 0..=20),
        ) {
            prop_assert!(pattern_contains(&items, &items));
        }

        // r[verify pattern.wildcard-contains-any]
        #[test]
        fn prop_wildcard_contains_any(
            items in proptest::collection::vec(0i64..=100, 0..=20),
        ) {
            prop_assert!(pattern_contains(&[-1], &items));
        }

        // r[verify pattern.containment-subset]
        #[test]
        fn prop_containment_subset_sound(
            superset in proptest::collection::vec(0i64..=10, 0..=15),
            subset_indices in proptest::collection::vec(any::<proptest::sample::Index>(), 0..=10),
        ) {
            if superset.is_empty() {
                prop_assert!(pattern_contains(&superset, &[]));
            } else {
                let subset: Vec<i64> = subset_indices.iter()
                    .map(|idx| superset[idx.index(superset.len())])
                    .collect();
                prop_assert!(pattern_contains(&superset, &subset));
            }
        }

        // r[verify pattern.containment-subset]
        #[test]
        fn prop_containment_implies_match(
            p1 in proptest::collection::vec(0i64..=20, 1..=10),
            p2_indices in proptest::collection::vec(any::<proptest::sample::Index>(), 1..=5),
        ) {
            // Build p2 as a subset of p1
            let p2: Vec<i64> = p2_indices.iter()
                .map(|idx| p1[idx.index(p1.len())])
                .collect();
            // Pick a value from p2
            let v = p2[0];
            // v matches p2 (it's a member)
            prop_assert!(match_pattern(&p2, v));
            // p1 contains p2
            prop_assert!(pattern_contains(&p1, &p2));
            // So v should match p1 too
            prop_assert!(match_pattern(&p1, v));
        }

        // r[verify pattern.non-member-rejected]
        #[test]
        fn prop_non_member_rejected(
            items in proptest::collection::vec(0i64..=50, 0..=20),
            v in 51i64..=100,
        ) {
            // v is in range 51..=100, items are in 0..=50, so v not in items
            prop_assert!(!match_pattern(&items, v));
        }
    }
}

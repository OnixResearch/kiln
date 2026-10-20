//! Verified string-based ability hierarchy for UCAN capabilities.
//!
//! Models abilities as slash-separated segment paths (e.g., "crud/read",
//! "msg/send"). Supports glob matching: `"*"` as the sole segment is the
//! top ability that subsumes all abilities. Single-segment `"*"` in a
//! multi-segment pattern matches one level.
//!
//! Proves reflexivity, transitivity, top-subsumes-all, namespace isolation,
//! and containment monotonicity.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// An ability is a sequence of path segments.
/// E.g., `crud/read` is `[['c','r','u','d'], ['r','e','a','d']]`.
/// The top ability is `[['*']]`.
pub struct Ability {
    pub segments: Vec<Vec<u8>>,
}

/// The top ability pattern: a single segment "*".
// r[impl ucan-ability.top-subsumes-all]
pub open spec fn top_ability_spec() -> Seq<Seq<u8>> {
    seq![seq![0x2au8]]  // "*"
}

/// The glob byte.
pub open spec fn glob_byte_spec() -> u8 { 0x2au8 }

/// A segment is the glob wildcard.
// r[impl ucan-ability.glob-one-level]
pub open spec fn is_glob_segment_spec(seg: Seq<u8>) -> bool {
    seg.len() == 1 && seg[0] == glob_byte_spec()
}

/// Segment equality.
pub open spec fn segments_equal_spec(a: Seq<u8>, b: Seq<u8>) -> bool {
    a =~= b
}

/// Segment-by-segment subsumption (pattern vs concrete ability).
// r[impl ucan-ability.subsumption-reflexive]
// r[impl ucan-ability.subsumption-transitive]
// r[impl ucan-ability.top-subsumes-all]
pub open spec fn ability_subsumes_spec(pattern: Seq<Seq<u8>>, ability: Seq<Seq<u8>>) -> bool
{
    // Top ability subsumes everything
    if pattern =~= top_ability_spec() {
        true
    } else if pattern.len() != ability.len() {
        false
    } else {
        ability_segments_match_spec(pattern, ability, 0)
    }
}

/// Recursive segment matching from index `from`.
// r[impl ucan-ability.glob-one-level]
pub open spec fn ability_segments_match_spec(
    pattern: Seq<Seq<u8>>,
    ability: Seq<Seq<u8>>,
    from: int,
) -> bool
    recommends pattern.len() == ability.len(), 0 <= from <= pattern.len(),
    decreases pattern.len() - from,
{
    if from >= pattern.len() {
        true
    } else {
        (is_glob_segment_spec(pattern[from]) || segments_equal_spec(pattern[from], ability[from]))
        && ability_segments_match_spec(pattern, ability, from + 1)
    }
}

/// Namespace (first segment) of an ability.
// r[impl ucan-ability.namespace-isolation]
pub open spec fn namespace_spec(ability: Seq<Seq<u8>>) -> Seq<u8>
    recommends ability.len() > 0,
{
    ability[0]
}

/// Ability containment for delegation.
// r[impl ucan-ability.containment-monotone]
pub open spec fn ability_contains_spec(parent: Seq<Seq<u8>>, child: Seq<Seq<u8>>) -> bool {
    ability_subsumes_spec(parent, child)
}

/// Ability containment for delegation.
// r[impl ucan-ability.containment-monotone]
pub open spec fn ability_contains_spec2(parent: Ability, child: Ability) -> bool {
    ability_subsumes_spec(
        parent.segments@.map_values(|s: Vec<u8>| s@),
        child.segments@.map_values(|s: Vec<u8>| s@),
    )
}

/// Whether an ability contains at least one glob wildcard segment.
pub open spec fn ability_has_glob_segment_spec(segments: Seq<Seq<u8>>) -> bool {
    exists|i: int| 0 <= i < segments.len() && is_glob_segment_spec(#[trigger] segments[i])
}

/// Whether an exec ability segment vector contains at least one glob wildcard segment.
pub open spec fn ability_has_glob_segment_vec_spec(segments: Seq<Vec<u8>>) -> bool {
    exists|i: int| 0 <= i < segments.len() && is_glob_segment_spec(#[trigger] segments[i]@)
}

/// Whether an ability has no glob segments and can represent a concrete request.
pub open spec fn ability_is_concrete_spec(segments: Seq<Seq<u8>>) -> bool {
    !ability_has_glob_segment_spec(segments)
}

/// Whether an exec ability segment vector has no glob segments.
pub open spec fn ability_is_concrete_vec_spec(segments: Seq<Vec<u8>>) -> bool {
    !ability_has_glob_segment_vec_spec(segments)
}

// --- Exec ---

/// Check byte-segment equality with an extensional postcondition.
// r[impl ucan-ability.glob-one-level]
pub fn segments_equal(a: &Vec<u8>, b: &Vec<u8>) -> (result: bool)
    ensures result == segments_equal_spec(a@, b@),
{
    if a.len() != b.len() {
        assert(a@.len() != b@.len());
        assert(!(a@ =~= b@));
        return false;
    }

    let mut i: usize = 0;
    while i < a.len()
        invariant
            a.len() == b.len(),
            0 <= i <= a.len(),
            forall|k: int| 0 <= k < i ==> a@[k] == b@[k],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            assert(a@[i as int] != b@[i as int]);
            assert(!(a@ =~= b@));
            return false;
        }
        i = i + 1;
    }

    assert(a@ =~= b@);
    true
}

/// Check whether a segment is the single-level glob wildcard.
// r[impl ucan-ability.glob-one-level]
pub fn is_glob_segment(seg: &Vec<u8>) -> (result: bool)
    ensures result == is_glob_segment_spec(seg@),
{
    seg.len() == 1 && seg[0] == 0x2au8
}

/// Check whether an ability contains any glob wildcard segment.
// r[impl ucan-ability.glob-one-level]
pub fn ability_has_glob_segment(ability: &Ability) -> (result: bool)
    ensures result == ability_has_glob_segment_vec_spec(ability.segments@),
{
    let mut i: usize = 0;
    while i < ability.segments.len()
        invariant
            0 <= i <= ability.segments.len(),
            forall|k: int| 0 <= k < i ==> !is_glob_segment_spec(#[trigger] ability.segments@[k]@),
        decreases ability.segments.len() - i,
    {
        if is_glob_segment(&ability.segments[i]) {
            assert(is_glob_segment_spec(ability.segments@[i as int]@));
            return true;
        }
        i = i + 1;
    }
    assert(forall|k: int| 0 <= k < ability.segments@.len() ==> !is_glob_segment_spec(#[trigger] ability.segments@[k]@));
    false
}

/// Check whether an ability has no glob wildcard segments.
// r[impl ucan-ability.glob-one-level]
pub fn ability_is_concrete(ability: &Ability) -> (result: bool)
    ensures result == ability_is_concrete_vec_spec(ability.segments@),
{
    !ability_has_glob_segment(ability)
}

proof fn top_ability_exec_equiv(pattern: Seq<Vec<u8>>)
    ensures
        ((pattern.len() == 1 && is_glob_segment_spec(pattern[0]@))
            == (pattern.map_values(|s: Vec<u8>| s@) =~= top_ability_spec())),
{
    if pattern.len() == 1 && is_glob_segment_spec(pattern[0]@) {
        assert(pattern[0]@ =~= seq![glob_byte_spec()]);
        assert(pattern.map_values(|s: Vec<u8>| s@).len() == 1);
        assert(pattern.map_values(|s: Vec<u8>| s@)[0] == pattern[0]@);
        assert(pattern.map_values(|s: Vec<u8>| s@) =~= top_ability_spec());
    } else if pattern.map_values(|s: Vec<u8>| s@) =~= top_ability_spec() {
        assert(pattern.map_values(|s: Vec<u8>| s@).len() == 1);
        assert(pattern.len() == 1);
        assert(pattern.map_values(|s: Vec<u8>| s@)[0] == seq![glob_byte_spec()]);
        assert(pattern[0]@ =~= seq![glob_byte_spec()]);
        assert(is_glob_segment_spec(pattern[0]@));
    }
}

proof fn segment_match_at(pattern: Seq<Vec<u8>>, ability: Seq<Vec<u8>>, i: int)
    requires
        0 <= i < pattern.len(),
        pattern.len() == ability.len(),
        is_glob_segment_spec(pattern[i]@) || segments_equal_spec(pattern[i]@, ability[i]@),
    ensures
        is_glob_segment_spec(pattern.map_values(|s: Vec<u8>| s@)[i])
            || segments_equal_spec(
                pattern.map_values(|s: Vec<u8>| s@)[i],
                ability.map_values(|s: Vec<u8>| s@)[i],
            ),
{
    assert(pattern.map_values(|s: Vec<u8>| s@)[i] == pattern[i]@);
    assert(ability.map_values(|s: Vec<u8>| s@)[i] == ability[i]@);
}

proof fn segment_mismatch_at(pattern: Seq<Vec<u8>>, ability: Seq<Vec<u8>>, i: int)
    requires
        0 <= i < pattern.len(),
        pattern.len() == ability.len(),
        !is_glob_segment_spec(pattern[i]@),
        !segments_equal_spec(pattern[i]@, ability[i]@),
    ensures
        !(is_glob_segment_spec(pattern.map_values(|s: Vec<u8>| s@)[i])
            || segments_equal_spec(
                pattern.map_values(|s: Vec<u8>| s@)[i],
                ability.map_values(|s: Vec<u8>| s@)[i],
            )),
{
    assert(pattern.map_values(|s: Vec<u8>| s@)[i] == pattern[i]@);
    assert(ability.map_values(|s: Vec<u8>| s@)[i] == ability[i]@);
}

proof fn all_vec_segments_match_implies_spec(pattern: Seq<Vec<u8>>, ability: Seq<Vec<u8>>, from: int)
    requires
        pattern.len() == ability.len(),
        0 <= from <= pattern.len(),
        forall|k: int| from <= k < pattern.len() ==>
            is_glob_segment_spec(#[trigger] pattern[k]@) || segments_equal_spec(pattern[k]@, ability[k]@),
    ensures ability_segments_match_spec(
        pattern.map_values(|s: Vec<u8>| s@),
        ability.map_values(|s: Vec<u8>| s@),
        from,
    ),
    decreases pattern.len() - from,
{
    if from < pattern.len() {
        segment_match_at(pattern, ability, from);
        all_vec_segments_match_implies_spec(pattern, ability, from + 1);
    }
}

proof fn vec_mismatch_implies_not_segments_match(
    pattern: Seq<Vec<u8>>,
    ability: Seq<Vec<u8>>,
    from: int,
    bad: int,
)
    requires
        pattern.len() == ability.len(),
        0 <= from <= bad < pattern.len(),
        forall|k: int| from <= k < bad ==>
            is_glob_segment_spec(#[trigger] pattern[k]@) || segments_equal_spec(pattern[k]@, ability[k]@),
        !is_glob_segment_spec(pattern[bad]@),
        !segments_equal_spec(pattern[bad]@, ability[bad]@),
    ensures !ability_segments_match_spec(
        pattern.map_values(|s: Vec<u8>| s@),
        ability.map_values(|s: Vec<u8>| s@),
        from,
    ),
    decreases bad - from,
{
    if from < bad {
        segment_match_at(pattern, ability, from);
        vec_mismatch_implies_not_segments_match(pattern, ability, from + 1, bad);
    } else {
        segment_mismatch_at(pattern, ability, bad);
    }
}

/// Check segment-by-segment ability matching.
// r[impl ucan-ability.glob-one-level]
pub fn ability_segments_match(pattern: &Vec<Vec<u8>>, ability: &Vec<Vec<u8>>) -> (result: bool)
    requires pattern.len() == ability.len(),
    ensures result == ability_segments_match_spec(
        pattern@.map_values(|s: Vec<u8>| s@),
        ability@.map_values(|s: Vec<u8>| s@),
        0,
    ),
{
    let mut i: usize = 0;
    while i < pattern.len()
        invariant
            pattern.len() == ability.len(),
            0 <= i <= pattern.len(),
            forall|k: int| 0 <= k < i ==>
                is_glob_segment_spec(#[trigger] pattern@[k]@) || segments_equal_spec(pattern@[k]@, ability@[k]@),
        decreases pattern.len() - i,
    {
        let glob = is_glob_segment(&pattern[i]);
        if !glob {
            let equal = segments_equal(&pattern[i], &ability[i]);
            if !equal {
                proof {
                    vec_mismatch_implies_not_segments_match(pattern@, ability@, 0, i as int);
                }
                return false;
            }
        }
        proof { segment_match_at(pattern@, ability@, i as int); }
        i = i + 1;
    }

    proof {
        all_vec_segments_match_implies_spec(pattern@, ability@, 0);
    }
    true
}

/// Check if a pattern subsumes an ability at runtime.
// r[impl ucan-ability.subsumption-reflexive]
// r[impl ucan-ability.top-subsumes-all]
pub fn ability_subsumes(pattern: &Ability, ability: &Ability) -> (result: bool)
    ensures result == ability_subsumes_spec(
        pattern.segments@.map_values(|s: Vec<u8>| s@),
        ability.segments@.map_values(|s: Vec<u8>| s@),
    ),
{
    // Top ability: single "*" segment
    if pattern.segments.len() == 1 && is_glob_segment(&pattern.segments[0]) {
        proof { top_ability_exec_equiv(pattern.segments@); }
        return true;
    }
    proof { top_ability_exec_equiv(pattern.segments@); }

    if pattern.segments.len() != ability.segments.len() {
        return false;
    }

    ability_segments_match(&pattern.segments, &ability.segments)
}

// --- Proofs ---

/// Reflexivity: a concrete ability subsumes itself.
// r[verify ucan-ability.subsumption-reflexive]
pub proof fn ability_subsumes_reflexive(a: Seq<Seq<u8>>)
    requires
        a.len() > 0,
        // Not the top pattern itself (which trivially works anyway)
        forall|i: int| 0 <= i < a.len() ==> !is_glob_segment_spec(#[trigger] a[i]),
    ensures ability_subsumes_spec(a, a),
{
    if a =~= top_ability_spec() {
        // Top subsumes everything including itself
    } else {
        ability_segments_match_self(a, 0);
    }
}

proof fn ability_segments_match_self(a: Seq<Seq<u8>>, from: int)
    requires
        0 <= from <= a.len(),
        forall|i: int| 0 <= i < a.len() ==> !is_glob_segment_spec(#[trigger] a[i]),
    ensures ability_segments_match_spec(a, a, from),
    decreases a.len() - from,
{
    if from < a.len() {
        assert(segments_equal_spec(a[from], a[from]));
        ability_segments_match_self(a, from + 1);
    }
}

/// Top ability subsumes all abilities.
// r[verify ucan-ability.top-subsumes-all]
pub proof fn top_subsumes_all(a: Seq<Seq<u8>>)
    ensures ability_subsumes_spec(top_ability_spec(), a),
{
}

/// Namespace isolation: different top-level namespaces never subsume (non-top patterns).
// r[verify ucan-ability.namespace-isolation]
pub proof fn namespace_isolation(pattern: Seq<Seq<u8>>, ability: Seq<Seq<u8>>)
    requires
        pattern.len() > 0,
        ability.len() > 0,
        !(pattern =~= top_ability_spec()),
        namespace_spec(pattern) != namespace_spec(ability),
        !is_glob_segment_spec(pattern[0]),
    ensures !ability_subsumes_spec(pattern, ability),
{
    if pattern.len() != ability.len() {
        // Different lengths, can't subsume
    } else {
        // Same length, but first segments differ and pattern[0] is not glob
        namespace_isolation_helper(pattern, ability);
    }
}

proof fn namespace_isolation_helper(pattern: Seq<Seq<u8>>, ability: Seq<Seq<u8>>)
    requires
        pattern.len() == ability.len(),
        pattern.len() > 0,
        pattern[0] != ability[0],
        !is_glob_segment_spec(pattern[0]),
    ensures !ability_segments_match_spec(pattern, ability, 0),
{
    // First segment doesn't match and isn't glob
}

/// Single glob matches one level, not deeper nesting.
// r[verify ucan-ability.glob-one-level]
pub proof fn glob_one_level_matches(ns: Seq<u8>, action: Seq<u8>)
    requires
        !is_glob_segment_spec(ns),
        !is_glob_segment_spec(action),
    ensures
        ability_subsumes_spec(seq![ns, seq![glob_byte_spec()]], seq![ns, action]),
        !ability_subsumes_spec(seq![ns, seq![glob_byte_spec()]], seq![ns, action, action]),
{
    let pattern = seq![ns, seq![glob_byte_spec()]];
    let match_ability = seq![ns, action];
    let deep_ability = seq![ns, action, action];

    // pattern is not top (has 2 segments)
    assert(pattern.len() == 2);
    assert(top_ability_spec().len() == 1);
    assert(!(pattern =~= top_ability_spec()));

    // Two segments match
    assert(pattern.len() == match_ability.len());
    // Base case at index 2
    assert(ability_segments_match_spec(pattern, match_ability, 2));
    // Index 1: glob matches anything
    assert(is_glob_segment_spec(pattern[1]));
    assert(ability_segments_match_spec(pattern, match_ability, 1));
    // Index 0: namespace segments are equal
    assert(segments_equal_spec(pattern[0], match_ability[0]));
    assert(ability_segments_match_spec(pattern, match_ability, 0));

    // Three segments vs two: length mismatch
    assert(pattern.len() != deep_ability.len());
}

/// Containment monotonicity: if parent contains child, anything child subsumes,
/// parent also subsumes.
// r[verify ucan-ability.containment-monotone]
pub proof fn containment_monotone(parent: Seq<Seq<u8>>, child: Seq<Seq<u8>>, op: Seq<Seq<u8>>)
    requires
        ability_contains_spec(parent, child),
        ability_subsumes_spec(child, op),
    ensures ability_subsumes_spec(parent, op),
{
    // Parent subsumes child, child subsumes op.
    // If parent is top, parent subsumes everything.
    // If child is top, then child subsumes op for any op, and parent subsumes child (which is top).
    // Otherwise, all three have equal length, and segment matching is transitive.
    if parent =~= top_ability_spec() {
        // trivial
    } else if child =~= top_ability_spec() {
        // parent subsumes top means parent == top (since non-top can't subsume top with different length)
        // But we assumed parent != top. Contradiction: non-top pattern has length != 1 or
        // doesn't match top structure. So parent must subsume top only if parent is top.
        // Actually, ability_subsumes_spec(parent, top) requires parent.len() == top.len() == 1
        // and segments match. Since parent[0] isn't glob (parent != top), it must equal "*" which
        // means parent IS top. Contradiction with our assumption.
        assert(parent.len() == 1);
        assert(ability_segments_match_spec(parent, top_ability_spec(), 0));
        assert(segments_equal_spec(parent[0], seq![glob_byte_spec()]) || is_glob_segment_spec(parent[0]));
    } else {
        // All three have equal length
        assert(parent.len() == child.len());
        assert(child.len() == op.len());
        segments_match_transitive(parent, child, op, 0);
    }
}

proof fn segments_match_transitive(
    a: Seq<Seq<u8>>,
    b: Seq<Seq<u8>>,
    c: Seq<Seq<u8>>,
    from: int,
)
    requires
        a.len() == b.len(),
        b.len() == c.len(),
        0 <= from <= a.len(),
        ability_segments_match_spec(a, b, from),
        ability_segments_match_spec(b, c, from),
    ensures ability_segments_match_spec(a, c, from),
    decreases a.len() - from,
{
    if from < a.len() {
        // a[from] matches b[from]: either glob or equal
        // b[from] matches c[from]: either glob or equal
        // Cases:
        // 1. a[from] is glob => a[from] matches c[from] (glob matches anything)
        // 2. a[from] == b[from] and b[from] is glob => b[from] matches c[from], a[from] == "*" => a is glob
        // 3. a[from] == b[from] and b[from] == c[from] => a[from] == c[from]
        segments_match_transitive(a, b, c, from + 1);
    }
}

/// Transitivity: if a subsumes b and b subsumes c, then a subsumes c.
// r[verify ucan-ability.subsumption-transitive]
pub proof fn ability_subsumes_transitive(a: Seq<Seq<u8>>, b: Seq<Seq<u8>>, c: Seq<Seq<u8>>)
    requires
        ability_subsumes_spec(a, b),
        ability_subsumes_spec(b, c),
    ensures ability_subsumes_spec(a, c),
{
    containment_monotone(a, b, c);
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ability(segs: &[&[u8]]) -> Ability {
        Ability {
            segments: segs.iter().map(|s| s.to_vec()).collect(),
        }
    }

    #[test]
    fn test_top_subsumes_all() {
        let top = make_ability(&[b"*"]);
        let crud_read = make_ability(&[b"crud", b"read"]);
        let msg_send = make_ability(&[b"msg", b"send"]);
        assert!(ability_subsumes(&top, &crud_read));
        assert!(ability_subsumes(&top, &msg_send));
    }

    #[test]
    fn test_reflexive() {
        let a = make_ability(&[b"crud", b"read"]);
        assert!(ability_subsumes(&a, &a));
    }

    #[test]
    fn test_glob_one_level() {
        let pattern = make_ability(&[b"crud", b"*"]);
        let match_target = make_ability(&[b"crud", b"read"]);
        let deep = make_ability(&[b"crud", b"nested", b"deep"]);
        assert!(ability_subsumes(&pattern, &match_target));
        assert!(!ability_subsumes(&pattern, &deep));
    }

    #[test]
    fn test_namespace_isolation() {
        let msg = make_ability(&[b"msg", b"send"]);
        let crud = make_ability(&[b"crud", b"read"]);
        assert!(!ability_subsumes(&msg, &crud));
        assert!(!ability_subsumes(&crud, &msg));
    }

    #[test]
    fn test_glob_and_concrete_admission() {
        let concrete = make_ability(&[b"crud", b"read"]);
        let scoped_wildcard = make_ability(&[b"crud", b"*"]);
        let top = make_ability(&[b"*"]);

        assert!(!ability_has_glob_segment(&concrete));
        assert!(ability_is_concrete(&concrete));
        assert!(ability_has_glob_segment(&scoped_wildcard));
        assert!(!ability_is_concrete(&scoped_wildcard));
        assert!(ability_has_glob_segment(&top));
        assert!(!ability_is_concrete(&top));
    }

    #[test]
    fn test_different_actions_dont_subsume() {
        let read = make_ability(&[b"crud", b"read"]);
        let write = make_ability(&[b"crud", b"write"]);
        assert!(!ability_subsumes(&read, &write));
    }
}

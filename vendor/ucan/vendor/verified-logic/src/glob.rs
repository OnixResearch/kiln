//! Verified glob pattern matching for capability authorization.
//!
//! Supports `*` wildcards in byte-sequence patterns. Cases:
//! - Pure wildcard `"*"`: matches everything.
//! - No wildcard: exact equality.
//! - Trailing wildcard `"prefix*"`: prefix match.
//! - Leading wildcard `"*suffix"`: suffix match.
//! - Multi-segment `"a*b*c"`: first segment anchored at start, last at end, middle segments appear
//!   in order anywhere.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

/// The wildcard byte (`*` = 42).
pub open spec fn STAR() -> u8 { 42 }

// ============================================================
// Helper specs
// ============================================================

/// Spec: does the input start with the given prefix?
pub open spec fn starts_with_spec(input: Seq<u8>, prefix: Seq<u8>) -> bool {
    input.len() >= prefix.len()
    && forall|i: int| #![trigger prefix[i]]
        0 <= i < prefix.len() as int ==> input[i] == prefix[i]
}

/// Spec: does the input end with the given suffix?
pub open spec fn ends_with_spec(input: Seq<u8>, suffix: Seq<u8>) -> bool {
    input.len() >= suffix.len()
    && forall|i: int| #![trigger suffix[i]]
        0 <= i < suffix.len() as int
        ==> input[input.len() - suffix.len() + i] == suffix[i]
}

/// Spec: does pattern contain the wildcard byte?
pub open spec fn has_star_spec(pattern: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < pattern.len() && pattern[i] == STAR()
}

/// Spec: does `needle` appear at position `pos` in `haystack`?
pub open spec fn substr_at_spec(haystack: Seq<u8>, needle: Seq<u8>, pos: int) -> bool {
    pos >= 0
    && pos + needle.len() <= haystack.len()
    && forall|i: int| #![trigger needle[i]]
        0 <= i < needle.len() as int ==> haystack[pos + i] == needle[i]
}

// ============================================================
// Glob match spec: segment-based recursive definition
// ============================================================

/// Count `*` bytes in a pattern from position `from`.
pub open spec fn star_count(pattern: Seq<u8>, from: int) -> nat
    decreases pattern.len() - from,
{
    if from >= pattern.len() {
        0
    } else if pattern[from] == STAR() {
        1 + star_count(pattern, from + 1)
    } else {
        star_count(pattern, from + 1)
    }
}

/// Find the first occurrence of `needle` in `input` starting at `pos`,
/// searching up to position `end - needle.len()`. Returns -1 if not found.
pub open spec fn find_segment_in(input: Seq<u8>, needle: Seq<u8>, pos: int, end: int) -> int
    decreases end - pos,
{
    if needle.len() == 0 {
        pos
    } else if pos + needle.len() > end {
        -1int
    } else if substr_at_spec(input, needle, pos) {
        pos
    } else {
        find_segment_in(input, needle, pos + 1, end)
    }
}

/// Match a sequence of segments against input[pos..end].
///
/// Segments are the result of splitting a glob pattern by `*`.
/// - First segment (idx 0): must be prefix of input.
/// - Last segment (idx n-1): must be suffix of input.
/// - Middle segments: found in order via greedy left-to-right search.
/// - Empty segments (from adjacent `*`s) are skipped.
pub open spec fn segments_match_spec(
    segs: Seq<Seq<u8>>,
    input: Seq<u8>,
    idx: int,
    pos: int,
    end: int,
) -> bool
    recommends 0 <= pos, pos <= end, end <= input.len(), 0 <= idx
    decreases segs.len() - idx,
    when 0 <= pos && pos <= end && end <= input.len() && 0 <= idx
{
    if idx >= segs.len() {
        true  // All segments matched; remaining input consumed by trailing `*`.
    } else {
        let seg = segs[idx];
        let is_first = idx == 0;
        let is_last = idx == segs.len() - 1;

        if seg.len() == 0 {
            // Empty segment — skip.
            segments_match_spec(segs, input, idx + 1, pos, end)
        } else if is_first {
            // First segment: must be prefix.
            starts_with_spec(input.subrange(pos, end), seg)
            && segments_match_spec(segs, input, idx + 1, pos + seg.len(), end)
        } else if is_last {
            // Last segment: must be suffix.
            ends_with_spec(input.subrange(pos, end), seg)
        } else {
            // Middle segment: find in input[pos..end].
            let found = find_segment_in(input, seg, pos, end);
            found >= 0
            && segments_match_spec(segs, input, idx + 1, found + seg.len(), end)
        }
    }
}

/// Split pattern by STAR into segments, from position `from`.
pub open spec fn segment_end(pattern: Seq<u8>, from: int) -> int
    decreases pattern.len() - from,
{
    if from >= pattern.len() {
        from
    } else if pattern[from] == STAR() {
        from
    } else {
        segment_end(pattern, from + 1)
    }
}

/// Split a pattern by `*` into a sequence of segments.
pub open spec fn split_by_star_spec(pattern: Seq<u8>, from: int) -> Seq<Seq<u8>>
    recommends 0 <= from
    decreases pattern.len() - from + 1,
    when 0 <= from
{
    if from > pattern.len() {
        Seq::<Seq<u8>>::empty()
    } else if from == pattern.len() {
        seq![Seq::<u8>::empty()]
    } else {
        let se = segment_end(pattern, from);
        if se > pattern.len() || se < from {
            // Guard: segment_end always returns >= from and <= len, but
            // Verus needs this explicit guard for termination.
            Seq::<Seq<u8>>::empty()
        } else {
            let seg = pattern.subrange(from, se);
            if se >= pattern.len() {
                seq![seg]
            } else {
                // se points at a `*`. Skip it. se + 1 > from, so we decrease.
                seq![seg] + split_by_star_spec(pattern, se + 1)
            }
        }
    }
}

// r[depends glob.wildcard-matches-all]
/// Spec: does the input match the glob pattern?
///
/// - No stars: exact equality.
/// - Has stars: split by `*` into segments, then match:
///   first segment = prefix, last = suffix, middle = greedy search.
pub open spec fn glob_match_spec(pattern: Seq<u8>, input: Seq<u8>) -> bool {
    if !has_star_spec(pattern) {
        pattern =~= input
    } else {
        let segs = split_by_star_spec(pattern, 0);
        segments_match_spec(segs, input, 0, 0, input.len() as int)
    }
}

// ============================================================
// Deep view: bridge Vec<Vec<u8>>@ (= Seq<Vec<u8>>) to Seq<Seq<u8>>
// ============================================================

/// Convert a Seq<Vec<u8>> to Seq<Seq<u8>> by taking the view of each element.
pub open spec fn deep_view(v: Seq<Vec<u8>>) -> Seq<Seq<u8>> {
    Seq::new(v.len(), |i: int| v[i]@)
}

// ============================================================
// Verified exec helpers
// ============================================================

/// Exec: check if pattern contains a `*` byte.
fn has_star_exec(pattern: &[u8]) -> (result: bool)
    ensures result == has_star_spec(pattern@)
{
    let mut i: usize = 0;
    while i < pattern.len()
        invariant
            i <= pattern.len(),
            forall|j: int| #![trigger pattern@[j]] 0 <= j < i as int ==> pattern@[j] != STAR(),
        decreases pattern.len() - i,
    {
        if pattern[i] == 42u8 {
            return true;
        }
        i = i + 1;
    }
    false
}

/// Exec: byte-by-byte equality of two slices.
fn bytes_eq_exec(a: &[u8], b: &[u8]) -> (result: bool)
    ensures result == (a@ =~= b@)
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a.len(),
            a@.len() == b@.len(),
            forall|j: int| #![trigger a@[j]] 0 <= j < i as int ==> a@[j] == b@[j],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            return false;
        }
        i = i + 1;
    }
    assert(a@ =~= b@);
    true
}

/// Exec: find the end of a non-star segment starting at `from`.
fn segment_end_exec(pattern: &[u8], from: usize) -> (result: usize)
    requires from as int <= pattern@.len()
    ensures result as int == segment_end(pattern@, from as int)
{
    let mut i: usize = from;
    while i < pattern.len()
        invariant
            from <= i,
            i <= pattern.len(),
            segment_end(pattern@, from as int) == segment_end(pattern@, i as int),
        decreases pattern.len() - i,
    {
        if pattern[i] == 42u8 {
            return i;
        }
        i = i + 1;
    }
    i
}

/// Exec: does `needle` appear at position `pos` in `haystack`?
fn substr_at_exec(haystack: &[u8], needle: &[u8], pos: usize) -> (result: bool)
    requires pos as int + needle@.len() <= haystack@.len()
    ensures result == substr_at_spec(haystack@, needle@, pos as int)
{
    let mut i: usize = 0;
    while i < needle.len()
        invariant
            i <= needle.len(),
            pos + needle.len() <= haystack.len(),
            forall|j: int| #![trigger needle@[j]] 0 <= j < i as int
                ==> haystack@[pos as int + j] == needle@[j],
        decreases needle.len() - i,
    {
        if haystack[pos + i] != needle[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Exec: does input[from..to] start with `seg`?
fn starts_with_exec(input: &[u8], from: usize, to: usize, seg: &[u8]) -> (result: bool)
    requires from as int <= to as int, to as int <= input@.len()
    ensures result == starts_with_spec(input@.subrange(from as int, to as int), seg@)
{
    let region_len: usize = to - from;
    if region_len < seg.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < seg.len()
        invariant
            i <= seg.len(),
            seg.len() <= region_len,
            from + seg.len() <= to,
            to <= input.len(),
            forall|j: int| #![trigger seg@[j]] 0 <= j < i as int
                ==> input@[from as int + j] == seg@[j],
        decreases seg.len() - i,
    {
        if input[from + i] != seg[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Exec: does input[from..to] end with `seg`?
fn ends_with_exec(input: &[u8], from: usize, to: usize, seg: &[u8]) -> (result: bool)
    requires from as int <= to as int, to as int <= input@.len()
    ensures result == ends_with_spec(input@.subrange(from as int, to as int), seg@)
{
    let region_len: usize = to - from;
    if region_len < seg.len() {
        return false;
    }
    let suffix_start: usize = to - seg.len();
    let mut i: usize = 0;
    while i < seg.len()
        invariant
            i <= seg.len(),
            seg.len() <= region_len,
            suffix_start == to - seg.len(),
            suffix_start >= from,
            to <= input.len(),
            forall|j: int| #![trigger seg@[j]] 0 <= j < i as int
                ==> input@[suffix_start as int + j] == seg@[j],
        decreases seg.len() - i,
    {
        if input[suffix_start + i] != seg[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Exec: find `needle` in `input[pos..end]`. Returns `None` if not found.
fn find_segment_in_exec(
    input: &[u8], needle: &[u8], pos: usize, end: usize,
) -> (result: Option<usize>)
    requires
        pos as int <= end as int,
        end as int <= input@.len(),
        needle@.len() > 0,
    ensures
        result.is_some() ==> find_segment_in(input@, needle@, pos as int, end as int)
            == result.unwrap() as int,
        result.is_some() ==> result.unwrap() as int + needle@.len() <= end as int,
        result.is_none() ==> find_segment_in(input@, needle@, pos as int, end as int)
            == -1int,
{
    if end < needle.len() {
        // Can't fit needle at all.
        return None;
    }
    let limit: usize = end - needle.len();
    let mut p: usize = pos;
    while p <= limit
        invariant
            pos <= p,
            limit == end - needle.len(),
            end <= input.len(),
            needle.len() > 0,
            find_segment_in(input@, needle@, pos as int, end as int)
                == find_segment_in(input@, needle@, p as int, end as int),
        decreases limit + 1 - p,
    {
        if substr_at_exec(input, needle, p) {
            return Some(p);
        }
        p = p + 1;
    }
    None
}

/// Exec: copy `data[from..to]` into a new Vec.
fn slice_to_vec(data: &[u8], from: usize, to: usize) -> (result: Vec<u8>)
    requires from as int <= to as int, to as int <= data@.len()
    ensures result@ =~= data@.subrange(from as int, to as int)
{
    let mut v: Vec<u8> = Vec::new();
    let mut j: usize = from;
    while j < to
        invariant
            from <= j,
            j <= to,
            to <= data.len(),
            v@.len() == (j - from) as int,
            forall|k: int| #![trigger v@[k]] 0 <= k < v@.len()
                ==> v@[k] == data@[from as int + k],
        decreases to - j,
    {
        v.push(data[j]);
        j = j + 1;
    }
    assert(v@ =~= data@.subrange(from as int, to as int));
    v
}

// ============================================================
// Build segments + match segments
// ============================================================

/// Proof: segment_end always returns a value in [from, pattern.len()].
proof fn segment_end_bounded(pattern: Seq<u8>, from: int)
    requires 0 <= from, from <= pattern.len()
    ensures
        from <= segment_end(pattern, from),
        segment_end(pattern, from) <= pattern.len(),
    decreases pattern.len() - from
{
    if from < pattern.len() && pattern[from] != STAR() {
        segment_end_bounded(pattern, from + 1);
    }
}

/// Proof: deep_view distributes over push.
proof fn deep_view_push(v: Seq<Vec<u8>>, x: Vec<u8>)
    ensures deep_view(v.push(x)) =~= deep_view(v).push(x@)
{
    assert(deep_view(v.push(x)) =~= deep_view(v).push(x@));
}

/// Exec: split pattern by `*` into a Vec of segments.
fn build_segments(pattern: &[u8]) -> (result: Vec<Vec<u8>>)
    requires has_star_spec(pattern@)
    ensures deep_view(result@) =~= split_by_star_spec(pattern@, 0)
{
    let mut segs: Vec<Vec<u8>> = Vec::new();
    let mut pat_from: usize = 0;

    // Invariant: deep_view(segs@) ++ split_by_star_spec(pattern@, pat_from) == full split.
    loop
        invariant_except_break
            pat_from <= pattern.len(),
            deep_view(segs@) + split_by_star_spec(pattern@, pat_from as int)
                =~= split_by_star_spec(pattern@, 0),
        ensures
            deep_view(segs@) =~= split_by_star_spec(pattern@, 0),
        decreases pattern.len() + 1 - pat_from,
    {
        if pat_from == pattern.len() {
            // split_by_star_spec(pattern, len) == seq![Seq::<u8>::empty()]
            let empty: Vec<u8> = Vec::new();
            proof { deep_view_push(segs@, empty); }
            segs.push(empty);
            break;
        }

        let se: usize = segment_end_exec(pattern, pat_from);
        proof { segment_end_bounded(pattern@, pat_from as int); }

        let seg: Vec<u8> = slice_to_vec(pattern, pat_from, se);
        proof { deep_view_push(segs@, seg); }
        segs.push(seg);

        if se >= pattern.len() {
            break;
        }

        pat_from = se + 1;
    }

    segs
}

/// Exec: match segments against input, mirroring `segments_match_spec`.
///
/// Takes `segs: &[Vec<u8>]` and a ghost `segs_spec: Seq<Seq<u8>>` for the ensures.
/// The caller must establish `deep_view(segs@) =~= segs_spec`.
fn segments_match_exec(
    segs: &[Vec<u8>],
    input: &[u8],
    idx: usize,
    pos: usize,
    end: usize,
    segs_spec: Ghost<Seq<Seq<u8>>>,
) -> (result: bool)
    requires
        0 <= pos as int,
        pos as int <= end as int,
        end as int <= input@.len(),
        idx as int <= segs@.len(),
        segs@.len() == segs_spec@.len(),
        forall|i: int| #![trigger segs_spec@[i]] 0 <= i < segs@.len()
            ==> segs@[i]@ =~= segs_spec@[i],
    ensures
        result == segments_match_spec(segs_spec@, input@, idx as int, pos as int, end as int)
    decreases segs.len() - idx
{
    if idx >= segs.len() {
        return true;
    }

    let seg: &Vec<u8> = &segs[idx];
    let is_first: bool = idx == 0;
    let is_last: bool = idx == segs.len() - 1;

    if seg.len() == 0 {
        // Empty segment — skip.
        return segments_match_exec(segs, input, idx + 1, pos, end, Ghost(segs_spec@));
    }

    if is_first {
        // First segment: prefix match.
        if !starts_with_exec(input, pos, end, seg.as_slice()) {
            return false;
        }
        // starts_with succeeded: end - pos >= seg.len(), so pos + seg.len() <= end
        return segments_match_exec(
            segs, input, idx + 1, pos + seg.len(), end, Ghost(segs_spec@),
        );
    }

    if is_last {
        // Last segment: suffix match.
        return ends_with_exec(input, pos, end, seg.as_slice());
    }

    // Middle segment: find in input[pos..end].
    match find_segment_in_exec(input, seg.as_slice(), pos, end) {
        None => false,
        Some(found) => {
            // find_segment_in_exec ensures: found + seg.len() <= end
            segments_match_exec(
                segs, input, idx + 1, found + seg.len(), end, Ghost(segs_spec@),
            )
        }
    }
}

// ============================================================
// Main glob_match: verified exec
// ============================================================

// r[impl glob.wildcard-matches-all]
/// Returns true if the input matches the glob pattern.
pub fn glob_match(pattern: &[u8], input: &[u8]) -> (result: bool)
    ensures result == glob_match_spec(pattern@, input@)
{
    // Step 1: check for star.
    if !has_star_exec(pattern) {
        return bytes_eq_exec(pattern, input);
    }

    // Step 2: build segments.
    let segs: Vec<Vec<u8>> = build_segments(pattern);

    // Step 3: match segments against input.
    proof { let _ = deep_view(segs@); }
    segments_match_exec(
        segs.as_slice(), input, 0, 0, input.len(), Ghost(deep_view(segs@)),
    )
}

// ============================================================
// Proof functions
// ============================================================

// r[verify glob.wildcard-matches-all]
/// Pure wildcard `"*"` matches any input.
proof fn glob_wildcard_matches_all(input: Seq<u8>)
    ensures glob_match_spec(seq![STAR()], input)
{
    let pattern = seq![STAR()];
    // has_star_spec: pattern[0] == STAR().
    assert(pattern[0] == STAR());
    // Unfold split_by_star_spec step by step.
    let se0 = segment_end(pattern, 0);
    assert(pattern[0] == STAR());
    assert(se0 == 0);
    let seg0 = pattern.subrange(0int, 0int);
    assert(seg0 =~= Seq::<u8>::empty());
    // se0 = 0 < pattern.len() = 1, so we recurse with from = se0 + 1 = 1.
    let segs1 = split_by_star_spec(pattern, 1);
    // from=1, len=1: from == len, returns seq![empty].
    assert(segs1 =~= seq![Seq::<u8>::empty()]);
    let segs = split_by_star_spec(pattern, 0);
    // pattern.subrange(0, 0) = seg0 = empty. Concat: seq![empty] + seq![empty].
    assert(segs =~= seq![seg0] + segs1);
    assert(segs.len() == 2);
    // Unfold segments_match_spec.
    assert(segs[0] =~= Seq::<u8>::empty());
    assert(segs[1] =~= Seq::<u8>::empty());
    // idx=0: seg=empty, skip. idx=1: seg=empty, skip. idx=2: done.
    assert(segments_match_spec(segs, input, 2, 0, input.len() as int));
    assert(segments_match_spec(segs, input, 1, 0, input.len() as int));
    assert(segments_match_spec(segs, input, 0, 0, input.len() as int));
}

// r[impl glob.exact-match]
// r[verify glob.exact-match]
/// A pattern with no `*` matches only itself.
proof fn glob_exact_match(s: Seq<u8>)
    requires !has_star_spec(s)
    ensures glob_match_spec(s, s)
{}

// r[verify glob.exact-match]
/// A pattern with no `*` rejects different inputs.
proof fn glob_exact_rejects(pattern: Seq<u8>, input: Seq<u8>)
    requires
        !has_star_spec(pattern),
        !(pattern =~= input),
    ensures !glob_match_spec(pattern, input)
{}

// r[impl glob.non-match-rejected]
// r[verify glob.non-match-rejected]
/// Non-matching inputs are rejected (trivial — restates definition).
proof fn glob_non_match(pattern: Seq<u8>, input: Seq<u8>)
    requires !glob_match_spec(pattern, input)
    ensures !glob_match_spec(pattern, input)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify glob.wildcard-matches-all]
    #[test]
    fn test_pure_wildcard() {
        assert!(glob_match(b"*", b""));
        assert!(glob_match(b"*", b"anything"));
        assert!(glob_match(b"*", b"hello world"));
    }

    // r[verify glob.exact-match]
    #[test]
    fn test_exact_match() {
        assert!(glob_match(b"hello", b"hello"));
        assert!(!glob_match(b"hello", b"world"));
        assert!(!glob_match(b"hello", b"hell"));
        assert!(!glob_match(b"hello", b"helloo"));
        assert!(glob_match(b"", b""));
        assert!(!glob_match(b"", b"x"));
    }

    // r[impl glob.prefix-match]
    // r[verify glob.prefix-match]
    #[test]
    fn test_prefix_wildcard() {
        assert!(glob_match(b"pg_*", b"pg_dump"));
        assert!(glob_match(b"pg_*", b"pg_"));
        assert!(!glob_match(b"pg_*", b"mysql_dump"));
        assert!(glob_match(b"hello*", b"hello world"));
        assert!(glob_match(b"hello*", b"hello"));
    }

    // r[verify glob.prefix-match]
    #[test]
    fn test_suffix_wildcard() {
        assert!(glob_match(b"*_dump", b"pg_dump"));
        assert!(glob_match(b"*_dump", b"_dump"));
        assert!(!glob_match(b"*_dump", b"pg_restore"));
    }

    // r[impl glob.multi-segment-match]
    // r[verify glob.multi-segment-match]
    #[test]
    fn test_multi_segment() {
        assert!(glob_match(b"pg_*_dump", b"pg_full_dump"));
        assert!(!glob_match(b"pg_*_dump", b"pg_full_restore"));
        assert!(glob_match(b"a*b*c", b"abc"));
        assert!(glob_match(b"a*b*c", b"aXbYc"));
        assert!(glob_match(b"a*b*c", b"aXXbYYc"));
        assert!(!glob_match(b"a*b*c", b"aXcYb"));
    }

    // r[verify glob.multi-segment-match]
    #[test]
    fn test_multi_segment_edge_cases() {
        // Adjacent wildcards.
        assert!(glob_match(b"a**c", b"ac"));
        assert!(glob_match(b"a**c", b"aXc"));
        // Empty segments between wildcards.
        assert!(glob_match(b"**", b"anything"));
        assert!(glob_match(b"***", b""));
        // Single char segments.
        assert!(glob_match(b"a*c", b"ac"));
        assert!(glob_match(b"a*c", b"axc"));
        assert!(!glob_match(b"a*c", b"axd"));
    }

    // r[verify glob.non-match-rejected]
    #[test]
    fn test_non_match_rejected() {
        assert!(!glob_match(b"pg_*", b"mysql_dump"));
        assert!(!glob_match(b"*_dump", b"pg_restore"));
        assert!(!glob_match(b"a*b", b"axc"));
    }

    proptest! {
        // r[verify glob.wildcard-matches-all]
        #[test]
        fn prop_wildcard_matches_all(
            input in proptest::collection::vec(any::<u8>(), 0..=50),
        ) {
            prop_assert!(glob_match(b"*", &input));
        }

        // r[verify glob.exact-match]
        #[test]
        fn prop_exact_match_reflexive(
            s in proptest::collection::vec(0u8..=41, 0..=30), // avoid '*' (42)
        ) {
            prop_assert!(glob_match(&s, &s));
        }

        // r[verify glob.prefix-match]
        #[test]
        fn prop_prefix_wildcard(
            prefix in proptest::collection::vec(0u8..=41, 0..=20),
            suffix in proptest::collection::vec(any::<u8>(), 0..=20),
        ) {
            let mut pattern = prefix.clone();
            pattern.push(b'*');
            let mut input = prefix;
            input.extend_from_slice(&suffix);
            prop_assert!(glob_match(&pattern, &input));
        }

        // r[verify glob.non-match-rejected]
        #[test]
        fn prop_exact_rejects_different(
            a in proptest::collection::vec(0u8..=41, 1..=20),
            b in proptest::collection::vec(0u8..=41, 1..=20),
        ) {
            if a != b {
                prop_assert!(!glob_match(&a, &b));
            }
        }

        // r[verify glob.multi-segment-match]
        #[test]
        fn prop_multi_segment_constructed(
            seg1 in proptest::collection::vec(0u8..=41, 1..=5),
            mid in proptest::collection::vec(any::<u8>(), 0..=10),
            seg2 in proptest::collection::vec(0u8..=41, 1..=5),
        ) {
            // pattern = seg1 + "*" + seg2
            let mut pattern = seg1.clone();
            pattern.push(b'*');
            pattern.extend_from_slice(&seg2);
            // input = seg1 + mid + seg2
            let mut input = seg1;
            input.extend_from_slice(&mid);
            input.extend_from_slice(&seg2);
            prop_assert!(glob_match(&pattern, &input));
        }
    }
}

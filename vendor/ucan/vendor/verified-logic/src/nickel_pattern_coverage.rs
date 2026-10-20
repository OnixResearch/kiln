//! Verified Nickel pattern coverage and overlap detection.
//!
//! Models pattern match completeness from `core/src/term/pattern/compile.rs`:
//! a set of patterns covers a type when every value matches at least one.
//! Proves wildcard completeness, enum coverage, and overlap detection.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

// --- Spec-level types ---

/// Spec pattern.
pub enum SpecPattern {
    Wildcard,
    EnumVariant(Seq<u8>),
}

/// Spec type descriptor.
pub enum SpecTypeDesc {
    Enum(Seq<Seq<u8>>),
    Other,
}

/// Spec: does a pattern match a given tag?
pub open spec fn pattern_matches_tag(p: SpecPattern, tag: Seq<u8>) -> bool {
    match p {
        SpecPattern::Wildcard => true,
        SpecPattern::EnumVariant(t) => t =~= tag,
    }
}

/// Spec: does any pattern in the set match a tag?
pub open spec fn any_matches_tag(patterns: Seq<SpecPattern>, tag: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < patterns.len() && pattern_matches_tag(patterns[i], tag)
}

/// Spec: does the pattern set contain a wildcard?
// r[depends nickel-pattern.wildcard-covers-all]
pub open spec fn has_wildcard(patterns: Seq<SpecPattern>) -> bool {
    exists|i: int| 0 <= i < patterns.len() && patterns[i] is Wildcard
}

/// Spec: is a pattern set complete for a type?
// r[depends nickel-pattern.coverage-completeness]
// r[depends nickel-pattern.enum-coverage]
pub open spec fn is_complete(patterns: Seq<SpecPattern>, ty: SpecTypeDesc) -> bool {
    match ty {
        SpecTypeDesc::Enum(variants) => all_variants_covered(patterns, variants),
        SpecTypeDesc::Other => has_wildcard(patterns),
    }
}

/// Spec: are all enum variants covered by at least one pattern?
pub open spec fn all_variants_covered(
    patterns: Seq<SpecPattern>, variants: Seq<Seq<u8>>,
) -> bool {
    forall|i: int| 0 <= i < variants.len()
        ==> any_matches_tag(patterns, variants[i])
}

/// Spec: do two patterns overlap (can match the same value)?
// r[depends nickel-pattern.overlap-detection]
pub open spec fn patterns_overlap(a: SpecPattern, b: SpecPattern) -> bool {
    match (a, b) {
        (SpecPattern::Wildcard, _) => true,
        (_, SpecPattern::Wildcard) => true,
        (SpecPattern::EnumVariant(t1), SpecPattern::EnumVariant(t2)) => t1 =~= t2,
    }
}

/// Spec: find uncovered variants.
pub open spec fn uncovered_variants_spec(
    patterns: Seq<SpecPattern>, variants: Seq<Seq<u8>>,
) -> Seq<Seq<u8>>
    decreases variants.len()
{
    if variants.len() == 0 {
        Seq::empty()
    } else {
        let rest = uncovered_variants_spec(patterns, variants.subrange(1, variants.len() as int));
        if any_matches_tag(patterns, variants[0int]) {
            rest
        } else {
            seq![variants[0int]] + rest
        }
    }
}

// --- Exec-level types ---

/// Exec pattern: uses Vec<u8> for runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Wildcard,
    EnumVariant(Vec<u8>),
}

/// Map exec pattern to spec.
pub open spec fn pattern_view(p: Pattern) -> SpecPattern {
    match p {
        Pattern::Wildcard => SpecPattern::Wildcard,
        Pattern::EnumVariant(v) => SpecPattern::EnumVariant(v@),
    }
}

/// Map exec pattern vec to spec.
pub open spec fn patterns_view(ps: Seq<Pattern>) -> Seq<SpecPattern> {
    Seq::new(ps.len(), |i: int| pattern_view(ps[i]))
}

/// Spec: view a Pattern by reference (avoids clone in ensures).
pub open spec fn pattern_ref_view(p: &Pattern) -> SpecPattern {
    match *p {
        Pattern::Wildcard => SpecPattern::Wildcard,
        Pattern::EnumVariant(ref v) => SpecPattern::EnumVariant(v@),
    }
}

/// Exec helper: check if pattern at index k matches tag.
fn pattern_matches_tag_exec(pat: &Pattern, tag: &Vec<u8>) -> (result: bool)
    ensures result == pattern_matches_tag(pattern_ref_view(pat), tag@)
{
    match pat {
        Pattern::Wildcard => true,
        Pattern::EnumVariant(t) => *t == *tag,
    }
}

/// Exec: check if a pattern set is complete for an enum type.
// r[impl nickel-pattern.coverage-completeness]
// r[impl nickel-pattern.enum-coverage]
pub fn check_enum_coverage(
    patterns: &Vec<Pattern>, variants: &Vec<Vec<u8>>,
) -> (result: bool)
    ensures result == all_variants_covered(
        patterns_view(patterns@),
        variants@.map_values(|v: Vec<u8>| v@),
    )
{
    let ghost sp = patterns_view(patterns@);
    let ghost sv = variants@.map_values(|v: Vec<u8>| v@);

    let mut i: usize = 0;
    while i < variants.len()
        invariant
            i <= variants.len(),
            sp =~= patterns_view(patterns@),
            sv =~= variants@.map_values(|v: Vec<u8>| v@),
            forall|j: int| 0 <= j < i as int
                ==> any_matches_tag(sp, sv[j]),
        decreases variants.len() - i,
    {
        let mut found = false;
        let mut k: usize = 0;
        while k < patterns.len()
            invariant
                k <= patterns.len(),
                i < variants.len(),
                sp =~= patterns_view(patterns@),
                sv =~= variants@.map_values(|v: Vec<u8>| v@),
                found ==> any_matches_tag(sp, sv[i as int]),
                !found ==> forall|m: int| 0 <= m < k as int
                    ==> !pattern_matches_tag(sp[m], sv[i as int]),
            decreases patterns.len() - k,
        {
            let matched = pattern_matches_tag_exec(&patterns[k], &variants[i]);
            proof {
                // Bridge: pattern_ref_view(&patterns[k]) == sp[k]
                assert(pattern_ref_view(&patterns@[k as int]) =~= sp[k as int]);
            }
            if matched {
                assert(pattern_matches_tag(sp[k as int], sv[i as int]));
                found = true;
            }
            k = k + 1;
        }
        if !found {
            assert(!any_matches_tag(sp, sv[i as int]));
            return false;
        }
        i = i + 1;
    }
    true
}

/// Exec: check if a pattern set contains a wildcard.
// r[impl nickel-pattern.wildcard-covers-all]
pub fn has_wildcard_exec(patterns: &Vec<Pattern>) -> (result: bool)
    ensures result == has_wildcard(patterns_view(patterns@))
{
    let ghost sp = patterns_view(patterns@);
    let mut i: usize = 0;
    while i < patterns.len()
        invariant
            i <= patterns.len(),
            sp =~= patterns_view(patterns@),
            forall|j: int| 0 <= j < i as int ==> !(sp[j] is Wildcard),
        decreases patterns.len() - i,
    {
        match &patterns[i] {
            Pattern::Wildcard => {
                assert(sp[i as int] is Wildcard);
                return true;
            }
            Pattern::EnumVariant(_) => {}
        }
        i = i + 1;
    }
    false
}

/// Exec: find uncovered enum variants.
// r[impl nickel-pattern.coverage-completeness]
pub fn find_uncovered(
    patterns: &Vec<Pattern>, variants: &Vec<Vec<u8>>,
) -> (result: Vec<Vec<u8>>)
    ensures result@.map_values(|v: Vec<u8>| v@) =~=
        uncovered_variants_spec(
            patterns_view(patterns@),
            variants@.map_values(|v: Vec<u8>| v@),
        )
{
    let ghost sp = patterns_view(patterns@);
    let ghost mv = variants@.map_values(|v: Vec<u8>| v@);

    let mut result: Vec<Vec<u8>> = Vec::new();
    let ghost mut rg: Seq<Seq<u8>> = Seq::empty();
    let mut i: usize = 0;

    proof {
        assert(mv.subrange(0, mv.len() as int) =~= mv);
    }

    while i < variants.len()
        invariant
            i <= variants.len(),
            sp =~= patterns_view(patterns@),
            mv =~= variants@.map_values(|v: Vec<u8>| v@),
            rg.len() == result@.len(),
            forall|k: int| 0 <= k < rg.len() ==> rg[k] =~= result@[k]@,
            uncovered_variants_spec(sp, mv) =~=
                rg + uncovered_variants_spec(sp, mv.subrange(i as int, mv.len() as int)),
        decreases variants.len() - i,
    {
        proof {
            let rem = mv.subrange(i as int, mv.len() as int);
            assert(rem.len() > 0);
            assert(rem[0int] =~= mv[i as int]);
            assert(rem.subrange(1, rem.len() as int) =~= mv.subrange(i as int + 1, mv.len() as int));
        }

        let mut found = false;
        let mut k: usize = 0;
        while k < patterns.len()
            invariant
                k <= patterns.len(),
                i < variants.len(),
                sp =~= patterns_view(patterns@),
                mv =~= variants@.map_values(|v: Vec<u8>| v@),
                found ==> any_matches_tag(sp, mv[i as int]),
                !found ==> forall|m: int| 0 <= m < k as int
                    ==> !pattern_matches_tag(sp[m], mv[i as int]),
            decreases patterns.len() - k,
        {
            let matched = pattern_matches_tag_exec(&patterns[k], &variants[i]);
            proof {
                assert(pattern_ref_view(&patterns@[k as int]) =~= sp[k as int]);
            }
            if matched {
                assert(pattern_matches_tag(sp[k as int], mv[i as int]));
                found = true;
            }
            k = k + 1;
        }

        if !found {
            result.push(variants[i].clone());
            proof { rg = rg + seq![mv[i as int]]; }
        }

        i = i + 1;
    }

    proof {
        assert(mv.subrange(i as int, mv.len() as int).len() == 0);
        assert(uncovered_variants_spec(sp, mv.subrange(i as int, mv.len() as int))
            =~= Seq::<Seq<u8>>::empty());
        assert(rg =~= result@.map_values(|v: Vec<u8>| v@));
    }
    result
}

/// Exec: detect overlap between two specific patterns.
// r[impl nickel-pattern.overlap-detection]
pub fn check_overlap(a: &Pattern, b: &Pattern) -> (result: bool)
    ensures result == patterns_overlap(pattern_ref_view(a), pattern_ref_view(b))
{
    match (a, b) {
        (Pattern::Wildcard, _) | (_, Pattern::Wildcard) => true,
        (Pattern::EnumVariant(t1), Pattern::EnumVariant(t2)) => *t1 == *t2,
    }
}

// --- Proofs ---

// r[verify nickel-pattern.wildcard-covers-all]
/// A wildcard matches any tag.
proof fn wildcard_matches_all(tag: Seq<u8>)
    ensures pattern_matches_tag(SpecPattern::Wildcard, tag)
{}

// r[verify nickel-pattern.wildcard-covers-all]
/// A pattern set containing a wildcard is always complete.
proof fn wildcard_set_complete(patterns: Seq<SpecPattern>, ty: SpecTypeDesc)
    requires has_wildcard(patterns)
    ensures is_complete(patterns, ty)
{
    let wi = choose|i: int| 0 <= i < patterns.len() && patterns[i] is Wildcard;
    match ty {
        SpecTypeDesc::Enum(variants) => {
            assert forall|j: int| 0 <= j < variants.len()
            implies any_matches_tag(patterns, variants[j]) by {
                assert(pattern_matches_tag(patterns[wi], variants[j]));
            }
        }
        SpecTypeDesc::Other => {}
    }
}

// r[verify nickel-pattern.enum-coverage]
/// Complete enum coverage: every variant has a matching pattern.
proof fn enum_coverage_complete(patterns: Seq<SpecPattern>, variants: Seq<Seq<u8>>)
    requires all_variants_covered(patterns, variants)
    ensures is_complete(patterns, SpecTypeDesc::Enum(variants))
{}

// r[verify nickel-pattern.overlap-detection]
/// Wildcard overlaps with everything.
proof fn wildcard_overlaps_all(p: SpecPattern)
    ensures patterns_overlap(SpecPattern::Wildcard, p)
{}

// r[verify nickel-pattern.enum-coverage]
/// Uncovered variants list is empty when all variants are covered.
proof fn uncovered_empty_when_covered(patterns: Seq<SpecPattern>, variants: Seq<Seq<u8>>)
    requires all_variants_covered(patterns, variants)
    ensures uncovered_variants_spec(patterns, variants).len() == 0
    decreases variants.len()
{
    if variants.len() > 0 {
        assert(any_matches_tag(patterns, variants[0int]));
        uncovered_empty_when_covered(
            patterns,
            variants.subrange(1, variants.len() as int),
        );
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    // r[verify nickel-pattern.wildcard-covers-all]
    #[test]
    fn test_wildcard_complete() {
        let patterns = vec![Pattern::Wildcard];
        let variants = vec![tag("a"), tag("b"), tag("c")];
        assert!(check_enum_coverage(&patterns, &variants));
    }

    // r[verify nickel-pattern.enum-coverage]
    #[test]
    fn test_complete_enum() {
        let patterns = vec![
            Pattern::EnumVariant(tag("a")),
            Pattern::EnumVariant(tag("b")),
            Pattern::EnumVariant(tag("c")),
        ];
        let variants = vec![tag("a"), tag("b"), tag("c")];
        assert!(check_enum_coverage(&patterns, &variants));
    }

    // r[verify nickel-pattern.enum-coverage]
    #[test]
    fn test_incomplete_enum() {
        let patterns = vec![Pattern::EnumVariant(tag("a")), Pattern::EnumVariant(tag("b"))];
        let variants = vec![tag("a"), tag("b"), tag("c")];
        assert!(!check_enum_coverage(&patterns, &variants));
    }

    // r[verify nickel-pattern.coverage-completeness]
    #[test]
    fn test_find_uncovered() {
        let patterns = vec![Pattern::EnumVariant(tag("a")), Pattern::EnumVariant(tag("b"))];
        let variants = vec![tag("a"), tag("b"), tag("c")];
        let uncovered = find_uncovered(&patterns, &variants);
        assert_eq!(uncovered.len(), 1);
        assert_eq!(uncovered[0], tag("c"));
    }

    // r[verify nickel-pattern.overlap-detection]
    #[test]
    fn test_overlap_detection() {
        assert!(check_overlap(&Pattern::Wildcard, &Pattern::EnumVariant(tag("a"))));
        assert!(check_overlap(&Pattern::EnumVariant(tag("a")), &Pattern::EnumVariant(tag("a")),));
        assert!(!check_overlap(&Pattern::EnumVariant(tag("a")), &Pattern::EnumVariant(tag("b")),));
    }

    // r[verify nickel-pattern.wildcard-covers-all]
    #[test]
    fn test_has_wildcard() {
        assert!(has_wildcard_exec(&vec![Pattern::Wildcard]));
        assert!(!has_wildcard_exec(&vec![Pattern::EnumVariant(tag("a"))]));
    }
}

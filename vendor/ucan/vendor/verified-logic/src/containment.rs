//! Verified delegation containment for capability authorization.
//!
//! A parent capability contains a child capability if the child's
//! access is equal or narrower on every axis: variant subsumption,
//! scope coverage, and mode ordering.

use vstd::prelude::*;

use crate::access_mode;
#[cfg(verus_keep_ghost)]
use crate::access_mode::*;
use crate::scope;
#[cfg(verus_keep_ghost)]
use crate::scope::*;
use crate::subsumption;
#[cfg(verus_keep_ghost)]
use crate::subsumption::*;

verus! {

// ============================================================
// Containment: subsumes && covers && mode_le
// ============================================================

// r[depends containment.decomposition]
/// Spec: does the parent capability contain the child capability?
///
/// Three-factor decomposition:
/// 1. Parent variant subsumes child variant.
/// 2. Parent scope covers child scope (child is narrower or equal).
/// 3. Child mode ≤ parent mode (child doesn't escalate).
pub open spec fn contains_spec(
    parent_variant: u8,
    parent_scope: Seq<u8>,
    parent_mode: u8,
    child_variant: u8,
    child_scope: Seq<u8>,
    child_mode: u8,
) -> bool {
    subsumes_spec(parent_variant, child_variant)
    && covers_spec(parent_scope, child_scope)
    && mode_le_spec(child_mode, parent_mode)
}

// r[impl containment.decomposition]
/// Returns true if the parent capability contains the child.
pub fn contains(
    parent_variant: u8,
    parent_scope: &[u8],
    parent_mode: u8,
    child_variant: u8,
    child_scope: &[u8],
    child_mode: u8,
) -> (result: bool)
    ensures result == contains_spec(
        parent_variant, parent_scope@, parent_mode,
        child_variant, child_scope@, child_mode,
    ),
{
    subsumption::subsumes(parent_variant, child_variant)
    && scope::covers(parent_scope, child_scope)
    && access_mode::mode_le(child_mode, parent_mode)
}

// ============================================================
// Factor failure proofs
// ============================================================

// r[impl containment.no-scope-widening]
// r[verify containment.no-scope-widening]
/// Scope widening denies containment.
proof fn no_scope_widening(
    pv: u8, ps: Seq<u8>, pm: u8,
    cv: u8, cs: Seq<u8>, cm: u8,
)
    requires !covers_spec(ps, cs)
    ensures !contains_spec(pv, ps, pm, cv, cs, cm)
{}

// r[impl containment.no-mode-escalation]
// r[verify containment.no-mode-escalation]
/// Mode escalation denies containment.
proof fn no_mode_escalation(
    pv: u8, ps: Seq<u8>, pm: u8,
    cv: u8, cs: Seq<u8>, cm: u8,
)
    requires !mode_le_spec(cm, pm)
    ensures !contains_spec(pv, ps, pm, cv, cs, cm)
{}

// r[impl containment.no-variant-escalation]
// r[verify containment.no-variant-escalation]
/// Variant escalation denies containment.
proof fn no_variant_escalation(
    pv: u8, ps: Seq<u8>, pm: u8,
    cv: u8, cs: Seq<u8>, cm: u8,
)
    requires !subsumes_spec(pv, cv)
    ensures !contains_spec(pv, ps, pm, cv, cs, cm)
{}

// r[impl containment.attenuation-allowed]
// r[verify containment.attenuation-allowed]
/// Attenuation is allowed: narrower scope + equal-or-lower mode is contained.
proof fn attenuation_allowed(
    pv: u8, ps: Seq<u8>, pm: u8,
    cv: u8, cs: Seq<u8>, cm: u8,
)
    requires
        subsumes_spec(pv, cv),
        covers_spec(ps, cs),
        mode_le_spec(cm, pm),
    ensures contains_spec(pv, ps, pm, cv, cs, cm)
{}

// r[impl containment.transitive]
// r[verify containment.transitive]
/// Containment is transitive: if a contains b and b contains c, then a contains c.
proof fn contains_transitive(
    av: u8, a_scope: Seq<u8>, am: u8,
    bv: u8, b_scope: Seq<u8>, bm: u8,
    cv_val: u8, c_scope: Seq<u8>, cm: u8,
)
    requires
        contains_spec(av, a_scope, am, bv, b_scope, bm),
        contains_spec(bv, b_scope, bm, cv_val, c_scope, cm),
    ensures contains_spec(av, a_scope, am, cv_val, c_scope, cm)
{
    // Subsumption transitive: subsumes(a,b) && subsumes(b,c) ==> subsumes(a,c)
    subsumes_transitive(av, bv, cv_val);
    // Scope coverage transitive: covers(a,b) && covers(b,c) ==> covers(a,c)
    covers_transitive(a_scope, b_scope, c_scope);
    // Mode ordering transitive: le(c,b) && le(b,a) ==> le(c,a)
    mode_le_transitive(cm, bm, am);
}

// r[impl containment.self-contained]
// r[verify containment.self-contained]
/// Every capability contains itself.
proof fn contains_reflexive(v: u8, s: Seq<u8>, m: u8)
    ensures contains_spec(v, s, m, v, s, m)
{}

// ============================================================
// Compound scope containment
// ============================================================

// r[depends containment.compound-scope]
/// Spec: containment with compound scope.
pub open spec fn contains_compound_spec(
    pv: u8, p_mount: Seq<u8>, p_prefix: Seq<u8>, pm: u8,
    cv: u8, c_mount: Seq<u8>, c_prefix: Seq<u8>, cm: u8,
) -> bool {
    subsumes_spec(pv, cv)
    && compound_covers_spec(p_mount, p_prefix, c_mount, c_prefix)
    && mode_le_spec(cm, pm)
}

// r[impl containment.compound-scope]
/// Returns true if parent contains child with compound scope.
pub fn contains_compound(
    pv: u8, p_mount: &[u8], p_prefix: &[u8], pm: u8,
    cv: u8, c_mount: &[u8], c_prefix: &[u8], cm: u8,
) -> (result: bool)
    ensures result == contains_compound_spec(
        pv, p_mount@, p_prefix@, pm,
        cv, c_mount@, c_prefix@, cm,
    ),
{
    subsumption::subsumes(pv, cv)
    && scope::compound_covers(p_mount, p_prefix, c_mount, c_prefix)
    && access_mode::mode_le(cm, pm)
}

// r[verify containment.compound-scope]
/// Compound containment: mount mismatch denies.
proof fn compound_mount_mismatch_denies(
    pv: u8, pm: Seq<u8>, pp: Seq<u8>, pmode: u8,
    cv: u8, cm: Seq<u8>, cp: Seq<u8>, cmode: u8,
)
    requires !(pm =~= cm)
    ensures !contains_compound_spec(pv, pm, pp, pmode, cv, cm, cp, cmode)
{}

// ============================================================
// Optional constraint containment
// ============================================================

// r[depends containment.optional-constraint]
/// Spec: containment with optional constraint.
pub open spec fn contains_with_optional_spec(
    pv: u8, ps: Seq<u8>, pm: u8, p_opt: Option<Seq<u8>>,
    cv: u8, cs: Seq<u8>, cm: u8, c_opt: Option<Seq<u8>>,
) -> bool {
    subsumes_spec(pv, cv)
    && covers_spec(ps, cs)
    && mode_le_spec(cm, pm)
    && optional_covers_spec(p_opt, c_opt)
}

// r[impl containment.optional-constraint]
/// Returns true if parent contains child with optional constraint.
pub fn contains_with_optional(
    pv: u8, ps: &[u8], pm: u8, p_opt: Option<&[u8]>,
    cv: u8, cs: &[u8], cm: u8, c_opt: Option<&[u8]>,
) -> (result: bool)
    ensures result == contains_with_optional_spec(
        pv, ps@, pm,
        match p_opt { None => None, Some(s) => Some(s@) },
        cv, cs@, cm,
        match c_opt { None => None, Some(s) => Some(s@) },
    ),
{
    subsumption::subsumes(pv, cv)
    && scope::covers(ps, cs)
    && access_mode::mode_le(cm, pm)
    && scope::optional_covers(p_opt, c_opt)
}

// r[verify containment.optional-constraint]
/// Optional containment: parent None contains any child constraint.
proof fn optional_parent_none_contains(
    pv: u8, ps: Seq<u8>, pm: u8,
    cv: u8, cs: Seq<u8>, cm: u8, c_opt: Option<Seq<u8>>,
)
    requires
        subsumes_spec(pv, cv),
        covers_spec(ps, cs),
        mode_le_spec(cm, pm),
    ensures contains_with_optional_spec(pv, ps, pm, None, cv, cs, cm, c_opt)
{}

// r[verify containment.optional-constraint]
/// Optional containment: parent Some blocks child None.
proof fn optional_parent_some_blocks_none(
    pv: u8, ps: Seq<u8>, pm: u8, p_wd: Seq<u8>,
    cv: u8, cs: Seq<u8>, cm: u8,
)
    ensures !contains_with_optional_spec(
        pv, ps, pm, Some(p_wd),
        cv, cs, cm, None::<Seq<u8>>,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_self_contained() {
        assert!(contains(0, b"/data", 0, 0, b"/data", 0));
        assert!(contains(4, b"", 3, 4, b"", 3));
    }

    #[test]
    fn test_attenuation() {
        // Full+Full scope → Read+narrower scope
        assert!(contains(4, b"/data/", 3, 0, b"/data/sub/", 0));
    }

    #[test]
    fn test_scope_widening_denied() {
        assert!(!contains(0, b"/data/sub/", 0, 0, b"/data/", 0));
    }

    #[test]
    fn test_mode_escalation_denied() {
        assert!(!contains(0, b"", 0, 0, b"", 3));
    }

    #[test]
    fn test_variant_escalation_denied() {
        // Read parent, Full child
        assert!(!contains(0, b"", 3, 4, b"", 3));
    }

    #[test]
    fn test_compound_mount_mismatch() {
        assert!(!contains_compound(4, b"secret/", b"", 3, 0, b"kv/", b"", 0));
    }

    #[test]
    fn test_compound_narrowing() {
        assert!(contains_compound(4, b"secret/", b"app/", 3, 0, b"secret/", b"app/sub/", 0));
    }

    #[test]
    fn test_optional_parent_none() {
        assert!(contains_with_optional(4, b"", 3, None, 0, b"key", 0, Some(b"/wd")));
    }

    #[test]
    fn test_optional_parent_some_child_none() {
        assert!(!contains_with_optional(4, b"", 3, Some(b"/wd"), 0, b"key", 0, None));
    }

    proptest! {
        // r[verify containment.self-contained]
        #[test]
        fn prop_self_contained(v in 0u8..=7, m in 0u8..=3,
            scope in proptest::collection::vec(any::<u8>(), 0..=20),
        ) {
            prop_assert!(contains(v, &scope, m, v, &scope, m));
        }

        // r[verify containment.transitive]
        #[test]
        fn prop_transitive(
            scope_a in proptest::collection::vec(any::<u8>(), 0..=10),
            ext_b in proptest::collection::vec(any::<u8>(), 0..=5),
            ext_c in proptest::collection::vec(any::<u8>(), 0..=5),
        ) {
            let mut scope_b = scope_a.clone();
            scope_b.extend_from_slice(&ext_b);
            let mut scope_c = scope_b.clone();
            scope_c.extend_from_slice(&ext_c);
            // Full+Full contains Full+narrower contains Full+narrowest
            prop_assert!(contains(4, &scope_a, 3, 4, &scope_b, 3));
            prop_assert!(contains(4, &scope_b, 3, 4, &scope_c, 3));
            prop_assert!(contains(4, &scope_a, 3, 4, &scope_c, 3));
        }

        // r[verify containment.no-mode-escalation]
        #[test]
        fn prop_no_mode_escalation(
            parent_mode in 0u8..=2,
            child_mode in 0u8..=2,
        ) {
            if parent_mode != child_mode {
                // leaf modes don't permit escalation
                prop_assert!(!contains(0, b"", parent_mode, 0, b"", child_mode));
            }
        }

        // r[verify containment.decomposition]
        #[test]
        fn prop_decomposition(
            pv in 0u8..=7, cv in 0u8..=7,
            pm in 0u8..=3, cm in 0u8..=3,
            ps in proptest::collection::vec(any::<u8>(), 0..=10),
            cs in proptest::collection::vec(any::<u8>(), 0..=10),
        ) {
            let cont = contains(pv, &ps, pm, cv, &cs, cm);
            let sub = crate::subsumption::subsumes(pv, cv);
            let cov = crate::scope::covers(&ps, &cs);
            let ord = crate::access_mode::mode_le(cm, pm);
            prop_assert_eq!(cont, sub && cov && ord);
        }
    }
}

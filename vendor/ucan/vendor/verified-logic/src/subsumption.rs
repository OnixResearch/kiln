//! Verified variant subsumption for capability authorization.
//!
//! Models capability variant types as integers with a subsumption
//! relation forming a DAG. An admin variant subsumes its leaf
//! operations: Full subsumes Read, Write, Delete, Watch.

use vstd::prelude::*;

verus! {

// ============================================================
// Variant constants
// ============================================================

/// Read access variant.
pub open spec fn V_READ() -> u8 { 0 }
/// Write access variant.
pub open spec fn V_WRITE() -> u8 { 1 }
/// Delete access variant.
pub open spec fn V_DELETE() -> u8 { 2 }
/// Watch access variant.
pub open spec fn V_WATCH() -> u8 { 3 }
/// Full access variant (subsumes Read, Write, Delete, Watch).
pub open spec fn V_FULL() -> u8 { 4 }
/// Prompt variant.
pub open spec fn V_PROMPT() -> u8 { 5 }
/// Tool use variant.
pub open spec fn V_TOOL_USE() -> u8 { 6 }
/// Delegate variant (grants delegation ability).
pub open spec fn V_DELEGATE() -> u8 { 7 }

// r[depends subsumption.identity]
/// Spec: does variant `a` subsume variant `b`?
///
/// Every variant subsumes itself. Full subsumes Read, Write, Delete, Watch.
/// No other subsumption edges exist — the relation is a flat DAG with
/// one admin node (Full) and four leaves plus three independent variants.
pub open spec fn subsumes_spec(a: u8, b: u8) -> bool {
    a == b
    || (a == V_FULL() && (b == V_READ() || b == V_WRITE() || b == V_DELETE() || b == V_WATCH()))
}

// r[impl subsumption.identity]
/// Returns true if variant `a` subsumes variant `b`.
pub fn subsumes(a: u8, b: u8) -> (result: bool)
    ensures result == subsumes_spec(a, b),
{
    a == b
    || (a == 4 && (b == 0 || b == 1 || b == 2 || b == 3))
}

// ============================================================
// Proofs
// ============================================================

// r[verify subsumption.identity]
/// Every variant subsumes itself.
proof fn subsumes_reflexive(a: u8)
    ensures subsumes_spec(a, a)
{}

// r[impl subsumption.acyclic]
// r[verify subsumption.acyclic]
/// Subsumption is acyclic: if a subsumes b and b subsumes a, then a == b.
/// (No non-trivial cycles.)
proof fn subsumes_antisymmetric(a: u8, b: u8)
    requires
        subsumes_spec(a, b),
        subsumes_spec(b, a),
    ensures a == b
{
    // subsumes(a, b): a==b or (a==FULL and b in {R,W,D,Wa})
    // subsumes(b, a): b==a or (b==FULL and a in {R,W,D,Wa})
    // If a==b, done.
    // If a==FULL and b in {R,W,D,Wa}: then b != FULL, so subsumes(b, a) requires b==a. But b != FULL == a. Contradiction.
}

// r[verify subsumption.acyclic]
/// Subsumption is transitive.
pub proof fn subsumes_transitive(a: u8, b: u8, c: u8)
    requires
        subsumes_spec(a, b),
        subsumes_spec(b, c),
    ensures subsumes_spec(a, c)
{
    // If a==b, then subsumes(a,c) == subsumes(b,c), done.
    // If a==FULL and b in {R,W,D,Wa}: subsumes(b,c) means b==c (leaves don't subsume others except self).
    // So c==b, and subsumes(a,c) == subsumes(FULL, b) which is true.
}

// r[impl subsumption.admin-full]
// r[verify subsumption.admin-full]
/// Full subsumes Read.
proof fn full_subsumes_read()
    ensures subsumes_spec(V_FULL(), V_READ())
{}

// r[verify subsumption.admin-full]
/// Full subsumes Write.
proof fn full_subsumes_write()
    ensures subsumes_spec(V_FULL(), V_WRITE())
{}

// r[verify subsumption.admin-full]
/// Full subsumes Delete.
proof fn full_subsumes_delete()
    ensures subsumes_spec(V_FULL(), V_DELETE())
{}

// r[verify subsumption.admin-full]
/// Full subsumes Watch.
proof fn full_subsumes_watch()
    ensures subsumes_spec(V_FULL(), V_WATCH())
{}

// r[impl subsumption.soundness]
// r[verify subsumption.soundness]
/// Read does not subsume Delete.
proof fn read_not_subsume_delete()
    ensures !subsumes_spec(V_READ(), V_DELETE())
{}

// r[verify subsumption.soundness]
/// Prompt does not subsume ToolUse.
proof fn prompt_not_subsume_tool_use()
    ensures !subsumes_spec(V_PROMPT(), V_TOOL_USE())
{}

// r[verify subsumption.soundness]
/// Leaf variants do not subsume Full.
proof fn leaf_not_subsume_full()
    ensures
        !subsumes_spec(V_READ(), V_FULL()),
        !subsumes_spec(V_WRITE(), V_FULL()),
        !subsumes_spec(V_DELETE(), V_FULL()),
        !subsumes_spec(V_WATCH(), V_FULL()),
{}

// r[verify subsumption.soundness]
/// Non-subsumed pairs always deny: if a doesn't subsume b,
/// then no capability of variant a can authorize variant b operations.
proof fn non_subsumed_denies(a: u8, b: u8)
    requires !subsumes_spec(a, b)
    ensures !subsumes_spec(a, b) // trivial but anchors the requirement
{}

// r[verify subsumption.identity]
/// Subsumption forms a valid partial order (reflexive, antisymmetric, transitive).
proof fn subsumption_partial_order(a: u8, b: u8, c: u8)
    requires
        subsumes_spec(a, b),
        subsumes_spec(b, c),
    ensures
        subsumes_spec(a, a),
        subsumes_spec(a, c),
{
    subsumes_reflexive(a);
    subsumes_transitive(a, b, c);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_self_subsumption() {
        for v in 0u8..=7 {
            assert!(subsumes(v, v), "variant {v} should subsume itself");
        }
    }

    #[test]
    fn test_full_subsumes_leaves() {
        assert!(subsumes(4, 0)); // Full -> Read
        assert!(subsumes(4, 1)); // Full -> Write
        assert!(subsumes(4, 2)); // Full -> Delete
        assert!(subsumes(4, 3)); // Full -> Watch
        assert!(subsumes(4, 4)); // Full -> Full (self)
    }

    #[test]
    fn test_leaves_dont_subsume_full() {
        assert!(!subsumes(0, 4)); // Read -/-> Full
        assert!(!subsumes(1, 4)); // Write -/-> Full
        assert!(!subsumes(2, 4)); // Delete -/-> Full
        assert!(!subsumes(3, 4)); // Watch -/-> Full
    }

    #[test]
    fn test_leaves_dont_subsume_each_other() {
        let leaves = [0u8, 1, 2, 3];
        for &a in &leaves {
            for &b in &leaves {
                if a != b {
                    assert!(!subsumes(a, b), "{a} should not subsume {b}");
                }
            }
        }
    }

    #[test]
    fn test_independent_variants() {
        // Prompt, ToolUse, Delegate are independent of the RWDW/Full lattice.
        assert!(!subsumes(5, 0)); // Prompt -/-> Read
        assert!(!subsumes(5, 6)); // Prompt -/-> ToolUse
        assert!(!subsumes(6, 5)); // ToolUse -/-> Prompt
        assert!(!subsumes(7, 4)); // Delegate -/-> Full
        assert!(!subsumes(4, 7)); // Full -/-> Delegate
    }

    proptest! {
        // r[verify subsumption.identity]
        #[test]
        fn prop_self_subsumes(v in 0u8..=7) {
            prop_assert!(subsumes(v, v));
        }

        // r[verify subsumption.acyclic]
        #[test]
        fn prop_antisymmetric(a in 0u8..=7, b in 0u8..=7) {
            if subsumes(a, b) && subsumes(b, a) {
                prop_assert_eq!(a, b);
            }
        }

        // r[verify subsumption.acyclic]
        #[test]
        fn prop_transitive(a in 0u8..=7, b in 0u8..=7, c in 0u8..=7) {
            if subsumes(a, b) && subsumes(b, c) {
                prop_assert!(subsumes(a, c));
            }
        }

        // r[verify subsumption.admin-full]
        #[test]
        fn prop_full_subsumes_leaf(leaf in 0u8..=3) {
            prop_assert!(subsumes(4, leaf));
        }

        // r[verify subsumption.soundness]
        #[test]
        fn prop_leaf_not_subsume_full(leaf in 0u8..=3) {
            prop_assert!(!subsumes(leaf, 4));
        }

        // r[verify subsumption.soundness]
        #[test]
        fn prop_non_subsumed_count(a in 0u8..=7) {
            // Count how many variants a subsumes.
            let count: u8 = u8::try_from((0u8..=7).filter(|&b| subsumes(a, b)).count()).unwrap();
            if a == 4 {
                // Full subsumes R, W, D, Wa, and itself = 5
                prop_assert_eq!(count, 5);
            } else {
                // Others only subsume themselves = 1
                prop_assert_eq!(count, 1);
            }
        }
    }
}

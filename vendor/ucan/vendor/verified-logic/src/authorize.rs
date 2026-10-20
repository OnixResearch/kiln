//! Verified authorization composition for capability systems.
//!
//! Authorization decomposes into three factors: variant subsumption,
//! scope coverage, and access mode permission. Each factor is
//! verified independently; this module composes them.

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
// Authorization: subsumes && covers && mode_permits
// ============================================================

// r[depends authorize.decomposition]
/// Spec: does the capability authorize the operation?
///
/// Three-factor decomposition:
/// 1. Variant subsumption: cap variant subsumes op variant.
/// 2. Scope coverage: cap scope covers op target.
/// 3. Mode permission: cap mode permits op mode.
pub open spec fn authorizes_spec(
    cap_variant: u8,
    cap_scope: Seq<u8>,
    cap_mode: u8,
    op_variant: u8,
    op_target: Seq<u8>,
    op_mode: u8,
) -> bool {
    subsumes_spec(cap_variant, op_variant)
    && covers_spec(cap_scope, op_target)
    && mode_permits_spec(cap_mode, op_mode)
}

// r[impl authorize.decomposition]
/// Returns true if the capability authorizes the operation.
pub fn authorizes(
    cap_variant: u8,
    cap_scope: &[u8],
    cap_mode: u8,
    op_variant: u8,
    op_target: &[u8],
    op_mode: u8,
) -> (result: bool)
    ensures result == authorizes_spec(
        cap_variant, cap_scope@, cap_mode,
        op_variant, op_target@, op_mode,
    ),
{
    subsumption::subsumes(cap_variant, op_variant)
    && scope::covers(cap_scope, op_target)
    && access_mode::mode_permits(cap_mode, op_mode)
}

// ============================================================
// Factor failure proofs
// ============================================================

// r[impl authorize.rejects-scope-failure]
// r[verify authorize.rejects-scope-failure]
/// Scope mismatch denies even with matching variant and mode.
proof fn scope_failure_denies(
    cap_variant: u8, cap_scope: Seq<u8>, cap_mode: u8,
    op_variant: u8, op_target: Seq<u8>, op_mode: u8,
)
    requires
        subsumes_spec(cap_variant, op_variant),
        mode_permits_spec(cap_mode, op_mode),
        !covers_spec(cap_scope, op_target),
    ensures !authorizes_spec(cap_variant, cap_scope, cap_mode, op_variant, op_target, op_mode)
{}

// r[impl authorize.rejects-mode-failure]
// r[verify authorize.rejects-mode-failure]
/// Mode mismatch denies even with matching variant and scope.
proof fn mode_failure_denies(
    cap_variant: u8, cap_scope: Seq<u8>, cap_mode: u8,
    op_variant: u8, op_target: Seq<u8>, op_mode: u8,
)
    requires
        subsumes_spec(cap_variant, op_variant),
        covers_spec(cap_scope, op_target),
        !mode_permits_spec(cap_mode, op_mode),
    ensures !authorizes_spec(cap_variant, cap_scope, cap_mode, op_variant, op_target, op_mode)
{}

// r[impl authorize.rejects-variant-failure]
// r[verify authorize.rejects-variant-failure]
/// Variant mismatch denies even with matching scope and mode.
proof fn variant_failure_denies(
    cap_variant: u8, cap_scope: Seq<u8>, cap_mode: u8,
    op_variant: u8, op_target: Seq<u8>, op_mode: u8,
)
    requires
        covers_spec(cap_scope, op_target),
        mode_permits_spec(cap_mode, op_mode),
        !subsumes_spec(cap_variant, op_variant),
    ensures !authorizes_spec(cap_variant, cap_scope, cap_mode, op_variant, op_target, op_mode)
{}

// r[impl authorize.all-match]
// r[verify authorize.all-match]
/// All three factors match implies authorized.
proof fn all_match_authorizes(
    cap_variant: u8, cap_scope: Seq<u8>, cap_mode: u8,
    op_variant: u8, op_target: Seq<u8>, op_mode: u8,
)
    requires
        subsumes_spec(cap_variant, op_variant),
        covers_spec(cap_scope, op_target),
        mode_permits_spec(cap_mode, op_mode),
    ensures authorizes_spec(cap_variant, cap_scope, cap_mode, op_variant, op_target, op_mode)
{}

// ============================================================
// Compound and optional scope composition
// ============================================================

// r[depends authorize.compound-scope]
/// Spec: authorization with compound scope (mount + prefix).
pub open spec fn authorizes_compound_spec(
    cap_variant: u8,
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    cap_mode: u8,
    op_variant: u8,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
    op_mode: u8,
) -> bool {
    subsumes_spec(cap_variant, op_variant)
    && compound_covers_spec(cap_mount, cap_prefix, op_mount, op_path)
    && mode_permits_spec(cap_mode, op_mode)
}

// r[impl authorize.compound-scope]
/// Returns true if the capability authorizes with compound scope.
pub fn authorizes_compound(
    cap_variant: u8,
    cap_mount: &[u8],
    cap_prefix: &[u8],
    cap_mode: u8,
    op_variant: u8,
    op_mount: &[u8],
    op_path: &[u8],
    op_mode: u8,
) -> (result: bool)
    ensures result == authorizes_compound_spec(
        cap_variant, cap_mount@, cap_prefix@, cap_mode,
        op_variant, op_mount@, op_path@, op_mode,
    ),
{
    subsumption::subsumes(cap_variant, op_variant)
    && scope::compound_covers(cap_mount, cap_prefix, op_mount, op_path)
    && access_mode::mode_permits(cap_mode, op_mode)
}

// r[verify authorize.compound-scope]
/// Compound scope: mount mismatch denies.
proof fn compound_mount_mismatch_denies(
    cap_variant: u8, cap_mount: Seq<u8>, cap_prefix: Seq<u8>, cap_mode: u8,
    op_variant: u8, op_mount: Seq<u8>, op_path: Seq<u8>, op_mode: u8,
)
    requires !(cap_mount =~= op_mount)
    ensures !authorizes_compound_spec(
        cap_variant, cap_mount, cap_prefix, cap_mode,
        op_variant, op_mount, op_path, op_mode,
    )
{}

// r[depends authorize.optional-constraint]
/// Spec: authorization with optional constraint field.
pub open spec fn authorizes_with_optional_spec(
    cap_variant: u8,
    cap_scope: Seq<u8>,
    cap_mode: u8,
    cap_constraint: Option<Seq<u8>>,
    op_variant: u8,
    op_target: Seq<u8>,
    op_mode: u8,
    op_constraint: Option<Seq<u8>>,
) -> bool {
    subsumes_spec(cap_variant, op_variant)
    && covers_spec(cap_scope, op_target)
    && mode_permits_spec(cap_mode, op_mode)
    && optional_covers_spec(cap_constraint, op_constraint)
}

// r[impl authorize.optional-constraint]
/// Returns true if the capability authorizes with optional constraint.
pub fn authorizes_with_optional(
    cap_variant: u8,
    cap_scope: &[u8],
    cap_mode: u8,
    cap_constraint: Option<&[u8]>,
    op_variant: u8,
    op_target: &[u8],
    op_mode: u8,
    op_constraint: Option<&[u8]>,
) -> (result: bool)
    ensures result == authorizes_with_optional_spec(
        cap_variant, cap_scope@, cap_mode,
        match cap_constraint { None => None, Some(s) => Some(s@) },
        op_variant, op_target@, op_mode,
        match op_constraint { None => None, Some(s) => Some(s@) },
    ),
{
    subsumption::subsumes(cap_variant, op_variant)
    && scope::covers(cap_scope, op_target)
    && access_mode::mode_permits(cap_mode, op_mode)
    && scope::optional_covers(cap_constraint, op_constraint)
}

// r[verify authorize.optional-constraint]
/// Optional constraint None never blocks authorization.
proof fn optional_none_doesnt_block(
    cap_variant: u8, cap_scope: Seq<u8>, cap_mode: u8,
    op_variant: u8, op_target: Seq<u8>, op_mode: u8,
    op_constraint: Option<Seq<u8>>,
)
    requires
        subsumes_spec(cap_variant, op_variant),
        covers_spec(cap_scope, op_target),
        mode_permits_spec(cap_mode, op_mode),
    ensures authorizes_with_optional_spec(
        cap_variant, cap_scope, cap_mode, None,
        op_variant, op_target, op_mode, op_constraint,
    )
{}

// r[verify authorize.optional-constraint]
/// Optional constraint Some blocks when target is None.
proof fn optional_some_blocks_none(
    cap_variant: u8, cap_scope: Seq<u8>, cap_mode: u8,
    cap_wd: Seq<u8>,
    op_variant: u8, op_target: Seq<u8>, op_mode: u8,
)
    ensures !authorizes_with_optional_spec(
        cap_variant, cap_scope, cap_mode, Some(cap_wd),
        op_variant, op_target, op_mode, None::<Seq<u8>>,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify authorize.all-match]
    #[test]
    fn test_full_read_within_scope() {
        // Full cap with scope "/data/", Read op for "/data/key"
        assert!(authorizes(4, b"/data/", 3, 0, b"/data/key", 0));
    }

    // r[verify authorize.rejects-scope-failure]
    #[test]
    fn test_scope_mismatch_denies() {
        // Full cap with scope "/data/", Read op for "/etc/key"
        assert!(!authorizes(4, b"/data/", 3, 0, b"/etc/key", 0));
    }

    // r[verify authorize.rejects-mode-failure]
    #[test]
    fn test_mode_mismatch_denies() {
        // Read cap with scope "/data/", Write op for "/data/key"
        assert!(!authorizes(0, b"/data/", 0, 0, b"/data/key", 1));
    }

    // r[verify authorize.rejects-variant-failure]
    #[test]
    fn test_variant_mismatch_denies() {
        // Prompt cap, ToolUse op
        assert!(!authorizes(5, b"", 3, 6, b"", 3));
    }

    // r[verify authorize.all-match]
    #[test]
    fn test_self_variant_authorizes() {
        // Read cap, Read op, same scope, same mode
        assert!(authorizes(0, b"/x", 0, 0, b"/x/y", 0));
    }

    // r[verify authorize.compound-scope]
    #[test]
    fn test_compound_mount_mismatch() {
        assert!(!authorizes_compound(4, b"secret/", b"", 3, 0, b"kv/", b"key", 0));
    }

    // r[verify authorize.compound-scope]
    #[test]
    fn test_compound_both_match() {
        assert!(authorizes_compound(4, b"secret/", b"app/", 3, 0, b"secret/", b"app/db", 0));
    }

    // r[verify authorize.optional-constraint]
    #[test]
    fn test_optional_none_passes() {
        assert!(authorizes_with_optional(4, b"", 3, None, 0, b"key", 0, Some(b"wd")));
    }

    // r[verify authorize.optional-constraint]
    #[test]
    fn test_optional_some_none_blocks() {
        assert!(!authorizes_with_optional(4, b"", 3, Some(b"/home"), 0, b"key", 0, None));
    }

    // r[verify authorize.optional-constraint]
    #[test]
    fn test_optional_some_some_covers() {
        assert!(authorizes_with_optional(4, b"", 3, Some(b"/home"), 0, b"key", 0, Some(b"/home/sub"),));
    }

    proptest! {
        // r[verify authorize.all-match]
        #[test]
        fn prop_self_authorizes(
            scope in proptest::collection::vec(any::<u8>(), 0..=20),
            target_ext in proptest::collection::vec(any::<u8>(), 0..=10),
            variant in 0u8..=7,
            mode in 0u8..=3,
        ) {
            let mut target = scope.clone();
            target.extend_from_slice(&target_ext);
            // same variant, same mode, scope covers target
            prop_assert!(authorizes(variant, &scope, mode, variant, &target, mode));
        }

        // r[verify authorize.rejects-variant-failure]
        #[test]
        fn prop_variant_mismatch_denies(
            a in 0u8..=3,
            b in 0u8..=3,
        ) {
            // leaf variants don't subsume each other
            if a != b {
                prop_assert!(!authorizes(a, b"", 3, b, b"", 3));
            }
        }

        // r[verify authorize.rejects-mode-failure]
        #[test]
        fn prop_mode_mismatch_denies(
            cap_mode in 0u8..=2,
            op_mode in 0u8..=2,
        ) {
            // leaf modes don't permit each other
            if cap_mode != op_mode {
                prop_assert!(!authorizes(0, b"", cap_mode, 0, b"", op_mode));
            }
        }

        // r[verify authorize.decomposition]
        #[test]
        fn prop_decomposition_matches(
            cv in 0u8..=7, ov in 0u8..=7,
            cm in 0u8..=3, om in 0u8..=3,
            scope in proptest::collection::vec(any::<u8>(), 0..=10),
            target in proptest::collection::vec(any::<u8>(), 0..=10),
        ) {
            let auth = authorizes(cv, &scope, cm, ov, &target, om);
            let sub = crate::subsumption::subsumes(cv, ov);
            let cov = crate::scope::covers(&scope, &target);
            let perm = crate::access_mode::mode_permits(cm, om);
            prop_assert_eq!(auth, sub && cov && perm);
        }
    }
}

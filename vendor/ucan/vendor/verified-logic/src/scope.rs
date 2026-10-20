//! Verified scope coverage for capability authorization.
//!
//! Prefix-based scope coverage, compound scoping (mount + prefix),
//! and optional constraint combinators. These are the foundational
//! predicates for UCAN-style authorization and delegation containment.

use vstd::prelude::*;

verus! {

// ============================================================
// Starts-with predicate (byte sequences)
// ============================================================

// r[depends scope.coverage-reflexive]
/// Spec: does `s` start with `prefix`?
pub open spec fn starts_with_bytes_spec(s: Seq<u8>, prefix: Seq<u8>) -> bool {
    s.len() >= prefix.len()
    && forall|i: int| #![trigger prefix[i]]
        0 <= i < prefix.len() as int
        ==> s[i] == prefix[i]
}

// r[impl scope.coverage-reflexive]
/// Checks if a byte slice starts with a given prefix.
pub fn starts_with_bytes(s: &[u8], prefix: &[u8]) -> (result: bool)
    ensures result == starts_with_bytes_spec(s@, prefix@),
{
    if s.len() < prefix.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < prefix.len()
        invariant
            i <= prefix.len(),
            prefix.len() <= s.len(),
            forall|j: int| #![trigger prefix@[j]]
                0 <= j < i as int ==> s@[j] == prefix@[j],
        decreases prefix.len() - i,
    {
        if s[i] != prefix[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

// ============================================================
// Scope coverage: covers(scope, target) = starts_with(target, scope)
// ============================================================

// r[depends scope.coverage-reflexive]
/// Spec: does the target fall within the scope?
/// A scope covers a target if the target starts with the scope prefix.
pub open spec fn covers_spec(scope: Seq<u8>, target: Seq<u8>) -> bool {
    starts_with_bytes_spec(target, scope)
}

// r[impl scope.coverage-reflexive]
/// Returns true if the target falls within the scope (target starts with scope).
pub fn covers(scope: &[u8], target: &[u8]) -> (result: bool)
    ensures result == covers_spec(scope@, target@),
{
    starts_with_bytes(target, scope)
}

// r[verify scope.coverage-reflexive]
/// A scope covers itself.
proof fn covers_reflexive(s: Seq<u8>)
    ensures covers_spec(s, s)
{}

// r[impl scope.coverage-transitive]
// r[verify scope.coverage-transitive]
/// If a covers b and b covers c, then a covers c.
pub proof fn covers_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
    requires
        covers_spec(a, b),
        covers_spec(b, c),
    ensures covers_spec(a, c)
{
    // a covers b means b starts with a: b.len >= a.len and b[i] == a[i] for i < a.len
    // b covers c means c starts with b: c.len >= b.len and c[i] == b[i] for i < b.len
    // Need: c.len >= a.len and c[i] == a[i] for i < a.len
    // c.len >= b.len >= a.len, and for i < a.len: c[i] == b[i] (since i < a.len <= b.len) == a[i]
    assert(c.len() >= a.len());
    assert forall|i: int| #![trigger a[i]]
        0 <= i < a.len() as int
    implies
        c[i] == a[i]
    by {
        // i < a.len <= b.len, so b[i] == a[i] (from covers(a, b))
        // i < a.len <= b.len <= c.len, so c[i] == b[i] (from covers(b, c))
        assert(a[i] == b[i]);  // from starts_with(b, a)
        assert(b[i] == c[i]);  // from starts_with(c, b)
    }
}

// r[impl scope.coverage-rejects-non-prefix]
// r[verify scope.coverage-rejects-non-prefix]
/// A scope rejects targets that don't start with it.
proof fn covers_rejects_non_prefix(scope: Seq<u8>, target: Seq<u8>)
    requires !starts_with_bytes_spec(target, scope)
    ensures !covers_spec(scope, target)
{}

// r[impl scope.coverage-narrowing-monotone]
// r[verify scope.coverage-narrowing-monotone]
/// If parent covers child, then everything child covers is also covered by parent.
proof fn covers_narrowing_monotone(parent: Seq<u8>, child: Seq<u8>, target: Seq<u8>)
    requires
        covers_spec(parent, child),
        covers_spec(child, target),
    ensures covers_spec(parent, target)
{
    covers_transitive(parent, child, target);
}

// ============================================================
// Compound scope: mount equality + prefix coverage
// ============================================================

// r[depends scope.compound-requires-all-fields]
/// Spec: compound scope requires mount equality and prefix coverage.
pub open spec fn compound_covers_spec(
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
) -> bool {
    cap_mount =~= op_mount && covers_spec(cap_prefix, op_path)
}

// r[impl scope.compound-requires-all-fields]
/// Returns true if the mounts match and the capability prefix covers the operation path.
pub fn compound_covers(
    cap_mount: &[u8],
    cap_prefix: &[u8],
    op_mount: &[u8],
    op_path: &[u8],
) -> (result: bool)
    ensures result == compound_covers_spec(cap_mount@, cap_prefix@, op_mount@, op_path@),
{
    bytes_eq(cap_mount, op_mount) && covers(cap_prefix, op_path)
}

/// Spec: byte-for-byte equality of two sequences.
pub open spec fn bytes_eq_spec(a: Seq<u8>, b: Seq<u8>) -> bool {
    a =~= b
}

// r[impl scope.bytes-eq]
/// Byte-for-byte equality comparison.
pub fn bytes_eq(a: &[u8], b: &[u8]) -> (result: bool)
    ensures result == bytes_eq_spec(a@, b@),
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a.len(),
            a@.len() == b@.len(),
            forall|j: int| 0 <= j < i as int ==> a@[j] == b@[j],
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

// r[verify scope.compound-requires-all-fields]
/// Mount mismatch denies regardless of prefix match.
proof fn compound_mount_mismatch_denies(
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
)
    requires !(cap_mount =~= op_mount)
    ensures !compound_covers_spec(cap_mount, cap_prefix, op_mount, op_path)
{}

// r[verify scope.compound-requires-all-fields]
/// When mounts match and prefix covers, compound coverage holds.
proof fn compound_both_match(
    cap_mount: Seq<u8>,
    cap_prefix: Seq<u8>,
    op_mount: Seq<u8>,
    op_path: Seq<u8>,
)
    requires
        cap_mount =~= op_mount,
        covers_spec(cap_prefix, op_path),
    ensures compound_covers_spec(cap_mount, cap_prefix, op_mount, op_path)
{}

// ============================================================
// Optional constraint: Option<Scope> combinator
// ============================================================

// r[depends scope.optional-none-unconstrained]
/// Spec: optional scope constraint.
/// - None cap: unconstrained, always passes.
/// - Some cap, None target: always fails.
/// - Some cap, Some target: delegates to covers.
pub open spec fn optional_covers_spec(
    cap: Option<Seq<u8>>,
    target: Option<Seq<u8>>,
) -> bool {
    match cap {
        None => true,
        Some(scope) => match target {
            None => false,
            Some(t) => covers_spec(scope, t),
        },
    }
}

// r[impl scope.optional-none-unconstrained]
/// Returns true based on optional constraint rules:
/// None cap always passes, Some cap with None target fails,
/// Some cap with Some target delegates to covers.
pub fn optional_covers(
    cap: Option<&[u8]>,
    target: Option<&[u8]>,
) -> (result: bool)
    ensures result == optional_covers_spec(
        match cap { None => None, Some(s) => Some(s@) },
        match target { None => None, Some(s) => Some(s@) },
    ),
{
    match cap {
        None => true,
        Some(scope) => match target {
            None => false,
            Some(t) => covers(scope, t),
        },
    }
}

// r[verify scope.optional-none-unconstrained]
/// None cap is unconstrained — passes for any target.
proof fn optional_none_unconstrained(target: Option<Seq<u8>>)
    ensures optional_covers_spec(None, target)
{}

// r[impl scope.optional-some-none-denies]
// r[verify scope.optional-some-none-denies]
/// Some cap with None target always denies.
proof fn optional_some_none_denies(scope: Seq<u8>)
    ensures !optional_covers_spec(Some(scope), None)
{}

// r[impl scope.optional-some-some-delegates]
// r[verify scope.optional-some-some-delegates]
/// Some cap with Some target delegates to covers.
proof fn optional_some_some_delegates(scope: Seq<u8>, target: Seq<u8>)
    ensures optional_covers_spec(Some(scope), Some(target)) == covers_spec(scope, target)
{}

// r[verify scope.optional-none-unconstrained]
/// Optional containment: None parent contains any child.
proof fn optional_none_contains_any(child: Option<Seq<u8>>)
    ensures optional_covers_spec(None, child)
{
    // None => true regardless of child
}

// r[verify scope.optional-some-some-delegates]
/// Optional containment with Some parent requires child to be narrower.
proof fn optional_some_narrowing(parent: Seq<u8>, child: Seq<u8>, target: Seq<u8>)
    requires
        optional_covers_spec(Some(parent), Some(child)),
        optional_covers_spec(Some(child), Some(target)),
    ensures optional_covers_spec(Some(parent), Some(target))
{
    // Some(parent) covers Some(child) means covers(parent, child)
    // Some(child) covers Some(target) means covers(child, target)
    // Need: covers(parent, target) — follows from transitivity
    covers_transitive(parent, child, target);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify scope.coverage-reflexive]
    #[test]
    fn test_starts_with_bytes() {
        assert!(starts_with_bytes(b"hello world", b"hello"));
        assert!(starts_with_bytes(b"hello", b"hello"));
        assert!(starts_with_bytes(b"hello", b""));
        assert!(starts_with_bytes(b"", b""));
        assert!(!starts_with_bytes(b"", b"x"));
        assert!(!starts_with_bytes(b"hello", b"world"));
        assert!(!starts_with_bytes(b"he", b"hello"));
    }

    // r[verify scope.coverage-reflexive]
    #[test]
    fn test_covers_basic() {
        assert!(covers(b"/home/user/", b"/home/user/docs/file.txt"));
        assert!(covers(b"/home/user/", b"/home/user/"));
        assert!(covers(b"", b"anything"));
        assert!(!covers(b"/home/user/", b"/etc/passwd"));
        assert!(!covers(b"/home/user/", b"/home/use"));
    }

    // r[verify scope.coverage-transitive]
    #[test]
    fn test_covers_transitive() {
        let a = b"/home/";
        let b = b"/home/user/";
        let c = b"/home/user/docs/";
        assert!(covers(a, b));
        assert!(covers(b, c));
        assert!(covers(a, c));
    }

    // r[verify scope.compound-requires-all-fields]
    #[test]
    fn test_compound_mount_mismatch() {
        assert!(!compound_covers(b"secret/", b"", b"kv/", b"key"));
    }

    // r[verify scope.compound-requires-all-fields]
    #[test]
    fn test_compound_both_match() {
        assert!(compound_covers(b"secret/", b"app/", b"secret/", b"app/db-pass"));
    }

    // r[verify scope.compound-requires-all-fields]
    #[test]
    fn test_compound_prefix_mismatch() {
        assert!(!compound_covers(b"secret/", b"app/", b"secret/", b"other/key"));
    }

    // r[verify scope.optional-none-unconstrained]
    #[test]
    fn test_optional_none_always_passes() {
        assert!(optional_covers(None, None));
        assert!(optional_covers(None, Some(b"anything")));
    }

    // r[verify scope.optional-some-none-denies]
    #[test]
    fn test_optional_some_none_denies() {
        assert!(!optional_covers(Some(b"/home/user"), None));
    }

    // r[verify scope.optional-some-some-delegates]
    #[test]
    fn test_optional_some_some_covers() {
        assert!(optional_covers(Some(b"/home/user"), Some(b"/home/user/project")));
        assert!(!optional_covers(Some(b"/home/user"), Some(b"/etc/passwd")));
    }

    #[test]
    fn test_bytes_eq() {
        assert!(bytes_eq(b"hello", b"hello"));
        assert!(!bytes_eq(b"hello", b"world"));
        assert!(!bytes_eq(b"hello", b"hell"));
        assert!(bytes_eq(b"", b""));
    }

    proptest! {
        // r[verify scope.coverage-reflexive]
        #[test]
        fn prop_covers_reflexive(
            s in proptest::collection::vec(any::<u8>(), 0..=100),
        ) {
            prop_assert!(covers(&s, &s));
        }

        // r[verify scope.coverage-transitive]
        #[test]
        fn prop_covers_transitive(
            prefix in proptest::collection::vec(any::<u8>(), 0..=30),
            mid in proptest::collection::vec(any::<u8>(), 0..=30),
            suffix in proptest::collection::vec(any::<u8>(), 0..=30),
        ) {
            let a = prefix.clone();
            let mut b = prefix.clone();
            b.extend_from_slice(&mid);
            let mut c = b.clone();
            c.extend_from_slice(&suffix);
            // a is prefix of b, b is prefix of c
            prop_assert!(covers(&a, &b));
            prop_assert!(covers(&b, &c));
            prop_assert!(covers(&a, &c));
        }

        // r[verify scope.coverage-rejects-non-prefix]
        #[test]
        fn prop_covers_non_prefix_rejected(
            scope in proptest::collection::vec(any::<u8>(), 1..=50),
            target in proptest::collection::vec(any::<u8>(), 0..=50),
        ) {
            if !target.starts_with(&scope) {
                prop_assert!(!covers(&scope, &target));
            }
        }

        // r[verify scope.coverage-narrowing-monotone]
        #[test]
        fn prop_narrowing_monotone(
            parent in proptest::collection::vec(any::<u8>(), 0..=20),
            child_ext in proptest::collection::vec(any::<u8>(), 0..=20),
            target_ext in proptest::collection::vec(any::<u8>(), 0..=20),
        ) {
            let mut child = parent.clone();
            child.extend_from_slice(&child_ext);
            let mut target = child.clone();
            target.extend_from_slice(&target_ext);
            // parent covers child, child covers target
            // therefore parent covers target
            prop_assert!(covers(&parent, &child));
            prop_assert!(covers(&child, &target));
            prop_assert!(covers(&parent, &target));
        }

        // r[verify scope.compound-requires-all-fields]
        #[test]
        fn prop_compound_mount_mismatch_denies(
            cap_mount in proptest::collection::vec(any::<u8>(), 1..=20),
            cap_prefix in proptest::collection::vec(any::<u8>(), 0..=20),
            op_mount in proptest::collection::vec(any::<u8>(), 1..=20),
            op_path in proptest::collection::vec(any::<u8>(), 0..=20),
        ) {
            if cap_mount != op_mount {
                prop_assert!(!compound_covers(&cap_mount, &cap_prefix, &op_mount, &op_path));
            }
        }

        // r[verify scope.optional-none-unconstrained]
        #[test]
        fn prop_optional_none_always_passes(
            target in proptest::collection::vec(any::<u8>(), 0..=50),
        ) {
            prop_assert!(optional_covers(None, Some(&target)));
            prop_assert!(optional_covers(None, None));
        }

        // r[verify scope.optional-some-none-denies]
        #[test]
        fn prop_optional_some_none_denies(
            scope in proptest::collection::vec(any::<u8>(), 0..=50),
        ) {
            prop_assert!(!optional_covers(Some(scope.as_slice()), None));
        }

        // r[verify scope.optional-some-some-delegates]
        #[test]
        fn prop_optional_delegates_to_covers(
            scope in proptest::collection::vec(any::<u8>(), 0..=50),
            target in proptest::collection::vec(any::<u8>(), 0..=50),
        ) {
            let opt_result = optional_covers(Some(&scope), Some(&target));
            let direct_result = covers(&scope, &target);
            prop_assert_eq!(opt_result, direct_result);
        }
    }
}

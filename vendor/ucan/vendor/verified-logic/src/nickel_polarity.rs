//! Verified Nickel contract blame polarity tracking.
//!
//! Models the `Polarity` enum from `core/src/label.rs`: polarity flips
//! on function domain, stays on codomain. Proves double-flip identity
//! and path-based polarity computation.

use vstd::prelude::*;

verus! {

/// Blame polarity for contract checking.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Polarity {
    Positive,
    Negative,
}

/// Spec: flip polarity.
// r[depends nickel-polarity.flip-domain]
// r[depends nickel-polarity.double-flip]
pub open spec fn flip_spec(p: Polarity) -> Polarity {
    match p {
        Polarity::Positive => Polarity::Negative,
        Polarity::Negative => Polarity::Positive,
    }
}

/// Exec: flip polarity.
// r[impl nickel-polarity.flip-domain]
// r[impl nickel-polarity.double-flip]
pub fn flip(p: Polarity) -> (result: Polarity)
    ensures result == flip_spec(p)
{
    match p {
        Polarity::Positive => Polarity::Negative,
        Polarity::Negative => Polarity::Positive,
    }
}

/// Type path step: navigating into a function type.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PathStep {
    Domain,   // entering function argument position
    Codomain, // entering function return position
}

/// Exec: is this step a Domain step?
// r[impl nickel-polarity.path-tracking]
// r[impl nickel-polarity.preserve-codomain]
pub fn is_domain(step: PathStep) -> (result: bool)
    ensures result == (step == PathStep::Domain)
{
    match step {
        PathStep::Domain => true,
        PathStep::Codomain => false,
    }
}

/// Spec: compute polarity after following a path.
/// Each Domain step flips, each Codomain step preserves.
// r[depends nickel-polarity.path-tracking]
pub open spec fn polarity_after_path(initial: Polarity, path: Seq<PathStep>) -> Polarity
    decreases path.len()
{
    if path.len() == 0 {
        initial
    } else {
        let next = match path[0int] {
            PathStep::Domain => flip_spec(initial),
            PathStep::Codomain => initial,
        };
        polarity_after_path(next, path.subrange(1, path.len() as int))
    }
}

/// Spec: double-flip identity as a standalone property.
// r[depends nickel-polarity.double-flip]
pub open spec fn double_flip_spec(p: Polarity) -> bool {
    flip_spec(flip_spec(p)) == p
}

/// Spec: count domain steps in a path.
pub open spec fn domain_count(path: Seq<PathStep>) -> nat
    decreases path.len()
{
    if path.len() == 0 { 0 }
    else {
        let rest = domain_count(path.subrange(1, path.len() as int));
        if path[0int] == PathStep::Domain { 1 + rest } else { rest }
    }
}

/// Spec: apply one polarity flip for each counted domain step.
pub open spec fn apply_domain_flips(initial: Polarity, count: nat) -> Polarity
    decreases count
{
    if count == 0 {
        initial
    } else {
        apply_domain_flips(flip_spec(initial), (count - 1) as nat)
    }
}

/// Exec: compute polarity after a path.
// r[impl nickel-polarity.path-tracking]
// r[impl nickel-polarity.preserve-codomain]
pub fn polarity_at(initial: Polarity, path: &Vec<PathStep>) -> (result: Polarity)
    ensures result == polarity_after_path(initial, path@)
{
    let mut current = initial;
    let mut i: usize = 0;

    proof {
        assert(path@.subrange(0, path@.len() as int) =~= path@);
    }

    while i < path.len()
        invariant
            i <= path.len(),
            polarity_after_path(initial, path@) ==
                polarity_after_path(current, path@.subrange(i as int, path@.len() as int)),
        decreases path.len() - i,
    {
        proof {
            let rem = path@.subrange(i as int, path@.len() as int);
            assert(rem.len() > 0);
            assert(rem[0int] == path@[i as int]);
            assert(rem.subrange(1, rem.len() as int) =~= path@.subrange(i as int + 1, path@.len() as int));
        }
        if is_domain(path[i]) {
            current = flip(current);
        }
        i = i + 1;
    }

    proof {
        assert(path@.subrange(i as int, path@.len() as int).len() == 0);
    }
    current
}

// --- Proofs ---

// r[verify nickel-polarity.double-flip]
/// Double-flip identity: flip(flip(p)) == p.
proof fn double_flip_identity(p: Polarity)
    ensures double_flip_spec(p)
{}

// r[verify nickel-polarity.flip-domain]
/// Domain flips polarity.
proof fn domain_flips(p: Polarity)
    ensures polarity_after_path(p, seq![PathStep::Domain]) == flip_spec(p)
{
    let path = seq![PathStep::Domain];
    let tail = path.subrange(1, path.len() as int);
    assert(path[0int] == PathStep::Domain);
    assert(tail =~= Seq::<PathStep>::empty());
    assert(polarity_after_path(p, path) == polarity_after_path(flip_spec(p), tail));
    assert(polarity_after_path(flip_spec(p), tail) == flip_spec(p));
}

// r[verify nickel-polarity.preserve-codomain]
/// Codomain preserves polarity.
proof fn codomain_preserves(p: Polarity)
    ensures polarity_after_path(p, seq![PathStep::Codomain]) == p
{
    let path = seq![PathStep::Codomain];
    let tail = path.subrange(1, path.len() as int);
    assert(path[0int] == PathStep::Codomain);
    assert(tail =~= Seq::<PathStep>::empty());
    assert(polarity_after_path(p, path) == polarity_after_path(p, tail));
    assert(polarity_after_path(p, tail) == p);
}

// r[verify nickel-polarity.path-tracking]
/// Path tracking matches flipping once for each domain step counted in the path.
proof fn path_matches_domain_flip_count(initial: Polarity, path: Seq<PathStep>)
    ensures polarity_after_path(initial, path) == apply_domain_flips(initial, domain_count(path))
    decreases path.len()
{
    if path.len() == 0 {
    } else {
        let tail = path.subrange(1, path.len() as int);
        if path[0int] == PathStep::Domain {
            path_matches_domain_flip_count(flip_spec(initial), tail);
            assert(domain_count(path) == 1 + domain_count(tail));
            assert(polarity_after_path(initial, path) == polarity_after_path(flip_spec(initial), tail));
            assert(apply_domain_flips(initial, domain_count(path)) == apply_domain_flips(flip_spec(initial), domain_count(tail)));
        } else {
            path_matches_domain_flip_count(initial, tail);
            assert(domain_count(path) == domain_count(tail));
            assert(polarity_after_path(initial, path) == polarity_after_path(initial, tail));
        }
    }
}

// r[verify nickel-polarity.path-tracking]
/// A single domain step flips.
proof fn path_single_domain(initial: Polarity)
    ensures ({
        let path = seq![PathStep::Domain];
        polarity_after_path(initial, path) == flip_spec(initial)
    })
{
    domain_flips(initial);
}

// r[verify nickel-polarity.path-tracking]
/// Two domain steps return to original (even parity).
proof fn path_double_domain(initial: Polarity)
    ensures ({
        let path = seq![PathStep::Domain, PathStep::Domain];
        polarity_after_path(initial, path) == initial
    })
{
    let path = seq![PathStep::Domain, PathStep::Domain];
    let tail = path.subrange(1, path.len() as int);
    assert(path[0int] == PathStep::Domain);
    assert(tail =~= seq![PathStep::Domain]);
    assert(polarity_after_path(initial, path) == polarity_after_path(flip_spec(initial), tail));
    domain_flips(flip_spec(initial));
    double_flip_identity(initial);
}

// r[verify nickel-polarity.path-tracking]
/// Higher-order: (A -> B) -> C with initial Positive.
proof fn higher_order_path(initial: Polarity)
    ensures ({
        let path = seq![PathStep::Domain, PathStep::Domain];
        polarity_after_path(Polarity::Positive, path) == Polarity::Positive
    })
{
    path_double_domain(Polarity::Positive);
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify nickel-polarity.double-flip]
    #[test]
    fn test_double_flip() {
        assert_eq!(flip(flip(Polarity::Positive)), Polarity::Positive);
        assert_eq!(flip(flip(Polarity::Negative)), Polarity::Negative);
    }

    // r[verify nickel-polarity.flip-domain]
    #[test]
    fn test_domain_flip() {
        assert_eq!(flip(Polarity::Positive), Polarity::Negative);
        assert_eq!(flip(Polarity::Negative), Polarity::Positive);
    }

    // r[verify nickel-polarity.path-tracking]
    #[test]
    fn test_first_order_function() {
        // Number -> Number, checking argument: one domain flip
        let path = vec![PathStep::Domain];
        assert_eq!(polarity_at(Polarity::Positive, &path), Polarity::Negative);
    }

    // r[verify nickel-polarity.path-tracking]
    #[test]
    fn test_higher_order_function() {
        // (Number -> Number) -> Number, checking inner argument: two domain flips
        let path = vec![PathStep::Domain, PathStep::Domain];
        assert_eq!(polarity_at(Polarity::Positive, &path), Polarity::Positive);
    }

    // r[verify nickel-polarity.preserve-codomain]
    #[test]
    fn test_codomain_preserves() {
        let path = vec![PathStep::Codomain];
        assert_eq!(polarity_at(Polarity::Positive, &path), Polarity::Positive);
    }

    // r[verify nickel-polarity.path-tracking]
    #[test]
    fn test_mixed_path_even_domain_count() {
        let path = vec![
            PathStep::Codomain,
            PathStep::Domain,
            PathStep::Codomain,
            PathStep::Domain,
            PathStep::Codomain,
        ];
        assert_eq!(polarity_at(Polarity::Positive, &path), Polarity::Positive);
    }

    // r[verify nickel-polarity.path-tracking]
    #[test]
    fn test_mixed_path_odd_domain_count() {
        let path = vec![
            PathStep::Codomain,
            PathStep::Domain,
            PathStep::Codomain,
            PathStep::Domain,
            PathStep::Domain,
        ];
        assert_eq!(polarity_at(Polarity::Positive, &path), Polarity::Negative);
    }
}

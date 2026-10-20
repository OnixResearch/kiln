//! Generic algebraic property predicates and proof lemmas.
//!
//! Reusable specs for common mathematical properties: commutativity,
//! associativity, idempotence, identity elements, involution, monotonicity,
//! and roundtrip (section-retraction) pairs. Modules instantiate these
//! with their concrete operations instead of re-proving from scratch.
//!
//! All predicates operate on `spec_fn` — ghost-only, erased at runtime.

use vstd::prelude::*;

verus! {

// ============================================================
// Binary operation properties (over u64)
// ============================================================

/// A binary operation is commutative: f(a, b) == f(b, a).
pub open spec fn is_commutative(f: spec_fn(u64, u64) -> u64) -> bool {
    forall|a: u64, b: u64| #[trigger] f(a, b) == f(b, a)
}

/// A binary operation is associative: f(f(a, b), c) == f(a, f(b, c)).
pub open spec fn is_associative(f: spec_fn(u64, u64) -> u64) -> bool {
    forall|a: u64, b: u64, c: u64| #[trigger] f(f(a, b), c) == f(a, f(b, c))
}

/// A binary operation is idempotent: f(a, a) == a.
pub open spec fn is_idempotent_binary(f: spec_fn(u64, u64) -> u64) -> bool {
    forall|a: u64| #![trigger f(a, a)] f(a, a) == a
}

/// `e` is a left identity for `f`: f(e, a) == a.
pub open spec fn has_left_identity(f: spec_fn(u64, u64) -> u64, e: u64) -> bool {
    forall|a: u64| #[trigger] f(e, a) == a
}

/// `e` is a right identity for `f`: f(a, e) == a.
pub open spec fn has_right_identity(f: spec_fn(u64, u64) -> u64, e: u64) -> bool {
    forall|a: u64| #[trigger] f(a, e) == a
}

/// A binary operation forms a semilattice: commutative + associative + idempotent.
pub open spec fn is_semilattice(f: spec_fn(u64, u64) -> u64) -> bool {
    is_commutative(f) && is_associative(f) && is_idempotent_binary(f)
}

// ============================================================
// Unary operation properties (over u64)
// ============================================================

/// A unary operation is idempotent: f(f(x)) == f(x).
pub open spec fn is_idempotent(f: spec_fn(u64) -> u64) -> bool {
    forall|x: u64| #![trigger f(f(x))] f(f(x)) == f(x)
}

/// A unary operation is an involution: f(f(x)) == x.
pub open spec fn is_involution(f: spec_fn(u64) -> u64) -> bool {
    forall|x: u64| #![trigger f(f(x))] f(f(x)) == x
}

// ============================================================
// Two-function properties (roundtrips / sections-retractions)
// ============================================================

/// g is a left inverse of f: g(f(x)) == x for all x.
pub open spec fn is_left_inverse(f: spec_fn(u64) -> u64, g: spec_fn(u64) -> u64) -> bool {
    forall|x: u64| #![trigger g(f(x))] g(f(x)) == x
}

/// g is a right inverse of f: f(g(x)) == x for all x.
pub open spec fn is_right_inverse(f: spec_fn(u64) -> u64, g: spec_fn(u64) -> u64) -> bool {
    forall|x: u64| #![trigger f(g(x))] f(g(x)) == x
}

/// f and g form a roundtrip: g(f(x)) == x AND f(g(x)) == x.
pub open spec fn is_roundtrip(f: spec_fn(u64) -> u64, g: spec_fn(u64) -> u64) -> bool {
    is_left_inverse(f, g) && is_right_inverse(f, g)
}

// ============================================================
// Order-related properties
// ============================================================

/// f is monotone with respect to <=.
pub open spec fn is_monotone(f: spec_fn(u64) -> u64) -> bool {
    forall|a: u64, b: u64| a <= b ==> #[trigger] f(a) <= #[trigger] f(b)
}

/// A binary operation f(x, y) is monotone in the first argument.
pub open spec fn is_monotone_left(f: spec_fn(u64, u64) -> u64) -> bool {
    forall|a: u64, b: u64, c: u64| a <= b ==> #[trigger] f(a, c) <= #[trigger] f(b, c)
}

/// f is bounded above by `bound` for all inputs.
pub open spec fn is_bounded_by(f: spec_fn(u64) -> u64, bound: u64) -> bool {
    forall|x: u64| #[trigger] f(x) <= bound
}

// ============================================================
// Relation properties (partial orders, preorders)
// ============================================================

/// A relation is reflexive.
pub open spec fn is_reflexive(r: spec_fn(u64, u64) -> bool) -> bool {
    forall|x: u64| #![trigger r(x, x)] r(x, x)
}

/// A relation is antisymmetric: r(a,b) && r(b,a) ==> a == b.
pub open spec fn is_antisymmetric(r: spec_fn(u64, u64) -> bool) -> bool {
    forall|a: u64, b: u64| #![trigger r(a, b), r(b, a)] r(a, b) && r(b, a) ==> a == b
}

/// A relation is transitive.
pub open spec fn is_transitive(r: spec_fn(u64, u64) -> bool) -> bool {
    forall|a: u64, b: u64, c: u64| #![trigger r(a, b), r(b, c)] r(a, b) && r(b, c) ==> r(a, c)
}

/// A relation is a partial order: reflexive + antisymmetric + transitive.
pub open spec fn is_partial_order(r: spec_fn(u64, u64) -> bool) -> bool {
    is_reflexive(r) && is_antisymmetric(r) && is_transitive(r)
}

/// A relation is irreflexive: !r(x, x) for all x.
pub open spec fn is_irreflexive(r: spec_fn(u64, u64) -> bool) -> bool {
    forall|x: u64| #![trigger r(x, x)] !r(x, x)
}

/// A relation is a strict partial order: irreflexive + transitive.
pub open spec fn is_strict_partial_order(r: spec_fn(u64, u64) -> bool) -> bool {
    is_irreflexive(r) && is_transitive(r)
}

// ============================================================
// Semilattice proof: min is a semilattice
// ============================================================

// r[verify algebraic.min-semilattice]
/// min is a join-semilattice (commutative, associative, idempotent).
pub proof fn min_is_semilattice()
    ensures is_semilattice(|a: u64, b: u64| if a <= b { a } else { b })
{}

// r[verify algebraic.max-semilattice]
/// max is a meet-semilattice (commutative, associative, idempotent).
pub proof fn max_is_semilattice()
    ensures is_semilattice(|a: u64, b: u64| if a >= b { a } else { b })
{}

// r[verify algebraic.advance-partial-order]
/// <= on u64 is a partial order.
pub proof fn le_is_partial_order()
    ensures is_partial_order(|a: u64, b: u64| a <= b)
{}

// r[verify algebraic.strict-advance-strict-partial-order]
/// < on u64 is a strict partial order.
pub proof fn lt_is_strict_partial_order()
    ensures is_strict_partial_order(|a: u64, b: u64| a < b)
{}

} // verus!

#[cfg(test)]
mod tests {
    // No exec functions — algebraic module is pure ghost code.
    // Tests are in the modules that instantiate these properties.
}

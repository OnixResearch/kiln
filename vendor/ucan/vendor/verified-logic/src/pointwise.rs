//! Generic pointwise lifting of scalar operations to sequences.
//!
//! `pointwise_merge_spec` applies a binary function element-wise to two
//! equal-length sequences. The key theorem: if the scalar function
//! is a semilattice (commutative + associative + idempotent), the
//! lifted operation inherits all three properties.
//!
//! This eliminates duplicated merge specs and proofs across
//! `vector_clock`, `crdt` (G-Counter), and `causal_order` (cut join),
//! which all independently defined the same pointwise-max pattern.

use vstd::prelude::*;

verus! {

// ============================================================
// Spec: pointwise merge
// ============================================================

// r[depends pointwise.merge]
/// Apply a binary function element-wise to two equal-length sequences.
pub open spec fn pointwise_merge_spec(
    a: Seq<u64>, b: Seq<u64>, f: spec_fn(u64, u64) -> u64,
) -> Seq<u64>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| f(a[i], b[i]))
}

// ============================================================
// Lifted algebraic proofs
// ============================================================

// r[verify pointwise.merge]
/// Pointwise merge inherits commutativity from the scalar function.
pub proof fn pointwise_commutative(
    a: Seq<u64>, b: Seq<u64>, f: spec_fn(u64, u64) -> u64,
)
    requires
        a.len() == b.len(),
        forall|x: u64, y: u64| #[trigger] f(x, y) == f(y, x),
    ensures
        pointwise_merge_spec(a, b, f) =~= pointwise_merge_spec(b, a, f),
{}

// r[verify pointwise.merge]
/// Pointwise merge inherits associativity from the scalar function.
pub proof fn pointwise_associative(
    a: Seq<u64>, b: Seq<u64>, c: Seq<u64>, f: spec_fn(u64, u64) -> u64,
)
    requires
        a.len() == b.len(),
        b.len() == c.len(),
        forall|x: u64, y: u64, z: u64| #[trigger] f(f(x, y), z) == f(x, f(y, z)),
    ensures
        pointwise_merge_spec(pointwise_merge_spec(a, b, f), c, f)
            =~= pointwise_merge_spec(a, pointwise_merge_spec(b, c, f), f),
{}

// r[verify pointwise.merge]
/// Pointwise merge inherits idempotence from the scalar function.
pub proof fn pointwise_idempotent(
    a: Seq<u64>, f: spec_fn(u64, u64) -> u64,
)
    requires
        forall|x: u64| #![trigger f(x, x)] f(x, x) == x,
    ensures
        pointwise_merge_spec(a, a, f) =~= a,
{}

// r[verify pointwise.merge]
/// Each component of the merge dominates the corresponding component of `a`.
/// Requires that `f(x, y) >= x` for all x, y (i.e., f is a join).
pub proof fn pointwise_dominates_left(
    a: Seq<u64>, b: Seq<u64>, f: spec_fn(u64, u64) -> u64,
)
    requires
        a.len() == b.len(),
        forall|x: u64, y: u64| #[trigger] f(x, y) >= x,
    ensures
        forall|i: int| 0 <= i < a.len() as int ==>
            pointwise_merge_spec(a, b, f)[i] >= a[i],
{}

// r[verify pointwise.merge]
/// Each component of the merge dominates the corresponding component of `b`.
pub proof fn pointwise_dominates_right(
    a: Seq<u64>, b: Seq<u64>, f: spec_fn(u64, u64) -> u64,
)
    requires
        a.len() == b.len(),
        forall|x: u64, y: u64| #[trigger] f(x, y) >= y,
    ensures
        forall|i: int| 0 <= i < a.len() as int ==>
            pointwise_merge_spec(a, b, f)[i] >= b[i],
{}

// r[verify pointwise.merge]
/// Pointwise merge preserves length.
pub proof fn pointwise_preserves_length(
    a: Seq<u64>, b: Seq<u64>, f: spec_fn(u64, u64) -> u64,
)
    requires a.len() == b.len()
    ensures pointwise_merge_spec(a, b, f).len() == a.len()
{}

// ============================================================
// max_spec as the canonical scalar semilattice
// ============================================================

// We provide the max scalar facts as `open spec fn` + proof pairs,
// so callers can use `max_u64` directly in pointwise_merge_spec
// and reference these proofs for the preconditions.

/// The canonical scalar max function (same as ordering::max_spec).
pub open spec fn max_u64(a: u64, b: u64) -> u64 {
    if a >= b { a } else { b }
}

// r[verify pointwise.merge]
/// max_u64 is commutative.
pub proof fn max_u64_commutative()
    ensures forall|x: u64, y: u64| #[trigger] max_u64(x, y) == max_u64(y, x)
{}

// r[verify pointwise.merge]
/// max_u64 is associative.
pub proof fn max_u64_associative()
    ensures forall|x: u64, y: u64, z: u64|
        #[trigger] max_u64(max_u64(x, y), z) == max_u64(x, max_u64(y, z))
{}

// r[verify pointwise.merge]
/// max_u64 is idempotent.
pub proof fn max_u64_idempotent()
    ensures forall|x: u64| #![trigger max_u64(x, x)] max_u64(x, x) == x
{}

// r[verify pointwise.merge]
/// max_u64(x, y) >= x for all x, y.
pub proof fn max_u64_dominates_left()
    ensures forall|x: u64, y: u64| #[trigger] max_u64(x, y) >= x
{}

// r[verify pointwise.merge]
/// max_u64(x, y) >= y for all x, y.
pub proof fn max_u64_dominates_right()
    ensures forall|x: u64, y: u64| #[trigger] max_u64(x, y) >= y
{}

} // verus!

#[cfg(test)]
mod tests {
    // Pure spec/proof module — tested through vector_clock, crdt, causal_order.
}

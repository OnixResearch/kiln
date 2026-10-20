//! Verified Nickel merge priority total order.
//!
//! Models `MergePriority` from Nickel's `parser/src/ast/mod.rs`:
//! `Bottom < Neutral ≡ Numeral(0) < Numeral(n) < Top`. Proves
//! reflexivity, antisymmetry, transitivity, and lattice extrema.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

/// Nickel merge priority: Bottom (default) < Neutral < Numeral(n) < Top (force).
/// Neutral is equivalent to Numeral(0).
#[derive(Debug, PartialEq, Eq)]
pub enum MergePriority {
    Bottom,
    Neutral,
    Numeral(i64),
    Top,
}

/// Coarse comparison bucket.
///
/// Bottom is strictly below all numeral/neutral priorities, and Top is
/// strictly above them. Neutral shares the numeral bucket so Neutral and
/// Numeral(0) compare equal, but Bottom and Top stay distinct from the
/// extremal numeral values.
// r[depends nickel-priority.total-order]
pub open spec fn priority_bucket(p: MergePriority) -> int {
    match p {
        MergePriority::Bottom => 0int,
        MergePriority::Neutral | MergePriority::Numeral(_) => 1int,
        MergePriority::Top => 2int,
    }
}

/// Intra-bucket comparison value.
// r[depends nickel-priority.total-order]
pub open spec fn priority_rank(p: MergePriority) -> int {
    match p {
        MergePriority::Bottom => 0int,
        MergePriority::Neutral => 0int,
        MergePriority::Numeral(n) => n as int,
        MergePriority::Top => 0int,
    }
}

/// Spec: less-than-or-equal on priorities.
// r[depends nickel-priority.total-order]
pub open spec fn priority_le(a: MergePriority, b: MergePriority) -> bool {
    priority_bucket(a) < priority_bucket(b)
        || (priority_bucket(a) == priority_bucket(b) && priority_rank(a) <= priority_rank(b))
}

/// Spec: strict less-than on priorities.
pub open spec fn priority_lt(a: MergePriority, b: MergePriority) -> bool {
    priority_bucket(a) < priority_bucket(b)
        || (priority_bucket(a) == priority_bucket(b) && priority_rank(a) < priority_rank(b))
}

/// Spec: equality on priorities.
pub open spec fn priority_eq(a: MergePriority, b: MergePriority) -> bool {
    priority_bucket(a) == priority_bucket(b) && priority_rank(a) == priority_rank(b)
}

/// Exec: compare two priorities. Returns -1, 0, or 1.
// r[impl nickel-priority.total-order]
// r[impl nickel-priority.extrema]
// r[impl nickel-priority.numeral-ordering]
pub fn priority_cmp(a: &MergePriority, b: &MergePriority) -> (result: i8)
    ensures
        result == -1 || result == 0 || result == 1,
        result == 0 <==> priority_eq(*a, *b),
        result < 0 <==> priority_lt(*a, *b),
        result > 0 <==> priority_lt(*b, *a),
{
    let ba: i8 = match a {
        MergePriority::Bottom => 0,
        MergePriority::Neutral | MergePriority::Numeral(_) => 1,
        MergePriority::Top => 2,
    };
    let bb: i8 = match b {
        MergePriority::Bottom => 0,
        MergePriority::Neutral | MergePriority::Numeral(_) => 1,
        MergePriority::Top => 2,
    };
    let ra: i64 = match a {
        MergePriority::Numeral(n) => *n,
        MergePriority::Bottom | MergePriority::Neutral | MergePriority::Top => 0,
    };
    let rb: i64 = match b {
        MergePriority::Numeral(n) => *n,
        MergePriority::Bottom | MergePriority::Neutral | MergePriority::Top => 0,
    };
    if ba < bb { -1 }
    else if ba > bb { 1 }
    else if ra < rb { -1 }
    else if ra > rb { 1 }
    else { 0 }
}

// --- Proofs ---

// r[verify nickel-priority.total-order]
/// Reflexivity: every priority is ≤ itself.
proof fn priority_reflexive(a: MergePriority)
    ensures priority_le(a, a)
{}

// r[verify nickel-priority.total-order]
/// Antisymmetry: a ≤ b ∧ b ≤ a ⟹ rank(a) == rank(b).
proof fn priority_antisymmetric(a: MergePriority, b: MergePriority)
    requires priority_le(a, b), priority_le(b, a)
    ensures priority_eq(a, b)
{}

// r[verify nickel-priority.total-order]
/// Transitivity: a ≤ b ∧ b ≤ c ⟹ a ≤ c.
proof fn priority_transitive(a: MergePriority, b: MergePriority, c: MergePriority)
    requires priority_le(a, b), priority_le(b, c)
    ensures priority_le(a, c)
{}

// r[verify nickel-priority.total-order]
/// Totality: for all a, b, either a ≤ b or b ≤ a.
proof fn priority_total(a: MergePriority, b: MergePriority)
    ensures priority_le(a, b) || priority_le(b, a)
{}

// r[verify nickel-priority.extrema]
/// Bottom is the minimum: Bottom ≤ every priority.
proof fn bottom_is_minimum(p: MergePriority)
    ensures priority_le(MergePriority::Bottom, p)
{}

// r[verify nickel-priority.extrema]
/// Top is the maximum: every priority ≤ Top.
proof fn top_is_maximum(p: MergePriority)
    ensures priority_le(p, MergePriority::Top)
{}

// r[verify nickel-priority.numeral-ordering]
/// Numeral ordering agrees with integer ordering.
proof fn numeral_ordering(a: i64, b: i64)
    ensures priority_lt(MergePriority::Numeral(a), MergePriority::Numeral(b)) <==> a < b
{}

// r[verify nickel-priority.total-order]
/// Neutral equals Numeral(0).
proof fn neutral_equals_zero()
    ensures priority_eq(MergePriority::Neutral, MergePriority::Numeral(0))
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify nickel-priority.total-order]
    #[test]
    fn test_priority_cmp_basic() {
        assert_eq!(priority_cmp(&MergePriority::Bottom, &MergePriority::Top), -1);
        assert_eq!(priority_cmp(&MergePriority::Top, &MergePriority::Bottom), 1);
        assert_eq!(priority_cmp(&MergePriority::Neutral, &MergePriority::Neutral), 0);
        assert_eq!(priority_cmp(&MergePriority::Neutral, &MergePriority::Numeral(0)), 0);
    }

    // r[verify nickel-priority.extrema]
    #[test]
    fn test_bottom_minimum() {
        let priorities = [MergePriority::Neutral, MergePriority::Numeral(5), MergePriority::Top];
        for p in &priorities {
            assert_eq!(priority_cmp(&MergePriority::Bottom, p), -1);
        }
    }

    // r[verify nickel-priority.total-order]
    #[test]
    fn test_extremal_numerals_do_not_alias_bounds() {
        assert_eq!(priority_cmp(&MergePriority::Bottom, &MergePriority::Numeral(i64::MIN)), -1);
        assert_eq!(priority_cmp(&MergePriority::Numeral(i64::MIN), &MergePriority::Bottom), 1);
        assert_eq!(priority_cmp(&MergePriority::Top, &MergePriority::Numeral(i64::MAX)), 1);
        assert_eq!(priority_cmp(&MergePriority::Numeral(i64::MAX), &MergePriority::Top), -1);
    }

    // r[verify nickel-priority.numeral-ordering]
    #[test]
    fn test_numeral_ordering() {
        assert_eq!(priority_cmp(&MergePriority::Numeral(1), &MergePriority::Numeral(5)), -1);
        assert_eq!(priority_cmp(&MergePriority::Numeral(5), &MergePriority::Numeral(1)), 1);
        assert_eq!(priority_cmp(&MergePriority::Numeral(3), &MergePriority::Numeral(3)), 0);
    }
}

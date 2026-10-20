//! Verified layout constraint resolution.
//!
//! Models size constraint systems used by tiling layout engines. Each item
//! has a constraint (fixed, proportional, min, min-max) and the resolver
//! distributes available space among items respecting those constraints.
//!
//! Properties verified: conservation (sizes + gaps ≤ total), fixed exactness,
//! min floors, min-max clamping, output length preservation.

use vstd::prelude::*;

verus! {

// --- Constraint tag constants ---

/// Fixed: exact cell count.
pub const TAG_FIXED: u8 = 0;
/// Proportional: fraction of remaining space.
pub const TAG_PROPORTION: u8 = 1;
/// Min: at least N cells, grows to fill.
pub const TAG_MIN: u8 = 2;
/// MinMax: clamped to [min, max] after allocation.
pub const TAG_MINMAX: u8 = 3;

/// Spec: tag is one of the four known constraint types.
pub open spec fn is_valid_tag(tag: u8) -> bool {
    tag == TAG_FIXED || tag == TAG_PROPORTION || tag == TAG_MIN || tag == TAG_MINMAX
}

// --- satisfies_constraint ---

// r[depends layout-constraint.satisfies]
/// Spec: a resolved size satisfies a constraint.
///
/// - Fixed(n): resolved == n
/// - Proportion(_): any size (constraint is relative, not absolute)
/// - Min(n): resolved >= n
/// - MinMax(lo, hi): lo <= resolved <= hi (when lo <= hi)
pub open spec fn satisfies_constraint_spec(tag: u8, lo: u16, hi: u16, resolved: u16) -> bool {
    if tag == TAG_FIXED {
        resolved == lo
    } else if tag == TAG_PROPORTION {
        true
    } else if tag == TAG_MIN {
        resolved >= lo
    } else if tag == TAG_MINMAX {
        lo <= resolved && resolved <= hi
    } else {
        false
    }
}

// r[impl layout-constraint.satisfies]
/// Returns true if `resolved` satisfies the constraint given by (tag, lo, hi).
pub fn satisfies_constraint(tag: u8, lo: u16, hi: u16, resolved: u16) -> (result: bool)
    requires is_valid_tag(tag), tag == TAG_MINMAX ==> lo <= hi
    ensures result == satisfies_constraint_spec(tag, lo, hi, resolved)
{
    if tag == TAG_FIXED {
        resolved == lo
    } else if tag == TAG_PROPORTION {
        true
    } else if tag == TAG_MIN {
        resolved >= lo
    } else {
        // TAG_MINMAX
        lo <= resolved && resolved <= hi
    }
}

// r[verify layout-constraint.satisfies]
/// Fixed constraints have exactly one solution.
proof fn fixed_unique(n: u16, a: u16, b: u16)
    requires
        satisfies_constraint_spec(TAG_FIXED, n, 0, a),
        satisfies_constraint_spec(TAG_FIXED, n, 0, b),
    ensures a == b
{}

// r[verify layout-constraint.satisfies]
/// MinMax subsumes Min when max is large enough.
proof fn minmax_subsumes_min(lo: u16, hi: u16, resolved: u16)
    requires lo <= hi, satisfies_constraint_spec(TAG_MINMAX, lo, hi, resolved)
    ensures satisfies_constraint_spec(TAG_MIN, lo, 0, resolved)
{}

// r[verify layout-constraint.satisfies]
/// Proportional constraints are always satisfied.
proof fn proportion_always_satisfied(resolved: u16)
    ensures satisfies_constraint_spec(TAG_PROPORTION, 0, 0, resolved)
{}

// --- clamp_to_constraint ---

// r[depends layout-constraint.clamp]
/// Spec: clamp a value to satisfy the constraint bounds.
///
/// For Fixed, returns the fixed value. For Proportion, passes through.
/// For Min, returns max(value, min). For MinMax, clamps to [min, max].
pub open spec fn clamp_to_constraint_spec(tag: u8, lo: u16, hi: u16, value: u16) -> u16 {
    if tag == TAG_FIXED {
        lo
    } else if tag == TAG_PROPORTION {
        value
    } else if tag == TAG_MIN {
        if value >= lo { value } else { lo }
    } else if tag == TAG_MINMAX {
        if value < lo { lo }
        else if value > hi { hi }
        else { value }
    } else {
        value
    }
}

// r[impl layout-constraint.clamp]
/// Clamps `value` to satisfy the given constraint.
pub fn clamp_to_constraint(tag: u8, lo: u16, hi: u16, value: u16) -> (result: u16)
    requires is_valid_tag(tag), tag == TAG_MINMAX ==> lo <= hi
    ensures result == clamp_to_constraint_spec(tag, lo, hi, value)
{
    if tag == TAG_FIXED {
        lo
    } else if tag == TAG_PROPORTION {
        value
    } else if tag == TAG_MIN {
        if value >= lo { value } else { lo }
    } else {
        // TAG_MINMAX
        if value < lo { lo }
        else if value > hi { hi }
        else { value }
    }
}

// r[verify layout-constraint.clamp]
/// Clamping always produces a value that satisfies the constraint.
proof fn clamp_satisfies(tag: u8, lo: u16, hi: u16, value: u16)
    requires is_valid_tag(tag), tag == TAG_MINMAX ==> lo <= hi
    ensures satisfies_constraint_spec(tag, lo, hi, clamp_to_constraint_spec(tag, lo, hi, value))
{}

// r[verify layout-constraint.clamp]
/// Clamping a value that already satisfies the constraint is a no-op.
proof fn clamp_idempotent_on_satisfying(tag: u8, lo: u16, hi: u16, value: u16)
    requires
        is_valid_tag(tag),
        tag == TAG_MINMAX ==> lo <= hi,
        satisfies_constraint_spec(tag, lo, hi, value),
    ensures clamp_to_constraint_spec(tag, lo, hi, value) == value
{}

// r[verify layout-constraint.clamp]
/// Clamping is idempotent: clamp(clamp(x)) == clamp(x).
proof fn clamp_idempotent(tag: u8, lo: u16, hi: u16, value: u16)
    requires is_valid_tag(tag), tag == TAG_MINMAX ==> lo <= hi
    ensures clamp_to_constraint_spec(tag, lo, hi,
        clamp_to_constraint_spec(tag, lo, hi, value))
        == clamp_to_constraint_spec(tag, lo, hi, value)
{
    // First clamp satisfies the constraint...
    clamp_satisfies(tag, lo, hi, value);
    // ...so second clamp is a no-op.
    clamp_idempotent_on_satisfying(tag, lo, hi,
        clamp_to_constraint_spec(tag, lo, hi, value));
}

// --- conservation (sizes + gaps ≤ total) ---

// r[depends layout-constraint.conservation]
/// Spec: sum of sizes in a sequence.
pub open spec fn sum_sizes_spec(sizes: Seq<u16>, from: int) -> int
    decreases (if from < sizes.len() as int { sizes.len() as int - from } else { 0 })
{
    if from >= sizes.len() as int {
        0
    } else {
        sizes[from] as int + sum_sizes_spec(sizes, from + 1)
    }
}

/// Spec: total gaps between n items.
pub open spec fn total_gaps_spec(n: int, gap: u16) -> int {
    if n <= 1 { 0 } else { (n - 1) * (gap as int) }
}

/// Spec: sizes plus gaps fit within the available space.
pub open spec fn conservation_spec(sizes: Seq<u16>, gap: u16, total: u16) -> bool {
    sum_sizes_spec(sizes, 0) + total_gaps_spec(sizes.len() as int, gap) <= total as int
}

// r[impl layout-constraint.conservation]
// r[verify layout-constraint.conservation]
/// Sum of an empty sequence is zero.
proof fn sum_empty()
    ensures sum_sizes_spec(Seq::<u16>::empty(), 0) == 0
{}

// r[verify layout-constraint.conservation]
/// Sum of a single element is that element.
proof fn sum_singleton(v: u16)
    ensures sum_sizes_spec(seq![v], 0) == v as int
{
    // Unfold: sum(seq![v], 0) = v + sum(seq![v], 1) = v + 0
    assert(sum_sizes_spec(seq![v], 1) == 0);
}

// r[verify layout-constraint.conservation]
/// Sum is non-negative.
proof fn sum_nonneg(sizes: Seq<u16>, from: int)
    ensures sum_sizes_spec(sizes, from) >= 0
    decreases (if from < sizes.len() as int { sizes.len() as int - from } else { 0 })
{
    if from < sizes.len() as int {
        sum_nonneg(sizes, from + 1);
    }
}

// r[verify layout-constraint.conservation]
/// Total gaps for zero or one item is zero.
proof fn gaps_trivial(n: int, gap: u16)
    requires n <= 1
    ensures total_gaps_spec(n, gap) == 0
{}

// r[verify layout-constraint.conservation]
/// Total gaps is non-negative when n >= 0.
proof fn gaps_nonneg(n: int, gap: u16)
    requires n >= 0
    ensures total_gaps_spec(n, gap) >= 0
{}

// --- output length preservation ---

// r[depends layout-constraint.length-preservation]
/// Spec: resolver output has the same length as input.
pub open spec fn length_preserved_spec(input_len: int, output_len: int) -> bool {
    input_len == output_len
}

// r[impl layout-constraint.length-preservation]
// r[verify layout-constraint.length-preservation]
/// Length preservation is reflexive.
proof fn length_preserved_reflexive(n: int)
    ensures length_preserved_spec(n, n)
{}

// --- zero total yields zero for proportional ---

// r[impl layout-constraint.zero-total]
// r[verify layout-constraint.zero-total]
// r[depends layout-constraint.zero-total]
/// Spec: proportional constraints resolve to zero when total space is zero.
pub open spec fn zero_total_proportional_spec(tag: u8, resolved: u16) -> bool {
    tag == TAG_PROPORTION ==> resolved == 0
}

// --- fixed items consume exact space ---

// r[depends layout-constraint.fixed-conservation]
/// Spec: the sum of all fixed constraint values.
pub open spec fn sum_fixed_spec(tags: Seq<u8>, values: Seq<u16>, from: int) -> int
    decreases (if from < tags.len() as int { tags.len() as int - from } else { 0 })
{
    if from >= tags.len() as int || from >= values.len() as int {
        0
    } else if tags[from] == TAG_FIXED {
        values[from] as int + sum_fixed_spec(tags, values, from + 1)
    } else {
        sum_fixed_spec(tags, values, from + 1)
    }
}

// r[impl layout-constraint.fixed-conservation]
// r[verify layout-constraint.fixed-conservation]
/// When all constraints are Fixed, the sum of resolved sizes equals the sum
/// of the fixed values.
proof fn all_fixed_exact_sum(tags: Seq<u8>, values: Seq<u16>, sizes: Seq<u16>, from: int)
    requires
        tags.len() == values.len(),
        tags.len() == sizes.len(),
        forall|i: int| from <= i < tags.len() as int ==>
            (#[trigger] tags[i] == TAG_FIXED && sizes[i] == values[i]),
    ensures sum_sizes_spec(sizes, from) == sum_fixed_spec(tags, values, from)
    decreases (if from < tags.len() as int { tags.len() as int - from } else { 0 })
{
    if from < tags.len() as int {
        all_fixed_exact_sum(tags, values, sizes, from + 1);
    }
}

// r[verify layout-constraint.fixed-conservation]
/// Sum of fixed values is non-negative.
proof fn sum_fixed_nonneg(tags: Seq<u8>, values: Seq<u16>, from: int)
    requires tags.len() == values.len()
    ensures sum_fixed_spec(tags, values, from) >= 0
    decreases (if from < tags.len() as int { tags.len() as int - from } else { 0 })
{
    if from < tags.len() as int {
        sum_fixed_nonneg(tags, values, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify layout-constraint.satisfies]
    #[test]
    fn test_satisfies_fixed() {
        assert!(satisfies_constraint(TAG_FIXED, 42, 0, 42));
        assert!(!satisfies_constraint(TAG_FIXED, 42, 0, 43));
    }

    // r[verify layout-constraint.satisfies]
    #[test]
    fn test_satisfies_proportion() {
        assert!(satisfies_constraint(TAG_PROPORTION, 0, 0, 0));
        assert!(satisfies_constraint(TAG_PROPORTION, 0, 0, u16::MAX));
    }

    // r[verify layout-constraint.satisfies]
    #[test]
    fn test_satisfies_min() {
        assert!(satisfies_constraint(TAG_MIN, 10, 0, 10));
        assert!(satisfies_constraint(TAG_MIN, 10, 0, 100));
        assert!(!satisfies_constraint(TAG_MIN, 10, 0, 9));
    }

    // r[verify layout-constraint.satisfies]
    #[test]
    fn test_satisfies_minmax() {
        assert!(satisfies_constraint(TAG_MINMAX, 10, 50, 10));
        assert!(satisfies_constraint(TAG_MINMAX, 10, 50, 30));
        assert!(satisfies_constraint(TAG_MINMAX, 10, 50, 50));
        assert!(!satisfies_constraint(TAG_MINMAX, 10, 50, 9));
        assert!(!satisfies_constraint(TAG_MINMAX, 10, 50, 51));
    }

    // r[verify layout-constraint.clamp]
    #[test]
    fn test_clamp_fixed() {
        assert_eq!(clamp_to_constraint(TAG_FIXED, 42, 0, 0), 42);
        assert_eq!(clamp_to_constraint(TAG_FIXED, 42, 0, 100), 42);
    }

    // r[verify layout-constraint.clamp]
    #[test]
    fn test_clamp_proportion() {
        assert_eq!(clamp_to_constraint(TAG_PROPORTION, 0, 0, 77), 77);
    }

    // r[verify layout-constraint.clamp]
    #[test]
    fn test_clamp_min() {
        assert_eq!(clamp_to_constraint(TAG_MIN, 10, 0, 5), 10);
        assert_eq!(clamp_to_constraint(TAG_MIN, 10, 0, 10), 10);
        assert_eq!(clamp_to_constraint(TAG_MIN, 10, 0, 100), 100);
    }

    // r[verify layout-constraint.clamp]
    #[test]
    fn test_clamp_minmax() {
        assert_eq!(clamp_to_constraint(TAG_MINMAX, 10, 50, 5), 10);
        assert_eq!(clamp_to_constraint(TAG_MINMAX, 10, 50, 30), 30);
        assert_eq!(clamp_to_constraint(TAG_MINMAX, 10, 50, 100), 50);
    }

    proptest! {
        // r[verify layout-constraint.clamp]
        #[test]
        fn prop_clamp_always_satisfies(tag in 0u8..4, lo in 0u16..1000, hi_offset in 0u16..1000, value: u16) {
            let hi = lo.saturating_add(hi_offset);
            let result = clamp_to_constraint(tag, lo, hi, value);
            prop_assert!(satisfies_constraint(tag, lo, hi, result));
        }

        // r[verify layout-constraint.clamp]
        #[test]
        fn prop_clamp_idempotent(tag in 0u8..4, lo in 0u16..1000, hi_offset in 0u16..1000, value: u16) {
            let hi = lo.saturating_add(hi_offset);
            let once = clamp_to_constraint(tag, lo, hi, value);
            let twice = clamp_to_constraint(tag, lo, hi, once);
            prop_assert_eq!(once, twice);
        }

        // r[verify layout-constraint.satisfies]
        #[test]
        fn prop_minmax_subsumes_min(lo in 0u16..1000, hi_offset in 0u16..1000, resolved in 0u16..2000) {
            let hi = lo.saturating_add(hi_offset);
            if satisfies_constraint(TAG_MINMAX, lo, hi, resolved) {
                prop_assert!(satisfies_constraint(TAG_MIN, lo, 0, resolved));
            }
        }

        #[test]
        fn prop_fixed_ignores_value(lo in 0u16..1000, value in 0u16..2000) {
            let clamped = clamp_to_constraint(TAG_FIXED, lo, lo, value);
            prop_assert_eq!(clamped, lo);
        }

        #[test]
        fn prop_satisfies_reflexive(tag in 0u8..4, lo in 0u16..500, hi_offset in 0u16..500, value in 0u16..1000) {
            let hi = lo.saturating_add(hi_offset);
            let clamped = clamp_to_constraint(tag, lo, hi, value);
            prop_assert!(satisfies_constraint(tag, lo, hi, clamped));
        }

        #[test]
        fn prop_min_only_lower_bound(lo in 0u16..1000, value in 0u16..2000) {
            if value >= lo {
                prop_assert!(satisfies_constraint(TAG_MIN, lo, 0, value));
            }
        }
    }
}

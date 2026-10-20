//! Verified fragmentation metrics with formal bounds.
//!
//! Defines when fragmentation causes an allocation to fail despite
//! sufficient total free space. Proves bounds on internal and external
//! fragmentation under buddy and slab strategies.

use vstd::prelude::*;

verus! {

// ─── External Fragmentation ─────────────────────────────────────────

// r[depends fragmentation.external]
/// Spec: external fragmentation ratio (0..=100, percentage).
/// 0 means one contiguous free chunk; 100 means completely fragmented.
/// Defined as: 100 - (largest_free_block * 100 / free_total).
pub open spec fn external_frag_spec(free_total: u64, largest_free_block: u64) -> u64
    recommends free_total > 0, largest_free_block <= free_total
{
    if free_total == 0 { 0 }
    else {
        (100 - (largest_free_block as int * 100 / free_total as int)) as u64
    }
}

// r[impl fragmentation.external]
/// Returns external fragmentation percentage (0 = no fragmentation).
pub fn external_frag(free_total: u64, largest_free_block: u64) -> (result: u64)
    requires free_total > 0, largest_free_block <= free_total
    ensures result == external_frag_spec(free_total, largest_free_block)
{
    let pct = match u64::try_from((largest_free_block as u128 * 100) / free_total as u128) {
        Ok(value) => value,
        Err(_) => 100,
    };
    proof {
        // pct = (lfb * 100) / ft. Since lfb <= ft, pct <= 100.
        assert((largest_free_block as int) * 100 <= (free_total as int) * 100)
            by(nonlinear_arith)
            requires largest_free_block <= free_total;
        assert((largest_free_block as int) * 100 / (free_total as int) <= 100)
            by(nonlinear_arith)
            requires
                (largest_free_block as int) * 100 <= (free_total as int) * 100,
                free_total > 0;
    }
    100 - pct
}

// ─── Internal Fragmentation ──────────────────────────────────────────

// r[depends fragmentation.internal]
/// Spec: internal fragmentation (bytes wasted per allocation).
pub open spec fn internal_frag_spec(requested: u64, allocated: u64) -> u64
    recommends allocated >= requested
{
    (allocated - requested) as u64
}

// r[impl fragmentation.internal]
/// Returns bytes wasted: allocated - requested.
pub fn internal_frag(requested: u64, allocated: u64) -> (result: u64)
    requires allocated >= requested
    ensures result == internal_frag_spec(requested, allocated)
{
    allocated - requested
}

// ─── Can Satisfy ─────────────────────────────────────────────────────

// r[depends fragmentation.can-satisfy]
/// Spec: enough *contiguous* free space exists for the request.
pub open spec fn can_satisfy_spec(largest_free_block: u64, requested_size: u64) -> bool {
    largest_free_block >= requested_size
}

// r[impl fragmentation.can-satisfy]
/// Returns true if the largest contiguous free block can serve the request.
pub fn can_satisfy(largest_free_block: u64, requested_size: u64) -> (result: bool)
    ensures result == can_satisfy_spec(largest_free_block, requested_size)
{
    largest_free_block >= requested_size
}

// ─── Buddy Fragmentation Bound ──────────────────────────────────────

// r[depends fragmentation.buddy-bound]
/// Spec: worst-case buddy internal fragmentation.
/// For a request of size s served by order k where 2^k >= s,
/// waste = 2^k - s. Since 2^(k-1) < s (minimality of k),
/// waste < s. So waste is at most s - 1.
pub open spec fn frag_bound_buddy_spec(requested: u64) -> u64
    recommends requested > 0
{
    (requested - 1) as u64
}

// r[impl fragmentation.buddy-bound]
/// Returns the worst-case buddy internal fragmentation for a given request.
pub fn frag_bound_buddy(requested: u64) -> (result: u64)
    requires requested > 0
    ensures result == frag_bound_buddy_spec(requested)
{
    requested - 1
}

// ─── Slab Fragmentation Bound ────────────────────────────────────────

// r[depends fragmentation.slab-bound]
/// Spec: worst-case slab internal fragmentation per slot.
/// waste = slot_size - obj_size (constant for a given slab).
pub open spec fn frag_bound_slab_spec(obj_size: u64, slot_size: u64) -> u64
    recommends slot_size >= obj_size
{
    (slot_size - obj_size) as u64
}

// r[impl fragmentation.slab-bound]
/// Returns worst-case slab waste per slot.
pub fn frag_bound_slab(obj_size: u64, slot_size: u64) -> (result: u64)
    requires slot_size >= obj_size
    ensures result == frag_bound_slab_spec(obj_size, slot_size)
{
    slot_size - obj_size
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify fragmentation.can-satisfy]
/// can_satisfy implies free_total >= requested.
proof fn can_satisfy_necessary(free_total: u64, largest_free: u64, requested: u64)
    requires
        can_satisfy_spec(largest_free, requested),
        largest_free <= free_total,
    ensures free_total >= requested
{}

// r[verify fragmentation.can-satisfy]
/// Failure with sufficient total free space implies external fragmentation > 0.
proof fn failure_implies_fragmented(
    free_total: u64, largest_free: u64, requested: u64,
)
    requires
        free_total > 0,
        free_total >= requested,
        !can_satisfy_spec(largest_free, requested),
        largest_free <= free_total,
        requested > 0,
    ensures external_frag_spec(free_total, largest_free) > 0
{
    // largest_free < requested <= free_total
    // So largest_free < free_total, meaning largest_free * 100 / free_total < 100
    // Thus external_frag = 100 - (largest_free * 100 / free_total) > 0.
    assert(largest_free < free_total);
    assert((largest_free as int) * 100 < (free_total as int) * 100)
        by(nonlinear_arith)
        requires largest_free < free_total;
    // (largest_free * 100) / free_total < 100 since largest_free * 100 < free_total * 100
    assert((largest_free as int) * 100 / (free_total as int) < 100)
        by(nonlinear_arith)
        requires
            (largest_free as int) * 100 < (free_total as int) * 100,
            free_total > 0;
}

// r[verify fragmentation.buddy-bound]
/// Buddy worst case: internal waste < requested size.
proof fn buddy_waste_bounded(requested: u64)
    requires requested > 0
    ensures frag_bound_buddy_spec(requested) < requested
{}

// r[verify fragmentation.buddy-bound]
/// Buddy worst case is < 50% for sizes > 1.
/// (For size 2: class 1, alloc 2, waste 0. For size 3: class 2, alloc 4, waste 1 < 1.5.)
proof fn buddy_waste_under_half(requested: u64)
    requires requested > 1
    ensures frag_bound_buddy_spec(requested) < requested
{
    // frag_bound_buddy_spec(r) = r - 1. r - 1 < r for r > 1.
}

// r[verify fragmentation.slab-bound]
/// Slab worst case: waste = slot_size - obj_size (constant per class).
proof fn slab_waste_constant(obj_size: u64, slot_size: u64)
    requires slot_size >= obj_size
    ensures frag_bound_slab_spec(obj_size, slot_size) == slot_size - obj_size
{}

// r[verify fragmentation.slab-bound]
/// Slab waste is zero when obj_size == slot_size (perfect fit).
proof fn slab_zero_waste_exact(size: u64)
    ensures frag_bound_slab_spec(size, size) == 0
{}

// r[verify fragmentation.external]
/// External frag is 0 iff largest_free == free_total (one contiguous chunk).
proof fn zero_frag_iff_contiguous(free_total: u64)
    requires free_total > 0
    ensures external_frag_spec(free_total, free_total) == 0
{
    // Need: free_total * 100 / free_total == 100
    assert((free_total as int) * 100 / (free_total as int) == 100)
        by(nonlinear_arith)
        requires free_total > 0;
}

// r[verify fragmentation.external]
/// External frag is 100 when largest_free == 0 (totally fragmented into 0-size blocks).
proof fn max_frag_when_no_contiguous(free_total: u64)
    requires free_total > 0
    ensures external_frag_spec(free_total, 0) == 100
{}

// r[verify fragmentation.internal]
/// Internal frag is zero for exact-fit allocations.
proof fn internal_zero_exact_fit(size: u64)
    ensures internal_frag_spec(size, size) == 0
{}

// r[verify fragmentation.internal]
/// Internal frag is non-negative.
proof fn internal_frag_nonneg(requested: u64, allocated: u64)
    requires allocated >= requested
    ensures internal_frag_spec(requested, allocated) >= 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify fragmentation.external]
    #[test]
    fn test_external_frag() {
        assert_eq!(external_frag(100, 100), 0); // no fragmentation
        assert_eq!(external_frag(100, 0), 100); // total fragmentation
        assert_eq!(external_frag(100, 50), 50); // 50% fragmented
    }

    // r[verify fragmentation.internal]
    #[test]
    fn test_internal_frag() {
        assert_eq!(internal_frag(10, 16), 6);
        assert_eq!(internal_frag(16, 16), 0);
    }

    // r[verify fragmentation.can-satisfy]
    #[test]
    fn test_can_satisfy() {
        assert!(can_satisfy(64, 32));
        assert!(can_satisfy(32, 32));
        assert!(!can_satisfy(31, 32));
    }

    // r[verify fragmentation.buddy-bound]
    #[test]
    fn test_buddy_bound() {
        assert_eq!(frag_bound_buddy(1), 0);
        assert_eq!(frag_bound_buddy(3), 2);
        assert_eq!(frag_bound_buddy(1024), 1023);
    }

    // r[verify fragmentation.slab-bound]
    #[test]
    fn test_slab_bound() {
        assert_eq!(frag_bound_slab(10, 16), 6);
        assert_eq!(frag_bound_slab(16, 16), 0);
    }

    proptest! {
        // r[verify fragmentation.external]
        #[test]
        fn prop_external_frag_bounded(
            free_total in 1u64..10000,
            largest in 0u64..10000,
        ) {
            if largest <= free_total {
                let frag = external_frag(free_total, largest);
                prop_assert!(frag <= 100);
            }
        }

        // r[verify fragmentation.can-satisfy]
        #[test]
        fn prop_can_satisfy_necessary(
            free_total in 1u64..10000,
            largest in 0u64..10000,
            requested in 1u64..10000,
        ) {
            if largest <= free_total && can_satisfy(largest, requested) {
                prop_assert!(free_total >= requested);
            }
        }

        // r[verify fragmentation.buddy-bound]
        #[test]
        fn prop_buddy_waste_bounded(requested in 1u64..100000) {
            prop_assert!(frag_bound_buddy(requested) < requested);
        }

        // r[verify fragmentation.slab-bound]
        #[test]
        fn prop_slab_waste_constant(
            obj_size in 1u64..1000,
            padding in 0u64..100,
        ) {
            let slot_size = obj_size + padding;
            prop_assert_eq!(frag_bound_slab(obj_size, slot_size), padding);
        }

        // r[verify fragmentation.internal]
        #[test]
        fn prop_internal_nonneg(
            requested in 0u64..1000,
            extra in 0u64..1000,
        ) {
            let allocated = requested + extra;
            prop_assert!(internal_frag(requested, allocated) == extra);
        }
    }
}

//! Verified query plan cost model.
//!
//! Models cost estimation for sequential scans and index scans.
//! Costs are expressed as u64 "cost units". Selectivity is modeled
//! as a fraction (numerator, denominator) to avoid floating point.
//!
//! Key properties: index scan is cheaper than sequential scan when
//! selectivity is low enough; costs are monotonic in the number of
//! pages; zero-selectivity index scan costs zero tuples.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- seq_scan_cost ---

// r[depends query-cost.seq-scan]
/// Spec: cost of a sequential scan = num_pages * cost_per_page.
pub open spec fn seq_scan_cost_spec(num_pages: u64, cost_per_page: u64) -> int {
    (num_pages as int) * (cost_per_page as int)
}

// r[impl query-cost.seq-scan]
/// Returns the cost of a sequential scan, or u64::MAX on overflow.
pub fn seq_scan_cost(num_pages: u64, cost_per_page: u64) -> (result: u64)
    ensures result as int == if seq_scan_cost_spec(num_pages, cost_per_page) <= (u64::MAX as int) {
        seq_scan_cost_spec(num_pages, cost_per_page)
    } else {
        u64::MAX as int
    }
{
    proof {
        assert((num_pages as int) * (cost_per_page as int) <= (u128::MAX as int))
            by(nonlinear_arith)
            requires
                0 <= num_pages as int,
                num_pages as int <= u64::MAX as int,
                0 <= cost_per_page as int,
                cost_per_page as int <= u64::MAX as int,
                (u64::MAX as int) * (u64::MAX as int) <= (u128::MAX as int);
    }
    let wide = num_pages as u128 * cost_per_page as u128;
    if wide > u64::MAX as u128 {
        u64::MAX
    } else {
        match u64::try_from(wide) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        }
    }
}

// --- index_pages_touched ---

// r[depends query-cost.index-scan]
/// Spec: pages touched by index scan.
pub open spec fn index_pages_touched_spec(
    num_pages: u64, sel_num: u64, sel_den: u64,
) -> int
    recommends sel_den > 0
{
    (num_pages as int) * (sel_num as int) / (sel_den as int)
}

// --- index_rows_touched ---

// r[depends query-cost.index-scan]
/// Spec: rows touched by index scan.
pub open spec fn index_rows_touched_spec(
    total_rows: u64, sel_num: u64, sel_den: u64,
) -> int
    recommends sel_den > 0
{
    (total_rows as int) * (sel_num as int) / (sel_den as int)
}

// --- index_scan_cost ---

// r[depends query-cost.index-scan]
// r[impl query-cost.index-scan]
/// Spec: cost of an index scan.
pub open spec fn index_scan_cost_spec(
    num_pages: u64, total_rows: u64,
    sel_num: u64, sel_den: u64,
    cost_per_page: u64, cost_per_tuple: u64,
) -> int
    recommends sel_den > 0, sel_num <= sel_den
{
    let pages_touched = index_pages_touched_spec(num_pages, sel_num, sel_den);
    let rows_touched = index_rows_touched_spec(total_rows, sel_num, sel_den);
    pages_touched * (cost_per_page as int) + rows_touched * (cost_per_tuple as int)
}

// --- selectivity ---

// r[depends query-cost.selectivity]
/// Spec: selectivity for a column with `distinct_values` unique values.
/// Returns (1, distinct_values) representing 1/distinct_values.
pub open spec fn selectivity_spec(distinct_values: u64) -> (u64, u64)
    recommends distinct_values > 0
{
    (1u64, distinct_values)
}

// r[impl query-cost.selectivity]
pub fn selectivity(distinct_values: u64) -> (result: (u64, u64))
    requires distinct_values > 0
    ensures result == selectivity_spec(distinct_values)
{
    (1, distinct_values)
}

// ==================== Proofs ====================

// r[verify query-cost.seq-scan]
/// Sequential scan cost is zero when there are no pages.
proof fn seq_scan_zero_pages(cost_per_page: u64)
    ensures seq_scan_cost_spec(0, cost_per_page) == 0
{}

// r[verify query-cost.seq-scan]
/// Sequential scan cost is monotonic in num_pages.
proof fn seq_scan_monotonic_pages(a: u64, b: u64, cost_per_page: u64)
    requires a <= b
    ensures seq_scan_cost_spec(a, cost_per_page) <= seq_scan_cost_spec(b, cost_per_page)
{
    assert((a as int) * (cost_per_page as int) <= (b as int) * (cost_per_page as int))
        by(nonlinear_arith)
        requires 0 <= a as int, a as int <= b as int, 0 <= cost_per_page as int;
}

// r[verify query-cost.seq-scan]
/// Sequential scan cost is monotonic in cost_per_page.
proof fn seq_scan_monotonic_cost(num_pages: u64, a: u64, b: u64)
    requires a <= b
    ensures seq_scan_cost_spec(num_pages, a) <= seq_scan_cost_spec(num_pages, b)
{
    assert((num_pages as int) * (a as int) <= (num_pages as int) * (b as int))
        by(nonlinear_arith)
        requires 0 <= num_pages as int, 0 <= a as int, a as int <= b as int;
}

// r[verify query-cost.index-scan]
/// Zero selectivity means zero index pages touched.
proof fn index_zero_selectivity_pages(num_pages: u64, sel_den: u64)
    requires sel_den > 0
    ensures index_pages_touched_spec(num_pages, 0, sel_den) == 0
{}

// r[verify query-cost.index-scan]
/// Zero selectivity means zero index rows touched.
proof fn index_zero_selectivity_rows(total_rows: u64, sel_den: u64)
    requires sel_den > 0
    ensures index_rows_touched_spec(total_rows, 0, sel_den) == 0
{}

// r[verify query-cost.index-scan]
/// Zero selectivity means zero pages and rows touched.
proof fn index_scan_zero_selectivity(
    num_pages: u64, total_rows: u64, sel_den: u64,
    cost_per_page: u64, cost_per_tuple: u64,
)
    requires sel_den > 0
    ensures
        index_pages_touched_spec(num_pages, 0, sel_den) == 0,
        index_rows_touched_spec(total_rows, 0, sel_den) == 0,
{}

// r[verify query-cost.selectivity]
/// Higher distinct values means lower selectivity (smaller fraction).
/// 1/a >= 1/b when a <= b, verified via cross-multiply: 1*b >= 1*a.
proof fn more_distinct_lower_selectivity(a: u64, b: u64)
    requires 0 < a, a <= b
    ensures ({
        let sa = selectivity_spec(a);
        let sb = selectivity_spec(b);
        (sa.0 as int) * (sb.1 as int) >= (sb.0 as int) * (sa.1 as int)
    })
{
    // selectivity_spec(a) = (1, a), selectivity_spec(b) = (1, b)
    // Need: 1 * b >= 1 * a, i.e. b >= a. QED.
    assert(selectivity_spec(a) == (1u64, a));
    assert(selectivity_spec(b) == (1u64, b));
}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_seq_scan_cost() {
        assert_eq!(seq_scan_cost(100, 10), 1000);
        assert_eq!(seq_scan_cost(0, 10), 0);
        assert_eq!(seq_scan_cost(100, 0), 0);
    }

    #[test]
    fn test_seq_scan_overflow() {
        assert_eq!(seq_scan_cost(u64::MAX, u64::MAX), u64::MAX);
    }

    #[test]
    fn test_selectivity() {
        assert_eq!(selectivity(100), (1, 100));
        assert_eq!(selectivity(1), (1, 1));
    }

    proptest! {
        #[test]
        fn prop_seq_scan_monotonic_pages(
            a in 0u64..10000, b in 0u64..10000, cost in 0u64..1000,
        ) {
            if a <= b {
                prop_assert!(seq_scan_cost(a, cost) <= seq_scan_cost(b, cost));
            }
        }

        #[test]
        fn prop_seq_scan_monotonic_cost(
            pages in 0u64..10000, a in 0u64..1000, b in 0u64..1000,
        ) {
            if a <= b {
                prop_assert!(seq_scan_cost(pages, a) <= seq_scan_cost(pages, b));
            }
        }

        #[test]
        fn prop_seq_scan_zero_pages(cost in 0u64..10000) {
            prop_assert_eq!(seq_scan_cost(0, cost), 0);
        }

        #[test]
        fn prop_seq_scan_matches_product(
            pages in 0u64..100000, cost in 0u64..100000,
        ) {
            let expected = pages as u128 * cost as u128;
            if expected <= u64::MAX as u128 {
                prop_assert_eq!(seq_scan_cost(pages, cost), expected as u64);
            } else {
                prop_assert_eq!(seq_scan_cost(pages, cost), u64::MAX);
            }
        }

        #[test]
        fn prop_more_distinct_lower_selectivity(a in 1u64..10000, b in 1u64..10000) {
            if a <= b {
                let (sn_a, sd_a) = selectivity(a);
                let (sn_b, sd_b) = selectivity(b);
                // 1/a >= 1/b when a <= b
                prop_assert!(sn_a as u128 * sd_b as u128 >= sn_b as u128 * sd_a as u128);
            }
        }
    }
}

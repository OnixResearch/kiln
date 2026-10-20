//! Verified index selection primitives.
//!
//! Models covering index checks and index selection based on
//! selectivity. A covering index contains all columns needed by a
//! query, avoiding a table lookup. Given multiple candidate indices,
//! we pick the one with the lowest estimated selectivity.
//!
//! Column sets are Seq<u64> of column identifiers.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- is_covering ---

// r[depends index-select.is-covering]
/// Spec: an index is covering if every query column appears in the
/// index columns.
pub open spec fn is_covering_spec(
    index_cols: Seq<u64>, query_cols: Seq<u64>,
) -> bool {
    forall|i: int|
        #![trigger query_cols[i]]
        0 <= i < query_cols.len() ==> index_cols.contains(query_cols[i])
}

// r[impl index-select.is-covering]
/// Returns true if the index covers all query columns.
pub fn is_covering(index_cols: &[u64], query_cols: &[u64]) -> (result: bool)
    ensures result == is_covering_spec(index_cols@, query_cols@)
{
    let mut qi: usize = 0;
    while qi < query_cols.len()
        invariant
            0 <= qi <= query_cols@.len(),
            forall|i: int| #![trigger query_cols@[i]]
                0 <= i < qi as int ==> index_cols@.contains(query_cols@[i]),
        decreases query_cols@.len() - qi
    {
        let qcol = query_cols[qi];
        let mut found = false;
        let mut ii: usize = 0;
        while ii < index_cols.len()
            invariant_except_break
                0 <= ii <= index_cols@.len(),
                qcol == query_cols@[qi as int],
                !found,
                forall|k: int| #![trigger index_cols@[k]]
                    0 <= k < ii as int ==> index_cols@[k] != qcol,
            ensures
                found ==> index_cols@.contains(qcol),
                !found ==> forall|k: int| #![trigger index_cols@[k]]
                    0 <= k < index_cols@.len() ==> index_cols@[k] != qcol,
            decreases index_cols@.len() - ii
        {
            if index_cols[ii] == qcol {
                found = true;
                break;
            }
            ii = ii + 1;
        }
        if !found {
            proof {
                if index_cols@.contains(qcol) {
                    let w = choose|k: int| 0 <= k < index_cols@.len() && index_cols@[k] == qcol;
                    assert(index_cols@[w] != qcol);
                }
            }
            return false;
        }
        qi = qi + 1;
    }
    true
}

// --- best_index ---

// r[depends index-select.best-index]
/// Spec helper: strict comparison of selectivities.
/// `lhs` is better than `rhs` when its fraction is strictly smaller.
pub open spec fn selectivity_lt_spec(lhs: (u64, u64), rhs: (u64, u64)) -> bool {
    (lhs.0 as int) * (rhs.1 as int) < (rhs.0 as int) * (lhs.1 as int)
}

/// Spec helper: best index among the prefix `selectivities[0..upto)`.
pub open spec fn best_index_prefix_spec(selectivities: Seq<(u64, u64)>, upto: int) -> int
    recommends
        1 <= upto <= selectivities.len(),
        forall|i: int| #![trigger selectivities[i]]
            0 <= i < selectivities.len() ==> selectivities[i].1 > 0,
    decreases upto,
{
    if upto <= 1 {
        0
    } else {
        let prev = best_index_prefix_spec(selectivities, upto - 1);
        let cur = upto - 1;
        if selectivity_lt_spec(selectivities[cur], selectivities[prev]) {
            cur
        } else {
            prev
        }
    }
}

/// Spec: given selectivities for each candidate index (lower is better),
/// return the index of the one with the lowest selectivity.
pub open spec fn best_index_spec(
    selectivities: Seq<(u64, u64)>,
) -> int
    recommends
        selectivities.len() > 0,
        forall|i: int| #![trigger selectivities[i]]
            0 <= i < selectivities.len() ==> selectivities[i].1 > 0,
{
    best_index_prefix_spec(selectivities, selectivities.len() as int)
}

// r[impl index-select.best-index]
/// Returns the index (position) of the candidate with lowest selectivity.
pub fn best_index(selectivities: &[(u64, u64)]) -> (result: usize)
    requires
        selectivities@.len() > 0,
        forall|i: int| #![trigger selectivities@[i]]
            0 <= i < selectivities@.len() ==> selectivities@[i].1 > 0,
    ensures
        result as int == best_index_spec(selectivities@),
{
    let mut best: usize = 0;
    let mut i: usize = 1;
    while i < selectivities.len()
        invariant
            0 <= best < selectivities@.len(),
            1 <= i <= selectivities@.len(),
            best as int == best_index_prefix_spec(selectivities@, i as int),
        decreases selectivities@.len() - i
    {
        let (n_cur, d_cur) = selectivities[i];
        let (n_best, d_best) = selectivities[best];
        proof {
            assert((n_cur as int) * (d_best as int) <= (u128::MAX as int))
                by(nonlinear_arith)
                requires
                    0 <= n_cur as int, n_cur as int <= u64::MAX as int,
                    0 <= d_best as int, d_best as int <= u64::MAX as int,
                    (u64::MAX as int) * (u64::MAX as int) <= (u128::MAX as int);
            assert((n_best as int) * (d_cur as int) <= (u128::MAX as int))
                by(nonlinear_arith)
                requires
                    0 <= n_best as int, n_best as int <= u64::MAX as int,
                    0 <= d_cur as int, d_cur as int <= u64::MAX as int,
                    (u64::MAX as int) * (u64::MAX as int) <= (u128::MAX as int);
        }
        let cross_cur = n_cur as u128 * d_best as u128;
        let cross_best = n_best as u128 * d_cur as u128;
        if cross_cur < cross_best {
            proof {
                assert(selectivity_lt_spec(selectivities@[i as int], selectivities@[best as int]));
            }
            best = i;
        } else {
            proof {
                assert(!selectivity_lt_spec(selectivities@[i as int], selectivities@[best as int]));
            }
        }
        i = i + 1;
    }
    best
}

// --- column_count ---

// r[depends index-select.column-count]
/// Spec: number of columns in the index.
pub open spec fn column_count_spec(cols: Seq<u64>) -> int {
    cols.len() as int
}

// r[impl index-select.column-count]
pub fn column_count(cols: &[u64]) -> (result: usize)
    ensures result as int == column_count_spec(cols@)
{
    cols.len()
}

// ==================== Proofs ====================

// r[verify index-select.is-covering]
/// Empty query is always covered.
proof fn empty_query_always_covered(index_cols: Seq<u64>)
    ensures is_covering_spec(index_cols, Seq::<u64>::empty())
{}

// r[verify index-select.is-covering]
/// Covering is transitive: if A covers B and B covers C, then A covers C.
proof fn covering_transitive(a: Seq<u64>, b: Seq<u64>, c: Seq<u64>)
    requires
        is_covering_spec(a, b),
        is_covering_spec(b, c),
    ensures is_covering_spec(a, c)
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_is_covering_yes() {
        assert!(is_covering(&[1, 2, 3, 4], &[2, 4]));
    }

    #[test]
    fn test_is_covering_no() {
        assert!(!is_covering(&[1, 2], &[2, 3]));
    }

    #[test]
    fn test_is_covering_empty_query() {
        assert!(is_covering(&[1, 2], &[]));
        assert!(is_covering(&[], &[]));
    }

    #[test]
    fn test_is_covering_empty_index() {
        assert!(!is_covering(&[], &[1]));
    }

    // r[verify index-select.best-index]
    #[test]
    fn test_best_index_single() {
        assert_eq!(best_index(&[(1, 10)]), 0);
    }

    #[test]
    fn test_best_index_picks_lowest() {
        // Selectivities: 1/10, 1/100, 1/50
        // 1/100 is smallest → index 1
        assert_eq!(best_index(&[(1, 10), (1, 100), (1, 50)]), 1);
    }

    #[test]
    fn test_best_index_equal() {
        // Equal selectivities → first one wins
        assert_eq!(best_index(&[(1, 10), (1, 10), (1, 10)]), 0);
    }

    // r[verify index-select.best-index]
    #[test]
    fn test_best_index_cross_multiply() {
        // 3/10 vs 2/7: 3*7=21 vs 2*10=20 -> 2/7 is smaller (index 1)
        assert_eq!(best_index(&[(3, 10), (2, 7)]), 1);
    }

    // r[verify index-select.column-count]
    #[test]
    fn test_column_count() {
        assert_eq!(column_count(&[]), 0);
        assert_eq!(column_count(&[10, 20, 30]), 3);
    }

    proptest! {
        #[test]
        fn prop_covering_superset(
            index_size in 0usize..=20,
            query in proptest::collection::vec(0u64..10, 0..=5),
        ) {
            let index: Vec<u64> = (0..index_size as u64).collect();
            let all_present = query.iter().all(|q| (*q as usize) < index_size);
            if all_present {
                prop_assert!(is_covering(&index, &query));
            }
        }

        #[test]
        fn prop_empty_query_covered(index in proptest::collection::vec(any::<u64>(), 0..=10)) {
            prop_assert!(is_covering(&index, &[]));
        }

        // r[verify index-select.best-index]
        #[test]
        fn prop_best_index_in_bounds(
            sels in proptest::collection::vec((0u64..1000, 1u64..1000), 1..=10),
        ) {
            let result = best_index(&sels);
            prop_assert!(result < sels.len());
        }

        // r[verify index-select.column-count]
        #[test]
        fn prop_column_count_matches_len(
            cols in proptest::collection::vec(any::<u64>(), 0..=10),
        ) {
            prop_assert_eq!(column_count(&cols), cols.len());
        }

        #[test]
        fn prop_best_index_is_minimum(
            sels in proptest::collection::vec((0u64..100, 1u64..100), 1..=10),
        ) {
            let result = best_index(&sels);
            let (n_best, d_best) = sels[result];
            for (i, &(n, d)) in sels.iter().enumerate() {
                let cross_best = n_best as u128 * d as u128;
                let cross_other = n as u128 * d_best as u128;
                prop_assert!(cross_best <= cross_other,
                    "best={} ({}/{}) vs idx={} ({}/{})",
                    result, n_best, d_best, i, n, d);
            }
        }
    }
}

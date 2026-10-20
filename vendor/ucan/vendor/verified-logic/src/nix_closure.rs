//! Verified Nix generic closure.
//!
//! Models `builtins.genericClosure`: starting from a set of items,
//! repeatedly apply a finite operator table to produce new items until no
//! new keys appear.

#![allow(clippy::ptr_arg)]

use std::collections::HashSet;

use vstd::prelude::*;

verus! {

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureItem {
    /// Unique identifier for the closure item.
    pub key: u64,
    /// Associated data for the closure item.
    pub data: u64,
}

pub open spec fn operator_view_spec(
    operator: Seq<(u64, Vec<ClosureItem>)>,
) -> Seq<(u64, Seq<ClosureItem>)> {
    operator.map_values(|entry: (u64, Vec<ClosureItem>)| (entry.0, entry.1@))
}

/// Spec: does the result contain a key?
pub open spec fn closure_contains_key(result: Seq<ClosureItem>, key: u64) -> bool {
    exists|i: int| 0 <= i < result.len() && result[i].key == key
}

/// Spec: are all keys unique?
pub open spec fn keys_unique(items: Seq<ClosureItem>) -> bool {
    forall|i: int, j: int| 0 <= i < j < items.len() ==> items[i].key != items[j].key
}

pub open spec fn operator_produces_key_spec(
    operator: Seq<(u64, Seq<ClosureItem>)>,
    from_key: u64,
    to_key: u64,
) -> bool {
    exists|i: int, j: int|
        0 <= i < operator.len()
        && operator[i].0 == from_key
        && 0 <= j < operator[i].1.len()
        && operator[i].1[j].key == to_key
}

pub open spec fn reachable_key_fuel_spec(
    start: Seq<ClosureItem>,
    operator: Seq<(u64, Seq<ClosureItem>)>,
    key: u64,
    fuel: nat,
) -> bool
    decreases fuel,
{
    closure_contains_key(start, key)
    || (fuel > 0
        && exists|parent: u64|
            reachable_key_fuel_spec(start, operator, parent, (fuel - 1) as nat)
            && operator_produces_key_spec(operator, parent, key))
}

pub open spec fn reachable_key_spec(
    start: Seq<ClosureItem>,
    operator: Seq<(u64, Seq<ClosureItem>)>,
    key: u64,
) -> bool {
    exists|fuel: nat| reachable_key_fuel_spec(start, operator, key, fuel)
}

// r[depends nix-closure.generic-closure]
/// Spec: `result` is a unique-key closure for `start` under `operator`.
pub open spec fn generic_closure_spec(
    start: Seq<ClosureItem>,
    operator: Seq<(u64, Seq<ClosureItem>)>,
    result: Seq<ClosureItem>,
) -> bool {
    &&& keys_unique(result)
    &&& forall|i: int| 0 <= i < start.len() ==> closure_contains_key(result, start[i].key)
    &&& forall|i: int| 0 <= i < result.len() ==> reachable_key_spec(start, operator, result[i].key)
    &&& forall|oi: int, cj: int|
            0 <= oi < operator.len()
            && closure_contains_key(result, operator[oi].0)
            && 0 <= cj < operator[oi].1.len()
            ==> closure_contains_key(result, operator[oi].1[cj].key)
}

// r[verify nix-closure.generic-closure]
proof fn reachable_start_key(
    start: Seq<ClosureItem>,
    operator: Seq<(u64, Seq<ClosureItem>)>,
    key: u64,
)
    requires closure_contains_key(start, key)
    ensures reachable_key_spec(start, operator, key)
{
    assert(reachable_key_fuel_spec(start, operator, key, 0));
}

// r[verify nix-closure.generic-closure]
proof fn reachable_via_output(
    start: Seq<ClosureItem>,
    operator: Seq<(u64, Seq<ClosureItem>)>,
    parent_key: u64,
    child_key: u64,
)
    requires
        reachable_key_spec(start, operator, parent_key),
        operator_produces_key_spec(operator, parent_key, child_key),
    ensures reachable_key_spec(start, operator, child_key)
{
    let fuel = choose|fuel: nat| reachable_key_fuel_spec(start, operator, parent_key, fuel);
    assert(reachable_key_fuel_spec(start, operator, child_key, (fuel + 1) as nat));
}

} // verus!

// r[impl nix-closure.generic-closure]
/// Compute the generic closure from a finite operator table.
///
/// Each table entry `(k, items)` means applying the operator to key `k`
/// yields `items`. Multiple entries with the same key are all applied.
pub fn generic_closure(start: &Vec<ClosureItem>, operator: &Vec<(u64, Vec<ClosureItem>)>) -> Vec<ClosureItem> {
    let mut seen = HashSet::new();
    let mut queue: Vec<ClosureItem> = start.clone();
    let mut result = Vec::new();
    let mut qi: usize = 0;

    while qi < queue.len() {
        let item = queue[qi].clone();
        qi += 1;

        if !seen.insert(item.key) {
            continue;
        }

        for (from_key, outputs) in operator {
            if *from_key == item.key {
                for new_item in outputs {
                    if !seen.contains(&new_item.key) {
                        queue.push(new_item.clone());
                    }
                }
            }
        }

        result.push(item);
    }

    result
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn graph_operator(adj: &[Vec<u64>]) -> Vec<(u64, Vec<ClosureItem>)> {
        let n = adj.len() as u64;
        adj.iter()
            .enumerate()
            .map(|(idx, nbrs)| {
                (
                    idx as u64,
                    nbrs.iter().filter(|&&nbr| nbr < n).map(|&nbr| ClosureItem { key: nbr, data: 0 }).collect(),
                )
            })
            .collect()
    }

    // r[verify nix-closure.generic-closure]
    #[test]
    fn test_generic_closure_simple() {
        let operator = vec![
            (1, vec![ClosureItem { key: 2, data: 20 }, ClosureItem { key: 3, data: 30 }]),
            (2, vec![ClosureItem { key: 3, data: 30 }]),
            (3, vec![]),
        ];
        let result = generic_closure(&vec![ClosureItem { key: 1, data: 10 }], &operator);
        let keys: Vec<u64> = result.iter().map(|i| i.key).collect();
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
        assert_eq!(keys.len(), 3);
    }

    // r[verify nix-closure.generic-closure]
    #[test]
    fn test_generic_closure_no_duplicates() {
        let operator = vec![(1, vec![ClosureItem { key: 1, data: 0 }])];
        let result = generic_closure(&vec![ClosureItem { key: 1, data: 0 }], &operator);
        assert_eq!(result.len(), 1);
    }

    // r[verify nix-closure.generic-closure]
    #[test]
    fn test_generic_closure_empty_start() {
        let result = generic_closure(&vec![], &vec![]);
        assert!(result.is_empty());
    }

    proptest! {
        // r[verify nix-closure.generic-closure]
        #[test]
        fn prop_closure_unique_keys(
            start_keys in proptest::collection::vec(0u64..100, 1..=10),
        ) {
            let start: Vec<ClosureItem> = start_keys.iter()
                .map(|&k| ClosureItem { key: k, data: k })
                .collect();
            let result = generic_closure(&start, &vec![]);
            let keys: Vec<u64> = result.iter().map(|i| i.key).collect();
            let mut sorted = keys.clone();
            sorted.sort_unstable();
            sorted.dedup();
            prop_assert_eq!(keys.len(), sorted.len());
        }

        // r[verify nix-closure.generic-closure]
        #[test]
        fn prop_closure_contains_start(
            start_keys in proptest::collection::vec(0u64..100, 1..=5),
        ) {
            let start: Vec<ClosureItem> = start_keys.iter()
                .map(|&k| ClosureItem { key: k, data: k })
                .collect();
            let result = generic_closure(&start, &vec![]);
            let result_keys: Vec<u64> = result.iter().map(|i| i.key).collect();
            for &k in &start_keys {
                prop_assert!(result_keys.contains(&k));
            }
        }

        // r[verify nix-closure.generic-closure]
        #[test]
        fn prop_closure_is_closed(
            adj in proptest::collection::vec(
                proptest::collection::vec(0u64..10, 0..3), 1..=10
            ),
        ) {
            let n = adj.len() as u64;
            let operator = graph_operator(&adj);
            let start = vec![ClosureItem { key: 0, data: 0 }];
            let result = generic_closure(&start, &operator);
            let result_keys: std::collections::HashSet<u64> =
                result.iter().map(|i| i.key).collect();
            for item in &result {
                if (item.key as usize) < adj.len() {
                    for &nbr in &adj[item.key as usize] {
                        if nbr < n {
                            prop_assert!(result_keys.contains(&nbr));
                        }
                    }
                }
            }
        }

        // r[verify nix-closure.generic-closure]
        #[test]
        fn prop_closure_idempotent(
            adj in proptest::collection::vec(
                proptest::collection::vec(0u64..8, 0..3), 1..=8
            ),
        ) {
            let operator = graph_operator(&adj);
            let start = vec![ClosureItem { key: 0, data: 0 }];
            let first = generic_closure(&start, &operator);
            let second = generic_closure(&first, &operator);
            let mut keys1: Vec<u64> = first.iter().map(|i| i.key).collect();
            let mut keys2: Vec<u64> = second.iter().map(|i| i.key).collect();
            keys1.sort_unstable();
            keys2.sort_unstable();
            prop_assert_eq!(keys1, keys2);
        }

        // r[verify nix-closure.generic-closure]
        #[test]
        fn prop_closure_matches_manual_bfs(
            adj in proptest::collection::vec(
                proptest::collection::vec(0u64..10, 0..3), 1..=10
            ),
        ) {
            let n = adj.len() as u64;
            let operator = graph_operator(&adj);
            let start = vec![ClosureItem { key: 0, data: 0 }];
            let result = generic_closure(&start, &operator);
            let result_keys: std::collections::HashSet<u64> =
                result.iter().map(|i| i.key).collect();
            let mut manual = std::collections::HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            manual.insert(0u64);
            queue.push_back(0u64);
            while let Some(k) = queue.pop_front() {
                if (k as usize) < adj.len() {
                    for &nbr in &adj[k as usize] {
                        if nbr < n && manual.insert(nbr) {
                            queue.push_back(nbr);
                        }
                    }
                }
            }
            prop_assert_eq!(result_keys, manual);
        }
    }
}

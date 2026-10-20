//! Verified intrusive free list with no-cycle and containment proofs.
//!
//! A free list is the fundamental bookkeeping structure for general-purpose
//! allocators. Modeled as a `Seq<u64>` of block addresses with LIFO
//! (stack) semantics. Proofs guarantee no duplicates (acyclicity),
//! bounding region containment, and push/pop correctness.

use vstd::prelude::*;

verus! {

// ─── Spec Predicates ─────────────────────────────────────────────────

// r[depends free-list.all-distinct]
/// Spec: all elements in the list are distinct (no duplicates / no cycles).
pub open spec fn all_distinct_spec(list: Seq<u64>) -> bool {
    forall|i: int, j: int|
        #![trigger list[i], list[j]]
        0 <= i < list.len() && 0 <= j < list.len() && i != j
        ==> list[i] != list[j]
}

// r[depends free-list.all-within]
/// Spec: all entries fall within [0, bound).
pub open spec fn all_within_spec(list: Seq<u64>, bound: u64) -> bool {
    forall|i: int|
        #![trigger list[i]]
        0 <= i < list.len()
        ==> (list[i] as int) < (bound as int)
}

// ─── Contains ────────────────────────────────────────────────────────

// r[depends free-list.contains]
/// Spec: list contains the given block address.
pub open spec fn free_list_contains_spec(list: Seq<u64>, block: u64) -> bool {
    exists|i: int| #![trigger list[i]] 0 <= i < list.len() && list[i] == block
}

// r[impl free-list.contains]
/// Returns true if block is in the list.
pub fn free_list_contains(list: &[u64], block: u64) -> (result: bool)
    ensures result == free_list_contains_spec(list@, block)
{
    let mut i: usize = 0;
    while i < list.len()
        invariant
            0 <= i <= list@.len(),
            forall|j: int| #![trigger list@[j]] 0 <= j < i as int ==> list@[j] != block,
        decreases list@.len() - i,
    {
        if list[i] == block {
            return true;
        }
        i = i + 1;
    }
    false
}

// ─── Length ──────────────────────────────────────────────────────────

// r[depends free-list.len]
/// Spec: number of blocks in the free list.
pub open spec fn free_list_len_spec(list: Seq<u64>) -> int {
    list.len() as int
}

// r[impl free-list.len]
/// Returns the number of blocks in the free list.
pub fn free_list_len(list: &[u64]) -> (result: u64)
    requires list@.len() <= u64::MAX as int
    ensures result as int == free_list_len_spec(list@)
{
    list.len() as u64
}

/// Helper: copy slice suffix starting at `from`.
fn clone_from(list: &[u64], from: usize) -> (result: Vec<u64>)
    requires from <= list@.len()
    ensures result@ == list@.subrange(from as int, list@.len() as int)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = from;
    while i < list.len()
        invariant
            from <= i <= list@.len(),
            result@ == list@.subrange(from as int, i as int),
        decreases list@.len() - i,
    {
        result.push(list[i]);
        i = i + 1;
    }
    result
}

// ─── Push ────────────────────────────────────────────────────────────

// r[depends free-list.push]
/// Spec: prepend block to the list.
pub open spec fn free_list_push_spec(list: Seq<u64>, block: u64) -> Seq<u64> {
    seq![block] + list
}

// r[impl free-list.push]
/// Add block to front of the list. Requires block not already present.
pub fn free_list_push(list: &[u64], block: u64) -> (result: Vec<u64>)
    requires
        !free_list_contains_spec(list@, block),
        list@.len() < u64::MAX as int,
    ensures result@ == free_list_push_spec(list@, block)
{
    let mut result: Vec<u64> = Vec::new();
    result.push(block);
    let mut i: usize = 0;
    while i < list.len()
        invariant
            0 <= i <= list@.len(),
            result@ == seq![block] + list@.subrange(0, i as int),
        decreases list@.len() - i,
    {
        result.push(list[i]);
        i = i + 1;
    }
    result
}

// ─── Pop ─────────────────────────────────────────────────────────────

// r[depends free-list.pop]
/// Spec: remove first block from the list (LIFO).
pub open spec fn free_list_pop_spec(list: Seq<u64>) -> (u64, Seq<u64>)
    recommends list.len() > 0
{
    (list[0], list.subrange(1, list.len() as int))
}

// r[impl free-list.pop]
/// Remove and return the first block.
pub fn free_list_pop(list: &[u64]) -> (result: (u64, Vec<u64>))
    requires list@.len() > 0
    ensures result.0 == free_list_pop_spec(list@).0,
        result.1@ == free_list_pop_spec(list@).1
{
    let block = list[0];
    let rest = clone_from(list, 1);
    (block, rest)
}

// ─── Remove ──────────────────────────────────────────────────────────

/// Spec helper: remove all occurrences of block from list[from..].
pub open spec fn remove_from_spec(list: Seq<u64>, block: u64, from: int) -> Seq<u64>
    decreases (if list.len() >= from { list.len() - from } else { 0 }),
{
    if from < 0 || from >= list.len() {
        seq![]
    } else if list[from] == block {
        remove_from_spec(list, block, from + 1)
    } else {
        seq![list[from]] + remove_from_spec(list, block, from + 1)
    }
}

/// Spec helper: remove all occurrences of block from list[..upto].
pub open spec fn remove_prefix_spec(list: Seq<u64>, block: u64, upto: int) -> Seq<u64>
    decreases (if upto >= 0 { upto } else { 0 }),
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_prefix_spec(list, block, upto - 1);
        if list[upto - 1] == block { prefix } else { prefix.push(list[upto - 1]) }
    }
}

// r[depends free-list.remove]
/// Spec: remove block from the list.
pub open spec fn free_list_remove_spec(list: Seq<u64>, block: u64) -> Seq<u64> {
    remove_prefix_spec(list, block, list.len() as int)
}

// r[impl free-list.remove]
/// Remove the given block from the list.
pub fn free_list_remove(list: &[u64], block: u64) -> (result: Vec<u64>)
    requires free_list_contains_spec(list@, block)
    ensures result@ == free_list_remove_spec(list@, block)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < list.len()
        invariant
            0 <= i <= list@.len(),
            result@ == remove_prefix_spec(list@, block, i as int),
        decreases list@.len() - i,
    {
        if list[i] != block {
            result.push(list[i]);
            proof {
                assert(remove_prefix_spec(list@, block, i as int + 1)
                    == remove_prefix_spec(list@, block, i as int).push(list[i as int]));
            }
        } else {
            proof {
                assert(remove_prefix_spec(list@, block, i as int + 1)
                    == remove_prefix_spec(list@, block, i as int));
            }
        }
        i = i + 1;
    }
    result
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify free-list.push]
/// Push then pop returns the same block (LIFO).
proof fn push_pop_roundtrip(list: Seq<u64>, block: u64)
    requires !free_list_contains_spec(list, block)
    ensures ({
        let pushed = free_list_push_spec(list, block);
        &&& pushed.len() > 0
        &&& free_list_pop_spec(pushed).0 == block
    })
{
    let pushed = free_list_push_spec(list, block);
    assert(pushed[0] == block);
}

// r[verify free-list.push]
/// Push increments length by 1.
proof fn push_increments_len(list: Seq<u64>, block: u64)
    ensures free_list_push_spec(list, block).len() == list.len() + 1
{}

// r[verify free-list.pop]
/// Pop decrements length by 1.
proof fn pop_decrements_len(list: Seq<u64>)
    requires list.len() > 0
    ensures free_list_pop_spec(list).1.len() == list.len() - 1
{}

// r[verify free-list.contains]
/// Empty list contains nothing.
proof fn empty_contains_nothing(block: u64)
    ensures ({let empty: Seq<u64> = seq![]; !free_list_contains_spec(empty, block)})
{}

// r[verify free-list.push]
/// Pushed block is in the list.
proof fn push_contains_block(list: Seq<u64>, block: u64)
    ensures free_list_contains_spec(free_list_push_spec(list, block), block)
{
    let pushed = free_list_push_spec(list, block);
    assert(pushed[0] == block);
}

// r[verify free-list.push]
/// Push preserves existing elements.
proof fn push_preserves_old(list: Seq<u64>, block: u64, other: u64)
    requires free_list_contains_spec(list, other)
    ensures free_list_contains_spec(free_list_push_spec(list, block), other)
{
    let pushed = free_list_push_spec(list, block);
    // other is at some index i in list, so it's at i+1 in pushed
    let i = choose|i: int| #![trigger list[i]] 0 <= i < list.len() && list[i] == other;
    assert(pushed[i + 1] == other);
}

// r[impl free-list.all-distinct]
// r[verify free-list.all-distinct]
/// Push onto a distinct list with a fresh block produces a distinct list.
proof fn push_preserves_distinct(list: Seq<u64>, block: u64)
    requires
        all_distinct_spec(list),
        !free_list_contains_spec(list, block),
    ensures all_distinct_spec(free_list_push_spec(list, block))
{
    let pushed = free_list_push_spec(list, block);
    assert forall|i: int, j: int|
        #![trigger pushed[i], pushed[j]]
        0 <= i < pushed.len() && 0 <= j < pushed.len() && i != j
    implies pushed[i] != pushed[j] by {
        if i == 0 {
            // pushed[0] == block, pushed[j] is from list[j-1]
            // block not in list, so list[j-1] != block
        } else if j == 0 {
            // symmetric
        } else {
            // Both from original list: list[i-1] != list[j-1] by all_distinct
            assert(list[i - 1] != list[j - 1]);
        }
    }
}

// r[verify free-list.all-distinct]
/// Pop from a distinct list produces a distinct list.
proof fn pop_preserves_distinct(list: Seq<u64>)
    requires list.len() > 0, all_distinct_spec(list)
    ensures all_distinct_spec(free_list_pop_spec(list).1)
{
    let rest = free_list_pop_spec(list).1;
    assert forall|i: int, j: int|
        #![trigger rest[i], rest[j]]
        0 <= i < rest.len() && 0 <= j < rest.len() && i != j
    implies rest[i] != rest[j] by {
        // rest[i] == list[i+1], rest[j] == list[j+1]
        assert(list[i + 1] != list[j + 1]);
    }
}

// r[impl free-list.all-within]
// r[verify free-list.all-within]
/// Push with a bounded block preserves the bounding invariant.
proof fn push_preserves_within(list: Seq<u64>, block: u64, bound: u64)
    requires
        all_within_spec(list, bound),
        (block as int) < (bound as int),
    ensures all_within_spec(free_list_push_spec(list, block), bound)
{
    let pushed = free_list_push_spec(list, block);
    assert forall|i: int|
        #![trigger pushed[i]]
        0 <= i < pushed.len()
    implies (pushed[i] as int) < (bound as int) by {
        if i == 0 {
            // pushed[0] == block < bound
        } else {
            // pushed[i] == list[i-1] < bound
            assert((list[i - 1] as int) < (bound as int));
        }
    }
}

// r[verify free-list.len]
/// Length of empty list is zero.
proof fn empty_len()
    ensures ({let empty: Seq<u64> = seq![]; free_list_len_spec(empty) == 0})
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify free-list.contains]
    #[test]
    fn test_contains() {
        assert!(free_list_contains(&[1, 2, 3], 2));
        assert!(!free_list_contains(&[1, 2, 3], 4));
        assert!(!free_list_contains(&[], 1));
    }

    // r[verify free-list.push]
    #[test]
    fn test_push_pop() {
        let list: &[u64] = &[];
        let pushed = free_list_push(list, 42);
        assert_eq!(pushed.len(), 1);
        let (block, rest) = free_list_pop(&pushed);
        assert_eq!(block, 42);
        assert_eq!(rest.len(), 0);
    }

    // r[verify free-list.push]
    #[test]
    fn test_push_lifo() {
        let list: &[u64] = &[10, 20];
        let pushed = free_list_push(list, 5);
        assert_eq!(pushed, vec![5, 10, 20]);
        let (first, rest) = free_list_pop(&pushed);
        assert_eq!(first, 5);
        assert_eq!(rest, vec![10, 20]);
    }

    // r[verify free-list.remove]
    #[test]
    fn test_remove() {
        let list: &[u64] = &[10, 20, 30];
        let removed = free_list_remove(list, 20);
        assert_eq!(removed, vec![10, 30]);
    }

    // r[verify free-list.len]
    #[test]
    fn test_len() {
        assert_eq!(free_list_len(&[]), 0);
        assert_eq!(free_list_len(&[1, 2, 3]), 3);
    }

    proptest! {
        // r[verify free-list.push]
        #[test]
        fn prop_push_increments_len(
            list in proptest::collection::vec(0u64..1000, 0..50),
            block in 1000u64..2000,
        ) {
            // block > max(list elements), so not in list
            let pushed = free_list_push(&list, block);
            prop_assert_eq!(pushed.len(), list.len() + 1);
        }

        // r[verify free-list.push]
        #[test]
        fn prop_push_pop_roundtrip(
            list in proptest::collection::vec(0u64..1000, 0..50),
            block in 1000u64..2000,
        ) {
            let pushed = free_list_push(&list, block);
            let (popped, rest) = free_list_pop(&pushed);
            prop_assert_eq!(popped, block);
            prop_assert_eq!(rest.len(), list.len());
        }

        // r[verify free-list.contains]
        #[test]
        fn prop_push_then_contains(
            list in proptest::collection::vec(0u64..1000, 0..50),
            block in 1000u64..2000,
        ) {
            let pushed = free_list_push(&list, block);
            prop_assert!(free_list_contains(&pushed, block));
        }

        // r[verify free-list.remove]
        #[test]
        fn prop_remove_then_not_contains(
            list in proptest::collection::vec(0u64..100, 1..20),
        ) {
            if !list.is_empty() {
                let block = list[0];
                let removed = free_list_remove(&list, block);
                // For a distinct list, block is gone after remove
                if list.iter().filter(|&&x| x == block).count() == 1 {
                    prop_assert!(!free_list_contains(&removed, block));
                }
            }
        }
    }
}

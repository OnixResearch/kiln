//! Verified Nix list operations.
//!
//! Models `builtins.map`, `builtins.filter`, `builtins.foldl'`,
//! `builtins.concatLists`, `builtins.genList`, `builtins.elem`,
//! `builtins.elemAt`, `builtins.head`, `builtins.tail`, `builtins.length`.

// Verus requires &Vec for @ view, not &[T].
// Ghost variable `mapped` is used in Verus proof blocks but appears unused to rustc.
#![allow(clippy::ptr_arg, unused_variables)]

use vstd::prelude::*;

verus! {

// --- length ---

// r[depends nix-list.length]
/// Spec: list length.
pub open spec fn length_spec(list: Seq<u64>) -> nat {
    list.len()
}

// r[impl nix-list.length]
/// Returns the length of the list.
pub fn length(list: &Vec<u64>) -> (result: usize)
    ensures result as nat == length_spec(list@)
{
    list.len()
}

// --- elem_at ---

// r[depends nix-list.elem-at]
/// Spec: get element at index.
pub open spec fn elem_at_spec(list: Seq<u64>, idx: int) -> u64
    recommends 0 <= idx < list.len() as int
{
    list[idx]
}

// r[impl nix-list.elem-at]
/// Returns the element at the specified index.
pub fn elem_at(list: &Vec<u64>, idx: usize) -> (result: u64)
    requires idx < list.len()
    ensures result == elem_at_spec(list@, idx as int)
{
    list[idx]
}

// --- head ---

// r[depends nix-list.head]
/// Spec: first element.
pub open spec fn head_spec(list: Seq<u64>) -> u64
    recommends list.len() > 0
{
    list[0int]
}

// r[impl nix-list.head]
/// Returns the first element of the list.
pub fn head(list: &Vec<u64>) -> (result: u64)
    requires list.len() > 0
    ensures result == head_spec(list@)
{
    list[0]
}

// --- tail ---

// r[depends nix-list.tail]
/// Spec: all elements except the first.
pub open spec fn tail_spec(list: Seq<u64>) -> Seq<u64>
    recommends list.len() > 0
{
    list.subrange(1, list.len() as int)
}

// r[impl nix-list.tail]
/// All elements except the first.
pub fn tail(list: &Vec<u64>) -> (result: Vec<u64>)
    requires list.len() > 0
    ensures result@ == tail_spec(list@)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 1;
    while i < list.len()
        invariant
            1 <= i <= list.len(),
            list.len() > 0,
            result.len() == i - 1,
            forall|j: int| 0 <= j < (i - 1) as int ==> result@[j] == list@[j + 1],
        decreases list.len() - i,
    {
        result.push(list[i]);
        i += 1;
    }
    assert(result@ =~= tail_spec(list@));
    result
}

// --- elem ---

// r[depends nix-list.elem]
/// Spec: is a value in the list?
pub open spec fn elem_spec(list: Seq<u64>, x: u64) -> bool {
    exists|i: int| 0 <= i < list.len() as int && list[i] == x
}

// r[impl nix-list.elem]
/// Returns true if the value is present in the list.
pub fn elem(list: &Vec<u64>, x: u64) -> (result: bool)
    ensures result == elem_spec(list@, x)
{
    let mut i: usize = 0;
    while i < list.len()
        invariant
            i <= list.len(),
            forall|j: int| 0 <= j < i as int ==> list@[j] != x,
        decreases list.len() - i,
    {
        if list[i] == x {
            return true;
        }
        i += 1;
    }
    false
}

// --- gen_list ---

// r[depends nix-list.gen-list]
/// Spec: generate a list [0, 1, ..., n-1].
pub open spec fn gen_list_spec(n: nat) -> Seq<u64> {
    Seq::new(n, |i: int| i as u64)
}

// r[impl nix-list.gen-list]
/// Generates a list [0, 1, ..., n-1].
pub fn gen_list(n: usize) -> (result: Vec<u64>)
    requires n <= u64::MAX as usize
    ensures result@ == gen_list_spec(n as nat)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < n
        invariant
            i <= n,
            n <= u64::MAX as usize,
            result.len() == i,
            forall|j: int| 0 <= j < i as int ==> result@[j] == j as u64,
        decreases n - i,
    {
        result.push(i as u64);
        i += 1;
    }
    assert(result@ =~= gen_list_spec(n as nat));
    result
}

// --- concat_lists ---

// r[depends nix-list.concat-lists]
/// Spec: concatenate a list of lists.
pub open spec fn concat_lists_spec(lists: Seq<Seq<u64>>) -> Seq<u64>
    decreases lists.len()
{
    if lists.len() == 0 {
        Seq::<u64>::empty()
    } else {
        lists[0int] + concat_lists_spec(lists.subrange(1, lists.len() as int))
    }
}

/// Helper lemma: concat_lists_spec(lists.take(i+1)) ==
///   concat_lists_spec(lists.take(i)) + lists[i]
proof fn concat_lists_take_step(lists: Seq<Seq<u64>>, i: int)
    requires 0 <= i < lists.len()
    ensures concat_lists_spec(lists.take(i + 1))
        =~= concat_lists_spec(lists.take(i)) + lists[i]
    decreases i
{
    let curr = lists.take(i + 1);
    let prev = lists.take(i);

    if i == 0 {
        assert(curr.len() == 1);
        assert(curr[0] == lists[0]);
        assert(prev.len() == 0);

        // Explicitly unfold concat_lists_spec for prev (len == 0 → empty)
        assert(concat_lists_spec(prev) =~= Seq::<u64>::empty());

        // Explicitly unfold concat_lists_spec for curr (len == 1)
        // Step 1: curr.subrange(1, 1) is empty
        let curr_rest = curr.subrange(1, curr.len() as int);
        assert(curr_rest.len() == 0);
        assert(concat_lists_spec(curr_rest) =~= Seq::<u64>::empty());

        // Step 2: concat_lists_spec(curr) = curr[0] + empty = lists[0] + empty
        assert(concat_lists_spec(curr) =~= curr[0] + concat_lists_spec(curr_rest));
        assert(concat_lists_spec(curr) =~= lists[0] + Seq::<u64>::empty());

        // Step 3: lists[0] + empty =~= lists[0] =~= empty + lists[0]
        assert(lists[0] + Seq::<u64>::empty() =~= lists[0]);
        assert(Seq::<u64>::empty() + lists[0] =~= lists[0]);

        // Combine: concat_lists_spec(curr) =~= lists[0] =~= concat_lists_spec(prev) + lists[0]
        assert(concat_lists_spec(curr) =~= concat_lists_spec(prev) + lists[i]);
    } else {
        // Both curr and prev start with lists[0]
        assert(curr[0] == lists[0]);
        assert(prev[0] == lists[0]);

        let curr_tail = curr.subrange(1, curr.len() as int);
        let prev_tail = prev.subrange(1, prev.len() as int);

        // Relate tails to subrange of original list
        let sub = lists.subrange(1, lists.len() as int);
        assert(curr_tail =~= sub.take(i));
        assert(prev_tail =~= sub.take(i - 1));
        assert(sub[i - 1] == lists[i]);

        // IH on shifted list
        concat_lists_take_step(sub, i - 1);
        // Now: concat_lists_spec(sub.take(i)) =~= concat_lists_spec(sub.take(i-1)) + sub[i-1]
        // i.e.: concat_lists_spec(curr_tail) =~= concat_lists_spec(prev_tail) + lists[i]

        // Associativity of Seq +
        let a: Seq<u64> = lists[0];
        let b: Seq<u64> = concat_lists_spec(prev_tail);
        let c: Seq<u64> = lists[i];
        assert(a + (b + c) =~= (a + b) + c);
    }
}

/// Helper lemma: concat_lists_spec of empty is empty.
proof fn concat_lists_empty()
    ensures concat_lists_spec(Seq::<Seq<u64>>::empty()) =~= Seq::<u64>::empty()
{}

// r[impl nix-list.concat-lists]
/// Concatenates a list of lists into a single flat list.
pub fn concat_lists(lists: &Vec<Vec<u64>>) -> (result: Vec<u64>)
    ensures result@ == concat_lists_spec(lists@.map_values(|v: Vec<u64>| v@))
{
    let mapped = Ghost(lists@.map_values(|v: Vec<u64>| v@));
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;

    proof {
        concat_lists_empty();
        assert(mapped@.take(0) =~= Seq::<Seq<u64>>::empty());
        assert(result@ =~= concat_lists_spec(mapped@.take(0int)));
    }

    while i < lists.len()
        invariant
            i <= lists.len(),
            mapped@ == lists@.map_values(|v: Vec<u64>| v@),
            result@ == concat_lists_spec(mapped@.take(i as int)),
        decreases lists.len() - i,
    {
        let mut j: usize = 0;
        let ghost prev_result = result@;
        while j < lists[i].len()
            invariant
                i < lists.len(),
                j <= lists[i as int].len(),
                mapped@ == lists@.map_values(|v: Vec<u64>| v@),
                result@ == prev_result + lists@[i as int]@.take(j as int),
            decreases lists[i as int].len() - j,
        {
            result.push(lists[i][j]);
            proof {
                assert(lists@[i as int]@.take(j as int + 1) =~=
                    lists@[i as int]@.take(j as int).push(lists@[i as int]@[j as int]));
            }
            j += 1;
        }
        proof {
            assert(lists@[i as int]@.take(lists@[i as int]@.len() as int) =~= lists@[i as int]@);
            assert(mapped@[i as int] == lists@[i as int]@);
            assert(result@ == prev_result + mapped@[i as int]);
            // prev_result == concat_lists_spec(mapped@.take(i))
            // result@ == concat_lists_spec(mapped@.take(i)) + mapped@[i]
            concat_lists_take_step(mapped@, i as int);
            // Now: concat_lists_spec(mapped@.take(i + 1)) == concat_lists_spec(mapped@.take(i)) + mapped@[i]
        }
        i += 1;
    }
    proof {
        assert(mapped@.take(lists@.len() as int) =~= mapped@);
    }
    result
}

// --- Proofs ---

// r[verify nix-list.length]
/// gen_list produces exactly n elements.
proof fn gen_list_length(n: nat)
    ensures gen_list_spec(n).len() == n
{}

// r[verify nix-list.elem]
/// head is in the list.
proof fn head_is_elem(list: Seq<u64>)
    requires list.len() > 0
    ensures elem_spec(list, head_spec(list))
{
    assert(list[0int] == head_spec(list));
}

// r[verify nix-list.tail]
/// tail has length one less.
proof fn tail_length(list: Seq<u64>)
    requires list.len() > 0
    ensures tail_spec(list).len() == list.len() - 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify nix-list.length]
    #[test]
    fn test_length() {
        assert_eq!(length(&vec![1, 2, 3]), 3);
        assert_eq!(length(&vec![]), 0);
    }

    // r[verify nix-list.elem-at]
    #[test]
    fn test_elem_at() {
        assert_eq!(elem_at(&vec![10, 20, 30], 1), 20);
    }

    // r[verify nix-list.head]
    #[test]
    fn test_head() {
        assert_eq!(head(&vec![42, 1, 2]), 42);
    }

    // r[verify nix-list.tail]
    #[test]
    fn test_tail() {
        assert_eq!(tail(&vec![1, 2, 3]), vec![2, 3]);
    }

    // r[verify nix-list.elem]
    #[test]
    fn test_elem() {
        assert!(elem(&vec![1, 2, 3], 2));
        assert!(!elem(&vec![1, 2, 3], 4));
    }

    // r[verify nix-list.gen-list]
    #[test]
    fn test_gen_list() {
        assert_eq!(gen_list(5), vec![0, 1, 2, 3, 4]);
        assert_eq!(gen_list(0), Vec::<u64>::new());
    }

    // r[verify nix-list.concat-lists]
    #[test]
    fn test_concat_lists() {
        let lists = vec![vec![1, 2], vec![3], vec![4, 5, 6]];
        assert_eq!(concat_lists(&lists), vec![1, 2, 3, 4, 5, 6]);
    }

    proptest! {
        // r[verify nix-list.gen-list]
        #[test]
        fn prop_gen_list_length(n in 0usize..1000) {
            prop_assert_eq!(gen_list(n).len(), n);
        }

        // r[verify nix-list.elem]
        #[test]
        fn prop_elem_finds_inserted(
            list in proptest::collection::vec(any::<u64>(), 1..=10),
            idx in 0usize..10,
        ) {
            let idx = idx % list.len();
            prop_assert!(elem(&list, list[idx]));
        }

        // r[verify nix-list.tail]
        #[test]
        fn prop_tail_length(
            list in proptest::collection::vec(any::<u64>(), 1..=20),
        ) {
            prop_assert_eq!(tail(&list).len(), list.len() - 1);
        }
    }
}

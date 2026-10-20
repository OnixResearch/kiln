//! Verified seL4-style guarded CSpace lookup.
//!
//! Models capability-space traversal as explicit path-segment consumption.
//! Each CNode contributes a guard sequence that must match the next path
//! segments, then consumes one radix segment selecting exactly one slot.
//! If path segments remain, lookup follows the selected child CNode when the
//! slot contains one. Successful lookup returns the final resolved slot.

use vstd::prelude::*;

verus! {

pub struct CNode {
    pub guard: Vec<usize>,
    pub slots: Vec<Option<usize>>,
}

pub struct CSpace {
    pub nodes: Vec<CNode>,
    pub root: usize,
}

pub struct ResolvedSlot {
    pub node: usize,
    pub slot: usize,
    pub consumed: usize,
}

// r[impl sel4-cspace.lookup]
// r[depends sel4-cspace.lookup]
/// Exact guard match starting at `from`.
pub open spec fn guard_matches_from_spec(path: Seq<usize>, from: int, guard: Seq<usize>) -> bool {
    &&& 0 <= from
    &&& from + guard.len() <= path.len()
    &&& forall|i: int| 0 <= i < guard.len() ==> path[from + i] == guard[i]
}

// r[impl sel4-cspace.lookup]
/// Returns true when `guard` matches `path` starting at `from`.
pub fn guard_matches_from(path: &Vec<usize>, from: usize, guard: &Vec<usize>) -> (result: bool)
    ensures result == guard_matches_from_spec(path@, from as int, guard@)
{
    if from > path.len() {
        return false;
    }
    let remaining = path.len() - from;
    if guard.len() > remaining {
        return false;
    }
    let mut i: usize = 0;
    while i < guard.len()
        invariant
            from <= path.len(),
            guard.len() <= path.len() - from,
            0 <= i <= guard.len(),
            forall|j: int| 0 <= j < i as int ==> path@[from as int + j] == guard@[j],
        decreases guard.len() - i,
    {
        if path[from + i] != guard[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl sel4-cspace.lookup]
// r[depends sel4-cspace.lookup]
/// Slot segment selected immediately after a matching guard.
pub open spec fn select_slot_spec(path: Seq<usize>, from: int, guard: Seq<usize>) -> Option<usize> {
    if 0 <= from && from + guard.len() < path.len() {
        Some(path[from + guard.len()])
    } else {
        None
    }
}

// r[impl sel4-cspace.lookup]
/// Returns the slot segment immediately after `guard`, if present.
pub fn select_slot(path: &Vec<usize>, from: usize, guard: &Vec<usize>) -> (result: Option<usize>)
    ensures result == select_slot_spec(path@, from as int, guard@)
{
    if from > path.len() {
        return None;
    }
    let remaining = path.len() - from;
    if guard.len() >= remaining {
        None
    } else {
        proof {
            assert(from + guard.len() < path.len()) by(nonlinear_arith)
                requires from <= path.len(), guard.len() < path.len() - from;
        }
        let guard_end = from + guard.len();
        Some(path[guard_end])
    }
}

// r[impl sel4-cspace.lookup]
// r[depends sel4-cspace.lookup]
/// Recursive guarded lookup from `node_idx`, starting at consumed-depth `from`.
pub open spec fn lookup_from_spec(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int) -> Option<ResolvedSlot>
    recommends
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
    decreases if 0 <= from && from <= path.len() { path.len() - from } else { 0 }
{
    if node_idx < 0 || node_idx >= nodes.len() || from < 0 {
        None
    } else {
        let node = nodes[node_idx];
        if !guard_matches_from_spec(path, from, node.guard@) {
            None
        } else {
            match select_slot_spec(path, from, node.guard@) {
                None => None,
                Some(slot) => {
                    let consumed = from + node.guard@.len() + 1;
                    if slot >= node.slots@.len() {
                        None
                    } else if consumed == path.len() {
                        Some(ResolvedSlot {
                            node: node_idx as usize,
                            slot,
                            consumed: consumed as usize,
                        })
                    } else {
                        match node.slots@[slot as int] {
                            None => None,
                            Some(child) => lookup_from_spec(nodes, child as int, path, consumed),
                        }
                    }
                }
            }
        }
    }
}

impl CSpace {
    // r[impl sel4-cspace.lookup]
    // r[impl sel4-cspace.unique-slot]
    // r[impl sel4-cspace.failure-no-mutation]
    // r[depends sel4-cspace.lookup]
    /// Rooted CSpace lookup.
    pub open spec fn lookup_spec(&self, path: Seq<usize>) -> Option<ResolvedSlot>
        recommends self.nodes@.len() <= usize::MAX as int, path.len() <= usize::MAX as int
    {
        lookup_from_spec(self.nodes@, self.root as int, path, 0)
    }
}

fn lookup_from_valid(cspace: &CSpace, node_idx: usize, path: &Vec<usize>, from: usize) -> (result: Option<ResolvedSlot>)
    requires from <= path.len()
    ensures result == lookup_from_spec(cspace.nodes@, node_idx as int, path@, from as int)
    decreases path.len() - from
{
    if node_idx >= cspace.nodes.len() {
        return None;
    }
    let node = &cspace.nodes[node_idx];
    if !guard_matches_from(path, from, &node.guard) {
        return None;
    }
    let slot = select_slot(path, from, &node.guard)?;
    if slot >= node.slots.len() {
        return None;
    }
    proof {
        assert(from + node.guard.len() < path.len()) by(nonlinear_arith)
            requires
                from <= path.len(),
                node.guard.len() < path.len() - from;
    }
    let consumed = from + node.guard.len() + 1;
    if consumed == path.len() {
        return Some(ResolvedSlot { node: node_idx, slot, consumed });
    }
    match node.slots[slot] {
        None => None,
        Some(child) => {
            assert(consumed <= path.len());
            lookup_from_valid(cspace, child, path, consumed)
        }
    }
}

// r[impl sel4-cspace.lookup]
// r[impl sel4-cspace.unique-slot]
// r[impl sel4-cspace.failure-no-mutation]
/// Public lookup helper with explicit consumed-depth accounting.
pub fn lookup_from(cspace: &CSpace, node_idx: usize, path: &Vec<usize>, from: usize) -> (result: Option<ResolvedSlot>)
    ensures
        result == lookup_from_spec(cspace.nodes@, node_idx as int, path@, from as int)
{
    if from > path.len() {
        None
    } else {
        lookup_from_valid(cspace, node_idx, path, from)
    }
}

// r[impl sel4-cspace.lookup]
// r[impl sel4-cspace.unique-slot]
// r[impl sel4-cspace.failure-no-mutation]
/// Root lookup from the CSpace root.
pub fn lookup(cspace: &CSpace, path: &Vec<usize>) -> (result: Option<ResolvedSlot>)
    ensures
        result == cspace.lookup_spec(path@)
{
    lookup_from(cspace, cspace.root, path, 0)
}

// r[verify sel4-cspace.failure-no-mutation]
pub proof fn guard_mismatch_causes_failure(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int)
    requires
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        0 <= node_idx < nodes.len(),
        from >= 0,
        !guard_matches_from_spec(path, from, nodes[node_idx].guard@),
    ensures
        lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>
{
}

// r[verify sel4-cspace.failure-no-mutation]
pub proof fn insufficient_depth_causes_failure(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int)
    requires
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        0 <= node_idx < nodes.len(),
        from >= 0,
        from + nodes[node_idx].guard@.len() >= path.len(),
    ensures
        lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>
{
}

// r[verify sel4-cspace.lookup]
pub proof fn lookup_step_is_guarded(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int)
    -> (result: ResolvedSlot)
    requires
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        0 <= node_idx < nodes.len(),
        lookup_from_spec(nodes, node_idx, path, from) is Some,
    ensures
        lookup_from_spec(nodes, node_idx, path, from) == Some(result),
        guard_matches_from_spec(path, from, nodes[node_idx].guard@),
        select_slot_spec(path, from, nodes[node_idx].guard@) is Some,
        ({
            let slot = select_slot_spec(path, from, nodes[node_idx].guard@).unwrap();
            slot < nodes[node_idx].slots@.len()
        }),
        from + nodes[node_idx].guard@.len() + 1 <= result.consumed as int <= path.len(),
    decreases if 0 <= from && from <= path.len() { path.len() - from } else { 0 }
{
    let node = nodes[node_idx];
    if !guard_matches_from_spec(path, from, node.guard@) {
        assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
        arbitrary()
    } else {
        match select_slot_spec(path, from, node.guard@) {
            None => {
                assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
                arbitrary()
            }
            Some(slot) => {
                let consumed = from + node.guard@.len() + 1;
                if slot >= node.slots@.len() {
                    assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
                    arbitrary()
                } else if consumed == path.len() {
                    ResolvedSlot { node: node_idx as usize, slot, consumed: consumed as usize }
                } else {
                    match node.slots@[slot as int] {
                        None => {
                            assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
                            arbitrary()
                        }
                        Some(child) => lookup_step_is_guarded(nodes, child as int, path, consumed),
                    }
                }
            }
        }
    }
}

// r[verify sel4-cspace.lookup]
pub proof fn lookup_hit_consumes_all(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int)
    -> (result: ResolvedSlot)
    requires
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        lookup_from_spec(nodes, node_idx, path, from) is Some,
    ensures
        lookup_from_spec(nodes, node_idx, path, from) == Some(result),
        result.consumed as int == path.len()
    decreases if 0 <= from && from <= path.len() { path.len() - from } else { 0 }
{
    if node_idx < 0 || node_idx >= nodes.len() || from < 0 {
        arbitrary()
    } else {
        let node = nodes[node_idx];
        if !guard_matches_from_spec(path, from, node.guard@) {
            assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
            arbitrary()
        } else {
            match select_slot_spec(path, from, node.guard@) {
                None => {
                    assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
                    arbitrary()
                }
                Some(slot) => {
                    let consumed = from + node.guard@.len() + 1;
                    if slot >= node.slots@.len() {
                        assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
                        arbitrary()
                    } else if consumed == path.len() {
                        ResolvedSlot { node: node_idx as usize, slot, consumed: consumed as usize }
                    } else {
                        match node.slots@[slot as int] {
                            None => {
                                assert(lookup_from_spec(nodes, node_idx, path, from) == None::<ResolvedSlot>);
                                arbitrary()
                            }
                            Some(child) => lookup_hit_consumes_all(nodes, child as int, path, consumed),
                        }
                    }
                }
            }
        }
    }
}

// r[verify sel4-cspace.lookup]
pub proof fn rooted_lookup_consumes_all(cspace: CSpace, path: Seq<usize>)
    -> (result: ResolvedSlot)
    requires
        cspace.nodes@.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        cspace.lookup_spec(path) is Some,
    ensures
        cspace.lookup_spec(path) == Some(result),
        result.consumed as int == path.len()
{
    lookup_hit_consumes_all(cspace.nodes@, cspace.root as int, path, 0)
}

// r[verify sel4-cspace.unique-slot]
pub proof fn lookup_hit_in_bounds(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int)
    -> (result: ResolvedSlot)
    requires
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        lookup_from_spec(nodes, node_idx, path, from) is Some,
    ensures
        lookup_from_spec(nodes, node_idx, path, from) == Some(result),
        (result.node as int) < nodes.len(),
        (result.slot as int) < nodes[result.node as int].slots@.len(),
        from <= result.consumed as int <= path.len(),
    decreases if 0 <= from && from <= path.len() { path.len() - from } else { 0 }
{
    if node_idx < 0 || node_idx >= nodes.len() || from < 0 {
        arbitrary()
    } else {
        let node = nodes[node_idx];
        if !guard_matches_from_spec(path, from, node.guard@) {
            arbitrary()
        } else {
            match select_slot_spec(path, from, node.guard@) {
                None => arbitrary(),
                Some(slot) => {
                    let consumed = from + node.guard@.len() + 1;
                    if slot >= node.slots@.len() {
                        arbitrary()
                    } else if consumed == path.len() {
                        ResolvedSlot { node: node_idx as usize, slot, consumed: consumed as usize }
                    } else {
                        match node.slots@[slot as int] {
                            None => arbitrary(),
                            Some(child) => {
                                lookup_hit_in_bounds(nodes, child as int, path, consumed)
                            }
                        }
                    }
                }
            }
        }
    }
}

// r[verify sel4-cspace.unique-slot]
pub proof fn lookup_result_unique(nodes: Seq<CNode>, node_idx: int, path: Seq<usize>, from: int, a: ResolvedSlot, b: ResolvedSlot)
    requires
        nodes.len() <= usize::MAX as int,
        path.len() <= usize::MAX as int,
        lookup_from_spec(nodes, node_idx, path, from) == Some(a),
        lookup_from_spec(nodes, node_idx, path, from) == Some(b),
    ensures
        a == b
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn root_only_cspace() -> CSpace {
        CSpace {
            nodes: vec![CNode {
                guard: vec![],
                slots: vec![None, None, None],
            }],
            root: 0,
        }
    }

    fn nested_cspace() -> CSpace {
        CSpace {
            nodes: vec![
                CNode {
                    guard: vec![7],
                    slots: vec![None, Some(1), None],
                },
                CNode {
                    guard: vec![9],
                    slots: vec![None, None, None],
                },
            ],
            root: 0,
        }
    }

    // r[verify sel4-cspace.lookup]
    #[test]
    fn test_root_lookup() {
        let cspace = root_only_cspace();
        let path = vec![1usize];
        let resolved = lookup(&cspace, &path).expect("root lookup should succeed");
        assert_eq!(resolved.node, 0);
        assert_eq!(resolved.slot, 1);
        assert_eq!(resolved.consumed, 1);
    }

    // r[verify sel4-cspace.lookup]
    #[test]
    fn test_nested_lookup() {
        let cspace = nested_cspace();
        let path = vec![7usize, 1usize, 9usize, 2usize];
        let resolved = lookup(&cspace, &path).expect("nested lookup should succeed");
        assert_eq!(resolved.node, 1);
        assert_eq!(resolved.slot, 2);
        assert_eq!(resolved.consumed, 4);
    }

    // r[verify sel4-cspace.failure-no-mutation]
    #[test]
    fn test_guard_mismatch_fails() {
        let cspace = nested_cspace();
        let before = cspace.nodes[0].guard.clone();
        let path = vec![8usize, 1usize];
        assert!(lookup(&cspace, &path).is_none());
        assert_eq!(cspace.nodes[0].guard, before);
    }

    // r[verify sel4-cspace.failure-no-mutation]
    #[test]
    fn test_insufficient_depth_fails() {
        let cspace = nested_cspace();
        let before_len = cspace.nodes.len();
        let path = vec![7usize];
        assert!(lookup(&cspace, &path).is_none());
        assert_eq!(cspace.nodes.len(), before_len);
    }

    // r[verify sel4-cspace.failure-no-mutation]
    #[test]
    fn test_failed_nested_follow_does_not_mutate() {
        let cspace = nested_cspace();
        let before_slots = cspace.nodes[0].slots.clone();
        let path = vec![7usize, 1usize, 8usize, 0usize];
        assert!(lookup(&cspace, &path).is_none());
        assert_eq!(cspace.nodes[0].slots, before_slots);
    }

    // r[verify sel4-cspace.unique-slot]
    #[test]
    fn test_lookup_from_explicit_depth() {
        let cspace = nested_cspace();
        let path = vec![7usize, 1usize, 9usize, 0usize];
        let resolved = lookup_from(&cspace, 0, &path, 0).expect("lookup_from should succeed");
        assert_eq!(resolved.node, 1);
        assert_eq!(resolved.slot, 0);
        assert_eq!(resolved.consumed, 4);
    }

    proptest! {
        // r[verify sel4-cspace.lookup]
        #[test]
        fn prop_root_slot_matches(slot in 0usize..3) {
            let cspace = root_only_cspace();
            let path = vec![slot];
            let resolved = lookup(&cspace, &path).expect("root lookup must resolve chosen slot");
            prop_assert_eq!(resolved.node, 0);
            prop_assert_eq!(resolved.slot, slot);
            prop_assert_eq!(resolved.consumed, 1);
        }

        // r[verify sel4-cspace.failure-no-mutation]
        #[test]
        fn prop_wrong_guard_fails(slot in 0usize..3, wrong in 0usize..20) {
            prop_assume!(wrong != 7);
            let cspace = nested_cspace();
            let before = cspace.nodes[0].guard.clone();
            let path = vec![wrong, slot];
            prop_assert!(lookup(&cspace, &path).is_none());
            prop_assert_eq!(&cspace.nodes[0].guard, &before);
        }

        // r[verify sel4-cspace.unique-slot]
        #[test]
        fn prop_lookup_unique_root(slot in 0usize..3) {
            let cspace = root_only_cspace();
            let path = vec![slot];
            let a = lookup(&cspace, &path).expect("lookup should succeed");
            let b = lookup(&cspace, &path).expect("lookup should be deterministic");
            prop_assert_eq!(a.node, b.node);
            prop_assert_eq!(a.slot, b.slot);
            prop_assert_eq!(a.consumed, b.consumed);
        }
    }
}

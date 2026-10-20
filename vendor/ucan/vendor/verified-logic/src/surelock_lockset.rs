//! Verified surelock same-level lock-set preparation.
//!
//! A lock set prepares an internal acquisition plan by sorting locks of one
//! level by lock ID. Duplicate IDs are rejected. Caller order stays available
//! through a position map back into canonical plan.

use vstd::prelude::*;

use crate::surelock_id::SureLock;

verus! {

// r[depends surelock-lockset.canonical-ordering]
pub open spec fn same_level_group_spec(locks: Seq<SureLock>) -> bool {
    forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < locks.len() && 0 <= j < locks.len() ==> locks[i].level == locks[j].level
}

// r[depends surelock-lockset.duplicate-rejection]
// r[depends surelock-lockset.membership-preservation]
// r[depends surelock-lockset.caller-order-preservation]
pub open spec fn contains_id_spec(locks: Seq<SureLock>, id: u64) -> bool {
    exists|i: int| #![trigger locks[i]] 0 <= i < locks.len() && locks[i].id == id
}

// r[depends surelock-lockset.canonical-ordering]
pub open spec fn strict_id_order_spec(locks: Seq<SureLock>) -> bool {
    forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len() ==> locks[i].id < locks[j].id
}

// r[depends surelock-lockset.duplicate-rejection]
pub open spec fn no_duplicate_ids_spec(locks: Seq<SureLock>) -> bool {
    forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len() ==> locks[i].id != locks[j].id
}

// r[depends surelock-lockset.membership-preservation]
pub open spec fn same_id_set_spec(a: Seq<SureLock>, b: Seq<SureLock>) -> bool {
    forall|id: u64| contains_id_spec(a, id) == contains_id_spec(b, id)
}

pub open spec fn insert_seq_spec(locks: Seq<SureLock>, pos: int, lock: SureLock) -> Seq<SureLock>
    recommends 0 <= pos <= locks.len()
{
    locks.subrange(0, pos) + seq![lock] + locks.subrange(pos, locks.len() as int)
}

proof fn strict_order_implies_no_duplicates(locks: Seq<SureLock>)
    requires strict_id_order_spec(locks)
    ensures no_duplicate_ids_spec(locks)
{
    assert forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len()
    implies locks[i].id != locks[j].id by {
        assert(locks[i].id < locks[j].id);
    }
}

proof fn insert_preserves_membership(locks: Seq<SureLock>, pos: int, lock: SureLock, id: u64)
    requires 0 <= pos <= locks.len()
    ensures contains_id_spec(insert_seq_spec(locks, pos, lock), id) == (id == lock.id || contains_id_spec(locks, id))
{
    let inserted = insert_seq_spec(locks, pos, lock);
    if contains_id_spec(inserted, id) {
        let i = choose|i: int| 0 <= i < inserted.len() && inserted[i].id == id;
        if i == pos {
            assert(id == lock.id);
        } else if i < pos {
            assert(locks[i].id == id);
        } else {
            assert(locks[i - 1].id == id);
        }
    }
    if id == lock.id || contains_id_spec(locks, id) {
        if id == lock.id {
            assert(inserted[pos].id == id);
        } else {
            let i = choose|i: int| 0 <= i < locks.len() && locks[i].id == id;
            if i < pos {
                assert(inserted[i].id == id);
            } else {
                assert(inserted[i + 1].id == id);
            }
        }
    }
}

proof fn insert_preserves_strict_order(locks: Seq<SureLock>, pos: int, lock: SureLock)
    requires
        strict_id_order_spec(locks),
        0 <= pos <= locks.len(),
        forall|i: int| #![trigger locks[i]] 0 <= i < pos ==> locks[i].id < lock.id,
        forall|i: int| #![trigger locks[i]] pos <= i < locks.len() ==> lock.id < locks[i].id,
    ensures strict_id_order_spec(insert_seq_spec(locks, pos, lock))
{
    let inserted = insert_seq_spec(locks, pos, lock);
    assert forall|i: int, j: int| #![trigger inserted[i], inserted[j]]
        0 <= i < j < inserted.len()
    implies inserted[i].id < inserted[j].id by {
        if j == pos {
            assert(i < pos);
            assert(inserted[i] == locks[i]);
        } else if i == pos {
            assert(pos < j);
            if j == pos + 1 {
                assert(inserted[j] == locks[pos]);
            } else {
                assert(inserted[j] == locks[j - 1]);
            }
        } else if j < pos {
            assert(inserted[i] == locks[i]);
            assert(inserted[j] == locks[j]);
        } else if i < pos && j > pos {
            assert(inserted[i] == locks[i]);
            assert(inserted[j] == locks[j - 1]);
            if j == pos + 1 {
                assert(lock.id < locks[pos].id);
            } else {
                assert(lock.id < locks[j - 1].id);
                assert(locks[i].id < lock.id);
            }
        } else {
            assert(i > pos && j > pos);
            assert(inserted[i] == locks[i - 1]);
            assert(inserted[j] == locks[j - 1]);
        }
    }
}

fn find_insert_position_no_dup(plan: &[SureLock], id: u64) -> (result: (bool, usize))
    requires strict_id_order_spec(plan@)
    ensures
        result.1 <= plan@.len(),
        result.0 ==> result.1 < plan@.len() && plan@[result.1 as int].id == id,
        !result.0 ==> (
            forall|i: int| #![trigger plan@[i]] 0 <= i < result.1 as int ==> plan@[i].id < id
        ) && (
            forall|i: int| #![trigger plan@[i]] result.1 as int <= i < plan@.len() ==> id < plan@[i].id
        ),
{
    let mut i: usize = 0;
    while i < plan.len()
        invariant
            0 <= i <= plan@.len(),
            strict_id_order_spec(plan@),
            forall|j: int| #![trigger plan@[j]] 0 <= j < i as int ==> plan@[j].id < id,
        decreases plan.len() - i,
    {
        if plan[i].id == id {
            return (true, i);
        }
        if plan[i].id > id {
            assert forall|j: int| #![trigger plan@[j]] i as int <= j < plan@.len()
                implies id < plan@[j].id by {
                if j == i as int {
                } else {
                    assert(plan@[i as int].id < plan@[j].id);
                }
            }
            return (false, i);
        }
        i += 1;
    }
    (false, plan.len())
}

fn insert_at(plan: &[SureLock], pos: usize, lock: SureLock) -> (result: Vec<SureLock>)
    requires pos <= plan@.len()
    ensures result@ =~= insert_seq_spec(plan@, pos as int, lock)
{
    let mut result: Vec<SureLock> = Vec::new();
    let mut i: usize = 0;
    while i < pos
        invariant
            0 <= i <= pos,
            pos <= plan@.len(),
            result@ =~= plan@.subrange(0, i as int),
        decreases pos - i,
    {
        result.push(plan[i]);
        i += 1;
    }
    result.push(lock);
    let mut j: usize = pos;
    while j < plan.len()
        invariant
            pos <= j <= plan@.len(),
            pos <= plan@.len(),
            result@ =~= plan@.subrange(0, pos as int) + seq![lock] + plan@.subrange(pos as int, j as int),
        decreases plan.len() - j,
    {
        result.push(plan[j]);
        j += 1;
    }
    result
}

// r[impl surelock-lockset.canonical-ordering]
// r[impl surelock-lockset.duplicate-rejection]
// r[impl surelock-lockset.membership-preservation]
pub fn prepare_lockset(input: &[SureLock]) -> (result: Option<Vec<SureLock>>)
    ensures
        result.is_some() ==> {
            let plan = result.unwrap()@;
            &&& strict_id_order_spec(plan)
            &&& no_duplicate_ids_spec(plan)
            &&& plan.len() == input@.len()
        },
{
    let first_level = if input.len() == 0 { 0 } else { input[0].level };
    let mut plan: Vec<SureLock> = Vec::new();
    let mut i: usize = 0;
    while i < input.len()
        invariant
            0 <= i <= input@.len(),
            strict_id_order_spec(plan@),
            no_duplicate_ids_spec(plan@),
            plan@.len() == i as int,
        decreases input.len() - i,
    {
        let lock = input[i];
        if i > 0 && lock.level != first_level {
            return None;
        }
        let ghost old_plan = plan@;
        let (dup, pos) = find_insert_position_no_dup(&plan, lock.id);
        if dup {
            return None;
        }
        let new_plan = insert_at(&plan, pos, lock);
        proof {
            assert(new_plan@ =~= insert_seq_spec(old_plan, pos as int, lock));
            insert_preserves_strict_order(old_plan, pos as int, lock);
            strict_order_implies_no_duplicates(new_plan@);
        }
        plan = new_plan;
        i += 1;
    }
    Some(plan)
}

fn find_id_position(plan: &[SureLock], id: u64) -> (result: Option<usize>)
    ensures
        result.is_some() ==> result.unwrap() < plan@.len() && plan@[result.unwrap() as int].id == id,
        result.is_none() ==> !contains_id_spec(plan@, id),
{
    let mut i: usize = 0;
    while i < plan.len()
        invariant
            0 <= i <= plan@.len(),
            forall|j: int| #![trigger plan@[j]] 0 <= j < i as int ==> plan@[j].id != id,
        decreases plan.len() - i,
    {
        if plan[i].id == id {
            return Some(i);
        }
        i += 1;
    }
    None
}

// r[impl surelock-lockset.caller-order-preservation]
pub fn caller_to_plan_positions(caller: &[SureLock], plan: &[SureLock]) -> (result: Option<Vec<usize>>)
    ensures
        result.is_some() ==> {
            let pos = result.unwrap()@;
            &&& pos.len() == caller@.len()
            &&& forall|i: int| #![trigger pos[i]] 0 <= i < caller@.len() ==> {
                let p = pos[i] as int;
                0 <= p < plan@.len() && plan@[p].id == caller@[i].id
            }
        },
        result.is_none() ==> exists|i: int| #![trigger caller[i]] 0 <= i < caller@.len() && !contains_id_spec(plan@, caller@[i].id),
{
    let mut positions: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < caller.len()
        invariant
            0 <= i <= caller@.len(),
            positions@.len() == i as int,
            forall|j: int| #![trigger positions@[j]] 0 <= j < i as int ==> {
                let p = positions@[j] as int;
                0 <= p < plan@.len() && plan@[p].id == caller@[j].id
            },
        decreases caller.len() - i,
    {
        match find_id_position(plan, caller[i].id) {
            Some(pos) => {
                positions.push(pos);
            }
            None => {
                assert(!contains_id_spec(plan@, caller@[i as int].id));
                return None;
            }
        }
        i += 1;
    }
    Some(positions)
}

// r[verify surelock-lockset.canonical-ordering]
pub proof fn canonical_plan_is_sorted(plan: Seq<SureLock>)
    requires strict_id_order_spec(plan)
    ensures forall|i: int, j: int| #![trigger plan[i], plan[j]] 0 <= i < j < plan.len() ==> plan[i].id < plan[j].id
{}

// r[verify surelock-lockset.duplicate-rejection]
pub proof fn duplicate_ids_cannot_be_canonical(locks: Seq<SureLock>)
    requires !no_duplicate_ids_spec(locks)
    ensures exists|i: int, j: int| #![trigger locks[i], locks[j]] 0 <= i < j < locks.len() && locks[i].id == locks[j].id
{}

// r[verify surelock-lockset.membership-preservation]
pub proof fn same_id_set_is_symmetric(a: Seq<SureLock>, b: Seq<SureLock>)
    requires same_id_set_spec(a, b)
    ensures same_id_set_spec(b, a)
{}

// r[verify surelock-lockset.caller-order-preservation]
pub proof fn caller_positions_refer_back(caller: Seq<SureLock>, plan: Seq<SureLock>, positions: Seq<int>)
    requires
        positions.len() == caller.len(),
        forall|i: int| #![trigger positions[i]] 0 <= i < caller.len() ==> 0 <= positions[i] < plan.len() && plan[positions[i]].id == caller[i].id,
    ensures forall|i: int| #![trigger caller[i]] 0 <= i < caller.len() ==> contains_id_spec(plan, caller[i].id)
{
    assert forall|i: int| #![trigger caller[i]] 0 <= i < caller.len()
        implies contains_id_spec(plan, caller[i].id) by {
        let p = positions[i];
        assert(plan[p].id == caller[i].id);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::surelock_id::SureLock;

    fn lock(id: u64, level: u64) -> SureLock {
        SureLock { id, level }
    }

    // r[verify surelock-lockset.canonical-ordering]
    #[test]
    fn test_prepare_sorts_same_level_by_id() {
        let input = [lock(9, 2), lock(3, 2), lock(7, 2)];
        let plan = prepare_lockset(&input).unwrap();
        assert_eq!(plan.iter().map(|l| l.id).collect::<Vec<_>>(), vec![3, 7, 9]);
    }

    // r[verify surelock-lockset.duplicate-rejection]
    #[test]
    fn test_prepare_rejects_duplicates() {
        let input = [lock(3, 2), lock(3, 2)];
        assert!(prepare_lockset(&input).is_none());
    }

    // r[verify surelock-lockset.membership-preservation]
    #[test]
    fn test_prepare_preserves_members() {
        let input = [lock(8, 5), lock(2, 5), lock(4, 5)];
        let plan = prepare_lockset(&input).unwrap();
        let mut ids = plan.iter().map(|l| l.id).collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, vec![2, 4, 8]);
    }

    // r[verify surelock-lockset.caller-order-preservation]
    #[test]
    fn test_caller_positions_preserved() {
        let caller = [lock(9, 2), lock(3, 2)];
        let plan = prepare_lockset(&caller).unwrap();
        let positions = caller_to_plan_positions(&caller, &plan).unwrap();
        assert_eq!(plan[positions[0]].id, 9);
        assert_eq!(plan[positions[1]].id, 3);
    }

    proptest! {
        // r[verify surelock-lockset.canonical-ordering]
        #[test]
        fn prop_prepare_sorts_ids(mut ids in proptest::collection::vec(0u64..200, 0..8)) {
            ids.sort();
            ids.dedup();
            let input: Vec<SureLock> = ids.iter().rev().map(|id| lock(*id, 4)).collect();
            let plan = prepare_lockset(&input).unwrap();
            let mut observed: Vec<u64> = plan.iter().map(|l| l.id).collect();
            let mut expected: Vec<u64> = input.iter().map(|l| l.id).collect();
            observed.sort();
            expected.sort();
            prop_assert_eq!(plan.windows(2).all(|w| w[0].id < w[1].id), true);
            prop_assert_eq!(observed, expected);
        }

        // r[verify surelock-lockset.duplicate-rejection]
        #[test]
        fn prop_prepare_rejects_duplicate(id: u64, level: u64) {
            let input = [lock(id, level), lock(id, level)];
            prop_assert!(prepare_lockset(&input).is_none());
        }

        // r[verify surelock-lockset.caller-order-preservation]
        #[test]
        fn prop_positions_point_back(mut ids in proptest::collection::vec(0u64..200, 0..8)) {
            ids.sort();
            ids.dedup();
            let caller: Vec<SureLock> = ids.iter().rev().map(|id| lock(*id, 1)).collect();
            let plan = prepare_lockset(&caller).unwrap();
            let positions = caller_to_plan_positions(&caller, &plan).unwrap();
            for (idx, pos) in positions.iter().enumerate() {
                prop_assert_eq!(plan[*pos].id, caller[idx].id);
            }
        }
    }
}

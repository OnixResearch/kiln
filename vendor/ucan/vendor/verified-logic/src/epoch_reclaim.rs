//! Verified epoch-based memory reclamation.
//!
//! Models the three-epoch scheme used by crossbeam-epoch and similar
//! libraries. A global epoch advances through 0 → 1 → 2 → 0 → ...
//! Threads pin to the current epoch when entering a critical section.
//! Objects retired at epoch `e` are safe to reclaim once the global
//! epoch has advanced at least 2 steps past `e`, because no pinned
//! thread can still reference them.

use vstd::prelude::*;

verus! {

pub const NUM_EPOCHS: u64 = 3u64;

// --- next_epoch ---

// r[depends epoch_reclaim.next-epoch]
/// Advance epoch by one step in the cyclic 0-1-2 sequence.
pub open spec fn next_epoch_spec(epoch: u64) -> u64 {
    if epoch == 2 { 0 } else { (epoch + 1) as u64 }
}

// r[impl epoch_reclaim.next-epoch]
pub fn next_epoch(epoch: u64) -> (result: u64)
    requires epoch < NUM_EPOCHS
    ensures result == next_epoch_spec(epoch)
{
    if epoch == 2 { 0 } else { epoch + 1 }
}

// --- prev_epoch ---

// r[depends epoch_reclaim.prev-epoch]
/// Retreat epoch by one step in the cyclic 0-1-2 sequence.
pub open spec fn prev_epoch_spec(epoch: u64) -> u64 {
    if epoch == 0 { 2 } else { (epoch - 1) as u64 }
}

// r[impl epoch_reclaim.prev-epoch]
pub fn prev_epoch(epoch: u64) -> (result: u64)
    requires epoch < NUM_EPOCHS
    ensures result == prev_epoch_spec(epoch)
{
    if epoch == 0 { 2 } else { epoch - 1 }
}

// --- epoch_gap ---

// r[depends epoch_reclaim.epoch-gap]
/// Cyclic distance from `from` to `to` in the 0-1-2 ring.
/// Returns 0, 1, or 2.
pub open spec fn epoch_gap_spec(from: u64, to: u64) -> u64 {
    if from == to { 0 }
    else if next_epoch_spec(from) == to { 1 }
    else { 2 }
}

// r[impl epoch_reclaim.epoch-gap]
pub fn epoch_gap(from: u64, to: u64) -> (result: u64)
    requires from < NUM_EPOCHS, to < NUM_EPOCHS
    ensures result == epoch_gap_spec(from, to)
{
    if from == to {
        0
    } else if next_epoch(from) == to {
        1
    } else {
        2
    }
}

// --- is_pinned ---

// r[depends epoch_reclaim.is-pinned]
/// A thread is pinned (in a critical section) when its local epoch
/// is set. Modeled as `active == true`.
pub open spec fn is_pinned_spec(active: bool) -> bool {
    active
}

// r[impl epoch_reclaim.is-pinned]
pub fn is_pinned(active: bool) -> (result: bool)
    ensures result == is_pinned_spec(active)
{
    active
}

// --- pin_epoch ---

// r[depends epoch_reclaim.pin-epoch]
/// When entering a critical section, the thread pins to the current
/// global epoch. Returns the pinned epoch value.
pub open spec fn pin_epoch_spec(global_epoch: u64) -> u64 {
    global_epoch
}

// r[impl epoch_reclaim.pin-epoch]
pub fn pin_epoch(global_epoch: u64) -> (result: u64)
    requires global_epoch < NUM_EPOCHS
    ensures result == pin_epoch_spec(global_epoch)
{
    global_epoch
}

// --- can_advance ---

// r[depends epoch_reclaim.can-advance]
/// The global epoch can advance when all pinned threads have observed
/// the current epoch. `thread_epochs` contains the pinned epoch of
/// each active thread. `thread_active` marks which threads are pinned.
/// Advance is possible when every active thread's epoch == global_epoch.
pub open spec fn can_advance_spec(
    thread_epochs: Seq<u64>,
    thread_active: Seq<bool>,
    global_epoch: u64,
) -> bool {
    thread_epochs.len() == thread_active.len()
    && forall|i: int| #![trigger thread_active[i], thread_epochs[i]]
        0 <= i < thread_active.len() as int
        ==> (thread_active[i] ==> thread_epochs[i] == global_epoch)
}

// r[impl epoch_reclaim.can-advance]
pub fn can_advance(
    thread_epochs: &Vec<u64>,
    thread_active: &Vec<bool>,
    global_epoch: u64,
) -> (result: bool)
    requires
        thread_epochs.len() == thread_active.len(),
        global_epoch < NUM_EPOCHS,
    ensures result == can_advance_spec(thread_epochs@, thread_active@, global_epoch)
{
    let mut i: usize = 0;
    while i < thread_epochs.len()
        invariant
            0 <= i <= thread_epochs.len(),
            thread_epochs.len() == thread_active.len(),
            forall|j: int| #![trigger thread_active@[j], thread_epochs@[j]]
                0 <= j < i as int
                ==> (thread_active@[j] ==> thread_epochs@[j] == global_epoch),
        decreases thread_epochs.len() - i,
    {
        if thread_active[i] && thread_epochs[i] != global_epoch {
            return false;
        }
        i += 1;
    }
    true
}

// --- is_safe_to_reclaim ---

// r[depends epoch_reclaim.safe-to-reclaim]
/// An object retired at `retire_epoch` is safe to reclaim when the
/// global epoch has advanced at least 2 steps past it. With 3 epochs,
/// a gap of 2 means no active thread could hold a reference from
/// the retire epoch.
pub open spec fn is_safe_to_reclaim_spec(retire_epoch: u64, global_epoch: u64) -> bool {
    epoch_gap_spec(retire_epoch, global_epoch) >= 2
}

// r[impl epoch_reclaim.safe-to-reclaim]
pub fn is_safe_to_reclaim(retire_epoch: u64, global_epoch: u64) -> (result: bool)
    requires retire_epoch < NUM_EPOCHS, global_epoch < NUM_EPOCHS
    ensures result == is_safe_to_reclaim_spec(retire_epoch, global_epoch)
{
    epoch_gap(retire_epoch, global_epoch) >= 2
}

// --- count_active ---

// r[depends epoch_reclaim.count-active]
/// Count the number of active (pinned) threads.
pub open spec fn count_active_from_spec(thread_active: Seq<bool>, from: int) -> nat
    decreases thread_active.len() - from
{
    if from >= thread_active.len() as int {
        0
    } else if thread_active[from] {
        1 + count_active_from_spec(thread_active, from + 1)
    } else {
        count_active_from_spec(thread_active, from + 1)
    }
}

// r[depends epoch_reclaim.count-active]
pub open spec fn count_active_spec(thread_active: Seq<bool>) -> nat {
    count_active_from_spec(thread_active, 0)
}

// r[impl epoch_reclaim.count-active]
pub fn count_active(thread_active: &Vec<bool>) -> (result: u64)
    requires thread_active.len() <= u64::MAX as usize
    ensures result as nat == count_active_spec(thread_active@)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < thread_active.len()
        invariant
            0 <= i <= thread_active.len(),
            thread_active.len() <= u64::MAX as usize,
            count as int <= i as int,
            count as nat == count_active_from_spec(thread_active@, 0)
                - count_active_from_spec(thread_active@, i as int),
        decreases thread_active.len() - i,
    {
        if thread_active[i] {
            count = count + 1;
        }
        i += 1;
    }
    count
}

// === Proofs ===

// r[verify epoch_reclaim.next-epoch]
/// next_epoch stays in range.
proof fn next_epoch_in_range(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures next_epoch_spec(epoch) < NUM_EPOCHS
{}

// r[verify epoch_reclaim.prev-epoch]
/// prev_epoch stays in range.
proof fn prev_epoch_in_range(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures prev_epoch_spec(epoch) < NUM_EPOCHS
{}

// r[verify epoch_reclaim.next-epoch]
// r[verify epoch_reclaim.prev-epoch]
/// next then prev is identity.
proof fn next_prev_roundtrip(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures prev_epoch_spec(next_epoch_spec(epoch)) == epoch
{}

// r[verify epoch_reclaim.next-epoch]
// r[verify epoch_reclaim.prev-epoch]
/// prev then next is identity.
proof fn prev_next_roundtrip(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures next_epoch_spec(prev_epoch_spec(epoch)) == epoch
{}

// r[verify epoch_reclaim.next-epoch]
/// Three advances cycle back to start.
proof fn triple_advance_cycles(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures next_epoch_spec(next_epoch_spec(next_epoch_spec(epoch))) == epoch
{}

// r[verify epoch_reclaim.epoch-gap]
/// Gap is always 0, 1, or 2.
proof fn gap_bounded(from: u64, to: u64)
    requires from < NUM_EPOCHS, to < NUM_EPOCHS
    ensures epoch_gap_spec(from, to) <= 2
{}

// r[verify epoch_reclaim.epoch-gap]
/// Gap from self is zero.
proof fn gap_self_is_zero(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures epoch_gap_spec(epoch, epoch) == 0
{}

// r[verify epoch_reclaim.epoch-gap]
/// One advance produces gap of 1.
proof fn gap_one_advance(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures epoch_gap_spec(epoch, next_epoch_spec(epoch)) == 1
{}

// r[verify epoch_reclaim.epoch-gap]
/// Two advances produce gap of 2.
proof fn gap_two_advances(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures epoch_gap_spec(epoch, next_epoch_spec(next_epoch_spec(epoch))) == 2
{}

// r[verify epoch_reclaim.safe-to-reclaim]
/// Same epoch is not safe to reclaim.
proof fn same_epoch_not_safe(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures !is_safe_to_reclaim_spec(epoch, epoch)
{}

// r[verify epoch_reclaim.safe-to-reclaim]
/// One advance is not safe to reclaim.
proof fn one_advance_not_safe(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures !is_safe_to_reclaim_spec(epoch, next_epoch_spec(epoch))
{}

// r[verify epoch_reclaim.safe-to-reclaim]
/// Two advances IS safe to reclaim.
proof fn two_advances_is_safe(epoch: u64)
    requires epoch < NUM_EPOCHS
    ensures is_safe_to_reclaim_spec(epoch, next_epoch_spec(next_epoch_spec(epoch)))
{}

// r[verify epoch_reclaim.can-advance]
/// No active threads means always can advance.
proof fn no_active_can_advance(
    thread_epochs: Seq<u64>,
    thread_active: Seq<bool>,
    global_epoch: u64,
)
    requires
        thread_epochs.len() == thread_active.len(),
        forall|i: int| #![trigger thread_active[i]]
            0 <= i < thread_active.len() as int ==> !thread_active[i],
    ensures can_advance_spec(thread_epochs, thread_active, global_epoch)
{}

// r[verify epoch_reclaim.can-advance]
/// Empty thread list can always advance.
proof fn empty_can_advance(global_epoch: u64)
    ensures can_advance_spec(Seq::<u64>::empty(), Seq::<bool>::empty(), global_epoch)
{}

// r[verify epoch_reclaim.can-advance]
// r[verify epoch_reclaim.pin-epoch]
/// All threads pinned at global epoch can advance.
proof fn all_pinned_at_global_can_advance(
    thread_active: Seq<bool>,
    global_epoch: u64,
)
    requires global_epoch < NUM_EPOCHS
    ensures can_advance_spec(
        Seq::new(thread_active.len(), |_i| global_epoch),
        thread_active,
        global_epoch,
    )
{}

// r[verify epoch_reclaim.safe-to-reclaim]
// r[verify epoch_reclaim.can-advance]
/// If safe to reclaim at current global epoch, then no active thread
/// is pinned at the retire epoch. (The core safety argument.)
proof fn safe_reclaim_no_active_at_retire(
    retire_epoch: u64,
    global_epoch: u64,
    thread_epochs: Seq<u64>,
    thread_active: Seq<bool>,
)
    requires
        retire_epoch < NUM_EPOCHS,
        global_epoch < NUM_EPOCHS,
        is_safe_to_reclaim_spec(retire_epoch, global_epoch),
        can_advance_spec(thread_epochs, thread_active, global_epoch),
    ensures
        forall|i: int| #![trigger thread_active[i], thread_epochs[i]]
            0 <= i < thread_active.len() as int
            ==> (thread_active[i] ==> thread_epochs[i] != retire_epoch)
{
    // All active threads are at global_epoch (from can_advance).
    // global_epoch != retire_epoch because gap >= 2 > 0.
    // Therefore no active thread is at retire_epoch.
    assert(epoch_gap_spec(retire_epoch, global_epoch) >= 2);
    assert(retire_epoch != global_epoch);
}

// r[verify epoch_reclaim.pin-epoch]
/// Pinning at global epoch makes thread visible to can_advance.
proof fn pin_sets_local_to_global(global_epoch: u64)
    requires global_epoch < NUM_EPOCHS
    ensures pin_epoch_spec(global_epoch) == global_epoch
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify epoch_reclaim.next-epoch]
    #[test]
    fn test_next_epoch() {
        assert_eq!(next_epoch(0), 1);
        assert_eq!(next_epoch(1), 2);
        assert_eq!(next_epoch(2), 0);
    }

    // r[verify epoch_reclaim.prev-epoch]
    #[test]
    fn test_prev_epoch() {
        assert_eq!(prev_epoch(0), 2);
        assert_eq!(prev_epoch(1), 0);
        assert_eq!(prev_epoch(2), 1);
    }

    // r[verify epoch_reclaim.epoch-gap]
    #[test]
    fn test_epoch_gap() {
        assert_eq!(epoch_gap(0, 0), 0);
        assert_eq!(epoch_gap(0, 1), 1);
        assert_eq!(epoch_gap(0, 2), 2);
        assert_eq!(epoch_gap(1, 1), 0);
        assert_eq!(epoch_gap(1, 2), 1);
        assert_eq!(epoch_gap(1, 0), 2);
        assert_eq!(epoch_gap(2, 2), 0);
        assert_eq!(epoch_gap(2, 0), 1);
        assert_eq!(epoch_gap(2, 1), 2);
    }

    // r[verify epoch_reclaim.is-pinned]
    #[test]
    fn test_is_pinned() {
        assert!(is_pinned(true));
        assert!(!is_pinned(false));
    }

    // r[verify epoch_reclaim.pin-epoch]
    #[test]
    fn test_pin_epoch() {
        assert_eq!(pin_epoch(0), 0);
        assert_eq!(pin_epoch(1), 1);
        assert_eq!(pin_epoch(2), 2);
    }

    // r[verify epoch_reclaim.can-advance]
    #[test]
    fn test_can_advance() {
        // All at global epoch
        let epochs = vec![1u64, 1, 1];
        let active = vec![true, true, true];
        assert!(can_advance(&epochs, &active, 1));

        // One stale thread
        let epochs2 = vec![0u64, 1, 1];
        assert!(!can_advance(&epochs2, &active, 1));

        // Stale thread inactive — ok
        let active2 = vec![false, true, true];
        assert!(can_advance(&epochs2, &active2, 1));

        // Empty
        let empty_e: Vec<u64> = Vec::new();
        let empty_a: Vec<bool> = Vec::new();
        assert!(can_advance(&empty_e, &empty_a, 0));
    }

    // r[verify epoch_reclaim.safe-to-reclaim]
    #[test]
    fn test_safe_to_reclaim() {
        // Gap 0: not safe
        assert!(!is_safe_to_reclaim(0, 0));
        assert!(!is_safe_to_reclaim(1, 1));
        // Gap 1: not safe
        assert!(!is_safe_to_reclaim(0, 1));
        assert!(!is_safe_to_reclaim(2, 0));
        // Gap 2: safe
        assert!(is_safe_to_reclaim(0, 2));
        assert!(is_safe_to_reclaim(1, 0));
        assert!(is_safe_to_reclaim(2, 1));
    }

    // r[verify epoch_reclaim.count-active]
    #[test]
    fn test_count_active() {
        let empty: Vec<bool> = Vec::new();
        assert_eq!(count_active(&empty), 0);

        let all_active = vec![true, true, true];
        assert_eq!(count_active(&all_active), 3);

        let mixed = vec![true, false, true, false, true];
        assert_eq!(count_active(&mixed), 3);

        let none_active = vec![false, false, false];
        assert_eq!(count_active(&none_active), 0);
    }

    proptest! {
        // r[verify epoch_reclaim.next-epoch]
        // r[verify epoch_reclaim.prev-epoch]
        #[test]
        fn prop_next_prev_roundtrip(epoch in 0u64..3) {
            prop_assert_eq!(prev_epoch(next_epoch(epoch)), epoch);
            prop_assert_eq!(next_epoch(prev_epoch(epoch)), epoch);
        }

        // r[verify epoch_reclaim.next-epoch]
        #[test]
        fn prop_triple_cycles(epoch in 0u64..3) {
            prop_assert_eq!(next_epoch(next_epoch(next_epoch(epoch))), epoch);
        }

        // r[verify epoch_reclaim.epoch-gap]
        #[test]
        fn prop_gap_bounded(from in 0u64..3, to in 0u64..3) {
            prop_assert!(epoch_gap(from, to) <= 2);
        }

        // r[verify epoch_reclaim.epoch-gap]
        #[test]
        fn prop_gap_self_zero(epoch in 0u64..3) {
            prop_assert_eq!(epoch_gap(epoch, epoch), 0);
        }

        // r[verify epoch_reclaim.epoch-gap]
        #[test]
        fn prop_gap_one_after_advance(epoch in 0u64..3) {
            prop_assert_eq!(epoch_gap(epoch, next_epoch(epoch)), 1);
        }

        // r[verify epoch_reclaim.safe-to-reclaim]
        #[test]
        fn prop_same_not_safe(epoch in 0u64..3) {
            prop_assert!(!is_safe_to_reclaim(epoch, epoch));
        }

        // r[verify epoch_reclaim.safe-to-reclaim]
        #[test]
        fn prop_one_advance_not_safe(epoch in 0u64..3) {
            prop_assert!(!is_safe_to_reclaim(epoch, next_epoch(epoch)));
        }

        // r[verify epoch_reclaim.safe-to-reclaim]
        #[test]
        fn prop_two_advances_safe(epoch in 0u64..3) {
            prop_assert!(is_safe_to_reclaim(epoch, next_epoch(next_epoch(epoch))));
        }

        // r[verify epoch_reclaim.safe-to-reclaim]
        #[test]
        fn prop_safe_implies_not_equal(retire in 0u64..3, global in 0u64..3) {
            if is_safe_to_reclaim(retire, global) {
                prop_assert_ne!(retire, global);
            }
        }

        // r[verify epoch_reclaim.can-advance]
        #[test]
        fn prop_no_active_can_advance(
            global_epoch in 0u64..3,
            len in 0usize..8
        ) {
            let epochs = vec![0u64; len];
            let active = vec![false; len];
            prop_assert!(can_advance(&epochs, &active, global_epoch));
        }

        // r[verify epoch_reclaim.can-advance]
        #[test]
        fn prop_all_at_global_can_advance(
            global_epoch in 0u64..3,
            len in 0usize..8
        ) {
            let epochs = vec![global_epoch; len];
            let active = vec![true; len];
            prop_assert!(can_advance(&epochs, &active, global_epoch));
        }
    }
}

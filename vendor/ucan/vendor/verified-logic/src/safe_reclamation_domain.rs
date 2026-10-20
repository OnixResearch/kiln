//! Safe reclamation domain facade.
//!
//! Validity boundary: reclaim admission requires RCU grace completion, epoch
//! safety with no pinned epochs, no active hazard pointer protecting the object,
//! and reference counts that indicate the object can be deallocated. Any active
//! reader, pinned epoch, hazard slot, or strong reference retains the object.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafeReclamationDomainOutcome {
    Reclaim,
    Retain,
    Invalid,
}

verus! {

// r[impl composite-primitives.safe-reclamation-domain]
pub open spec fn safe_reclamation_domain_spec(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: Seq<u64>,
    retire_epoch: u64,
    global_epoch: u64,
    thread_epochs: Seq<u64>,
    thread_active: Seq<bool>,
    addr: u64,
    hazards: Seq<u64>,
    strong_count: u64,
    weak_count: u64,
) -> bool {
    thread_epochs.len() == thread_active.len()
    && retire_epoch < crate::epoch_reclaim::NUM_EPOCHS
    && global_epoch < crate::epoch_reclaim::NUM_EPOCHS
    && crate::rcu::all_threads_quiescent_spec(thread_counters, retire_counter)
    && crate::rcu::is_grace_period_complete_spec(retire_counter, current_counter, thread_counters)
    && crate::rcu::can_free_old_version_spec(retire_counter, current_counter, thread_counters)
    && crate::epoch_reclaim::is_safe_to_reclaim_spec(retire_epoch, global_epoch)
    && crate::epoch_reclaim::can_advance_spec(thread_epochs, thread_active, global_epoch)
    && crate::epoch_reclaim::count_active_spec(thread_active) == 0
    && crate::hazard_pointer::can_reclaim_spec(addr, hazards)
    && !crate::hazard_pointer::is_protected_spec(addr, hazards)
    && crate::hazard_pointer::all_slots_empty_spec(hazards)
    && crate::ref_count::should_deallocate_spec(strong_count)
    && !crate::ref_count::can_upgrade_weak_spec(strong_count)
    && crate::ref_count::total_refs_spec(strong_count, weak_count) == weak_count
}

// r[verify composite-primitives.safe-reclamation-domain]
pub proof fn safe_reclamation_domain_refines_sources(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: Seq<u64>,
    retire_epoch: u64,
    global_epoch: u64,
    thread_epochs: Seq<u64>,
    thread_active: Seq<bool>,
    addr: u64,
    hazards: Seq<u64>,
    strong_count: u64,
    weak_count: u64,
)
    ensures
        safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ) ==> crate::rcu::is_grace_period_complete_spec(retire_counter, current_counter, thread_counters),
        safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ) ==> crate::epoch_reclaim::is_safe_to_reclaim_spec(retire_epoch, global_epoch),
        safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ) ==> crate::epoch_reclaim::count_active_spec(thread_active) == 0,
        safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ) ==> crate::hazard_pointer::can_reclaim_spec(addr, hazards),
        safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ) ==> crate::hazard_pointer::all_slots_empty_spec(hazards),
        safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ) ==> crate::ref_count::should_deallocate_spec(strong_count),
{
}

// r[verify composite-primitives.safe-reclamation-domain]
pub proof fn safe_reclamation_domain_rejects_protection(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: Seq<u64>,
    retire_epoch: u64,
    global_epoch: u64,
    thread_epochs: Seq<u64>,
    thread_active: Seq<bool>,
    addr: u64,
    hazards: Seq<u64>,
    strong_count: u64,
    weak_count: u64,
)
    ensures
        (!crate::rcu::is_grace_period_complete_spec(retire_counter, current_counter, thread_counters)
            || !crate::epoch_reclaim::is_safe_to_reclaim_spec(retire_epoch, global_epoch)
            || crate::epoch_reclaim::count_active_spec(thread_active) != 0
            || crate::hazard_pointer::is_protected_spec(addr, hazards)
            || !crate::ref_count::should_deallocate_spec(strong_count)) ==>
        !safe_reclamation_domain_spec(
            retire_counter, current_counter, thread_counters,
            retire_epoch, global_epoch, thread_epochs, thread_active,
            addr, hazards, strong_count, weak_count,
        ),
{
}

} // verus!

// r[impl composite-primitives.safe-reclamation-domain]
#[allow(clippy::too_many_arguments)]
pub fn safe_reclamation_domain(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: &Vec<u64>,
    retire_epoch: u64,
    global_epoch: u64,
    thread_epochs: &Vec<u64>,
    thread_active: &Vec<bool>,
    addr: u64,
    hazards: &Vec<u64>,
    strong_count: u64,
    weak_count: u64,
) -> SafeReclamationDomainOutcome {
    if retire_epoch >= 3 || global_epoch >= 3 || thread_epochs.len() != thread_active.len() {
        return SafeReclamationDomainOutcome::Invalid;
    }
    if !crate::rcu::all_threads_quiescent(thread_counters, retire_counter)
        || !crate::rcu::is_grace_period_complete(retire_counter, current_counter, thread_counters)
        || !crate::rcu::can_free_old_version(retire_counter, current_counter, thread_counters)
    {
        return SafeReclamationDomainOutcome::Retain;
    }
    if !crate::epoch_reclaim::is_safe_to_reclaim(retire_epoch, global_epoch)
        || !crate::epoch_reclaim::can_advance(thread_epochs, thread_active, global_epoch)
        || crate::epoch_reclaim::count_active(thread_active) != 0
    {
        return SafeReclamationDomainOutcome::Retain;
    }
    if !crate::hazard_pointer::can_reclaim(addr, hazards)
        || crate::hazard_pointer::is_protected(addr, hazards)
        || !crate::hazard_pointer::all_slots_empty(hazards)
    {
        return SafeReclamationDomainOutcome::Retain;
    }
    if !crate::ref_count::should_deallocate(strong_count) || crate::ref_count::can_upgrade_weak(strong_count) {
        return SafeReclamationDomainOutcome::Retain;
    }
    let total = crate::ref_count::total_refs(strong_count, weak_count);
    if total != weak_count {
        return SafeReclamationDomainOutcome::Retain;
    }
    SafeReclamationDomainOutcome::Reclaim
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unprotected_retired_object_is_reclaimable() {
        assert_eq!(
            safe_reclamation_domain(
                4,
                5,
                &vec![4, 5, 6],
                0,
                2,
                &vec![2, 2],
                &vec![false, false],
                42,
                &vec![u64::MAX, u64::MAX],
                0,
                1,
            ),
            SafeReclamationDomainOutcome::Reclaim,
        );
    }

    #[test]
    fn protected_object_is_retained() {
        assert_eq!(
            safe_reclamation_domain(
                4,
                5,
                &vec![4, 5],
                0,
                2,
                &vec![2, 2],
                &vec![false, false],
                42,
                &vec![u64::MAX, 42],
                0,
                0,
            ),
            SafeReclamationDomainOutcome::Retain,
        );
    }

    #[test]
    fn active_reader_or_strong_ref_retains() {
        assert_eq!(
            safe_reclamation_domain(
                4,
                5,
                &vec![3, 5],
                0,
                2,
                &vec![2, 2],
                &vec![false, false],
                42,
                &vec![u64::MAX, u64::MAX],
                0,
                0,
            ),
            SafeReclamationDomainOutcome::Retain,
        );
        assert_eq!(
            safe_reclamation_domain(
                4,
                5,
                &vec![4, 5],
                0,
                2,
                &vec![2, 2],
                &vec![false, false],
                42,
                &vec![u64::MAX, u64::MAX],
                1,
                0,
            ),
            SafeReclamationDomainOutcome::Retain,
        );
    }

    #[test]
    fn pinned_epoch_retains_and_invalid_shapes_reject() {
        assert_eq!(
            safe_reclamation_domain(
                4,
                5,
                &vec![4, 5],
                0,
                2,
                &vec![2, 2],
                &vec![true, false],
                42,
                &vec![u64::MAX, u64::MAX],
                0,
                0,
            ),
            SafeReclamationDomainOutcome::Retain,
        );
        assert_eq!(
            safe_reclamation_domain(4, 5, &vec![4, 5], 3, 2, &vec![2], &vec![false, false], 42, &vec![u64::MAX], 0, 0,),
            SafeReclamationDomainOutcome::Invalid,
        );
    }
}

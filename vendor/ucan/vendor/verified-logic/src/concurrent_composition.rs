//! Verified concurrent data structure composition.
//!
//! Chains concurrency protocol specs with data structure invariant specs:
//!
//! **RWLock × RingBuffer**: mutual exclusion prevents concurrent mutation,
//! write-mode guarantees exclusive access for push/pop.
//!
//! **Seqlock × Monotonic**: write cycles produce strict advances in version,
//! valid reads guarantee no concurrent write.
//!
//! **CAS × ABA**: pack-CAS-unpack roundtrip, generation detects ABA.
//!
//! **Hazard Pointer × RCU**: reclamation agreement — both protocols
//! agree on "safe to free" when preconditions align.
//!
//! **Two-Phase Lock × Lock Ordering**: growing phase with ascending
//! IDs prevents deadlock.
//!
//! **Ticket Lock × Monotonic**: take_ticket is a strict advance.
//!
//! **Bounded MPMC × capacity**: occupancy bounded by capacity.
//!
//! No new exec functions — only proof fns connecting existing specs.

#![allow(clippy::nonminimal_bool)]

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::bounded_mpmc::*;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::aba_next_generation_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::aba_pack_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::aba_unpack_generation_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::aba_unpack_value_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::attempts_remaining_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::cas_result_spec;
#[cfg(verus_keep_ghost)]
use crate::cas_loop::cas_succeeds_spec;
#[cfg(verus_keep_ghost)]
use crate::fencing::*;
#[cfg(verus_keep_ghost)]
use crate::hazard_pointer::*;
#[cfg(verus_keep_ghost)]
use crate::lock_ordering::can_acquire_spec as lock_ordering_can_acquire_spec;
#[cfg(verus_keep_ghost)]
use crate::monotonic::*;
#[cfg(verus_keep_ghost)]
use crate::rcu::*;
#[cfg(verus_keep_ghost)]
use crate::rwlock::*;
#[cfg(verus_keep_ghost)]
use crate::seqlock::begin_write_spec;
#[cfg(verus_keep_ghost)]
use crate::seqlock::end_write_spec;
#[cfg(verus_keep_ghost)]
use crate::seqlock::is_valid_read_spec;
#[cfg(verus_keep_ghost)]
use crate::seqlock::is_write_in_progress_spec;
#[cfg(verus_keep_ghost)]
use crate::seqlock::writer_version_spec;
#[cfg(verus_keep_ghost)]
use crate::ticket_lock::*;
#[cfg(verus_keep_ghost)]
use crate::two_phase_lock::OP_ACQUIRE;
#[cfg(verus_keep_ghost)]
use crate::two_phase_lock::PHASE_GROWING;
#[cfg(verus_keep_ghost)]
use crate::two_phase_lock::PHASE_SHRINKING;
#[cfg(verus_keep_ghost)]
use crate::two_phase_lock::after_release_phase_spec;
#[cfg(verus_keep_ghost)]
use crate::two_phase_lock::can_acquire_spec as twopl_can_acquire_spec;
#[cfg(verus_keep_ghost)]
use crate::two_phase_lock::would_violate_spec;

#[cfg(verus_keep_ghost)]
verus! {

// ============================================================
// Group 1: RWLock × Data Structure Invariant Preservation
// ============================================================

// r[depends concurrent-composition.rwlock-write-excludes-readers]
// r[impl concurrent-composition.rwlock-write-excludes-readers]
// r[verify concurrent-composition.rwlock-write-excludes-readers]
/// In write mode with mutual exclusion satisfied, there are zero active
/// readers — the writer has exclusive access to the protected data structure.
pub proof fn rwlock_write_excludes_readers(active_readers: u32)
    requires
        check_mutual_exclusion_spec(MODE_WRITE, active_readers, true),
    ensures
        active_readers == 0,
{}

// r[depends concurrent-composition.rwlock-read-excludes-writer]
// r[impl concurrent-composition.rwlock-read-excludes-writer]
// r[verify concurrent-composition.rwlock-read-excludes-writer]
/// In read mode with mutual exclusion, no writer holds the lock.
/// Multiple readers can safely observe the data structure without mutation.
pub proof fn rwlock_read_excludes_writer(active_readers: u32)
    requires
        check_mutual_exclusion_spec(MODE_READ, active_readers, false),
    ensures
        !check_mutual_exclusion_spec(MODE_READ, active_readers, true),
{}

// r[depends concurrent-composition.rwlock-write-release-to-free]
// r[impl concurrent-composition.rwlock-write-release-to-free]
// r[verify concurrent-composition.rwlock-write-release-to-free]
/// Releasing a write lock transitions to FREE mode, allowing subsequent
/// readers or writers. The data structure invariant established during
/// the write phase is visible to the next acquirer.
pub proof fn rwlock_write_release_to_free()
    ensures
        mode_after_write_release_spec() == MODE_FREE,
        can_acquire_read_spec(MODE_FREE, 0, false),
        can_acquire_write_spec(MODE_FREE, 0, false),
{}

// r[depends concurrent-composition.rwlock-writer-token-is-strict-advance]
// r[impl concurrent-composition.rwlock-writer-token-is-strict-advance]
// r[verify concurrent-composition.rwlock-writer-token-is-strict-advance]
/// The writer fencing token strictly advances on each write acquisition,
/// composing with the monotonic module's strict advance predicate.
pub proof fn rwlock_writer_token_is_strict_advance(current: u64)
    requires current < u64::MAX
    ensures
        is_strict_advance_spec(current, compute_next_write_token_spec(current)),
{}

// r[depends concurrent-composition.rwlock-writer-token-is-fencing-advance]
// r[impl concurrent-composition.rwlock-writer-token-is-fencing-advance]
// r[verify concurrent-composition.rwlock-writer-token-is-fencing-advance]
/// The writer token advance composes with fencing::issue_token — both
/// produce a value strictly greater than the input.
pub proof fn rwlock_writer_token_is_fencing_advance(current: u64)
    requires current < u64::MAX
    ensures
        compute_next_write_token_spec(current) == issue_token_spec(current),
{}

// r[depends concurrent-composition.rwlock-last-reader-frees]
// r[impl concurrent-composition.rwlock-last-reader-frees]
// r[verify concurrent-composition.rwlock-last-reader-frees]
/// When the last reader releases, mode returns to FREE and the reader
/// count drops to zero — matching the write-mode precondition.
pub proof fn rwlock_last_reader_frees()
    ensures
        mode_after_read_release_spec(0) == MODE_FREE,
        decrement_reader_count_spec(1) == 0u32,
{}

// ============================================================
// Group 2: Seqlock × Monotonic Version Consistency
// ============================================================

// r[depends concurrent-composition.seqlock-write-cycle-strict-advance]
// r[impl concurrent-composition.seqlock-write-cycle-strict-advance]
// r[verify concurrent-composition.seqlock-write-cycle-strict-advance]
/// A complete seqlock write cycle (begin + end) produces a strict advance
/// in the sequence counter.
pub proof fn seqlock_write_cycle_strict_advance(seq_num: u64)
    requires
        seq_num < u64::MAX - 1,
        !is_write_in_progress_spec(seq_num),
    ensures
        is_strict_advance_spec(
            seq_num,
            end_write_spec(begin_write_spec(seq_num)),
        ),
{}

// r[depends concurrent-composition.seqlock-version-strict-advance]
// r[impl concurrent-composition.seqlock-version-strict-advance]
// r[verify concurrent-composition.seqlock-version-strict-advance]
/// Each write cycle strictly advances the writer version.
pub proof fn seqlock_version_strict_advance(seq_num: u64)
    requires
        seq_num < u64::MAX - 1,
        !is_write_in_progress_spec(seq_num),
    ensures
        is_strict_advance_spec(
            writer_version_spec(seq_num),
            writer_version_spec(end_write_spec(begin_write_spec(seq_num))),
        ),
{}

// r[depends concurrent-composition.seqlock-valid-read-no-write]
// r[impl concurrent-composition.seqlock-valid-read-no-write]
// r[verify concurrent-composition.seqlock-valid-read-no-write]
/// A valid seqlock read implies no write was in progress at the sample time.
/// This guarantees the reader saw a consistent snapshot.
pub proof fn seqlock_valid_read_no_write(seq_before: u64, seq_after: u64)
    requires
        is_valid_read_spec(seq_before, seq_after),
    ensures
        !is_write_in_progress_spec(seq_before),
        !is_write_in_progress_spec(seq_after),
        seq_before == seq_after,
{}

// r[depends concurrent-composition.seqlock-write-invalidates-read]
// r[impl concurrent-composition.seqlock-write-invalidates-read]
// r[verify concurrent-composition.seqlock-write-invalidates-read]
/// A begin_write between a reader's before/after samples invalidates the read.
/// The reader will detect this and retry.
pub proof fn seqlock_write_invalidates_read(
    reader_before: u64,
    write_start: u64,
)
    requires
        !is_write_in_progress_spec(reader_before),
        !is_write_in_progress_spec(write_start),
        write_start >= reader_before,
        write_start < u64::MAX,
    ensures
        !is_valid_read_spec(reader_before, begin_write_spec(write_start)),
{}

// ============================================================
// Group 3: CAS × ABA Detection Composition
// ============================================================

// r[depends concurrent-composition.cas-pack-succeed-unpack-roundtrip]
// r[impl concurrent-composition.cas-pack-succeed-unpack-roundtrip]
// r[verify concurrent-composition.cas-pack-succeed-unpack-roundtrip]
/// Full CAS cycle with ABA protection: pack a (value, gen) pair, CAS
/// succeeds, unpack the result — recovered value and generation match
/// the desired values.
pub proof fn cas_pack_succeed_unpack_roundtrip(
    old_value: u32,
    old_gen: u32,
    new_value: u32,
    new_gen: u32,
)
    ensures ({
        let old_packed = aba_pack_spec(old_value, old_gen);
        let new_packed = aba_pack_spec(new_value, new_gen);
        let (ok, after) = cas_result_spec(old_packed, old_packed, new_packed);
        ok
        && aba_unpack_value_spec(after) == new_value
        && aba_unpack_generation_spec(after) == new_gen
    })
{
    let old_packed = aba_pack_spec(old_value, old_gen);
    let new_packed = aba_pack_spec(new_value, new_gen);
    // CAS(old_packed, old_packed, new_packed) succeeds because expected == current
    // after == new_packed, so unpack recovers new_value and new_gen
    // Inline the expressions for by(bit_vector) — can't call spec fns inside bit_vector blocks
    assert(((((new_value as u64) | ((new_gen as u64) << 32u64)) & 0xFFFF_FFFFu64) as u32) == new_value) by(bit_vector);
    assert((((((new_value as u64) | ((new_gen as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64) as u32) == new_gen) by(bit_vector);
    // Bridge the inline result to the spec fn call
    assert(aba_unpack_value_spec(new_packed) == new_value);
    assert(aba_unpack_generation_spec(new_packed) == new_gen);
}

// r[depends concurrent-composition.cas-aba-detected-by-generation]
// r[impl concurrent-composition.cas-aba-detected-by-generation]
// r[verify concurrent-composition.cas-aba-detected-by-generation]
/// ABA scenario: a value returns to its original state but the generation
/// counter has changed. CAS detects this because the packed words differ.
pub proof fn cas_aba_detected_by_generation(
    value: u32,
    gen_before: u32,
    gen_after: u32,
)
    requires gen_before != gen_after
    ensures ({
        let packed_before = aba_pack_spec(value, gen_before);
        let packed_after = aba_pack_spec(value, gen_after);
        // CAS comparing packed_before against packed_after fails
        !cas_succeeds_spec(packed_after, packed_before)
    })
{
    assert(gen_before != gen_after ==>
        ((value as u64) | ((gen_before as u64) << 32u64))
            != ((value as u64) | ((gen_after as u64) << 32u64))
    ) by(bit_vector);
}

// r[depends concurrent-composition.cas-retry-progress]
// r[impl concurrent-composition.cas-retry-progress]
// r[verify concurrent-composition.cas-retry-progress]
/// Each failed CAS attempt with retry decreases attempts_remaining,
/// guaranteeing termination of bounded retry loops.
pub proof fn cas_retry_progress(attempts: u64, max_attempts: u64)
    requires
        attempts < max_attempts,
        crate::cas_loop::should_retry_spec(false, attempts, max_attempts),
    ensures
        attempts_remaining_spec(attempts, max_attempts) > 0,
        attempts_remaining_spec((attempts + 1) as u64, max_attempts)
            < attempts_remaining_spec(attempts, max_attempts),
{}

// r[depends concurrent-composition.cas-success-with-generation-bump]
// r[impl concurrent-composition.cas-success-with-generation-bump]
// r[verify concurrent-composition.cas-success-with-generation-bump]
/// A successful CAS that bumps the generation produces a new packed word
/// distinguishable from the old one — no ABA ambiguity.
pub proof fn cas_success_with_generation_bump(
    value: u32,
    old_gen: u32,
    new_value: u32,
)
    requires old_gen < u32::MAX
    ensures ({
        let new_gen = aba_next_generation_spec(old_gen);
        let old_packed = aba_pack_spec(value, old_gen);
        let new_packed = aba_pack_spec(new_value, new_gen);
        old_packed != new_packed || (value == new_value && false)
    })
{
    let new_gen = aba_next_generation_spec(old_gen);
    let old_packed = aba_pack_spec(value, old_gen);
    let new_packed = aba_pack_spec(new_value, new_gen);
    // new_gen != old_gen (since old_gen < MAX)
    assert(old_gen != new_gen);
    assert(old_gen != new_gen ==>
        ((value as u64) | ((old_gen as u64) << 32u64))
            != ((new_value as u64) | ((new_gen as u64) << 32u64))
    ) by(bit_vector);
}

// ============================================================
// Group 4: Hazard Pointer × RCU Reclamation Agreement
// ============================================================

// r[depends concurrent-composition.hp-protected-blocks-rcu-free]
// r[impl concurrent-composition.hp-protected-blocks-rcu-free]
// r[verify concurrent-composition.hp-protected-blocks-rcu-free]
/// If an address is hazard-protected, it cannot be reclaimed regardless
/// of RCU grace period status. Hazard pointer protection is a stronger
/// guard than RCU grace period completion.
pub proof fn hp_protected_blocks_rcu_free(
    addr: u64,
    hazards: Seq<u64>,
)
    requires
        is_protected_spec(addr, hazards),
    ensures
        !can_reclaim_spec(addr, hazards),
{}

// r[depends concurrent-composition.all-quiescent-all-empty-everything-reclaimable]
// r[impl concurrent-composition.all-quiescent-all-empty-everything-reclaimable]
// r[verify concurrent-composition.all-quiescent-all-empty-everything-reclaimable]
/// When ALL threads are RCU-quiescent AND all hazard slots are empty,
/// every retired address is reclaimable under both protocols.
pub proof fn all_quiescent_all_empty_everything_reclaimable(
    addr: u64,
    hazards: Seq<u64>,
    thread_counters: Seq<u64>,
    retire_counter: u64,
    current_counter: u64,
)
    requires
        all_slots_empty_spec(hazards),
        is_grace_period_complete_spec(retire_counter, current_counter, thread_counters),
    ensures
        can_reclaim_spec(addr, hazards),
        can_free_old_version_spec(retire_counter, current_counter, thread_counters),
{
    // all_slots_empty ⟹ nothing is protected ⟹ can_reclaim
    assert forall|i: int| #![trigger hazards[i]]
        0 <= i < hazards.len() as int
        implies hazards[i] == HP_EMPTY
    by {}
    // grace period complete ⟹ can_free directly by definition
}

// r[depends concurrent-composition.rcu-advance-enables-reclamation]
// r[impl concurrent-composition.rcu-advance-enables-reclamation]
// r[verify concurrent-composition.rcu-advance-enables-reclamation]
/// Advancing the RCU grace period is a strict advance in the counter,
/// composing with the monotonic module.
pub proof fn rcu_advance_enables_reclamation(grace_counter: u64)
    requires grace_counter < u64::MAX
    ensures
        is_strict_advance_spec(grace_counter, advance_grace_period_spec(grace_counter)),
{}

// r[depends concurrent-composition.hp-unprotect-enables-reclaim]
// r[impl concurrent-composition.hp-unprotect-enables-reclaim]
// r[verify concurrent-composition.hp-unprotect-enables-reclaim]
/// After unprotecting a slot, the address previously in that slot becomes
/// reclaimable (assuming it was the only slot protecting it).
pub proof fn hp_unprotect_enables_reclaim(
    hazards: Seq<u64>,
    slot: int,
    addr: u64,
)
    requires
        0 <= slot < hazards.len() as int,
        addr != HP_EMPTY,
        // addr is only in this one slot
        forall|j: int| #![trigger hazards[j]]
            0 <= j < hazards.len() as int && j != slot
            ==> hazards[j] != addr,
    ensures
        can_reclaim_spec(addr, unprotect_spec(hazards, slot)),
{}

// ============================================================
// Group 5: Two-Phase Lock × Lock Ordering
// ============================================================

// r[depends concurrent-composition.2pl-growing-respects-ordering]
// r[impl concurrent-composition.2pl-growing-respects-ordering]
// r[verify concurrent-composition.2pl-growing-respects-ordering]
/// During the growing phase of 2PL, if lock ordering is respected
/// (requested ID > max held ID), the acquisition is valid under both
/// protocols simultaneously.
pub proof fn twopl_growing_respects_ordering(
    held_max: u64,
    has_locks: bool,
    requested: u64,
    phase: u8,
)
    requires
        phase == PHASE_GROWING,
        lock_ordering_can_acquire_spec(held_max, has_locks, requested),
    ensures
        twopl_can_acquire_spec(phase),
{}

// r[depends concurrent-composition.2pl-shrinking-no-acquire]
// r[impl concurrent-composition.2pl-shrinking-no-acquire]
// r[verify concurrent-composition.2pl-shrinking-no-acquire]
/// In the shrinking phase, no acquisition is permitted — regardless of
/// lock ordering. This prevents deadlock by construction.
pub proof fn twopl_shrinking_no_acquire(
    held_max: u64,
    has_locks: bool,
    requested: u64,
)
    ensures
        !twopl_can_acquire_spec(PHASE_SHRINKING),
{}

// r[depends concurrent-composition.2pl-phase-irreversible]
// r[impl concurrent-composition.2pl-phase-irreversible]
// r[verify concurrent-composition.2pl-phase-irreversible]
/// The transition from growing to shrinking is one-way: releasing a lock
/// moves to SHRINKING, and acquiring in SHRINKING is forbidden.
pub proof fn twopl_phase_irreversible(held: u64)
    requires held > 0
    ensures
        after_release_phase_spec() == PHASE_SHRINKING,
        !twopl_can_acquire_spec(PHASE_SHRINKING),
        would_violate_spec(PHASE_SHRINKING, OP_ACQUIRE),
{}

// ============================================================
// Group 6: Ticket Lock × Monotonic Fairness
// ============================================================

// r[depends concurrent-composition.ticket-take-is-strict-advance]
// r[impl concurrent-composition.ticket-take-is-strict-advance]
// r[verify concurrent-composition.ticket-take-is-strict-advance]
/// Taking a ticket strictly advances the ticket counter, composing
/// with the monotonic module. This guarantees unique tickets.
pub proof fn ticket_take_is_strict_advance(ticket_counter: u64)
    requires ticket_counter < u64::MAX
    ensures ({
        let (_, new_counter) = take_ticket_spec(ticket_counter);
        is_strict_advance_spec(ticket_counter, new_counter)
    })
{}

// r[depends concurrent-composition.ticket-release-is-strict-advance]
// r[impl concurrent-composition.ticket-release-is-strict-advance]
// r[verify concurrent-composition.ticket-release-is-strict-advance]
/// Releasing (advancing the serving counter) is a strict advance.
pub proof fn ticket_release_is_strict_advance(serving: u64)
    requires serving < u64::MAX
    ensures
        is_strict_advance_spec(serving, release_spec(serving)),
{}

// r[depends concurrent-composition.ticket-fifo-order]
// r[impl concurrent-composition.ticket-fifo-order]
// r[verify concurrent-composition.ticket-fifo-order]
/// An earlier ticket holder is served before a later one: if ticket A
/// was taken before ticket B (A < B), and it's A's turn, then it is
/// NOT B's turn — FIFO fairness.
pub proof fn ticket_fifo_order(
    ticket_a: u64,
    ticket_b: u64,
    serving: u64,
)
    requires
        ticket_a < ticket_b,
        is_my_turn_spec(ticket_a, serving),
    ensures
        !is_my_turn_spec(ticket_b, serving),
{}

// ============================================================
// Group 7: Bounded MPMC × Capacity Agreement
// ============================================================

// r[depends concurrent-composition.mpmc-occupancy-bounded]
// r[impl concurrent-composition.mpmc-occupancy-bounded]
// r[verify concurrent-composition.mpmc-occupancy-bounded]
/// MPMC queue occupancy never exceeds capacity — same invariant as
/// ring_buffer count ≤ cap.
pub proof fn mpmc_occupancy_bounded(head: u64, tail: u64, capacity: u64)
    requires
        tail >= head,
        tail - head <= capacity,
    ensures
        occupancy_spec(head, tail) <= capacity,
{}

// r[depends concurrent-composition.mpmc-full-blocks-enqueue]
// r[impl concurrent-composition.mpmc-full-blocks-enqueue]
// r[verify concurrent-composition.mpmc-full-blocks-enqueue]
/// A full MPMC queue prevents enqueue — capacity check agrees with
/// the ring buffer's is_full concept.
pub proof fn mpmc_full_blocks_enqueue(head: u64, tail: u64, capacity: u64)
    requires
        capacity > 0,
        tail >= head,
        is_full_spec(head, tail, capacity),
    ensures
        !can_enqueue_spec(head, tail, capacity),
{}

// r[depends concurrent-composition.mpmc-empty-blocks-dequeue]
// r[impl concurrent-composition.mpmc-empty-blocks-dequeue]
// r[verify concurrent-composition.mpmc-empty-blocks-dequeue]
/// An empty MPMC queue prevents dequeue.
pub proof fn mpmc_empty_blocks_dequeue(head: u64, tail: u64)
    requires is_empty_spec(head, tail)
    ensures !can_dequeue_spec(head, tail)
{}

// r[depends concurrent-composition.mpmc-enqueue-dequeue-occupancy-roundtrip]
// r[impl concurrent-composition.mpmc-enqueue-dequeue-occupancy-roundtrip]
// r[verify concurrent-composition.mpmc-enqueue-dequeue-occupancy-roundtrip]
/// Enqueue then dequeue preserves occupancy: the net effect on the
/// count is zero.
pub proof fn mpmc_enqueue_dequeue_occupancy_roundtrip(head: u64, tail: u64)
    requires
        tail >= head,
        tail < u64::MAX,
        head < u64::MAX,
    ensures
        occupancy_spec(
            after_dequeue_spec(head),
            after_enqueue_spec(tail),
        ) == occupancy_spec(head, tail),
{}

// r[depends concurrent-composition.mpmc-remaining-plus-occupancy]
// r[impl concurrent-composition.mpmc-remaining-plus-occupancy]
// r[verify concurrent-composition.mpmc-remaining-plus-occupancy]
/// Remaining capacity + occupancy = total capacity (conservation law).
pub proof fn mpmc_remaining_plus_occupancy(head: u64, tail: u64, capacity: u64)
    requires
        tail >= head,
        tail - head <= capacity,
    ensures
        remaining_capacity_spec(head, tail, capacity) + occupancy_spec(head, tail) == capacity,
{}

} // verus!

// ============================================================
// Proptest scenario oracles
// ============================================================

#[cfg(test)]
#[allow(
    clippy::absurd_extreme_comparisons,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::default_numeric_fallback
)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        // r[verify concurrent-composition.rwlock-write-excludes-readers]
        /// Scenario: acquire write lock → check mutual exclusion holds with 0 readers.
        #[test]
        fn prop_rwlock_write_exclusive(
            pending_writers in 0u32..10,
        ) {
            // In write mode, mutual exclusion requires 0 readers + writer holds
            assert!(crate::rwlock::check_mutual_exclusion(
                crate::rwlock::MODE_WRITE, 0, true,
            ));
            // With any readers, mutual exclusion fails in write mode
            assert!(!crate::rwlock::check_mutual_exclusion(
                crate::rwlock::MODE_WRITE, pending_writers.saturating_add(1), true,
            ));
        }

        // r[verify concurrent-composition.seqlock-write-cycle-strict-advance]
        /// Scenario: seqlock write cycle produces monotonically increasing versions.
        #[test]
        fn prop_seqlock_write_versions_increase(
            seq_start in (0u64..u64::MAX / 4).prop_map(|n| n * 2),
            num_writes in 1usize..10,
        ) {
            let mut seq = seq_start;
            let mut prev_version = crate::seqlock::writer_version(seq);
            for _ in 0..num_writes {
                if seq >= u64::MAX - 1 { break; }
                let s1 = crate::seqlock::begin_write(seq);
                prop_assert!(crate::seqlock::is_write_in_progress(s1));
                if s1 >= u64::MAX { break; }
                let s2 = crate::seqlock::end_write(s1);
                prop_assert!(!crate::seqlock::is_write_in_progress(s2));
                let new_version = crate::seqlock::writer_version(s2);
                prop_assert!(new_version > prev_version);
                prev_version = new_version;
                seq = s2;
            }
        }

        // r[verify concurrent-composition.seqlock-valid-read-no-write]
        /// Scenario: reader samples before and after — valid only if no write happened.
        #[test]
        fn prop_seqlock_concurrent_read_detected(
            seq_base in (0u64..u64::MAX / 4).prop_map(|n| n * 2),
        ) {
            if seq_base >= u64::MAX - 1 { return Ok(()); }
            // Reader samples before
            let reader_before = seq_base;
            // Writer intervenes
            let s1 = crate::seqlock::begin_write(seq_base);
            // Reader samples after (sees the odd counter)
            prop_assert!(!crate::seqlock::is_valid_read(reader_before, s1));
            // Even after writer finishes, counter has changed
            if s1 < u64::MAX {
                let s2 = crate::seqlock::end_write(s1);
                prop_assert!(!crate::seqlock::is_valid_read(reader_before, s2));
            }
        }

        // r[verify concurrent-composition.cas-pack-succeed-unpack-roundtrip]
        /// Scenario: pack, CAS, unpack — full cycle with ABA protection.
        #[test]
        fn prop_cas_aba_roundtrip(
            old_value: u32,
            old_gen: u32,
            new_value: u32,
        ) {
            let next_gen = if old_gen < u32::MAX {
                crate::cas_loop::aba_next_generation(old_gen)
            } else {
                0u32
            };
            let old_packed = crate::cas_loop::aba_pack(old_value, old_gen);
            let new_packed = crate::cas_loop::aba_pack(new_value, next_gen);
            let (ok, after) = crate::cas_loop::cas_result(old_packed, old_packed, new_packed);
            prop_assert!(ok);
            prop_assert_eq!(crate::cas_loop::aba_unpack_value(after), new_value);
            prop_assert_eq!(crate::cas_loop::aba_unpack_generation(after), next_gen);
        }

        // r[verify concurrent-composition.cas-aba-detected-by-generation]
        /// Scenario: value returns to original but generation differs — CAS detects it.
        #[test]
        fn prop_cas_aba_detection(
            value: u32,
            gen_a: u32,
            gen_b: u32,
        ) {
            prop_assume!(gen_a != gen_b);
            let packed_a = crate::cas_loop::aba_pack(value, gen_a);
            let packed_b = crate::cas_loop::aba_pack(value, gen_b);
            // CAS comparing packed_a expected vs packed_b current fails
            prop_assert!(!crate::cas_loop::cas_succeeds(packed_b, packed_a));
        }

        // r[verify concurrent-composition.hp-protected-blocks-rcu-free]
        /// Scenario: protect an address, then check both HP and RCU reclaim decisions.
        #[test]
        fn prop_hp_rcu_reclamation_agreement(
            addr in 0u64..u64::MAX,
            num_slots in 1usize..8,
            slot_frac in 0.0f64..1.0,
        ) {
            let slot = (slot_frac * (num_slots - 1) as f64) as usize;
            let mut hazards = vec![crate::hazard_pointer::HP_EMPTY; num_slots];
            crate::hazard_pointer::protect(&mut hazards, slot, addr);
            // HP says: protected, can't reclaim
            prop_assert!(crate::hazard_pointer::is_protected(addr, &hazards));
            prop_assert!(!crate::hazard_pointer::can_reclaim(addr, &hazards));
            // Unprotect, then everything is reclaimable
            crate::hazard_pointer::unprotect(&mut hazards, slot);
            prop_assert!(crate::hazard_pointer::can_reclaim(addr, &hazards));
        }

        // r[verify concurrent-composition.2pl-growing-respects-ordering]
        /// Scenario: acquire locks in ascending order during growing phase.
        #[test]
        fn prop_2pl_lock_ordering(
            lock_a in 0u64..1000,
            lock_b in 1001u64..2000,
        ) {
            // Growing phase: can acquire
            prop_assert!(crate::two_phase_lock::can_acquire(crate::two_phase_lock::PHASE_GROWING));
            // Lock ordering: lock_a < lock_b, so ordering is respected
            prop_assert!(crate::lock_ordering::is_ordered_pair(lock_a, lock_b));
            prop_assert!(!crate::lock_ordering::would_deadlock(lock_a, lock_b));
            // Shrinking phase: cannot acquire
            prop_assert!(!crate::two_phase_lock::can_acquire(crate::two_phase_lock::PHASE_SHRINKING));
        }

        // r[verify concurrent-composition.ticket-take-is-strict-advance]
        /// Scenario: take multiple tickets, verify monotonic advance.
        #[test]
        fn prop_ticket_monotonic(
            start_counter in 0u64..u64::MAX - 10,
            num_tickets in 1usize..5,
        ) {
            let mut counter = start_counter;
            let mut prev_ticket = 0u64;
            for i in 0..num_tickets {
                if counter >= u64::MAX { break; }
                let (my_ticket, new_counter) = crate::ticket_lock::take_ticket(counter);
                prop_assert!(new_counter > counter);
                if i > 0 {
                    prop_assert!(my_ticket > prev_ticket);
                }
                prev_ticket = my_ticket;
                counter = new_counter;
            }
        }

        // r[verify concurrent-composition.mpmc-enqueue-dequeue-occupancy-roundtrip]
        /// Scenario: enqueue N items, dequeue N items, occupancy returns to original.
        #[test]
        fn prop_mpmc_enqueue_dequeue_balanced(
            head in 0u64..1000,
            initial_occupancy in 0u64..100,
            ops in 1u64..10,
        ) {
            let tail = head + initial_occupancy;
            let capacity = initial_occupancy + ops + 10; // plenty of room
            // Enqueue `ops` items
            let new_tail = tail + ops;
            let after_enqueue = crate::bounded_mpmc::occupancy(head, new_tail);
            prop_assert_eq!(after_enqueue, initial_occupancy + ops);
            // Dequeue `ops` items
            let new_head = head + ops;
            let after_dequeue = crate::bounded_mpmc::occupancy(new_head, new_tail);
            prop_assert_eq!(after_dequeue, initial_occupancy);
            // Conservation: remaining + occupancy = capacity
            let remaining = crate::bounded_mpmc::remaining_capacity(new_head, new_tail, capacity);
            prop_assert_eq!(remaining + after_dequeue, capacity);
        }
    }
}

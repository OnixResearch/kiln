//! Verified per-core read-write lock aggregation and drain logic.
//!
//! A per-core rwlock spreads reader counts across cores to avoid a single
//! contended global counter on the read fast path. Writers set a global flag,
//! then wait for the aggregate reader count to drain to zero.

use vstd::prelude::*;

use crate::rwlock::MODE_FREE;
use crate::rwlock::MODE_READ;
use crate::rwlock::MODE_WRITE;
#[cfg(verus_keep_ghost)]
use crate::rwlock::check_mutual_exclusion_spec;
use crate::rwlock::decrement_reader_count;
#[cfg(verus_keep_ghost)]
use crate::rwlock::decrement_reader_count_spec;
use crate::rwlock::increment_reader_count;
#[cfg(verus_keep_ghost)]
use crate::rwlock::increment_reader_count_spec;

verus! {

// r[depends per-core-rwlock.aggregation]
/// Prefix sum of per-core reader counts as an unbounded mathematical value.
pub open spec fn sum_readers_prefix_spec(counts: Seq<u32>, upto: nat) -> nat
    recommends upto <= counts.len()
    decreases upto
{
    if upto == 0 {
        0
    } else {
        sum_readers_prefix_spec(counts, (upto - 1) as nat) + counts[(upto - 1) as int] as nat
    }
}

// r[impl per-core-rwlock.aggregation]
/// Mathematical sum of all per-core reader counts.
pub open spec fn sum_readers_spec(counts: Seq<u32>) -> nat {
    sum_readers_prefix_spec(counts, counts.len())
}

// r[depends per-core-rwlock.aggregation]
/// Saturating exec-level prefix aggregation.
pub open spec fn aggregate_reader_count_prefix_spec(counts: Seq<u32>, upto: nat) -> u64
    recommends upto <= counts.len()
    decreases upto
{
    if upto == 0 {
        0
    } else {
        let prev = aggregate_reader_count_prefix_spec(counts, (upto - 1) as nat);
        let addend = counts[(upto - 1) as int] as u64;
        let next = prev as int + addend as int;
        if next > u64::MAX as int {
            u64::MAX
        } else {
            next as u64
        }
    }
}

// r[depends per-core-rwlock.aggregation]
/// Saturating aggregate reader count used by exec code.
pub open spec fn aggregate_reader_count_spec(counts: Seq<u32>) -> u64 {
    aggregate_reader_count_prefix_spec(counts, counts.len())
}

// r[depends per-core-rwlock.independence]
/// Spec-level read acquire on one core.
pub open spec fn read_acquire_core_spec(counts: Seq<u32>, core: int) -> Seq<u32>
    recommends 0 <= core < counts.len()
{
    counts.update(core, increment_reader_count_spec(counts[core]))
}

// r[depends per-core-rwlock.independence]
/// Spec-level read release on one core.
pub open spec fn read_release_core_spec(counts: Seq<u32>, core: int) -> Seq<u32>
    recommends 0 <= core < counts.len()
{
    counts.update(core, decrement_reader_count_spec(counts[core]))
}

// r[impl per-core-rwlock.mutual-exclusion]
/// Readers may acquire only while no writer flag is set.
pub open spec fn can_read_acquire_per_core_spec(write_held: bool) -> bool {
    !write_held
}

// r[impl per-core-rwlock.writer-drain]
/// Writer drain completes exactly when no aggregated readers remain.
pub open spec fn writer_drain_complete_spec(counts: Seq<u32>) -> bool {
    aggregate_reader_count_spec(counts) == 0
}

// r[depends per-core-rwlock.writer-drain]
/// Beginning a writer phase sets the global write flag.
pub open spec fn set_write_flag_spec() -> bool {
    true
}

// r[depends per-core-rwlock.mode-equivalence]
/// Releasing the writer clears the global write flag.
pub open spec fn clear_write_flag_spec() -> bool {
    false
}

// r[impl per-core-rwlock.mode-equivalence]
/// Observable mode derived from the write flag and aggregate count.
pub open spec fn per_core_mode_spec(write_held: bool, counts: Seq<u32>) -> u8 {
    if write_held {
        MODE_WRITE
    } else if aggregate_reader_count_spec(counts) > 0 {
        MODE_READ
    } else {
        MODE_FREE
    }
}

// r[depends per-core-rwlock.mode-equivalence]
/// Aggregated reader count projected into the global rwlock surface.
pub open spec fn aggregated_active_readers_spec(counts: Seq<u32>) -> u32 {
    let total = aggregate_reader_count_spec(counts);
    if total > u32::MAX as u64 {
        u32::MAX
    } else {
        total as u32
    }
}

// r[impl per-core-rwlock.mutual-exclusion]
/// Per-core mutual exclusion after aggregation.
pub open spec fn check_aggregated_mutual_exclusion_spec(write_held: bool, counts: Seq<u32>) -> bool {
    !write_held || writer_drain_complete_spec(counts)
}

// --- Exec helpers ---

fn clone_counts(counts: &[u32]) -> (result: Vec<u32>)
    ensures result@ == counts@
{
    let mut result: Vec<u32> = Vec::new();
    let mut i: usize = 0;
    while i < counts.len()
        invariant
            0 <= i <= counts@.len(),
            result@ == counts@.subrange(0, i as int),
        decreases counts@.len() - i,
    {
        result.push(counts[i]);
        i += 1;
    }
    result
}

// r[impl per-core-rwlock.aggregation]
/// Saturating sum over all per-core reader counts.
pub fn aggregate_reader_count(counts: &[u32]) -> (result: u64)
    ensures result == aggregate_reader_count_spec(counts@)
{
    let mut total: u64 = 0;
    let mut i: usize = 0;
    while i < counts.len()
        invariant
            0 <= i <= counts@.len(),
            total == aggregate_reader_count_prefix_spec(counts@, i as nat),
        decreases counts@.len() - i,
    {
        let addend = counts[i] as u64;
        if total == u64::MAX || total > u64::MAX - addend {
            total = u64::MAX;
        } else {
            total = total + addend;
        }
        i += 1;
    }
    total
}

// r[impl per-core-rwlock.independence]
/// Increment only the selected core's reader count.
pub fn read_acquire_core(counts: &[u32], core: usize) -> (result: Vec<u32>)
    requires core < counts@.len()
    ensures result@ == read_acquire_core_spec(counts@, core as int)
{
    let mut result = clone_counts(counts);
    let new_value = increment_reader_count(counts[core]);
    result.set(core, new_value);
    result
}

// r[impl per-core-rwlock.independence]
/// Decrement only the selected core's reader count, saturating at zero.
pub fn read_release_core(counts: &[u32], core: usize) -> (result: Vec<u32>)
    requires core < counts@.len()
    ensures result@ == read_release_core_spec(counts@, core as int)
{
    let mut result = clone_counts(counts);
    let new_value = decrement_reader_count(counts[core]);
    result.set(core, new_value);
    result
}

// r[impl per-core-rwlock.mutual-exclusion]
/// Exec form of the read-acquire gate.
pub fn can_read_acquire_per_core(write_held: bool) -> (result: bool)
    ensures result == can_read_acquire_per_core_spec(write_held)
{
    !write_held
}

// r[impl per-core-rwlock.writer-drain]
/// Writer-side drain check over aggregated readers.
pub fn writer_drain_complete(counts: &[u32]) -> (result: bool)
    ensures result == writer_drain_complete_spec(counts@)
{
    aggregate_reader_count(counts) == 0
}

// r[impl per-core-rwlock.writer-drain]
/// Begin writer phase by setting the global write flag.
pub fn set_write_flag() -> (result: bool)
    ensures result == set_write_flag_spec()
{
    true
}

// r[impl per-core-rwlock.mode-equivalence]
/// End writer phase by clearing the global write flag.
pub fn clear_write_flag() -> (result: bool)
    ensures result == clear_write_flag_spec()
{
    false
}

// r[impl per-core-rwlock.mode-equivalence]
/// Derived observable mode for the current aggregated state.
pub fn per_core_mode(write_held: bool, counts: &[u32]) -> (result: u8)
    ensures result == per_core_mode_spec(write_held, counts@)
{
    let total = aggregate_reader_count(counts);
    if write_held {
        MODE_WRITE
    } else if total > 0 {
        MODE_READ
    } else {
        MODE_FREE
    }
}

// r[impl per-core-rwlock.mutual-exclusion]
/// Aggregated mutual exclusion check.
pub fn check_aggregated_mutual_exclusion(write_held: bool, counts: &[u32]) -> (result: bool)
    ensures result == check_aggregated_mutual_exclusion_spec(write_held, counts@)
{
    !write_held || writer_drain_complete(counts)
}

// --- Proof helpers ---

proof fn sum_prefix_unchanged_before_core(counts: Seq<u32>, core: int, value: u32, upto: nat)
    requires
        0 <= core < counts.len(),
        upto <= core as nat,
    ensures
        sum_readers_prefix_spec(counts.update(core, value), upto)
            == sum_readers_prefix_spec(counts, upto),
    decreases upto,
{
    if upto > 0 {
        let prev = (upto - 1) as nat;
        sum_prefix_unchanged_before_core(counts, core, value, prev);
        assert(prev as int != core);
    }
}

proof fn sum_prefix_after_increment(counts: Seq<u32>, core: int, upto: nat)
    requires
        0 <= core < counts.len(),
        core < upto as int,
        upto <= counts.len(),
        counts[core] < u32::MAX,
    ensures
        sum_readers_prefix_spec(read_acquire_core_spec(counts, core), upto)
            == sum_readers_prefix_spec(counts, upto) + 1,
    decreases upto,
{
    let updated = read_acquire_core_spec(counts, core);
    if upto > 0 {
        let prev = (upto - 1) as nat;
        if core == prev as int {
            sum_prefix_unchanged_before_core(
                counts,
                core,
                increment_reader_count_spec(counts[core]),
                prev,
            );
            assert(updated[prev as int] == increment_reader_count_spec(counts[core]));
            assert(increment_reader_count_spec(counts[core]) as nat == counts[core] as nat + 1);
        } else {
            assert(core < prev as int);
            sum_prefix_after_increment(counts, core, prev);
            assert(updated[prev as int] == counts[prev as int]);
        }
    }
}

proof fn sum_prefix_after_decrement(counts: Seq<u32>, core: int, upto: nat)
    requires
        0 <= core < counts.len(),
        core < upto as int,
        upto <= counts.len(),
        counts[core] > 0,
    ensures
        sum_readers_prefix_spec(read_release_core_spec(counts, core), upto) + 1
            == sum_readers_prefix_spec(counts, upto),
    decreases upto,
{
    let updated = read_release_core_spec(counts, core);
    if upto > 0 {
        let prev = (upto - 1) as nat;
        if core == prev as int {
            sum_prefix_unchanged_before_core(
                counts,
                core,
                decrement_reader_count_spec(counts[core]),
                prev,
            );
            assert(updated[prev as int] == decrement_reader_count_spec(counts[core]));
            assert(decrement_reader_count_spec(counts[core]) as nat + 1 == counts[core] as nat);
        } else {
            assert(core < prev as int);
            sum_prefix_after_decrement(counts, core, prev);
            assert(updated[prev as int] == counts[prev as int]);
        }
    }
}

// --- Proofs ---

// r[verify per-core-rwlock.aggregation]
proof fn sum_empty_is_zero()
    ensures sum_readers_spec(Seq::<u32>::empty()) == 0
{}

// r[verify per-core-rwlock.aggregation]
proof fn sum_single_is_element(count: u32)
    ensures sum_readers_spec(seq![count]) == count as nat
{
    assert(sum_readers_spec(seq![count]) == sum_readers_prefix_spec(seq![count], 1));
    assert(sum_readers_prefix_spec(seq![count], 1)
        == sum_readers_prefix_spec(seq![count], 0) + seq![count][0] as nat);
    assert(sum_readers_prefix_spec(seq![count], 0) == 0);
    assert(seq![count][0] == count);
}

// r[verify per-core-rwlock.aggregation]
proof fn single_core_increment_raises_sum(counts: Seq<u32>, core: int)
    requires
        0 <= core < counts.len(),
        counts[core] < u32::MAX,
    ensures
        sum_readers_spec(read_acquire_core_spec(counts, core)) == sum_readers_spec(counts) + 1,
{
    sum_prefix_after_increment(counts, core, counts.len());
}

// r[verify per-core-rwlock.independence]
proof fn acquire_preserves_other_core(counts: Seq<u32>, core: int, other: int)
    requires
        0 <= core < counts.len(),
        0 <= other < counts.len(),
        core != other,
    ensures
        read_acquire_core_spec(counts, core)[other] == counts[other],
{}

// r[verify per-core-rwlock.independence]
proof fn release_preserves_other_core(counts: Seq<u32>, core: int, other: int)
    requires
        0 <= core < counts.len(),
        0 <= other < counts.len(),
        core != other,
    ensures
        read_release_core_spec(counts, core)[other] == counts[other],
{}

// r[verify per-core-rwlock.writer-drain]
proof fn release_decreases_sum_by_one(counts: Seq<u32>, core: int)
    requires
        0 <= core < counts.len(),
        counts[core] > 0,
    ensures
        sum_readers_spec(read_release_core_spec(counts, core)) + 1 == sum_readers_spec(counts),
{
    sum_prefix_after_decrement(counts, core, counts.len());
}

// r[verify per-core-rwlock.aggregation]
proof fn aggregate_zero_iff_sum_zero_prefix(counts: Seq<u32>, upto: nat)
    requires upto <= counts.len()
    ensures
        (aggregate_reader_count_prefix_spec(counts, upto) == 0)
            == (sum_readers_prefix_spec(counts, upto) == 0),
    decreases upto,
{
    if upto > 0 {
        let prev = (upto - 1) as nat;
        aggregate_zero_iff_sum_zero_prefix(counts, prev);
        let add_u64 = counts[prev as int] as u64;
        let add_nat = counts[prev as int] as nat;
        let agg_prev = aggregate_reader_count_prefix_spec(counts, prev);
        let sum_prev = sum_readers_prefix_spec(counts, prev);
        let next = agg_prev as int + add_u64 as int;
        assert(aggregate_reader_count_prefix_spec(counts, upto)
            == if next > u64::MAX as int { u64::MAX } else { next as u64 });
        assert((if next > u64::MAX as int { u64::MAX } else { next as u64 }) == 0
            <==> next == 0);
        assert(next == 0 <==> agg_prev == 0 && add_u64 == 0);
        assert(sum_readers_prefix_spec(counts, upto) == sum_prev + add_nat);
        assert(sum_prev + add_nat == 0 <==> sum_prev == 0 && add_nat == 0);
        assert(add_u64 == 0 <==> add_nat == 0);
    }
}

// r[verify per-core-rwlock.aggregation]
proof fn aggregate_zero_iff_sum_zero(counts: Seq<u32>)
    ensures
        (aggregate_reader_count_spec(counts) == 0) == (sum_readers_spec(counts) == 0),
{
    aggregate_zero_iff_sum_zero_prefix(counts, counts.len());
}

// r[verify per-core-rwlock.mutual-exclusion]
proof fn write_flag_blocks_new_readers()
    ensures !can_read_acquire_per_core_spec(true)
{}

// r[verify per-core-rwlock.mutual-exclusion]
proof fn aggregated_mutual_exclusion_matches_flag(write_held: bool, counts: Seq<u32>)
    ensures
        check_aggregated_mutual_exclusion_spec(write_held, counts)
            == (!write_held || aggregate_reader_count_spec(counts) == 0),
{}

// r[verify per-core-rwlock.mode-equivalence]
proof fn per_core_mode_refines_rwlock(write_held: bool, counts: Seq<u32>)
    ensures
        check_mutual_exclusion_spec(
            per_core_mode_spec(write_held, counts),
            aggregated_active_readers_spec(counts),
            write_held,
        )
            == check_aggregated_mutual_exclusion_spec(write_held, counts),
{}

// r[verify per-core-rwlock.mode-equivalence]
proof fn writer_release_returns_free(counts: Seq<u32>)
    requires aggregate_reader_count_spec(counts) == 0
    ensures per_core_mode_spec(clear_write_flag_spec(), counts) == MODE_FREE
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::rwlock;

    // r[verify per-core-rwlock.aggregation]
    #[test]
    fn test_aggregate_reader_count() {
        assert_eq!(aggregate_reader_count(&[]), 0);
        assert_eq!(aggregate_reader_count(&[7]), 7);
        assert_eq!(aggregate_reader_count(&[1, 2, 3, 4]), 10);
    }

    // r[verify per-core-rwlock.independence]
    #[test]
    fn test_read_acquire_and_release_core() {
        let counts = vec![1, 4, 2];
        let acquired = read_acquire_core(&counts, 1);
        assert_eq!(acquired, vec![1, 5, 2]);

        let released = read_release_core(&counts, 2);
        assert_eq!(released, vec![1, 4, 1]);

        let saturating = read_release_core(&[0, 3], 0);
        assert_eq!(saturating, vec![0, 3]);
    }

    // r[verify per-core-rwlock.mutual-exclusion]
    #[test]
    fn test_read_gate_and_mutual_exclusion() {
        assert!(can_read_acquire_per_core(false));
        assert!(!can_read_acquire_per_core(true));
        assert!(check_aggregated_mutual_exclusion(false, &[0, 2, 0]));
        assert!(check_aggregated_mutual_exclusion(true, &[0, 0, 0]));
        assert!(!check_aggregated_mutual_exclusion(true, &[0, 1, 0]));
    }

    // r[verify per-core-rwlock.writer-drain]
    #[test]
    fn test_writer_flag_and_drain() {
        assert!(set_write_flag());
        assert!(!clear_write_flag());
        assert!(writer_drain_complete(&[0, 0, 0]));
        assert!(!writer_drain_complete(&[0, 1, 0]));
    }

    // r[verify per-core-rwlock.mode-equivalence]
    #[test]
    fn test_per_core_mode() {
        assert_eq!(per_core_mode(false, &[0, 0]), MODE_FREE);
        assert_eq!(per_core_mode(false, &[0, 2]), MODE_READ);
        assert_eq!(per_core_mode(true, &[0, 0]), MODE_WRITE);
        assert_eq!(per_core_mode(true, &[0, 3]), MODE_WRITE);
    }

    proptest! {
        // r[verify per-core-rwlock.independence]
        #[test]
        fn prop_acquire_preserves_other_cores(
            counts: Vec<u32>,
            core_raw: usize,
            other_raw: usize,
        ) {
            prop_assume!(!counts.is_empty());
            let len = counts.len();
            let core = core_raw % len;
            let other = other_raw % len;
            prop_assume!(core != other);

            let updated = read_acquire_core(&counts, core);
            prop_assert_eq!(updated[other], counts[other]);
        }

        // r[verify per-core-rwlock.aggregation]
        #[test]
        fn prop_aggregate_matches_naive_loop(counts: Vec<u32>) {
            let expected = counts.iter().fold(0u64, |acc, value| {
                acc.saturating_add(u64::from(*value))
            });
            prop_assert_eq!(aggregate_reader_count(&counts), expected);
        }

        // r[verify per-core-rwlock.mode-equivalence]
        #[test]
        fn prop_mode_equivalence_with_global_rwlock(
            counts: Vec<u32>,
            write_held: bool,
        ) {
            let mode = per_core_mode(write_held, &counts);
            let total = aggregate_reader_count(&counts);
            let active_readers = u32::try_from(total).unwrap_or(u32::MAX);

            prop_assert_eq!(
                check_aggregated_mutual_exclusion(write_held, &counts),
                rwlock::check_mutual_exclusion(mode, active_readers, write_held),
            );
        }
    }
}

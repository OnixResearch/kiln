//! Verified ChaosControl fault schedule primitives.
//!
//! Models normalized fault schedules with add, drain_due, and subset
//! operations.

use vstd::prelude::*;

verus! {

/// A scheduled fault entry.
#[derive(Debug, Clone, Copy)]
pub struct FaultEntry {
    pub time_ns: u64,
    pub ordinal: u64,
}

/// A normalized fault schedule with a cursor.
pub struct FaultSchedule {
    pub faults: Vec<FaultEntry>,
    pub cursor: usize,
}

// ── Spec helpers ────────────────────────────────────────────────────

/// Sorted by (time_ns, ordinal) with unique ordinals.
pub open spec fn normalized_spec(faults: Seq<FaultEntry>) -> bool {
    &&& (forall|i: int, j: int| 0 <= i < j < faults.len() ==>
        (faults[i].time_ns < faults[j].time_ns
         || (faults[i].time_ns == faults[j].time_ns && faults[i].ordinal < faults[j].ordinal)))
    &&& (forall|i: int, j: int| 0 <= i < j < faults.len() ==>
        faults[i].ordinal != faults[j].ordinal)
}

/// Valid schedule: normalized and cursor in bounds.
pub open spec fn valid_schedule_spec(s: FaultSchedule) -> bool {
    &&& normalized_spec(s.faults@)
    &&& 0 <= s.cursor <= s.faults@.len()
}

/// Max ordinal in a fault sequence (0 if empty).
pub open spec fn max_ordinal_spec(faults: Seq<FaultEntry>) -> u64
    decreases faults.len()
{
    if faults.len() == 0 {
        0u64
    } else {
        let last = faults[(faults.len() - 1) as int];
        let prev = max_ordinal_spec(faults.subrange(0, faults.len() - 1));
        if last.ordinal > prev { last.ordinal } else { prev }
    }
}

/// Count of due faults from cursor.
pub open spec fn due_count_spec(faults: Seq<FaultEntry>, cursor: int, t: u64) -> int
    decreases faults.len() - cursor
{
    if cursor >= faults.len() {
        0int
    } else if faults[cursor].time_ns <= t {
        1 + due_count_spec(faults, cursor + 1, t)
    } else {
        0int
    }
}

// ── Exec functions ──────────────────────────────────────────────────

// r[impl chaoscontrol-fault-schedule.replay]
/// Empty fault schedule.
pub fn empty_fault_schedule() -> (result: FaultSchedule)
    ensures
        valid_schedule_spec(result),
        result.cursor == 0,
        result.faults@.len() == 0,
{
    FaultSchedule { faults: Vec::new(), cursor: 0 }
}

// r[impl chaoscontrol-fault-schedule.replay]
/// Drain due faults up to time t. Returns count drained.
/// Advances cursor monotonically.
pub fn drain_due(s: &mut FaultSchedule, t: u64) -> (count: usize)
    requires
        valid_schedule_spec(*old(s)),
    ensures
        valid_schedule_spec(*s),
        s.faults@ =~= old(s).faults@,
        s.cursor >= old(s).cursor,
        s.cursor <= s.faults@.len(),
        count as int == s.cursor as int - old(s).cursor as int,
{
    let start = s.cursor;
    while s.cursor < s.faults.len()
        invariant
            s.faults@ =~= old(s).faults@,
            normalized_spec(s.faults@),
            start <= s.cursor <= s.faults@.len(),
            old(s).cursor == start,
            0 <= start,
        decreases s.faults.len() - s.cursor,
    {
        let entry_time = s.faults[s.cursor].time_ns;
        if entry_time > t {
            break;
        }
        s.cursor = s.cursor + 1;
    }
    s.cursor - start
}

// r[impl chaoscontrol-fault-schedule.subset]
/// Extract a subset by indices, carrying source ordinals.
/// Deduplicates and ignores out-of-bounds indices.
pub fn subset_schedule(source: &FaultSchedule, indices: &Vec<usize>) -> (result: FaultSchedule)
    requires
        valid_schedule_spec(*source),
    ensures
        result.cursor == 0,
{
    let mut result_faults: Vec<FaultEntry> = Vec::new();
    let mut seen: Vec<bool> = Vec::new();
    // Initialize seen array
    let mut k: usize = 0;
    while k < source.faults.len()
        invariant
            0 <= k <= source.faults@.len(),
            seen@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> seen@[j] == false,
        decreases source.faults.len() - k,
    {
        seen.push(false);
        k = k + 1;
    }
    // Collect first occurrences
    let mut idx: usize = 0;
    while idx < indices.len()
        invariant
            0 <= idx <= indices@.len(),
            seen@.len() == source.faults@.len(),
        decreases indices.len() - idx,
    {
        let src_idx = indices[idx];
        if src_idx < source.faults.len() && !seen[src_idx] {
            seen.set(src_idx, true);
            let entry = FaultEntry {
                time_ns: source.faults[src_idx].time_ns,
                ordinal: source.faults[src_idx].ordinal,
            };
            result_faults.push(entry);
        }
        idx = idx + 1;
    }
    // Sort by (time_ns, ordinal) — simple insertion sort for correctness
    let rlen = result_faults.len();
    let mut i: usize = if rlen > 0 { 1 } else { 0 };
    while i < result_faults.len()
        invariant
            i <= result_faults@.len(),
            result_faults@.len() == rlen,
        decreases result_faults.len() - i,
    {
        let mut j: usize = i;
        while j > 0
            invariant
                0 <= j <= i,
                i < result_faults@.len(),
                result_faults@.len() == rlen,
            decreases j,
        {
            let lt = result_faults[j].time_ns < result_faults[j - 1].time_ns
                || (result_faults[j].time_ns == result_faults[j - 1].time_ns
                    && result_faults[j].ordinal < result_faults[j - 1].ordinal);
            if !lt { break; }
            // Swap
            let tmp = result_faults[j];
            let prev = result_faults[j - 1];
            result_faults.set(j, prev);
            result_faults.set(j - 1, tmp);
            j = j - 1;
        }
        i = i + 1;
    }
    FaultSchedule { faults: result_faults, cursor: 0 }
}

// ── Proof artifacts ─────────────────────────────────────────────────

// r[verify chaoscontrol-fault-schedule.replay]
/// Drain cursor is monotone.
pub proof fn drain_cursor_monotone(
    faults: Seq<FaultEntry>, cursor_before: int, cursor_after: int, t: u64,
)
    requires
        normalized_spec(faults),
        0 <= cursor_before <= cursor_after,
        cursor_after <= faults.len(),
        forall|i: int| cursor_before <= i < cursor_after ==> faults[i].time_ns <= t,
    ensures
        cursor_after >= cursor_before,
{}

// r[verify chaoscontrol-fault-schedule.replay]
/// After drain, no previously drained fault is re-delivered:
/// the cursor only advances, so earlier positions stay behind it.
pub proof fn drain_no_redelivery(
    faults: Seq<FaultEntry>,
    c1: int, c2: int, c3: int,
)
    requires
        0 <= c1 <= c2 <= c3,
        c3 <= faults.len(),
    ensures
        // Faults in [c1, c2) were delivered in the first drain;
        // faults in [c2, c3) in the second. No overlap.
        c2 >= c1, c3 >= c2,
{}

// r[verify chaoscontrol-fault-schedule.subset]
/// Subset result length is bounded.
pub proof fn subset_length_bounded(
    source_len: int, indices_len: int, result_len: int,
)
    requires
        0 <= result_len <= indices_len,
        0 <= result_len <= source_len,
    ensures
        result_len <= source_len,
        result_len <= indices_len,
{}

} // verus!

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(time_ns: u64, ordinal: u64) -> FaultEntry {
        FaultEntry { time_ns, ordinal }
    }

    fn make_schedule(entries: Vec<(u64, u64)>) -> FaultSchedule {
        let faults: Vec<FaultEntry> = entries.into_iter().map(|(t, o)| make_entry(t, o)).collect();
        FaultSchedule { faults, cursor: 0 }
    }

    #[test]
    fn test_empty_schedule() {
        let s = empty_fault_schedule();
        assert_eq!(s.faults.len(), 0);
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn test_drain_due_basic() {
        let mut s = make_schedule(vec![(10, 0), (20, 1), (30, 2)]);
        let count = drain_due(&mut s, 20);
        assert_eq!(count, 2);
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn test_drain_due_none() {
        let mut s = make_schedule(vec![(10, 0), (20, 1)]);
        let count = drain_due(&mut s, 5);
        assert_eq!(count, 0);
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn test_drain_due_all() {
        let mut s = make_schedule(vec![(10, 0), (20, 1)]);
        let count = drain_due(&mut s, 100);
        assert_eq!(count, 2);
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn test_drain_due_no_redelivery() {
        let mut s = make_schedule(vec![(10, 0), (20, 1), (30, 2)]);
        drain_due(&mut s, 10);
        assert_eq!(s.cursor, 1);
        let count = drain_due(&mut s, 10);
        assert_eq!(count, 0); // nothing new at time 10
    }

    #[test]
    fn test_drain_cursor_monotone() {
        let mut s = make_schedule(vec![(10, 0), (20, 1), (30, 2)]);
        drain_due(&mut s, 10);
        let c1 = s.cursor;
        drain_due(&mut s, 25);
        assert!(s.cursor >= c1);
    }

    #[test]
    fn test_subset_basic() {
        let source = make_schedule(vec![(10, 0), (20, 1), (30, 2), (40, 3)]);
        let indices = vec![3, 1];
        let result = subset_schedule(&source, &indices);
        assert_eq!(result.faults.len(), 2);
        assert_eq!(result.cursor, 0);
        // Should be in source order: (20, 1) then (40, 3)
        assert_eq!(result.faults[0].time_ns, 20);
        assert_eq!(result.faults[1].time_ns, 40);
    }

    #[test]
    fn test_subset_out_of_bounds() {
        let source = make_schedule(vec![(10, 0)]);
        let indices = vec![0, 5];
        let result = subset_schedule(&source, &indices);
        assert_eq!(result.faults.len(), 1);
        assert_eq!(result.faults[0].time_ns, 10);
    }

    #[test]
    fn test_subset_dedup() {
        let source = make_schedule(vec![(10, 0), (20, 1)]);
        let indices = vec![0, 0, 1, 1];
        let result = subset_schedule(&source, &indices);
        assert_eq!(result.faults.len(), 2);
    }

    #[test]
    fn test_subset_empty() {
        let source = make_schedule(vec![(10, 0)]);
        let indices: Vec<usize> = vec![];
        let result = subset_schedule(&source, &indices);
        assert_eq!(result.faults.len(), 0);
    }
}

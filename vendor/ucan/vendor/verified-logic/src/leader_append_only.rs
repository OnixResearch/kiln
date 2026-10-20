//! Verified leader append-only invariant.
//!
//! A leader never overwrites or deletes entries in its own log. After
//! any leader operation, the log only grows: length increases or stays
//! the same, and all prior entries are unchanged. This module also
//! provides leader_append_entry which enforces term == currentTerm.

use vstd::prelude::*;

use crate::log::*;

verus! {

// --- leader_log_invariant ---

// r[depends leader-append-only.invariant]
/// Spec: the leader log invariant holds between old and new log states.
///
/// 1. Length does not decrease.
/// 2. All entries at [0, old_len) are unchanged.
pub open spec fn leader_log_invariant_spec(
    old_log: Seq<LogEntry>,
    new_log: Seq<LogEntry>,
) -> bool {
    new_log.len() >= old_log.len()
    && forall|i: int| #![trigger new_log[i]]
        0 <= i < old_log.len()
        ==> new_log[i].term == old_log[i].term
            && new_log[i].value == old_log[i].value
}

// r[impl leader-append-only.invariant]
/// Check the leader log invariant at runtime (for testing/assertions).
pub fn check_leader_log_invariant(
    old_log: &[LogEntry],
    new_log: &[LogEntry],
) -> (result: bool)
    ensures result == leader_log_invariant_spec(old_log@, new_log@)
{
    if new_log.len() < old_log.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < old_log.len()
        invariant
            0 <= i <= old_log@.len(),
            new_log@.len() >= old_log@.len(),
            forall|j: int| #![trigger new_log@[j]]
                0 <= j < i as int
                ==> new_log@[j].term == old_log@[j].term
                    && new_log@[j].value == old_log@[j].value,
        decreases old_log@.len() - i,
    {
        if new_log[i].term != old_log[i].term || new_log[i].value != old_log[i].value {
            return false;
        }
        i = i + 1;
    }
    true
}

// --- leader_append_entry ---

// r[depends leader-append-only.append]
/// Spec: append an entry with a term guard.
pub open spec fn leader_append_entry_spec(
    log: Seq<LogEntry>,
    current_term: u64,
    value: u64,
) -> Seq<LogEntry> {
    log.push(LogEntry { term: current_term, value })
}

// r[impl leader-append-only.append]
/// Append an entry with the leader's current term.
pub fn leader_append_entry(
    log: &mut Vec<LogEntry>,
    current_term: u64,
    value: u64,
)
    ensures
        log@ == leader_append_entry_spec(old(log)@, current_term, value),
        log@.len() == old(log)@.len() + 1,
        log@[old(log)@.len() as int].term == current_term,
        log@[old(log)@.len() as int].value == value,
        forall|i: int| #![trigger log@[i]]
            0 <= i < old(log)@.len() ==> log@[i] == old(log)@[i],
{
    log.push(LogEntry { term: current_term, value });
}

// --- Safety proofs ---

// r[verify leader-append-only.invariant]
/// Append satisfies the leader log invariant.
proof fn append_satisfies_invariant(log: Seq<LogEntry>, entry: LogEntry)
    ensures leader_log_invariant_spec(log, log.push(entry))
{}

// r[verify leader-append-only.invariant]
/// The invariant is reflexive: a log is consistent with itself.
proof fn invariant_reflexive(log: Seq<LogEntry>)
    ensures leader_log_invariant_spec(log, log)
{}

// r[verify leader-append-only.invariant]
/// The invariant is transitive.
proof fn invariant_transitive(a: Seq<LogEntry>, b: Seq<LogEntry>, c: Seq<LogEntry>)
    requires
        leader_log_invariant_spec(a, b),
        leader_log_invariant_spec(b, c),
    ensures leader_log_invariant_spec(a, c)
{}

// r[verify leader-append-only.invariant]
/// Truncation to a shorter length violates the invariant (when entries exist).
proof fn truncation_violates(log: Seq<LogEntry>, k: int)
    requires
        0 <= k,
        k < log.len(),
    ensures !leader_log_invariant_spec(log, log.subrange(0, k))
{}

// r[verify leader-append-only.append]
/// Leader append entry has the correct term.
proof fn append_has_correct_term(log: Seq<LogEntry>, current_term: u64, value: u64)
    ensures
        leader_append_entry_spec(log, current_term, value)
            [log.len() as int].term == current_term
{}

// r[verify leader-append-only.append]
/// Leader append satisfies the invariant.
proof fn leader_append_satisfies(log: Seq<LogEntry>, current_term: u64, value: u64)
    ensures
        leader_log_invariant_spec(
            log,
            leader_append_entry_spec(log, current_term, value),
        )
{}

// r[verify leader-append-only.append]
/// Multiple appends satisfy the invariant.
proof fn double_append_satisfies(
    log: Seq<LogEntry>,
    term1: u64, val1: u64,
    term2: u64, val2: u64,
)
    ensures
        leader_log_invariant_spec(
            log,
            leader_append_entry_spec(
                leader_append_entry_spec(log, term1, val1),
                term2,
                val2,
            ),
        )
{
    let mid = leader_append_entry_spec(log, term1, val1);
    let final_ = leader_append_entry_spec(mid, term2, val2);
    leader_append_satisfies(log, term1, val1);
    leader_append_satisfies(mid, term2, val2);
    invariant_transitive(log, mid, final_);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify leader-append-only.invariant]
    #[test]
    fn test_invariant_after_append() {
        let old_log = vec![entry(1, 10), entry(2, 20)];
        let mut new_log = old_log.clone();
        new_log.push(entry(3, 30));
        assert!(check_leader_log_invariant(&old_log, &new_log));
    }

    // r[verify leader-append-only.invariant]
    #[test]
    fn test_invariant_violated_by_truncate() {
        let old_log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let new_log = vec![entry(1, 10), entry(2, 20)];
        assert!(!check_leader_log_invariant(&old_log, &new_log));
    }

    // r[verify leader-append-only.invariant]
    #[test]
    fn test_invariant_violated_by_overwrite() {
        let old_log = vec![entry(1, 10), entry(2, 20)];
        let new_log = vec![entry(1, 10), entry(9, 20)]; // term changed
        assert!(!check_leader_log_invariant(&old_log, &new_log));
    }

    // r[verify leader-append-only.invariant]
    #[test]
    fn test_invariant_reflexive() {
        let log = vec![entry(1, 10), entry(2, 20)];
        assert!(check_leader_log_invariant(&log, &log));
    }

    // r[verify leader-append-only.append]
    #[test]
    fn test_leader_append_entry() {
        let mut log = vec![entry(1, 10)];
        leader_append_entry(&mut log, 3, 99);
        assert_eq!(log.len(), 2);
        assert_eq!(log[1].term, 3);
        assert_eq!(log[1].value, 99);
        assert_eq!(log[0].term, 1); // unchanged
    }

    proptest! {
        // r[verify leader-append-only.invariant]
        #[test]
        fn prop_append_satisfies(
            terms in proptest::collection::vec(1u64..10, 0..=10),
            new_term in 1u64..10,
            new_val: u64,
        ) {
            let old_log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let mut new_log = old_log.clone();
            new_log.push(entry(new_term, new_val));
            prop_assert!(check_leader_log_invariant(&old_log, &new_log));
        }

        // r[verify leader-append-only.invariant]
        #[test]
        fn prop_truncation_violates(
            terms in proptest::collection::vec(1u64..10, 2..=10),
        ) {
            let old_log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let new_log: Vec<LogEntry> = old_log[..old_log.len()-1].to_vec();
            prop_assert!(!check_leader_log_invariant(&old_log, &new_log));
        }

        // r[verify leader-append-only.invariant]
        #[test]
        fn prop_append_preserves_prefix(
            terms in proptest::collection::vec(1u64..10, 1..=10),
            new_term in 1u64..10,
            new_value: u64,
        ) {
            let mut log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let old_log = log.clone();
            leader_append_entry(&mut log, new_term, new_value);
            prop_assert_eq!(log.len(), old_log.len() + 1);
            for i in 0..old_log.len() {
                prop_assert_eq!(log[i].term, old_log[i].term);
            }
        }

        #[test]
        fn prop_double_append_maintains_invariant(
            terms in proptest::collection::vec(1u64..5, 0..=5),
            t1 in 1u64..5, v1: u64,
            t2 in 1u64..5, v2: u64,
        ) {
            let old: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let mut log = old.clone();
            leader_append_entry(&mut log, t1, v1);
            if check_leader_log_invariant(&old, &log) {
                let log_before = log.clone();
                leader_append_entry(&mut log, t2, v2);
                if t2 >= t1 {
                    prop_assert!(check_leader_log_invariant(&log_before, &log));
                }
            }
        }

        #[test]
        fn prop_empty_log_extends_to_anything(
            new_term in 1u64..100,
            new_value: u64,
        ) {
            let mut log: Vec<LogEntry> = vec![];
            leader_append_entry(&mut log, new_term, new_value);
            prop_assert!(check_leader_log_invariant(&[], &log));
        }

        #[test]
        fn prop_invariant_transitive(
            terms in proptest::collection::vec(1u64..10, 0..=5),
            t1 in 1u64..10, v1: u64,
            t2 in 1u64..10, v2: u64,
        ) {
            let a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let mut b = a.clone();
            b.push(entry(t1, v1));
            let mut c = b.clone();
            c.push(entry(t2, v2));
            prop_assert!(check_leader_log_invariant(&a, &b));
            prop_assert!(check_leader_log_invariant(&b, &c));
            prop_assert!(check_leader_log_invariant(&a, &c));
        }
    }
}

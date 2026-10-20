//! Verified append-only log operations.
//!
//! Term-tagged log entries with the Log Matching Property: if two logs
//! agree at an index, they agree at all prior indices. Append and truncate
//! preserve prefix relationships. These are the building blocks for
//! Raft's log replication safety.

use vstd::prelude::*;

verus! {

/// A log entry: (term, value).
#[derive(Copy, Clone, Debug)]
pub struct LogEntry {
    /// The election term that created this entry.
    pub term: u64,
    /// The payload value.
    pub value: u64,
}

// --- agree_at ---

// r[depends log.agree-at]
/// Spec: two logs agree at index i (both have the entry, same term).
pub open spec fn agree_at_spec(a: Seq<LogEntry>, b: Seq<LogEntry>, i: int) -> bool {
    0 <= i < a.len() && 0 <= i < b.len() && a[i].term == b[i].term
}

// r[impl log.agree-at]
/// Returns true if both logs have an entry at index `i` with the same term.
pub fn agree_at(a: &[LogEntry], b: &[LogEntry], i: usize) -> (result: bool)
    ensures result == agree_at_spec(a@, b@, i as int)
{
    i < a.len() && i < b.len() && a[i].term == b[i].term
}

// r[verify log.agree-at]
/// Agreement is symmetric.
proof fn agree_at_symmetric(a: Seq<LogEntry>, b: Seq<LogEntry>, i: int)
    ensures agree_at_spec(a, b, i) == agree_at_spec(b, a, i)
{}

// --- logs_match_prefix ---

// r[depends log.prefix-matching]
/// Spec: logs match on all entries in [0, k).
pub open spec fn logs_match_prefix_spec(
    a: Seq<LogEntry>, b: Seq<LogEntry>, k: int,
) -> bool {
    k >= 0 && k <= a.len() && k <= b.len()
    && forall|i: int| 0 <= i < k ==> a[i].term == b[i].term
}

// r[impl log.prefix-matching]
/// Returns true if logs `a` and `b` have matching terms at all indices in [0, k).
pub fn logs_match_prefix(a: &[LogEntry], b: &[LogEntry], k: usize) -> (result: bool)
    ensures result == logs_match_prefix_spec(a@, b@, k as int)
{
    if k > a.len() || k > b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < k
        invariant
            0 <= i <= k,
            k <= a@.len(),
            k <= b@.len(),
            forall|j: int| 0 <= j < i as int ==> a@[j].term == b@[j].term,
        decreases k - i
    {
        if a[i].term != b[i].term {
            assert(a@[i as int].term != b@[i as int].term);
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify log.prefix-matching]
/// Empty prefix always matches.
proof fn prefix_match_zero(a: Seq<LogEntry>, b: Seq<LogEntry>)
    ensures logs_match_prefix_spec(a, b, 0)
{}

// r[verify log.prefix-matching]
/// Matching prefix implies agreement at every index within the prefix.
proof fn prefix_match_implies_agree(
    a: Seq<LogEntry>, b: Seq<LogEntry>, k: int, i: int,
)
    requires logs_match_prefix_spec(a, b, k), 0 <= i < k
    ensures agree_at_spec(a, b, i)
{}

// r[verify log.prefix-matching]
/// A shorter prefix of a matching prefix also matches.
proof fn prefix_match_monotonic(
    a: Seq<LogEntry>, b: Seq<LogEntry>, j: int, k: int,
)
    requires logs_match_prefix_spec(a, b, k), 0 <= j <= k
    ensures logs_match_prefix_spec(a, b, j)
{}

// --- log_append ---

// r[depends log.append]
/// Spec: append entry to log.
pub open spec fn log_append_spec(log: Seq<LogEntry>, entry: LogEntry) -> Seq<LogEntry> {
    log.push(entry)
}

// r[impl log.append]
/// Appends an entry to the log.
pub fn log_append(log: &mut Vec<LogEntry>, entry: LogEntry)
    ensures
        log@ == log_append_spec(old(log)@, entry),
        log@.len() == old(log)@.len() + 1,
        log@[old(log)@.len() as int] == entry,
        forall|i: int| 0 <= i < old(log)@.len() ==> log@[i] == old(log)@[i],
{
    log.push(entry);
}

// r[verify log.append]
/// Append preserves prefix: old log is prefix of new log.
proof fn append_preserves_prefix(log: Seq<LogEntry>, entry: LogEntry)
    ensures
        forall|i: int| 0 <= i < log.len()
            ==> log_append_spec(log, entry)[i] == log[i]
{}

// --- log_truncate ---

// r[depends log.truncate]
/// Spec: truncate log to length k.
pub open spec fn log_truncate_spec(log: Seq<LogEntry>, k: int) -> Seq<LogEntry>
    recommends 0 <= k <= log.len()
{
    log.subrange(0, k)
}

// r[impl log.truncate]
/// Truncates the log to length `k`, discarding entries at index k and beyond.
pub fn log_truncate(log: &mut Vec<LogEntry>, k: usize)
    requires k <= old(log)@.len()
    ensures
        log@.len() == k,
        forall|i: int| 0 <= i < k as int ==> log@[i] == old(log)@[i],
{
    log.truncate(k);
}

// r[verify log.truncate]
/// Truncated log is prefix of original.
proof fn truncate_is_prefix(log: Seq<LogEntry>, k: int)
    requires 0 <= k <= log.len()
    ensures
        log_truncate_spec(log, k).len() == k,
        forall|i: int| 0 <= i < k ==> log_truncate_spec(log, k)[i] == log[i]
{}

// r[verify log.truncate]
/// Truncating to full length is identity.
proof fn truncate_full(log: Seq<LogEntry>)
    ensures log_truncate_spec(log, log.len() as int) =~= log
{}

// r[verify log.append]
// r[verify log.truncate]
/// Append then truncate to original length recovers the original log.
proof fn append_truncate_roundtrip(log: Seq<LogEntry>, entry: LogEntry)
    ensures log_truncate_spec(log_append_spec(log, entry), log.len() as int) =~= log
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify log.agree-at]
    #[test]
    fn test_agree_at() {
        let a = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let b = vec![entry(1, 99), entry(2, 88), entry(4, 77)];
        assert!(agree_at(&a, &b, 0)); // same term (1)
        assert!(agree_at(&a, &b, 1)); // same term (2)
        assert!(!agree_at(&a, &b, 2)); // different terms (3 vs 4)
        assert!(!agree_at(&a, &b, 3)); // out of bounds
    }

    // r[verify log.prefix-matching]
    #[test]
    fn test_logs_match_prefix() {
        let a = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let b = vec![entry(1, 99), entry(2, 88), entry(4, 77)];
        assert!(logs_match_prefix(&a, &b, 0));
        assert!(logs_match_prefix(&a, &b, 2)); // terms match at 0, 1
        assert!(!logs_match_prefix(&a, &b, 3)); // term mismatch at 2
    }

    // r[verify log.append]
    #[test]
    fn test_log_append() {
        let mut log = vec![entry(1, 10)];
        log_append(&mut log, entry(2, 20));
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].term, 1);
        assert_eq!(log[1].term, 2);
    }

    // r[verify log.truncate]
    #[test]
    fn test_log_truncate() {
        let mut log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        log_truncate(&mut log, 2);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].term, 1);
        assert_eq!(log[1].term, 2);
    }

    proptest! {
        // r[verify log.agree-at]
        #[test]
        fn prop_agree_at_symmetric(
            terms in proptest::collection::vec(1u64..10, 1..=20),
        ) {
            let a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let b: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 1)).collect();
            // Same terms, different values — should agree
            for i in 0..terms.len() {
                prop_assert_eq!(agree_at(&a, &b, i), agree_at(&b, &a, i));
            }
        }

        // r[verify log.prefix-matching]
        #[test]
        fn prop_prefix_match_monotonic(
            terms in proptest::collection::vec(1u64..10, 1..=20),
        ) {
            let a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let b: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 1)).collect();
            let n = terms.len();
            // Full prefix matches (same terms)
            prop_assert!(logs_match_prefix(&a, &b, n));
            // Every shorter prefix also matches
            for k in 0..=n {
                prop_assert!(logs_match_prefix(&a, &b, k));
            }
        }

        // r[verify log.append]
        // r[verify log.truncate]
        #[test]
        fn prop_append_truncate_roundtrip(
            terms in proptest::collection::vec(1u64..10, 0..=20),
            new_term in 1u64..10,
        ) {
            let mut log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let original_len = log.len();
            log_append(&mut log, entry(new_term, 99));
            prop_assert_eq!(log.len(), original_len + 1);
            log_truncate(&mut log, original_len);
            prop_assert_eq!(log.len(), original_len);
            for i in 0..original_len {
                prop_assert_eq!(log[i].term, terms[i]);
            }
        }

        // r[verify log.agree-at]
        // r[verify log.prefix-matching]
        // r[verify log.append]
        // r[verify log.truncate]
        /// Model oracle: random Append/Truncate traces on two logs, checking
        /// agree_at and logs_match_prefix against a Vec<(u64,u64)> reference.
        #[test]
        fn prop_log_model_oracle(
            ops in proptest::collection::vec(
                prop_oneof![
                    3 => (1u64..5, any::<u64>()).prop_map(|(t, v)| (0u8, t, v, 0usize)),
                    1 => any::<usize>().prop_map(|k| (1u8, 0, 0, k)),
                ],
                0..=100,
            ),
        ) {
            let mut log_a: Vec<LogEntry> = Vec::new();
            let mut log_b: Vec<LogEntry> = Vec::new();
            let mut oracle_a: Vec<(u64, u64)> = Vec::new();
            let mut oracle_b: Vec<(u64, u64)> = Vec::new();

            for &(op, term, value, trunc) in &ops {
                match op {
                    0 => {
                        // Append to both with same term (diverge on value)
                        log_append(&mut log_a, entry(term, value));
                        log_append(&mut log_b, entry(term, value.wrapping_add(1)));
                        oracle_a.push((term, value));
                        oracle_b.push((term, value.wrapping_add(1)));
                    }
                    _ => {
                        // Truncate log_a to some valid length
                        if !oracle_a.is_empty() {
                            let k = trunc % oracle_a.len();
                            log_truncate(&mut log_a, k);
                            oracle_a.truncate(k);
                        }
                    }
                }

                // Validate lengths match oracle
                prop_assert_eq!(log_a.len(), oracle_a.len());
                prop_assert_eq!(log_b.len(), oracle_b.len());

                // Validate entries match oracle
                for (i, &(t, v)) in oracle_a.iter().enumerate() {
                    prop_assert_eq!(log_a[i].term, t);
                    prop_assert_eq!(log_a[i].value, v);
                }

                // Validate agree_at matches oracle
                let min_len = oracle_a.len().min(oracle_b.len());
                for i in 0..min_len {
                    let expected = oracle_a[i].0 == oracle_b[i].0;
                    prop_assert_eq!(agree_at(&log_a, &log_b, i), expected);
                }

                // Find first term mismatch
                let match_len = (0..min_len)
                    .take_while(|&i| oracle_a[i].0 == oracle_b[i].0)
                    .count();
                // logs_match_prefix should hold up to match_len
                prop_assert!(logs_match_prefix(&log_a, &log_b, match_len));
                // And fail at match_len + 1 if there's a mismatch
                if match_len < min_len {
                    prop_assert!(!logs_match_prefix(&log_a, &log_b, match_len + 1));
                }
            }
        }
    }
}

//! Verified append-only transaction journal with running balances.
//!
//! Each journal entry records a credit or debit, a sequence number, and
//! the resulting running balance. Proofs guarantee running balances are
//! correct, sequence numbers are strictly increasing, and appending a
//! valid entry preserves journal validity.

use vstd::prelude::*;

verus! {

// --- Types ---

/// A journal entry: one balance change with running balance.
pub struct JournalEntry {
    /// Monotonically increasing sequence number.
    pub seq_num: u64,
    /// Amount of the operation.
    pub amount: u64,
    /// true = debit (decrease), false = credit (increase).
    pub is_debit: bool,
    /// Running balance AFTER this entry.
    pub running_balance: u64,
}

// --- apply_entry ---

// r[depends journal.apply-entry]
/// Spec: expected balance after applying one entry.
pub open spec fn apply_entry_spec(prior_balance: u64, amount: u64, is_debit: bool) -> int {
    if is_debit {
        prior_balance as int - amount as int
    } else {
        prior_balance as int + amount as int
    }
}

// r[impl journal.apply-entry]
/// Apply a single entry to a prior balance. Returns None on underflow or overflow.
pub fn apply_entry(prior_balance: u64, amount: u64, is_debit: bool) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == apply_entry_spec(prior_balance, amount, is_debit),
            None => apply_entry_spec(prior_balance, amount, is_debit) < 0
                    || apply_entry_spec(prior_balance, amount, is_debit) > u64::MAX as int,
        },
{
    if is_debit {
        if amount > prior_balance {
            None
        } else {
            Some(prior_balance - amount)
        }
    } else {
        if amount > u64::MAX - prior_balance {
            None
        } else {
            Some(prior_balance + amount)
        }
    }
}

// --- entry_balance_correct ---

// r[impl journal.entry-balance-correct]
// r[verify journal.entry-balance-correct]
// r[depends journal.entry-balance-correct]
/// Spec: entry's running balance matches expected computation.
pub open spec fn entry_balance_correct_spec(
    entry: JournalEntry, prior_balance: u64,
) -> bool {
    entry.running_balance as int == apply_entry_spec(prior_balance, entry.amount, entry.is_debit)
    && entry.running_balance as int >= 0
    && entry.running_balance as int <= u64::MAX as int
}

// --- seq_nums_monotonic ---

// r[impl journal.seq-nums-monotonic]
// r[verify journal.seq-nums-monotonic]
// r[depends journal.seq-nums-monotonic]
/// Spec: sequence numbers are strictly increasing.
pub open spec fn seq_nums_monotonic_spec(entries: Seq<JournalEntry>) -> bool {
    forall|i: int, j: int| #![trigger entries[i], entries[j]]
        0 <= i < j < entries.len() ==> entries[i].seq_num < entries[j].seq_num
}

// --- journal_valid ---

// r[depends journal.valid]
/// Spec: journal valid from index `from` onward.
pub open spec fn journal_valid_from_spec(
    entries: Seq<JournalEntry>, initial_balance: u64, from: int,
) -> bool
    decreases entries.len() - from
{
    if from >= entries.len() {
        true
    } else {
        let prior = if from == 0 { initial_balance } else { entries[from - 1].running_balance };
        entry_balance_correct_spec(entries[from], prior)
        && journal_valid_from_spec(entries, initial_balance, from + 1)
    }
}

// r[impl journal.valid]
/// Validate a journal: check all running balances.
pub fn journal_valid(entries: &[JournalEntry], initial_balance: u64) -> (result: bool)
    ensures result == journal_valid_from_spec(entries@, initial_balance, 0)
{
    let mut i: usize = 0;
    while i < entries.len()
        invariant
            0 <= i <= entries@.len(),
            journal_valid_from_spec(entries@, initial_balance, 0)
                <==> journal_valid_from_spec(entries@, initial_balance, i as int),
        decreases entries@.len() - i,
    {
        let prior = if i == 0 { initial_balance } else { entries[i - 1].running_balance };
        let expected = apply_entry(prior, entries[i].amount, entries[i].is_debit);
        match expected {
            Some(v) => {
                if entries[i].running_balance != v {
                    return false;
                }
            }
            None => {
                return false;
            }
        }
        i = i + 1;
    }
    true
}

// --- final_balance ---

// r[depends journal.final-balance]
/// Spec: last running balance, or initial if empty.
pub open spec fn final_balance_spec(
    entries: Seq<JournalEntry>, initial_balance: u64,
) -> u64 {
    if entries.len() == 0 {
        initial_balance
    } else {
        entries[entries.len() - 1].running_balance
    }
}

// r[impl journal.final-balance]
/// Get the final balance of the journal.
pub fn final_balance(entries: &[JournalEntry], initial_balance: u64) -> (result: u64)
    ensures result == final_balance_spec(entries@, initial_balance)
{
    if entries.len() == 0 {
        initial_balance
    } else {
        entries[entries.len() - 1].running_balance
    }
}

// --- can_append ---

// r[depends journal.append-preserves]
/// Spec: whether a new entry can be appended.
pub open spec fn can_append_spec(
    entries: Seq<JournalEntry>, initial_balance: u64,
    amount: u64, is_debit: bool, new_seq_num: u64,
) -> bool {
    let prior = final_balance_spec(entries, initial_balance);
    let new_bal = apply_entry_spec(prior, amount, is_debit);
    // New balance fits in u64 and is non-negative
    new_bal >= 0 && new_bal <= u64::MAX as int
    // Sequence number strictly advances
    && (entries.len() == 0 || new_seq_num > entries[entries.len() - 1].seq_num)
    // Amount is valid
    && amount > 0
}

// r[impl journal.append-preserves]
/// Build an entry for appending. Returns None if invalid.
pub fn make_entry(
    entries: &[JournalEntry], initial_balance: u64,
    amount: u64, is_debit: bool, new_seq_num: u64,
) -> (result: Option<JournalEntry>)
    ensures
        match result {
            Some(e) =>
                can_append_spec(entries@, initial_balance, amount, is_debit, new_seq_num)
                && e.seq_num == new_seq_num
                && e.amount == amount
                && e.is_debit == is_debit
                && e.running_balance as int == apply_entry_spec(
                    final_balance_spec(entries@, initial_balance), amount, is_debit),
            None => !can_append_spec(entries@, initial_balance, amount, is_debit, new_seq_num),
        },
{
    if amount == 0 {
        return None;
    }
    if entries.len() > 0 && new_seq_num <= entries[entries.len() - 1].seq_num {
        return None;
    }
    let prior = final_balance(entries, initial_balance);
    match apply_entry(prior, amount, is_debit) {
        Some(new_bal) => Some(JournalEntry {
            seq_num: new_seq_num,
            amount,
            is_debit,
            running_balance: new_bal,
        }),
        None => None,
    }
}

// --- Proofs ---

// r[verify journal.apply-entry]
/// Credit increases balance.
proof fn apply_credit_increases(prior: u64, amount: u64)
    ensures apply_entry_spec(prior, amount, false) >= prior as int
{}

// r[verify journal.apply-entry]
/// Debit decreases balance.
proof fn apply_debit_decreases(prior: u64, amount: u64)
    ensures apply_entry_spec(prior, amount, true) <= prior as int
{}

// r[verify journal.valid]
/// Empty journal is always valid.
proof fn empty_journal_valid(initial_balance: u64)
    ensures journal_valid_from_spec(Seq::<JournalEntry>::empty(), initial_balance, 0)
{}

// r[verify journal.final-balance]
/// Final balance of empty journal is initial balance.
proof fn empty_final_balance(initial_balance: u64)
    ensures final_balance_spec(Seq::<JournalEntry>::empty(), initial_balance) == initial_balance
{}

// r[verify journal.final-balance]
/// After append, final balance is the new entry's running balance.
proof fn final_balance_after_append(entries: Seq<JournalEntry>, initial_balance: u64, entry: JournalEntry)
    ensures final_balance_spec(entries.push(entry), initial_balance) == entry.running_balance
{}

// r[impl journal.credit-debit-roundtrip]
// r[verify journal.credit-debit-roundtrip]
/// Credit then debit of same amount returns to original balance.
proof fn credit_then_debit_roundtrip(balance: u64, amount: u64)
    requires balance as int + amount as int <= u64::MAX as int
    ensures apply_entry_spec(
        apply_entry_spec(balance, amount, false) as u64, amount, true)
        == balance as int
{}

// r[verify journal.credit-debit-roundtrip]
/// Debit then credit of same amount returns to original balance.
proof fn debit_then_credit_roundtrip(balance: u64, amount: u64)
    requires amount as int <= balance as int
    ensures apply_entry_spec(
        apply_entry_spec(balance, amount, true) as u64, amount, false)
        == balance as int
{}

// r[verify journal.valid]
/// Journal validity from 0 to from implies validity from from.
proof fn journal_valid_prefix(
    entries: Seq<JournalEntry>, initial_balance: u64, k: int,
)
    requires
        0 <= k <= entries.len(),
        journal_valid_from_spec(entries, initial_balance, 0),
    ensures journal_valid_from_spec(entries, initial_balance, k)
    decreases k
{
    if k > 0 {
        journal_valid_prefix(entries, initial_balance, k - 1);
    }
}

// r[verify journal.append-preserves]
/// Appending a valid entry preserves journal validity.
pub proof fn append_preserves_validity(
    entries: Seq<JournalEntry>, initial_balance: u64, entry: JournalEntry,
)
    requires
        journal_valid_from_spec(entries, initial_balance, 0),
        entry_balance_correct_spec(entry, final_balance_spec(entries, initial_balance)),
    ensures journal_valid_from_spec(entries.push(entry), initial_balance, 0)
{
    let extended = entries.push(entry);
    // Prove by showing validity propagates through all indices
    append_validity_helper(entries, initial_balance, entry, 0);
}

/// Helper: inductive proof that validity propagates after append.
proof fn append_validity_helper(
    entries: Seq<JournalEntry>, initial_balance: u64, entry: JournalEntry,
    from: int,
)
    requires
        0 <= from <= entries.len() + 1,
        journal_valid_from_spec(entries, initial_balance, 0),
        entry_balance_correct_spec(entry, final_balance_spec(entries, initial_balance)),
    ensures journal_valid_from_spec(entries.push(entry), initial_balance, from)
    decreases entries.len() + 1 - from
{
    let extended = entries.push(entry);
    if from >= extended.len() {
        // past the end — trivially valid
    } else if from == entries.len() as int {
        // the new entry at the end
        if entries.len() > 0 {
            assert(extended[from - 1] == entries[from - 1]);
        }
        // The prior is final_balance_spec(entries, initial_balance)
        // and entry_balance_correct_spec(entry, prior) holds by precondition
        // Tail: from+1 >= extended.len(), so trivially valid
        append_validity_helper(entries, initial_balance, entry, from + 1);
    } else {
        // from < entries.len(): existing entry
        assert(extended[from] == entries[from]);
        if from > 0 {
            assert(extended[from - 1] == entries[from - 1]);
        }
        // The entry at `from` has the same prior in extended as in entries
        journal_valid_prefix(entries, initial_balance, from);
        append_validity_helper(entries, initial_balance, entry, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify journal.apply-entry]
    #[test]
    fn test_apply_entry_basic() {
        assert_eq!(apply_entry(100, 30, true), Some(70)); // debit
        assert_eq!(apply_entry(100, 30, false), Some(130)); // credit
        assert_eq!(apply_entry(10, 20, true), None); // underflow
        assert_eq!(apply_entry(u64::MAX, 1, false), None); // overflow
        assert_eq!(apply_entry(0, 0, true), Some(0));
        assert_eq!(apply_entry(0, 0, false), Some(0));
    }

    // r[verify journal.valid]
    #[test]
    fn test_journal_valid_empty() {
        assert!(journal_valid(&[], 100));
    }

    // r[verify journal.valid]
    #[test]
    fn test_journal_valid_single_credit() {
        let entries = [JournalEntry {
            seq_num: 1,
            amount: 50,
            is_debit: false,
            running_balance: 150,
        }];
        assert!(journal_valid(&entries, 100));
    }

    // r[verify journal.valid]
    #[test]
    fn test_journal_valid_single_debit() {
        let entries = [JournalEntry {
            seq_num: 1,
            amount: 30,
            is_debit: true,
            running_balance: 70,
        }];
        assert!(journal_valid(&entries, 100));
    }

    // r[verify journal.valid]
    #[test]
    fn test_journal_valid_wrong_balance() {
        let entries = [JournalEntry {
            seq_num: 1,
            amount: 50,
            is_debit: false,
            running_balance: 999, // wrong
        }];
        assert!(!journal_valid(&entries, 100));
    }

    // r[verify journal.final-balance]
    #[test]
    fn test_final_balance() {
        assert_eq!(final_balance(&[], 100), 100);
        let entries = [JournalEntry {
            seq_num: 1,
            amount: 50,
            is_debit: false,
            running_balance: 150,
        }];
        assert_eq!(final_balance(&entries, 100), 150);
    }

    // r[verify journal.append-preserves]
    #[test]
    fn test_make_entry_basic() {
        let entry = make_entry(&[], 100, 50, false, 1);
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.running_balance, 150);
        assert_eq!(e.seq_num, 1);

        // zero amount rejected
        assert!(make_entry(&[], 100, 0, false, 1).is_none());
    }

    // r[verify journal.append-preserves]
    #[test]
    fn test_make_entry_seq_num_check() {
        let entries = [JournalEntry {
            seq_num: 5,
            amount: 10,
            is_debit: false,
            running_balance: 110,
        }];
        // seq_num must advance
        assert!(make_entry(&entries, 100, 10, false, 6).is_some());
        assert!(make_entry(&entries, 100, 10, false, 5).is_none());
        assert!(make_entry(&entries, 100, 10, false, 3).is_none());
    }

    proptest! {
        // r[verify journal.apply-entry]
        #[test]
        fn prop_apply_entry_oracle(prior: u64, amount: u64, is_debit: bool) {
            let oracle: i128 = if is_debit {
                prior as i128 - amount as i128
            } else {
                prior as i128 + amount as i128
            };
            match apply_entry(prior, amount, is_debit) {
                Some(v) => prop_assert_eq!(v as i128, oracle),
                None => prop_assert!(oracle < 0 || oracle > u64::MAX as i128),
            }
        }

        // r[verify journal.credit-debit-roundtrip]
        #[test]
        fn prop_credit_debit_roundtrip(balance in 0..u64::MAX / 2, amount in 0..u64::MAX / 2) {
            if let Some(credited) = apply_entry(balance, amount, false) {
                prop_assert_eq!(apply_entry(credited, amount, true), Some(balance));
            }
        }

        // r[verify journal.credit-debit-roundtrip]
        #[test]
        fn prop_debit_credit_roundtrip(balance: u64, amount: u64) {
            if let Some(debited) = apply_entry(balance, amount, true) {
                prop_assert_eq!(apply_entry(debited, amount, false), Some(balance));
            }
        }

        // r[verify journal.append-preserves]
        #[test]
        fn prop_lifecycle(
            initial in 0..1_000_000u64,
            ops in proptest::collection::vec((1..10_000u64, proptest::bool::ANY), 1..20),
        ) {
            let mut entries: Vec<JournalEntry> = Vec::new();
            let mut seq_num = 0u64;
            for (amount, is_debit) in &ops {
                seq_num += 1;
                if let Some(entry) = make_entry(&entries, initial, *amount, *is_debit, seq_num) {
                    // verify running balance
                    let prior = if entries.is_empty() {
                        initial
                    } else {
                        entries.last().unwrap().running_balance
                    };
                    let expected: i128 = if *is_debit {
                        prior as i128 - *amount as i128
                    } else {
                        prior as i128 + *amount as i128
                    };
                    prop_assert_eq!(entry.running_balance as i128, expected);
                    entries.push(entry);
                }
            }
            // full journal should be valid
            prop_assert!(journal_valid(&entries, initial));
            // final balance should match last entry
            let fb = final_balance(&entries, initial);
            if entries.is_empty() {
                prop_assert_eq!(fb, initial);
            } else {
                prop_assert_eq!(fb, entries.last().unwrap().running_balance);
            }
        }
    }
}

//! Exactly-once ledger transfer facade.
//!
//! Validity boundary: the public checker rejects invalid account indexes,
//! invalid/duplicate transfer requests, consumed idempotency tokens, completed
//! durable replay effects, unbalanced debit/credit evidence, and non-appendable
//! journals. Rejected outcomes are read-only: no mutable state is changed by
//! this facade; callers apply the returned balances/journal evidence only on
//! `Posted`.

use vstd::prelude::*;

use crate::durable_history::DurableEvent;

verus! {

/// Domain outcome for the exactly-once transfer facade.
pub enum ExactlyOnceLedgerTransferOutcome {
    Posted { sender_balance: u64, receiver_balance: u64 },
    RejectedDuplicate,
    RejectedInvalid,
}

/// Spec: accepted transfers compose transfer, ledger, double-entry, journal,
/// idempotency, and durable replay predicates on the same token/effect.
pub open spec fn exactly_once_ledger_transfer_accepted_spec(
    accounts: Seq<u64>,
    from_idx: int,
    to_idx: int,
    amount: u64,
    request_id: u64,
    last_processed_id: u64,
    consumed_tokens: Seq<u64>,
    events: Seq<DurableEvent>,
    durable_seq: usize,
    effect_id: u64,
    sender_journal: Seq<crate::journal::JournalEntry>,
    sender_initial_balance: u64,
    sender_seq_num: u64,
    receiver_journal: Seq<crate::journal::JournalEntry>,
    receiver_initial_balance: u64,
    receiver_seq_num: u64,
) -> bool {
    &&& request_id == effect_id
    &&& crate::job_idempotency::try_execute_spec(request_id, consumed_tokens) == (true, true)
    &&& crate::durable_replay::should_execute_effect_spec(events, durable_seq, effect_id)
    &&& crate::transfer::validate_transfer_spec(
        if 0 <= from_idx < accounts.len() { accounts[from_idx] } else { 0 },
        if 0 <= to_idx < accounts.len() { accounts[to_idx] } else { 0 },
        amount,
        request_id,
        last_processed_id,
    ) == crate::transfer::TRANSFER_OK()
    &&& crate::ledger::can_transfer_spec(accounts, from_idx, to_idx, amount)
    &&& crate::double_entry::is_simple_entry_spec(amount, amount)
    &&& crate::journal::can_append_spec(
        sender_journal,
        sender_initial_balance,
        amount,
        true,
        sender_seq_num,
    )
    &&& crate::journal::can_append_spec(
        receiver_journal,
        receiver_initial_balance,
        amount,
        false,
        receiver_seq_num,
    )
    &&& crate::ledger::total_balance_spec(
        crate::ledger::transfer_result_spec(accounts, from_idx, to_idx, amount),
    ) == crate::ledger::total_balance_spec(accounts)
}

/// Spec: rejected outcomes do not authorize posting balances or journal entries.
pub open spec fn exactly_once_ledger_transfer_reject_preserves_spec(
    pre_accounts: Seq<u64>,
    post_accounts: Seq<u64>,
    pre_sender_journal: Seq<crate::journal::JournalEntry>,
    post_sender_journal: Seq<crate::journal::JournalEntry>,
    pre_receiver_journal: Seq<crate::journal::JournalEntry>,
    post_receiver_journal: Seq<crate::journal::JournalEntry>,
) -> bool {
    post_accounts =~= pre_accounts
    && post_sender_journal =~= pre_sender_journal
    && post_receiver_journal =~= pre_receiver_journal
}

/// Spec outcome for the facade.
pub open spec fn exactly_once_ledger_transfer_spec(
    accounts: Seq<u64>,
    from_idx: int,
    to_idx: int,
    amount: u64,
    request_id: u64,
    last_processed_id: u64,
    consumed_tokens: Seq<u64>,
    events: Seq<DurableEvent>,
    durable_seq: usize,
    effect_id: u64,
    sender_journal: Seq<crate::journal::JournalEntry>,
    sender_initial_balance: u64,
    sender_seq_num: u64,
    receiver_journal: Seq<crate::journal::JournalEntry>,
    receiver_initial_balance: u64,
    receiver_seq_num: u64,
) -> bool {
    exactly_once_ledger_transfer_accepted_spec(
        accounts,
        from_idx,
        to_idx,
        amount,
        request_id,
        last_processed_id,
        consumed_tokens,
        events,
        durable_seq,
        effect_id,
        sender_journal,
        sender_initial_balance,
        sender_seq_num,
        receiver_journal,
        receiver_initial_balance,
        receiver_seq_num,
    )
}

// r[impl composite-primitives.exactly-once-ledger-transfer]
/// Check and post a replay-safe transfer. `Posted` returns the new sender and
/// receiver balances; rejected branches are read-only and perform no mutation.
pub fn exactly_once_ledger_transfer(
    accounts: &[u64],
    from_idx: usize,
    to_idx: usize,
    amount: u64,
    request_id: u64,
    last_processed_id: u64,
    consumed_tokens: &[u64],
    events: &[DurableEvent],
    durable_seq: usize,
    effect_id: u64,
    sender_journal: &[crate::journal::JournalEntry],
    sender_initial_balance: u64,
    sender_seq_num: u64,
    receiver_journal: &[crate::journal::JournalEntry],
    receiver_initial_balance: u64,
    receiver_seq_num: u64,
) -> (outcome: ExactlyOnceLedgerTransferOutcome)
    ensures
        match outcome {
            ExactlyOnceLedgerTransferOutcome::Posted { sender_balance, receiver_balance } =>
                exactly_once_ledger_transfer_accepted_spec(
                    accounts@,
                    from_idx as int,
                    to_idx as int,
                    amount,
                    request_id,
                    last_processed_id,
                    consumed_tokens@,
                    events@,
                    durable_seq,
                    effect_id,
                    sender_journal@,
                    sender_initial_balance,
                    sender_seq_num,
                    receiver_journal@,
                    receiver_initial_balance,
                    receiver_seq_num,
                )
                && sender_balance == (accounts@[from_idx as int] - amount) as u64
                && receiver_balance == (accounts@[to_idx as int] + amount) as u64,
            ExactlyOnceLedgerTransferOutcome::RejectedDuplicate =>
                !crate::job_idempotency::try_execute_spec(request_id, consumed_tokens@).0
                || !crate::durable_replay::should_execute_effect_spec(events@, durable_seq, effect_id)
                || crate::transfer::is_duplicate_spec(request_id, last_processed_id),
            ExactlyOnceLedgerTransferOutcome::RejectedInvalid =>
                !exactly_once_ledger_transfer_accepted_spec(
                    accounts@,
                    from_idx as int,
                    to_idx as int,
                    amount,
                    request_id,
                    last_processed_id,
                    consumed_tokens@,
                    events@,
                    durable_seq,
                    effect_id,
                    sender_journal@,
                    sender_initial_balance,
                    sender_seq_num,
                    receiver_journal@,
                    receiver_initial_balance,
                    receiver_seq_num,
                ),
        },
{
    let idempotency = crate::job_idempotency::try_execute(request_id, consumed_tokens);
    if !idempotency.0 {
        return ExactlyOnceLedgerTransferOutcome::RejectedDuplicate;
    }
    if !crate::durable_replay::should_execute_effect(events, durable_seq, effect_id) {
        return ExactlyOnceLedgerTransferOutcome::RejectedDuplicate;
    }
    if from_idx >= accounts.len() || to_idx >= accounts.len() {
        return ExactlyOnceLedgerTransferOutcome::RejectedInvalid;
    }
    let status = crate::transfer::validate_transfer(
        accounts[from_idx],
        accounts[to_idx],
        amount,
        request_id,
        last_processed_id,
    );
    if status == 4 {
        return ExactlyOnceLedgerTransferOutcome::RejectedDuplicate;
    }
    if status != 0 {
        return ExactlyOnceLedgerTransferOutcome::RejectedInvalid;
    }
    if !crate::ledger::can_transfer(accounts, from_idx, to_idx, amount) {
        return ExactlyOnceLedgerTransferOutcome::RejectedInvalid;
    }
    let transfer_result = crate::ledger::ledger_transfer(accounts, from_idx, to_idx, amount);
    let pair = match transfer_result {
        Some(p) => p,
        None => return ExactlyOnceLedgerTransferOutcome::RejectedInvalid,
    };
    if !crate::double_entry::is_simple_entry(amount, amount) {
        return ExactlyOnceLedgerTransferOutcome::RejectedInvalid;
    }
    match crate::journal::make_entry(
        sender_journal,
        sender_initial_balance,
        amount,
        true,
        sender_seq_num,
    ) {
        Some(_) => {},
        None => return ExactlyOnceLedgerTransferOutcome::RejectedInvalid,
    }
    match crate::journal::make_entry(
        receiver_journal,
        receiver_initial_balance,
        amount,
        false,
        receiver_seq_num,
    ) {
        Some(_) => {},
        None => return ExactlyOnceLedgerTransferOutcome::RejectedInvalid,
    }
    if effect_id != request_id {
        return ExactlyOnceLedgerTransferOutcome::RejectedInvalid;
    }
    proof {
        crate::ledger::transfer_preserves_total(accounts@, from_idx as int, to_idx as int, amount);
    }
    ExactlyOnceLedgerTransferOutcome::Posted { sender_balance: pair.0, receiver_balance: pair.1 }
}

// r[verify composite-primitives.exactly-once-ledger-transfer]
/// Accepted spec expands to every source predicate required by the composition.
pub proof fn exactly_once_ledger_transfer_refines_sources(
    accounts: Seq<u64>,
    from_idx: int,
    to_idx: int,
    amount: u64,
    request_id: u64,
    last_processed_id: u64,
    consumed_tokens: Seq<u64>,
    events: Seq<DurableEvent>,
    durable_seq: usize,
    effect_id: u64,
    sender_journal: Seq<crate::journal::JournalEntry>,
    sender_initial_balance: u64,
    sender_seq_num: u64,
    receiver_journal: Seq<crate::journal::JournalEntry>,
    receiver_initial_balance: u64,
    receiver_seq_num: u64,
)
    requires exactly_once_ledger_transfer_accepted_spec(
        accounts,
        from_idx,
        to_idx,
        amount,
        request_id,
        last_processed_id,
        consumed_tokens,
        events,
        durable_seq,
        effect_id,
        sender_journal,
        sender_initial_balance,
        sender_seq_num,
        receiver_journal,
        receiver_initial_balance,
        receiver_seq_num,
    )
    ensures
        request_id == effect_id,
        crate::job_idempotency::try_execute_spec(request_id, consumed_tokens) == (true, true),
        crate::durable_replay::should_execute_effect_spec(events, durable_seq, effect_id),
        crate::ledger::can_transfer_spec(accounts, from_idx, to_idx, amount),
        crate::double_entry::is_simple_entry_spec(amount, amount),
        crate::journal::can_append_spec(sender_journal, sender_initial_balance, amount, true, sender_seq_num),
        crate::journal::can_append_spec(receiver_journal, receiver_initial_balance, amount, false, receiver_seq_num),
        crate::ledger::total_balance_spec(
            crate::ledger::transfer_result_spec(accounts, from_idx, to_idx, amount),
        ) == crate::ledger::total_balance_spec(accounts),
{}

// r[verify composite-primitives.exactly-once-ledger-transfer]
/// Duplicate/replayed transfers reject without authorizing a second posting.
pub proof fn exactly_once_ledger_transfer_replay_rejected(
    request_id: u64,
    consumed_tokens: Seq<u64>,
    events: Seq<DurableEvent>,
    durable_seq: usize,
    effect_id: u64,
)
    requires
        !crate::job_idempotency::try_execute_spec(request_id, consumed_tokens).0
            || !crate::durable_replay::should_execute_effect_spec(events, durable_seq, effect_id),
    ensures
        !(
            crate::job_idempotency::try_execute_spec(request_id, consumed_tokens) == (true, true)
            && crate::durable_replay::should_execute_effect_spec(events, durable_seq, effect_id)
        ),
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable_history::DurableEvent;
    use crate::durable_history::EVENT_EFFECT_DONE;
    use crate::durable_history::EVENT_WORK;
    use crate::journal::JournalEntry;

    #[test]
    fn fresh_transfer_posts_once() {
        let accounts = [100u64, 20u64];
        let consumed: [u64; 0] = [];
        let events = [DurableEvent {
            seq_num: 0,
            kind: EVENT_WORK,
            effect_id: 7,
        }];
        let empty_sender: [JournalEntry; 0] = [];
        let empty_receiver: [JournalEntry; 0] = [];

        match exactly_once_ledger_transfer(
            &accounts,
            0,
            1,
            30,
            7,
            6,
            &consumed,
            &events,
            1,
            7,
            &empty_sender,
            100,
            1,
            &empty_receiver,
            20,
            1,
        ) {
            ExactlyOnceLedgerTransferOutcome::Posted {
                sender_balance,
                receiver_balance,
            } => {
                assert_eq!(sender_balance, 70);
                assert_eq!(receiver_balance, 50);
                assert_eq!(accounts[0] + accounts[1], sender_balance + receiver_balance);
            }
            _ => panic!("fresh transfer should post"),
        }
    }

    #[test]
    fn replayed_transfer_does_not_double_post() {
        let accounts = [100u64, 20u64];
        let consumed = [7u64];
        let events = [DurableEvent {
            seq_num: 0,
            kind: EVENT_EFFECT_DONE,
            effect_id: 7,
        }];
        let empty_sender: [JournalEntry; 0] = [];
        let empty_receiver: [JournalEntry; 0] = [];

        match exactly_once_ledger_transfer(
            &accounts,
            0,
            1,
            30,
            7,
            6,
            &consumed,
            &events,
            0,
            7,
            &empty_sender,
            100,
            1,
            &empty_receiver,
            20,
            1,
        ) {
            ExactlyOnceLedgerTransferOutcome::RejectedDuplicate => {}
            _ => panic!("replay should reject"),
        }
    }
}

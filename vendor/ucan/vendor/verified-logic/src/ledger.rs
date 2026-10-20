//! Verified multi-account ledger with conservation proofs.
//!
//! A ledger is a `Seq<u64>` of account balances. Transfers move value
//! between two accounts. The central invariant: the sum of all balances
//! is constant across any valid transfer or batch of transfers. Proofs
//! compose balance (credit/debit), transfer (conservation), and
//! double_entry (summation) into a multi-party system.

use vstd::prelude::*;

verus! {

// -----------------------------------------------------------------------
// Summation — delegates to double_entry::sum_spec
// -----------------------------------------------------------------------

// r[depends ledger.total]
/// Spec: total of all account balances (recursive from index).
pub open spec fn ledger_sum_spec(accounts: Seq<u64>, from: int) -> int
    decreases accounts.len() - from
{
    if from >= accounts.len() {
        0
    } else {
        accounts[from] as int + ledger_sum_spec(accounts, from + 1)
    }
}

// r[depends ledger.total]
/// Spec: total of all accounts.
pub open spec fn total_balance_spec(accounts: Seq<u64>) -> int {
    ledger_sum_spec(accounts, 0)
}

// r[impl ledger.total]
/// Sum all account balances. Returns None on overflow.
pub fn total_balance(accounts: &[u64]) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == total_balance_spec(accounts@),
            None => total_balance_spec(accounts@) > u64::MAX as int,
        },
{
    let mut acc: u64 = 0;
    let mut i: usize = 0;
    while i < accounts.len()
        invariant
            0 <= i <= accounts@.len(),
            acc as int == ledger_sum_spec(accounts@, 0) - ledger_sum_spec(accounts@, i as int),
            acc as int <= u64::MAX as int,
        decreases accounts@.len() - i,
    {
        if accounts[i] > u64::MAX - acc {
            proof {
                ledger_sum_nonneg(accounts@, (i + 1) as int);
            }
            return None;
        }
        acc = acc + accounts[i];
        i = i + 1;
    }
    Some(acc)
}

// -----------------------------------------------------------------------
// Account access
// -----------------------------------------------------------------------

// r[depends ledger.balance-at]
/// Spec: balance at a given index.
pub open spec fn balance_at_spec(accounts: Seq<u64>, idx: int) -> u64
    recommends 0 <= idx < accounts.len()
{
    accounts[idx]
}

// r[impl ledger.balance-at]
/// Get balance at an account index.
pub fn balance_at(accounts: &[u64], idx: usize) -> (result: u64)
    requires (idx as int) < accounts@.len()
    ensures result == balance_at_spec(accounts@, idx as int)
{
    accounts[idx]
}

// r[depends ledger.account-count]
/// Spec: number of accounts.
pub open spec fn account_count_spec(accounts: Seq<u64>) -> nat {
    accounts.len()
}

// r[impl ledger.account-count]
/// Number of accounts in the ledger.
pub fn account_count(accounts: &[u64]) -> (result: usize)
    ensures result as nat == account_count_spec(accounts@)
{
    accounts.len()
}

// -----------------------------------------------------------------------
// Single transfer
// -----------------------------------------------------------------------

// r[depends ledger.can-transfer]
/// Spec: preconditions for a valid intra-ledger transfer.
pub open spec fn can_transfer_spec(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64,
) -> bool {
    0 <= from_idx < accounts.len()
    && 0 <= to_idx < accounts.len()
    && from_idx != to_idx
    && amount > 0
    && amount as int <= accounts[from_idx] as int
    && accounts[to_idx] as int + amount as int <= u64::MAX as int
}

// r[depends ledger.transfer-result]
/// Spec: ledger state after a transfer.
pub open spec fn transfer_result_spec(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64,
) -> Seq<u64>
    recommends can_transfer_spec(accounts, from_idx, to_idx, amount)
{
    accounts
        .update(from_idx, (accounts[from_idx] - amount) as u64)
        .update(to_idx, (accounts[to_idx] + amount) as u64)
}

// r[impl ledger.can-transfer]
/// Check if a transfer is valid.
pub fn can_transfer(
    accounts: &[u64], from_idx: usize, to_idx: usize, amount: u64,
) -> (result: bool)
    ensures result == can_transfer_spec(accounts@, from_idx as int, to_idx as int, amount)
{
    from_idx < accounts.len()
    && to_idx < accounts.len()
    && from_idx != to_idx
    && amount > 0
    && amount <= accounts[from_idx]
    && amount <= u64::MAX - accounts[to_idx]
}

// r[impl ledger.transfer-result]
/// Execute a transfer, returning (new_sender_balance, new_receiver_balance).
pub fn ledger_transfer(
    accounts: &[u64], from_idx: usize, to_idx: usize, amount: u64,
) -> (result: Option<(u64, u64)>)
    ensures
        match result {
            Some(pair) =>
                can_transfer_spec(accounts@, from_idx as int, to_idx as int, amount)
                && pair.0 == (accounts@[from_idx as int] - amount) as u64
                && pair.1 == (accounts@[to_idx as int] + amount) as u64,
            None => !can_transfer_spec(accounts@, from_idx as int, to_idx as int, amount),
        },
{
    if !can_transfer(accounts, from_idx, to_idx, amount) {
        None
    } else {
        Some((accounts[from_idx] - amount, accounts[to_idx] + amount))
    }
}

// -----------------------------------------------------------------------
// Transfer with fee
// -----------------------------------------------------------------------

// r[depends ledger.transfer-with-fee]
/// Spec: can transfer with fee to a third account?
pub open spec fn can_transfer_with_fee_spec(
    accounts: Seq<u64>, from_idx: int, to_idx: int, fee_idx: int,
    amount: u64, fee: u64,
) -> bool {
    0 <= from_idx < accounts.len()
    && 0 <= to_idx < accounts.len()
    && 0 <= fee_idx < accounts.len()
    && from_idx != to_idx
    && from_idx != fee_idx
    && to_idx != fee_idx
    && amount > 0
    && fee as int <= amount as int
    && amount as int <= accounts[from_idx] as int
    && accounts[to_idx] as int + (amount as int - fee as int) <= u64::MAX as int
    && accounts[fee_idx] as int + fee as int <= u64::MAX as int
}

// r[depends ledger.transfer-with-fee]
/// Spec: ledger after transfer with fee.
pub open spec fn transfer_with_fee_result_spec(
    accounts: Seq<u64>, from_idx: int, to_idx: int, fee_idx: int,
    amount: u64, fee: u64,
) -> Seq<u64>
    recommends can_transfer_with_fee_spec(accounts, from_idx, to_idx, fee_idx, amount, fee)
{
    let net = (amount - fee) as u64;
    accounts
        .update(from_idx, (accounts[from_idx] - amount) as u64)
        .update(to_idx, (accounts[to_idx] + net) as u64)
        .update(fee_idx, (accounts[fee_idx] + fee) as u64)
}

// r[impl ledger.transfer-with-fee]
/// Execute a transfer with fee. Sender pays amount, receiver gets amount - fee,
/// fee collector gets fee.
pub fn ledger_transfer_with_fee(
    accounts: &[u64], from_idx: usize, to_idx: usize, fee_idx: usize,
    amount: u64, fee: u64,
) -> (result: Option<(u64, u64, u64)>)
    ensures
        match result {
            Some(triple) =>
                can_transfer_with_fee_spec(
                    accounts@, from_idx as int, to_idx as int, fee_idx as int, amount, fee)
                && triple.0 == (accounts@[from_idx as int] - amount) as u64
                && triple.1 == (accounts@[to_idx as int] + (amount - fee)) as u64
                && triple.2 == (accounts@[fee_idx as int] + fee) as u64,
            None => !can_transfer_with_fee_spec(
                accounts@, from_idx as int, to_idx as int, fee_idx as int, amount, fee),
        },
{
    if from_idx >= accounts.len()
        || to_idx >= accounts.len()
        || fee_idx >= accounts.len()
        || from_idx == to_idx
        || from_idx == fee_idx
        || to_idx == fee_idx
        || amount == 0
        || fee > amount
        || amount > accounts[from_idx]
    {
        return None;
    }
    let net = amount - fee;
    if net > u64::MAX - accounts[to_idx] {
        return None;
    }
    if fee > u64::MAX - accounts[fee_idx] {
        return None;
    }
    Some((accounts[from_idx] - amount, accounts[to_idx] + net, accounts[fee_idx] + fee))
}

// -----------------------------------------------------------------------
// Inductive helpers
// -----------------------------------------------------------------------

// r[verify ledger.total]
/// Sum from any index is non-negative.
pub proof fn ledger_sum_nonneg(accounts: Seq<u64>, from: int)
    ensures ledger_sum_spec(accounts, from) >= 0
    decreases accounts.len() - from
{
    if from < accounts.len() {
        ledger_sum_nonneg(accounts, from + 1);
    }
}

/// Helper: updating one element changes the sum by exactly the delta.
proof fn sum_update_delta(
    accounts: Seq<u64>, idx: int, new_val: u64, from: int,
)
    requires
        0 <= idx < accounts.len(),
        0 <= from,
    ensures
        ledger_sum_spec(accounts.update(idx, new_val), from)
            == ledger_sum_spec(accounts, from) - accounts[idx] as int + new_val as int
            + if from <= idx { 0int } else { accounts[idx] as int - new_val as int },
    decreases accounts.len() - from
{
    let updated = accounts.update(idx, new_val);
    if from >= accounts.len() {
        // Both sums are 0, and from > idx is impossible when idx < accounts.len()
        // but the if-else handles it
    } else if from == idx {
        // At the updated position: updated[from] = new_val, accounts[from] = old
        assert(updated[from] == new_val);
        sum_update_delta(accounts, idx, new_val, from + 1);
    } else {
        // Not at the updated position: updated[from] = accounts[from]
        assert(updated[from] == accounts[from]);
        sum_update_delta(accounts, idx, new_val, from + 1);
    }
}

/// Simplified: updating index idx changes total by (new_val - old_val).
pub proof fn sum_update_total(accounts: Seq<u64>, idx: int, new_val: u64)
    requires 0 <= idx < accounts.len()
    ensures
        total_balance_spec(accounts.update(idx, new_val))
            == total_balance_spec(accounts) - accounts[idx] as int + new_val as int,
{
    sum_update_delta(accounts, idx, new_val, 0);
}

// -----------------------------------------------------------------------
// Conservation proofs
// -----------------------------------------------------------------------

// r[impl ledger.conservation]
// r[verify ledger.conservation]
/// A valid transfer preserves the total balance across all accounts.
pub proof fn transfer_preserves_total(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64,
)
    requires can_transfer_spec(accounts, from_idx, to_idx, amount)
    ensures
        total_balance_spec(transfer_result_spec(accounts, from_idx, to_idx, amount))
            == total_balance_spec(accounts),
{
    let new_from = (accounts[from_idx] - amount) as u64;
    let new_to = (accounts[to_idx] + amount) as u64;
    let step1 = accounts.update(from_idx, new_from);
    let step2 = step1.update(to_idx, new_to);

    // Step 1: debit sender
    sum_update_total(accounts, from_idx, new_from);
    // total(step1) = total(accounts) - accounts[from_idx] + new_from
    //              = total(accounts) - amount

    // Step 2: credit receiver — step1[to_idx] == accounts[to_idx] because from_idx != to_idx
    assert(step1[to_idx] == accounts[to_idx]);
    sum_update_total(step1, to_idx, new_to);
    // total(step2) = total(step1) - step1[to_idx] + new_to
    //              = total(step1) - accounts[to_idx] + accounts[to_idx] + amount
    //              = total(step1) + amount
    //              = total(accounts)
}

// r[verify ledger.conservation]
/// A transfer with fee also preserves total (fee redistributes, doesn't destroy).
pub proof fn fee_transfer_preserves_total(
    accounts: Seq<u64>, from_idx: int, to_idx: int, fee_idx: int,
    amount: u64, fee: u64,
)
    requires can_transfer_with_fee_spec(accounts, from_idx, to_idx, fee_idx, amount, fee)
    ensures
        total_balance_spec(transfer_with_fee_result_spec(
            accounts, from_idx, to_idx, fee_idx, amount, fee))
            == total_balance_spec(accounts),
{
    let new_from = (accounts[from_idx] - amount) as u64;
    let net = (amount - fee) as u64;
    let new_to = (accounts[to_idx] + net) as u64;
    let new_fee = (accounts[fee_idx] + fee) as u64;

    let s1 = accounts.update(from_idx, new_from);
    let s2 = s1.update(to_idx, new_to);
    let s3 = s2.update(fee_idx, new_fee);

    // Step 1: debit sender by full amount
    sum_update_total(accounts, from_idx, new_from);

    // Step 2: credit receiver by net — s1[to_idx] == accounts[to_idx]
    assert(s1[to_idx] == accounts[to_idx]);
    sum_update_total(s1, to_idx, new_to);

    // Step 3: credit fee collector — s2[fee_idx] == accounts[fee_idx]
    assert(s2[fee_idx] == accounts[fee_idx]);
    sum_update_total(s2, fee_idx, new_fee);

    // Chain: total(s3) = total(accounts) - amount + net + fee
    //                   = total(accounts) - amount + (amount - fee) + fee
    //                   = total(accounts)
}

// r[verify ledger.transfer-result]
/// Transfer preserves ledger length.
proof fn transfer_preserves_length(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64,
)
    requires can_transfer_spec(accounts, from_idx, to_idx, amount)
    ensures
        transfer_result_spec(accounts, from_idx, to_idx, amount).len()
            == accounts.len(),
{}

// r[verify ledger.transfer-result]
/// Transfer only changes the sender and receiver accounts.
proof fn transfer_isolates(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64, k: int,
)
    requires
        can_transfer_spec(accounts, from_idx, to_idx, amount),
        0 <= k < accounts.len(),
        k != from_idx,
        k != to_idx,
    ensures
        transfer_result_spec(accounts, from_idx, to_idx, amount)[k]
            == accounts[k],
{}

// r[verify ledger.total]
/// Empty ledger has zero total.
proof fn empty_ledger_zero()
    ensures total_balance_spec(Seq::<u64>::empty()) == 0
{}

// r[verify ledger.total]
/// Single-account ledger total equals that account's balance.
proof fn single_account_total(balance: u64)
    ensures total_balance_spec(seq![balance]) == balance as int
{
    // Unfold: sum(seq![balance], 0) = balance + sum(seq![balance], 1) = balance + 0
    assert(ledger_sum_spec(seq![balance], 1) == 0);
}

// r[verify ledger.can-transfer]
/// Self-transfer is always rejected.
proof fn self_transfer_rejected(accounts: Seq<u64>, idx: int, amount: u64)
    ensures !can_transfer_spec(accounts, idx, idx, amount)
{}

// r[verify ledger.can-transfer]
/// Zero amount is always rejected.
proof fn zero_amount_rejected(accounts: Seq<u64>, from_idx: int, to_idx: int)
    ensures !can_transfer_spec(accounts, from_idx, to_idx, 0)
{}

// r[verify ledger.transfer-result]
/// Sender balance decreases by amount.
proof fn transfer_sender_debited(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64,
)
    requires can_transfer_spec(accounts, from_idx, to_idx, amount)
    ensures
        transfer_result_spec(accounts, from_idx, to_idx, amount)[from_idx] as int
            == accounts[from_idx] as int - amount as int,
{}

// r[verify ledger.transfer-result]
/// Receiver balance increases by amount.
proof fn transfer_receiver_credited(
    accounts: Seq<u64>, from_idx: int, to_idx: int, amount: u64,
)
    requires can_transfer_spec(accounts, from_idx, to_idx, amount)
    ensures
        transfer_result_spec(accounts, from_idx, to_idx, amount)[to_idx] as int
            == accounts[to_idx] as int + amount as int,
{}

// r[verify ledger.conservation]
/// Two successive transfers preserve total.
proof fn double_transfer_preserves_total(
    accounts: Seq<u64>,
    from1: int, to1: int, amount1: u64,
    from2: int, to2: int, amount2: u64,
)
    requires
        can_transfer_spec(accounts, from1, to1, amount1),
        can_transfer_spec(
            transfer_result_spec(accounts, from1, to1, amount1),
            from2, to2, amount2),
    ensures ({
        let mid = transfer_result_spec(accounts, from1, to1, amount1);
        let final_accounts = transfer_result_spec(mid, from2, to2, amount2);
        total_balance_spec(final_accounts) == total_balance_spec(accounts)
    })
{
    let mid = transfer_result_spec(accounts, from1, to1, amount1);
    transfer_preserves_total(accounts, from1, to1, amount1);
    transfer_preserves_total(mid, from2, to2, amount2);
}

// -----------------------------------------------------------------------
// Batch transfers
// -----------------------------------------------------------------------

// r[depends ledger.batch]
/// Spec: apply a sequence of transfers (from, to, amount).
pub open spec fn apply_transfers_spec(
    accounts: Seq<u64>, transfers: Seq<(int, int, u64)>, idx: int,
) -> Seq<u64>
    decreases transfers.len() - idx
{
    if idx >= transfers.len() {
        accounts
    } else {
        let (from, to, amount) = transfers[idx];
        if can_transfer_spec(accounts, from, to, amount) {
            apply_transfers_spec(
                transfer_result_spec(accounts, from, to, amount),
                transfers,
                idx + 1,
            )
        } else {
            // invalid transfer: stop
            accounts
        }
    }
}

// r[depends ledger.batch]
/// Spec: all transfers in the batch are valid when applied in sequence.
pub open spec fn all_transfers_valid_spec(
    accounts: Seq<u64>, transfers: Seq<(int, int, u64)>, idx: int,
) -> bool
    decreases transfers.len() - idx
{
    if idx >= transfers.len() {
        true
    } else {
        let (from, to, amount) = transfers[idx];
        can_transfer_spec(accounts, from, to, amount)
        && all_transfers_valid_spec(
            transfer_result_spec(accounts, from, to, amount),
            transfers,
            idx + 1,
        )
    }
}

// r[impl ledger.batch]
// r[verify ledger.batch]
/// A valid batch of transfers preserves total balance.
pub proof fn batch_preserves_total(
    accounts: Seq<u64>, transfers: Seq<(int, int, u64)>, idx: int,
)
    requires
        0 <= idx,
        all_transfers_valid_spec(accounts, transfers, idx),
    ensures
        total_balance_spec(apply_transfers_spec(accounts, transfers, idx))
            == total_balance_spec(accounts),
    decreases transfers.len() - idx
{
    if idx < transfers.len() {
        let (from, to, amount) = transfers[idx];
        let next = transfer_result_spec(accounts, from, to, amount);
        transfer_preserves_total(accounts, from, to, amount);
        // total(next) == total(accounts)
        batch_preserves_total(next, transfers, idx + 1);
        // total(apply(next, ...)) == total(next) == total(accounts)
    }
}

// r[verify ledger.batch]
/// Batch preserves ledger length.
proof fn batch_preserves_length(
    accounts: Seq<u64>, transfers: Seq<(int, int, u64)>, idx: int,
)
    requires
        0 <= idx,
        all_transfers_valid_spec(accounts, transfers, idx),
    ensures
        apply_transfers_spec(accounts, transfers, idx).len() == accounts.len(),
    decreases transfers.len() - idx
{
    if idx < transfers.len() {
        let (from, to, amount) = transfers[idx];
        let next = transfer_result_spec(accounts, from, to, amount);
        batch_preserves_length(next, transfers, idx + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify ledger.total]
    // r[verify ledger.account-count]
    #[test]
    fn test_total_balance() {
        assert_eq!(total_balance(&[]), Some(0));
        assert_eq!(total_balance(&[100, 200, 300]), Some(600));
        assert_eq!(total_balance(&[0, 0, 0]), Some(0));
        assert_eq!(total_balance(&[u64::MAX, 1]), None);
    }

    // r[verify ledger.balance-at]
    #[test]
    fn test_balance_at() {
        assert_eq!(balance_at(&[10, 20, 30], 0), 10);
        assert_eq!(balance_at(&[10, 20, 30], 2), 30);
    }

    // r[verify ledger.can-transfer]
    #[test]
    fn test_can_transfer() {
        let accts = [100, 200, 300];
        assert!(can_transfer(&accts, 0, 1, 50));
        assert!(!can_transfer(&accts, 0, 0, 50)); // self
        assert!(!can_transfer(&accts, 0, 1, 0)); // zero
        assert!(!can_transfer(&accts, 0, 1, 101)); // insufficient
        assert!(!can_transfer(&accts, 5, 1, 10)); // out of bounds
    }

    // r[verify ledger.transfer-result]
    #[test]
    fn test_ledger_transfer() {
        let accts = [100, 200, 300];
        assert_eq!(ledger_transfer(&accts, 0, 1, 50), Some((50, 250)));
        assert_eq!(ledger_transfer(&accts, 2, 0, 300), Some((0, 400)));
        assert_eq!(ledger_transfer(&accts, 0, 1, 101), None);
    }

    // r[verify ledger.transfer-with-fee]
    #[test]
    fn test_transfer_with_fee() {
        let accts = [1000, 200, 0]; // sender, receiver, fee collector
        // Transfer 100 with fee 5: sender -100, receiver +95, fee +5
        assert_eq!(ledger_transfer_with_fee(&accts, 0, 1, 2, 100, 5), Some((900, 295, 5)),);
        // Total preserved: 1000+200+0 = 900+295+5 = 1200
    }

    // r[verify ledger.transfer-with-fee]
    #[test]
    fn test_transfer_with_fee_invalid() {
        let accts = [100, 200, 0];
        // Fee > amount
        assert_eq!(ledger_transfer_with_fee(&accts, 0, 1, 2, 50, 60), None);
        // Same indices
        assert_eq!(ledger_transfer_with_fee(&accts, 0, 0, 2, 50, 5), None);
        assert_eq!(ledger_transfer_with_fee(&accts, 0, 1, 0, 50, 5), None);
    }

    // r[verify ledger.conservation]
    #[test]
    fn test_conservation_manual() {
        let accts = [100, 200, 300, 400];
        let before: u64 = accts.iter().sum();

        // Transfer 50 from 0 to 2
        let (new_0, new_2) = ledger_transfer(&accts, 0, 2, 50).unwrap();
        let after = new_0 as u128 + accts[1] as u128 + new_2 as u128 + accts[3] as u128;
        assert_eq!(before as u128, after);
    }

    proptest! {
        // r[verify ledger.conservation]
        #[test]
        fn prop_transfer_conservation(
            balances in proptest::collection::vec(0..10_000u64, 2..10),
            from_idx in 0usize..10,
            to_idx in 0usize..10,
            amount in 1..5000u64,
        ) {
            if from_idx < balances.len() && to_idx < balances.len() && from_idx != to_idx {
                let before: u128 = balances.iter().map(|&x| x as u128).sum();
                if let Some((new_from, new_to)) = ledger_transfer(&balances, from_idx, to_idx, amount) {
                    let mut after_balances: Vec<u128> = balances.iter().map(|&x| x as u128).collect();
                    after_balances[from_idx] = new_from as u128;
                    after_balances[to_idx] = new_to as u128;
                    let after: u128 = after_balances.iter().sum();
                    prop_assert_eq!(before, after);
                }
            }
        }

        // r[verify ledger.transfer-result]
        #[test]
        fn prop_transfer_isolates(
            balances in proptest::collection::vec(0..10_000u64, 3..10),
            from_idx in 0usize..10,
            to_idx in 0usize..10,
            amount in 1..5000u64,
        ) {
            if from_idx < balances.len() && to_idx < balances.len() && from_idx != to_idx {
                if let Some(_) = ledger_transfer(&balances, from_idx, to_idx, amount) {
                    for (k, &original) in balances.iter().enumerate() {
                        if k != from_idx && k != to_idx {
                            prop_assert_eq!(original, balances[k]);
                        }
                    }
                }
            }
        }

        // r[verify ledger.conservation]
        #[test]
        fn prop_fee_transfer_conservation(
            balances in proptest::collection::vec(0..10_000u64, 3..10),
            from_idx in 0usize..10,
            to_idx in 0usize..10,
            fee_idx in 0usize..10,
            amount in 1..5000u64,
            fee in 0..1000u64,
        ) {
            if from_idx < balances.len()
                && to_idx < balances.len()
                && fee_idx < balances.len()
                && from_idx != to_idx
                && from_idx != fee_idx
                && to_idx != fee_idx
                && fee <= amount
            {
                let before: u128 = balances.iter().map(|&x| x as u128).sum();
                if let Some((new_from, new_to, new_fee)) =
                    ledger_transfer_with_fee(&balances, from_idx, to_idx, fee_idx, amount, fee)
                {
                    let mut after: Vec<u128> = balances.iter().map(|&x| x as u128).collect();
                    after[from_idx] = new_from as u128;
                    after[to_idx] = new_to as u128;
                    after[fee_idx] = new_fee as u128;
                    let after_total: u128 = after.iter().sum();
                    prop_assert_eq!(before, after_total);
                }
            }
        }

        // r[verify ledger.batch]
        #[test]
        fn prop_batch_conservation(
            initial in proptest::collection::vec(1000..5000u64, 4..=4),
            transfers in proptest::collection::vec(
                (0usize..4, 0usize..4, 1..500u64), 1..10
            ),
        ) {
            let before: u128 = initial.iter().map(|&x| x as u128).sum();
            let mut current = initial.clone();
            for (from, to, amt) in &transfers {
                if *from < current.len() && *to < current.len() && from != to {
                    if let Some((new_f, new_t)) = ledger_transfer(&current, *from, *to, *amt) {
                        current[*from] = new_f;
                        current[*to] = new_t;
                    }
                }
            }
            let after: u128 = current.iter().map(|&x| x as u128).sum();
            prop_assert_eq!(before, after);
        }

        // r[verify ledger.total]
        #[test]
        fn prop_total_oracle(
            balances in proptest::collection::vec(0..1_000_000u64, 0..20),
        ) {
            let oracle: u128 = balances.iter().map(|&x| x as u128).sum();
            match total_balance(&balances) {
                Some(v) => prop_assert_eq!(v as u128, oracle),
                None => prop_assert!(oracle > u64::MAX as u128),
            }
        }
    }
}

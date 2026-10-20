//! Verified value transfer between accounts.
//!
//! Validates transfers: sufficient sender funds, valid amount, no receiver
//! overflow. Proves conservation of total value — the sum of sender and
//! receiver balances is unchanged. Idempotency key comparison rejects
//! duplicate transfers before any balance check.

use vstd::prelude::*;

verus! {

// --- Status constants ---

/// Transfer succeeded.
pub open spec fn TRANSFER_OK() -> u8 { 0 }
/// Sender has insufficient funds.
pub open spec fn INSUFFICIENT_FUNDS() -> u8 { 1 }
/// Amount is zero (invalid).
pub open spec fn INVALID_AMOUNT() -> u8 { 2 }
/// Receiver balance would overflow.
pub open spec fn RECEIVER_OVERFLOW() -> u8 { 3 }
/// Duplicate transfer (already processed).
pub open spec fn DUPLICATE_TRANSFER() -> u8 { 4 }

// --- transfer_valid ---

// r[depends transfer.valid]
/// Spec: all preconditions for a valid transfer.
pub open spec fn transfer_valid_spec(
    from_balance: u64, to_balance: u64, amount: u64,
) -> bool {
    amount > 0
    && amount as int <= from_balance as int
    && to_balance as int + amount as int <= u64::MAX as int
}

// r[impl transfer.valid]
/// Returns true if the transfer is valid.
pub fn transfer_valid(
    from_balance: u64, to_balance: u64, amount: u64,
) -> (result: bool)
    ensures result == transfer_valid_spec(from_balance, to_balance, amount)
{
    amount > 0
    && amount <= from_balance
    && amount <= u64::MAX - to_balance
}

// --- sender_after / receiver_after ---

// r[depends transfer.execute]
/// Spec: sender balance after transfer.
pub open spec fn sender_after_spec(from_balance: u64, amount: u64) -> u64
    recommends amount as int <= from_balance as int
{
    (from_balance - amount) as u64
}

// r[depends transfer.execute]
/// Spec: receiver balance after transfer.
pub open spec fn receiver_after_spec(to_balance: u64, amount: u64) -> u64
    recommends to_balance as int + amount as int <= u64::MAX as int
{
    (to_balance + amount) as u64
}

// r[depends transfer.conservation]
/// Spec: sum of two balances.
pub open spec fn total_spec(a: u64, b: u64) -> int {
    a as int + b as int
}

// r[impl transfer.execute]
/// Execute a valid transfer. Returns (new_sender_balance, new_receiver_balance).
pub fn execute_transfer(
    from_balance: u64, to_balance: u64, amount: u64,
) -> (result: (u64, u64))
    requires transfer_valid_spec(from_balance, to_balance, amount)
    ensures
        result.0 == sender_after_spec(from_balance, amount),
        result.1 == receiver_after_spec(to_balance, amount),
{
    (from_balance - amount, to_balance + amount)
}

// --- idempotency ---

// r[depends transfer.idempotency]
/// Spec: a request is a duplicate when its id is not newer than last processed.
pub open spec fn is_duplicate_spec(request_id: u64, last_processed_id: u64) -> bool {
    request_id <= last_processed_id
}

// r[impl transfer.idempotency]
/// Returns true if this request has already been processed.
pub fn is_duplicate(request_id: u64, last_processed_id: u64) -> (result: bool)
    ensures result == is_duplicate_spec(request_id, last_processed_id)
{
    request_id <= last_processed_id
}

// --- validate_transfer ---

// r[depends transfer.validate-exhaustive]
/// Spec: classify the transfer outcome.
pub open spec fn validate_transfer_spec(
    from_balance: u64, to_balance: u64, amount: u64,
    request_id: u64, last_processed_id: u64,
) -> u8 {
    if is_duplicate_spec(request_id, last_processed_id) {
        DUPLICATE_TRANSFER()
    } else if amount == 0 {
        INVALID_AMOUNT()
    } else if amount as int > from_balance as int {
        INSUFFICIENT_FUNDS()
    } else if to_balance as int + amount as int > u64::MAX as int {
        RECEIVER_OVERFLOW()
    } else {
        TRANSFER_OK()
    }
}

// r[impl transfer.validate-exhaustive]
/// Validate a transfer request, returning a status code.
pub fn validate_transfer(
    from_balance: u64, to_balance: u64, amount: u64,
    request_id: u64, last_processed_id: u64,
) -> (result: u8)
    ensures result == validate_transfer_spec(
        from_balance, to_balance, amount, request_id, last_processed_id)
{
    if request_id <= last_processed_id {
        4
    } else if amount == 0 {
        2
    } else if amount > from_balance {
        1
    } else if amount > u64::MAX - to_balance {
        3
    } else {
        0
    }
}

// --- Proofs ---

// r[impl transfer.conservation]
// r[verify transfer.conservation]
/// Total value is conserved across a valid transfer.
proof fn conservation_of_value(
    from_balance: u64, to_balance: u64, amount: u64,
)
    requires transfer_valid_spec(from_balance, to_balance, amount)
    ensures total_spec(
        sender_after_spec(from_balance, amount),
        receiver_after_spec(to_balance, amount),
    ) == total_spec(from_balance, to_balance)
{}

// r[verify transfer.execute]
/// Sender balance strictly decreases.
proof fn transfer_sender_decreases(
    from_balance: u64, to_balance: u64, amount: u64,
)
    requires transfer_valid_spec(from_balance, to_balance, amount)
    ensures (sender_after_spec(from_balance, amount) as int) < from_balance as int
{}

// r[verify transfer.execute]
/// Receiver balance strictly increases.
proof fn transfer_receiver_increases(
    from_balance: u64, to_balance: u64, amount: u64,
)
    requires transfer_valid_spec(from_balance, to_balance, amount)
    ensures (receiver_after_spec(to_balance, amount) as int) > to_balance as int
{}

// r[verify transfer.valid]
/// Valid transfer implies sender has funds.
proof fn transfer_valid_implies_has_funds(
    from_balance: u64, to_balance: u64, amount: u64,
)
    requires transfer_valid_spec(from_balance, to_balance, amount)
    ensures crate::balance::has_funds_spec(from_balance, amount)
{}

// r[verify transfer.validate-exhaustive]
/// OK status if and only if valid and not duplicate.
proof fn valid_result_iff_ok(
    from_balance: u64, to_balance: u64, amount: u64,
    request_id: u64, last_processed_id: u64,
)
    ensures
        (validate_transfer_spec(from_balance, to_balance, amount,
            request_id, last_processed_id) == TRANSFER_OK())
        <==> (transfer_valid_spec(from_balance, to_balance, amount)
              && !is_duplicate_spec(request_id, last_processed_id)),
{}

// r[verify transfer.validate-exhaustive]
/// The status is always one of the five codes.
proof fn statuses_exhaustive(
    from_balance: u64, to_balance: u64, amount: u64,
    request_id: u64, last_processed_id: u64,
)
    ensures ({
        let s = validate_transfer_spec(
            from_balance, to_balance, amount, request_id, last_processed_id);
        s == TRANSFER_OK() || s == INSUFFICIENT_FUNDS() || s == INVALID_AMOUNT()
        || s == RECEIVER_OVERFLOW() || s == DUPLICATE_TRANSFER()
    })
{}

// r[verify transfer.idempotency]
/// Duplicates are always rejected regardless of balances.
proof fn duplicate_always_rejected(
    from_balance: u64, to_balance: u64, amount: u64,
    request_id: u64, last_processed_id: u64,
)
    requires is_duplicate_spec(request_id, last_processed_id)
    ensures validate_transfer_spec(
        from_balance, to_balance, amount,
        request_id, last_processed_id) == DUPLICATE_TRANSFER()
{}

// r[verify transfer.execute]
/// Transferring zero amount changes nothing.
proof fn zero_transfer_self(balance: u64)
    ensures
        sender_after_spec(balance, 0) == balance,
        receiver_after_spec(balance, 0) == balance,
{}

// r[verify transfer.conservation]
/// Two successive valid transfers preserve three-account total.
proof fn double_transfer_conservation(
    b1: u64, b2: u64, b3: u64, a1: u64, a2: u64,
)
    requires
        transfer_valid_spec(b1, b2, a1),
        transfer_valid_spec(receiver_after_spec(b2, a1), b3, a2),
    ensures ({
        let b2_mid = receiver_after_spec(b2, a1);
        total_spec(sender_after_spec(b1, a1), b2_mid) + b3 as int
            == b1 as int + b2 as int + b3 as int
    })
{}

// r[verify transfer.idempotency]
/// A non-duplicate request has a strictly higher id.
proof fn idempotency_advance(request_id: u64, last_processed_id: u64)
    requires !is_duplicate_spec(request_id, last_processed_id)
    ensures request_id > last_processed_id
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify transfer.valid]
    #[test]
    fn test_transfer_valid() {
        assert!(transfer_valid(100, 50, 30));
        assert!(!transfer_valid(100, 50, 0)); // zero amount
        assert!(!transfer_valid(100, 50, 101)); // insufficient
        assert!(!transfer_valid(100, u64::MAX, 1)); // overflow
    }

    // r[verify transfer.execute]
    #[test]
    fn test_execute_transfer() {
        assert_eq!(execute_transfer(100, 50, 30), (70, 80));
        assert_eq!(execute_transfer(100, 0, 100), (0, 100));
    }

    // r[verify transfer.idempotency]
    #[test]
    fn test_is_duplicate() {
        assert!(is_duplicate(5, 5));
        assert!(is_duplicate(3, 5));
        assert!(!is_duplicate(6, 5));
        assert!(is_duplicate(0, 0));
    }

    // r[verify transfer.validate-exhaustive]
    #[test]
    fn test_validate_transfer() {
        assert_eq!(validate_transfer(100, 50, 30, 2, 1), 0); // OK
        assert_eq!(validate_transfer(100, 50, 30, 1, 1), 4); // duplicate
        assert_eq!(validate_transfer(100, 50, 0, 2, 1), 2); // invalid amount
        assert_eq!(validate_transfer(100, 50, 101, 2, 1), 1); // insufficient
        assert_eq!(validate_transfer(100, u64::MAX, 1, 2, 1), 3); // overflow
    }

    proptest! {
        // r[verify transfer.conservation]
        #[test]
        fn prop_conservation(from in 1..u64::MAX / 2, to in 0..u64::MAX / 2, amt in 1..u64::MAX / 2) {
            if transfer_valid(from, to, amt) {
                let (new_from, new_to) = execute_transfer(from, to, amt);
                prop_assert_eq!(
                    from as u128 + to as u128,
                    new_from as u128 + new_to as u128,
                );
            }
        }

        // r[verify transfer.validate-exhaustive]
        #[test]
        fn prop_valid_iff_ok(from: u64, to: u64, amt: u64, rid: u64, lid: u64) {
            let status = validate_transfer(from, to, amt, rid, lid);
            let ok = transfer_valid(from, to, amt) && !is_duplicate(rid, lid);
            prop_assert_eq!(status == 0, ok);
        }

        // r[verify transfer.idempotency]
        #[test]
        fn prop_duplicate_blocks(from: u64, to: u64, amt: u64, lid in 1..u64::MAX) {
            let rid = lid - 1;
            prop_assert_eq!(validate_transfer(from, to, amt, rid, lid), 4);
        }

        // r[verify transfer.conservation]
        #[test]
        fn prop_sender_decreases_receiver_increases(
            from in 1..u64::MAX / 2,
            to in 0..u64::MAX / 2,
            amt in 1..u64::MAX / 4,
        ) {
            if transfer_valid(from, to, amt) {
                let (new_from, new_to) = execute_transfer(from, to, amt);
                prop_assert!(new_from < from);
                prop_assert!(new_to > to);
            }
        }

        // r[verify transfer.idempotency]
        #[test]
        fn prop_fresh_request_has_higher_id(rid: u64, lid: u64) {
            if !is_duplicate(rid, lid) {
                prop_assert!(rid > lid);
            }
        }
    }
}

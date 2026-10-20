//! Verified AppendEntries response dispatch for Raft leaders.
//!
//! When a leader receives an AppendEntries response, it classifies the
//! response and updates matchIndex/nextIndex accordingly. Four cases:
//! higher term (step down), stale term (ignore), success (advance),
//! failure (backtrack). Individual pieces live in match_index.rs and
//! fast_backtrack.rs; this module provides the dispatch logic.

use vstd::prelude::*;

verus! {

/// Response from a server with a higher term — must step down.
pub const RESP_HIGHER_TERM: u8 = 0u8;
/// Response from a server with a stale (lower) term — ignore.
pub const RESP_STALE: u8 = 1u8;
/// Same-term success — entries accepted.
pub const RESP_SUCCESS: u8 = 2u8;
/// Same-term failure — log consistency check failed.
pub const RESP_FAILURE: u8 = 3u8;

// --- classify_append_response ---

// r[depends append-response.classify]
/// Spec: classify an AppendEntries response into one of four categories.
pub open spec fn classify_append_response_spec(
    response_term: u64,
    current_term: u64,
    success: bool,
) -> u8 {
    if response_term > current_term {
        RESP_HIGHER_TERM
    } else if response_term < current_term {
        RESP_STALE
    } else if success {
        RESP_SUCCESS
    } else {
        RESP_FAILURE
    }
}

// r[impl append-response.classify]
/// Classify an AppendEntries response.
pub fn classify_append_response(
    response_term: u64,
    current_term: u64,
    success: bool,
) -> (result: u8)
    ensures result == classify_append_response_spec(response_term, current_term, success)
{
    if response_term > current_term {
        RESP_HIGHER_TERM
    } else if response_term < current_term {
        RESP_STALE
    } else if success {
        RESP_SUCCESS
    } else {
        RESP_FAILURE
    }
}

// --- on_append_success ---

// r[depends append-response.on-success]
/// Spec: compute new (match_index, next_index) after a successful response.
///
/// match advances to cover the acknowledged entries.
/// next moves one past the new match.
pub open spec fn on_append_success_spec(
    match_index: u64,
    next_index: u64,
    entries_sent: u64,
) -> (u64, u64)
    recommends
        match_index as int + entries_sent as int <= u64::MAX as int,
{
    let new_match = (match_index as int + entries_sent as int) as u64;
    let new_next = (new_match as int + 1) as u64;
    (new_match, new_next)
}

// r[impl append-response.on-success]
/// Update match/next indices after a successful AppendEntries.
pub fn on_append_success(
    match_index: u64,
    next_index: u64,
    entries_sent: u64,
) -> (result: (u64, u64))
    requires
        match_index as int + entries_sent as int <= u64::MAX as int,
        match_index as int + entries_sent as int + 1 <= u64::MAX as int,
    ensures result == on_append_success_spec(match_index, next_index, entries_sent)
{
    let new_match = match_index + entries_sent;
    let new_next = new_match + 1;
    (new_match, new_next)
}

// --- on_append_failure ---

// r[depends append-response.on-failure]
/// Spec: compute new next_index after a failed response (basic backtrack).
///
/// Decrements next_index by 1, with a floor of 1.
pub open spec fn on_append_failure_spec(next_index: u64) -> u64 {
    if next_index > 1 {
        (next_index - 1) as u64
    } else {
        1u64
    }
}

// r[impl append-response.on-failure]
/// Backtrack next_index by one step after a failed AppendEntries.
pub fn on_append_failure(next_index: u64) -> (result: u64)
    ensures result == on_append_failure_spec(next_index)
{
    if next_index > 1 {
        next_index - 1
    } else {
        1
    }
}

// --- Safety proofs ---

// r[verify append-response.classify]
/// The four classification results are exhaustive and correct.
proof fn classify_exhaustive(response_term: u64, current_term: u64, success: bool)
    ensures ({
        let c = classify_append_response_spec(response_term, current_term, success);
        c == RESP_HIGHER_TERM || c == RESP_STALE || c == RESP_SUCCESS || c == RESP_FAILURE
    })
{}

// r[verify append-response.classify]
/// Higher-term response is classified correctly.
proof fn classify_higher_term(response_term: u64, current_term: u64, success: bool)
    requires response_term > current_term
    ensures classify_append_response_spec(response_term, current_term, success) == RESP_HIGHER_TERM
{}

// r[verify append-response.classify]
/// Stale-term response is classified correctly.
proof fn classify_stale(response_term: u64, current_term: u64, success: bool)
    requires response_term < current_term
    ensures classify_append_response_spec(response_term, current_term, success) == RESP_STALE
{}

// r[verify append-response.classify]
/// Same-term success is classified correctly.
proof fn classify_success(term: u64)
    ensures classify_append_response_spec(term, term, true) == RESP_SUCCESS
{}

// r[verify append-response.classify]
/// Same-term failure is classified correctly.
proof fn classify_failure(term: u64)
    ensures classify_append_response_spec(term, term, false) == RESP_FAILURE
{}

// r[impl append-response.match-advances]
// r[verify append-response.match-advances]
/// Match index advances on success.
proof fn match_advances_on_success(
    match_index: u64,
    next_index: u64,
    entries_sent: u64,
)
    requires
        entries_sent > 0,
        match_index as int + entries_sent as int <= u64::MAX as int,
    ensures on_append_success_spec(match_index, next_index, entries_sent).0 > match_index
{}

// r[verify append-response.match-advances]
/// Match index is monotonic: new >= old.
proof fn match_monotonic_on_success(
    match_index: u64,
    next_index: u64,
    entries_sent: u64,
)
    requires match_index as int + entries_sent as int <= u64::MAX as int
    ensures on_append_success_spec(match_index, next_index, entries_sent).0 >= match_index
{}

// r[impl append-response.next-after-success]
// r[verify append-response.next-after-success]
/// After success, next_index == match_index + 1.
proof fn next_follows_match(
    match_index: u64,
    next_index: u64,
    entries_sent: u64,
)
    requires
        match_index as int + entries_sent as int <= u64::MAX as int,
        match_index as int + entries_sent as int + 1 <= u64::MAX as int,
    ensures ({
        let (new_match, new_next) = on_append_success_spec(match_index, next_index, entries_sent);
        new_next == new_match + 1
    })
{}

// r[verify append-response.on-failure]
/// Backtrack floor: result >= 1.
proof fn backtrack_floor(next_index: u64)
    ensures on_append_failure_spec(next_index) >= 1
{}

// r[verify append-response.on-failure]
/// Backtrack decreases next_index (when > 1).
proof fn backtrack_decreases(next_index: u64)
    requires next_index > 1
    ensures on_append_failure_spec(next_index) < next_index
{}

// r[verify append-response.on-failure]
/// Backtrack at floor is a no-op.
proof fn backtrack_at_floor()
    ensures on_append_failure_spec(1) == 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify append-response.classify]
    #[test]
    fn test_classify_append_response() {
        // Higher term → HIGHER_TERM
        assert_eq!(classify_append_response(5, 3, true), RESP_HIGHER_TERM);
        assert_eq!(classify_append_response(5, 3, false), RESP_HIGHER_TERM);
        // Stale term → STALE
        assert_eq!(classify_append_response(2, 3, true), RESP_STALE);
        assert_eq!(classify_append_response(2, 3, false), RESP_STALE);
        // Same term success → SUCCESS
        assert_eq!(classify_append_response(3, 3, true), RESP_SUCCESS);
        // Same term failure → FAILURE
        assert_eq!(classify_append_response(3, 3, false), RESP_FAILURE);
    }

    // r[verify append-response.on-success]
    #[test]
    fn test_on_append_success() {
        let (new_match, new_next) = on_append_success(0, 1, 5);
        assert_eq!(new_match, 5);
        assert_eq!(new_next, 6);

        let (m, n) = on_append_success(3, 4, 2);
        assert_eq!(m, 5);
        assert_eq!(n, 6);
    }

    // r[verify append-response.on-failure]
    #[test]
    fn test_on_append_failure() {
        assert_eq!(on_append_failure(5), 4);
        assert_eq!(on_append_failure(2), 1);
        assert_eq!(on_append_failure(1), 1); // floor
        assert_eq!(on_append_failure(0), 1); // floor
    }

    proptest! {
        // r[verify append-response.classify]
        #[test]
        fn prop_classify_exhaustive(
            response_term in 0u64..10,
            current_term in 0u64..10,
            success: bool,
        ) {
            let c = classify_append_response(response_term, current_term, success);
            prop_assert!(
                c == RESP_HIGHER_TERM || c == RESP_STALE
                || c == RESP_SUCCESS || c == RESP_FAILURE
            );
        }

        // r[verify append-response.match-advances]
        #[test]
        fn prop_match_monotonic(
            match_index in 0u64..1000,
            next_index in 0u64..1000,
            entries_sent in 0u64..100,
        ) {
            let sum = match_index.checked_add(entries_sent).and_then(|s| s.checked_add(1));
            if let Some(_) = sum {
                let (new_match, _) = on_append_success(match_index, next_index, entries_sent);
                prop_assert!(new_match >= match_index);
            }
        }

        // r[verify append-response.on-failure]
        #[test]
        fn prop_backtrack_floor(next_index in 0u64..1000) {
            let result = on_append_failure(next_index);
            prop_assert!(result >= 1);
            if next_index > 1 {
                prop_assert!(result < next_index);
            }
        }

        // r[verify append-response.next-after-success]
        #[test]
        fn prop_next_follows_match(
            match_index in 0u64..1000,
            entries_sent in 0u64..100,
        ) {
            let sum = match_index.checked_add(entries_sent).and_then(|s| s.checked_add(1));
            if let Some(_) = sum {
                let (new_match, new_next) = on_append_success(match_index, 0, entries_sent);
                prop_assert_eq!(new_next, new_match + 1);
            }
        }
    }
}

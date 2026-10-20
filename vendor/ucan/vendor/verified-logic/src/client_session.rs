//! Verified client session tracking for Raft exactly-once semantics.
//!
//! Clients register sessions with unique IDs. Each command carries a
//! (client_id, sequence_number) pair. The state machine maintains a
//! deduplication table mapping (client_id, seq_no) → cached response.
//! Duplicate commands return the cached response instead of re-executing.
//! This prevents duplicate execution on client retries — the piece that
//! makes Raft linearizable end-to-end (§8 of the Raft paper).

use vstd::prelude::*;

verus! {

// --- Session entry ---

/// A cached response entry in the deduplication table.
/// Tracks the highest sequence number processed and its response.
pub struct SessionEntry {
    /// Highest sequence number processed for this client.
    pub last_seq: u64,
    /// Cached response value for the last processed command.
    pub last_response: u64,
}

// --- is_duplicate ---

// r[depends client-session.is-duplicate]
/// Spec: is this command a duplicate (already processed)?
///
/// A command is a duplicate when the session entry exists and
/// the command's sequence number is <= the last processed sequence.
pub open spec fn is_duplicate_spec(
    entry_last_seq: u64,
    command_seq: u64,
) -> bool {
    command_seq <= entry_last_seq
}

// r[impl client-session.is-duplicate]
/// Check if a command is a duplicate.
pub fn is_duplicate(
    entry_last_seq: u64,
    command_seq: u64,
) -> (result: bool)
    ensures result == is_duplicate_spec(entry_last_seq, command_seq)
{
    command_seq <= entry_last_seq
}

// --- is_next_in_sequence ---

// r[depends client-session.is-next-in-sequence]
/// Spec: is this the expected next command in the session?
///
/// Exactly-once semantics require that commands arrive in order.
/// The next expected sequence number is last_seq + 1.
pub open spec fn is_next_in_sequence_spec(
    entry_last_seq: u64,
    command_seq: u64,
) -> bool {
    command_seq == entry_last_seq + 1
}

// r[impl client-session.is-next-in-sequence]
/// Check if a command is the next expected in the session.
pub fn is_next_in_sequence(
    entry_last_seq: u64,
    command_seq: u64,
) -> (result: bool)
    requires entry_last_seq < u64::MAX
    ensures result == is_next_in_sequence_spec(entry_last_seq, command_seq)
{
    command_seq == entry_last_seq + 1
}

// --- classify_command ---

/// Command classification result.
/// 0 = duplicate (already processed, return cached response)
/// 1 = next (expected next command, process it)
/// 2 = future (sequence gap, reject — client must resend intermediates)
pub const COMMAND_DUPLICATE: u8 = 0;
/// Expected next command — process it.
pub const COMMAND_NEXT: u8 = 1;
/// Future command — sequence gap, reject.
pub const COMMAND_FUTURE: u8 = 2;

// r[depends client-session.classify-command]
/// Spec: classify a command relative to the session state.
pub open spec fn classify_command_spec(
    entry_last_seq: u64,
    command_seq: u64,
) -> u8 {
    if command_seq <= entry_last_seq {
        COMMAND_DUPLICATE
    } else if command_seq == entry_last_seq + 1 {
        COMMAND_NEXT
    } else {
        COMMAND_FUTURE
    }
}

// r[impl client-session.classify-command]
/// Classify a command: duplicate, next, or future.
pub fn classify_command(
    entry_last_seq: u64,
    command_seq: u64,
) -> (result: u8)
    ensures result == classify_command_spec(entry_last_seq, command_seq)
{
    if command_seq <= entry_last_seq {
        COMMAND_DUPLICATE
    } else if entry_last_seq < u64::MAX && command_seq == entry_last_seq + 1 {
        COMMAND_NEXT
    } else {
        COMMAND_FUTURE
    }
}

// --- update_session ---

// r[depends client-session.update-session]
/// Spec: update the session entry after processing a command.
///
/// The sequence number advances and the response is cached.
pub open spec fn update_session_spec(
    old_last_seq: u64,
    new_seq: u64,
    response: u64,
) -> SessionEntry {
    SessionEntry { last_seq: new_seq, last_response: response }
}

// r[impl client-session.update-session]
/// Update a session entry after processing the command.
pub fn update_session(
    _old_last_seq: u64,
    new_seq: u64,
    response: u64,
) -> (result: SessionEntry)
    requires new_seq > _old_last_seq
    ensures
        result.last_seq == new_seq,
        result.last_response == response,
{
    SessionEntry { last_seq: new_seq, last_response: response }
}

// --- new_session ---

// r[depends client-session.new-session]
/// Spec: initial session state (no commands processed).
pub open spec fn new_session_spec() -> SessionEntry {
    SessionEntry { last_seq: 0, last_response: 0 }
}

// r[impl client-session.new-session]
/// Create a new session entry.
pub fn new_session() -> (result: SessionEntry)
    ensures
        result.last_seq == 0,
        result.last_response == 0,
{
    SessionEntry { last_seq: 0, last_response: 0 }
}

// --- is_session_expired ---

// r[depends client-session.session-expired]
/// Spec: has the session expired (no activity within timeout)?
pub open spec fn is_session_expired_spec(
    last_activity: u64,
    now: u64,
    session_timeout: u64,
) -> bool {
    now > last_activity && (now - last_activity) > session_timeout
}

// r[impl client-session.session-expired]
/// Check if a client session has expired.
pub fn is_session_expired(
    last_activity: u64,
    now: u64,
    session_timeout: u64,
) -> (result: bool)
    ensures result == is_session_expired_spec(last_activity, now, session_timeout)
{
    now > last_activity && (now - last_activity) > session_timeout
}

// --- register_session ---

// r[depends client-session.register]
pub open spec fn register_session_spec(next_session_id: u64) -> (u64, u64) {
    (next_session_id, (next_session_id + 1) as u64)
}

// r[impl client-session.register]
pub fn register_session(next_session_id: u64) -> (result: (u64, u64))
    requires next_session_id < u64::MAX
    ensures result.0 == next_session_id, result.1 == (next_session_id + 1) as u64,
{ (next_session_id, next_session_id + 1) }

// --- advance_session_seq ---

// r[depends client-session.advance-seq]
pub open spec fn advance_session_seq_spec(current_seq: u64, request_seq: u64) -> u64 {
    if request_seq > current_seq { request_seq } else { current_seq }
}

// r[impl client-session.advance-seq]
pub fn advance_session_seq(current_seq: u64, request_seq: u64) -> (result: u64)
    ensures result == advance_session_seq_spec(current_seq, request_seq)
{ if request_seq > current_seq { request_seq } else { current_seq } }

// --- session_exists ---

// r[depends client-session.lookup]
pub open spec fn session_exists_spec(table_len: u64, session_id: u64) -> bool { session_id < table_len }

// r[impl client-session.lookup]
pub fn session_exists(table_len: u64, session_id: u64) -> (result: bool)
    ensures result == session_exists_spec(table_len, session_id)
{ session_id < table_len }

// --- session_timed_out ---

// r[depends client-session.expire]
pub open spec fn session_timed_out_spec(last_activity: u64, now: u64, session_timeout: u64) -> bool {
    is_session_expired_spec(last_activity, now, session_timeout)
}

// r[impl client-session.expire]
pub fn session_timed_out(last_activity: u64, now: u64, session_timeout: u64) -> (result: bool)
    ensures result == session_timed_out_spec(last_activity, now, session_timeout)
{ is_session_expired(last_activity, now, session_timeout) }

// --- cached_response_valid ---

// r[depends client-session.response-cache]
pub open spec fn cached_response_valid_spec(cached_seq: u64, request_seq: u64) -> bool { cached_seq == request_seq }

// r[impl client-session.response-cache]
pub fn cached_response_valid(cached_seq: u64, request_seq: u64) -> (result: bool)
    ensures result == cached_response_valid_spec(cached_seq, request_seq)
{ cached_seq == request_seq }

// --- session_count_within_limit ---

// r[depends client-session.table-bounded]
pub open spec fn session_count_within_limit_spec(count: u64, max_sessions: u64) -> bool { count < max_sessions }

// r[impl client-session.table-bounded]
pub fn session_count_within_limit(count: u64, max_sessions: u64) -> (result: bool)
    ensures result == session_count_within_limit_spec(count, max_sessions)
{ count < max_sessions }

// --- Safety proofs ---

// r[verify client-session.register]
proof fn register_advances(id: u64) requires id < u64::MAX
    ensures register_session_spec(id).0 == id, register_session_spec(id).1 == (id + 1) as u64,
{}

// r[verify client-session.advance-seq]
proof fn advance_monotonic(current_seq: u64, request_seq: u64)
    ensures advance_session_seq_spec(current_seq, request_seq) >= current_seq
{}

// r[verify client-session.lookup]
proof fn session_zero_exists(table_len: u64) requires table_len > 0
    ensures session_exists_spec(table_len, 0)
{}

// r[verify client-session.expire]
proof fn just_active_not_timed_out(now: u64, timeout: u64)
    ensures !session_timed_out_spec(now, now, timeout)
{}

// r[verify client-session.response-cache]
proof fn cached_response_exact(seq: u64) ensures cached_response_valid_spec(seq, seq) {}

// r[verify client-session.table-bounded]
proof fn empty_table_within_limit(max_sessions: u64) requires max_sessions > 0
    ensures session_count_within_limit_spec(0, max_sessions)
{}

// r[verify client-session.is-duplicate]
/// First command (seq 1) on a fresh session is not a duplicate.
proof fn first_command_not_duplicate()
    ensures !is_duplicate_spec(0, 1)
{}

// r[verify client-session.is-duplicate]
/// Re-sending the last processed command is a duplicate.
proof fn resend_is_duplicate(entry_last_seq: u64)
    ensures is_duplicate_spec(entry_last_seq, entry_last_seq)
{}

// r[verify client-session.is-duplicate]
/// Any command with seq < last_seq is a duplicate.
proof fn past_command_is_duplicate(entry_last_seq: u64, command_seq: u64)
    requires command_seq < entry_last_seq
    ensures is_duplicate_spec(entry_last_seq, command_seq)
{}

// r[verify client-session.is-duplicate]
/// Command with seq > last_seq is not a duplicate.
proof fn future_command_not_duplicate(entry_last_seq: u64, command_seq: u64)
    requires command_seq > entry_last_seq
    ensures !is_duplicate_spec(entry_last_seq, command_seq)
{}

// r[verify client-session.classify-command]
/// Classification is exhaustive: every command is exactly one of the three.
proof fn classify_exhaustive(entry_last_seq: u64, command_seq: u64)
    ensures
        classify_command_spec(entry_last_seq, command_seq) == COMMAND_DUPLICATE
        || classify_command_spec(entry_last_seq, command_seq) == COMMAND_NEXT
        || classify_command_spec(entry_last_seq, command_seq) == COMMAND_FUTURE
{}

// r[verify client-session.classify-command]
/// Duplicate classification agrees with is_duplicate for seq <= last_seq.
proof fn classify_duplicate_agrees(entry_last_seq: u64, command_seq: u64)
    requires command_seq <= entry_last_seq
    ensures
        classify_command_spec(entry_last_seq, command_seq) == COMMAND_DUPLICATE,
        is_duplicate_spec(entry_last_seq, command_seq),
{}

// r[verify client-session.classify-command]
/// Next classification agrees with is_next_in_sequence.
proof fn classify_next_agrees(entry_last_seq: u64, command_seq: u64)
    requires command_seq == entry_last_seq + 1
    ensures
        classify_command_spec(entry_last_seq, command_seq) == COMMAND_NEXT,
        is_next_in_sequence_spec(entry_last_seq, command_seq),
{}

// r[verify client-session.classify-command]
/// Future command has seq > last_seq + 1.
proof fn classify_future_gap(entry_last_seq: u64, command_seq: u64)
    requires
        command_seq > entry_last_seq,
        command_seq != entry_last_seq + 1,
    ensures classify_command_spec(entry_last_seq, command_seq) == COMMAND_FUTURE
{}

// r[verify client-session.update-session]
/// Update advances the sequence number.
proof fn update_advances_seq(old_last_seq: u64, new_seq: u64, response: u64)
    requires new_seq > old_last_seq
    ensures update_session_spec(old_last_seq, new_seq, response).last_seq > old_last_seq
{}

// r[verify client-session.update-session]
/// After update, the old command becomes a duplicate.
proof fn update_makes_old_duplicate(old_last_seq: u64, new_seq: u64, response: u64)
    requires new_seq > old_last_seq
    ensures is_duplicate_spec(
        update_session_spec(old_last_seq, new_seq, response).last_seq,
        old_last_seq,
    )
{}

// r[verify client-session.update-session]
/// After update, the just-processed command is also a duplicate.
proof fn update_makes_current_duplicate(old_last_seq: u64, new_seq: u64, response: u64)
    requires new_seq > old_last_seq
    ensures is_duplicate_spec(
        update_session_spec(old_last_seq, new_seq, response).last_seq,
        new_seq,
    )
{}

// r[verify client-session.update-session]
/// Sequence monotonicity: updating with the next command produces
/// a session where the following command is next_in_sequence.
proof fn update_next_sequence(old_last_seq: u64, response: u64)
    requires old_last_seq + 1 < u64::MAX
    ensures is_next_in_sequence_spec(
        update_session_spec(old_last_seq, (old_last_seq + 1) as u64, response).last_seq,
        (old_last_seq + 2) as u64,
    )
{}

// r[verify client-session.session-expired]
/// A session that was just active is not expired.
proof fn active_session_not_expired(now: u64, timeout: u64)
    ensures !is_session_expired_spec(now, now, timeout)
{}

// r[verify client-session.session-expired]
/// A session becomes expired when enough time passes.
proof fn session_expires_after_timeout(last_activity: u64, timeout: u64)
    requires
        last_activity < u64::MAX,
        timeout < u64::MAX,
        last_activity as int + timeout as int + 1 <= u64::MAX as int,
    ensures is_session_expired_spec(
        last_activity,
        (last_activity + timeout + 1) as u64,
        timeout,
    )
{}

// r[verify client-session.is-duplicate]
/// Duplicate detection is exact: not-duplicate implies seq > last_seq.
proof fn not_duplicate_implies_new(entry_last_seq: u64, command_seq: u64)
    requires !is_duplicate_spec(entry_last_seq, command_seq)
    ensures command_seq > entry_last_seq
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify client-session.new-session]
    #[test]
    fn test_new_session() {
        let s = new_session();
        assert_eq!(s.last_seq, 0);
        assert_eq!(s.last_response, 0);
    }

    // r[verify client-session.is-duplicate]
    #[test]
    fn test_is_duplicate() {
        assert!(!is_duplicate(0, 1)); // first command
        assert!(is_duplicate(5, 5)); // resend last
        assert!(is_duplicate(5, 3)); // old command
        assert!(!is_duplicate(5, 6)); // new command
    }

    // r[verify client-session.is-next-in-sequence]
    #[test]
    fn test_is_next_in_sequence() {
        assert!(is_next_in_sequence(0, 1));
        assert!(is_next_in_sequence(5, 6));
        assert!(!is_next_in_sequence(5, 7)); // gap
        assert!(!is_next_in_sequence(5, 5)); // resend
    }

    // r[verify client-session.classify-command]
    #[test]
    fn test_classify_command() {
        assert_eq!(classify_command(5, 3), COMMAND_DUPLICATE);
        assert_eq!(classify_command(5, 5), COMMAND_DUPLICATE);
        assert_eq!(classify_command(5, 6), COMMAND_NEXT);
        assert_eq!(classify_command(5, 8), COMMAND_FUTURE);
    }

    // r[verify client-session.update-session]
    #[test]
    fn test_update_session() {
        let s = update_session(0, 1, 42);
        assert_eq!(s.last_seq, 1);
        assert_eq!(s.last_response, 42);

        // Old command is now duplicate
        assert!(is_duplicate(s.last_seq, 1));
        // Next command is seq 2
        assert!(is_next_in_sequence(s.last_seq, 2));
    }

    // r[verify client-session.session-expired]
    #[test]
    fn test_session_expired() {
        assert!(!is_session_expired(100, 100, 50)); // same time
        assert!(!is_session_expired(100, 120, 50)); // within timeout
        assert!(!is_session_expired(100, 150, 50)); // at boundary
        assert!(is_session_expired(100, 151, 50)); // expired
    }

    // r[verify client-session.classify-command]
    #[test]
    fn test_sequential_commands() {
        let mut s = new_session();
        for seq in 1..=10u64 {
            assert_eq!(classify_command(s.last_seq, seq), COMMAND_NEXT);
            s = update_session(s.last_seq, seq, seq * 10);
            assert_eq!(s.last_seq, seq);
            // Previous commands are now duplicates
            for past in 1..=seq {
                assert!(is_duplicate(s.last_seq, past));
            }
        }
    }

    proptest! {
        // r[verify client-session.is-duplicate]
        #[test]
        fn prop_duplicate_iff_leq(last_seq: u64, cmd_seq: u64) {
            prop_assert_eq!(is_duplicate(last_seq, cmd_seq), cmd_seq <= last_seq);
        }

        // r[verify client-session.classify-command]
        #[test]
        fn prop_classify_exhaustive(last_seq in 0u64..u64::MAX - 1, cmd_seq: u64) {
            let c = classify_command(last_seq, cmd_seq);
            prop_assert!(c == COMMAND_DUPLICATE || c == COMMAND_NEXT || c == COMMAND_FUTURE);
        }

        // r[verify client-session.classify-command]
        #[test]
        fn prop_classify_duplicate_agrees(last_seq: u64, cmd_seq: u64) {
            if cmd_seq <= last_seq {
                prop_assert_eq!(classify_command(last_seq, cmd_seq), COMMAND_DUPLICATE);
                prop_assert!(is_duplicate(last_seq, cmd_seq));
            }
        }

        // r[verify client-session.update-session]
        #[test]
        fn prop_update_makes_old_duplicate(
            old_seq in 0u64..u64::MAX / 2,
            response: u64,
        ) {
            let new_seq = old_seq + 1;
            let s = update_session(old_seq, new_seq, response);
            prop_assert!(is_duplicate(s.last_seq, old_seq));
            prop_assert!(is_duplicate(s.last_seq, new_seq));
        }

        // r[verify client-session.session-expired]
        #[test]
        fn prop_current_activity_not_expired(now: u64, timeout: u64) {
            prop_assert!(!is_session_expired(now, now, timeout));
        }

        // r[verify client-session.classify-command]
        #[test]
        fn prop_next_then_duplicate_after_update(
            last_seq in 0u64..u64::MAX - 2,
            response: u64,
        ) {
            let cmd_seq = last_seq + 1;
            prop_assert_eq!(classify_command(last_seq, cmd_seq), COMMAND_NEXT);
            let s = update_session(last_seq, cmd_seq, response);
            prop_assert_eq!(classify_command(s.last_seq, cmd_seq), COMMAND_DUPLICATE);
        }
    }
}

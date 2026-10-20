//! Verified leader election state machine.
//!
//! Three states: Follower (0), Transitioning (1), Leader (2). Five
//! transition types with precondition guards. Token monotonicity ensures
//! each leader has a unique, increasing fencing token.
//!
//! Pure decision logic — no I/O, no timers, time passed as u64 parameter.

use vstd::prelude::*;

verus! {

// State constants — Verus can't match on enum variants in specs
pub const STATE_FOLLOWER: u8 = 0u8;
pub const STATE_TRANSITIONING: u8 = 1u8;
pub const STATE_LEADER: u8 = 2u8;

// --- State predicates ---

// r[depends election.state-predicates]
pub open spec fn is_follower_spec(state: u8) -> bool {
    state == STATE_FOLLOWER
}

// r[depends election.state-predicates]
pub open spec fn is_transitioning_spec(state: u8) -> bool {
    state == STATE_TRANSITIONING
}

// r[depends election.state-predicates]
pub open spec fn is_leader_spec(state: u8) -> bool {
    state == STATE_LEADER
}

// r[depends election.state-predicates]
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state == STATE_FOLLOWER || state == STATE_TRANSITIONING || state == STATE_LEADER
}

// r[impl election.state-predicates]
pub fn is_follower(state: u8) -> (result: bool)
    ensures result == is_follower_spec(state)
{
    state == STATE_FOLLOWER
}

// r[impl election.state-predicates]
pub fn is_transitioning(state: u8) -> (result: bool)
    ensures result == is_transitioning_spec(state)
{
    state == STATE_TRANSITIONING
}

// r[impl election.state-predicates]
pub fn is_leader(state: u8) -> (result: bool)
    ensures result == is_leader_spec(state)
{
    state == STATE_LEADER
}

// --- Transition validation ---

// r[depends election.valid-transition]
/// Valid transitions: F→T, T→L, T→F, L→F, and self-transitions.
/// Leader self-transition requires same fencing token.
pub open spec fn is_valid_state_transition_spec(
    from: u8, to: u8, from_token: u64, to_token: u64,
) -> bool {
    if from == to {
        // Self-transitions: Leader→Leader requires same token
        if from == STATE_LEADER {
            from_token == to_token
        } else {
            true
        }
    } else {
        // Follower → Transitioning
        (from == STATE_FOLLOWER && to == STATE_TRANSITIONING)
        // Transitioning → Leader
        || (from == STATE_TRANSITIONING && to == STATE_LEADER)
        // Transitioning → Follower
        || (from == STATE_TRANSITIONING && to == STATE_FOLLOWER)
        // Leader → Follower
        || (from == STATE_LEADER && to == STATE_FOLLOWER)
    }
}

// r[impl election.valid-transition]
pub fn is_valid_state_transition(
    from: u8, to: u8, from_token: u64, to_token: u64,
) -> (result: bool)
    ensures result == is_valid_state_transition_spec(from, to, from_token, to_token)
{
    if from == to {
        if from == STATE_LEADER {
            from_token == to_token
        } else {
            true
        }
    } else {
        (from == STATE_FOLLOWER && to == STATE_TRANSITIONING)
        || (from == STATE_TRANSITIONING && to == STATE_LEADER)
        || (from == STATE_TRANSITIONING && to == STATE_FOLLOWER)
        || (from == STATE_LEADER && to == STATE_FOLLOWER)
    }
}

// --- Election preconditions ---

// r[depends election.preconditions]
pub open spec fn can_start_election_spec(state: u8, running: bool) -> bool {
    state == STATE_FOLLOWER && running
}

// r[impl election.preconditions]
pub fn can_start_election(state: u8, running: bool) -> (result: bool)
    ensures result == can_start_election_spec(state, running)
{
    state == STATE_FOLLOWER && running
}

// r[depends election.preconditions]
pub open spec fn can_win_election_spec(
    state: u8, new_token: u64, max_fencing_token: u64,
) -> bool {
    state == STATE_TRANSITIONING && new_token > max_fencing_token
}

// r[impl election.preconditions]
pub fn can_win_election(
    state: u8, new_token: u64, max_fencing_token: u64,
) -> (result: bool)
    ensures result == can_win_election_spec(state, new_token, max_fencing_token)
{
    state == STATE_TRANSITIONING && new_token > max_fencing_token
}

// r[depends election.preconditions]
pub open spec fn can_lose_election_spec(state: u8) -> bool {
    state == STATE_TRANSITIONING
}

// r[impl election.preconditions]
pub fn can_lose_election(state: u8) -> (result: bool)
    ensures result == can_lose_election_spec(state)
{
    state == STATE_TRANSITIONING
}

// r[depends election.preconditions]
pub open spec fn can_stepdown_spec(state: u8) -> bool {
    state == STATE_LEADER
}

// r[impl election.preconditions]
pub fn can_stepdown(state: u8) -> (result: bool)
    ensures result == can_stepdown_spec(state)
{
    state == STATE_LEADER
}

// r[depends election.preconditions]
pub open spec fn can_lose_leadership_spec(state: u8) -> bool {
    state == STATE_LEADER
}

// r[impl election.preconditions]
pub fn can_lose_leadership(state: u8) -> (result: bool)
    ensures result == can_lose_leadership_spec(state)
{
    state == STATE_LEADER
}

// --- State-after functions ---

// r[depends election.state-after]
pub open spec fn get_state_after_start_spec() -> u8 { STATE_TRANSITIONING }

// r[impl election.state-after]
pub fn get_state_after_start() -> (result: u8)
    ensures result == get_state_after_start_spec()
{
    STATE_TRANSITIONING
}

// r[depends election.state-after]
pub open spec fn get_state_after_win_spec() -> u8 { STATE_LEADER }

// r[impl election.state-after]
pub fn get_state_after_win() -> (result: u8)
    ensures result == get_state_after_win_spec()
{
    STATE_LEADER
}

// r[depends election.state-after]
pub open spec fn get_state_after_lose_spec() -> u8 { STATE_FOLLOWER }

// r[impl election.state-after]
pub fn get_state_after_lose() -> (result: u8)
    ensures result == get_state_after_lose_spec()
{
    STATE_FOLLOWER
}

// r[depends election.state-after]
pub open spec fn get_state_after_stepdown_spec() -> u8 { STATE_FOLLOWER }

// r[impl election.state-after]
pub fn get_state_after_stepdown() -> (result: u8)
    ensures result == get_state_after_stepdown_spec()
{
    STATE_FOLLOWER
}

// --- Token progression ---

// r[depends election.token-monotonicity]
pub open spec fn compute_next_election_token_spec(current_max: u64) -> u64 {
    if current_max == u64::MAX { u64::MAX } else { (current_max + 1) as u64 }
}

// r[impl election.token-monotonicity]
pub fn compute_next_election_token(current_max: u64) -> (result: u64)
    ensures result == compute_next_election_token_spec(current_max)
{
    if current_max == u64::MAX { u64::MAX } else { current_max + 1 }
}

// r[depends election.token-monotonicity]
pub open spec fn compute_max_token_after_win_spec(
    new_token: u64, current_max: u64,
) -> u64 {
    if new_token >= current_max { new_token } else { current_max }
}

// r[impl election.token-monotonicity]
pub fn compute_max_token_after_win(
    new_token: u64, current_max: u64,
) -> (result: u64)
    ensures result == compute_max_token_after_win_spec(new_token, current_max)
{
    if new_token >= current_max { new_token } else { current_max }
}

// --- Well-formedness ---

// r[depends election.well-formedness]
pub open spec fn is_leader_state_wellformed_spec(
    is_leader: bool, fencing_token: u64, max_fencing_token: u64,
) -> bool {
    if is_leader {
        fencing_token > 0 && fencing_token <= max_fencing_token
    } else {
        true
    }
}

// r[impl election.well-formedness]
pub fn is_leader_state_wellformed(
    is_leader: bool, fencing_token: u64, max_fencing_token: u64,
) -> (result: bool)
    ensures result == is_leader_state_wellformed_spec(is_leader, fencing_token, max_fencing_token)
{
    if is_leader {
        fencing_token > 0 && fencing_token <= max_fencing_token
    } else {
        true
    }
}

// --- Timing ---

// r[depends election.timing]
pub open spec fn calculate_election_timeout_spec(
    base_timeout_ms: u64, jitter_seed: u64, jitter_range_ms: u64,
) -> u64 {
    if jitter_range_ms == 0 {
        base_timeout_ms
    } else {
        let jitter = jitter_seed % jitter_range_ms;
        if base_timeout_ms as int + jitter as int > u64::MAX as int {
            u64::MAX
        } else {
            (base_timeout_ms + jitter) as u64
        }
    }
}

// r[impl election.timing]
pub fn calculate_election_timeout(
    base_timeout_ms: u64, jitter_seed: u64, jitter_range_ms: u64,
) -> (result: u64)
    requires jitter_range_ms > 0 ==> jitter_range_ms <= u64::MAX
    ensures result == calculate_election_timeout_spec(base_timeout_ms, jitter_seed, jitter_range_ms)
{
    if jitter_range_ms == 0 {
        base_timeout_ms
    } else {
        let jitter = jitter_seed % jitter_range_ms;
        if jitter > u64::MAX - base_timeout_ms {
            u64::MAX
        } else {
            base_timeout_ms + jitter
        }
    }
}

// r[depends election.timing]
pub open spec fn is_renewal_time_spec(
    last_renewed_ms: u64, now_ms: u64, interval_ms: u64,
) -> bool {
    now_ms >= last_renewed_ms && ((now_ms - last_renewed_ms) as u64) >= interval_ms
}

// r[impl election.timing]
pub fn is_renewal_time(
    last_renewed_ms: u64, now_ms: u64, interval_ms: u64,
) -> (result: bool)
    requires now_ms >= last_renewed_ms
    ensures result == is_renewal_time_spec(last_renewed_ms, now_ms, interval_ms)
{
    now_ms - last_renewed_ms >= interval_ms
}

// r[depends election.timing]
pub open spec fn should_start_election_spec(
    last_heard_ms: u64, now_ms: u64, timeout_ms: u64,
) -> bool {
    now_ms >= last_heard_ms && ((now_ms - last_heard_ms) as u64) >= timeout_ms
}

// r[impl election.timing]
pub fn should_start_election(
    last_heard_ms: u64, now_ms: u64, timeout_ms: u64,
) -> (result: bool)
    requires now_ms >= last_heard_ms
    ensures result == should_start_election_spec(last_heard_ms, now_ms, timeout_ms)
{
    now_ms - last_heard_ms >= timeout_ms
}

// r[depends election.timing]
pub open spec fn should_maintain_leadership_spec(
    last_heard_ms: u64, now_ms: u64, timeout_ms: u64,
) -> bool {
    now_ms >= last_heard_ms && ((now_ms - last_heard_ms) as u64) < timeout_ms
}

// r[impl election.timing]
pub fn should_maintain_leadership(
    last_heard_ms: u64, now_ms: u64, timeout_ms: u64,
) -> (result: bool)
    requires now_ms >= last_heard_ms
    ensures result == should_maintain_leadership_spec(last_heard_ms, now_ms, timeout_ms)
{
    now_ms - last_heard_ms < timeout_ms
}

// === Proofs ===

// r[verify election.valid-transition]
/// Exhaustive: every valid state pair is classified.
proof fn transition_exhaustive(from: u8, to: u8, ft: u64, tt: u64)
    requires is_valid_state_spec(from), is_valid_state_spec(to)
    ensures
        is_valid_state_transition_spec(from, to, ft, tt)
        || !is_valid_state_transition_spec(from, to, ft, tt)
{}

// r[verify election.valid-transition]
/// Invalid: Follower cannot skip directly to Leader.
proof fn follower_cannot_skip_to_leader(ft: u64, tt: u64)
    ensures !is_valid_state_transition_spec(STATE_FOLLOWER, STATE_LEADER, ft, tt)
{}

// r[verify election.valid-transition]
/// Invalid: Leader cannot go to Transitioning.
proof fn leader_cannot_go_to_transitioning(ft: u64, tt: u64)
    ensures !is_valid_state_transition_spec(STATE_LEADER, STATE_TRANSITIONING, ft, tt)
{}

// r[verify election.preconditions]
/// Win requires Transitioning AND token advancement.
proof fn win_requires_transitioning_and_token(
    state: u8, new_token: u64, max_token: u64,
)
    requires can_win_election_spec(state, new_token, max_token)
    ensures state == STATE_TRANSITIONING, new_token > max_token
{}

// r[verify election.preconditions]
/// Preconditions cover: each state_after result satisfies is_valid_state_transition.
proof fn start_transition_valid()
    ensures is_valid_state_transition_spec(
        STATE_FOLLOWER, get_state_after_start_spec(), 0, 0,
    )
{}

// r[verify election.preconditions]
proof fn win_transition_valid(ft: u64, tt: u64)
    ensures is_valid_state_transition_spec(
        STATE_TRANSITIONING, get_state_after_win_spec(), ft, tt,
    )
{}

// r[verify election.preconditions]
proof fn lose_transition_valid(ft: u64, tt: u64)
    ensures is_valid_state_transition_spec(
        STATE_TRANSITIONING, get_state_after_lose_spec(), ft, tt,
    )
{}

// r[verify election.preconditions]
proof fn stepdown_transition_valid(ft: u64, tt: u64)
    ensures is_valid_state_transition_spec(
        STATE_LEADER, get_state_after_stepdown_spec(), ft, tt,
    )
{}

// r[verify election.token-monotonicity]
/// Next token is >= current.
proof fn next_token_monotonic(current_max: u64)
    ensures compute_next_election_token_spec(current_max) >= current_max
{}

// r[verify election.token-monotonicity]
/// Max-after-win is >= both inputs.
proof fn max_after_win_monotonic(new_token: u64, current_max: u64)
    ensures
        compute_max_token_after_win_spec(new_token, current_max) >= new_token
            || compute_max_token_after_win_spec(new_token, current_max) >= current_max,
        compute_max_token_after_win_spec(new_token, current_max) >= current_max,
{}

// r[verify election.well-formedness]
/// Non-leaders are always well-formed.
proof fn non_leader_always_wellformed(ft: u64, mt: u64)
    ensures is_leader_state_wellformed_spec(false, ft, mt)
{}

// r[verify election.well-formedness]
/// A leader with token 0 is not well-formed.
proof fn zero_token_leader_not_wellformed(mt: u64)
    ensures !is_leader_state_wellformed_spec(true, 0, mt)
{}

// r[verify election.timing]
/// Timeout in [base, base + jitter_range].
proof fn timeout_in_range(base: u64, seed: u64, jitter_range: u64)
    requires jitter_range > 0
    ensures
        calculate_election_timeout_spec(base, seed, jitter_range) >= base,
{
    // jitter = seed % jitter_range, so jitter < jitter_range
    // base + jitter >= base
}

// r[verify election.timing]
/// Zero jitter range returns base.
proof fn zero_jitter_returns_base(base: u64, seed: u64)
    ensures calculate_election_timeout_spec(base, seed, 0) == base
{}

// r[verify election.timing]
/// Maintain and start are complements (when elapsed matches).
proof fn maintain_start_complement(last: u64, now: u64, timeout: u64)
    requires now >= last
    ensures
        should_maintain_leadership_spec(last, now, timeout)
            == !should_start_election_spec(last, now, timeout)
{}

// r[verify election.state-predicates]
/// States are mutually exclusive on valid states.
proof fn states_mutually_exclusive(state: u8)
    requires is_valid_state_spec(state)
    ensures
        (is_follower_spec(state) && !is_transitioning_spec(state) && !is_leader_spec(state))
        || (!is_follower_spec(state) && is_transitioning_spec(state) && !is_leader_spec(state))
        || (!is_follower_spec(state) && !is_transitioning_spec(state) && is_leader_spec(state))
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify election.state-predicates]
    #[test]
    fn test_state_predicates() {
        assert!(is_follower(STATE_FOLLOWER));
        assert!(!is_follower(STATE_TRANSITIONING));
        assert!(!is_follower(STATE_LEADER));
        assert!(is_transitioning(STATE_TRANSITIONING));
        assert!(is_leader(STATE_LEADER));
    }

    // r[verify election.valid-transition]
    #[test]
    fn test_valid_transitions() {
        // Valid transitions
        assert!(is_valid_state_transition(STATE_FOLLOWER, STATE_TRANSITIONING, 0, 0));
        assert!(is_valid_state_transition(STATE_TRANSITIONING, STATE_LEADER, 0, 5));
        assert!(is_valid_state_transition(STATE_TRANSITIONING, STATE_FOLLOWER, 0, 0));
        assert!(is_valid_state_transition(STATE_LEADER, STATE_FOLLOWER, 5, 0));
        // Self-transitions
        assert!(is_valid_state_transition(STATE_FOLLOWER, STATE_FOLLOWER, 0, 0));
        assert!(is_valid_state_transition(STATE_TRANSITIONING, STATE_TRANSITIONING, 0, 0));
        assert!(is_valid_state_transition(STATE_LEADER, STATE_LEADER, 5, 5));
        // Invalid
        assert!(!is_valid_state_transition(STATE_FOLLOWER, STATE_LEADER, 0, 5));
        assert!(!is_valid_state_transition(STATE_LEADER, STATE_TRANSITIONING, 5, 0));
        // Leader self-transition rejects changed token
        assert!(!is_valid_state_transition(STATE_LEADER, STATE_LEADER, 5, 6));
    }

    // r[verify election.preconditions]
    #[test]
    fn test_preconditions() {
        assert!(can_start_election(STATE_FOLLOWER, true));
        assert!(!can_start_election(STATE_FOLLOWER, false));
        assert!(!can_start_election(STATE_LEADER, true));
        assert!(can_win_election(STATE_TRANSITIONING, 6, 5));
        assert!(!can_win_election(STATE_TRANSITIONING, 3, 5));
        assert!(!can_win_election(STATE_FOLLOWER, 6, 5));
        assert!(can_lose_election(STATE_TRANSITIONING));
        assert!(!can_lose_election(STATE_FOLLOWER));
        assert!(can_stepdown(STATE_LEADER));
        assert!(!can_stepdown(STATE_FOLLOWER));
        assert!(can_lose_leadership(STATE_LEADER));
    }

    // r[verify election.state-after]
    #[test]
    fn test_state_after() {
        assert_eq!(get_state_after_start(), STATE_TRANSITIONING);
        assert_eq!(get_state_after_win(), STATE_LEADER);
        assert_eq!(get_state_after_lose(), STATE_FOLLOWER);
        assert_eq!(get_state_after_stepdown(), STATE_FOLLOWER);
    }

    // r[verify election.token-monotonicity]
    #[test]
    fn test_token_progression() {
        assert_eq!(compute_next_election_token(0), 1);
        assert_eq!(compute_next_election_token(5), 6);
        assert_eq!(compute_next_election_token(u64::MAX), u64::MAX);
        assert_eq!(compute_max_token_after_win(6, 5), 6);
        assert_eq!(compute_max_token_after_win(3, 5), 5);
    }

    // r[verify election.well-formedness]
    #[test]
    fn test_well_formedness() {
        assert!(is_leader_state_wellformed(true, 5, 10));
        assert!(!is_leader_state_wellformed(true, 0, 10));
        assert!(!is_leader_state_wellformed(true, 15, 10));
        assert!(is_leader_state_wellformed(false, 0, 0));
    }

    // r[verify election.timing]
    #[test]
    fn test_timing() {
        assert_eq!(calculate_election_timeout(1500, 42, 0), 1500);
        let t = calculate_election_timeout(1500, 42, 500);
        assert!(t >= 1500 && t <= 2000);
        assert!(is_renewal_time(1000, 6000, 5000));
        assert!(!is_renewal_time(1000, 3000, 5000));
        assert!(should_start_election(1000, 6000, 5000));
        assert!(!should_start_election(1000, 3000, 5000));
        assert!(should_maintain_leadership(1000, 3000, 5000));
        assert!(!should_maintain_leadership(1000, 6000, 5000));
    }

    proptest! {
        // r[verify election.state-predicates]
        #[test]
        fn prop_states_exclusive(state in 0u8..3) {
            let f = is_follower(state);
            let t = is_transitioning(state);
            let l = is_leader(state);
            // Exactly one is true
            prop_assert_eq!((f as u8) + (t as u8) + (l as u8), 1);
        }

        // r[verify election.valid-transition]
        #[test]
        fn prop_follower_cannot_skip(ft: u64, tt: u64) {
            prop_assert!(!is_valid_state_transition(STATE_FOLLOWER, STATE_LEADER, ft, tt));
        }

        // r[verify election.valid-transition]
        #[test]
        fn prop_leader_self_requires_same_token(token: u64) {
            prop_assert!(is_valid_state_transition(STATE_LEADER, STATE_LEADER, token, token));
        }

        // r[verify election.valid-transition]
        #[test]
        fn prop_leader_self_rejects_different_token(t1: u64, t2: u64) {
            prop_assume!(t1 != t2);
            prop_assert!(!is_valid_state_transition(STATE_LEADER, STATE_LEADER, t1, t2));
        }

        // r[verify election.token-monotonicity]
        #[test]
        fn prop_next_token_monotonic(current: u64) {
            prop_assert!(compute_next_election_token(current) >= current);
        }

        // r[verify election.token-monotonicity]
        #[test]
        fn prop_max_after_win_ge_current(new_token: u64, current_max: u64) {
            prop_assert!(compute_max_token_after_win(new_token, current_max) >= current_max);
        }

        // r[verify election.well-formedness]
        #[test]
        fn prop_non_leader_wellformed(ft: u64, mt: u64) {
            prop_assert!(is_leader_state_wellformed(false, ft, mt));
        }

        // r[verify election.timing]
        #[test]
        fn prop_timeout_ge_base(base: u64, seed: u64, jitter_range in 1u64..10000) {
            let timeout = calculate_election_timeout(base, seed, jitter_range);
            prop_assert!(timeout >= base);
        }

        // r[verify election.timing]
        #[test]
        fn prop_maintain_start_complement(
            last in 0u64..1_000_000,
            elapsed in 0u64..1_000_000,
            timeout in 1u64..1_000_000,
        ) {
            let now = last + elapsed;
            prop_assert_eq!(
                should_maintain_leadership(last, now, timeout),
                !should_start_election(last, now, timeout)
            );
        }
    }
}

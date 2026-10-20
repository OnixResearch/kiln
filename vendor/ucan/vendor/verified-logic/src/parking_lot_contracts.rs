//! Extension contracts for parking-lot-style synchronization models.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffDecision {
    Unlocked,
    DirectHandoff { waiter: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimedWaitOutcome {
    Notified,
    TimedOut,
    StillParked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReentrantMutexState {
    pub owner: Option<u64>,
    pub recursion: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnceState {
    New,
    Running { initializer: u64 },
    Complete,
    Poisoned,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawLockContract {
    pub mutual_exclusion: bool,
    pub unlock_requires_owner: bool,
    pub waiters_accounted: bool,
    pub timed_wait_distinguishes_timeout: bool,
    pub fair_unlock_handoff: bool,
    pub downgrade_preserves_read: bool,
    pub upgrade_requires_unique_upgradable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitForEdge {
    pub waiter: u64,
    pub blocker: u64,
}

// r[impl domain-primitives.parking-lot-fairness.eventual-handoff]
pub fn fair_unlock_decision(waiters: &[u64], fair_unlock: bool) -> HandoffDecision {
    if fair_unlock {
        waiters
            .first()
            .copied()
            .map_or(HandoffDecision::Unlocked, |waiter| HandoffDecision::DirectHandoff { waiter })
    } else {
        HandoffDecision::Unlocked
    }
}

// r[impl domain-primitives.parking-lot-fairness.eventual-handoff]
pub fn handoff_serves_oldest_waiter(waiters: &[u64], decision: HandoffDecision) -> bool {
    match decision {
        HandoffDecision::Unlocked => true,
        HandoffDecision::DirectHandoff { waiter } => waiters.first().copied() == Some(waiter),
    }
}

// r[verify domain-primitives.parking-lot-fairness.eventual-handoff]
// r[impl domain-primitives.parking-lot-fairness.eventual-handoff]
pub fn fair_nonempty_unlock_hands_off(waiters: &[u64]) -> bool {
    if waiters.is_empty() {
        true
    } else {
        matches!(fair_unlock_decision(waiters, true), HandoffDecision::DirectHandoff { waiter } if waiter == waiters[0])
    }
}

// r[impl domain-primitives.parking-lot-timeout.timed-wait]
pub fn timed_wait_outcome(notified: bool, deadline_reached: bool, was_registered: bool) -> TimedWaitOutcome {
    if notified {
        TimedWaitOutcome::Notified
    } else if deadline_reached {
        TimedWaitOutcome::TimedOut
    } else if was_registered {
        TimedWaitOutcome::StillParked
    } else {
        TimedWaitOutcome::TimedOut
    }
}

// r[impl domain-primitives.parking-lot-timeout.timed-wait]
pub fn timeout_admission_valid(was_registered: bool, outcome: TimedWaitOutcome) -> bool {
    was_registered || outcome == TimedWaitOutcome::TimedOut
}

// r[verify domain-primitives.parking-lot-timeout.timed-wait]
// r[impl domain-primitives.parking-lot-timeout.timed-wait]
pub fn notified_beats_timeout_race() -> bool {
    timed_wait_outcome(true, true, true) == TimedWaitOutcome::Notified
}

// r[impl domain-primitives.parking-lot-reentrant-mutex.owner-count]
pub fn reentrant_lock(state: ReentrantMutexState, caller: u64) -> Option<ReentrantMutexState> {
    match state.owner {
        None => Some(ReentrantMutexState {
            owner: Some(caller),
            recursion: 1,
        }),
        Some(owner) if owner == caller => state.recursion.checked_add(1).map(|recursion| ReentrantMutexState {
            owner: Some(caller),
            recursion,
        }),
        Some(_) => None,
    }
}

// r[impl domain-primitives.parking-lot-reentrant-mutex.owner-count]
pub fn reentrant_unlock(state: ReentrantMutexState, caller: u64) -> Option<ReentrantMutexState> {
    if state.owner != Some(caller) || state.recursion == 0 {
        return None;
    }
    if state.recursion == 1 {
        Some(ReentrantMutexState {
            owner: None,
            recursion: 0,
        })
    } else {
        Some(ReentrantMutexState {
            owner: Some(caller),
            recursion: state.recursion - 1,
        })
    }
}

// r[verify domain-primitives.parking-lot-reentrant-mutex.owner-count]
// r[impl domain-primitives.parking-lot-reentrant-mutex.owner-count]
pub fn reentrant_other_thread_cannot_enter(owner: u64, other: u64) -> bool {
    owner == other
        || reentrant_lock(
            ReentrantMutexState {
                owner: Some(owner),
                recursion: 1,
            },
            other,
        )
        .is_none()
}

// r[impl domain-primitives.parking-lot-once.exactly-once]
pub fn once_start(state: OnceState, initializer: u64) -> Option<OnceState> {
    match state {
        OnceState::New | OnceState::Poisoned => Some(OnceState::Running { initializer }),
        OnceState::Running { .. } | OnceState::Complete => None,
    }
}

// r[impl domain-primitives.parking-lot-once.exactly-once]
pub fn once_finish(state: OnceState, initializer: u64, success: bool) -> Option<OnceState> {
    match state {
        OnceState::Running { initializer: running } if running == initializer && success => Some(OnceState::Complete),
        OnceState::Running { initializer: running } if running == initializer => Some(OnceState::Poisoned),
        _ => None,
    }
}

// r[impl domain-primitives.parking-lot-once.exactly-once]
pub fn once_observe_initialized(state: OnceState) -> bool {
    state == OnceState::Complete
}

// r[verify domain-primitives.parking-lot-once.exactly-once]
// r[impl domain-primitives.parking-lot-once.exactly-once]
pub fn once_complete_is_terminal(initializer: u64) -> bool {
    once_start(OnceState::Complete, initializer).is_none()
        && once_finish(OnceState::Complete, initializer, true).is_none()
}

// r[impl domain-primitives.parking-lot-raw-lock-contracts.contracts]
pub fn raw_lock_contract_valid(contract: RawLockContract) -> bool {
    contract.mutual_exclusion
        && contract.unlock_requires_owner
        && contract.waiters_accounted
        && contract.timed_wait_distinguishes_timeout
        && contract.fair_unlock_handoff
        && contract.downgrade_preserves_read
        && contract.upgrade_requires_unique_upgradable
}

// r[verify domain-primitives.parking-lot-raw-lock-contracts.contracts]
// r[impl domain-primitives.parking-lot-raw-lock-contracts.contracts]
pub fn canonical_raw_lock_contract_valid() -> bool {
    raw_lock_contract_valid(RawLockContract {
        mutual_exclusion: true,
        unlock_requires_owner: true,
        waiters_accounted: true,
        timed_wait_distinguishes_timeout: true,
        fair_unlock_handoff: true,
        downgrade_preserves_read: true,
        upgrade_requires_unique_upgradable: true,
    })
}

// r[impl domain-primitives.parking-lot-deadlock-detector.cycle-soundness]
pub fn wait_for_has_edge(edges: &[WaitForEdge], waiter: u64, blocker: u64) -> bool {
    edges.iter().any(|edge| edge.waiter == waiter && edge.blocker == blocker)
}

// r[impl domain-primitives.parking-lot-deadlock-detector.cycle-soundness]
pub fn wait_for_reported_cycle_sound(edges: &[WaitForEdge], cycle: &[u64]) -> bool {
    if cycle.len() < 2 {
        return false;
    }
    cycle.windows(2).all(|pair| wait_for_has_edge(edges, pair[0], pair[1]))
        && wait_for_has_edge(edges, cycle[cycle.len() - 1], cycle[0])
}

// r[impl domain-primitives.parking-lot-deadlock-detector.cycle-soundness]
pub fn wait_for_contains_two_thread_cycle(edges: &[WaitForEdge], a: u64, b: u64) -> bool {
    a != b && wait_for_has_edge(edges, a, b) && wait_for_has_edge(edges, b, a)
}

// r[verify domain-primitives.parking-lot-deadlock-detector.cycle-soundness]
// r[impl domain-primitives.parking-lot-deadlock-detector.cycle-soundness]
pub fn detected_two_thread_cycle_is_sound(edges: &[WaitForEdge], a: u64, b: u64) -> bool {
    !wait_for_contains_two_thread_cycle(edges, a, b) || wait_for_reported_cycle_sound(edges, &[a, b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fair_handoff_and_timeout_are_deterministic() {
        let waiters = [10, 11];
        let decision = fair_unlock_decision(&waiters, true);
        assert_eq!(decision, HandoffDecision::DirectHandoff { waiter: 10 });
        assert!(handoff_serves_oldest_waiter(&waiters, decision));
        assert_eq!(timed_wait_outcome(true, true, true), TimedWaitOutcome::Notified);
        assert_eq!(timed_wait_outcome(false, true, true), TimedWaitOutcome::TimedOut);
    }

    #[test]
    fn reentrant_owner_count_controls_entry_and_release() {
        let locked = reentrant_lock(
            ReentrantMutexState {
                owner: None,
                recursion: 0,
            },
            7,
        )
        .unwrap();
        let relocked = reentrant_lock(locked, 7).unwrap();
        assert_eq!(relocked.recursion, 2);
        assert!(reentrant_lock(relocked, 8).is_none());
        let one_left = reentrant_unlock(relocked, 7).unwrap();
        assert_eq!(one_left.recursion, 1);
        let unlocked = reentrant_unlock(one_left, 7).unwrap();
        assert_eq!(unlocked.owner, None);
    }

    #[test]
    fn once_is_exactly_once_after_completion() {
        let running = once_start(OnceState::New, 3).unwrap();
        assert_eq!(running, OnceState::Running { initializer: 3 });
        assert_eq!(once_finish(running, 3, true), Some(OnceState::Complete));
        assert!(once_observe_initialized(OnceState::Complete));
        assert!(once_complete_is_terminal(3));
    }

    #[test]
    fn raw_lock_contract_and_deadlock_cycle_soundness() {
        assert!(canonical_raw_lock_contract_valid());
        let edges = [WaitForEdge { waiter: 1, blocker: 2 }, WaitForEdge {
            waiter: 2,
            blocker: 1,
        }];
        assert!(wait_for_contains_two_thread_cycle(&edges, 1, 2));
        assert!(wait_for_reported_cycle_sound(&edges, &[1, 2]));
        assert!(!wait_for_reported_cycle_sound(&edges, &[1, 3]));
    }
}

//! Finite parking-lot condition-variable requeue model.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondvarWakeKind {
    Notified,
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CondvarState {
    pub associated_mutex: Option<u64>,
    pub waiter_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CondvarNotifyResult {
    pub woken: u64,
    pub requeued: u64,
    pub remaining_condvar_waiters: u64,
    pub associated_mutex: Option<u64>,
}

// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_can_wait(state: CondvarState, mutex_key: u64) -> bool {
    state.waiter_count == 0 || state.associated_mutex == Some(mutex_key)
}

// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_wait(state: CondvarState, mutex_key: u64) -> Option<CondvarState> {
    if condvar_can_wait(state, mutex_key) {
        Some(CondvarState {
            associated_mutex: Some(mutex_key),
            waiter_count: state.waiter_count.saturating_add(1),
        })
    } else {
        None
    }
}

// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_notify_one(state: CondvarState) -> CondvarNotifyResult {
    let woken = u64::from(state.waiter_count > 0);
    let remaining = state.waiter_count.saturating_sub(woken);
    CondvarNotifyResult {
        woken,
        requeued: 0,
        remaining_condvar_waiters: remaining,
        associated_mutex: if remaining == 0 { None } else { state.associated_mutex },
    }
}

// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_notify_all_requeue(state: CondvarState) -> CondvarNotifyResult {
    let woken = u64::from(state.waiter_count > 0);
    let requeued = state.waiter_count.saturating_sub(woken);
    CondvarNotifyResult {
        woken,
        requeued,
        remaining_condvar_waiters: 0,
        associated_mutex: None,
    }
}

// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_wake_valid(kind: CondvarWakeKind, notify_event: bool, deadline_reached: bool) -> bool {
    match kind {
        CondvarWakeKind::Notified => notify_event,
        CondvarWakeKind::TimedOut => deadline_reached,
    }
}

// r[verify domain-primitives.parking-lot-condvar.requeue-no-spurious]
// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_notify_all_accounts_for_all(state: CondvarState) -> bool {
    let result = condvar_notify_all_requeue(state);
    result.woken + result.requeued == state.waiter_count && result.remaining_condvar_waiters == 0
}

// r[verify domain-primitives.parking-lot-condvar.requeue-no-spurious]
// r[impl domain-primitives.parking-lot-condvar.requeue-no-spurious]
pub fn condvar_non_timeout_requires_notify() -> bool {
    !condvar_wake_valid(CondvarWakeKind::Notified, false, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condvar_single_mutex_association() {
        let state = CondvarState {
            associated_mutex: Some(7),
            waiter_count: 1,
        };
        assert!(condvar_can_wait(state, 7));
        assert!(!condvar_can_wait(state, 8));
    }

    #[test]
    fn notify_all_requeues_rest_without_spurious_wake() {
        let state = CondvarState {
            associated_mutex: Some(7),
            waiter_count: 3,
        };
        let result = condvar_notify_all_requeue(state);
        assert_eq!(result.woken, 1);
        assert_eq!(result.requeued, 2);
        assert!(condvar_notify_all_accounts_for_all(state));
        assert!(!condvar_wake_valid(CondvarWakeKind::Notified, false, false));
    }
}

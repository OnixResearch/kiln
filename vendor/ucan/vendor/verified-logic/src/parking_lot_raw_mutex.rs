//! Finite parking-lot raw mutex state machine.

use crate::parking_lot_core_model::ParkingLotTable;
use crate::parking_lot_core_model::has_waiter;

pub const RAW_MUTEX_LOCKED_BIT: u8 = 0b01;
pub const RAW_MUTEX_PARKED_BIT: u8 = 0b10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawMutexStep {
    AcquiredFast,
    Queued,
    Unlocked,
    Handoff,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawMutexState {
    pub bits: u8,
    pub owner: Option<u64>,
}

impl RawMutexState {
    pub const fn unlocked() -> Self {
        Self { bits: 0, owner: None }
    }
}

// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_is_locked(state: RawMutexState) -> bool {
    state.bits & RAW_MUTEX_LOCKED_BIT != 0
}

// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_has_parked_bit(state: RawMutexState) -> bool {
    state.bits & RAW_MUTEX_PARKED_BIT != 0
}

// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_state_valid(state: RawMutexState, table: &ParkingLotTable, key: u64) -> bool {
    let owner_matches = raw_mutex_is_locked(state) == state.owner.is_some();
    let waiter_present = table.queues.iter().any(|queue| queue.key == key && !queue.waiters.is_empty());
    owner_matches && (!waiter_present || raw_mutex_has_parked_bit(state))
}

// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_lock(state: RawMutexState, thread_id: u64) -> (RawMutexStep, RawMutexState) {
    if !raw_mutex_is_locked(state) {
        (RawMutexStep::AcquiredFast, RawMutexState {
            bits: RAW_MUTEX_LOCKED_BIT,
            owner: Some(thread_id),
        })
    } else {
        (RawMutexStep::Queued, RawMutexState {
            bits: state.bits | RAW_MUTEX_PARKED_BIT,
            owner: state.owner,
        })
    }
}

// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_unlock(state: RawMutexState, table: &ParkingLotTable, key: u64) -> (RawMutexStep, RawMutexState) {
    if !raw_mutex_is_locked(state) {
        return (RawMutexStep::Rejected, state);
    }
    let waiter_present = table.queues.iter().any(|queue| queue.key == key && !queue.waiters.is_empty());
    if waiter_present {
        (RawMutexStep::Handoff, RawMutexState {
            bits: RAW_MUTEX_LOCKED_BIT | RAW_MUTEX_PARKED_BIT,
            owner: None,
        })
    } else {
        (RawMutexStep::Unlocked, RawMutexState::unlocked())
    }
}

// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_waiter_accounted(
    state: RawMutexState,
    table: &ParkingLotTable,
    key: u64,
    waiter: u64,
    explicitly_removed: bool,
) -> bool {
    !has_waiter(table, waiter)
        || raw_mutex_has_parked_bit(state)
        || explicitly_removed
        || table.queues.iter().all(|queue| queue.key != key)
}

// r[verify domain-primitives.parking-lot-raw-mutex.state-machine]
// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_fast_path_locks(thread_id: u64) -> bool {
    let (step, state) = raw_mutex_lock(RawMutexState::unlocked(), thread_id);
    step == RawMutexStep::AcquiredFast && raw_mutex_is_locked(state) && state.owner == Some(thread_id)
}

// r[verify domain-primitives.parking-lot-raw-mutex.state-machine]
// r[impl domain-primitives.parking-lot-raw-mutex.state-machine]
pub fn raw_mutex_queued_sets_parked(holder: u64, waiter: u64) -> bool {
    let state = RawMutexState {
        bits: RAW_MUTEX_LOCKED_BIT,
        owner: Some(holder),
    };
    let (step, next) = raw_mutex_lock(state, waiter);
    step == RawMutexStep::Queued && raw_mutex_has_parked_bit(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parking_lot_core_model::ParkingLotTable;
    use crate::parking_lot_core_model::park_waiter;

    #[test]
    fn fast_lock_and_empty_unlock() {
        let empty = ParkingLotTable { queues: Vec::new() };
        let (step, state) = raw_mutex_lock(RawMutexState::unlocked(), 1);
        assert_eq!(step, RawMutexStep::AcquiredFast);
        assert!(raw_mutex_state_valid(state, &empty, 5));
        let (unlock_step, unlocked) = raw_mutex_unlock(state, &empty, 5);
        assert_eq!(unlock_step, RawMutexStep::Unlocked);
        assert_eq!(unlocked, RawMutexState::unlocked());
    }

    #[test]
    fn waiters_require_parked_bit() {
        let table = park_waiter(&ParkingLotTable { queues: Vec::new() }, 5, 2);
        let bad = RawMutexState {
            bits: RAW_MUTEX_LOCKED_BIT,
            owner: Some(1),
        };
        assert!(!raw_mutex_state_valid(bad, &table, 5));
        let (_, parked) = raw_mutex_lock(bad, 2);
        assert!(raw_mutex_state_valid(parked, &table, 5));
    }
}

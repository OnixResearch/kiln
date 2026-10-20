//! Finite packed parking-lot RwLock state model.

pub const RWLOCK_PARKED_BIT: usize = 0b0001;
pub const RWLOCK_WRITER_PARKED_BIT: usize = 0b0010;
pub const RWLOCK_UPGRADABLE_BIT: usize = 0b0100;
pub const RWLOCK_WRITER_BIT: usize = 0b1000;
pub const RWLOCK_ONE_READER: usize = 0b1_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedRwLockState {
    pub bits: usize,
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_reader_count(state: PackedRwLockState) -> usize {
    state.bits / RWLOCK_ONE_READER
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_has_writer(state: PackedRwLockState) -> bool {
    state.bits & RWLOCK_WRITER_BIT != 0
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_has_pending_writer(state: PackedRwLockState) -> bool {
    state.bits & (RWLOCK_WRITER_BIT | RWLOCK_WRITER_PARKED_BIT) != 0
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_has_upgradable(state: PackedRwLockState) -> bool {
    state.bits & RWLOCK_UPGRADABLE_BIT != 0
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_state_valid(state: PackedRwLockState) -> bool {
    let readers = rwlock_reader_count(state);
    let writer = rwlock_has_writer(state);
    let upgradable = rwlock_has_upgradable(state);
    (!writer || readers == 0) && (!upgradable || readers > 0) && !(writer && upgradable)
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_can_read(state: PackedRwLockState, in_read_phase: bool) -> bool {
    rwlock_state_valid(state) && !rwlock_has_writer(state) && (!rwlock_has_pending_writer(state) || in_read_phase)
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_read_acquire(state: PackedRwLockState, in_read_phase: bool) -> Option<PackedRwLockState> {
    if rwlock_can_read(state, in_read_phase) {
        Some(PackedRwLockState {
            bits: state.bits + RWLOCK_ONE_READER,
        })
    } else {
        None
    }
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_can_write(state: PackedRwLockState) -> bool {
    rwlock_state_valid(state) && rwlock_reader_count(state) == 0 && !rwlock_has_writer(state)
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_write_acquire(state: PackedRwLockState) -> Option<PackedRwLockState> {
    if rwlock_can_write(state) {
        Some(PackedRwLockState {
            bits: state.bits | RWLOCK_WRITER_BIT,
        })
    } else {
        None
    }
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_downgrade(state: PackedRwLockState) -> Option<PackedRwLockState> {
    if rwlock_state_valid(state) && rwlock_has_writer(state) && rwlock_reader_count(state) == 0 {
        Some(PackedRwLockState {
            bits: (state.bits & !RWLOCK_WRITER_BIT) + RWLOCK_ONE_READER,
        })
    } else {
        None
    }
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_upgradable_acquire(state: PackedRwLockState) -> Option<PackedRwLockState> {
    if rwlock_can_read(state, true) && !rwlock_has_upgradable(state) {
        Some(PackedRwLockState {
            bits: (state.bits + RWLOCK_ONE_READER) | RWLOCK_UPGRADABLE_BIT,
        })
    } else {
        None
    }
}

// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_upgrade(state: PackedRwLockState) -> Option<PackedRwLockState> {
    if rwlock_state_valid(state) && rwlock_has_upgradable(state) && rwlock_reader_count(state) == 1 {
        Some(PackedRwLockState {
            bits: ((state.bits & !RWLOCK_UPGRADABLE_BIT) - RWLOCK_ONE_READER) | RWLOCK_WRITER_BIT,
        })
    } else {
        None
    }
}

// r[verify domain-primitives.parking-lot-rwlock.packed-fair-state]
// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_downgrade_preserves_validity(state: PackedRwLockState) -> bool {
    rwlock_downgrade(state).is_none_or(rwlock_state_valid)
}

// r[verify domain-primitives.parking-lot-rwlock.packed-fair-state]
// r[impl domain-primitives.parking-lot-rwlock.packed-fair-state]
pub fn rwlock_upgradable_is_unique(state: PackedRwLockState) -> bool {
    if rwlock_has_upgradable(state) {
        rwlock_upgradable_acquire(state).is_none()
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_writer_and_downgrade() {
        let free = PackedRwLockState { bits: 0 };
        let read = rwlock_read_acquire(free, false).unwrap();
        assert_eq!(rwlock_reader_count(read), 1);
        assert!(rwlock_write_acquire(read).is_none());
        let writer = rwlock_write_acquire(free).unwrap();
        let downgraded = rwlock_downgrade(writer).unwrap();
        assert_eq!(rwlock_reader_count(downgraded), 1);
        assert!(!rwlock_has_writer(downgraded));
    }

    #[test]
    fn pending_writer_blocks_readers_and_upgradable_is_unique() {
        let pending = PackedRwLockState {
            bits: RWLOCK_WRITER_PARKED_BIT,
        };
        assert!(rwlock_read_acquire(pending, false).is_none());
        let upg = rwlock_upgradable_acquire(PackedRwLockState { bits: 0 }).unwrap();
        assert!(rwlock_has_upgradable(upg));
        assert!(rwlock_upgradable_acquire(upg).is_none());
    }
}

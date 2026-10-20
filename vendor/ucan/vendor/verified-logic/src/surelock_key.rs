//! Verified surelock key flow.
//!
//! A `MutexKey` carries highest level reached so far. Acquiring a lock or
//! admitted lock group consumes key and returns new key at highest reached
//! level. Entering or leaving subscope preserves level.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::max_spec;
use crate::surelock_id::SureLock;
#[cfg(verus_keep_ghost)]
use crate::surelock_id::lock_level_spec;
#[cfg(verus_keep_ghost)]
use crate::surelock_level::lock_after_spec;

verus! {

#[derive(Copy, Clone, Debug)]
pub struct MutexKey {
    pub level: u64,
}

// r[depends surelock-key-flow.progression]
// r[depends surelock-key-flow.subscope]
pub open spec fn key_level_spec(key: MutexKey) -> u64 {
    key.level
}

// r[impl surelock-key-flow.progression]
// r[impl surelock-key-flow.subscope]
pub fn key_level(key: &MutexKey) -> (result: u64)
    ensures result == key_level_spec(*key)
{
    key.level
}

// r[depends surelock-key-flow.progression]
pub open spec fn admit_lock_spec(key: MutexKey, lock: SureLock) -> bool {
    lock_after_spec(key.level, lock_level_spec(lock))
}

// r[depends surelock-key-flow.progression]
pub open spec fn acquire_lock_spec(key: MutexKey, lock: SureLock) -> Option<MutexKey> {
    if admit_lock_spec(key, lock) {
        Some(MutexKey { level: lock.level })
    } else {
        None
    }
}

// r[impl surelock-key-flow.progression]
// r[impl surelock-key-flow.no-backward]
pub fn acquire_lock(key: &MutexKey, lock: &SureLock) -> (result: Option<MutexKey>)
    ensures result == acquire_lock_spec(*key, *lock)
{
    if key.level <= lock.level {
        Some(MutexKey { level: lock.level })
    } else {
        None
    }
}

pub open spec fn group_max_prefix_spec(base: u64, locks: Seq<SureLock>, upto: nat) -> u64
    recommends upto <= locks.len()
    decreases upto,
{
    if upto == 0 {
        base
    } else {
        max_spec(
            group_max_prefix_spec(base, locks, (upto - 1) as nat),
            locks[upto as int - 1].level,
        )
    }
}

// r[depends surelock-key-flow.progression]
pub open spec fn admit_group_spec(key: MutexKey, locks: Seq<SureLock>) -> bool {
    forall|i: int| #![trigger locks[i]] 0 <= i < locks.len() ==> key.level <= locks[i].level
}

// r[depends surelock-key-flow.progression]
pub open spec fn acquire_group_spec(key: MutexKey, locks: Seq<SureLock>) -> Option<MutexKey> {
    if admit_group_spec(key, locks) {
        Some(MutexKey {
            level: group_max_prefix_spec(key.level, locks, locks.len() as nat),
        })
    } else {
        None
    }
}

// r[impl surelock-key-flow.progression]
// r[impl surelock-key-flow.no-backward]
pub fn acquire_group(key: &MutexKey, locks: &[SureLock]) -> (result: Option<MutexKey>)
    ensures result == acquire_group_spec(*key, locks@)
{
    let mut max_level = key.level;
    let mut i: usize = 0;
    while i < locks.len()
        invariant
            0 <= i <= locks@.len(),
            forall|j: int| #![trigger locks@[j]] 0 <= j < i as int ==> key.level <= locks@[j].level,
            max_level == group_max_prefix_spec(key.level, locks@, i as nat),
        decreases locks.len() - i,
    {
        if locks[i].level < key.level {
            assert(!admit_group_spec(*key, locks@));
            return None;
        }
        if max_level < locks[i].level {
            max_level = locks[i].level;
        }
        i += 1;
    }
    Some(MutexKey { level: max_level })
}

// r[depends surelock-key-flow.subscope]
pub open spec fn subscope_spec(key: MutexKey) -> MutexKey {
    MutexKey { level: key.level }
}

// r[impl surelock-key-flow.subscope]
pub fn subscope(key: &MutexKey) -> (result: MutexKey)
    ensures result == subscope_spec(*key)
{
    MutexKey { level: key.level }
}

// r[verify surelock-key-flow.progression]
pub proof fn acquire_lock_returns_lock_level(key: MutexKey, lock: SureLock)
    requires admit_lock_spec(key, lock),
    ensures acquire_lock_spec(key, lock).unwrap().level == lock.level
{}

// r[verify surelock-key-flow.progression]
pub proof fn acquire_group_returns_max_level(key: MutexKey, locks: Seq<SureLock>)
    requires admit_group_spec(key, locks),
    ensures acquire_group_spec(key, locks).unwrap().level == group_max_prefix_spec(key.level, locks, locks.len() as nat)
{}

// r[verify surelock-key-flow.no-backward]
pub proof fn admitted_group_is_nondecreasing(key: MutexKey, locks: Seq<SureLock>)
    requires admit_group_spec(key, locks),
    ensures acquire_group_spec(key, locks).unwrap().level >= key.level
    decreases locks.len()
{
    if locks.len() > 0 {
        admitted_group_is_nondecreasing(key, locks.subrange(0, locks.len() - 1));
    }
}

// r[verify surelock-key-flow.no-backward]
pub proof fn backward_group_rejected(key: MutexKey, locks: Seq<SureLock>)
    requires exists|i: int| #![trigger locks[i]] 0 <= i < locks.len() && locks[i].level < key.level,
    ensures !admit_group_spec(key, locks)
{}

// r[verify surelock-key-flow.subscope]
pub proof fn subscope_preserves_level(key: MutexKey)
    ensures subscope_spec(key).level == key.level
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::surelock_id::SureLock;

    // r[verify surelock-key-flow.progression]
    #[test]
    fn test_single_lock_progression() {
        let key = MutexKey { level: 0 };
        let lock = SureLock { id: 1, level: 2 };
        assert_eq!(acquire_lock(&key, &lock).unwrap().level, 2);
    }

    // r[verify surelock-key-flow.progression]
    #[test]
    fn test_group_progression() {
        let key = MutexKey { level: 1 };
        let locks = [SureLock { id: 1, level: 1 }, SureLock { id: 2, level: 3 }, SureLock {
            id: 3,
            level: 2,
        }];
        assert_eq!(acquire_group(&key, &locks).unwrap().level, 3);
    }

    // r[verify surelock-key-flow.no-backward]
    #[test]
    fn test_backward_group_rejected() {
        let key = MutexKey { level: 3 };
        let locks = [SureLock { id: 1, level: 1 }];
        assert!(acquire_group(&key, &locks).is_none());
    }

    // r[verify surelock-key-flow.subscope]
    #[test]
    fn test_subscope_preserves_level() {
        let key = MutexKey { level: 2 };
        assert_eq!(subscope(&key).level, 2);
    }

    proptest! {
        // r[verify surelock-key-flow.progression]
        #[test]
        fn prop_single_lock_progression(key_level: u64, extra in 0u64..1000) {
            let key = MutexKey { level: key_level };
            let lock = SureLock { id: 1, level: key_level.saturating_add(extra) };
            prop_assert_eq!(acquire_lock(&key, &lock).unwrap().level, lock.level);
        }

        // r[verify surelock-key-flow.no-backward]
        #[test]
        fn prop_backward_single_lock_rejected(key_level in 1u64..u64::MAX, lock_level in 0u64..u64::MAX) {
            let key = MutexKey { level: key_level };
            let lock = SureLock { id: 7, level: lock_level.min(key_level - 1) };
            prop_assert!(acquire_lock(&key, &lock).is_none());
        }

        // r[verify surelock-key-flow.subscope]
        #[test]
        fn prop_subscope_preserves_level(level: u64) {
            let key = MutexKey { level };
            prop_assert_eq!(subscope(&key).level, level);
        }
    }
}

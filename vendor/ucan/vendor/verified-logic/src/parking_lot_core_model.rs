//! Finite parking-lot core queue model.
//!
//! This module models the queue-table part of `parking_lot_core` with ordinary
//! finite vectors: lock keys map to waiter queues, each waiter identity may
//! appear at most once across the table, and queued waiters must record the key
//! they are parked on.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParkingLotWaiter {
    pub thread_id: u64,
    pub parked_key: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParkingLotQueue {
    pub key: u64,
    pub waiters: Vec<ParkingLotWaiter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParkingLotTable {
    pub queues: Vec<ParkingLotQueue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequeueResult {
    pub woken: Option<ParkingLotWaiter>,
    pub moved: usize,
    pub table: ParkingLotTable,
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn waiter_matches_key(waiter: &ParkingLotWaiter, key: u64) -> bool {
    waiter.parked_key == key
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn waiter_count(table: &ParkingLotTable, thread_id: u64) -> usize {
    let mut count = 0usize;
    for queue in &table.queues {
        for waiter in &queue.waiters {
            if waiter.thread_id == thread_id {
                count += 1;
            }
        }
    }
    count
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn has_waiter(table: &ParkingLotTable, thread_id: u64) -> bool {
    waiter_count(table, thread_id) > 0
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn queue_waiters_match_key(queue: &ParkingLotQueue) -> bool {
    for waiter in &queue.waiters {
        if !waiter_matches_key(waiter, queue.key) {
            return false;
        }
    }
    true
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn queues_have_unique_keys(table: &ParkingLotTable) -> bool {
    for i in 0..table.queues.len() {
        for j in i + 1..table.queues.len() {
            if table.queues[i].key == table.queues[j].key {
                return false;
            }
        }
    }
    true
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn waiters_unique(table: &ParkingLotTable) -> bool {
    for i in 0..table.queues.len() {
        for wi in 0..table.queues[i].waiters.len() {
            let id = table.queues[i].waiters[wi].thread_id;
            let mut seen = 0usize;
            for queue in &table.queues {
                for waiter in &queue.waiters {
                    if waiter.thread_id == id {
                        seen += 1;
                    }
                }
            }
            if seen != 1 {
                return false;
            }
        }
    }
    true
}

// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn table_valid(table: &ParkingLotTable) -> bool {
    if !queues_have_unique_keys(table) || !waiters_unique(table) {
        return false;
    }
    for queue in &table.queues {
        if !queue_waiters_match_key(queue) {
            return false;
        }
    }
    true
}

fn find_queue_index(table: &ParkingLotTable, key: u64) -> Option<usize> {
    table.queues.iter().position(|queue| queue.key == key)
}

fn queue_for_key_mut(table: &mut ParkingLotTable, key: u64) -> &mut ParkingLotQueue {
    if let Some(index) = find_queue_index(table, key) {
        &mut table.queues[index]
    } else {
        table.queues.push(ParkingLotQueue {
            key,
            waiters: Vec::new(),
        });
        let last = table.queues.len() - 1;
        &mut table.queues[last]
    }
}

// r[impl domain-primitives.parking-lot-core.operations]
pub fn park_waiter(table: &ParkingLotTable, key: u64, thread_id: u64) -> ParkingLotTable {
    let mut next = table.clone();
    if has_waiter(&next, thread_id) {
        return next;
    }
    queue_for_key_mut(&mut next, key).waiters.push(ParkingLotWaiter {
        thread_id,
        parked_key: key,
    });
    next
}

// r[impl domain-primitives.parking-lot-core.operations]
pub fn unpark_one(table: &ParkingLotTable, key: u64) -> (Option<ParkingLotWaiter>, ParkingLotTable) {
    let mut next = table.clone();
    let Some(index) = find_queue_index(&next, key) else {
        return (None, next);
    };
    if next.queues[index].waiters.is_empty() {
        return (None, next);
    }
    let waiter = next.queues[index].waiters.remove(0);
    (Some(waiter), next)
}

// r[impl domain-primitives.parking-lot-core.operations]
pub fn unpark_all(table: &ParkingLotTable, key: u64) -> (Vec<ParkingLotWaiter>, ParkingLotTable) {
    let mut next = table.clone();
    let Some(index) = find_queue_index(&next, key) else {
        return (Vec::new(), next);
    };
    let removed = std::mem::take(&mut next.queues[index].waiters);
    (removed, next)
}

// r[impl domain-primitives.parking-lot-core.operations]
pub fn requeue_after_wake(table: &ParkingLotTable, from: u64, to: u64) -> RequeueResult {
    let (woken, after_wake) = unpark_one(table, from);
    let (mut moving, mut next) = unpark_all(&after_wake, from);
    let moved = moving.len();
    if moved > 0 {
        let dest = queue_for_key_mut(&mut next, to);
        for mut waiter in moving.drain(..) {
            waiter.parked_key = to;
            dest.waiters.push(waiter);
        }
    }
    RequeueResult {
        woken,
        moved,
        table: next,
    }
}

// r[impl domain-primitives.parking-lot-core.operations]
pub fn requeue_preserves_waiter_count(before: &ParkingLotTable, after: &RequeueResult) -> bool {
    let before_count: usize = before.queues.iter().map(|queue| queue.waiters.len()).sum();
    let after_count: usize = after.table.queues.iter().map(|queue| queue.waiters.len()).sum();
    let woken_count = usize::from(after.woken.is_some());
    before_count == after_count + woken_count
}

// r[verify domain-primitives.parking-lot-core.queue-validity]
// r[impl domain-primitives.parking-lot-core.queue-validity]
pub fn valid_table_has_unique_waiters(table: &ParkingLotTable) -> bool {
    !table_valid(table) || waiters_unique(table)
}

// r[verify domain-primitives.parking-lot-core.operations]
// r[impl domain-primitives.parking-lot-core.operations]
pub fn park_preserves_validity_when_absent(table: &ParkingLotTable, key: u64, thread_id: u64) -> bool {
    if !table_valid(table) || has_waiter(table, thread_id) {
        true
    } else {
        table_valid(&park_waiter(table, key, thread_id))
    }
}

// r[verify domain-primitives.parking-lot-core.operations]
// r[impl domain-primitives.parking-lot-core.operations]
pub fn requeue_preserves_validity(table: &ParkingLotTable, from: u64, to: u64) -> bool {
    if !table_valid(table) {
        true
    } else {
        let result = requeue_after_wake(table, from, to);
        table_valid(&result.table) && requeue_preserves_waiter_count(table, &result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ParkingLotTable {
        ParkingLotTable {
            queues: vec![ParkingLotQueue {
                key: 7,
                waiters: vec![
                    ParkingLotWaiter {
                        thread_id: 1,
                        parked_key: 7,
                    },
                    ParkingLotWaiter {
                        thread_id: 2,
                        parked_key: 7,
                    },
                ],
            }],
        }
    }

    #[test]
    fn distinct_waiters_are_valid() {
        assert!(table_valid(&table()));
    }

    #[test]
    fn duplicate_waiter_rejected() {
        let mut t = table();
        t.queues.push(ParkingLotQueue {
            key: 8,
            waiters: vec![ParkingLotWaiter {
                thread_id: 1,
                parked_key: 8,
            }],
        });
        assert!(!table_valid(&t));
    }

    #[test]
    fn wrong_key_rejected() {
        let t = ParkingLotTable {
            queues: vec![ParkingLotQueue {
                key: 7,
                waiters: vec![ParkingLotWaiter {
                    thread_id: 1,
                    parked_key: 8,
                }],
            }],
        };
        assert!(!table_valid(&t));
    }

    #[test]
    fn park_empty_unpark_and_requeue_behave() {
        let empty = ParkingLotTable { queues: Vec::new() };
        let parked = park_waiter(&empty, 1, 10);
        assert!(table_valid(&parked));
        assert_eq!(waiter_count(&parked, 10), 1);
        let (none, unchanged) = unpark_one(&parked, 99);
        assert_eq!(none, None);
        assert_eq!(unchanged, parked);

        let result = requeue_after_wake(&table(), 7, 9);
        assert_eq!(result.woken.map(|w| w.thread_id), Some(1));
        assert_eq!(result.moved, 1);
        assert!(table_valid(&result.table));
        assert!(requeue_preserves_waiter_count(&table(), &result));
    }
}

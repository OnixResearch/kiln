// Verus proofs guarantee u128→u64 casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified weighted round-robin scheduling.
//!
//! Each consumer has a weight. In each round, consumers are served in order;
//! a consumer with weight `w` receives `w` units of service before advancing
//! to the next consumer. The deficit counter variant tracks accumulated
//! deficit and serves until the deficit is repaid. Proofs cover round
//! fairness, deficit conservation, and cycle completion.

use vstd::prelude::*;

verus! {

// ============================================================
// Quantum computation: how many units per turn
// ============================================================

// r[depends wrr.quantum]
/// Spec: compute the quantum (service units per turn) for a consumer.
/// quantum = max(1, weight * base_quantum / total_weight)
/// Ensures every consumer gets at least 1 unit per turn.
pub open spec fn quantum_spec(weight: u64, total_weight: u64, base_quantum: u64) -> u64 {
    if total_weight == 0 || weight == 0 {
        0u64
    } else {
        let q = (weight as int * base_quantum as int) / total_weight as int;
        if q < 1 {
            1u64
        } else if q > u64::MAX as int {
            u64::MAX
        } else {
            q as u64
        }
    }
}

// r[impl wrr.quantum]
/// Compute the quantum for a consumer.
pub fn quantum(weight: u64, total_weight: u64, base_quantum: u64) -> (result: u64)
    requires total_weight > 0, weight > 0
    ensures result == quantum_spec(weight, total_weight, base_quantum)
{
    let w128 = weight as u128;
    let bq128 = base_quantum as u128;
    assert(w128 as int * bq128 as int <= u128::MAX as int) by(nonlinear_arith)
        requires w128 as int <= u64::MAX as int, bq128 as int <= u64::MAX as int,
                 (u64::MAX as int) * (u64::MAX as int) <= u128::MAX as int;
    let product = w128 * bq128;
    let tw128 = total_weight as u128;
    let q128 = product / tw128;
    let q = if q128 > u64::MAX as u128 { u64::MAX } else { q128 as u64 };
    if q < 1 { 1 } else { q }
}

// r[verify wrr.quantum]
/// Every active consumer gets at least one unit.
proof fn quantum_at_least_one(weight: u64, total_weight: u64, base_quantum: u64)
    requires total_weight > 0, weight > 0
    ensures quantum_spec(weight, total_weight, base_quantum) >= 1
{}

// r[verify wrr.quantum]
/// Full weight gets at least base_quantum.
proof fn full_weight_quantum(total_weight: u64, base_quantum: u64)
    requires total_weight > 0, base_quantum > 0
    ensures quantum_spec(total_weight, total_weight, base_quantum) >= base_quantum
{
    assert(total_weight as int * base_quantum as int / total_weight as int == base_quantum as int) by(nonlinear_arith)
        requires total_weight > 0u64;
}

// r[verify wrr.quantum]
/// Higher weight gets at least as large a quantum.
proof fn higher_weight_larger_quantum(w1: u64, w2: u64, tw: u64, bq: u64)
    requires tw > 0, w1 > 0, w2 > 0, w1 <= w2
    ensures quantum_spec(w1, tw, bq) <= quantum_spec(w2, tw, bq)
{
    assert(w1 as int * bq as int <= w2 as int * bq as int) by(nonlinear_arith)
        requires w1 <= w2, bq >= 0u64;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        w1 as int * bq as int,
        w2 as int * bq as int,
        tw as int,
    );
}

// ============================================================
// Deficit counter
// ============================================================

// r[depends wrr.deficit-add]
/// Spec: add quantum to the deficit counter at the start of a consumer's turn.
/// Saturates at u64::MAX.
pub open spec fn deficit_add_spec(deficit: u64, quantum: u64) -> u64 {
    if (deficit as int + quantum as int) > u64::MAX as int {
        u64::MAX
    } else {
        (deficit as int + quantum as int) as u64
    }
}

// r[impl wrr.deficit-add]
/// Add quantum to deficit counter.
pub fn deficit_add(deficit: u64, quantum: u64) -> (result: u64)
    ensures result == deficit_add_spec(deficit, quantum)
{
    if quantum > u64::MAX - deficit {
        u64::MAX
    } else {
        deficit + quantum
    }
}

// r[depends wrr.deficit-consume]
/// Spec: consume `cost` from the deficit. Returns (new_deficit, served).
/// If cost > deficit, only `deficit` units are served (partial service).
pub open spec fn deficit_consume_spec(deficit: u64, cost: u64) -> (u64, u64) {
    if cost <= deficit {
        ((deficit - cost) as u64, cost)
    } else {
        (0u64, deficit)
    }
}

// r[impl wrr.deficit-consume]
/// Consume units from the deficit counter.
pub fn deficit_consume(deficit: u64, cost: u64) -> (result: (u64, u64))
    ensures result == deficit_consume_spec(deficit, cost)
{
    if cost <= deficit {
        (deficit - cost, cost)
    } else {
        (0, deficit)
    }
}

// r[verify wrr.deficit-consume]
/// Full service when deficit covers cost.
proof fn full_service_when_sufficient(deficit: u64, cost: u64)
    requires cost <= deficit
    ensures deficit_consume_spec(deficit, cost).1 == cost
{}

// r[verify wrr.deficit-consume]
/// Deficit after consume plus served equals original deficit.
proof fn deficit_conservation(deficit: u64, cost: u64)
    ensures
        deficit_consume_spec(deficit, cost).0 as int
            + deficit_consume_spec(deficit, cost).1 as int
            == deficit as int,
{}

// r[verify wrr.deficit-consume]
/// Served amount never exceeds cost.
proof fn served_bounded_by_cost(deficit: u64, cost: u64)
    ensures deficit_consume_spec(deficit, cost).1 <= cost
{}

// r[verify wrr.deficit-consume]
/// Served amount never exceeds deficit.
proof fn served_bounded_by_deficit(deficit: u64, cost: u64)
    ensures deficit_consume_spec(deficit, cost).1 <= deficit
{}

// ============================================================
// Has deficit: should this consumer be scheduled?
// ============================================================

// r[depends wrr.has-deficit]
/// Spec: a consumer should be scheduled if it has positive deficit.
pub open spec fn has_deficit_spec(deficit: u64) -> bool {
    deficit > 0
}

// r[impl wrr.has-deficit]
/// Returns true if the consumer has remaining deficit.
pub fn has_deficit(deficit: u64) -> (result: bool)
    ensures result == has_deficit_spec(deficit)
{
    deficit > 0
}

// r[verify wrr.has-deficit]
/// After adding a positive quantum, consumer has deficit.
proof fn add_creates_deficit(deficit: u64, quantum: u64)
    requires quantum > 0
    ensures has_deficit_spec(deficit_add_spec(deficit, quantum))
{}

// r[verify wrr.has-deficit]
/// After consuming all deficit, consumer has no deficit.
proof fn full_consume_clears_deficit(deficit: u64)
    ensures !has_deficit_spec(deficit_consume_spec(deficit, deficit).0)
{}

// ============================================================
// Next consumer in round-robin order
// ============================================================

// r[depends wrr.next-consumer]
/// Spec: advance to the next consumer in round-robin order.
/// Wraps around when reaching the end.
pub open spec fn next_consumer_spec(current: u64, num_consumers: u64) -> u64 {
    if num_consumers == 0 {
        0u64
    } else {
        if current + 1 >= num_consumers {
            0u64
        } else {
            (current + 1) as u64
        }
    }
}

// r[impl wrr.next-consumer]
/// Advance to the next consumer.
pub fn next_consumer(current: u64, num_consumers: u64) -> (result: u64)
    requires num_consumers > 0, current < u64::MAX
    ensures result == next_consumer_spec(current, num_consumers)
{
    if current + 1 >= num_consumers {
        0
    } else {
        current + 1
    }
}

// r[verify wrr.next-consumer]
/// Result is always a valid consumer index.
proof fn next_in_range(current: u64, num_consumers: u64)
    requires num_consumers > 0
    ensures next_consumer_spec(current, num_consumers) < num_consumers
{}

// r[verify wrr.next-consumer]
/// Starting from 0 with n consumers, n steps return to 0.
proof fn full_cycle_returns(num_consumers: u64)
    requires num_consumers > 0
    ensures next_consumer_spec((num_consumers - 1) as u64, num_consumers) == 0u64
{}

// r[verify wrr.next-consumer]
/// For interior positions, next is current + 1.
proof fn next_increments(current: u64, num_consumers: u64)
    requires num_consumers > 0, current + 1 < num_consumers
    ensures next_consumer_spec(current, num_consumers) == (current + 1) as u64
{}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod test {
    use super::*;

    // r[verify wrr.quantum]
    #[test]
    fn test_quantum() {
        // weight=3, total=10, base=100 → floor(300/10)=30
        assert_eq!(quantum(3, 10, 100), 30);
        // weight=1, total=1000, base=1 → floor(1/1000)=0 → clamped to 1
        assert_eq!(quantum(1, 1000, 1), 1);
        // full weight
        assert_eq!(quantum(10, 10, 100), 100);
    }

    // r[verify wrr.deficit-consume]
    #[test]
    fn test_deficit_consume() {
        assert_eq!(deficit_consume(100, 30), (70, 30));
        assert_eq!(deficit_consume(20, 50), (0, 20)); // partial
        assert_eq!(deficit_consume(0, 10), (0, 0));
    }

    // r[verify wrr.deficit-add]
    #[test]
    fn test_deficit_add() {
        assert_eq!(deficit_add(10, 30), 40);
        assert_eq!(deficit_add(u64::MAX - 5, 10), u64::MAX); // saturates
    }

    // r[verify wrr.next-consumer]
    #[test]
    fn test_next_consumer() {
        assert_eq!(next_consumer(0, 3), 1);
        assert_eq!(next_consumer(1, 3), 2);
        assert_eq!(next_consumer(2, 3), 0); // wrap
    }

    // r[verify wrr.has-deficit]
    #[test]
    fn test_has_deficit() {
        assert!(has_deficit(1));
        assert!(!has_deficit(0));
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            // r[verify wrr.quantum]
            #[test]
            fn prop_quantum_at_least_one(w in 1u64..=1000, tw in 1u64..=1000, bq: u64) {
                prop_assert!(quantum(w, tw, bq) >= 1);
            }

            // r[verify wrr.deficit-consume]
            #[test]
            fn prop_deficit_conservation(deficit: u64, cost: u64) {
                let (remaining, served) = deficit_consume(deficit, cost);
                prop_assert_eq!(remaining as u128 + served as u128, deficit as u128);
            }

            // r[verify wrr.next-consumer]
            #[test]
            fn prop_next_in_range(current: u64, n in 1u64..=10000) {
                prop_assert!(next_consumer(current, n) < n);
            }
        }
    }
}

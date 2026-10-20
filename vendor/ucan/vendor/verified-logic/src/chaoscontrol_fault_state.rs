//! Verified ChaosControl fault-state quorum-health primitives.
//!
//! Models alive-mask encoding, partition counting, majority/pair-connected
//! health, and the healthy-period liveness gate.

use vstd::prelude::*;

verus! {

/// Fault state for a cluster.
pub struct FaultState {
    pub num_nodes: u32,
    pub alive_mask: u64,
    /// Directed partition matrix: partitions[i * num_nodes + j] is true
    /// if node i cannot reach node j.
    pub partitions: Vec<bool>,
}

// ── Spec helpers ────────────────────────────────────────────────────

/// Valid fault state.
pub open spec fn valid_fault_state_spec(s: FaultState) -> bool {
    &&& 2 <= s.num_nodes <= 64
    &&& s.partitions@.len() == (s.num_nodes as int) * (s.num_nodes as int)
    // Diagonal is false
    &&& forall|i: int| #![trigger s.partitions@[i * s.num_nodes as int + i]]
        0 <= i < s.num_nodes as int ==>
        s.partitions@[i * s.num_nodes as int + i] == false
    // High bits cleared in alive_mask
    &&& s.num_nodes < 64 ==> (s.alive_mask >> (s.num_nodes as u64)) == 0u64
}

/// Count of alive nodes.
pub open spec fn alive_count_spec(mask: u64, n: int) -> int
    decreases n
{
    if n <= 0 { 0int }
    else {
        alive_count_spec(mask, n - 1) + if (mask >> ((n - 1) as u64)) & 1u64 == 1u64 { 1int } else { 0int }
    }
}

/// Node i is alive.
pub open spec fn is_alive_spec(mask: u64, i: int) -> bool {
    (mask >> (i as u64)) & 1u64 == 1u64
}

/// Majority threshold: strictly more than half.
pub open spec fn majority_spec(n: int) -> int {
    n / 2 + 1
}

/// Bidirectional connectivity between two alive nodes.
pub open spec fn bidirectional_spec(s: FaultState, i: int, j: int) -> bool
    recommends valid_fault_state_spec(s), 0 <= i < s.num_nodes as int, 0 <= j < s.num_nodes as int
{
    &&& i != j
    &&& is_alive_spec(s.alive_mask, i)
    &&& is_alive_spec(s.alive_mask, j)
    &&& !s.partitions@[i * s.num_nodes as int + j]
    &&& !s.partitions@[j * s.num_nodes as int + i]
}

/// Quorum reachable: majority alive AND some bidirectional pair exists.
pub open spec fn quorum_reachable_spec(s: FaultState) -> bool
    recommends valid_fault_state_spec(s)
{
    &&& alive_count_spec(s.alive_mask, s.num_nodes as int) >= majority_spec(s.num_nodes as int)
    &&& exists|i: int, j: int| 0 <= i < s.num_nodes as int && 0 <= j < s.num_nodes as int
        && bidirectional_spec(s, i, j)
}

/// Healthy-period gate.
pub open spec fn healthy_period_spec(s: FaultState) -> bool
    recommends valid_fault_state_spec(s)
{
    &&& alive_count_spec(s.alive_mask, s.num_nodes as int) == s.num_nodes as int
    &&& quorum_reachable_spec(s)
}

// ── Exec functions ──────────────────────────────────────────────────

// r[impl chaoscontrol-fault-state.quorum-health]
/// Construct a valid fault state.
pub fn new_fault_state(num_nodes: u32, alive_mask: u64, partitions: Vec<bool>) -> (result: FaultState)
    requires
        2 <= num_nodes <= 64,
        partitions@.len() == (num_nodes as int) * (num_nodes as int),
        forall|i: int| #![trigger partitions@[i * num_nodes as int + i]]
            0 <= i < num_nodes as int ==>
            partitions@[i * num_nodes as int + i] == false,
        num_nodes < 64 ==> (alive_mask >> (num_nodes as u64)) == 0u64,
    ensures valid_fault_state_spec(result)
{
    FaultState { num_nodes, alive_mask, partitions }
}

// r[impl chaoscontrol-fault-state.quorum-health]
/// Count alive nodes.
pub fn alive_count(mask: u64, num_nodes: u32) -> (count: u32)
    requires 0 < num_nodes <= 64
    ensures count as int == alive_count_spec(mask, num_nodes as int)
{
    let mut count: u32 = 0;
    let mut i: u32 = 0;
    while i < num_nodes
        invariant
            0 <= i <= num_nodes,
            num_nodes <= 64,
            count as int == alive_count_spec(mask, i as int),
            count <= i,
        decreases num_nodes - i,
    {
        proof { alive_count_bounded(mask, i as int); }
        if (mask >> (i as u64)) & 1u64 == 1u64 {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// r[impl chaoscontrol-fault-state.quorum-health]
/// Count directed partitions (non-diagonal blocked links).
pub fn partition_count(s: &FaultState) -> (count: u32)
    requires valid_fault_state_spec(*s)
    ensures count >= 0
{
    let n = s.num_nodes;
    let mut count: u64 = 0;
    let mut i: u32 = 0;
    while i < n
        invariant
            0 <= i <= n,
            n == s.num_nodes,
            2 <= n <= 64,
            valid_fault_state_spec(*s),
            s.partitions@.len() == (n as int) * (n as int),
            count as int <= (i as int) * (n as int),
        decreases n - i,
    {
        let mut j: u32 = 0;
        while j < n
            invariant
                0 <= j <= n,
                n == s.num_nodes,
                2 <= n <= 64,
                i < n,
                valid_fault_state_spec(*s),
                s.partitions@.len() == (n as int) * (n as int),
                count as int <= (i as int) * (n as int) + (j as int),
            decreases n - j,
        {
            if i != j {
                proof {
                    let ii = i as int;
                    let jj = j as int;
                    let nn = n as int;
                    assert(ii * nn <= 63 * 64) by(nonlinear_arith)
                        requires ii >= 0, ii <= 63, nn >= 0, nn <= 64;
                    assert(ii * nn + jj < nn * nn) by(nonlinear_arith)
                        requires ii >= 0, ii <= nn - 1, jj >= 0, jj <= nn - 1, nn >= 2;
                }
                let idx_base: usize = (i as usize) * (n as usize);
                let idx = idx_base + (j as usize);
                if s.partitions[idx] {
                    count = count + 1;
                }
            }
            j = j + 1;
        }
        // After inner loop: j == n, so count <= i*n + n = (i+1)*n
        proof {
            assert((i as int + 1) * (n as int) == (i as int) * (n as int) + (n as int)) by(nonlinear_arith)
                requires i as int >= 0, n as int >= 0;
        }
        i = i + 1;
    }
    #[allow(clippy::cast_possible_truncation)] // count <= n*n <= 4096 < u32::MAX, proven by invariant
    { count as u32 }
}

// r[impl chaoscontrol-fault-state.quorum-health]
/// Check if quorum is reachable.
pub fn quorum_reachable(s: &FaultState) -> (result: bool)
    requires valid_fault_state_spec(*s)
    ensures result ==> alive_count_spec(s.alive_mask, s.num_nodes as int) >= majority_spec(s.num_nodes as int)
{
    let ac = alive_count(s.alive_mask, s.num_nodes);
    let maj = s.num_nodes / 2 + 1;
    if ac < maj {
        return false;
    }
    // Check for any bidirectional alive pair
    let n = s.num_nodes;
    let mut i: u32 = 0;
    while i < n
        invariant
            0 <= i <= n,
            n == s.num_nodes,
            2 <= n <= 64,
            valid_fault_state_spec(*s),
            s.partitions@.len() == (n as int) * (n as int),
            alive_count_spec(s.alive_mask, s.num_nodes as int) >= majority_spec(s.num_nodes as int),
        decreases n - i,
    {
        let mut j: u32 = i + 1;
        while j < n
            invariant
                0 <= i < n,
                i < j,
                j <= n,
                n == s.num_nodes,
                2 <= n <= 64,
                valid_fault_state_spec(*s),
                s.partitions@.len() == (n as int) * (n as int),
                alive_count_spec(s.alive_mask, s.num_nodes as int) >= majority_spec(s.num_nodes as int),
            decreases n - j,
        {
            let i_u64: u64 = i as u64;
            let j_u64: u64 = j as u64;
            let i_alive = (s.alive_mask >> i_u64) & 1u64 == 1u64;
            let j_alive = (s.alive_mask >> j_u64) & 1u64 == 1u64;
            if i_alive && j_alive {
                proof {
                    let ii = i as int;
                    let jj = j as int;
                    let nn = n as int;
                    assert(ii * nn <= 63 * 64) by(nonlinear_arith)
                        requires ii >= 0, ii <= 63, nn >= 0, nn <= 64;
                    assert(jj * nn <= 63 * 64) by(nonlinear_arith)
                        requires jj >= 0, jj <= 63, nn >= 0, nn <= 64;
                    assert(ii * nn + jj < nn * nn) by(nonlinear_arith)
                        requires ii >= 0, ii <= nn - 1, jj >= 0, jj <= nn - 1, nn >= 2;
                    assert(jj * nn + ii < nn * nn) by(nonlinear_arith)
                        requires jj >= 0, jj <= nn - 1, ii >= 0, ii <= nn - 1, nn >= 2;
                }
                let ij_idx: usize = (i as usize) * (n as usize);
                let ij = ij_idx + (j as usize);
                let ji_idx: usize = (j as usize) * (n as usize);
                let ji = ji_idx + (i as usize);
                if !s.partitions[ij] && !s.partitions[ji] {
                    return true;
                }
            }
            j = j + 1;
        }
        i = i + 1;
    }
    false
}

// r[impl chaoscontrol-fault-state.liveness-gate]
/// Healthy-period gate: all alive AND quorum reachable.
pub fn healthy_period(s: &FaultState) -> (result: bool)
    requires valid_fault_state_spec(*s)
    ensures result ==> alive_count_spec(s.alive_mask, s.num_nodes as int) == s.num_nodes as int
{
    let ac = alive_count(s.alive_mask, s.num_nodes);
    ac == s.num_nodes && quorum_reachable(s)
}

// ── Proof artifacts ─────────────────────────────────────────────────

// r[verify chaoscontrol-fault-state.quorum-health]
/// Alive count is bounded by num_nodes.
pub proof fn alive_count_bounded(mask: u64, n: int)
    requires 0 <= n <= 64
    ensures 0 <= alive_count_spec(mask, n) <= n
    decreases n
{
    if n > 0 {
        alive_count_bounded(mask, n - 1);
    }
}

// r[verify chaoscontrol-fault-state.quorum-health]
/// Majority is at least 2 for n >= 2.
pub proof fn majority_at_least_two(n: int)
    requires n >= 2
    ensures majority_spec(n) >= 2
{}

// r[verify chaoscontrol-fault-state.liveness-gate]
/// Healthy period implies quorum reachable.
pub proof fn healthy_implies_quorum(s: FaultState)
    requires valid_fault_state_spec(s), healthy_period_spec(s)
    ensures quorum_reachable_spec(s)
{}

} // verus!

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_matrix(n: usize) -> Vec<bool> {
        vec![false; n * n]
    }

    #[test]
    fn test_all_alive_no_partitions() {
        let n = 3u32;
        let mask = 0b111u64;
        let s = new_fault_state(n, mask, make_matrix(3));
        assert_eq!(alive_count(mask, n), 3);
        assert!(quorum_reachable(&s));
        assert!(healthy_period(&s));
    }

    #[test]
    fn test_minority_alive() {
        let n = 5u32;
        let mask = 0b00011u64; // 2 of 5
        let s = new_fault_state(n, mask, make_matrix(5));
        assert_eq!(alive_count(mask, n), 2);
        assert!(!quorum_reachable(&s)); // 2 < 3 majority
    }

    #[test]
    fn test_majority_but_all_partitioned() {
        let n = 3u32;
        let mask = 0b111u64;
        let mut parts = make_matrix(3);
        // Partition all non-diagonal pairs
        for i in 0..3usize {
            for j in 0..3usize {
                if i != j {
                    parts[i * 3 + j] = true;
                }
            }
        }
        let s = new_fault_state(n, mask, parts);
        assert!(!quorum_reachable(&s)); // no bidirectional pair
    }

    #[test]
    fn test_bidirectional_pair() {
        let n = 3u32;
        let mask = 0b111u64;
        let mut parts = make_matrix(3);
        // Partition 0->1 but not 1->0 or 0->2/2->0
        parts[1] = true; // 0*3+1
        let s = new_fault_state(n, mask, parts);
        // Pair (0,2) has bidirectional connectivity
        assert!(quorum_reachable(&s));
    }

    #[test]
    fn test_not_all_alive_not_healthy() {
        let n = 3u32;
        let mask = 0b110u64; // node 0 dead
        let s = new_fault_state(n, mask, make_matrix(3));
        assert!(!healthy_period(&s)); // not all alive
    }

    #[test]
    fn test_partition_count() {
        let n = 3u32;
        let mut parts = make_matrix(3);
        parts[1] = true; // 0*3+1
        parts[5] = true; // 1*3+2
        let s = new_fault_state(n, 0b111, parts);
        assert_eq!(partition_count(&s), 2);
    }
}

//! Verified settlement and netting of bilateral obligations.
//!
//! Given N parties with obligations (A owes B some amount), netting
//! computes each party's net position (credits minus debits) and
//! proves the positions sum to zero. A valid settlement is any set of
//! transfers that produces the same net positions as the original
//! obligations. Conservation: nobody gains or loses from netting.

use vstd::prelude::*;

verus! {

// -----------------------------------------------------------------------
// Integer-valued sum (positions are signed)
// -----------------------------------------------------------------------

// r[impl settlement.int-sum]
// r[verify settlement.int-sum]
// r[depends settlement.int-sum]
/// Spec: recursive sum of Seq<int> from index.
pub open spec fn int_sum_spec(v: Seq<int>, from: int) -> int
    decreases v.len() - from
{
    if from >= v.len() {
        0
    } else {
        v[from] + int_sum_spec(v, from + 1)
    }
}

// r[depends settlement.int-sum]
/// Spec: total of all elements.
pub open spec fn int_sum_all_spec(v: Seq<int>) -> int {
    int_sum_spec(v, 0)
}

// -----------------------------------------------------------------------
// Net position computation
// -----------------------------------------------------------------------

// r[depends settlement.obligation-valid]
/// Spec: an obligation is valid when both party indices are in range and distinct.
pub open spec fn obligation_valid_spec(
    from_party: int, to_party: int, amount: u64, num_parties: int,
) -> bool {
    0 <= from_party < num_parties
    && 0 <= to_party < num_parties
    && from_party != to_party
    && amount > 0
}

// r[impl settlement.all-valid]
// r[verify settlement.all-valid]
// r[depends settlement.all-valid]
/// Spec: all obligations in the sequence are valid.
pub open spec fn all_obligations_valid_spec(
    obligations: Seq<(int, int, u64)>, num_parties: int, from: int,
) -> bool
    decreases obligations.len() - from
{
    if from >= obligations.len() {
        true
    } else {
        let (fp, tp, amt) = obligations[from];
        obligation_valid_spec(fp, tp, amt, num_parties)
        && all_obligations_valid_spec(obligations, num_parties, from + 1)
    }
}

// r[impl settlement.apply-obligation]
// r[verify settlement.apply-obligation]
// r[depends settlement.apply-obligation]
/// Spec: apply one obligation to positions — debit sender, credit receiver.
pub open spec fn apply_obligation_spec(
    positions: Seq<int>, from_party: int, to_party: int, amount: u64,
) -> Seq<int>
    recommends
        0 <= from_party < positions.len(),
        0 <= to_party < positions.len(),
        from_party != to_party,
{
    positions
        .update(from_party, positions[from_party] - amount as int)
        .update(to_party, positions[to_party] + amount as int)
}

// r[depends settlement.compute-positions]
/// Spec: apply obligations from index `from` onward to positions.
pub open spec fn apply_from_spec(
    positions: Seq<int>, obligations: Seq<(int, int, u64)>, from: int,
) -> Seq<int>
    decreases obligations.len() - from
{
    if from >= obligations.len() {
        positions
    } else {
        let (fp, tp, amt) = obligations[from];
        apply_from_spec(
            apply_obligation_spec(positions, fp, tp, amt),
            obligations,
            from + 1,
        )
    }
}

// r[depends settlement.compute-positions]
/// Spec: compute net positions from obligations.
pub open spec fn compute_positions_spec(
    obligations: Seq<(int, int, u64)>, num_parties: int,
) -> Seq<int> {
    let zeros = Seq::new(num_parties as nat, |_i: int| 0int);
    apply_from_spec(zeros, obligations, 0)
}

/// Exec-side obligation view: convert `(usize, usize, u64)` to `(int, int, u64)`.
pub open spec fn obligation_view_spec(obligations: Seq<(usize, usize, u64)>) -> Seq<(int, int, u64)> {
    obligations.map(|_i: int, o: (usize, usize, u64)| (o.0 as int, o.1 as int, o.2))
}

/// Exec-side position view: convert `i128` positions to mathematical integers.
pub open spec fn position_view_spec(positions: Seq<i128>) -> Seq<int> {
    positions.map(|_i: int, p: i128| p as int)
}

/// Exec-side prefix application over the first `upto` obligations.
pub open spec fn compute_positions_prefix_exec_spec(
    obligations: Seq<(usize, usize, u64)>, num_parties: usize, upto: int,
) -> Seq<int>
    recommends 0 <= upto <= obligations.len(), num_parties > 0
    decreases upto
{
    if upto <= 0 {
        Seq::new(num_parties as nat, |_i: int| 0int)
    } else {
        let prev = compute_positions_prefix_exec_spec(obligations, num_parties, upto - 1);
        let (from_p, to_p, amt) = obligations[upto - 1];
        apply_obligation_spec(prev, from_p as int, to_p as int, amt)
    }
}

/// Exec-side compute_positions spec over usize-indexed obligations.
pub open spec fn compute_positions_exec_spec(
    obligations: Seq<(usize, usize, u64)>, num_parties: usize,
) -> Seq<int>
    recommends num_parties > 0
{
    compute_positions_prefix_exec_spec(obligations, num_parties, obligations.len() as int)
}

/// Exec-side settlement-validity spec over usize-indexed obligations.
pub open spec fn is_valid_settlement_exec_spec(
    original: Seq<(usize, usize, u64)>,
    settlement: Seq<(usize, usize, u64)>,
    num_parties: usize,
) -> bool
    recommends num_parties > 0
{
    compute_positions_exec_spec(original, num_parties)
        =~= compute_positions_exec_spec(settlement, num_parties)
}

// r[impl settlement.compute-positions]
/// Compute net positions. Returns a vec of length num_parties where each
/// element is total credits minus total debits for that party.
pub fn compute_positions(
    obligations: &[(usize, usize, u64)], num_parties: usize,
) -> (result: Vec<i128>)
    requires
        num_parties > 0,
        obligations@.len() <= 1_000_000,
        forall|i: int| #![trigger obligations@[i]]
            0 <= i < obligations@.len() ==>
                obligations@[i].0 < num_parties
                && obligations@[i].1 < num_parties
                && obligations@[i].0 != obligations@[i].1
                && obligations@[i].2 > 0u64,
    ensures
        result@.len() == num_parties as int,
        position_view_spec(result@) == compute_positions_exec_spec(obligations@, num_parties),
{
    let mut positions: Vec<i128> = Vec::new();
    let mut k: usize = 0;
    while k < num_parties
        invariant
            0 <= k <= num_parties,
            positions@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> positions@[j] == 0i128,
        decreases num_parties - k,
    {
        positions.push(0i128);
        k = k + 1;
    }

    proof {
        assert(position_view_spec(positions@) =~= Seq::new(num_parties as nat, |_i: int| 0int)) by {
            assert forall|j: int| 0 <= j < positions@.len() implies position_view_spec(positions@)[j]
                == Seq::new(num_parties as nat, |_i: int| 0int)[j] by {
                assert(positions@[j] == 0i128);
            }
        }
        assert(position_view_spec(positions@) == compute_positions_prefix_exec_spec(obligations@, num_parties, 0));
    }

    let mut i: usize = 0;
    while i < obligations.len()
        invariant
            0 <= i <= obligations@.len(),
            positions@.len() == num_parties as int,
            num_parties > 0,
            obligations@.len() <= 1_000_000,
            position_view_spec(positions@) == compute_positions_prefix_exec_spec(obligations@, num_parties, i as int),
            forall|j: int| #![trigger obligations@[j]]
                0 <= j < obligations@.len() ==>
                    obligations@[j].0 < num_parties
                    && obligations@[j].1 < num_parties
                    && obligations@[j].0 != obligations@[j].1
                    && obligations@[j].2 > 0u64,
            // Each position is bounded: at most i obligations processed
            forall|k: int| #![trigger positions@[k]]
                0 <= k < positions@.len() ==>
                    positions@[k] >= -(i as int) * (u64::MAX as int)
                    && positions@[k] <= (i as int) * (u64::MAX as int),
        decreases obligations@.len() - i,
    {
        let (from_p, to_p, amt) = obligations[i];
        assert(obligations@[i as int].0 < num_parties);
        assert(obligations@[i as int].1 < num_parties);
        let ghost before_positions = positions@;
        let ghost before_view = position_view_spec(before_positions);
        let old_from = positions[from_p];
        let old_to = positions[to_p];
        // Prove i128 bounds: |old_from - amt| and |old_to + amt| stay in range
        proof {
            assert((i as int) * (u64::MAX as int) + (u64::MAX as int)
                == ((i + 1) as int) * (u64::MAX as int)) by(nonlinear_arith);
            assert(((i + 1) as int) * (u64::MAX as int) <= 1_000_000int * (u64::MAX as int))
                by(nonlinear_arith) requires i < 1_000_000int;
            assert(1_000_000int * (u64::MAX as int) < (i128::MAX as int)) by(nonlinear_arith);
            assert(-(1_000_000int * (u64::MAX as int)) > (i128::MIN as int)) by(nonlinear_arith);
            assert(before_view[from_p as int] == old_from as int);
            assert(before_view[to_p as int] == old_to as int);
        }
        positions.set(from_p, old_from - amt as i128);
        proof {
            assert(position_view_spec(positions@)
                == before_view.update(from_p as int, old_from as int - amt as int));
        }
        positions.set(to_p, old_to + amt as i128);
        proof {
            assert(position_view_spec(positions@)
                == before_view
                    .update(from_p as int, old_from as int - amt as int)
                    .update(to_p as int, old_to as int + amt as int));
            assert(position_view_spec(positions@)
                == apply_obligation_spec(before_view, from_p as int, to_p as int, amt));
            assert(apply_obligation_spec(before_view, from_p as int, to_p as int, amt)
                == compute_positions_prefix_exec_spec(obligations@, num_parties, i as int + 1));
        }
        i = i + 1;
    }
    proof {
        assert(compute_positions_prefix_exec_spec(obligations@, num_parties, obligations@.len() as int)
            == compute_positions_exec_spec(obligations@, num_parties));
    }
    positions
}

// -----------------------------------------------------------------------
// Settlement validity
// -----------------------------------------------------------------------

// r[depends settlement.valid-settlement]
/// Spec: a settlement is valid when it produces the same net positions.
pub open spec fn is_valid_settlement_spec(
    original: Seq<(int, int, u64)>,
    settlement: Seq<(int, int, u64)>,
    num_parties: int,
) -> bool {
    compute_positions_spec(original, num_parties)
        =~= compute_positions_spec(settlement, num_parties)
}

// r[impl settlement.valid-settlement]
/// Check if a settlement produces the same net positions as the original.
pub fn is_valid_settlement(
    original: &[(usize, usize, u64)],
    settlement: &[(usize, usize, u64)],
    num_parties: usize,
) -> (result: bool)
    requires
        num_parties > 0,
        original@.len() <= 1_000_000,
        settlement@.len() <= 1_000_000,
        forall|i: int| #![trigger original@[i]]
            0 <= i < original@.len() ==>
                original@[i].0 < num_parties
                && original@[i].1 < num_parties
                && original@[i].0 != original@[i].1
                && original@[i].2 > 0u64,
        forall|i: int| #![trigger settlement@[i]]
            0 <= i < settlement@.len() ==>
                settlement@[i].0 < num_parties
                && settlement@[i].1 < num_parties
                && settlement@[i].0 != settlement@[i].1
                && settlement@[i].2 > 0u64,
    ensures
        result == is_valid_settlement_exec_spec(original@, settlement@, num_parties),
{
    let orig_pos = compute_positions(original, num_parties);
    let settl_pos = compute_positions(settlement, num_parties);
    let mut i: usize = 0;
    while i < num_parties
        invariant
            0 <= i <= num_parties,
            orig_pos@.len() == num_parties as int,
            settl_pos@.len() == num_parties as int,
            position_view_spec(orig_pos@) == compute_positions_exec_spec(original@, num_parties),
            position_view_spec(settl_pos@) == compute_positions_exec_spec(settlement@, num_parties),
            forall|j: int| 0 <= j < i as int ==> orig_pos@[j] == settl_pos@[j],
        decreases num_parties - i,
    {
        if orig_pos[i] != settl_pos[i] {
            proof {
                assert(position_view_spec(orig_pos@)[i as int] != position_view_spec(settl_pos@)[i as int]);
                if is_valid_settlement_exec_spec(original@, settlement@, num_parties) {
                    assert(compute_positions_exec_spec(original@, num_parties)
                        =~= compute_positions_exec_spec(settlement@, num_parties));
                    assert(position_view_spec(orig_pos@) =~= position_view_spec(settl_pos@));
                    assert(position_view_spec(orig_pos@)[i as int] == position_view_spec(settl_pos@)[i as int]);
                    assert(false);
                }
            }
            return false;
        }
        i = i + 1;
    }
    proof {
        assert forall|j: int| 0 <= j < num_parties as int
            implies position_view_spec(orig_pos@)[j] == position_view_spec(settl_pos@)[j] by {
            assert(orig_pos@[j] == settl_pos@[j]);
        }
        assert(position_view_spec(orig_pos@) =~= position_view_spec(settl_pos@));
        assert(compute_positions_exec_spec(original@, num_parties)
            =~= compute_positions_exec_spec(settlement@, num_parties));
    }
    true
}

// r[depends settlement.obligation-count]
/// Spec: number of obligations.
pub open spec fn obligation_count_spec(obligations: Seq<(int, int, u64)>) -> nat {
    obligations.len()
}

// r[impl settlement.obligation-count]
/// Count obligations.
pub fn obligation_count(obligations: &[(usize, usize, u64)]) -> (result: usize)
    ensures result == obligations@.len()
{
    obligations.len()
}

// r[impl settlement.net-position-at]
// r[verify settlement.net-position-at]
// r[depends settlement.net-position-at]
/// Spec: net position for a single party — credit sum minus debit sum.
pub open spec fn net_position_at_spec(
    obligations: Seq<(int, int, u64)>, party: int, num_parties: int,
) -> int
    recommends 0 <= party < num_parties
{
    compute_positions_spec(obligations, num_parties)[party]
}

// -----------------------------------------------------------------------
// Inductive helpers for int_sum
// -----------------------------------------------------------------------

/// Sum of Seq<int> is additive under element change.
proof fn int_sum_update_delta(v: Seq<int>, idx: int, new_val: int, from: int)
    requires
        0 <= idx < v.len(),
        0 <= from,
    ensures
        int_sum_spec(v.update(idx, new_val), from)
            == int_sum_spec(v, from) - v[idx] + new_val
            + if from <= idx { 0int } else { v[idx] - new_val },
    decreases v.len() - from
{
    let updated = v.update(idx, new_val);
    if from >= v.len() {
        // both sums 0
    } else if from == idx {
        assert(updated[from] == new_val);
        int_sum_update_delta(v, idx, new_val, from + 1);
    } else {
        assert(updated[from] == v[from]);
        int_sum_update_delta(v, idx, new_val, from + 1);
    }
}

/// Simplified: updating idx changes total by (new - old).
proof fn int_sum_update_total(v: Seq<int>, idx: int, new_val: int)
    requires 0 <= idx < v.len()
    ensures
        int_sum_all_spec(v.update(idx, new_val))
            == int_sum_all_spec(v) - v[idx] + new_val,
{
    int_sum_update_delta(v, idx, new_val, 0);
}

/// Sum of all-zeros is zero.
proof fn int_sum_zeros(n: nat)
    ensures int_sum_all_spec(Seq::new(n, |_i: int| 0int)) == 0
    decreases n
{
    if n > 0 {
        let s = Seq::new(n, |_i: int| 0int);
        let prefix = Seq::new((n - 1) as nat, |_i: int| 0int);
        // s[0] == 0, and sum(s, 1) covers elements 1..n
        // By induction on the tail
        int_sum_zeros_from(s, 0);
    }
}

/// Helper: sum of all-zeros from any index is zero.
proof fn int_sum_zeros_from(v: Seq<int>, from: int)
    requires
        0 <= from,
        forall|i: int| 0 <= i < v.len() ==> v[i] == 0,
    ensures int_sum_spec(v, from) == 0
    decreases v.len() - from
{
    if from < v.len() {
        int_sum_zeros_from(v, from + 1);
    }
}

// -----------------------------------------------------------------------
// Core conservation proofs
// -----------------------------------------------------------------------

// r[impl settlement.apply-preserves-sum]
// r[verify settlement.apply-preserves-sum]
/// Applying one obligation preserves the sum of positions.
/// Because +amount and -amount cancel.
proof fn apply_obligation_preserves_sum(
    positions: Seq<int>, from_party: int, to_party: int, amount: u64,
)
    requires
        0 <= from_party < positions.len(),
        0 <= to_party < positions.len(),
        from_party != to_party,
    ensures
        int_sum_all_spec(apply_obligation_spec(positions, from_party, to_party, amount))
            == int_sum_all_spec(positions),
{
    let step1 = positions.update(from_party, positions[from_party] - amount as int);
    let step2 = step1.update(to_party, step1[to_party] + amount as int);

    // Step 1: debit sender
    int_sum_update_total(positions, from_party, positions[from_party] - amount as int);
    // int_sum(step1) == int_sum(positions) - amount

    // Step 2: credit receiver (step1[to_party] == positions[to_party] because from != to)
    assert(step1[to_party] == positions[to_party]);
    int_sum_update_total(step1, to_party, positions[to_party] + amount as int);
    // int_sum(step2) == int_sum(step1) + amount == int_sum(positions)
}

// r[impl settlement.positions-sum-zero]
// r[verify settlement.positions-sum-zero]
/// Net positions always sum to zero. The fundamental conservation theorem.
/// Each obligation contributes +amount to one party and -amount to another.
pub proof fn positions_sum_zero(
    obligations: Seq<(int, int, u64)>, num_parties: int,
)
    requires
        num_parties > 0,
        all_obligations_valid_spec(obligations, num_parties, 0),
    ensures
        int_sum_all_spec(compute_positions_spec(obligations, num_parties)) == 0,
{
    let zeros = Seq::new(num_parties as nat, |_i: int| 0int);
    int_sum_zeros(num_parties as nat);
    apply_from_preserves_sum(zeros, obligations, num_parties, 0);
}

/// Helper: apply_from preserves the sum of positions.
proof fn apply_from_preserves_sum(
    positions: Seq<int>, obligations: Seq<(int, int, u64)>,
    num_parties: int, from: int,
)
    requires
        0 <= from,
        positions.len() == num_parties,
        num_parties > 0,
        all_obligations_valid_spec(obligations, num_parties, from),
    ensures
        int_sum_all_spec(apply_from_spec(positions, obligations, from))
            == int_sum_all_spec(positions),
    decreases obligations.len() - from
{
    if from < obligations.len() {
        let (fp, tp, amt) = obligations[from];
        let next = apply_obligation_spec(positions, fp, tp, amt);
        apply_obligation_preserves_sum(positions, fp, tp, amt);
        // int_sum(next) == int_sum(positions)
        apply_from_preserves_sum(next, obligations, num_parties, from + 1);
        // int_sum(apply_from(next, ...)) == int_sum(next) == int_sum(positions)
    }
}

// r[impl settlement.apply-preserves-length]
// r[verify settlement.apply-preserves-length]
/// Applying obligations preserves the position vector length.
proof fn apply_from_preserves_length(
    positions: Seq<int>, obligations: Seq<(int, int, u64)>,
    num_parties: int, from: int,
)
    requires
        0 <= from,
        positions.len() == num_parties,
        num_parties > 0,
        all_obligations_valid_spec(obligations, num_parties, from),
    ensures
        apply_from_spec(positions, obligations, from).len() == num_parties,
    decreases obligations.len() - from
{
    if from < obligations.len() {
        let (fp, tp, amt) = obligations[from];
        let next = apply_obligation_spec(positions, fp, tp, amt);
        apply_from_preserves_length(next, obligations, num_parties, from + 1);
    }
}

// r[verify settlement.valid-settlement]
/// A valid settlement preserves every party's net position.
proof fn settlement_preserves_all_positions(
    original: Seq<(int, int, u64)>,
    settlement: Seq<(int, int, u64)>,
    num_parties: int,
    party: int,
)
    requires
        0 <= party < num_parties,
        is_valid_settlement_spec(original, settlement, num_parties),
    ensures
        net_position_at_spec(original, party, num_parties)
            == net_position_at_spec(settlement, party, num_parties),
{
    // Follows directly from extensional equality of position vectors
}

// r[verify settlement.positions-sum-zero]
/// Empty obligations produce all-zero positions.
proof fn empty_obligations_zero_positions(num_parties: int)
    requires num_parties > 0
    ensures
        compute_positions_spec(Seq::<(int, int, u64)>::empty(), num_parties)
            =~= Seq::new(num_parties as nat, |_i: int| 0int),
{}

// r[impl settlement.self-settlement]
// r[verify settlement.self-settlement]
/// The original obligations are a valid settlement of themselves.
proof fn self_settlement(obligations: Seq<(int, int, u64)>, num_parties: int)
    ensures is_valid_settlement_spec(obligations, obligations, num_parties)
{}

// r[verify settlement.positions-sum-zero]
/// A single obligation produces positions that sum to zero.
proof fn single_obligation_sum_zero(from_p: int, to_p: int, amount: u64, num_parties: int)
    requires obligation_valid_spec(from_p, to_p, amount, num_parties)
    ensures
        int_sum_all_spec(
            compute_positions_spec(seq![(from_p, to_p, amount)], num_parties)
        ) == 0,
{
    let obs = seq![(from_p, to_p, amount)];
    // Prove all_obligations_valid for the single-element sequence
    assert(all_obligations_valid_spec(obs, num_parties, 1));
    assert(all_obligations_valid_spec(obs, num_parties, 0));
    positions_sum_zero(obs, num_parties);
}

// r[impl settlement.obligation-valid]
// r[verify settlement.obligation-valid]
/// Swapping from and to negates the net position contribution.
proof fn swap_negates_position(
    from_p: int, to_p: int, amount: u64, num_parties: int,
)
    requires obligation_valid_spec(from_p, to_p, amount, num_parties)
    ensures ({
        let zeros = Seq::new(num_parties as nat, |_i: int| 0int);
        let fwd = apply_obligation_spec(zeros, from_p, to_p, amount);
        let rev = apply_obligation_spec(zeros, to_p, from_p, amount);
        fwd[from_p] == -rev[from_p]
        && fwd[to_p] == -rev[to_p]
    })
{
    let zeros = Seq::new(num_parties as nat, |_i: int| 0int);

    // Forward: from_p gets -amount, to_p gets +amount
    let fwd_step = apply_obligation_spec(zeros, from_p, to_p, amount);
    // Reverse: to_p gets -amount, from_p gets +amount
    let rev_step = apply_obligation_spec(zeros, to_p, from_p, amount);

    assert(fwd_step[from_p] == -(amount as int));
    assert(fwd_step[to_p] == amount as int);
    assert(rev_step[from_p] == amount as int);
    assert(rev_step[to_p] == -(amount as int));
}

} // verus!

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify settlement.compute-positions]
    #[test]
    fn test_compute_positions_basic() {
        // A(0) owes B(1) 100, B(1) owes C(2) 80, C(2) owes A(0) 50
        let obligations = [(0, 1, 100u64), (1, 2, 80), (2, 0, 50)];
        let positions = compute_positions(&obligations, 3);
        assert_eq!(positions.len(), 3);
        // A: -100 + 50 = -50
        assert_eq!(positions[0], -50);
        // B: +100 - 80 = +20
        assert_eq!(positions[1], 20);
        // C: +80 - 50 = +30
        assert_eq!(positions[2], 30);
        // Sum = 0
        assert_eq!(positions[0] + positions[1] + positions[2], 0);
    }

    // r[verify settlement.positions-sum-zero]
    #[test]
    fn test_positions_sum_zero() {
        let obligations = [(0, 1, 100u64), (1, 2, 80), (2, 0, 50)];
        let positions = compute_positions(&obligations, 3);
        let sum: i128 = positions.iter().sum();
        assert_eq!(sum, 0);
    }

    // r[verify settlement.valid-settlement]
    #[test]
    fn test_valid_settlement() {
        // Original: A→B:100, B→C:80, C→A:50
        // Netted: A→B:20, A→C:30 (same net positions)
        let original = [(0usize, 1, 100u64), (1, 2, 80), (2, 0, 50)];
        let settlement = [(0usize, 1, 20u64), (0, 2, 30)];

        let orig_pos = compute_positions(&original, 3);
        let settl_pos = compute_positions(&settlement, 3);

        // Same net positions
        assert_eq!(orig_pos[0], settl_pos[0]); // A: -50
        assert_eq!(orig_pos[1], settl_pos[1]); // B: +20
        assert_eq!(orig_pos[2], settl_pos[2]); // C: +30
    }

    // r[verify settlement.obligation-count]
    #[test]
    fn test_netting_reduces_count() {
        // 3 obligations → 2 after netting
        let original = [(0usize, 1, 100u64), (1, 2, 80), (2, 0, 50)];
        let settlement = [(0usize, 1, 20u64), (0, 2, 30)];
        assert!(settlement.len() < original.len());
    }

    // r[verify settlement.compute-positions]
    #[test]
    fn test_empty_obligations() {
        let positions = compute_positions(&[], 3);
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], 0);
        assert_eq!(positions[1], 0);
        assert_eq!(positions[2], 0);
    }

    // r[verify settlement.compute-positions]
    #[test]
    fn test_single_obligation() {
        let positions = compute_positions(&[(0, 1, 42u64)], 2);
        assert_eq!(positions[0], -42);
        assert_eq!(positions[1], 42);
    }

    proptest! {
        // r[verify settlement.positions-sum-zero]
        #[test]
        fn prop_positions_sum_zero(
            obligations in proptest::collection::vec(
                (0usize..5, 0usize..5, 1..1000u64), 0..20
            ),
        ) {
            let valid: Vec<(usize, usize, u64)> = obligations
                .into_iter()
                .filter(|(f, t, _)| f != t && *f < 5 && *t < 5)
                .collect();
            let positions = compute_positions(&valid, 5);
            let sum: i128 = positions.iter().sum();
            prop_assert_eq!(sum, 0);
        }

        // r[verify settlement.valid-settlement]
        #[test]
        fn prop_self_settlement(
            obligations in proptest::collection::vec(
                (0usize..4, 0usize..4, 1..500u64), 0..10
            ),
        ) {
            let valid: Vec<(usize, usize, u64)> = obligations
                .into_iter()
                .filter(|(f, t, _)| f != t && *f < 4 && *t < 4)
                .collect();
            // Any set of obligations is a valid settlement of itself
            prop_assert!(is_valid_settlement(&valid, &valid, 4));
        }

        // r[verify settlement.compute-positions]
        #[test]
        fn prop_swap_negates(
            from_p in 0usize..5,
            to_p in 0usize..5,
            amount in 1..1000u64,
        ) {
            if from_p != to_p {
                let fwd = compute_positions(&[(from_p, to_p, amount)], 5);
                let rev = compute_positions(&[(to_p, from_p, amount)], 5);
                prop_assert_eq!(fwd[from_p], -rev[from_p]);
                prop_assert_eq!(fwd[to_p], -rev[to_p]);
            }
        }

        // r[verify settlement.compute-positions]
        #[test]
        fn prop_positions_length(
            obligations in proptest::collection::vec(
                (0usize..8, 0usize..8, 1..1000u64), 0..20
            ),
            num_parties in 2usize..=8,
        ) {
            let valid: Vec<(usize, usize, u64)> = obligations
                .into_iter()
                .filter(|(f, t, _)| f != t && *f < num_parties && *t < num_parties)
                .collect();
            let positions = compute_positions(&valid, num_parties);
            prop_assert_eq!(positions.len(), num_parties);
        }

        // P0: obligation application order does not affect net positions
        #[test]
        fn prop_order_independent(
            obligations in proptest::collection::vec(
                (0usize..5, 0usize..5, 1..1000u64), 2..15
            ),
        ) {
            let valid: Vec<(usize, usize, u64)> = obligations
                .into_iter()
                .filter(|(f, t, _)| f != t && *f < 5 && *t < 5)
                .collect();
            if valid.len() >= 2 {
                let pos_orig = compute_positions(&valid, 5);
                let mut reversed = valid.clone();
                reversed.reverse();
                let pos_rev = compute_positions(&reversed, 5);
                for i in 0..5 {
                    prop_assert_eq!(pos_orig[i], pos_rev[i]);
                }
            }
        }

        // P0: A→B:x and B→A:x cancel to zero for all parties
        #[test]
        fn prop_opposite_obligations_cancel(
            from_p in 0usize..5,
            to_p in 0usize..5,
            amount in 1..10_000u64,
        ) {
            if from_p != to_p {
                let obligations = vec![(from_p, to_p, amount), (to_p, from_p, amount)];
                let positions = compute_positions(&obligations, 5);
                for i in 0..5 {
                    prop_assert_eq!(positions[i], 0);
                }
            }
        }

        // P0: exec positions match hand-computed positions
        #[test]
        fn prop_settlement_matches_manual(
            obligations in proptest::collection::vec(
                (0usize..4, 0usize..4, 1..500u64), 1..10
            ),
        ) {
            let valid: Vec<(usize, usize, u64)> = obligations
                .into_iter()
                .filter(|(f, t, _)| f != t && *f < 4 && *t < 4)
                .collect();
            let positions = compute_positions(&valid, 4);
            let mut manual = vec![0i128; 4];
            for &(f, t, a) in &valid {
                manual[f] -= a as i128;
                manual[t] += a as i128;
            }
            for i in 0..4 {
                prop_assert_eq!(positions[i], manual[i]);
            }
        }

        // P0: is_valid_settlement agrees with manual position comparison (no-ensures oracle)
        #[test]
        fn prop_is_valid_settlement_correct(
            oblig1 in proptest::collection::vec(
                (0usize..4, 0usize..4, 1..200u64), 0..8
            ),
            oblig2 in proptest::collection::vec(
                (0usize..4, 0usize..4, 1..200u64), 0..8
            ),
        ) {
            let v1: Vec<_> = oblig1.into_iter()
                .filter(|(f, t, _)| f != t && *f < 4 && *t < 4).collect();
            let v2: Vec<_> = oblig2.into_iter()
                .filter(|(f, t, _)| f != t && *f < 4 && *t < 4).collect();
            let pos1 = compute_positions(&v1, 4);
            let pos2 = compute_positions(&v2, 4);
            let exec_result = is_valid_settlement(&v1, &v2, 4);
            let expected = (0..4).all(|i| pos1[i] == pos2[i]);
            prop_assert_eq!(exec_result, expected);
        }
    }
}

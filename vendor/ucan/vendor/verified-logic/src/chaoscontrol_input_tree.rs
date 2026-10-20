//! Verified ChaosControl input-tree alternative generation primitives.
//!
//! Models normalized choice tuples, exact rational score ordering via
//! denominator comparison, lexicographic tie-breaks, and budget-prefix
//! deterministic output.

use vstd::prelude::*;

verus! {

/// A recorded choice from the input tree.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceRecord {
    pub vm_id: u32,
    pub sequence_id: u64,
    pub n_options: u32,
    pub original_value: u64,
}

/// An emitted alternative.
#[derive(Debug, Clone, Copy)]
pub struct Alternative {
    pub vm_id: u32,
    pub sequence_id: u64,
    pub alternative_value: u64,
}

// ── Spec helpers ────────────────────────────────────────────────────

/// A single choice record is valid.
pub open spec fn valid_choice_spec(c: ChoiceRecord) -> bool {
    &&& c.n_options >= 2
    &&& (c.original_value as int) < (c.n_options as int)
    &&& (c.sequence_id as int) <= u64::MAX as int - 100
    &&& (c.n_options as u64 as int) * (100 + c.sequence_id as int) <= u64::MAX as int
}

/// A batch of choices has unique (vm_id, sequence_id) pairs.
pub open spec fn unique_ids_from_spec(batch: Seq<ChoiceRecord>, from: int) -> bool
    decreases batch.len() - from
{
    if from >= batch.len() { true }
    else {
        &&& forall|j: int| #![trigger batch[j]]
            from < j < batch.len() ==>
            (batch[from].vm_id != batch[j].vm_id || batch[from].sequence_id != batch[j].sequence_id)
        &&& unique_ids_from_spec(batch, from + 1)
    }
}

/// A batch is valid.
pub open spec fn valid_batch_spec(batch: Seq<ChoiceRecord>) -> bool {
    &&& forall|i: int| #![trigger batch[i]] 0 <= i < batch.len() ==> valid_choice_spec(batch[i])
    &&& unique_ids_from_spec(batch, 0)
}

/// Score denominator: n_options * (100 + sequence_id). Smaller denominator = higher score.
pub open spec fn score_denom_spec(c: ChoiceRecord) -> int {
    c.n_options as int * (100 + c.sequence_id as int)
}

/// Choice a has higher or equal score than b (smaller or equal denominator).
pub open spec fn score_ge_choice_spec(a: ChoiceRecord, b: ChoiceRecord) -> bool {
    score_denom_spec(a) <= score_denom_spec(b)
}

/// Lexicographic tie-break: a <= b by (vm_id, sequence_id).
pub open spec fn lex_le_spec(a: ChoiceRecord, b: ChoiceRecord) -> bool {
    ||| (a.vm_id as int) < (b.vm_id as int)
    ||| (a.vm_id == b.vm_id && a.sequence_id <= b.sequence_id)
}

/// Full ordering for normalized sort: higher score first, then lex tie-break.
pub open spec fn choice_le_spec(a: ChoiceRecord, b: ChoiceRecord) -> bool {
    ||| score_denom_spec(a) < score_denom_spec(b)
    ||| (score_denom_spec(a) == score_denom_spec(b) && lex_le_spec(a, b))
}

/// Count of alternatives for a single choice: n_options - 1 (excluding original).
pub open spec fn alt_count_spec(c: ChoiceRecord) -> int {
    c.n_options as int - 1
}

/// Full stream length: sum of alt_count over all choices.
pub open spec fn full_stream_len_spec(batch: Seq<ChoiceRecord>, from: int) -> int
    decreases batch.len() - from
{
    if from >= batch.len() { 0int }
    else { alt_count_spec(batch[from]) + full_stream_len_spec(batch, from + 1) }
}

// ── Private proof helpers ───────────────────────────────────────────

// r[verify chaoscontrol-input-tree.alternatives]
/// Helper: a >= q+1 and d >= 1 and m == q*d + r and r < d ==> a*d > m.
proof fn gt_from_quotient(a: int, d: int, m: int, q: int, r: int)
    requires a >= q + 1, d >= 1, q >= 0, r >= 0, r < d, m == q * d + r
    ensures a * d > m
{
    assert(a * d >= (q + 1) * d) by(nonlinear_arith)
        requires a >= q + 1, d >= 1;
    assert((q + 1) * d == q * d + d) by(nonlinear_arith)
        requires q >= 0, d >= 1;
}

/// a <= floor(m/d) <==> a*d <= m, for d >= 1, a >= 0, m >= 0.
proof fn div_mul_equiv(a: int, d: int, m: int)
    requires d >= 1, a >= 0, m >= 0
    ensures (a <= m / d) <==> (a * d <= m)
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(m, d);
    let q = m / d;
    let r = m % d;
    // lemma gives: m == d * q + r, 0 <= r < d
    assert(m == d * q + r);
    // Forward: a <= q ==> a * d <= m
    if a <= q {
        assert(d * a <= d * q) by(nonlinear_arith)
            requires a >= 0, a <= q, d >= 1;
        assert(d * a <= m);
    }
    // Backward: a * d <= m ==> a <= q
    if a > q {
        assert(a >= q + 1);
        assert(d * a >= d * (q + 1)) by(nonlinear_arith)
            requires a >= q + 1, d >= 1;
        assert(d * (q + 1) == d * q + d) by(nonlinear_arith)
            requires q >= 0, d >= 1;
        assert(d * a > m);
    }
}

// ── Exec functions ──────────────────────────────────────────────────

// r[impl chaoscontrol-input-tree.alternatives]
/// Validate a single choice record.
pub fn valid_choice(c: &ChoiceRecord) -> (result: bool)
    ensures result == valid_choice_spec(*c)
{
    if c.n_options < 2 {
        return false;
    }
    if c.original_value >= c.n_options as u64 {
        return false;
    }
    if c.sequence_id > u64::MAX - 100 {
        return false;
    }
    let d: u64 = 100u64 + c.sequence_id;
    let a: u64 = c.n_options as u64;
    proof {
        let dd = d as int;
        let aa = a as int;
        let mx = u64::MAX as int;
        let qq = mx / dd;
        let rr = mx % dd;
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(mx, dd);
        // mx == qq * dd + rr, 0 <= rr < dd
        div_mul_equiv(aa, dd, mx);
    }
    a <= u64::MAX / d
}

// r[impl chaoscontrol-input-tree.alternatives]
/// Score comparison: higher score = smaller denominator.
pub fn score_ge_choice(a: &ChoiceRecord, b: &ChoiceRecord) -> (result: bool)
    requires valid_choice_spec(*a), valid_choice_spec(*b)
    ensures result == score_ge_choice_spec(*a, *b)
{
    let a_denom = (a.n_options as u64) * (100u64 + a.sequence_id);
    let b_denom = (b.n_options as u64) * (100u64 + b.sequence_id);
    a_denom <= b_denom
}

// r[impl chaoscontrol-input-tree.alternatives]
/// Full ordering comparator for sort.
pub fn choice_le(a: &ChoiceRecord, b: &ChoiceRecord) -> (result: bool)
    requires valid_choice_spec(*a), valid_choice_spec(*b)
    ensures result == choice_le_spec(*a, *b)
{
    let a_denom = (a.n_options as u64) * (100u64 + a.sequence_id);
    let b_denom = (b.n_options as u64) * (100u64 + b.sequence_id);
    if a_denom < b_denom {
        true
    } else if a_denom > b_denom {
        false
    } else {
        // Equal denominators: lex tie-break by (vm_id, sequence_id)
        if a.vm_id < b.vm_id {
            true
        } else if a.vm_id > b.vm_id {
            false
        } else {
            a.sequence_id <= b.sequence_id
        }
    }
}

} // verus!

// r[impl chaoscontrol-input-tree.alternatives]
/// Generate alternatives from a valid, sorted batch, truncated to budget.
pub fn generate_alternatives(batch: &[ChoiceRecord], budget: usize) -> Vec<Alternative> {
    let mut result = Vec::new();
    for choice in batch {
        if result.len() >= budget {
            break;
        }
        for v in 0..choice.n_options as u64 {
            if v == choice.original_value {
                continue;
            }
            if result.len() >= budget {
                break;
            }
            result.push(Alternative {
                vm_id: choice.vm_id,
                sequence_id: choice.sequence_id,
                alternative_value: v,
            });
        }
    }
    result
}

// r[impl chaoscontrol-input-tree.alternatives]
/// Normalize a batch: validate, sort by score ordering.
/// Returns None if any record is invalid.
pub fn normalize_choice_batch(mut batch: Vec<ChoiceRecord>) -> Option<Vec<ChoiceRecord>> {
    for c in &batch {
        if !valid_choice(c) {
            return None;
        }
    }
    // Check unique (vm_id, sequence_id) pairs
    for i in 0..batch.len() {
        for j in (i + 1)..batch.len() {
            if batch[i].vm_id == batch[j].vm_id && batch[i].sequence_id == batch[j].sequence_id {
                return None;
            }
        }
    }
    // Insertion sort by choice_le
    let len = batch.len();
    for i in 1..len {
        let mut j = i;
        while j > 0 && !choice_le(&batch[j - 1], &batch[j]) {
            batch.swap(j - 1, j);
            j -= 1;
        }
    }
    Some(batch)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(vm: u32, seq: u64, opts: u32, orig: u64) -> ChoiceRecord {
        ChoiceRecord {
            vm_id: vm,
            sequence_id: seq,
            n_options: opts,
            original_value: orig,
        }
    }

    #[test]
    fn test_valid_choice() {
        assert!(valid_choice(&choice(0, 0, 3, 1)));
        // n_options < 2
        assert!(!valid_choice(&choice(0, 0, 1, 0)));
        // original_value >= n_options
        assert!(!valid_choice(&choice(0, 0, 3, 3)));
    }

    #[test]
    fn test_score_ordering() {
        // n_options=2, seq=0: denom = 2*100 = 200
        // n_options=4, seq=0: denom = 4*100 = 400
        // Smaller denom = higher score
        let a = choice(0, 0, 2, 0);
        let b = choice(0, 0, 4, 0);
        assert!(score_ge_choice(&a, &b)); // 200 <= 400
        assert!(!score_ge_choice(&b, &a)); // 400 > 200
    }

    #[test]
    fn test_lex_tie_break() {
        // Same denominator: n_options=2, seq=0 => denom=200 for both
        let a = choice(1, 0, 2, 0);
        let b = choice(2, 0, 2, 0);
        assert!(choice_le(&a, &b)); // vm_id 1 < 2
        assert!(!choice_le(&b, &a));
    }

    #[test]
    fn test_original_excluded() {
        let batch = vec![choice(0, 0, 3, 1)];
        let alts = generate_alternatives(&batch, 100);
        assert_eq!(alts.len(), 2); // values 0 and 2
        assert!(alts.iter().all(|a| a.alternative_value != 1));
    }

    #[test]
    fn test_budget_prefix() {
        let batch = vec![choice(0, 0, 5, 2)]; // 4 alternatives: 0,1,3,4
        let alts = generate_alternatives(&batch, 2);
        assert_eq!(alts.len(), 2);
        assert_eq!(alts[0].alternative_value, 0);
        assert_eq!(alts[1].alternative_value, 1);
    }

    #[test]
    fn test_normalize_sorts_by_score() {
        let batch = vec![
            choice(0, 0, 4, 0), // denom=400
            choice(1, 0, 2, 0), // denom=200 (higher score)
        ];
        let sorted = normalize_choice_batch(batch).unwrap();
        assert_eq!(sorted[0].n_options, 2); // higher score first
        assert_eq!(sorted[1].n_options, 4);
    }

    #[test]
    fn test_normalize_rejects_duplicate_ids() {
        let batch = vec![
            choice(0, 0, 3, 1),
            choice(0, 0, 4, 2), // duplicate (vm_id=0, seq=0)
        ];
        assert!(normalize_choice_batch(batch).is_none());
    }

    #[test]
    fn test_higher_score_emitted_first() {
        let batch = vec![
            choice(1, 0, 2, 0), // denom=200, higher score
            choice(0, 0, 4, 0), // denom=400, lower score
        ];
        let sorted = normalize_choice_batch(batch).unwrap();
        let alts = generate_alternatives(&sorted, 10);
        // First alt should come from the n_options=2 choice (higher score)
        assert_eq!(alts[0].vm_id, 1); // from higher-score choice
    }

    #[test]
    fn test_empty_batch() {
        let batch: Vec<ChoiceRecord> = vec![];
        let sorted = normalize_choice_batch(batch).unwrap();
        let alts = generate_alternatives(&sorted, 10);
        assert_eq!(alts.len(), 0);
    }
}

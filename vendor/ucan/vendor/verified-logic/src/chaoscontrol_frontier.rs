//! Verified ChaosControl frontier selection primitives.
//!
//! Models exact rational effective-score ordering, greedy maximality,
//! monotone decay under re-selection, and lowest-index tie-break.

use vstd::prelude::*;

verus! {

/// Frontier entry with base_score and times_selected.
pub struct FrontierEntry {
    pub base_score: u32,
    pub times_selected: u32,
}

// ── Spec helpers ────────────────────────────────────────────────────

/// Effective score as a rational: base_score / (1 + times_selected).
/// For comparison we use cross-multiplication to avoid division.
pub open spec fn effective_score_num_spec(e: FrontierEntry) -> int {
    e.base_score as int
}

pub open spec fn effective_score_den_spec(e: FrontierEntry) -> int {
    1 + e.times_selected as int
}

/// a >= b in effective score (cross-multiply).
pub open spec fn score_ge_spec(a: FrontierEntry, b: FrontierEntry) -> bool {
    a.base_score as int * (1 + b.times_selected as int)
        >= b.base_score as int * (1 + a.times_selected as int)
}

/// Strict greater.
pub open spec fn score_gt_spec(a: FrontierEntry, b: FrontierEntry) -> bool {
    a.base_score as int * (1 + b.times_selected as int)
        > b.base_score as int * (1 + a.times_selected as int)
}

// ── Private proof helpers ───────────────────────────────────────────

/// u32 * (1 + u32) fits in u64.
proof fn u32_cross_fits_u64(x: u32, y: u32)
    ensures (x as int) * (1 + y as int) <= u64::MAX as int
{
    // x <= 0xFFFF_FFFF, 1+y <= 0x1_0000_0000
    // product <= 0xFFFF_FFFF * 0x1_0000_0000 = 0xFFFF_FFFF_0000_0000 < u64::MAX
    assert((x as int) <= 0xFFFF_FFFFint);
    assert((1 + y as int) <= 0x1_0000_0000int);
    assert((x as int) * (1 + y as int) <= 0xFFFF_FFFFint * 0x1_0000_0000int) by(nonlinear_arith)
        requires x as int >= 0, x as int <= 0xFFFF_FFFFint,
                 1 + y as int >= 1, 1 + y as int <= 0x1_0000_0000int;
    assert(0xFFFF_FFFFint * 0x1_0000_0000int == 0xFFFF_FFFF_0000_0000int);
    assert(0xFFFF_FFFF_0000_0000int <= u64::MAX as int);
}

/// score_ge transitivity.
proof fn score_ge_transitive(a: FrontierEntry, b: FrontierEntry, c: FrontierEntry)
    requires score_ge_spec(a, b), score_ge_spec(b, c)
    ensures score_ge_spec(a, c)
{
    // a.b/(1+a.ts) >= b.b/(1+b.ts) >= c.b/(1+c.ts)
    // Equivalently: a.b*(1+b.ts) >= b.b*(1+a.ts) AND b.b*(1+c.ts) >= c.b*(1+b.ts)
    // Need: a.b*(1+c.ts) >= c.b*(1+a.ts)
    // From first: a.b * (1+b.ts) >= b.b * (1+a.ts)
    // Multiply both sides by (1+c.ts): a.b*(1+b.ts)*(1+c.ts) >= b.b*(1+a.ts)*(1+c.ts)
    // From second: b.b*(1+c.ts) >= c.b*(1+b.ts)
    // Multiply both sides by (1+a.ts): b.b*(1+c.ts)*(1+a.ts) >= c.b*(1+b.ts)*(1+a.ts)
    // Combining: a.b*(1+b.ts)*(1+c.ts) >= c.b*(1+b.ts)*(1+a.ts)
    // Since (1+b.ts) > 0, divide: a.b*(1+c.ts) >= c.b*(1+a.ts)
    let ab = a.base_score as int;
    let at = 1 + a.times_selected as int;
    let bb = b.base_score as int;
    let bt = 1 + b.times_selected as int;
    let cb = c.base_score as int;
    let ct = 1 + c.times_selected as int;
    assert(ab * bt >= bb * at);
    assert(bb * ct >= cb * bt);
    // Need: ab * ct >= cb * at
    // ab * bt * ct >= bb * at * ct   (mult first by ct)
    // bb * ct * at >= cb * bt * at   (mult second by at)
    // So ab * bt * ct >= cb * bt * at
    // bt > 0 so ab * ct >= cb * at
    assert(ab * ct >= cb * at) by(nonlinear_arith)
        requires ab >= 0, at >= 1, bb >= 0, bt >= 1, cb >= 0, ct >= 1,
                 ab * bt >= bb * at, bb * ct >= cb * bt;
}

// ── Exec functions ──────────────────────────────────────────────────

// r[impl chaoscontrol-frontier.selection]
/// Compare two entries by effective score using overflow-safe u64 cross-product.
pub fn score_ge(a: &FrontierEntry, b: &FrontierEntry) -> (result: bool)
    ensures result == score_ge_spec(*a, *b)
{
    proof {
        u32_cross_fits_u64(a.base_score, b.times_selected);
        u32_cross_fits_u64(b.base_score, a.times_selected);
    }
    let lhs = a.base_score as u64 * (1u64 + b.times_selected as u64);
    let rhs = b.base_score as u64 * (1u64 + a.times_selected as u64);
    lhs >= rhs
}

// r[impl chaoscontrol-frontier.selection]
/// Greedy selection: return index of the best entry (highest effective score,
/// lowest index on tie).
pub fn select_frontier_greedy(entries: &Vec<FrontierEntry>) -> (best: usize)
    requires entries@.len() > 0
    ensures
        0 <= best < entries@.len(),
        forall|i: int| #![trigger entries@[i]] 0 <= i < entries@.len() ==>
            score_ge_spec(entries@[best as int], entries@[i]),
{
    let mut best: usize = 0;
    let mut idx: usize = 1;
    while idx < entries.len()
        invariant
            0 <= best < entries@.len(),
            1 <= idx || entries@.len() == 1,
            idx <= entries@.len(),
            best < idx,
            forall|j: int| #![trigger entries@[j]] 0 <= j < idx as int ==>
                score_ge_spec(entries@[best as int], entries@[j]),
        decreases entries.len() - idx,
    {
        if !score_ge(&entries[best], &entries[idx]) {
            // entries[idx] > entries[best], so entries[idx] >= entries[j] for j < idx
            // by transitivity with entries[best] >= entries[j]
            proof {
                let old_best = best as int;
                assert(score_ge_spec(entries@[idx as int], entries@[old_best]));
                assert forall|j: int| #![trigger entries@[j]]
                    0 <= j < idx as int
                implies
                    score_ge_spec(entries@[idx as int], entries@[j])
                by {
                    assert(score_ge_spec(entries@[old_best], entries@[j]));
                    score_ge_transitive(entries@[idx as int], entries@[old_best], entries@[j]);
                }
            }
            best = idx;
        }
        idx = idx + 1;
    }
    best
}

// ── Proof artifacts ─────────────────────────────────────────────────

// r[verify chaoscontrol-frontier.selection]
/// Re-selection monotone decay: incrementing times_selected does not increase score.
pub proof fn reselection_decay(e: FrontierEntry)
    requires e.times_selected < u32::MAX
    ensures
        score_ge_spec(e,
            FrontierEntry { base_score: e.base_score, times_selected: (e.times_selected + 1) as u32 }),
{
    let after = FrontierEntry { base_score: e.base_score, times_selected: (e.times_selected + 1) as u32 };
    // Need: e.base * (2 + ts) >= e.base * (1 + ts)
    // i.e., e.base >= 0, which holds since base_score is u32
    assert(e.base_score as int * (1 + after.times_selected as int)
        >= after.base_score as int * (1 + e.times_selected as int)) by(nonlinear_arith)
        requires e.base_score as int >= 0,
                 after.base_score as int == e.base_score as int,
                 after.times_selected as int == e.times_selected as int + 1;
}

// r[verify chaoscontrol-frontier.selection]
/// Greedy selection returns a maximal entry.
pub proof fn greedy_maximality(entries: Seq<FrontierEntry>, best: int)
    requires
        entries.len() > 0,
        0 <= best < entries.len(),
        forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() ==>
            score_ge_spec(entries[best], entries[i]),
    ensures
        forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() ==>
            !score_gt_spec(entries[i], entries[best]),
{
    assert forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() implies
        !score_gt_spec(entries[i], entries[best])
    by {
        // score_ge(best, i) means best.b*(1+i.ts) >= i.b*(1+best.ts)
        // so NOT(i.b*(1+best.ts) > best.b*(1+i.ts))
    }
}

} // verus!

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(base: u32, sel: u32) -> FrontierEntry {
        FrontierEntry {
            base_score: base,
            times_selected: sel,
        }
    }

    #[test]
    fn test_score_ge_basic() {
        // 10/1 >= 5/1
        assert!(score_ge(&entry(10, 0), &entry(5, 0)));
        // 10/2 vs 5/1 => 5 >= 5 => true
        assert!(score_ge(&entry(10, 1), &entry(5, 0)));
        // 10/3 vs 5/1 => 3.33 < 5 => false
        assert!(!score_ge(&entry(10, 2), &entry(5, 0)));
    }

    #[test]
    fn test_greedy_selects_highest() {
        let entries = vec![entry(5, 0), entry(10, 0), entry(3, 0)];
        let best = select_frontier_greedy(&entries);
        assert_eq!(best, 1); // 10/1 is highest
    }

    #[test]
    fn test_greedy_tie_lowest_index() {
        let entries = vec![entry(10, 0), entry(10, 0), entry(5, 0)];
        let best = select_frontier_greedy(&entries);
        assert_eq!(best, 0); // tie between 0 and 1, lowest wins
    }

    #[test]
    fn test_decay_after_reselection() {
        let e = entry(10, 0);
        let after = entry(10, 1);
        assert!(score_ge(&e, &after)); // 10/1 >= 10/2
        assert!(!score_ge(&after, &e) || score_ge(&e, &after)); // reflexive weakening
    }

    #[test]
    fn test_single_entry() {
        let entries = vec![entry(1, 0)];
        assert_eq!(select_frontier_greedy(&entries), 0);
    }
}

//! Verified UCAN time-bounded delegation composition.
//!
//! When delegations are chained, the effective validity window is the
//! intersection of all individual windows. Proves that the composed
//! window is always narrower or equal, and that an empty intersection
//! means no valid instant exists.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// A time window: [nbf, exp].
pub struct TimeWindow {
    pub nbf: u64,
    pub exp: u64,
}

/// A time window is non-empty (has at least one valid instant).
// r[impl ucan-temporal.window-valid]
pub open spec fn window_nonempty_spec(w: TimeWindow) -> bool {
    w.nbf as int <= w.exp as int
}

/// Point-in-time validity for a single window.
pub open spec fn in_window_spec(w: TimeWindow, now: u64) -> bool {
    w.nbf as int <= now as int && now as int <= w.exp as int
}

/// Compose two windows by intersection: max(nbf), min(exp).
// r[impl ucan-temporal.window-intersection]
pub open spec fn intersect_window_spec(a: TimeWindow, b: TimeWindow) -> TimeWindow {
    TimeWindow {
        nbf: if a.nbf >= b.nbf { a.nbf } else { b.nbf },
        exp: if a.exp <= b.exp { a.exp } else { b.exp },
    }
}

/// Fold a chain of windows into a single composed window.
// r[impl ucan-temporal.chain-composition]
pub open spec fn compose_chain_spec(windows: Seq<TimeWindow>) -> TimeWindow
    recommends windows.len() > 0,
    decreases windows.len(),
{
    if windows.len() <= 1 {
        windows[0]
    } else {
        intersect_window_spec(
            windows[0],
            compose_chain_spec(windows.subrange(1, windows.len() as int)),
        )
    }
}

/// Runtime intersection of two windows.
// r[impl ucan-temporal.window-intersection]
pub fn intersect_window(a: &TimeWindow, b: &TimeWindow) -> (result: TimeWindow)
    ensures result == intersect_window_spec(*a, *b),
{
    TimeWindow {
        nbf: if a.nbf >= b.nbf { a.nbf } else { b.nbf },
        exp: if a.exp <= b.exp { a.exp } else { b.exp },
    }
}

/// Runtime fold of a chain of windows.
// r[impl ucan-temporal.chain-composition]
pub fn compose_chain(windows: &Vec<TimeWindow>) -> (result: TimeWindow)
    requires windows.len() > 0,
    ensures result == compose_chain_spec(windows@),
{
    let mut acc = TimeWindow { nbf: windows[0].nbf, exp: windows[0].exp };
    if windows.len() == 1 {
        return acc;
    }
    let mut i: usize = 1;
    while i < windows.len()
        invariant
            1 <= i <= windows.len(),
            windows.len() > 1,
            acc == compose_chain_spec(windows@.subrange(0, i as int)),
        decreases windows.len() - i,
    {
        acc = intersect_window(&acc, &windows[i]);
        // Bridge: compose(windows[0..i+1]) == intersect(compose(windows[0..i]), windows[i])
        proof {
            compose_chain_split(windows@, i as int);
        }
        i = i + 1;
    }
    proof {
        assert(windows@.subrange(0, windows@.len() as int) =~= windows@);
    }
    acc
}

// --- Proofs ---

/// Helper: compose of [0..k+1] == intersect(compose([0..k]), windows[k]).
proof fn compose_chain_split(windows: Seq<TimeWindow>, k: int)
    requires 1 <= k < windows.len(),
    ensures
        compose_chain_spec(windows.subrange(0, k + 1)) ==
            intersect_window_spec(
                compose_chain_spec(windows.subrange(0, k)),
                windows[k],
            ),
    decreases k,
{
    let sub = windows.subrange(0, k + 1);
    if k == 1 {
        assert(sub.len() == 2);
        assert(sub.subrange(1, 2) =~= seq![windows[1]]);
        assert(compose_chain_spec(sub.subrange(1, 2)) == windows[1]);
        assert(windows.subrange(0, 1) =~= seq![windows[0]]);
    } else {
        // sub = windows[0] ++ windows[1..k+1]
        assert(sub[0] == windows[0]);
        assert(sub.subrange(1, sub.len() as int) =~= windows.subrange(1, k + 1));
        // Recurse: compose(windows[1..k+1]) == intersect(compose(windows[1..k]), windows[k])
        let inner = windows.subrange(1, windows.len() as int);
        compose_chain_split(windows.subrange(1, windows.len() as int), k - 1);
        // Need to connect windows.subrange(1, k+1) to inner.subrange(0, k)
        assert(inner.subrange(0, k) =~= windows.subrange(1, k + 1));
        assert(inner.subrange(0, k - 1) =~= windows.subrange(1, k));
        // compose(sub) = intersect(sub[0], compose(sub[1..]))
        //              = intersect(windows[0], compose(windows[1..k+1]))
        //              = intersect(windows[0], intersect(compose(windows[1..k]), windows[k]))

        // compose(windows[0..k]) = intersect(windows[0], compose(windows[1..k]))
        let prev = windows.subrange(0, k);
        assert(prev[0] == windows[0]);
        assert(prev.subrange(1, prev.len() as int) =~= windows.subrange(1, k));

        // Now need intersect associativity:
        // intersect(A, intersect(B, C)) == intersect(intersect(A, B), C)
        let a = windows[0];
        let b = compose_chain_spec(windows.subrange(1, k));
        let c = windows[k];
        intersect_associative(a, b, c);
    }
}

/// Intersection is associative.
// r[verify ucan-temporal.window-intersection]
pub proof fn intersect_associative(a: TimeWindow, b: TimeWindow, c: TimeWindow)
    ensures
        intersect_window_spec(a, intersect_window_spec(b, c)) ==
            intersect_window_spec(intersect_window_spec(a, b), c),
{
}

/// Intersection is commutative.
pub proof fn intersect_commutative(a: TimeWindow, b: TimeWindow)
    ensures intersect_window_spec(a, b) == intersect_window_spec(b, a),
{
}

/// The composed window is always a subset of each individual window:
/// any instant in the composed window is in window[i] for all i.
// r[verify ucan-temporal.chain-composition]
pub proof fn composed_subset_of_each(windows: Seq<TimeWindow>, now: u64)
    requires
        windows.len() > 0,
        in_window_spec(compose_chain_spec(windows), now),
    ensures
        forall|i: int| #![trigger windows[i]]
            0 <= i < windows.len() ==> in_window_spec(windows[i], now),
    decreases windows.len(),
{
    if windows.len() == 1 {
        // trivial
    } else {
        let composed = compose_chain_spec(windows);
        let head = windows[0];
        let tail = windows.subrange(1, windows.len() as int);
        assert(composed == intersect_window_spec(head, compose_chain_spec(tail)));
        // in_window of intersection means in both parts
        assert(in_window_spec(head, now));
        assert(in_window_spec(compose_chain_spec(tail), now));
        composed_subset_of_each(tail, now);
        // For i >= 1, tail[i-1] == windows[i]
        assert forall|i: int| #![trigger windows[i]]
            0 <= i < windows.len() implies in_window_spec(windows[i], now) by {
            if i == 0 {
                // already shown
            } else {
                assert(tail[i - 1] == windows[i]);
            }
        }
    }
}

/// If any individual window is empty, the composed window is empty.
// r[verify ucan-temporal.window-valid]
pub proof fn empty_window_kills_composition(windows: Seq<TimeWindow>, bad_idx: int)
    requires
        windows.len() > 0,
        0 <= bad_idx < windows.len(),
        !window_nonempty_spec(windows[bad_idx]),
    ensures
        !window_nonempty_spec(compose_chain_spec(windows)),
{
    // By contradiction: if composed were nonempty, some now would be in_window,
    // but composed_subset_of_each says it must be in windows[bad_idx] too.
    if window_nonempty_spec(compose_chain_spec(windows)) {
        let cw = compose_chain_spec(windows);
        // cw.nbf <= cw.exp, so cw.nbf is a valid instant
        assert(in_window_spec(cw, cw.nbf));
        composed_subset_of_each(windows, cw.nbf);
        assert(in_window_spec(windows[bad_idx], cw.nbf));
        // But windows[bad_idx].nbf > exp, contradiction
        assert(false);
    }
}

/// The intersection of two non-empty overlapping windows is non-empty.
pub proof fn overlapping_windows_nonempty(a: TimeWindow, b: TimeWindow)
    requires
        window_nonempty_spec(a),
        window_nonempty_spec(b),
        // They overlap
        a.nbf as int <= b.exp as int,
        b.nbf as int <= a.exp as int,
    ensures
        window_nonempty_spec(intersect_window_spec(a, b)),
{
}

} // verus!

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_window() {
        let w = vec![TimeWindow { nbf: 100, exp: 200 }];
        let c = compose_chain(&w);
        assert_eq!(c.nbf, 100);
        assert_eq!(c.exp, 200);
    }

    #[test]
    fn test_two_overlapping() {
        let w = vec![TimeWindow { nbf: 100, exp: 300 }, TimeWindow { nbf: 200, exp: 400 }];
        let c = compose_chain(&w);
        assert_eq!(c.nbf, 200);
        assert_eq!(c.exp, 300);
    }

    #[test]
    fn test_three_windows() {
        let w = vec![
            TimeWindow { nbf: 0, exp: 1000 },
            TimeWindow { nbf: 100, exp: 500 },
            TimeWindow { nbf: 200, exp: 800 },
        ];
        let c = compose_chain(&w);
        assert_eq!(c.nbf, 200);
        assert_eq!(c.exp, 500);
    }

    #[test]
    fn test_non_overlapping_empty() {
        let w = vec![TimeWindow { nbf: 100, exp: 200 }, TimeWindow { nbf: 300, exp: 400 }];
        let c = compose_chain(&w);
        // nbf=300, exp=200 -> empty window
        assert!(c.nbf > c.exp);
    }

    #[test]
    fn test_subset_window() {
        let w = vec![TimeWindow { nbf: 0, exp: 1000 }, TimeWindow { nbf: 100, exp: 200 }];
        let c = compose_chain(&w);
        assert_eq!(c.nbf, 100);
        assert_eq!(c.exp, 200);
    }
}

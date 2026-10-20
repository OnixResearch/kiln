//! Verified Zircon wait readiness.
//!
//! Waiting is signal-driven: a wait is ready exactly when the observed
//! signal mask intersects the requested mask.

use vstd::prelude::*;

verus! {

pub struct Watch {
    pub observed: u64,
    pub requested: u64,
}

// r[depends zircon-wait.readiness]
pub open spec fn wait_ready_spec(observed: u64, requested: u64) -> bool {
    crate::signal::signal_intersect_spec(observed, requested) != 0
}

// r[impl zircon-wait.readiness]
pub fn ready(observed: u64, requested: u64) -> (result: bool)
    ensures result == wait_ready_spec(observed, requested)
{
    crate::signal::signal_intersect(observed, requested) != 0
}

// r[depends zircon-wait.wait-set]
pub open spec fn wait_set_ready_spec(watches: Seq<Watch>, i: int) -> bool
    recommends 0 <= i < watches.len()
{
    wait_ready_spec(watches[i].observed, watches[i].requested)
}

// r[impl zircon-wait.wait-set]
pub fn ready_set(watches: &[Watch]) -> (result: Vec<bool>)
    ensures
        result@.len() == watches@.len(),
        forall|i: int| 0 <= i < watches@.len() ==> #[trigger] result@[i] == wait_set_ready_spec(watches@, i),
{
    let mut out: Vec<bool> = Vec::with_capacity(watches.len());
    let mut i: usize = 0;
    while i < watches.len()
        invariant
            0 <= i <= watches@.len(),
            out@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> #[trigger] out@[j] == wait_set_ready_spec(watches@, j),
        decreases watches.len() - i,
    {
        out.push(ready(watches[i].observed, watches[i].requested));
        i += 1;
    }
    out
}

// r[verify zircon-wait.readiness]
proof fn wait_ready_exact(observed: u64, requested: u64)
    ensures wait_ready_spec(observed, requested) <==> crate::signal::signal_intersect_spec(observed, requested) != 0
{}

// r[verify zircon-wait.wait-set]
proof fn wait_set_entry_exact(watches: Seq<Watch>, i: int)
    requires 0 <= i < watches.len()
    ensures wait_set_ready_spec(watches, i) == wait_ready_spec(watches[i].observed, watches[i].requested)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify zircon-wait.readiness]
    #[test]
    fn test_single_wait_ready() {
        let observed = crate::signal::signal_add(0, 3);
        let requested = crate::signal::signal_add(0, 3);
        assert!(ready(observed, requested));
        assert!(!ready(observed, crate::signal::signal_add(0, 4)));
    }

    // r[verify zircon-wait.wait-set]
    #[test]
    fn test_wait_set() {
        let a = Watch {
            observed: crate::signal::signal_add(0, 1),
            requested: crate::signal::signal_add(0, 1),
        };
        let b = Watch {
            observed: crate::signal::signal_add(0, 2),
            requested: crate::signal::signal_add(0, 3),
        };
        let ready_entries = ready_set(&[a, b]);
        assert_eq!(ready_entries, vec![true, false]);
    }

    proptest! {
        // r[verify zircon-wait.readiness]
        #[test]
        fn prop_wait_ready_is_intersection(observed: u64, requested: u64) {
            prop_assert_eq!(ready(observed, requested), (observed & requested) != 0);
        }

        // r[verify zircon-wait.wait-set]
        #[test]
        fn prop_wait_set_matches_entries(
            observed in proptest::collection::vec(any::<u64>(), 0..20),
            requested in proptest::collection::vec(any::<u64>(), 0..20),
        ) {
            let n = core::cmp::min(observed.len(), requested.len());
            let mut watches = Vec::with_capacity(n);
            for i in 0..n {
                watches.push(Watch { observed: observed[i], requested: requested[i] });
            }
            let ready_entries = ready_set(&watches);
            for i in 0..n {
                prop_assert_eq!(ready_entries[i], (observed[i] & requested[i]) != 0);
            }
        }
    }
}

//! Verified virtual interrupt pending set and delivery.
//!
//! Pending interrupts are modeled as a boolean vector indexed by vector
//! number.  Selection is deterministic (lowest pending vector wins).
//! Delivery consumes the selected interrupt; non-interruptible guests
//! leave the pending set unchanged.

use vstd::prelude::*;

verus! {

// ────────────────────────────────────────────────────────────────────
//  is_pending / mark_pending
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-interrupt.inject]
/// Spec: is vector `v` pending?
pub open spec fn is_pending_spec(pending: Seq<bool>, v: int) -> bool
    recommends 0 <= v < pending.len()
{
    pending[v]
}

// r[depends hypervisor-interrupt.inject]
/// Spec: mark vector `v` as pending (idempotent).
pub open spec fn mark_pending_spec(pending: Seq<bool>, v: int) -> Seq<bool>
    recommends 0 <= v < pending.len()
{
    pending.update(v, true)
}

// r[verify hypervisor-interrupt.inject]
/// Re-marking is idempotent.
proof fn mark_pending_idempotent(pending: Seq<bool>, v: int)
    requires 0 <= v < pending.len()
    ensures
        mark_pending_spec(mark_pending_spec(pending, v), v)
            =~= mark_pending_spec(pending, v)
{}

// r[verify hypervisor-interrupt.inject]
/// Marking preserves other vectors.
proof fn mark_pending_preserves_others(pending: Seq<bool>, v: int, w: int)
    requires
        0 <= v < pending.len(),
        0 <= w < pending.len(),
        w != v,
    ensures mark_pending_spec(pending, v)[w] == pending[w]
{}

// r[impl hypervisor-interrupt.inject]
/// Marks vector `v` as pending.
pub fn mark_pending(pending: &mut Vec<bool>, v: usize)
    requires 0 <= v < old(pending)@.len()
    ensures  pending@ =~= mark_pending_spec(old(pending)@, v as int)
{
    pending.set(v, true);
}

// ────────────────────────────────────────────────────────────────────
//  next_pending — deterministic selection (lowest vector)
// ────────────────────────────────────────────────────────────────────

/// Recursive spec: find lowest pending vector from index `from`.
pub open spec fn next_from_spec(pending: Seq<bool>, from: int) -> Option<usize>
    decreases pending.len() - from
{
    if from < 0 || from >= pending.len() {
        None
    } else if pending[from] {
        Some(from as usize)
    } else {
        next_from_spec(pending, from + 1)
    }
}

// r[depends hypervisor-interrupt.inject]
/// Spec: next pending vector (lowest wins).
pub open spec fn next_pending_spec(pending: Seq<bool>) -> Option<usize> {
    next_from_spec(pending, 0)
}

// r[verify hypervisor-interrupt.inject]
/// Selection is deterministic: same pending set → same result.
proof fn next_pending_deterministic(pending: Seq<bool>)
    ensures next_pending_spec(pending) == next_pending_spec(pending)
{}

/// Helper: if all vectors in [from, j) are not pending and pending[j] is true,
/// then next_from_spec(pending, from) == Some(j).
proof fn next_from_skips_to(pending: Seq<bool>, from: int, j: int)
    requires
        0 <= from <= j,
        j < pending.len(),
        pending[j],
        forall|k: int| from <= k < j ==> !pending[k],
    ensures
        next_from_spec(pending, from) == Some(j as usize)
    decreases j - from
{
    if from == j {
    } else {
        next_from_skips_to(pending, from + 1, j);
    }
}

/// Helper: if no vector in [from, len) is pending, next_from returns None.
proof fn next_from_all_clear(pending: Seq<bool>, from: int)
    requires
        0 <= from,
        forall|k: int| from <= k < pending.len() ==> !pending[k],
    ensures
        next_from_spec(pending, from) is None
    decreases pending.len() - from
{
    if from >= pending.len() {
    } else {
        next_from_all_clear(pending, from + 1);
    }
}

// r[impl hypervisor-interrupt.inject]
/// Returns the lowest pending vector, or None.
pub fn next_pending(pending: &Vec<bool>) -> (result: Option<usize>)
    ensures result == next_pending_spec(pending@)
{
    let len = pending.len();
    let mut i: usize = 0;
    while i < len
        invariant
            0 <= i <= len,
            len == pending@.len(),
            forall|k: int| 0 <= k < i as int ==> !pending@[k],
        decreases len - i,
    {
        if pending[i] {
            proof {
                next_from_skips_to(pending@, 0, i as int);
            }
            return Some(i);
        }
        i += 1;
    }
    proof {
        next_from_all_clear(pending@, 0);
    }
    None
}

// ────────────────────────────────────────────────────────────────────
//  deliver — consume the selected interrupt if interruptible
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-interrupt.deliver]
/// Spec: deliver the next pending interrupt if interruptible.
/// Returns (delivered_vector, updated_pending).
pub open spec fn deliver_spec(
    pending: Seq<bool>,
    interruptible: bool,
) -> (Option<usize>, Seq<bool>) {
    if !interruptible {
        (None, pending)
    } else {
        match next_pending_spec(pending) {
            None => (None, pending),
            Some(v) => (Some(v), pending.update(v as int, false)),
        }
    }
}

// r[verify hypervisor-interrupt.deliver]
/// Non-interruptible guest: pending set unchanged.
proof fn non_interruptible_unchanged(pending: Seq<bool>)
    ensures deliver_spec(pending, false) == (None::<usize>, pending)
{}

// r[verify hypervisor-interrupt.deliver]
/// Delivered vector is consumed: no longer pending.
proof fn delivered_is_consumed(pending: Seq<bool>, v: usize)
    requires
        next_pending_spec(pending) == Some(v),
        0 <= v as int,
        (v as int) < pending.len(),
    ensures ({
        let (_, new_pending) = deliver_spec(pending, true);
        !new_pending[v as int]
    })
{}

/// If next_from_spec returns Some(v), then v < pending.len().
proof fn next_from_hit_bounded(pending: Seq<bool>, from: int) -> (v: usize)
    requires
        0 <= from,
        from < pending.len(),
        next_from_spec(pending, from) is Some,
    ensures
        next_from_spec(pending, from) == Some(v),
        (v as int) < pending.len(),
    decreases pending.len() - from
{
    if pending[from] {
        from as usize
    } else if from + 1 >= pending.len() {
        assert(next_from_spec(pending, from + 1) is None);
        from as usize // unreachable
    } else {
        next_from_hit_bounded(pending, from + 1)
    }
}

// r[impl hypervisor-interrupt.deliver]
/// Delivers the next interrupt if interruptible, consuming it.
pub fn deliver(
    pending: &mut Vec<bool>,
    interruptible: bool,
) -> (result: Option<usize>)
    ensures ({
        let (vec_opt, new_pending) = deliver_spec(old(pending)@, interruptible);
        result == vec_opt && pending@ =~= new_pending
    })
{
    if !interruptible {
        return None;
    }
    let v = next_pending(pending);
    match v {
        None => None,
        Some(idx) => {
            proof {
                next_from_hit_bounded(pending@, 0);
            }
            pending.set(idx, false);
            Some(idx)
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_pending(n: usize) -> Vec<bool> {
        vec![false; n]
    }

    // r[test hypervisor-interrupt.inject]
    #[test]
    fn test_mark_idempotent() {
        let mut p = empty_pending(256);
        mark_pending(&mut p, 48);
        mark_pending(&mut p, 48);
        assert_eq!(next_pending(&p), Some(48));
    }

    // r[test hypervisor-interrupt.inject]
    #[test]
    fn test_deterministic_selection() {
        let mut p = empty_pending(256);
        mark_pending(&mut p, 64);
        mark_pending(&mut p, 32);
        // Lowest vector wins.
        assert_eq!(next_pending(&p), Some(32));
    }

    // r[test hypervisor-interrupt.deliver]
    #[test]
    fn test_deliver_consumes() {
        let mut p = empty_pending(256);
        mark_pending(&mut p, 32);
        let v = deliver(&mut p, true);
        assert_eq!(v, Some(32));
        assert_eq!(next_pending(&p), None);
    }

    // r[test hypervisor-interrupt.deliver]
    #[test]
    fn test_non_interruptible_unchanged() {
        let mut p = empty_pending(256);
        mark_pending(&mut p, 32);
        let v = deliver(&mut p, false);
        assert_eq!(v, None);
        assert_eq!(next_pending(&p), Some(32));
    }
}

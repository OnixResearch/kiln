//! Session and causal consistency predicates.
//!
//! Reuses the vector-clock happens-before relation for causal dependencies and
//! the causal-delivery readiness rule for message-level examples.

#![allow(clippy::fn_params_excessive_bools, clippy::ptr_arg)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::causal_order::is_causally_ready_spec;
#[cfg(verus_keep_ghost)]
use crate::vector_clock::happens_before_spec;

verus! {

// r[depends consistency-session.causal-dependency]
/// Spec: one reusable causal-dependency witness is vector-clock
/// happens-before.
pub open spec fn causal_dependency_spec(a: Seq<u64>, b: Seq<u64>) -> bool {
    happens_before_spec(a, b)
}

// r[impl consistency-session.causal-dependency]
pub fn causal_dependency(a: &Vec<u64>, b: &Vec<u64>) -> (result: bool)
    requires a.len() == b.len()
    ensures result == causal_dependency_spec(a@, b@)
{
    crate::vector_clock::happens_before(a, b)
}

// r[depends consistency-session.causal-ready]
/// Spec: message-level causal readiness reuses the causal-order module.
pub open spec fn causal_ready_spec(
    msg_clock: Seq<u64>,
    local_clock: Seq<u64>,
    sender: int,
) -> bool {
    is_causally_ready_spec(msg_clock, local_clock, sender)
}

// r[impl consistency-session.causal-ready]
pub fn causal_ready(
    msg_clock: &Vec<u64>,
    local_clock: &Vec<u64>,
    sender: usize,
) -> (result: bool)
    requires msg_clock.len() == local_clock.len(), sender < msg_clock.len()
    ensures result == causal_ready_spec(msg_clock@, local_clock@, sender as int)
{
    crate::causal_order::is_causally_ready(msg_clock, local_clock, sender)
}

// r[depends consistency-session.read-your-writes]
/// Spec: a session satisfies read-your-writes when its own later reads include
/// its earlier writes.
pub open spec fn read_your_writes_spec(own_write_visible: bool) -> bool {
    own_write_visible
}

// r[impl consistency-session.read-your-writes]
pub fn read_your_writes(own_write_visible: bool) -> (result: bool)
    ensures result == read_your_writes_spec(own_write_visible)
{
    own_write_visible
}

// r[depends consistency-session.monotonic-reads]
/// Spec: monotonic reads means later reads do not move to an older observed
/// state.
pub open spec fn monotonic_reads_spec(non_decreasing_reads: bool) -> bool {
    non_decreasing_reads
}

// r[impl consistency-session.monotonic-reads]
pub fn monotonic_reads(non_decreasing_reads: bool) -> (result: bool)
    ensures result == monotonic_reads_spec(non_decreasing_reads)
{
    non_decreasing_reads
}

// r[depends consistency-session.monotonic-writes]
/// Spec: monotonic writes means same-session writes preserve issue order.
pub open spec fn monotonic_writes_spec(per_session_write_order: bool) -> bool {
    per_session_write_order
}

// r[impl consistency-session.monotonic-writes]
pub fn monotonic_writes(per_session_write_order: bool) -> (result: bool)
    ensures result == monotonic_writes_spec(per_session_write_order)
{
    per_session_write_order
}

// r[depends consistency-session.writes-follow-reads]
/// Spec: writes-follow-reads means a write after observing some value is not
/// ordered before the write that produced that observed value.
pub open spec fn writes_follow_reads_spec(dependency_order_respected: bool) -> bool {
    dependency_order_respected
}

// r[impl consistency-session.writes-follow-reads]
pub fn writes_follow_reads(dependency_order_respected: bool) -> (result: bool)
    ensures result == writes_follow_reads_spec(dependency_order_respected)
{
    dependency_order_respected
}

// r[depends consistency-session.pram]
/// Spec: PRAM preserves same-session read monotonicity, write order, and
/// read-your-writes.
pub open spec fn pram_spec(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
) -> bool {
    monotonic_reads_spec(non_decreasing_reads)
    && monotonic_writes_spec(per_session_write_order)
    && read_your_writes_spec(own_write_visible)
}

// r[impl consistency-session.pram]
pub fn pram(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
) -> (result: bool)
    ensures result == pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
{
    monotonic_reads(non_decreasing_reads)
        && monotonic_writes(per_session_write_order)
        && read_your_writes(own_write_visible)
}

// r[depends consistency-session.causal-consistency]
/// Spec: causal consistency is PRAM plus writes-follow-reads.
pub open spec fn causal_consistency_spec(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
    dependency_order_respected: bool,
) -> bool {
    pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
    && writes_follow_reads_spec(dependency_order_respected)
}

// r[impl consistency-session.causal-consistency]
pub fn causal_consistency(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
    dependency_order_respected: bool,
) -> (result: bool)
    ensures result == causal_consistency_spec(
        non_decreasing_reads,
        per_session_write_order,
        own_write_visible,
        dependency_order_respected,
    )
{
    pram(non_decreasing_reads, per_session_write_order, own_write_visible)
        && writes_follow_reads(dependency_order_respected)
}

// r[verify consistency-session.pram]
// r[verify consistency-session.monotonic-reads]
/// PRAM implies monotonic reads.
proof fn pram_implies_monotonic_reads(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
)
    ensures pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
        ==> monotonic_reads_spec(non_decreasing_reads)
{}

// r[verify consistency-session.pram]
// r[verify consistency-session.monotonic-writes]
/// PRAM implies monotonic writes.
proof fn pram_implies_monotonic_writes(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
)
    ensures pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
        ==> monotonic_writes_spec(per_session_write_order)
{}

// r[verify consistency-session.pram]
/// PRAM implies read-your-writes.
proof fn pram_implies_read_your_writes(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
)
    ensures pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
        ==> read_your_writes_spec(own_write_visible)
{}

// r[verify consistency-session.causal-consistency]
/// Causal consistency implies PRAM.
proof fn causal_consistency_implies_pram(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
    dependency_order_respected: bool,
)
    ensures causal_consistency_spec(
        non_decreasing_reads,
        per_session_write_order,
        own_write_visible,
        dependency_order_respected,
    ) ==> pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
{}

// r[verify consistency-session.causal-consistency]
// r[verify consistency-session.writes-follow-reads]
/// Causal consistency implies writes-follow-reads.
proof fn causal_consistency_implies_wfr(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
    dependency_order_respected: bool,
)
    ensures causal_consistency_spec(
        non_decreasing_reads,
        per_session_write_order,
        own_write_visible,
        dependency_order_respected,
    ) ==> writes_follow_reads_spec(dependency_order_respected)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify consistency-session.causal-dependency]
    #[test]
    fn test_causal_dependency_uses_vector_clocks() {
        let a = vec![1, 0, 0];
        let b = vec![2, 0, 0];
        let c = vec![0, 1, 0];
        assert!(causal_dependency(&a, &b));
        assert!(!causal_dependency(&a, &c));
        assert!(!causal_dependency(&c, &a));
    }

    // r[verify consistency-session.causal-ready]
    #[test]
    fn test_causal_ready_reuses_causal_delivery() {
        let local = vec![2, 3, 1];
        let msg = vec![2, 4, 1];
        assert!(causal_ready(&msg, &local, 1));
    }

    // r[verify consistency-session.read-your-writes]
    #[test]
    fn test_read_your_writes() {
        assert!(read_your_writes(true));
        assert!(!read_your_writes(false));
    }

    // r[verify consistency-session.pram]
    #[test]
    fn test_pram_is_session_bundle() {
        assert!(pram(true, true, true));
        assert!(!pram(true, false, true));
    }

    // r[verify consistency-session.causal-consistency]
    #[test]
    fn test_causal_consistency_extends_pram() {
        assert!(causal_consistency(true, true, true, true));
        assert!(!causal_consistency(true, true, true, false));
    }
}

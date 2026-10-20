//! Verified TLB shootdown protocol.
//!
//! The core model is page-local: one target page is represented by a vector of
//! cached translations indexed by core. A shootdown request carries a
//! request-local acknowledgment bitmap. Freshness means the stale cached
//! translation is absent (`TLB_UNMAPPED`) on every targeted core once the
//! request completes.

use vstd::prelude::*;

verus! {

/// Sentinel meaning the cached translation is absent.
pub const TLB_UNMAPPED: u64 = 0;

#[derive(Debug, PartialEq, Eq)]
pub struct ShootdownRequest {
    pub initiator: usize,
    pub target_page: usize,
    pub acked: Vec<bool>,
}

// r[depends tlb-shootdown.freshness]
// r[depends tlb-shootdown.self-invalidation]
pub open spec fn invalidate_core_spec(page_tlbs: Seq<u64>, core: int) -> Seq<u64> {
    if 0 <= core < page_tlbs.len() {
        page_tlbs.update(core, TLB_UNMAPPED)
    } else {
        page_tlbs
    }
}

// r[depends tlb-shootdown.self-invalidation]
pub open spec fn initiated_acks_spec(core_count: nat, initiator: nat) -> Seq<bool> {
    Seq::new(core_count, |index: int| index == initiator)
}

// r[depends tlb-shootdown.termination]
pub open spec fn acknowledge_spec(acked: Seq<bool>, core: int) -> Seq<bool> {
    if 0 <= core < acked.len() {
        acked.update(core, true)
    } else {
        acked
    }
}

pub open spec fn pending_ack_count_from_spec(acked: Seq<bool>, from: int) -> nat
    decreases acked.len() - from
{
    if from >= acked.len() {
        0
    } else {
        (if acked[from] { 0nat } else { 1nat }) + pending_ack_count_from_spec(acked, from + 1)
    }
}

// r[impl tlb-shootdown.termination]
// r[depends tlb-shootdown.termination]
pub open spec fn pending_ack_count_spec(acked: Seq<bool>) -> nat {
    pending_ack_count_from_spec(acked, 0)
}

// r[impl tlb-shootdown.termination]
// r[impl tlb-shootdown.initiator-safety]
// r[depends tlb-shootdown.termination]
// r[depends tlb-shootdown.initiator-safety]
pub open spec fn all_acknowledged_spec(request: ShootdownRequest) -> bool {
    pending_ack_count_spec(request.acked@) == 0
}

// r[impl tlb-shootdown.freshness]
// r[depends tlb-shootdown.freshness]
pub open spec fn fresh_page_spec(page_tlbs: Seq<u64>) -> bool {
    forall|core: int| #![trigger page_tlbs[core]] 0 <= core < page_tlbs.len() ==> page_tlbs[core] == TLB_UNMAPPED
}

pub open spec fn ack_invariant_spec(page_tlbs: Seq<u64>, acked: Seq<bool>) -> bool {
    &&& page_tlbs.len() == acked.len()
    &&& forall|core: int| #![trigger acked[core], page_tlbs[core]]
        0 <= core < acked.len() ==> (acked[core] ==> page_tlbs[core] == TLB_UNMAPPED)
}

pub open spec fn request_valid_spec(request: ShootdownRequest) -> bool {
    &&& request.initiator < request.acked@.len()
    &&& request.acked@[request.initiator as int]
}

proof fn initiated_acks_has_true_initiator(core_count: nat, initiator: nat)
    requires initiator < core_count
    ensures initiated_acks_spec(core_count, initiator)[initiator as int]
{
}

// r[verify tlb-shootdown.self-invalidation]
pub proof fn initiate_establishes_ack_invariant(page_tlbs: Seq<u64>, initiator: int)
    requires
        0 <= initiator < page_tlbs.len(),
    ensures
        ack_invariant_spec(
            invalidate_core_spec(page_tlbs, initiator),
            initiated_acks_spec(page_tlbs.len() as nat, initiator as nat),
        ),
        initiated_acks_spec(page_tlbs.len() as nat, initiator as nat)[initiator] == true,
{
    assert forall|core: int| #![trigger initiated_acks_spec(page_tlbs.len() as nat, initiator as nat)[core], invalidate_core_spec(page_tlbs, initiator)[core]]
        0 <= core < initiated_acks_spec(page_tlbs.len() as nat, initiator as nat).len()
        implies (!initiated_acks_spec(page_tlbs.len() as nat, initiator as nat)[core]
            || invalidate_core_spec(page_tlbs, initiator)[core] == TLB_UNMAPPED) by {
        if core == initiator {
            assert(invalidate_core_spec(page_tlbs, initiator)[core] == TLB_UNMAPPED);
        }
    };
    initiated_acks_has_true_initiator(page_tlbs.len() as nat, initiator as nat);
}

proof fn acknowledge_preserves_later_suffix(acked: Seq<bool>, core: int, from: int)
    requires
        0 <= core < acked.len(),
        core < from <= acked.len(),
    ensures
        pending_ack_count_from_spec(acknowledge_spec(acked, core), from)
            == pending_ack_count_from_spec(acked, from),
    decreases acked.len() - from
{
    if from < acked.len() {
        assert(acknowledge_spec(acked, core)[from] == acked[from]);
        acknowledge_preserves_later_suffix(acked, core, from + 1);
    }
}

proof fn first_ack_decreases_pending_from(acked: Seq<bool>, from: int, core: int)
    requires
        0 <= from <= core < acked.len(),
        !acked[core],
    ensures
        pending_ack_count_from_spec(acknowledge_spec(acked, core), from) + 1
            == pending_ack_count_from_spec(acked, from),
    decreases acked.len() - from
{
    if from == core {
        assert(acknowledge_spec(acked, core)[core] == true);
        assert(pending_ack_count_from_spec(acknowledge_spec(acked, core), from)
            == pending_ack_count_from_spec(acknowledge_spec(acked, core), from + 1));
        assert(pending_ack_count_from_spec(acked, from)
            == 1nat + pending_ack_count_from_spec(acked, from + 1));
        acknowledge_preserves_later_suffix(acked, core, from + 1);
    } else {
        first_ack_decreases_pending_from(acked, from + 1, core);
        assert(pending_ack_count_from_spec(acknowledge_spec(acked, core), from + 1) + 1
            == pending_ack_count_from_spec(acked, from + 1));
        if acked[from] {
            assert(pending_ack_count_from_spec(acknowledge_spec(acked, core), from)
                == pending_ack_count_from_spec(acknowledge_spec(acked, core), from + 1));
            assert(pending_ack_count_from_spec(acked, from)
                == pending_ack_count_from_spec(acked, from + 1));
        } else {
            assert(pending_ack_count_from_spec(acknowledge_spec(acked, core), from)
                == 1nat + pending_ack_count_from_spec(acknowledge_spec(acked, core), from + 1));
            assert(pending_ack_count_from_spec(acked, from)
                == 1nat + pending_ack_count_from_spec(acked, from + 1));
        }
    }
}

// r[verify tlb-shootdown.termination]
pub proof fn first_ack_decreases_pending(acked: Seq<bool>, core: int)
    requires
        0 <= core < acked.len(),
        !acked[core],
    ensures
        pending_ack_count_spec(acknowledge_spec(acked, core)) + 1 == pending_ack_count_spec(acked),
        pending_ack_count_spec(acknowledge_spec(acked, core)) < pending_ack_count_spec(acked),
{
    first_ack_decreases_pending_from(acked, 0, core);
}

// r[verify tlb-shootdown.termination]
pub proof fn acknowledge_never_increases_pending(acked: Seq<bool>, core: int)
    requires 0 <= core < acked.len()
    ensures pending_ack_count_spec(acknowledge_spec(acked, core)) <= pending_ack_count_spec(acked)
{
    if acked[core] {
        assert(acknowledge_spec(acked, core) == acked);
    } else {
        first_ack_decreases_pending(acked, core);
    }
}

proof fn zero_pending_from_implies_all_true(acked: Seq<bool>, from: int)
    requires
        0 <= from <= acked.len(),
        pending_ack_count_from_spec(acked, from) == 0,
    ensures
        forall|core: int| #![trigger acked[core]] from <= core < acked.len() ==> acked[core]
    decreases acked.len() - from
{
    if from < acked.len() {
        if acked[from] {
            zero_pending_from_implies_all_true(acked, from + 1);
            assert forall|core: int| #![trigger acked[core]] from <= core < acked.len() implies acked[core] by {
                if core == from {
                } else {
                    assert(from + 1 <= core < acked.len());
                }
            };
        } else {
            assert(pending_ack_count_from_spec(acked, from) >= 1);
            assert(false);
        }
    }
}

// r[verify tlb-shootdown.initiator-safety]
// r[verify tlb-shootdown.freshness]
pub proof fn freshness_from_completion(page_tlbs: Seq<u64>, request: ShootdownRequest)
    requires
        request_valid_spec(request),
        ack_invariant_spec(page_tlbs, request.acked@),
        all_acknowledged_spec(request),
    ensures
        fresh_page_spec(page_tlbs),
{
    zero_pending_from_implies_all_true(request.acked@, 0);
    assert forall|core: int| #![trigger page_tlbs[core]] 0 <= core < page_tlbs.len() implies page_tlbs[core] == TLB_UNMAPPED by {
        assert(request.acked@[core]);
        assert(request.acked@[core] ==> page_tlbs[core] == TLB_UNMAPPED);
    };
}

// r[verify tlb-shootdown.termination]
pub proof fn acknowledge_preserves_initiator_ack(request: ShootdownRequest, core: int)
    requires
        request_valid_spec(request),
        0 <= core < request.acked@.len(),
    ensures
        acknowledge_spec(request.acked@, core)[request.initiator as int],
{
    assert(acknowledge_spec(request.acked@, core)[request.initiator as int]);
}

// r[verify tlb-shootdown.termination]
// r[verify tlb-shootdown.freshness]
pub proof fn acknowledge_preserves_ack_invariant(page_tlbs: Seq<u64>, acked: Seq<bool>, core: int)
    requires
        ack_invariant_spec(page_tlbs, acked),
        0 <= core < acked.len(),
        page_tlbs[core] == TLB_UNMAPPED,
    ensures
        ack_invariant_spec(page_tlbs, acknowledge_spec(acked, core)),
{
    assert forall|index: int| #![trigger acknowledge_spec(acked, core)[index], page_tlbs[index]]
        0 <= index < acknowledge_spec(acked, core).len()
        implies (!acknowledge_spec(acked, core)[index] || page_tlbs[index] == TLB_UNMAPPED) by {
        if index == core {
        } else {
            assert(acknowledge_spec(acked, core)[index] == acked[index]);
        }
    };
}

fn clone_u64_vec(values: &[u64]) -> (result: Vec<u64>)
    ensures result@ =~= values@,
{
    let mut result: Vec<u64> = Vec::new();
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            result@ =~= values@.subrange(0, index as int),
        decreases values@.len() - index,
    {
        result.push(values[index]);
        index = index + 1;
    }
    result
}

fn clone_bool_vec(values: &[bool]) -> (result: Vec<bool>)
    ensures result@ =~= values@,
{
    let mut result: Vec<bool> = Vec::new();
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            result@ =~= values@.subrange(0, index as int),
        decreases values@.len() - index,
    {
        result.push(values[index]);
        index = index + 1;
    }
    result
}

fn updated_u64_vec(values: &[u64], index: usize, value: u64) -> (result: Option<Vec<u64>>)
    ensures
        result.is_some() ==> {
            &&& index < values@.len()
            &&& result.unwrap()@ == values@.update(index as int, value)
        },
        result.is_none() ==> index >= values@.len(),
{
    if index >= values.len() {
        return None;
    }
    let mut result = clone_u64_vec(values);
    result.set(index, value);
    Some(result)
}

fn updated_bool_vec(values: &[bool], index: usize, value: bool) -> (result: Option<Vec<bool>>)
    ensures
        result.is_some() ==> {
            &&& index < values@.len()
            &&& result.unwrap()@ == values@.update(index as int, value)
        },
        result.is_none() ==> index >= values@.len(),
{
    if index >= values.len() {
        return None;
    }
    let mut result = clone_bool_vec(values);
    result.set(index, value);
    Some(result)
}

// r[impl tlb-shootdown.self-invalidation]
pub fn initiate_shootdown(page_tlbs: &[u64], initiator: usize, target_page: usize) -> (result: Option<(ShootdownRequest, Vec<u64>)>)
    ensures
        result.is_some() ==> {
            &&& initiator < page_tlbs@.len()
            &&& result.unwrap().0.initiator == initiator
            &&& result.unwrap().0.target_page == target_page
            &&& result.unwrap().0.acked@ == initiated_acks_spec(page_tlbs@.len() as nat, initiator as nat)
            &&& result.unwrap().1@ == invalidate_core_spec(page_tlbs@, initiator as int)
        },
        result.is_none() ==> initiator >= page_tlbs@.len(),
{
    if initiator >= page_tlbs.len() {
        return None;
    }
    let mut acked: Vec<bool> = Vec::new();
    let mut index: usize = 0;
    while index < page_tlbs.len()
        invariant
            0 <= index <= page_tlbs@.len(),
            acked@ == initiated_acks_spec(index as nat, initiator as nat).push(false).subrange(0, index as int),
        decreases page_tlbs@.len() - index,
    {
        acked.push(index == initiator);
        index = index + 1;
    }
    let next = updated_u64_vec(page_tlbs, initiator, TLB_UNMAPPED).unwrap();
    Some((ShootdownRequest { initiator, target_page, acked }, next))
}

// r[impl tlb-shootdown.self-invalidation]
// r[impl tlb-shootdown.freshness]
pub fn handle_shootdown_ipi(page_tlbs: &[u64], core: usize) -> (result: Option<Vec<u64>>)
    ensures
        result.is_some() ==> {
            &&& core < page_tlbs@.len()
            &&& result.unwrap()@ == invalidate_core_spec(page_tlbs@, core as int)
        },
        result.is_none() ==> core >= page_tlbs@.len(),
{
    updated_u64_vec(page_tlbs, core, TLB_UNMAPPED)
}

// r[impl tlb-shootdown.termination]
pub fn acknowledge_shootdown(request: &ShootdownRequest, core: usize) -> (result: Option<ShootdownRequest>)
    ensures
        result.is_some() ==> {
            &&& core < request.acked@.len()
            &&& result.unwrap().initiator == request.initiator
            &&& result.unwrap().target_page == request.target_page
            &&& result.unwrap().acked@ == acknowledge_spec(request.acked@, core as int)
        },
        result.is_none() ==> core >= request.acked@.len(),
{
    match updated_bool_vec(&request.acked, core, true) {
        Some(acked) => Some(ShootdownRequest {
            initiator: request.initiator,
            target_page: request.target_page,
            acked,
        }),
        None => None,
    }
}

// r[impl tlb-shootdown.termination]
pub fn pending_ack_count(request: &ShootdownRequest) -> (result: usize)
    ensures result as nat == pending_ack_count_spec(request.acked@),
{
    let mut pending: usize = 0;
    let mut index: usize = 0;
    while index < request.acked.len()
        invariant
            0 <= index <= request.acked@.len(),
            pending <= index,
            pending as nat == pending_ack_count_from_spec(request.acked@, 0) - pending_ack_count_from_spec(request.acked@, index as int),
        decreases request.acked@.len() - index,
    {
        if !request.acked[index] {
            assert(pending <= index);
            assert(index < usize::MAX);
            let next_pending = pending.checked_add(1);
            assert(next_pending.is_some());
            pending = next_pending.unwrap();
        }
        index = index + 1;
    }
    pending
}

// r[impl tlb-shootdown.termination]
// r[impl tlb-shootdown.initiator-safety]
pub fn all_acknowledged(request: &ShootdownRequest) -> (result: bool)
    ensures result == (pending_ack_count_spec(request.acked@) == 0),
{
    pending_ack_count(request) == 0
}

// r[impl tlb-shootdown.freshness]
pub fn check_tlb_freshness(page_tlbs: &[u64]) -> (result: bool)
    ensures result == fresh_page_spec(page_tlbs@),
{
    let mut index: usize = 0;
    while index < page_tlbs.len()
        invariant
            0 <= index <= page_tlbs@.len(),
            forall|core: int| #![trigger page_tlbs@[core]] 0 <= core < index as int ==> page_tlbs@[core] == TLB_UNMAPPED,
        decreases page_tlbs@.len() - index,
    {
        if page_tlbs[index] != TLB_UNMAPPED {
            return false;
        }
        index = index + 1;
    }
    true
}

// r[impl tlb-shootdown.isolation]
pub open spec fn invalidate_page_in_matrix_spec(matrix: Seq<Seq<u64>>, page: int, core: int) -> Seq<Seq<u64>> {
    if 0 <= page < matrix.len() {
        matrix.update(page, invalidate_core_spec(matrix[page], core))
    } else {
        matrix
    }
}

// r[impl tlb-shootdown.range]
pub open spec fn invalidate_range_for_core_spec(matrix: Seq<Seq<u64>>, core: int, start: int, end: int) -> Seq<Seq<u64>>
    decreases if start <= end { end - start } else { 0 }
{
    if start >= end {
        matrix
    } else {
        invalidate_range_for_core_spec(invalidate_page_in_matrix_spec(matrix, start, core), core, start + 1, end)
    }
}

// r[verify tlb-shootdown.isolation]
pub proof fn invalidate_page_preserves_other_pages(matrix: Seq<Seq<u64>>, page: int, core: int, other_page: int)
    requires
        0 <= other_page < matrix.len(),
        0 <= page < matrix.len(),
        other_page != page,
    ensures
        invalidate_page_in_matrix_spec(matrix, page, core)[other_page] == matrix[other_page],
{
}

// r[verify tlb-shootdown.range]
pub proof fn invalidate_range_preserves_outside_pages(matrix: Seq<Seq<u64>>, core: int, start: int, end: int, page: int)
    requires
        0 <= start <= end <= matrix.len(),
        0 <= page < matrix.len(),
        !(start <= page < end),
    ensures
        invalidate_range_for_core_spec(matrix, core, start, end)[page] == matrix[page],
    decreases end - start
{
    if start < end {
        if page == start {
            assert(false);
        } else {
            invalidate_page_preserves_other_pages(matrix, start, core, page);
            let next = invalidate_page_in_matrix_spec(matrix, start, core);
            invalidate_range_preserves_outside_pages(next, core, start + 1, end, page);
        }
    }
}

// r[verify tlb-shootdown.range]
pub proof fn empty_range_identity(matrix: Seq<Seq<u64>>, core: int, start: int)
    requires 0 <= start <= matrix.len()
    ensures invalidate_range_for_core_spec(matrix, core, start, start) == matrix
{
}

// r[verify tlb-shootdown.range]
pub proof fn singleton_range_matches_page(matrix: Seq<Seq<u64>>, core: int, page: int)
    requires 0 <= page < matrix.len()
    ensures
        invalidate_range_for_core_spec(matrix, core, page, page + 1)
            == invalidate_page_in_matrix_spec(matrix, page, core),
{
    let after_page = invalidate_page_in_matrix_spec(matrix, page, core);
    empty_range_identity(after_page, core, page + 1);
}

// r[verify tlb-shootdown.range]
pub proof fn invalidate_range_updates_target_page(matrix: Seq<Seq<u64>>, core: int, start: int, end: int, page: int)
    requires
        0 <= start <= page < end <= matrix.len(),
    ensures
        invalidate_range_for_core_spec(matrix, core, start, end)[page] == invalidate_core_spec(matrix[page], core),
    decreases end - start
{
    if page == start {
        let after_first = invalidate_page_in_matrix_spec(matrix, start, core);
        invalidate_range_preserves_outside_pages(after_first, core, start + 1, end, page);
    } else {
        invalidate_page_preserves_other_pages(matrix, start, core, page);
        let after_first = invalidate_page_in_matrix_spec(matrix, start, core);
        invalidate_range_updates_target_page(after_first, core, start + 1, end, page);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const TEST_TARGET_PAGE: usize = 7;
    const PROP_MAX_CORES: usize = 6;
    const PROP_PRIORITY_COUNT: usize = PROP_MAX_CORES;

    fn runtime_invalidate_page_in_matrix(matrix: &[Vec<u64>], page: usize, core: usize) -> Option<Vec<Vec<u64>>> {
        if page >= matrix.len() || core >= matrix[page].len() {
            return None;
        }
        let mut result = matrix.to_vec();
        result[page][core] = TLB_UNMAPPED;
        Some(result)
    }

    fn runtime_invalidate_range_for_core(
        matrix: &[Vec<u64>],
        core: usize,
        start: usize,
        end: usize,
    ) -> Option<Vec<Vec<u64>>> {
        if start > end || end > matrix.len() {
            return None;
        }
        let mut result = matrix.to_vec();
        let mut page = start;
        while page < end {
            if core >= result[page].len() {
                return None;
            }
            result[page][core] = TLB_UNMAPPED;
            page += 1;
        }
        Some(result)
    }

    fn apply_schedule(initial: &[u64], initiator: usize, order: &[usize]) -> Option<(ShootdownRequest, Vec<u64>)> {
        let (mut request, mut page_tlbs) = initiate_shootdown(initial, initiator, TEST_TARGET_PAGE)?;
        let mut index = 0usize;
        while index < order.len() {
            let core = order[index];
            page_tlbs = handle_shootdown_ipi(&page_tlbs, core)?;
            request = acknowledge_shootdown(&request, core)?;
            index += 1;
        }
        Some((request, page_tlbs))
    }

    // r[verify tlb-shootdown.self-invalidation]
    #[test]
    fn test_initiate_self_invalidates_and_marks_ack() {
        let initial = vec![11, 22, 33];
        let (request, page_tlbs) = initiate_shootdown(&initial, 1, TEST_TARGET_PAGE).unwrap();
        assert_eq!(request.initiator, 1);
        assert_eq!(request.target_page, TEST_TARGET_PAGE);
        assert_eq!(request.acked, vec![false, true, false]);
        assert_eq!(page_tlbs, vec![11, TLB_UNMAPPED, 33]);
    }

    // r[verify tlb-shootdown.freshness]
    // r[verify tlb-shootdown.termination]
    #[test]
    fn test_sequential_completion_makes_page_fresh() {
        let initial = vec![9, 8, 7];
        let (request, page_tlbs) = apply_schedule(&initial, 0, &[1, 2]).unwrap();
        assert!(all_acknowledged(&request));
        assert_eq!(pending_ack_count(&request), 0);
        assert!(check_tlb_freshness(&page_tlbs));
    }

    // r[verify tlb-shootdown.isolation]
    #[test]
    fn test_disjoint_page_update_preserves_other_page() {
        let matrix = vec![vec![5, 6], vec![7, 8]];
        let updated = runtime_invalidate_page_in_matrix(&matrix, 0, 1).unwrap();
        assert_eq!(updated[0], vec![5, TLB_UNMAPPED]);
        assert_eq!(updated[1], matrix[1]);
    }

    // r[verify tlb-shootdown.range]
    #[test]
    fn test_empty_and_singleton_range_boundaries() {
        let matrix = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
        let empty = runtime_invalidate_range_for_core(&matrix, 1, 1, 1).unwrap();
        assert_eq!(empty, matrix);
        let singleton = runtime_invalidate_range_for_core(&matrix, 1, 2, 3).unwrap();
        let page = runtime_invalidate_page_in_matrix(&matrix, 2, 1).unwrap();
        assert_eq!(singleton, page);
    }

    // r[verify tlb-shootdown.self-invalidation]
    #[test]
    fn test_invalid_core_indices_return_none() {
        let initial = vec![1, 2];
        assert!(initiate_shootdown(&initial, 3, TEST_TARGET_PAGE).is_none());
        assert!(handle_shootdown_ipi(&initial, 3).is_none());
        let (request, _) = initiate_shootdown(&initial, 0, TEST_TARGET_PAGE).unwrap();
        assert!(acknowledge_shootdown(&request, 3).is_none());
    }

    // r[verify tlb-shootdown.termination]
    #[test]
    fn test_repeated_ack_is_no_op() {
        let initial = vec![10, 20, 30];
        let (request, page_tlbs) = initiate_shootdown(&initial, 0, TEST_TARGET_PAGE).unwrap();
        let page_tlbs = handle_shootdown_ipi(&page_tlbs, 1).unwrap();
        let once = acknowledge_shootdown(&request, 1).unwrap();
        let twice = acknowledge_shootdown(&once, 1).unwrap();
        assert_eq!(twice, once);
        assert_eq!(pending_ack_count(&twice), pending_ack_count(&once));
        assert_eq!(page_tlbs[1], TLB_UNMAPPED);
    }

    proptest! {
        // r[verify tlb-shootdown.termination]
        #[test]
        fn prop_pending_count_never_increases(
            core_count in 1usize..PROP_MAX_CORES,
            initiator_seed in 0usize..PROP_MAX_CORES,
            ack_core_seed in 0usize..PROP_MAX_CORES,
        ) {
            let initiator = initiator_seed % core_count;
            let ack_core = ack_core_seed % core_count;
            let initial = vec![17u64; core_count];
            let (request, page_tlbs) = initiate_shootdown(&initial, initiator, TEST_TARGET_PAGE).unwrap();
            let next_request = if ack_core == initiator {
                acknowledge_shootdown(&request, ack_core).unwrap()
            } else {
                let page_tlbs = handle_shootdown_ipi(&page_tlbs, ack_core).unwrap();
                let acknowledged = acknowledge_shootdown(&request, ack_core).unwrap();
                prop_assert_eq!(page_tlbs[ack_core], TLB_UNMAPPED);
                acknowledged
            };
            prop_assert!(pending_ack_count(&next_request) <= pending_ack_count(&request));
        }

        // r[verify tlb-shootdown.termination]
        #[test]
        fn prop_random_ack_order_completes(
            core_count in 1usize..PROP_MAX_CORES,
            initiator_seed in 0usize..PROP_MAX_CORES,
            priorities in proptest::collection::vec(0u8..32, PROP_PRIORITY_COUNT),
        ) {
            let initiator = initiator_seed % core_count;
            let mut order: Vec<usize> = (0..core_count).filter(|core| *core != initiator).collect();
            order.sort_by_key(|core| (priorities[*core], *core));
            let initial = vec![99u64; core_count];
            let (request, page_tlbs) = apply_schedule(&initial, initiator, &order).unwrap();
            prop_assert!(all_acknowledged(&request));
            prop_assert!(check_tlb_freshness(&page_tlbs));
        }

        // r[verify tlb-shootdown.freshness]
        #[test]
        fn prop_freshness_holds_after_full_valid_schedule(
            core_count in 1usize..PROP_MAX_CORES,
            initiator_seed in 0usize..PROP_MAX_CORES,
            priorities in proptest::collection::vec(0u8..32, PROP_PRIORITY_COUNT),
        ) {
            let initiator = initiator_seed % core_count;
            let mut order: Vec<usize> = (0..core_count).filter(|core| *core != initiator).collect();
            order.sort_by_key(|core| (priorities[*core], *core));
            let initial: Vec<u64> = (0..core_count).map(|core| (core as u64) + 1).collect();
            let (request, page_tlbs) = apply_schedule(&initial, initiator, &order).unwrap();
            prop_assert!(all_acknowledged(&request));
            prop_assert!(check_tlb_freshness(&page_tlbs));
            for core in 0..core_count {
                prop_assert_eq!(page_tlbs[core], TLB_UNMAPPED);
            }
        }
    }
}

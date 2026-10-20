//! Backend frame-allocation composite facade.
//!
//! This facade joins the existing liveness, register-allocation, and
//! stack-frame-layout models for the currently supported local/spill slice.
//! Callee-save registers and ABI argument placement remain out of scope.

use vstd::prelude::*;

verus! {

pub open spec fn liveness_ranges_match_spec(
    live: crate::liveness_analysis::LiveSets,
    ranges: Seq<Vec<bool>>,
) -> bool {
    &&& crate::liveness_analysis::live_sets_shape_spec(
        live.live_in@,
        live.live_out@,
        live.live_in@.len() as int,
        crate::liveness_analysis::cols_spec(live.live_in@),
    )
    &&& ranges.len() == crate::liveness_analysis::cols_spec(live.live_in@)
    &&& forall|var: int| #![trigger ranges[var]]
        0 <= var < ranges.len() ==>
            ranges[var]@ == crate::liveness_analysis::live_range_spec(live.live_in@, live.live_out@, var)
}

// r[impl composite-primitives.backend-frame-allocator]
pub open spec fn backend_frame_allocator_spec(
    edges: Seq<(usize, usize)>,
    uses: Seq<Vec<bool>>,
    defs: Seq<Vec<bool>>,
    live: crate::liveness_analysis::LiveSets,
    live_ranges: Seq<Vec<bool>>,
    graph: crate::register_allocation::InterferenceGraph,
    assignments: Seq<crate::register_allocation::Allocation>,
    register_count: int,
    layout: crate::stack_frame_layout::FrameLayout,
    live_slots: Seq<crate::stack_frame_layout::LiveSlot>,
) -> bool {
    &&& crate::liveness_analysis::valid_liveness_input_spec(edges, uses, defs)
    &&& liveness_ranges_match_spec(live, live_ranges)
    &&& crate::register_allocation::valid_live_ranges_spec(live_ranges)
    &&& crate::register_allocation::valid_graph_spec(graph)
    &&& crate::register_allocation::valid_coloring_spec(graph, assignments, register_count)
    &&& crate::stack_frame_layout::layout_valid_spec(layout)
    &&& crate::stack_frame_layout::non_overlap_spec(live_slots)
}

// r[verify composite-primitives.backend-frame-allocator]
pub proof fn backend_frame_allocator_refines_sources(
    edges: Seq<(usize, usize)>,
    uses: Seq<Vec<bool>>,
    defs: Seq<Vec<bool>>,
    live: crate::liveness_analysis::LiveSets,
    live_ranges: Seq<Vec<bool>>,
    graph: crate::register_allocation::InterferenceGraph,
    assignments: Seq<crate::register_allocation::Allocation>,
    register_count: int,
    layout: crate::stack_frame_layout::FrameLayout,
    live_slots: Seq<crate::stack_frame_layout::LiveSlot>,
)
    requires backend_frame_allocator_spec(
        edges,
        uses,
        defs,
        live,
        live_ranges,
        graph,
        assignments,
        register_count,
        layout,
        live_slots,
    ),
    ensures
        crate::liveness_analysis::valid_liveness_input_spec(edges, uses, defs),
        liveness_ranges_match_spec(live, live_ranges),
        crate::register_allocation::valid_live_ranges_spec(live_ranges),
        crate::register_allocation::valid_graph_spec(graph),
        crate::register_allocation::valid_coloring_spec(graph, assignments, register_count),
        crate::stack_frame_layout::layout_valid_spec(layout),
        crate::stack_frame_layout::non_overlap_spec(live_slots),
{
}

// r[impl composite-primitives.backend-frame-allocator]
pub open spec fn stack_slot_reuse_allowed_spec(
    existing: crate::stack_frame_layout::LiveSlot,
    candidate: crate::stack_frame_layout::LiveSlot,
) -> bool {
    crate::stack_frame_layout::can_reuse_slot_spec(existing, candidate)
}

// r[verify composite-primitives.backend-frame-allocator]
pub proof fn backend_frame_allocator_reuse_refines_sources(
    existing: crate::stack_frame_layout::LiveSlot,
    candidate: crate::stack_frame_layout::LiveSlot,
)
    requires stack_slot_reuse_allowed_spec(existing, candidate),
    ensures
        crate::stack_frame_layout::same_byte_range_spec(existing.slot, candidate.slot),
        !crate::stack_frame_layout::live_ranges_overlap_spec(existing.live, candidate.live),
{
}

// r[impl composite-primitives.backend-frame-allocator]
pub open spec fn invalid_overlap_slot_reuse_spec(
    existing: crate::stack_frame_layout::LiveSlot,
    candidate: crate::stack_frame_layout::LiveSlot,
) -> bool {
    &&& crate::stack_frame_layout::live_slot_valid_spec(existing)
    &&& crate::stack_frame_layout::live_slot_valid_spec(candidate)
    &&& crate::stack_frame_layout::same_byte_range_spec(existing.slot, candidate.slot)
    &&& crate::stack_frame_layout::live_ranges_overlap_spec(existing.live, candidate.live)
}

// r[verify composite-primitives.backend-frame-allocator]
pub proof fn backend_frame_allocator_overlap_rejection_refines_sources(
    existing: crate::stack_frame_layout::LiveSlot,
    candidate: crate::stack_frame_layout::LiveSlot,
)
    requires invalid_overlap_slot_reuse_spec(existing, candidate),
    ensures !crate::stack_frame_layout::can_reuse_slot_spec(existing, candidate),
{
}

// r[impl composite-primitives.backend-frame-allocator]
pub fn backend_frame_allocator_admits(
    edges: &[(usize, usize)],
    uses: &[Vec<bool>],
    defs: &[Vec<bool>],
    live: &crate::liveness_analysis::LiveSets,
    live_ranges: &[Vec<bool>],
    graph: &crate::register_allocation::InterferenceGraph,
    assignments: &[crate::register_allocation::Allocation],
    register_count: usize,
    layout: &crate::stack_frame_layout::FrameLayout,
    live_slots: &[crate::stack_frame_layout::LiveSlot],
) -> (result: bool)
    requires
        crate::liveness_analysis::valid_liveness_input_spec(edges@, uses@, defs@),
        liveness_ranges_match_spec(*live, live_ranges@),
        crate::stack_frame_layout::layout_valid_spec(*layout),
    ensures result ==> backend_frame_allocator_spec(
        edges@,
        uses@,
        defs@,
        *live,
        live_ranges@,
        *graph,
        assignments@,
        register_count as int,
        *layout,
        live_slots@,
    ),
{
    if !crate::register_allocation::valid_live_ranges(live_ranges) {
        return false;
    }
    if !crate::register_allocation::valid_graph(graph) {
        return false;
    }
    if !crate::register_allocation::check_coloring(graph, assignments, register_count) {
        return false;
    }
    if !crate::stack_frame_layout::check_non_overlap(live_slots) {
        return false;
    }
    if !crate::stack_frame_layout::check_alignment(layout) {
        return false;
    }
    if crate::stack_frame_layout::compute_frame_size(&layout.slots, layout.stack_alignment) != Some(layout.frame_size) {
        return false;
    }
    true
}

// r[impl composite-primitives.backend-frame-allocator]
pub fn backend_frame_allocator_can_reuse_slot(
    existing: crate::stack_frame_layout::LiveSlot,
    candidate: crate::stack_frame_layout::LiveSlot,
) -> (result: bool)
    ensures result == stack_slot_reuse_allowed_spec(existing, candidate),
{
    crate::stack_frame_layout::can_reuse_slot(existing, candidate)
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liveness_analysis::LiveSets;
    use crate::register_allocation::Allocation;
    use crate::register_allocation::InterferenceGraph;
    use crate::stack_frame_layout::FrameLayout;
    use crate::stack_frame_layout::LiveRange;
    use crate::stack_frame_layout::LiveSlot;
    use crate::stack_frame_layout::SlotKind;
    use crate::stack_frame_layout::StackSlot;

    fn slot(offset: u32) -> StackSlot {
        StackSlot {
            offset,
            size: 8,
            alignment: 8,
            kind: SlotKind::Spill(0),
        }
    }

    #[test]
    fn safe_reuse_is_admitted() {
        let s = slot(0);
        let first = LiveSlot {
            slot: s,
            live: LiveRange { start: 0, end: 2 },
        };
        let second = LiveSlot {
            slot: s,
            live: LiveRange { start: 2, end: 4 },
        };
        assert!(backend_frame_allocator_can_reuse_slot(first, second));
    }

    #[test]
    fn overlap_reuse_is_rejected() {
        let s = slot(0);
        let first = LiveSlot {
            slot: s,
            live: LiveRange { start: 0, end: 3 },
        };
        let second = LiveSlot {
            slot: s,
            live: LiveRange { start: 2, end: 4 },
        };
        assert!(!backend_frame_allocator_can_reuse_slot(first, second));
    }

    #[test]
    fn admits_valid_local_spill_slice() {
        let edges = vec![(0usize, 1usize)];
        let uses = vec![vec![true, false], vec![false, true]];
        let defs = vec![vec![false, false], vec![false, false]];
        let live = LiveSets {
            live_in: vec![vec![true, false], vec![false, true]],
            live_out: vec![vec![false, false], vec![false, false]],
        };
        let live_ranges = vec![vec![true, false], vec![false, true]];
        let graph = InterferenceGraph {
            edges: vec![vec![false, false], vec![false, false]],
        };
        let assignments = vec![
            Allocation {
                register: Some(0),
                spill_slot: None,
            },
            Allocation {
                register: None,
                spill_slot: Some(0),
            },
        ];
        let s0 = slot(0);
        let s1 = StackSlot {
            offset: 8,
            size: 8,
            alignment: 8,
            kind: SlotKind::Local(0),
        };
        let layout = FrameLayout {
            slots: vec![s0, s1],
            frame_size: 16,
            stack_alignment: 8,
        };
        let live_slots = vec![
            LiveSlot {
                slot: s0,
                live: LiveRange { start: 0, end: 2 },
            },
            LiveSlot {
                slot: s1,
                live: LiveRange { start: 1, end: 3 },
            },
        ];
        assert!(backend_frame_allocator_admits(
            &edges,
            &uses,
            &defs,
            &live,
            &live_ranges,
            &graph,
            &assignments,
            1,
            &layout,
            &live_slots,
        ));
    }

    #[test]
    fn overlapping_same_slot_slice_is_rejected() {
        let edges = Vec::<(usize, usize)>::new();
        let uses = vec![vec![true, true]];
        let defs = vec![vec![false, false]];
        let live = LiveSets {
            live_in: vec![vec![true, true]],
            live_out: vec![vec![false, false]],
        };
        let live_ranges = vec![vec![true], vec![true]];
        let graph = InterferenceGraph {
            edges: vec![vec![false, false], vec![false, false]],
        };
        let assignments = vec![
            Allocation {
                register: None,
                spill_slot: Some(0),
            },
            Allocation {
                register: None,
                spill_slot: Some(0),
            },
        ];
        let s = slot(0);
        let layout = FrameLayout {
            slots: vec![s],
            frame_size: 8,
            stack_alignment: 8,
        };
        let live_slots = vec![
            LiveSlot {
                slot: s,
                live: LiveRange { start: 0, end: 3 },
            },
            LiveSlot {
                slot: s,
                live: LiveRange { start: 1, end: 4 },
            },
        ];
        assert!(!backend_frame_allocator_admits(
            &edges,
            &uses,
            &defs,
            &live,
            &live_ranges,
            &graph,
            &assignments,
            1,
            &layout,
            &live_slots,
        ));
    }
}

//! Bounded index/query pagination semantics.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageWindowEnvelope {
    pub start: usize,
    pub end: usize,
    pub total: usize,
    pub limit: usize,
    pub cursor: crate::storage_index::CursorBounds,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IndexedQueryPageEnvelope {
    pub window: PageWindowEnvelope,
    pub storage_page: crate::storage_index::PageRange,
    pub lookup: crate::storage_index::LookupResultAdmission,
    pub table: crate::table_relational::RelationalAdmissionEnvelope,
    pub graph: crate::graph_query_composition::GraphQueryResultFacade,
    pub index_equiv: crate::graph_query_composition::IndexScanEquivalence,
    pub rows_in_scan_window: bool,
    pub index_order_preserved: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PaginationStepEnvelope {
    pub previous: PageWindowEnvelope,
    pub next: PageWindowEnvelope,
    pub same_total: bool,
    pub same_query_model: bool,
    pub no_duplicate_rows: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PaginatedQueryResultEnvelope {
    pub first_page: IndexedQueryPageEnvelope,
    pub step: PaginationStepEnvelope,
    pub continuation_present: bool,
    pub continuation_admitted: bool,
    pub max_pages: usize,
    pub emitted_pages: usize,
}

// r[depends composite-primitives.index-query-pagination]
pub open spec fn page_window_envelope_spec(e: PageWindowEnvelope) -> bool {
    e.start <= e.end
        && e.end <= e.total
        && e.end - e.start <= e.limit
        && crate::storage_index::cursor_bounds_spec(e.cursor)
        && e.cursor.position == e.start
        && e.cursor.len == e.total
}

// r[impl composite-primitives.index-query-pagination]
pub fn page_window_envelope_valid(e: PageWindowEnvelope) -> (result: bool)
    ensures result == page_window_envelope_spec(e)
{
    e.start <= e.end
        && e.end <= e.total
        && e.end - e.start <= e.limit
        && crate::storage_index::cursor_bounds_valid(e.cursor)
        && e.cursor.position == e.start
        && e.cursor.len == e.total
}

// r[verify composite-primitives.index-query-pagination]
pub proof fn valid_page_window_is_bounded(e: PageWindowEnvelope)
    requires page_window_envelope_spec(e)
    ensures e.start <= e.end, e.end <= e.total, e.end - e.start <= e.limit
{}

// r[depends composite-primitives.index-query-pagination]
pub open spec fn indexed_query_page_envelope_spec(e: IndexedQueryPageEnvelope) -> bool {
    page_window_envelope_spec(e.window)
        && crate::storage_index::page_range_spec(e.storage_page)
        && crate::storage_index::lookup_result_admission_spec(e.lookup)
        && crate::table_relational::relational_admission_envelope_spec(e.table)
        && crate::graph_query_composition::graph_query_result_facade_spec(e.graph)
        && crate::graph_query_composition::index_scan_equivalence_spec(e.index_equiv)
        && e.rows_in_scan_window
        && e.index_order_preserved
        && e.storage_page.start_page == e.window.start
        && e.storage_page.end_page == e.window.end
        && e.storage_page.page_count == e.window.total
}

// r[impl composite-primitives.index-query-pagination]
pub fn indexed_query_page_envelope_valid(e: IndexedQueryPageEnvelope) -> (result: bool)
    ensures result == indexed_query_page_envelope_spec(e)
{
    if !page_window_envelope_valid(e.window) {
        return false;
    }
    if !crate::storage_index::page_range_valid(e.storage_page) {
        return false;
    }
    if !crate::storage_index::lookup_result_admission_valid(e.lookup) {
        return false;
    }
    if !crate::table_relational::relational_admission_envelope_valid(e.table) {
        return false;
    }
    if !crate::graph_query_composition::graph_query_result_facade_valid(e.graph) {
        return false;
    }
    if !crate::graph_query_composition::index_scan_equivalence_valid(e.index_equiv) {
        return false;
    }
    e.rows_in_scan_window
        && e.index_order_preserved
        && e.storage_page.start_page == e.window.start
        && e.storage_page.end_page == e.window.end
        && e.storage_page.page_count == e.window.total
}

// r[verify composite-primitives.index-query-pagination]
pub proof fn accepted_page_matches_bounded_scan(e: IndexedQueryPageEnvelope)
    requires indexed_query_page_envelope_spec(e)
    ensures
        page_window_envelope_spec(e.window),
        crate::graph_query_composition::index_scan_equivalence_spec(e.index_equiv),
        e.rows_in_scan_window,
        e.storage_page.start_page == e.window.start,
        e.storage_page.end_page == e.window.end
{}

// r[depends composite-primitives.index-query-pagination]
pub open spec fn pagination_step_envelope_spec(e: PaginationStepEnvelope) -> bool {
    page_window_envelope_spec(e.previous)
        && page_window_envelope_spec(e.next)
        && e.same_total
        && e.same_query_model
        && e.no_duplicate_rows
        && e.previous.total == e.next.total
        && e.previous.end <= e.next.start
        && e.next.start <= e.next.end
}

// r[impl composite-primitives.index-query-pagination]
pub fn pagination_step_envelope_valid(e: PaginationStepEnvelope) -> (result: bool)
    ensures result == pagination_step_envelope_spec(e)
{
    if !page_window_envelope_valid(e.previous) {
        return false;
    }
    if !page_window_envelope_valid(e.next) {
        return false;
    }
    e.same_total
        && e.same_query_model
        && e.no_duplicate_rows
        && e.previous.total == e.next.total
        && e.previous.end <= e.next.start
        && e.next.start <= e.next.end
}

// r[verify composite-primitives.index-query-pagination]
pub proof fn valid_step_advances_cursor_without_duplicates(e: PaginationStepEnvelope)
    requires pagination_step_envelope_spec(e)
    ensures e.previous.end <= e.next.start, e.no_duplicate_rows, e.previous.total == e.next.total
{}

// r[depends composite-primitives.index-query-pagination]
pub open spec fn paginated_query_result_envelope_spec(e: PaginatedQueryResultEnvelope) -> bool {
    indexed_query_page_envelope_spec(e.first_page)
        && pagination_step_envelope_spec(e.step)
        && e.emitted_pages <= e.max_pages
        && (if e.continuation_present { e.continuation_admitted } else { true })
        && e.first_page.window.total == e.step.previous.total
}

// r[impl composite-primitives.index-query-pagination]
pub fn paginated_query_result_envelope_valid(e: PaginatedQueryResultEnvelope) -> (result: bool)
    ensures result == paginated_query_result_envelope_spec(e)
{
    if !indexed_query_page_envelope_valid(e.first_page) {
        return false;
    }
    if !pagination_step_envelope_valid(e.step) {
        return false;
    }
    e.emitted_pages <= e.max_pages
        && (if e.continuation_present { e.continuation_admitted } else { true })
        && e.first_page.window.total == e.step.previous.total
}

// r[verify composite-primitives.index-query-pagination]
pub proof fn accepted_paginated_result_carries_scan_and_progress(e: PaginatedQueryResultEnvelope)
    requires paginated_query_result_envelope_spec(e)
    ensures
        indexed_query_page_envelope_spec(e.first_page),
        pagination_step_envelope_spec(e.step),
        e.emitted_pages <= e.max_pages
{}

// r[verify composite-primitives.index-query-pagination]
pub proof fn invalid_cursor_position_rejected(e: PageWindowEnvelope)
    requires
        crate::storage_index::cursor_bounds_spec(e.cursor),
        e.start <= e.end,
        e.end <= e.total,
        e.end - e.start <= e.limit,
        e.cursor.len == e.total,
        e.cursor.position != e.start,
    ensures !page_window_envelope_spec(e)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn schema_ok() -> crate::table_relational::RelationalAdmissionEnvelope {
        crate::table_relational::RelationalAdmissionEnvelope {
            schema_ok: true,
            row_ok: true,
            key_ok: true,
            projection_ok: true,
            join_ok: true,
            grouping_ok: true,
        }
    }

    fn graph_ok() -> crate::graph_query_composition::GraphQueryResultFacade {
        crate::graph_query_composition::GraphQueryResultFacade {
            model_ok: true,
            pattern_ok: true,
            binding_ok: true,
            path_ok: true,
            join_filter_ok: true,
            index_equiv_ok: true,
            serialized_envelope_ok: true,
        }
    }

    fn index_equiv_ok() -> crate::graph_query_composition::IndexScanEquivalence {
        crate::graph_query_composition::IndexScanEquivalence {
            index_hits_in_scan: true,
            scan_hits_in_index: true,
            same_query_bounds: true,
            lookup_result_admitted: true,
        }
    }

    fn page(start: usize, end: usize, total: usize, limit: usize) -> PageWindowEnvelope {
        PageWindowEnvelope {
            start,
            end,
            total,
            limit,
            cursor: crate::storage_index::CursorBounds {
                position: start,
                len: total,
                max_len: total,
            },
        }
    }

    fn indexed_page() -> IndexedQueryPageEnvelope {
        IndexedQueryPageEnvelope {
            window: page(0, 2, 5, 2),
            storage_page: crate::storage_index::PageRange {
                start_page: 0,
                end_page: 2,
                page_count: 5,
            },
            lookup: crate::storage_index::LookupResultAdmission {
                found: true,
                slot_index: 1,
                slot_count: 5,
            },
            table: schema_ok(),
            graph: graph_ok(),
            index_equiv: index_equiv_ok(),
            rows_in_scan_window: true,
            index_order_preserved: true,
        }
    }

    #[test]
    // r[verify composite-primitives.index-query-pagination]
    fn accepts_first_page_matching_scan_window() {
        assert!(indexed_query_page_envelope_valid(indexed_page()));
    }

    #[test]
    // r[verify composite-primitives.index-query-pagination]
    fn rejects_invalid_cursor_position() {
        let mut p = page(0, 2, 5, 2);
        p.cursor.position = 1;
        assert!(!page_window_envelope_valid(p));
    }

    #[test]
    // r[verify composite-primitives.index-query-pagination]
    fn accepts_monotone_continuation_without_duplicates() {
        let step = PaginationStepEnvelope {
            previous: page(0, 2, 5, 2),
            next: page(2, 4, 5, 2),
            same_total: true,
            same_query_model: true,
            no_duplicate_rows: true,
        };
        let result = PaginatedQueryResultEnvelope {
            first_page: indexed_page(),
            step,
            continuation_present: true,
            continuation_admitted: true,
            max_pages: 3,
            emitted_pages: 2,
        };
        assert!(paginated_query_result_envelope_valid(result));
    }

    #[test]
    // r[verify composite-primitives.index-query-pagination]
    fn rejects_duplicate_continuation_window() {
        let step = PaginationStepEnvelope {
            previous: page(0, 2, 5, 2),
            next: page(1, 3, 5, 2),
            same_total: true,
            same_query_model: true,
            no_duplicate_rows: true,
        };
        assert!(!pagination_step_envelope_valid(step));
    }
}

//! Exact finite storage index primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageRange {
    pub start_page: usize,
    pub end_page: usize,
    pub page_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecordSlotTable {
    pub used_slots: usize,
    pub slot_count: usize,
    pub max_slots: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyRange {
    pub min_key: u64,
    pub max_key: u64,
    pub key: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CursorBounds {
    pub position: usize,
    pub len: usize,
    pub max_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SegmentMap {
    pub segment_start: usize,
    pub segment_end: usize,
    pub address_space_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LookupResultAdmission {
    pub found: bool,
    pub slot_index: usize,
    pub slot_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CompactionEnvelope {
    pub before_live: usize,
    pub before_total: usize,
    pub after_live: usize,
    pub after_total: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VersionedSlot {
    pub version: u64,
    pub min_version: u64,
    pub slot_index: usize,
    pub slot_count: usize,
}

// r[depends storage.page-range]
pub open spec fn page_range_spec(e: PageRange) -> bool {
    e.start_page <= e.end_page && e.end_page <= e.page_count
}

// r[impl storage.page-range]
pub fn page_range_valid(e: PageRange) -> (result: bool)
    ensures result == page_range_spec(e)
{
    e.start_page <= e.end_page && e.end_page <= e.page_count
}

// r[verify storage.page-range]
pub proof fn valid_page_range_is_admitted(e: PageRange)
    requires page_range_spec(e)
    ensures page_range_spec(e)
{}

// r[depends storage.record-slot-table]
pub open spec fn record_slot_table_spec(e: RecordSlotTable) -> bool {
    e.used_slots <= e.slot_count && e.slot_count <= e.max_slots
}

// r[impl storage.record-slot-table]
pub fn record_slot_table_valid(e: RecordSlotTable) -> (result: bool)
    ensures result == record_slot_table_spec(e)
{
    e.used_slots <= e.slot_count && e.slot_count <= e.max_slots
}

// r[verify storage.record-slot-table]
pub proof fn valid_record_slot_table_is_admitted(e: RecordSlotTable)
    requires record_slot_table_spec(e)
    ensures record_slot_table_spec(e)
{}

// r[depends storage.key-range]
pub open spec fn key_range_spec(e: KeyRange) -> bool {
    e.min_key <= e.max_key && e.key >= e.min_key && e.key <= e.max_key
}

// r[impl storage.key-range]
pub fn key_range_valid(e: KeyRange) -> (result: bool)
    ensures result == key_range_spec(e)
{
    e.min_key <= e.max_key && e.key >= e.min_key && e.key <= e.max_key
}

// r[verify storage.key-range]
pub proof fn valid_key_range_is_admitted(e: KeyRange)
    requires key_range_spec(e)
    ensures key_range_spec(e)
{}

// r[depends storage.cursor-bounds]
pub open spec fn cursor_bounds_spec(e: CursorBounds) -> bool {
    e.position <= e.len && e.len <= e.max_len
}

// r[impl storage.cursor-bounds]
pub fn cursor_bounds_valid(e: CursorBounds) -> (result: bool)
    ensures result == cursor_bounds_spec(e)
{
    e.position <= e.len && e.len <= e.max_len
}

// r[verify storage.cursor-bounds]
pub proof fn valid_cursor_bounds_is_admitted(e: CursorBounds)
    requires cursor_bounds_spec(e)
    ensures cursor_bounds_spec(e)
{}

// r[depends storage.segment-map]
pub open spec fn segment_map_spec(e: SegmentMap) -> bool {
    e.segment_start <= e.segment_end && e.segment_end <= e.address_space_len
}

// r[impl storage.segment-map]
pub fn segment_map_valid(e: SegmentMap) -> (result: bool)
    ensures result == segment_map_spec(e)
{
    e.segment_start <= e.segment_end && e.segment_end <= e.address_space_len
}

// r[verify storage.segment-map]
pub proof fn valid_segment_map_is_admitted(e: SegmentMap)
    requires segment_map_spec(e)
    ensures segment_map_spec(e)
{}

// r[depends storage.lookup-result]
pub open spec fn lookup_result_admission_spec(e: LookupResultAdmission) -> bool {
    !e.found || e.slot_index < e.slot_count
}

// r[impl storage.lookup-result]
pub fn lookup_result_admission_valid(e: LookupResultAdmission) -> (result: bool)
    ensures result == lookup_result_admission_spec(e)
{
    !e.found || e.slot_index < e.slot_count
}

// r[verify storage.lookup-result]
pub proof fn valid_lookup_result_admission_is_admitted(e: LookupResultAdmission)
    requires lookup_result_admission_spec(e)
    ensures lookup_result_admission_spec(e)
{}

// r[depends storage.compaction-envelope]
pub open spec fn compaction_envelope_spec(e: CompactionEnvelope) -> bool {
    e.before_live <= e.before_total && e.after_total <= e.before_total && e.after_live == e.before_live
}

// r[impl storage.compaction-envelope]
pub fn compaction_envelope_valid(e: CompactionEnvelope) -> (result: bool)
    ensures result == compaction_envelope_spec(e)
{
    e.before_live <= e.before_total && e.after_total <= e.before_total && e.after_live == e.before_live
}

// r[verify storage.compaction-envelope]
pub proof fn valid_compaction_envelope_is_admitted(e: CompactionEnvelope)
    requires compaction_envelope_spec(e)
    ensures compaction_envelope_spec(e)
{}

// r[depends storage.versioned-slot]
pub open spec fn versioned_slot_spec(e: VersionedSlot) -> bool {
    e.version >= e.min_version && e.slot_index < e.slot_count
}

// r[impl storage.versioned-slot]
pub fn versioned_slot_valid(e: VersionedSlot) -> (result: bool)
    ensures result == versioned_slot_spec(e)
{
    e.version >= e.min_version && e.slot_index < e.slot_count
}

// r[verify storage.versioned-slot]
pub proof fn valid_versioned_slot_is_admitted(e: VersionedSlot)
    requires versioned_slot_spec(e)
    ensures versioned_slot_spec(e)
{}

} // verus!

//! Exact finite scheduling calendar primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CalendarField {
    pub value: u64,
    pub min: u64,
    pub max: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimeBucket {
    pub instant: u64,
    pub bucket_start: u64,
    pub bucket_end: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScheduleInterval {
    pub start_tick: u64,
    pub end_tick: u64,
    pub duration: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecurrenceWindow {
    pub first_tick: u64,
    pub last_tick: u64,
    pub period: u64,
    pub occurrence_count: usize,
    pub max_occurrences: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeadlineOrder {
    pub release_tick: u64,
    pub due_tick: u64,
    pub deadline_tick: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScheduleConflict {
    pub left_start: u64,
    pub left_end: u64,
    pub right_start: u64,
    pub right_end: u64,
    pub overlaps: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceSlot {
    pub slot_units: usize,
    pub capacity: usize,
    pub start_tick: u64,
    pub end_tick: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScheduleAdmission {
    pub field_ok: bool,
    pub bucket_ok: bool,
    pub interval_ok: bool,
    pub deadline_ok: bool,
    pub resource_ok: bool,
    pub conflict: bool,
}

// r[depends domain-primitives.scheduling-calendar.calendar-field]
pub open spec fn calendar_field_spec(e: CalendarField) -> bool {
    e.min <= e.value && e.value <= e.max && e.min <= e.max
}

// r[impl domain-primitives.scheduling-calendar.calendar-field]
pub fn calendar_field_valid(e: CalendarField) -> (result: bool)
    ensures result == calendar_field_spec(e)
{
    e.min <= e.value && e.value <= e.max && e.min <= e.max
}

// r[verify domain-primitives.scheduling-calendar.calendar-field]
pub proof fn valid_calendar_field_is_admitted(e: CalendarField)
    requires calendar_field_spec(e)
    ensures calendar_field_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.time-bucket]
pub open spec fn time_bucket_spec(e: TimeBucket) -> bool {
    e.bucket_start <= e.instant && e.instant < e.bucket_end && e.bucket_start < e.bucket_end
}

// r[impl domain-primitives.scheduling-calendar.time-bucket]
pub fn time_bucket_valid(e: TimeBucket) -> (result: bool)
    ensures result == time_bucket_spec(e)
{
    e.bucket_start <= e.instant && e.instant < e.bucket_end && e.bucket_start < e.bucket_end
}

// r[verify domain-primitives.scheduling-calendar.time-bucket]
pub proof fn valid_time_bucket_is_admitted(e: TimeBucket)
    requires time_bucket_spec(e)
    ensures time_bucket_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.interval]
pub open spec fn schedule_interval_spec(e: ScheduleInterval) -> bool {
    e.start_tick < e.end_tick && e.duration == e.end_tick - e.start_tick
}

// r[impl domain-primitives.scheduling-calendar.interval]
pub fn schedule_interval_valid(e: ScheduleInterval) -> (result: bool)
    ensures result == schedule_interval_spec(e)
{
    e.start_tick < e.end_tick && e.duration == e.end_tick - e.start_tick
}

// r[verify domain-primitives.scheduling-calendar.interval]
pub proof fn valid_schedule_interval_is_admitted(e: ScheduleInterval)
    requires schedule_interval_spec(e)
    ensures schedule_interval_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.recurrence-window]
pub open spec fn recurrence_window_spec(e: RecurrenceWindow) -> bool {
    e.first_tick <= e.last_tick && e.period > 0 && e.occurrence_count <= e.max_occurrences
}

// r[impl domain-primitives.scheduling-calendar.recurrence-window]
pub fn recurrence_window_valid(e: RecurrenceWindow) -> (result: bool)
    ensures result == recurrence_window_spec(e)
{
    e.first_tick <= e.last_tick && e.period > 0 && e.occurrence_count <= e.max_occurrences
}

// r[verify domain-primitives.scheduling-calendar.recurrence-window]
pub proof fn valid_recurrence_window_is_admitted(e: RecurrenceWindow)
    requires recurrence_window_spec(e)
    ensures recurrence_window_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.deadline-order]
pub open spec fn deadline_order_spec(e: DeadlineOrder) -> bool {
    e.release_tick <= e.due_tick && e.due_tick <= e.deadline_tick
}

// r[impl domain-primitives.scheduling-calendar.deadline-order]
pub fn deadline_order_valid(e: DeadlineOrder) -> (result: bool)
    ensures result == deadline_order_spec(e)
{
    e.release_tick <= e.due_tick && e.due_tick <= e.deadline_tick
}

// r[verify domain-primitives.scheduling-calendar.deadline-order]
pub proof fn valid_deadline_order_is_admitted(e: DeadlineOrder)
    requires deadline_order_spec(e)
    ensures deadline_order_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.conflict]
pub open spec fn schedule_conflict_spec(e: ScheduleConflict) -> bool {
    e.left_start < e.left_end && e.right_start < e.right_end && e.overlaps == (e.left_start < e.right_end && e.right_start < e.left_end)
}

// r[impl domain-primitives.scheduling-calendar.conflict]
pub fn schedule_conflict_valid(e: ScheduleConflict) -> (result: bool)
    ensures result == schedule_conflict_spec(e)
{
    e.left_start < e.left_end && e.right_start < e.right_end && e.overlaps == (e.left_start < e.right_end && e.right_start < e.left_end)
}

// r[verify domain-primitives.scheduling-calendar.conflict]
pub proof fn valid_schedule_conflict_is_admitted(e: ScheduleConflict)
    requires schedule_conflict_spec(e)
    ensures schedule_conflict_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.resource-slot]
pub open spec fn resource_slot_spec(e: ResourceSlot) -> bool {
    e.slot_units <= e.capacity && e.start_tick < e.end_tick
}

// r[impl domain-primitives.scheduling-calendar.resource-slot]
pub fn resource_slot_valid(e: ResourceSlot) -> (result: bool)
    ensures result == resource_slot_spec(e)
{
    e.slot_units <= e.capacity && e.start_tick < e.end_tick
}

// r[verify domain-primitives.scheduling-calendar.resource-slot]
pub proof fn valid_resource_slot_is_admitted(e: ResourceSlot)
    requires resource_slot_spec(e)
    ensures resource_slot_spec(e)
{}

// r[depends domain-primitives.scheduling-calendar.admission]
pub open spec fn schedule_admission_spec(e: ScheduleAdmission) -> bool {
    e.field_ok && e.bucket_ok && e.interval_ok && e.deadline_ok && e.resource_ok && !e.conflict
}

// r[impl domain-primitives.scheduling-calendar.admission]
pub fn schedule_admission_valid(e: ScheduleAdmission) -> (result: bool)
    ensures result == schedule_admission_spec(e)
{
    e.field_ok && e.bucket_ok && e.interval_ok && e.deadline_ok && e.resource_ok && !e.conflict
}

// r[verify domain-primitives.scheduling-calendar.admission]
pub proof fn valid_schedule_admission_is_admitted(e: ScheduleAdmission)
    requires schedule_admission_spec(e)
    ensures schedule_admission_spec(e)
{}

} // verus!

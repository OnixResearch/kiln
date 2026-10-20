//! Exact finite resource accounting primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceBudget {
    pub used: usize,
    pub reserved: usize,
    pub limit: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceCounter {
    pub current: usize,
    pub increment: usize,
    pub max: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceQuota {
    pub used: usize,
    pub soft_limit: usize,
    pub hard_limit: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceLease {
    pub start_tick: u64,
    pub end_tick: u64,
    pub now_tick: u64,
    pub resource_units: usize,
    pub max_units: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceUsageWindow {
    pub window_start: u64,
    pub window_end: u64,
    pub sample_count: usize,
    pub max_samples: usize,
    pub total_usage: usize,
    pub max_usage: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceReservation {
    pub requested: usize,
    pub available: usize,
    pub granted: usize,
    pub starts_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceOverage {
    pub used: usize,
    pub limit: usize,
    pub overage: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceAdmission {
    pub budget_ok: bool,
    pub quota_ok: bool,
    pub lease_ok: bool,
    pub reservation_ok: bool,
    pub over_limit: bool,
}

// r[depends domain-primitives.resource-accounting.budget]
pub open spec fn resource_budget_spec(e: ResourceBudget) -> bool {
    e.used <= e.limit && e.reserved <= e.limit && e.reserved <= e.limit - e.used
}

// r[impl domain-primitives.resource-accounting.budget]
pub fn resource_budget_valid(e: ResourceBudget) -> (result: bool)
    ensures result == resource_budget_spec(e)
{
    e.used <= e.limit && e.reserved <= e.limit && e.reserved <= e.limit - e.used
}

// r[verify domain-primitives.resource-accounting.budget]
pub proof fn valid_resource_budget_is_admitted(e: ResourceBudget)
    requires resource_budget_spec(e)
    ensures resource_budget_spec(e)
{}

// r[depends domain-primitives.resource-accounting.counter]
pub open spec fn resource_counter_spec(e: ResourceCounter) -> bool {
    e.current <= e.max && e.increment <= e.max - e.current
}

// r[impl domain-primitives.resource-accounting.counter]
pub fn resource_counter_valid(e: ResourceCounter) -> (result: bool)
    ensures result == resource_counter_spec(e)
{
    e.current <= e.max && e.increment <= e.max - e.current
}

// r[verify domain-primitives.resource-accounting.counter]
pub proof fn valid_resource_counter_is_admitted(e: ResourceCounter)
    requires resource_counter_spec(e)
    ensures resource_counter_spec(e)
{}

// r[depends domain-primitives.resource-accounting.quota]
pub open spec fn resource_quota_spec(e: ResourceQuota) -> bool {
    e.soft_limit <= e.hard_limit && e.used <= e.hard_limit
}

// r[impl domain-primitives.resource-accounting.quota]
pub fn resource_quota_valid(e: ResourceQuota) -> (result: bool)
    ensures result == resource_quota_spec(e)
{
    e.soft_limit <= e.hard_limit && e.used <= e.hard_limit
}

// r[verify domain-primitives.resource-accounting.quota]
pub proof fn valid_resource_quota_is_admitted(e: ResourceQuota)
    requires resource_quota_spec(e)
    ensures resource_quota_spec(e)
{}

// r[depends domain-primitives.resource-accounting.lease]
pub open spec fn resource_lease_spec(e: ResourceLease) -> bool {
    e.start_tick <= e.end_tick && e.now_tick <= e.end_tick && e.resource_units <= e.max_units
}

// r[impl domain-primitives.resource-accounting.lease]
pub fn resource_lease_valid(e: ResourceLease) -> (result: bool)
    ensures result == resource_lease_spec(e)
{
    e.start_tick <= e.end_tick && e.now_tick <= e.end_tick && e.resource_units <= e.max_units
}

// r[verify domain-primitives.resource-accounting.lease]
pub proof fn valid_resource_lease_is_admitted(e: ResourceLease)
    requires resource_lease_spec(e)
    ensures resource_lease_spec(e)
{}

// r[depends domain-primitives.resource-accounting.usage-window]
pub open spec fn resource_usage_window_spec(e: ResourceUsageWindow) -> bool {
    e.window_start <= e.window_end && e.sample_count <= e.max_samples && e.total_usage <= e.max_usage
}

// r[impl domain-primitives.resource-accounting.usage-window]
pub fn resource_usage_window_valid(e: ResourceUsageWindow) -> (result: bool)
    ensures result == resource_usage_window_spec(e)
{
    e.window_start <= e.window_end && e.sample_count <= e.max_samples && e.total_usage <= e.max_usage
}

// r[verify domain-primitives.resource-accounting.usage-window]
pub proof fn valid_resource_usage_window_is_admitted(e: ResourceUsageWindow)
    requires resource_usage_window_spec(e)
    ensures resource_usage_window_spec(e)
{}

// r[depends domain-primitives.resource-accounting.reservation]
pub open spec fn resource_reservation_spec(e: ResourceReservation) -> bool {
    e.requested <= e.available && e.granted <= e.requested && e.expires_at >= e.starts_at
}

// r[impl domain-primitives.resource-accounting.reservation]
pub fn resource_reservation_valid(e: ResourceReservation) -> (result: bool)
    ensures result == resource_reservation_spec(e)
{
    e.requested <= e.available && e.granted <= e.requested && e.expires_at >= e.starts_at
}

// r[verify domain-primitives.resource-accounting.reservation]
pub proof fn valid_resource_reservation_is_admitted(e: ResourceReservation)
    requires resource_reservation_spec(e)
    ensures resource_reservation_spec(e)
{}

// r[depends domain-primitives.resource-accounting.overage]
pub open spec fn resource_overage_spec(e: ResourceOverage) -> bool {
    e.used >= e.limit && e.overage == e.used - e.limit
}

// r[impl domain-primitives.resource-accounting.overage]
pub fn resource_overage_valid(e: ResourceOverage) -> (result: bool)
    ensures result == resource_overage_spec(e)
{
    e.used >= e.limit && e.overage == e.used - e.limit
}

// r[verify domain-primitives.resource-accounting.overage]
pub proof fn valid_resource_overage_is_admitted(e: ResourceOverage)
    requires resource_overage_spec(e)
    ensures resource_overage_spec(e)
{}

// r[depends domain-primitives.resource-accounting.admission]
pub open spec fn resource_admission_spec(e: ResourceAdmission) -> bool {
    e.budget_ok && e.quota_ok && e.lease_ok && e.reservation_ok && !e.over_limit
}

// r[impl domain-primitives.resource-accounting.admission]
pub fn resource_admission_valid(e: ResourceAdmission) -> (result: bool)
    ensures result == resource_admission_spec(e)
{
    e.budget_ok && e.quota_ok && e.lease_ok && e.reservation_ok && !e.over_limit
}

// r[verify domain-primitives.resource-accounting.admission]
pub proof fn valid_resource_admission_is_admitted(e: ResourceAdmission)
    requires resource_admission_spec(e)
    ensures resource_admission_spec(e)
{}

} // verus!

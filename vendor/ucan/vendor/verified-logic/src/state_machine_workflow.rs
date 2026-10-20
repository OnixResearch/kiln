//! Exact finite state machine workflow primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StateSetEnvelope {
    pub state_count: usize,
    pub initial_state: usize,
    pub max_states: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TransitionRelationEnvelope {
    pub from_state: usize,
    pub to_state: usize,
    pub state_count: usize,
    pub transition_count: usize,
    pub max_transitions: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EventEnvelope {
    pub event_id: u64,
    pub max_event_id: u64,
    pub payload_len: usize,
    pub max_payload_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GuardPredicateEnvelope {
    pub guard_id: u64,
    pub max_guard_id: u64,
    pub input_len: usize,
    pub max_input_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TransitionAdmissionEnvelope {
    pub state_ok: bool,
    pub transition_ok: bool,
    pub event_ok: bool,
    pub guard_ok: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WorkflowStepEnvelope {
    pub step_id: u64,
    pub max_step_id: u64,
    pub state_id: usize,
    pub state_count: usize,
    pub duration_ticks: u64,
    pub max_duration_ticks: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WorkflowDependencyEnvelope {
    pub predecessor: usize,
    pub successor: usize,
    pub step_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReachabilityPathEnvelope {
    pub path_len: usize,
    pub max_path_len: usize,
    pub start_state: usize,
    pub end_state: usize,
    pub state_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TerminalStateEnvelope {
    pub state_id: usize,
    pub state_count: usize,
    pub outgoing_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TransitionTraceEnvelope {
    pub step_count: usize,
    pub max_steps: usize,
    pub first_state: usize,
    pub last_state: usize,
    pub state_count: usize,
}

// r[depends domain-primitives.state-machine-workflow.state]
pub open spec fn state_set_envelope_spec(e: StateSetEnvelope) -> bool {
    e.state_count > 0 && e.initial_state < e.state_count && e.state_count <= e.max_states
}

// r[impl domain-primitives.state-machine-workflow.state]
pub fn state_set_envelope_valid(e: StateSetEnvelope) -> (result: bool)
    ensures result == state_set_envelope_spec(e)
{
    e.state_count > 0 && e.initial_state < e.state_count && e.state_count <= e.max_states
}

// r[verify domain-primitives.state-machine-workflow.state]
pub proof fn valid_state_set_envelope_is_admitted(e: StateSetEnvelope)
    requires state_set_envelope_spec(e)
    ensures state_set_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.transition]
pub open spec fn transition_relation_envelope_spec(e: TransitionRelationEnvelope) -> bool {
    e.from_state < e.state_count && e.to_state < e.state_count && e.transition_count <= e.max_transitions
}

// r[impl domain-primitives.state-machine-workflow.transition]
pub fn transition_relation_envelope_valid(e: TransitionRelationEnvelope) -> (result: bool)
    ensures result == transition_relation_envelope_spec(e)
{
    e.from_state < e.state_count && e.to_state < e.state_count && e.transition_count <= e.max_transitions
}

// r[verify domain-primitives.state-machine-workflow.transition]
pub proof fn valid_transition_relation_envelope_is_admitted(e: TransitionRelationEnvelope)
    requires transition_relation_envelope_spec(e)
    ensures transition_relation_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.event]
pub open spec fn event_envelope_spec(e: EventEnvelope) -> bool {
    e.event_id <= e.max_event_id && e.payload_len <= e.max_payload_len
}

// r[impl domain-primitives.state-machine-workflow.event]
pub fn event_envelope_valid(e: EventEnvelope) -> (result: bool)
    ensures result == event_envelope_spec(e)
{
    e.event_id <= e.max_event_id && e.payload_len <= e.max_payload_len
}

// r[verify domain-primitives.state-machine-workflow.event]
pub proof fn valid_event_envelope_is_admitted(e: EventEnvelope)
    requires event_envelope_spec(e)
    ensures event_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.guard]
pub open spec fn guard_predicate_envelope_spec(e: GuardPredicateEnvelope) -> bool {
    e.guard_id <= e.max_guard_id && e.input_len <= e.max_input_len
}

// r[impl domain-primitives.state-machine-workflow.guard]
pub fn guard_predicate_envelope_valid(e: GuardPredicateEnvelope) -> (result: bool)
    ensures result == guard_predicate_envelope_spec(e)
{
    e.guard_id <= e.max_guard_id && e.input_len <= e.max_input_len
}

// r[verify domain-primitives.state-machine-workflow.guard]
pub proof fn valid_guard_predicate_envelope_is_admitted(e: GuardPredicateEnvelope)
    requires guard_predicate_envelope_spec(e)
    ensures guard_predicate_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.admission]
pub open spec fn transition_admission_envelope_spec(e: TransitionAdmissionEnvelope) -> bool {
    e.state_ok && e.transition_ok && e.event_ok && e.guard_ok && e.enabled
}

// r[impl domain-primitives.state-machine-workflow.admission]
pub fn transition_admission_envelope_valid(e: TransitionAdmissionEnvelope) -> (result: bool)
    ensures result == transition_admission_envelope_spec(e)
{
    e.state_ok && e.transition_ok && e.event_ok && e.guard_ok && e.enabled
}

// r[verify domain-primitives.state-machine-workflow.admission]
pub proof fn valid_transition_admission_envelope_is_admitted(e: TransitionAdmissionEnvelope)
    requires transition_admission_envelope_spec(e)
    ensures transition_admission_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.step]
pub open spec fn workflow_step_envelope_spec(e: WorkflowStepEnvelope) -> bool {
    e.step_id <= e.max_step_id && e.state_id < e.state_count && e.duration_ticks <= e.max_duration_ticks
}

// r[impl domain-primitives.state-machine-workflow.step]
pub fn workflow_step_envelope_valid(e: WorkflowStepEnvelope) -> (result: bool)
    ensures result == workflow_step_envelope_spec(e)
{
    e.step_id <= e.max_step_id && e.state_id < e.state_count && e.duration_ticks <= e.max_duration_ticks
}

// r[verify domain-primitives.state-machine-workflow.step]
pub proof fn valid_workflow_step_envelope_is_admitted(e: WorkflowStepEnvelope)
    requires workflow_step_envelope_spec(e)
    ensures workflow_step_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.dependency]
pub open spec fn workflow_dependency_envelope_spec(e: WorkflowDependencyEnvelope) -> bool {
    e.predecessor < e.step_count && e.successor < e.step_count && e.predecessor != e.successor
}

// r[impl domain-primitives.state-machine-workflow.dependency]
pub fn workflow_dependency_envelope_valid(e: WorkflowDependencyEnvelope) -> (result: bool)
    ensures result == workflow_dependency_envelope_spec(e)
{
    e.predecessor < e.step_count && e.successor < e.step_count && e.predecessor != e.successor
}

// r[verify domain-primitives.state-machine-workflow.dependency]
pub proof fn valid_workflow_dependency_envelope_is_admitted(e: WorkflowDependencyEnvelope)
    requires workflow_dependency_envelope_spec(e)
    ensures workflow_dependency_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.reachability]
pub open spec fn reachability_path_envelope_spec(e: ReachabilityPathEnvelope) -> bool {
    e.path_len <= e.max_path_len && e.start_state < e.state_count && e.end_state < e.state_count
}

// r[impl domain-primitives.state-machine-workflow.reachability]
pub fn reachability_path_envelope_valid(e: ReachabilityPathEnvelope) -> (result: bool)
    ensures result == reachability_path_envelope_spec(e)
{
    e.path_len <= e.max_path_len && e.start_state < e.state_count && e.end_state < e.state_count
}

// r[verify domain-primitives.state-machine-workflow.reachability]
pub proof fn valid_reachability_path_envelope_is_admitted(e: ReachabilityPathEnvelope)
    requires reachability_path_envelope_spec(e)
    ensures reachability_path_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.terminal]
pub open spec fn terminal_state_envelope_spec(e: TerminalStateEnvelope) -> bool {
    e.state_id < e.state_count && e.outgoing_count == 0
}

// r[impl domain-primitives.state-machine-workflow.terminal]
pub fn terminal_state_envelope_valid(e: TerminalStateEnvelope) -> (result: bool)
    ensures result == terminal_state_envelope_spec(e)
{
    e.state_id < e.state_count && e.outgoing_count == 0
}

// r[verify domain-primitives.state-machine-workflow.terminal]
pub proof fn valid_terminal_state_envelope_is_admitted(e: TerminalStateEnvelope)
    requires terminal_state_envelope_spec(e)
    ensures terminal_state_envelope_spec(e)
{}

// r[depends domain-primitives.state-machine-workflow.trace]
pub open spec fn transition_trace_envelope_spec(e: TransitionTraceEnvelope) -> bool {
    e.step_count <= e.max_steps && e.first_state < e.state_count && e.last_state < e.state_count
}

// r[impl domain-primitives.state-machine-workflow.trace]
pub fn transition_trace_envelope_valid(e: TransitionTraceEnvelope) -> (result: bool)
    ensures result == transition_trace_envelope_spec(e)
{
    e.step_count <= e.max_steps && e.first_state < e.state_count && e.last_state < e.state_count
}

// r[verify domain-primitives.state-machine-workflow.trace]
pub proof fn valid_transition_trace_envelope_is_admitted(e: TransitionTraceEnvelope)
    requires transition_trace_envelope_spec(e)
    ensures transition_trace_envelope_spec(e)
{}

} // verus!

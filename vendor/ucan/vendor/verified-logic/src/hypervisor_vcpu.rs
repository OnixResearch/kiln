//! Verified vCPU lifecycle state machine.
//!
//! Four abstract states: `Runnable`, `Halted`, `Blocked`, `Stopped`.
//! Only runnable vCPUs may enter guest mode.  `Stopped` is terminal.
//! Halt puts a runnable vCPU to sleep; a deliverable wake event resumes it.

use vstd::prelude::*;

verus! {

// State encoding: 0=Runnable, 1=Halted, 2=Blocked, 3=Stopped.

pub const VCPU_RUNNABLE: u64 = 0;
pub const VCPU_HALTED:   u64 = 1;
pub const VCPU_BLOCKED:  u64 = 2;
pub const VCPU_STOPPED:  u64 = 3;

// ────────────────────────────────────────────────────────────────────
//  is_valid_vcpu_state
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-vcpu.run-state]
pub open spec fn is_valid_vcpu_state_spec(state: u64) -> bool {
    state <= 3
}

// r[impl hypervisor-vcpu.run-state]
pub fn is_valid_vcpu_state(state: u64) -> (result: bool)
    ensures result == is_valid_vcpu_state_spec(state)
{
    state <= 3
}

// ────────────────────────────────────────────────────────────────────
//  vcpu_can_enter_guest
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-vcpu.run-state]
/// Spec: only runnable vCPUs may enter guest mode.
pub open spec fn vcpu_can_enter_guest_spec(state: u64) -> bool {
    state == VCPU_RUNNABLE
}

// r[impl hypervisor-vcpu.run-state]
pub fn vcpu_can_enter_guest(state: u64) -> (result: bool)
    ensures result == vcpu_can_enter_guest_spec(state)
{
    state == VCPU_RUNNABLE
}

// r[verify hypervisor-vcpu.run-state]
/// Only runnable can enter guest.
proof fn only_runnable_enters()
    ensures
        vcpu_can_enter_guest_spec(VCPU_RUNNABLE),
        !vcpu_can_enter_guest_spec(VCPU_HALTED),
        !vcpu_can_enter_guest_spec(VCPU_BLOCKED),
        !vcpu_can_enter_guest_spec(VCPU_STOPPED),
{}

// ────────────────────────────────────────────────────────────────────
//  vcpu_halt — Runnable → Halted
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-vcpu.halt-wakeup]
/// Spec: halt from Runnable moves to Halted.
pub open spec fn vcpu_halt_spec(state: u64) -> u64 {
    if state == VCPU_RUNNABLE { VCPU_HALTED } else { state }
}

// r[impl hypervisor-vcpu.halt-wakeup]
pub fn vcpu_halt(state: u64) -> (result: u64)
    ensures result == vcpu_halt_spec(state)
{
    if state == VCPU_RUNNABLE { VCPU_HALTED } else { state }
}

// r[verify hypervisor-vcpu.halt-wakeup]
/// Halt from Runnable produces Halted.
proof fn halt_runnable_is_halted()
    ensures vcpu_halt_spec(VCPU_RUNNABLE) == VCPU_HALTED
{}

// r[verify hypervisor-vcpu.halt-wakeup]
/// Halt is idempotent on non-runnable states.
proof fn halt_non_runnable_unchanged(state: u64)
    requires state != VCPU_RUNNABLE
    ensures vcpu_halt_spec(state) == state
{}

// ────────────────────────────────────────────────────────────────────
//  vcpu_wake — Halted → Runnable on deliverable event
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-vcpu.halt-wakeup]
/// Spec: wake a halted vCPU if a deliverable event is present.
pub open spec fn vcpu_wake_spec(state: u64, deliverable: bool) -> u64 {
    if state == VCPU_HALTED && deliverable { VCPU_RUNNABLE } else { state }
}

// r[impl hypervisor-vcpu.halt-wakeup]
pub fn vcpu_wake(state: u64, deliverable: bool) -> (result: u64)
    ensures result == vcpu_wake_spec(state, deliverable)
{
    if state == VCPU_HALTED && deliverable { VCPU_RUNNABLE } else { state }
}

// r[verify hypervisor-vcpu.halt-wakeup]
/// Deliverable event wakes halted vCPU.
proof fn wake_halted_with_event()
    ensures vcpu_wake_spec(VCPU_HALTED, true) == VCPU_RUNNABLE
{}

// r[verify hypervisor-vcpu.halt-wakeup]
/// No event keeps halted vCPU halted.
proof fn no_event_stays_halted()
    ensures vcpu_wake_spec(VCPU_HALTED, false) == VCPU_HALTED
{}

// ────────────────────────────────────────────────────────────────────
//  vcpu_stop — terminal transition
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-vcpu.run-state]
/// Spec: stop moves any non-stopped vCPU to Stopped.
pub open spec fn vcpu_stop_spec(state: u64) -> u64 {
    VCPU_STOPPED
}

// r[impl hypervisor-vcpu.run-state]
pub fn vcpu_stop(_state: u64) -> (result: u64)
    ensures result == vcpu_stop_spec(_state)
{
    VCPU_STOPPED
}

// r[verify hypervisor-vcpu.run-state]
/// Stopped is terminal: stop of stopped is still stopped.
proof fn stopped_is_terminal()
    ensures vcpu_stop_spec(VCPU_STOPPED) == VCPU_STOPPED
{}

// r[verify hypervisor-vcpu.run-state]
/// Stopped vCPU cannot enter guest.
proof fn stopped_cannot_run()
    ensures !vcpu_can_enter_guest_spec(VCPU_STOPPED)
{}

// r[verify hypervisor-vcpu.run-state]
/// Wake does not revive a stopped vCPU.
proof fn wake_does_not_revive_stopped()
    ensures
        vcpu_wake_spec(VCPU_STOPPED, true) == VCPU_STOPPED,
        vcpu_wake_spec(VCPU_STOPPED, false) == VCPU_STOPPED,
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[test hypervisor-vcpu.run-state]
    #[test]
    fn test_runnable_enters_guest() {
        assert!(vcpu_can_enter_guest(VCPU_RUNNABLE));
    }

    // r[test hypervisor-vcpu.run-state]
    #[test]
    fn test_blocked_cannot_enter() {
        assert!(!vcpu_can_enter_guest(VCPU_BLOCKED));
    }

    // r[test hypervisor-vcpu.halt-wakeup]
    #[test]
    fn test_halt_then_wake() {
        let halted = vcpu_halt(VCPU_RUNNABLE);
        assert_eq!(halted, VCPU_HALTED);
        let woken = vcpu_wake(halted, true);
        assert_eq!(woken, VCPU_RUNNABLE);
    }

    // r[test hypervisor-vcpu.halt-wakeup]
    #[test]
    fn test_no_wake_without_event() {
        assert_eq!(vcpu_wake(VCPU_HALTED, false), VCPU_HALTED);
    }

    // r[test hypervisor-vcpu.run-state]
    #[test]
    fn test_stopped_is_terminal() {
        assert_eq!(vcpu_stop(VCPU_RUNNABLE), VCPU_STOPPED);
        assert!(!vcpu_can_enter_guest(VCPU_STOPPED));
        assert_eq!(vcpu_wake(VCPU_STOPPED, true), VCPU_STOPPED);
    }
}

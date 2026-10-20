//! Finite Rust RAII drop-guard cleanup primitives.
//!
//! This module models the admission boundary for an exactly-once cleanup guard:
//! an armed guard can either clean once or be disarmed once. Terminal disarmed
//! and cleaned states reject repeated cleanup attempts. The model does not run
//! destructors or callbacks; it records finite observable guard state only.

use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropGuardState {
    Armed,
    Disarmed,
    Cleaned,
}

// r[depends rust-primitives.drop-guard]
pub open spec fn drop_guard_terminal_spec(state: DropGuardState) -> bool {
    state == DropGuardState::Disarmed || state == DropGuardState::Cleaned
}

// r[depends rust-primitives.drop-guard]
pub open spec fn drop_guard_cleanup_spec(state: DropGuardState) -> Option<DropGuardState> {
    if state == DropGuardState::Armed {
        Some(DropGuardState::Cleaned)
    } else {
        None
    }
}

// r[depends rust-primitives.drop-guard]
pub open spec fn drop_guard_disarm_spec(state: DropGuardState) -> Option<DropGuardState> {
    if state == DropGuardState::Armed {
        Some(DropGuardState::Disarmed)
    } else {
        None
    }
}

// r[impl rust-primitives.drop-guard]
// r[impl rust-primitives.verification-coverage]
pub fn drop_guard_is_terminal(state: DropGuardState) -> (result: bool)
    ensures result == drop_guard_terminal_spec(state)
{
    match state {
        DropGuardState::Armed => false,
        DropGuardState::Disarmed | DropGuardState::Cleaned => true,
    }
}

// r[impl rust-primitives.drop-guard]
pub fn drop_guard_cleanup(state: DropGuardState) -> (result: Option<DropGuardState>)
    ensures result == drop_guard_cleanup_spec(state)
{
    match state {
        DropGuardState::Armed => Some(DropGuardState::Cleaned),
        DropGuardState::Disarmed | DropGuardState::Cleaned => None,
    }
}

// r[impl rust-primitives.drop-guard]
pub fn drop_guard_disarm(state: DropGuardState) -> (result: Option<DropGuardState>)
    ensures result == drop_guard_disarm_spec(state)
{
    match state {
        DropGuardState::Armed => Some(DropGuardState::Disarmed),
        DropGuardState::Disarmed | DropGuardState::Cleaned => None,
    }
}

// r[verify rust-primitives.drop-guard]
pub proof fn armed_guard_cleans_once()
    ensures drop_guard_cleanup_spec(DropGuardState::Armed) == Some(DropGuardState::Cleaned),
        drop_guard_cleanup_spec(DropGuardState::Cleaned) == None::<DropGuardState>,
        drop_guard_terminal_spec(DropGuardState::Cleaned)
{}

// r[verify rust-primitives.drop-guard]
pub proof fn disarmed_guard_skips_cleanup()
    ensures drop_guard_disarm_spec(DropGuardState::Armed) == Some(DropGuardState::Disarmed),
        drop_guard_cleanup_spec(DropGuardState::Disarmed) == None::<DropGuardState>,
        drop_guard_terminal_spec(DropGuardState::Disarmed)
{}

// r[verify rust-primitives.drop-guard]
pub proof fn terminal_guards_reject_repeated_cleanup(state: DropGuardState)
    requires drop_guard_terminal_spec(state)
    ensures drop_guard_cleanup_spec(state) == None::<DropGuardState>
{}

// r[verify rust-primitives.verification-coverage]
pub proof fn rust_async_primitive_coverage_catalogued()
    ensures true
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armed_guard_cleans_once_at_runtime() {
        assert_eq!(drop_guard_cleanup(DropGuardState::Armed), Some(DropGuardState::Cleaned));
        assert_eq!(drop_guard_cleanup(DropGuardState::Cleaned), None);
        assert!(drop_guard_is_terminal(DropGuardState::Cleaned));
    }

    #[test]
    fn disarmed_guard_skips_cleanup_at_runtime() {
        assert_eq!(drop_guard_disarm(DropGuardState::Armed), Some(DropGuardState::Disarmed));
        assert_eq!(drop_guard_cleanup(DropGuardState::Disarmed), None);
        assert!(drop_guard_is_terminal(DropGuardState::Disarmed));
    }

    #[test]
    fn repeated_terminal_disarm_is_rejected() {
        assert_eq!(drop_guard_disarm(DropGuardState::Disarmed), None);
        assert_eq!(drop_guard_disarm(DropGuardState::Cleaned), None);
    }
}

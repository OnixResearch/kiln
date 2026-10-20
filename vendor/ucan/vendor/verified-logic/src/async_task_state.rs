//! Finite Rust async task-state lifecycle primitives.
//!
//! This module models the small admission boundary used by later async
//! primitives: a task starts pending, may become ready, and terminal completed
//! or cancelled states reject later mutation. It is a finite state model only;
//! it does not implement polling, callbacks, scheduler fairness, or executor
//! behavior.

use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsyncTaskState {
    Pending,
    Ready,
    Completed,
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AsyncTaskAction {
    MarkReady,
    Complete,
    Cancel,
}

// r[depends rust-primitives.async-task-state]
pub open spec fn async_task_terminal_spec(state: AsyncTaskState) -> bool {
    state == AsyncTaskState::Completed || state == AsyncTaskState::Cancelled
}

// r[depends rust-primitives.async-task-state]
pub open spec fn async_task_transition_spec(
    state: AsyncTaskState,
    action: AsyncTaskAction,
) -> Option<AsyncTaskState> {
    if state == AsyncTaskState::Pending {
        if action == AsyncTaskAction::MarkReady {
            Some(AsyncTaskState::Ready)
        } else if action == AsyncTaskAction::Complete {
            None
        } else {
            Some(AsyncTaskState::Cancelled)
        }
    } else if state == AsyncTaskState::Ready {
        if action == AsyncTaskAction::MarkReady {
            Some(AsyncTaskState::Ready)
        } else if action == AsyncTaskAction::Complete {
            Some(AsyncTaskState::Completed)
        } else {
            Some(AsyncTaskState::Cancelled)
        }
    } else {
        None
    }
}

// r[impl rust-primitives.async-task-state]
pub fn async_task_is_terminal(state: AsyncTaskState) -> (result: bool)
    ensures result == async_task_terminal_spec(state)
{
    match state {
        AsyncTaskState::Pending | AsyncTaskState::Ready => false,
        AsyncTaskState::Completed | AsyncTaskState::Cancelled => true,
    }
}

// r[impl rust-primitives.async-task-state]
pub fn async_task_transition(
    state: AsyncTaskState,
    action: AsyncTaskAction,
) -> (result: Option<AsyncTaskState>)
    ensures result == async_task_transition_spec(state, action)
{
    match state {
        AsyncTaskState::Pending => match action {
            AsyncTaskAction::MarkReady => Some(AsyncTaskState::Ready),
            AsyncTaskAction::Complete => None,
            AsyncTaskAction::Cancel => Some(AsyncTaskState::Cancelled),
        },
        AsyncTaskState::Ready => match action {
            AsyncTaskAction::MarkReady => Some(AsyncTaskState::Ready),
            AsyncTaskAction::Complete => Some(AsyncTaskState::Completed),
            AsyncTaskAction::Cancel => Some(AsyncTaskState::Cancelled),
        },
        AsyncTaskState::Completed | AsyncTaskState::Cancelled => None,
    }
}

// r[impl rust-primitives.async-task-state]
pub fn async_task_mark_ready(state: AsyncTaskState) -> (result: Option<AsyncTaskState>)
    ensures result == async_task_transition_spec(state, AsyncTaskAction::MarkReady)
{
    async_task_transition(state, AsyncTaskAction::MarkReady)
}

// r[impl rust-primitives.async-task-state]
pub fn async_task_complete(state: AsyncTaskState) -> (result: Option<AsyncTaskState>)
    ensures result == async_task_transition_spec(state, AsyncTaskAction::Complete)
{
    async_task_transition(state, AsyncTaskAction::Complete)
}

// r[impl rust-primitives.async-task-state]
pub fn async_task_cancel(state: AsyncTaskState) -> (result: Option<AsyncTaskState>)
    ensures result == async_task_transition_spec(state, AsyncTaskAction::Cancel)
{
    async_task_transition(state, AsyncTaskAction::Cancel)
}

// r[verify rust-primitives.async-task-state]
pub proof fn ready_task_can_complete()
    ensures async_task_transition_spec(AsyncTaskState::Ready, AsyncTaskAction::Complete)
        == Some(AsyncTaskState::Completed),
        async_task_terminal_spec(AsyncTaskState::Completed)
{}

// r[verify rust-primitives.async-task-state]
pub proof fn completed_task_rejects_later_cancellation()
    ensures async_task_transition_spec(AsyncTaskState::Completed, AsyncTaskAction::Cancel) == None::<AsyncTaskState>,
        async_task_terminal_spec(AsyncTaskState::Completed)
{}

// r[verify rust-primitives.async-task-state]
pub proof fn cancelled_task_rejects_later_ready()
    ensures async_task_transition_spec(AsyncTaskState::Cancelled, AsyncTaskAction::MarkReady) == None::<AsyncTaskState>,
        async_task_terminal_spec(AsyncTaskState::Cancelled)
{}

// r[verify rust-primitives.async-task-state]
pub proof fn admitted_terminal_successors_are_terminal(
    state: AsyncTaskState,
    action: AsyncTaskAction,
    next: AsyncTaskState,
)
    requires async_task_transition_spec(state, action) == Some(next),
        next == AsyncTaskState::Completed || next == AsyncTaskState::Cancelled,
    ensures async_task_terminal_spec(next)
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_task_can_complete_at_runtime() {
        assert_eq!(async_task_complete(AsyncTaskState::Ready), Some(AsyncTaskState::Completed));
        assert!(async_task_is_terminal(AsyncTaskState::Completed));
    }

    #[test]
    fn pending_task_can_become_ready_then_cancelled() {
        assert_eq!(async_task_mark_ready(AsyncTaskState::Pending), Some(AsyncTaskState::Ready));
        assert_eq!(async_task_cancel(AsyncTaskState::Ready), Some(AsyncTaskState::Cancelled));
        assert!(async_task_is_terminal(AsyncTaskState::Cancelled));
    }

    #[test]
    fn completed_task_rejects_later_cancellation() {
        assert_eq!(async_task_cancel(AsyncTaskState::Completed), None);
    }

    #[test]
    fn pending_task_cannot_complete_without_readiness() {
        assert_eq!(async_task_complete(AsyncTaskState::Pending), None);
    }
}

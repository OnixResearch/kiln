//! Verified seL4-style reply-cap one-shot authority.
//!
//! Models reply capabilities as a small lifecycle FSM carrying fixed
//! caller/callee pairing. A live reply cap authorizes exactly one matching
//! reply, then becomes consumed. Cancel and revoke invalidate a live reply cap
//! without changing the stored caller/callee identity.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone)]
pub enum ReplyState {
    Live,
    Consumed,
    Invalidated,
}

#[derive(Copy, Clone)]
pub struct ReplyCap {
    pub caller: usize,
    pub callee: usize,
    pub state: ReplyState,
}

// r[impl sel4-reply.pairing]
// r[depends sel4-reply.pairing]
pub open spec fn pair_matches_spec(cap: ReplyCap, caller: int, callee: int) -> bool {
    &&& 0 <= caller
    &&& 0 <= callee
    &&& cap.caller == caller
    &&& cap.callee == callee
}

// r[impl sel4-reply.one-shot]
// r[impl sel4-reply.pairing]
// r[depends sel4-reply.one-shot]
// r[depends sel4-reply.pairing]
pub open spec fn can_reply_spec(cap: ReplyCap, caller: int, callee: int) -> bool {
    &&& pair_matches_spec(cap, caller, callee)
    &&& match cap.state {
        ReplyState::Live => true,
        ReplyState::Consumed => false,
        ReplyState::Invalidated => false,
    }
}

// r[impl sel4-reply.one-shot]
// r[impl sel4-reply.pairing]
// r[depends sel4-reply.one-shot]
// r[depends sel4-reply.pairing]
pub open spec fn reply_transition_spec(cap: ReplyCap, caller: int, callee: int) -> ReplyCap {
    if can_reply_spec(cap, caller, callee) {
        ReplyCap {
            caller: cap.caller,
            callee: cap.callee,
            state: ReplyState::Consumed,
        }
    } else {
        cap
    }
}

// r[impl sel4-reply.invalidate]
// r[depends sel4-reply.invalidate]
pub open spec fn cancel_transition_spec(cap: ReplyCap) -> ReplyCap {
    match cap.state {
        ReplyState::Live => ReplyCap {
            caller: cap.caller,
            callee: cap.callee,
            state: ReplyState::Invalidated,
        },
        ReplyState::Consumed => cap,
        ReplyState::Invalidated => cap,
    }
}

// r[impl sel4-reply.invalidate]
// r[depends sel4-reply.invalidate]
pub open spec fn revoke_transition_spec(cap: ReplyCap) -> ReplyCap {
    cancel_transition_spec(cap)
}

// r[impl sel4-reply.one-shot]
// r[impl sel4-reply.pairing]
pub fn new_reply_cap(caller: usize, callee: usize) -> (result: ReplyCap)
    ensures
        result.caller == caller,
        result.callee == callee,
        matches!(result.state, ReplyState::Live),
        can_reply_spec(result, caller as int, callee as int),
{
    ReplyCap {
        caller,
        callee,
        state: ReplyState::Live,
    }
}

// r[impl sel4-reply.one-shot]
// r[impl sel4-reply.pairing]
pub fn reply(cap: &mut ReplyCap, caller: usize, callee: usize) -> (result: bool)
    ensures
        *cap == reply_transition_spec(*old(cap), caller as int, callee as int),
        result == can_reply_spec(*old(cap), caller as int, callee as int),
{
    if cap.caller == caller && cap.callee == callee {
        match cap.state {
            ReplyState::Live => {
                cap.state = ReplyState::Consumed;
                true
            }
            ReplyState::Consumed | ReplyState::Invalidated => false,
        }
    } else {
        false
    }
}

// r[impl sel4-reply.invalidate]
pub fn cancel(cap: &mut ReplyCap)
    ensures
        *cap == cancel_transition_spec(*old(cap)),
{
    match cap.state {
        ReplyState::Live => {
            cap.state = ReplyState::Invalidated;
        }
        ReplyState::Consumed | ReplyState::Invalidated => {}
    }
}

// r[impl sel4-reply.invalidate]
pub fn revoke(cap: &mut ReplyCap)
    ensures
        *cap == revoke_transition_spec(*old(cap)),
{
    cancel(cap);
}

// r[verify sel4-reply.one-shot]
proof fn first_reply_consumes_live_cap(caller: usize, callee: usize)
    ensures
        !can_reply_spec(
            reply_transition_spec(
                ReplyCap { caller, callee, state: ReplyState::Live },
                caller as int,
                callee as int,
            ),
            caller as int,
            callee as int,
        ),
{
}

// r[verify sel4-reply.pairing]
proof fn cross_pair_reply_rejected(caller: usize, callee: usize, other_caller: usize, other_callee: usize)
    requires
        other_caller != caller || other_callee != callee,
    ensures
        !can_reply_spec(
            ReplyCap { caller, callee, state: ReplyState::Live },
            other_caller as int,
            other_callee as int,
        ),
        reply_transition_spec(
            ReplyCap { caller, callee, state: ReplyState::Live },
            other_caller as int,
            other_callee as int,
        ) == (ReplyCap { caller, callee, state: ReplyState::Live }),
{
}

// r[verify sel4-reply.invalidate]
proof fn cancel_and_revoke_invalidate_live_cap(caller: usize, callee: usize)
    ensures
        !can_reply_spec(
            cancel_transition_spec(ReplyCap { caller, callee, state: ReplyState::Live }),
            caller as int,
            callee as int,
        ),
        !can_reply_spec(
            revoke_transition_spec(ReplyCap { caller, callee, state: ReplyState::Live }),
            caller as int,
            callee as int,
        ),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-reply.one-shot]
    #[test]
    fn test_first_reply_succeeds_second_denied() {
        let mut cap = new_reply_cap(1, 2);
        assert!(reply(&mut cap, 1, 2));
        assert!(matches!(cap.state, ReplyState::Consumed), "reply cap should be consumed");
        assert!(!reply(&mut cap, 1, 2));
    }

    // r[verify sel4-reply.pairing]
    #[test]
    fn test_cross_pair_reply_rejected() {
        let mut cap = new_reply_cap(3, 4);
        assert!(!reply(&mut cap, 9, 4));
        assert!(matches!(cap.state, ReplyState::Live), "mismatched reply should not change cap state");
        assert!(!reply(&mut cap, 3, 9));
    }

    // r[verify sel4-reply.invalidate]
    #[test]
    fn test_cancel_invalidates_live_cap() {
        let mut cap = new_reply_cap(5, 6);
        cancel(&mut cap);
        assert!(matches!(cap.state, ReplyState::Invalidated), "cancel should invalidate live cap");
        assert!(!reply(&mut cap, 5, 6));
    }

    // r[verify sel4-reply.invalidate]
    #[test]
    fn test_revoke_invalidates_live_cap() {
        let mut cap = new_reply_cap(7, 8);
        revoke(&mut cap);
        assert!(matches!(cap.state, ReplyState::Invalidated), "revoke should invalidate live cap");
        assert!(!reply(&mut cap, 7, 8));
    }

    proptest! {
        // r[verify sel4-reply.one-shot]
        #[test]
        fn prop_one_shot_reply(caller: usize, callee: usize) {
            let mut cap = new_reply_cap(caller, callee);
            prop_assert!(reply(&mut cap, caller, callee));
            prop_assert!(!reply(&mut cap, caller, callee));
        }

        // r[verify sel4-reply.pairing]
        #[test]
        fn prop_mismatched_reply_rejected(caller: usize, callee: usize, other_caller: usize, other_callee: usize) {
            prop_assume!(other_caller != caller || other_callee != callee);
            let mut cap = new_reply_cap(caller, callee);
            prop_assert!(!reply(&mut cap, other_caller, other_callee));
            prop_assert!(matches!(cap.state, ReplyState::Live), "mismatched reply should not change cap state");
        }

        // r[verify sel4-reply.invalidate]
        #[test]
        fn prop_cancelled_cap_cannot_reply(caller: usize, callee: usize) {
            let mut cap = new_reply_cap(caller, callee);
            cancel(&mut cap);
            prop_assert!(!reply(&mut cap, caller, callee));
            prop_assert!(matches!(cap.state, ReplyState::Invalidated), "cancelled cap should stay invalidated");
        }
    }
}

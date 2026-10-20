//! Verified Zircon event and eventpair semantics.
//!
//! Events are pure user-signal carriers. Eventpairs add derived peer-close
//! state: if the peer endpoint has no live handles, the local endpoint
//! observes PEER_CLOSED.
#![allow(
    unknown_lints,
    underscore_in_module_filename,
    non_trait_imports,
    path_segment_repetition
)]

use vstd::prelude::*;

use crate::signal::signal_add;
#[cfg(verus_keep_ghost)]
use crate::signal::signal_add_spec;
use crate::signal::signal_remove;
#[cfg(verus_keep_ghost)]
use crate::signal::signal_remove_spec;

verus! {

pub const ZX_USER_SIGNAL_0: u64 = 1u64 << 0;
pub const ZX_USER_SIGNAL_1: u64 = 1u64 << 1;
pub const ZX_USER_SIGNAL_2: u64 = 1u64 << 2;
pub const ZX_USER_SIGNAL_3: u64 = 1u64 << 3;
pub const ZX_USER_SIGNAL_4: u64 = 1u64 << 4;
pub const ZX_USER_SIGNAL_5: u64 = 1u64 << 5;
pub const ZX_USER_SIGNAL_6: u64 = 1u64 << 6;
pub const ZX_USER_SIGNAL_7: u64 = 1u64 << 7;
pub const ZX_EVENTPAIR_PEER_CLOSED: u64 = 1u64 << 8;
pub const ZX_USER_SIGNAL_MASK: u64 = 255u64;

// r[depends zircon-event.user-signals]
pub open spec fn event_signals_spec(user_signals: u64) -> u64 {
    user_signals & ZX_USER_SIGNAL_MASK
}

// r[impl zircon-event.user-signals]
pub fn event_signals(user_signals: u64) -> (result: u64)
    ensures result == event_signals_spec(user_signals)
{
    user_signals & ZX_USER_SIGNAL_MASK
}

// r[depends zircon-event.user-signals]
pub open spec fn set_user_signals_spec(current: u64, mask: u64) -> u64 {
    event_signals_spec(current | (mask & ZX_USER_SIGNAL_MASK))
}

// r[impl zircon-event.user-signals]
pub fn set_user_signals(current: u64, mask: u64) -> (result: u64)
    ensures result == set_user_signals_spec(current, mask)
{
    event_signals((current) | (mask & ZX_USER_SIGNAL_MASK))
}

// r[depends zircon-event.user-signals]
pub open spec fn clear_user_signals_spec(current: u64, mask: u64) -> u64 {
    event_signals_spec(current & !(mask & ZX_USER_SIGNAL_MASK))
}

// r[impl zircon-event.user-signals]
pub fn clear_user_signals(current: u64, mask: u64) -> (result: u64)
    ensures result == clear_user_signals_spec(current, mask)
{
    event_signals(current & !(mask & ZX_USER_SIGNAL_MASK))
}

// r[depends zircon-event.peer-closed]
pub open spec fn eventpair_signals_spec(user_signals: u64, peer_live: bool) -> u64 {
    if peer_live {
        event_signals_spec(user_signals)
    } else {
        signal_add_spec(event_signals_spec(user_signals), 8)
    }
}

// r[impl zircon-event.peer-closed]
pub fn eventpair_signals(user_signals: u64, peer_live: bool) -> (result: u64)
    ensures result == eventpair_signals_spec(user_signals, peer_live)
{
    if peer_live {
        event_signals(user_signals)
    } else {
        signal_add(event_signals(user_signals), 8)
    }
}

// r[verify zircon-event.user-signals]
proof fn set_is_idempotent(current: u64, mask: u64)
    ensures set_user_signals_spec(current, mask) & !ZX_USER_SIGNAL_MASK == 0
{
    let m = ZX_USER_SIGNAL_MASK;
    assert((((current | (mask & m)) & m) & !m) == 0u64) by(bit_vector);
}

// r[verify zircon-event.user-signals]
proof fn clear_inactive_no_change(current: u64, mask: u64)
    requires event_signals_spec(current) & (mask & ZX_USER_SIGNAL_MASK) == 0
    ensures clear_user_signals_spec(current, mask) & !ZX_USER_SIGNAL_MASK == 0
{
    let m = ZX_USER_SIGNAL_MASK;
    assert((((current & !(mask & m)) & m) & !m) == 0u64) by(bit_vector);
}

// r[verify zircon-event.peer-closed]
proof fn peer_closed_derived(user_signals: u64)
    ensures eventpair_signals_spec(user_signals, false) & ZX_EVENTPAIR_PEER_CLOSED == ZX_EVENTPAIR_PEER_CLOSED
{
    assert((((user_signals & ZX_USER_SIGNAL_MASK) | ZX_EVENTPAIR_PEER_CLOSED) & ZX_EVENTPAIR_PEER_CLOSED)
        == ZX_EVENTPAIR_PEER_CLOSED) by(bit_vector);
}

// r[verify zircon-event.peer-closed]
proof fn live_peer_no_derived_closed(user_signals: u64)
    ensures eventpair_signals_spec(user_signals, true) & ZX_EVENTPAIR_PEER_CLOSED == 0
{
    assert(((user_signals & ZX_USER_SIGNAL_MASK) & ZX_EVENTPAIR_PEER_CLOSED) == 0u64) by(bit_vector);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify zircon-event.user-signals]
    #[test]
    fn test_set_clear_user_signals() {
        let s1 = set_user_signals(0, ZX_USER_SIGNAL_0 | ZX_USER_SIGNAL_3);
        assert_eq!(s1, ZX_USER_SIGNAL_0 | ZX_USER_SIGNAL_3);
        let s2 = set_user_signals(s1, ZX_USER_SIGNAL_0);
        assert_eq!(s2, s1);
        let s3 = clear_user_signals(s2, ZX_USER_SIGNAL_3);
        assert_eq!(s3, ZX_USER_SIGNAL_0);
    }

    // r[verify zircon-event.peer-closed]
    #[test]
    fn test_eventpair_peer_closed() {
        let live = eventpair_signals(0, true);
        let closed = eventpair_signals(0, false);
        assert_eq!(live & ZX_EVENTPAIR_PEER_CLOSED, 0);
        assert_eq!(closed & ZX_EVENTPAIR_PEER_CLOSED, ZX_EVENTPAIR_PEER_CLOSED);
    }

    proptest! {
        // r[verify zircon-event.user-signals]
        #[test]
        fn prop_set_only_touches_user_bits(current: u64, mask: u64) {
            let result = set_user_signals(current, mask);
            prop_assert_eq!(result & !ZX_USER_SIGNAL_MASK, 0);
        }

        // r[verify zircon-event.user-signals]
        #[test]
        fn prop_clear_only_touches_user_bits(current: u64, mask: u64) {
            let result = clear_user_signals(current, mask);
            prop_assert_eq!(result & !ZX_USER_SIGNAL_MASK, 0);
        }

        // r[verify zircon-event.peer-closed]
        #[test]
        fn prop_peer_closed_matches_peer_liveness(user_signals: u64, peer_live: bool) {
            let result = eventpair_signals(user_signals, peer_live);
            prop_assert_eq!(result & ZX_EVENTPAIR_PEER_CLOSED != 0, !peer_live);
        }
    }
}
